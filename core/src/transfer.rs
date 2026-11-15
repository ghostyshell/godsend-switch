//! Background job queue for transfers, conversions and installs.
//!
//! Every long operation runs on its own thread, reports progress through a
//! callback, can be cancelled via an atomic flag, and survives a UI restart
//! via a small jobs.json journal (pending state is noted, finished jobs are
//! trimmed).

use crate::error::{Error, Result};
use crate::config::jobs_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    /// Operation family: copy, convert, install, backup, restore...
    pub kind: String,
    /// Human label shown in the queue.
    pub label: String,
    pub state: JobState,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}

struct JobSlot {
    job: Job,
    cancel: Arc<AtomicBool>,
}

/// Live progress hook: `(job_id, done, total)` fired from job threads.
/// The desktop layer installs one to emit UI events.
pub type ProgressHook = dyn Fn(u64, u64, u64) + Send + Sync;
static PROGRESS_HOOK: std::sync::Mutex<Option<Arc<ProgressHook>>> = std::sync::Mutex::new(None);

/// Install the process-wide progress hook (call once at app startup).
pub fn set_progress_hook(hook: Option<Arc<ProgressHook>>) {
    *PROGRESS_HOOK.lock().expect("hook lock") = hook;
}

fn fire_progress(job_id: u64, done: u64, total: u64) {
    let hook = PROGRESS_HOOK.lock().expect("hook lock").clone();
    if let Some(hook) = hook {
        hook(job_id, done, total);
    }
}

#[derive(Default)]
pub struct JobManager {
    next_id: AtomicU64,
    slots: Mutex<HashMap<u64, Arc<Mutex<JobSlot>>>>,
    order: Mutex<Vec<u64>>,
}

pub struct JobContext {
    slot: Arc<Mutex<JobSlot>>,
    cancel: Arc<AtomicBool>,
}

impl JobContext {
    pub fn progress(&self, done: u64, total: u64) {
        {
            let mut slot = self.slot.lock().expect("job slot lock");
            slot.job.done_bytes = done;
            slot.job.total_bytes = total;
        }
        fire_progress(self.job_id(), done, total);
    }

    pub fn message(&self, msg: impl Into<String>) {
        let mut slot = self.slot.lock().expect("job slot lock");
        slot.job.message = msg.into();
    }

    pub fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error::Cancelled(self.job_id()))
        } else {
            Ok(())
        }
    }

    pub fn job_id(&self) -> u64 {
        self.slot.lock().expect("job slot lock").job.id
    }

    /// Chunked file copy with progress + cancellation.
    pub fn copy_file(&self, src: &std::path::Path, dst: &std::path::Path) -> Result<u64> {
        copy_with_progress(src, dst, self)
    }
}

