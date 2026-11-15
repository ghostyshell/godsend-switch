//! Tauri command layer over `godsend-core`, plus the app-hosted MCP server.
//!
//! Long work runs in the job queue and reports through `job://progress` and
//! `job://job`. The MCP endpoint starts with the app on 127.0.0.1.

use godsend_core::config::{Config, Host, InstallTarget};
use godsend_core::doctor::{self, Check};
use godsend_core::fm;
use godsend_core::homebrew::{self, CatalogEntry};
use godsend_core::install;
use godsend_core::library::{self, Entry};
use godsend_core::mcp;
use godsend_core::mtp::{MtpSession, RemoteEntry, StorageOut};
use godsend_core::nsp::{self, FileInfo};
use godsend_core::sources::{self, GameHit, IaFile, IaItem};
use godsend_core::store;
use godsend_core::syscon::{self, Button, PadState};
use godsend_core::transfer::{set_progress_hook, Job, JobContext, JobManager};
use godsend_core::device::{self, DeviceStatus};
use godsend_core::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    jobs: JobManager,
    config: Mutex<Config>,
    /// The running MCP server: its port and stop flag.
    mcp: Mutex<Option<(u16, Arc<AtomicBool>)>>,
}

impl AppState {
    fn new() -> Self {
        let jobs = JobManager::new();
        jobs.load_journal();
        Self {
            jobs,
            config: Mutex::new(Config::load()),
            mcp: Mutex::new(None),
        }
    }