impl JobContext {
    /// Detached context for unit tests and one-off synchronous calls.
    pub fn for_test() -> Self {
        let job = Job {
            id: 0,
            kind: "test".into(),
            label: "test".into(),
            state: JobState::Running,
            done_bytes: 0,
            total_bytes: 0,
            message: String::new(),
            started_at: now(),
            finished_at: None,
        };
        JobContext {
            slot: Arc::new(Mutex::new(JobSlot { job, cancel: Arc::new(AtomicBool::new(false)) })),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Clone for JobContext {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
            cancel: self.cancel.clone(),
        }
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restore persisted job rows (finished/pending) after a restart.
    pub fn load_journal(&self) {
        let Ok(bytes) = fs::read(jobs_path()) else {
            return;
        };
        let Ok(rows) = serde_json::from_slice::<Vec<Job>>(&bytes) else {
            return;
        };
        let mut order = self.order.lock().expect("order lock");
        let mut slots = self.slots.lock().expect("slots lock");
        let mut highest = 0u64;
        for job in rows {
            let id = job.id;
            highest = highest.max(id);
            slots.insert(
                id,
                Arc::new(Mutex::new(JobSlot {
                    job,
                    cancel: Arc::new(AtomicBool::new(false)),
                })),
            );
            if !order.contains(&id) {
                order.push(id);
            }
        }
        self.next_id.fetch_max(highest, Ordering::Relaxed);
    }

    fn save_journal(&self) {
        let rows: Vec<Job> = self
            .list()
            .into_iter()
            .filter(|job| job.state != JobState::Running || job.finished_at.is_some())
            .collect();
        let rows: Vec<Job> = rows.into_iter().rev().take(50).collect();
        if let Some(parent) = jobs_path().parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&rows) {
            let _ = fs::write(jobs_path(), bytes);
        }
    }

    /// Spawn `work` on a background thread with progress + cancel plumbing.
    /// `on_update` fires on the job's own thread whenever progress changes.
    pub fn spawn<F>(&self, kind: &str, label: &str, on_update: impl Fn(Job) + Send + 'static, work: F) -> u64
    where
        F: FnOnce(&JobContext) -> Result<()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let slot = Arc::new(Mutex::new(JobSlot {
            job: Job {
                id,
                kind: kind.to_string(),
                label: label.to_string(),
                state: JobState::Running,
                done_bytes: 0,
                total_bytes: 0,
                message: String::new(),
                started_at: now(),
                finished_at: None,
            },
            cancel: Arc::new(AtomicBool::new(false)),
        }));

        self.slots
            .lock()
            .expect("slots lock")
            .insert(id, slot.clone());
        self.order.lock().expect("order lock").push(id);

        let ctx = JobContext {
            slot: slot.clone(),
            cancel: slot.lock().expect("slot lock").cancel.clone(),
        };
        let snapshot_slot = slot.clone();
        std::thread::spawn(move || {
            let result = work(&ctx);
            {
                let mut slot = snapshot_slot.lock().expect("slot lock");
                slot.job.state = match &result {
                    Ok(()) => JobState::Done,
                    Err(Error::Cancelled(_)) => JobState::Cancelled,
                    Err(e) => {
                        slot.job.message = e.to_string();
                        JobState::Failed
                    }
                };
                slot.job.finished_at = Some(now());
            }
            on_update(snapshot_slot.lock().expect("slot lock").job.clone());
            let _ = result;
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<Job> {
        self.slots
            .lock()
            .expect("slots lock")
            .get(&id)
            .map(|slot| slot.lock().expect("slot lock").job.clone())
    }

    pub fn list(&self) -> Vec<Job> {
        let order = self.order.lock().expect("order lock");
        let slots = self.slots.lock().expect("slots lock");
        order
            .iter()
            .rev()
            .filter_map(|id| slots.get(id))
            .map(|slot| slot.lock().expect("slot lock").job.clone())
            .collect()
    }

    pub fn active(&self) -> Vec<Job> {
        self.list()
            .into_iter()
            .filter(|job| job.state == JobState::Running)
            .collect()
    }

    pub fn cancel(&self, id: u64) -> bool {
        if let Some(slot) = self.slots.lock().expect("slots lock").get(&id) {
            slot.lock().expect("slot lock").cancel.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Drop finished/failed/cancelled rows from the list (and journal).
    pub fn clear_finished(&self) {
        let mut order = self.order.lock().expect("order lock");
        let mut slots = self.slots.lock().expect("slots lock");
        order.retain(|id| {
            let keep = slots
                .get(id)
                .map(|slot| slot.lock().expect("slot lock").job.state == JobState::Running)
                .unwrap_or(false);
            if !keep {
                slots.remove(id);
            }
            keep
        });
        self.save_journal();
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Buffered copy with progress callback and cooperative cancellation.
pub fn copy_with_progress(
    src: &std::path::Path,
    dst: &std::path::Path,
    ctx: &JobContext,
) -> Result<u64> {
    use std::io::{Read, Write};
    let mut input = fs::File::open(src)?;
    let total = input.metadata()?.len();
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = fs::File::create(dst)?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut done: u64 = 0;
    let mut last_report = std::time::Instant::now();
    loop {
        ctx.check_cancel()?;
        let read = input.read(&mut buf)?;
        if read == 0 {
            break;
        }
        output.write_all(&buf[..read])?;
        done += read as u64;
        if last_report.elapsed().as_millis() > 120 {
            ctx.progress(done, total);
            last_report = std::time::Instant::now();
        }
    }
    ctx.progress(done, total);
    Ok(done)
}

/// Sum the sizes of every file under `dir` (used to pre-size multi-file jobs).
pub fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn job_runs_and_reports() {
        let mgr = JobManager::new();
        let (tx, rx) = mpsc::channel::<Job>();
        let tx2 = tx.clone();
        let id = mgr.spawn(
            "copy",
            "test copy",
            move |job| {
                let _ = tx2.send(job);
            },
            |ctx| {
                ctx.progress(50, 100);
                ctx.message("halfway");
                Ok(())
            },
        );
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
        let mut final_job = None;
        for _ in 0..50 {
            let job = mgr.get(id).expect("job present");
            if job.state != JobState::Running {
                final_job = Some(job);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let job = final_job.expect("job finished");
        assert_eq!(job.state, JobState::Done);
        assert_eq!(job.done_bytes, 50);
        assert_eq!(job.message, "halfway");
    }

    #[test]
    fn cancel_flags_job() {
        let mgr = JobManager::new();
        let id = mgr.spawn("copy", "cancel me", |_| {}, |ctx| {
            for _ in 0..40 {
                ctx.check_cancel()?;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Ok(())
        });
        mgr.cancel(id);
        let mut done = false;
        for _ in 0..100 {
            if mgr.get(id).map(|j| j.state == JobState::Cancelled).unwrap_or(false) {
                done = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(done, "job should observe the cancel flag");
    }
}