    /// Start the app-hosted MCP server for the given config.
    fn start_mcp(&self, config: &Config) {
        self.stop_mcp();
        if !config.mcp_enabled {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let mcp_config = config.clone();
        let port = config.mcp_port;
        std::thread::spawn(move || {
            if let Err(e) = mcp::serve_http(mcp_config, port, worker_stop) {
                eprintln!("MCP server: {e}");
            }
        });
        *self.mcp.lock().expect("mcp lock") = Some((port, stop));
    }

    fn stop_mcp(&self) {
        if let Some((_, stop)) = self.mcp.lock().expect("mcp lock").take() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    fn mcp_port(&self) -> Option<u16> {
        self.mcp.lock().expect("mcp lock").as_ref().map(|(port, _)| *port)
    }

    fn config(&self) -> Config {
        self.config.lock().expect("config lock").clone()
    }

    fn replace_config(&self, next: Config) -> Result<Config> {
        next.save()?;
        *self.config.lock().expect("config lock") = next.clone();
        Ok(next)
    }
}

#[derive(Serialize)]
struct Snapshot {
    hosts: Vec<Host>,
    active: Option<String>,
    config: Config,
    status: DeviceStatus,
    doctor: Vec<Check>,
    jobs: Vec<Job>,
    mcp_port: u16,
    mcp_enabled: bool,
}

fn emit_job(app: &AppHandle, job: Job) {
    let _ = app.emit("job://job", &job);
}

fn spawn_job(
    app: &AppHandle,
    state: &AppState,
    kind: &str,
    label: &str,
    work: impl FnOnce(&JobContext) -> Result<()> + Send + 'static,
) -> u64 {
    let handle = app.clone();
    state.jobs.spawn(kind, label, move |job| emit_job(&handle, job), work)
}

#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> Result<Snapshot> {
    let config = state.config();
    let status = device::probe(&config);
    let checks = doctor::checks(&config, &status);
    Ok(Snapshot {
        hosts: config.hosts.clone(),
        active: config.active_host.clone(),
        mcp_port: config.mcp_port,
        mcp_enabled: config.mcp_enabled,
        doctor: checks,
        status,
        jobs: state.jobs.list(),
        config,
    })
}

#[tauri::command]
fn probe(state: State<'_, AppState>) -> DeviceStatus {
    device::probe(&state.config())
}

#[tauri::command]
async fn discover(subnet: String, port: u16) -> Result<Vec<Host>> {
    let port = if port == 0 { 5000 } else { port };
    tauri::async_runtime::spawn_blocking(move || {
        device::discover(&subnet, port, "anonymous", "")
    })
    .await
    .map_err(|e| Error::Other(format!("discovery task: {e}")))?
}

#[tauri::command]
fn host_add(state: State<'_, AppState>, mut host: Host) -> Result<Config> {
    let mut config = state.config();
    host.refresh_id();
    config.hosts.retain(|h| h.id != host.id);
    config.hosts.push(host.clone());
    if config.active_host.is_none() {
        config.active_host = Some(host.id);
    }
    state.replace_config(config)
}

#[tauri::command]
fn host_remove(state: State<'_, AppState>, id: String) -> Result<Config> {
    let mut config = state.config();
    config.hosts.retain(|h| h.id != id);
    if config.active_host.as_deref() == Some(id.as_str()) {
        config.active_host = config.hosts.first().map(|h| h.id.clone());
    }
    state.replace_config(config)
}

#[tauri::command]
fn host_set_active(state: State<'_, AppState>, id: String) -> Result<Config> {
    let mut config = state.config();
    config.active_host = Some(id);
    state.replace_config(config)
}

#[tauri::command]
fn storages(state: State<'_, AppState>) -> Result<Vec<StorageOut>> {
    let session = MtpSession::open(&state.config().mtp_serial)?;
    Ok(session.storages())
}

#[tauri::command]
fn library(state: State<'_, AppState>, source: String) -> Result<Vec<Entry>> {
    let config = state.config();
    if source == "local" {
        let dir = godsend_core::aria2::download_dir(&config);
        return Ok(library::local(&dir, &config.covers_base));
    }
    let session = MtpSession::open(&config.mtp_serial)?;
    library::installed(&session, &config.covers_base)
}

#[tauri::command]
fn list_storage(state: State<'_, AppState>, storage: String, path: String) -> Result<Vec<RemoteEntry>> {
    let kind = mcp::kind_from_str(&storage)?;
    let session = MtpSession::open(&state.config().mtp_serial)?;
    session.list(kind, if path.is_empty() { "/" } else { &path })
}

#[tauri::command]
fn storage_mkdir(state: State<'_, AppState>, storage: String, path: String) -> Result<()> {
    let kind = mcp::kind_from_str(&storage)?;
    let session = MtpSession::open(&state.config().mtp_serial)?;
    session.mkdir(kind, &path)
}

#[tauri::command]
fn storage_delete(state: State<'_, AppState>, storage: String, path: String) -> Result<()> {
    let kind = mcp::kind_from_str(&storage)?;
    let session = MtpSession::open(&state.config().mtp_serial)?;
    session.delete(kind, &path)
}

#[tauri::command]
fn upload_local(
    app: AppHandle,
    state: State<'_, AppState>,
    local: String,
    storage: String,
    dir: String,
) -> Result<u64> {
    let config = state.config();
    let kind = mcp::kind_from_str(&storage)?;
    let label = Path::new(&local)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(spawn_job(&app, &state, "upload", &label, move |ctx| {
        let session = MtpSession::open(&config.mtp_serial)?;
        fm::mtp_upload(&session, kind, &dir, Path::new(&local), ctx)?;
        Ok(())
    }))
}

#[tauri::command]
fn download_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    storage: String,
    path: String,
    local_dir: String,
) -> Result<u64> {
    let config = state.config();
    let kind = mcp::kind_from_str(&storage)?;
    let label = path.rsplit('/').next().unwrap_or("download").to_string();
    Ok(spawn_job(&app, &state, "download", &label, move |ctx| {
        std::fs::create_dir_all(&local_dir)?;
        let session = MtpSession::open(&config.mtp_serial)?;
        let target = Path::new(&local_dir).join(local_name(&path));
        session.download(kind, &path, &target, ctx)?;
        Ok(())
    }))
}

fn local_name(remote: &str) -> String {
    remote.rsplit('/').next().unwrap_or("download").to_string()
}

#[tauri::command]
fn install_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    via: Option<String>,
) -> Result<u64> {
    let label = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut config = state.config();
    if let Some(via) = via.as_deref() {
        config.install_target = match via {
            "sd" => InstallTarget::Sd,
            "nand" => InstallTarget::Nand,
            "tinfoil" => InstallTarget::Tinfoil,
            "network" => InstallTarget::Network,
            other => return Err(Error::Invalid(format!("unknown target {other}"))),
        };
    }
    Ok(spawn_job(&app, &state, "install", &label, move |ctx| {
        install::install(&config, Path::new(&path), ctx).map(|_| ())
    }))
}

#[tauri::command]
fn tinfoil_attached() -> Option<godsend_core::tinfoil::UsbInfo> {
    godsend_core::tinfoil::attached()
}

#[tauri::command]
fn pad_set(addr: String, port: u16, buttons: Vec<String>, left: Option<[f32; 2]>, right: Option<[f32; 2]>) -> Result<()> {
    let mut state = PadState::default();
    for name in buttons {
        state.buttons |= Button::parse(&name)?.mask();
    }
    if let Some([x, y]) = left {
        state = state.with_stick("l", x, y)?;
    }
    if let Some([x, y]) = right {
        state = state.with_stick("r", x, y)?;
    }
    syscon::send(&addr, port, &state)
}

#[tauri::command]
fn pad_release(addr: String, port: u16) -> Result<()> {
    syscon::disconnect(&addr, port)
}

#[tauri::command]
fn homebrew_catalog(state: State<'_, AppState>, refresh: bool) -> Vec<CatalogEntry> {
    homebrew::catalog(&state.config(), refresh)
}

#[tauri::command]
fn homebrew_install(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<u64> {
    let config = state.config();
    let label = format!("homebrew {id}");
    Ok(spawn_job(&app, &state, "homebrew", &label, move |ctx| {
        homebrew::install(&config, &id, ctx).map(|_| ())
    }))
}

#[tauri::command]
fn store_search(state: State<'_, AppState>, query: String, limit: u32) -> Result<Vec<GameHit>> {
    sources::gamesearch(&state.config().gamesearch_base, &query, if limit == 0 { 20 } else { limit })
}

#[tauri::command]
fn ia_search(query: String, rows: u32) -> Result<Vec<IaItem>> {
    sources::ia_search(&query, if rows == 0 { 15 } else { rows })
}

#[tauri::command]
fn ia_files(id: String) -> Result<Vec<IaFile>> {
    sources::ia_item_files(&id)
}

#[tauri::command]
fn store_download(app: AppHandle, state: State<'_, AppState>, slug: String) -> Result<u64> {
    let config = state.config();
    let label = slug.clone();
    Ok(spawn_job(&app, &state, "download", &label, move |ctx| {
        let hit = sources::gamesearch_detail(&config.gamesearch_base, &slug)?;
        store::download_hit(&hit, &config, ctx).map(|_| ())
    }))
}

#[tauri::command]
fn download_url(app: AppHandle, state: State<'_, AppState>, url: String, name: String) -> Result<u64> {
    let config = state.config();
    let label = name.clone();
    Ok(spawn_job(&app, &state, "download", &label, move |ctx| {
        let dir = godsend_core::aria2::download_dir(&config);
        godsend_core::aria2::download(&url, &dir, &name, &config, ctx).map(|_| ())
    }))
}

#[tauri::command]
fn downloads_list(state: State<'_, AppState>) -> Result<Vec<fm::LocalEntry>> {
    let dir = godsend_core::aria2::download_dir(&state.config());
    std::fs::create_dir_all(&dir)?;
    fm::local_list(&dir)
}

#[tauri::command]
fn download_open(path: String) -> Result<()> {
    let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(target_os = "windows") { "explorer" } else { "xdg-open" };
    std::process::Command::new(opener).arg(path).spawn()?;
    Ok(())
}

#[tauri::command]
fn download_delete(path: String) -> Result<()> {
    fm::local_delete(Path::new(&path))
}

#[tauri::command]
fn download_push(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<u64> {
    install_path(app, state, path, None)
}

#[tauri::command]
fn local_root(state: State<'_, AppState>) -> String {
    godsend_core::aria2::download_dir(&state.config()).to_string_lossy().to_string()
}

#[tauri::command]
fn fm_local(dir: String) -> Result<Vec<fm::LocalEntry>> {
    fm::local_list(Path::new(&dir))
}

#[tauri::command]
fn fm_mkdir(dir: String) -> Result<()> {
    fm::local_mkdir(Path::new(&dir))
}

#[tauri::command]
fn fm_delete(path: String) -> Result<()> {
    fm::local_delete(Path::new(&path))
}

#[tauri::command]
fn convert_inspect(path: String) -> Result<FileInfo> {
    nsp::info(Path::new(&path))
}

#[tauri::command]
fn convert_split(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<u64> {
    let chunk = state.config().split_bytes;
    let label = local_name(&path);
    Ok(spawn_job(&app, &state, "split", &label, move |ctx| {
        nsp::split(Path::new(&path), chunk, ctx).map(|_| ())
    }))
}

#[tauri::command]
fn convert_merge(app: AppHandle, state: State<'_, AppState>, base: String, out: String) -> Result<u64> {
    let label = local_name(&base);
    Ok(spawn_job(&app, &state, "merge", &label, move |ctx| {
        let parts = nsp::find_parts(Path::new(&base));
        nsp::merge(&parts, Path::new(&out), ctx).map(|_| ())
    }))
}

#[tauri::command]
fn config_get(state: State<'_, AppState>) -> Config {
    state.config()
}

#[tauri::command]
fn config_set(state: State<'_, AppState>, patch: serde_json::Value) -> Result<Config> {
    let before = state.config();
    let mut config = before.clone();
    config.patch(patch)?;
    let next = state.replace_config(config)?;
    if before.mcp_enabled != next.mcp_enabled || before.mcp_port != next.mcp_port {
        state.start_mcp(&next);
    }
    Ok(next)
}

#[tauri::command]
fn doctor_run(state: State<'_, AppState>) -> Result<Vec<Check>> {
    Ok(doctor::run(&state.config()))
}

#[tauri::command]
fn jobs(state: State<'_, AppState>) -> Vec<Job> {
    state.jobs.list()
}

#[tauri::command]
fn job_cancel(state: State<'_, AppState>, id: u64) -> bool {
    state.jobs.cancel(id)
}

#[tauri::command]
fn jobs_clear(state: State<'_, AppState>) {
    state.jobs.clear_finished()
}

#[tauri::command]
fn mcp_info(state: State<'_, AppState>) -> (bool, u16) {
    match state.mcp_port() {
        Some(port) => (true, port),
        None => (false, state.config().mcp_port),
    }
}

#[tauri::command]
fn open_data_dir() -> Result<()> {
    let dir = godsend_core::config::data_dir();
    std::fs::create_dir_all(&dir)?;
    download_open(dir.to_string_lossy().to_string())
}

#[tauri::command]
fn reveal(path: String) -> Result<()> {
    download_open(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = AppState::new();
            state.start_mcp(&state.config());
            let handle = app.handle().clone();
            set_progress_hook(Some(Arc::new(move |id, done, total| {
                let _ = handle.emit("job://progress", &serde_json::json!({"id": id, "done": done, "total": total}));
            })));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            probe,
            discover,
            host_add,
            host_remove,
            host_set_active,
            storages,
            library,
            list_storage,
            storage_mkdir,
            storage_delete,
            upload_local,
            download_remote,
            install_path,
            tinfoil_attached,
            pad_set,
            pad_release,
            homebrew_catalog,
            homebrew_install,
            store_search,
            ia_search,
            ia_files,
            store_download,
            download_url,
            downloads_list,
            download_open,
            download_delete,
            download_push,
            local_root,
            fm_local,
            fm_mkdir,
            fm_delete,
            convert_inspect,
            convert_split,
            convert_merge,
            config_get,
            config_set,
            doctor_run,
            jobs,
            job_cancel,
            jobs_clear,
            mcp_info,
            open_data_dir,
            reveal
        ])
        .run(tauri::generate_context!())
        .expect("error while running GODsend Switch");
}

#[allow(dead_code)]
fn unused(_: PathBuf) {}
