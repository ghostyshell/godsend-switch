//! MTP transport: the one USB path a stock console exposes.
//!
//! DBI, Sphaira and similar responders run an MTP server over USB. The app is
//! the host. Storage units carry the operations: the SD card and NAND are
//! browsable, "Installed games" lists what is installed, and the "SD install"
//! and "NAND install" units install whatever file is dropped into them.

use crate::error::{Error, Result};
use crate::transfer::JobContext;
use mtp_rs::mtp::{ByteRange, MtpDevice, NewObjectInfo, Storage};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::io::ReaderStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Sd,
    NandUser,
    NandSystem,
    SdInstall,
    NandInstall,
    Installed,
    Saves,
    Album,
    Gamecard,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Sd => "SD card",
            Kind::NandUser => "NAND USER",
            Kind::NandSystem => "NAND SYSTEM",
            Kind::SdInstall => "SD install",
            Kind::NandInstall => "NAND install",
            Kind::Installed => "Installed games",
            Kind::Saves => "Saves",
            Kind::Album => "Album",
            Kind::Gamecard => "Gamecard",
            Kind::Other => "Other",
        }
    }
}

/// Classify a responder's storage description.
pub fn kind_of(description: &str) -> Kind {
    let d = description.to_ascii_lowercase();
    if d.contains("installed") || d.contains("games") {
        return Kind::Installed;
    }
    if d.contains("install") {
        if d.contains("nand") {
            return Kind::NandInstall;
        }
        return Kind::SdInstall;
    }
    if d.contains("save") {
        return Kind::Saves;
    }
    if d.contains("album") || d.contains("photo") {
        return Kind::Album;
    }
    if d.contains("gamecard") || d.contains("game card") {
        return Kind::Gamecard;
    }
    if d.contains("nand system") {
        return Kind::NandSystem;
    }
    if d.contains("nand") {
        return Kind::NandUser;
    }
    if d.contains("sd") || d.contains("microsd") || d.contains("card") {
        return Kind::Sd;
    }
    Kind::Other
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageOut {
    pub id: u64,
    pub description: String,
    pub kind: Kind,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub writable: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceOut {
    pub serial: String,
    pub product: String,
    pub manufacturer: String,
    pub storages: Vec<StorageOut>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

pub struct MtpSession {
    rt: tokio::runtime::Runtime,
    device: MtpDevice,
    storages: Vec<Storage>,
    /// Only one process-wide MTP session may exist at a time: the console has
    /// one responder and concurrent claims fail.
    _device_lock: std::sync::MutexGuard<'static, ()>,
}

fn device_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<&'static std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let lock = LOCK.get_or_init(|| Box::leak(Box::new(std::sync::Mutex::new(()))));
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl MtpSession {
    /// Open the responder with the configured serial, else the first
    /// Nintendo-vendor device, else whatever MTP device is attached.
    pub fn open(preferred: &str) -> Result<Self> {
        let device_lock = device_lock();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| Error::Other(format!("tokio: {e}")))?;
        let device = {
            let preferred = preferred.trim().to_string();
            let nintendo = if preferred.is_empty() {
                MtpDevice::list_devices()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|info| info.vendor_id == 0x057E)
                    .map(|info| info.location_id)
            } else {
                None
            };
            rt.block_on(async move {
                if !preferred.is_empty() {
                    return MtpDevice::open_by_serial(&preferred).await;
                }
                match nintendo {
                    Some(location) => MtpDevice::open_by_location(location).await,
                    None => MtpDevice::open_first().await,
                }
            })
            .map_err(|e| Error::device(format!(
                "no MTP responder found ({e}); start DBI's MTP responder and connect USB"
            )))?
        };
        let storages = rt
            .block_on(device.storages())
            .map_err(|e| Error::Other(format!("MTP storages: {e}")))?;
        Ok(Self { rt, device, storages, _device_lock: device_lock })
    }

    /// True when any MTP responder is currently attached.
    pub fn attached(preferred: &str) -> bool {
        if !preferred.trim().is_empty() {
            return true;
        }
        MtpDevice::list_devices().map(|d| !d.is_empty()).unwrap_or(false)
    }

    pub fn describe(&self) -> DeviceOut {
        let info = self.device.device_info();
        DeviceOut {
            serial: info.serial_number.clone(),
            product: info.model.clone(),
            manufacturer: info.manufacturer.clone(),
            storages: self.storage_rows(),
        }
    }

    fn storage_rows(&self) -> Vec<StorageOut> {
        self.storages
            .iter()
            .map(|s| {
                let info = s.info();
                StorageOut {
                    id: s.id().0,
                    description: info.description.clone(),
                    kind: kind_of(&info.description),
                    free_bytes: info.free_space,
                    total_bytes: info.total_capacity,
                    writable: info.is_writable,
                }
            })
            .collect()
    }

    pub fn storages(&self) -> Vec<StorageOut> {
        self.storage_rows()
    }

    pub fn storage(&self, kind: Kind) -> Result<&Storage> {
        self.storages
            .iter()
            .find(|s| kind_of(&s.info().description) == kind)
            .ok_or_else(|| {
                Error::device(format!(
                    "the responder has no '{}' storage; check its MTP storage settings",
                    kind.label()
                ))
            })
    }

    /// True when the responder offers an install destination.
    pub fn has_install(&self, kind: Kind) -> bool {
        self.storages
            .iter()
            .any(|s| kind_of(&s.info().description) == kind)
    }

    pub fn list(&self, kind: Kind, path: &str) -> Result<Vec<RemoteEntry>> {
        let storage = self.storage(kind)?;
        let parent = self.rt.block_on(resolve(storage, path, false))?;
        let objects = self
            .rt
            .block_on(storage.list_objects(parent))
            .map_err(|e| Error::Other(format!("MTP list: {e}")))?;
        let mut rows: Vec<RemoteEntry> = objects
            .into_iter()
            .map(|o| RemoteEntry {
                name: o.filename.clone(),
                path: join_path(path, &o.filename),
                is_dir: o.format.is_association(),
                size: o.size,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(rows)
    }

    /// Push one local file into `dir` (creating folders as needed).
    pub fn upload(&self, kind: Kind, dir: &str, local: &Path, ctx: &JobContext) -> Result<u64> {
        let storage = self.storage(kind)?;
        let total = std::fs::metadata(local)?.len();
        let name = local
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| Error::Invalid("file has no name".into()))?;
        ctx.message(format!("uploading {name} to {}", kind.label()));
        ctx.progress(0, total);
        let cancelled = AtomicBool::new(false);
        self.rt.block_on(async {
            let parent = resolve(storage, dir, true).await?;
            let file = tokio::fs::File::open(local).await?;
            let stream = ReaderStream::new(file);
            let info = NewObjectInfo::file(name.clone(), total);
            let result = storage
                .upload_with_progress(parent, info, stream, |progress| {
                    ctx.progress(progress.bytes_transferred, total);
                    if ctx.check_cancel().is_err() {
                        cancelled.store(true, Ordering::Relaxed);
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                })
                .await;
            match result {
                Ok(_) => Ok(()),
                Err(e) => {
                    if let Some(partial) = e.partial {
                        let _ = storage.delete(partial).await;
                    }
                    let cancelled = ctx.check_cancel().is_err()
                        || matches!(e.source, mtp_rs::mtp::Error::Cancelled);
                    if cancelled {
                        Err(Error::Cancelled(ctx.job_id()))
                    } else {
                        Err(Error::Other(format!("MTP upload: {}", e.source)))
                    }
                }
            }
        })?;
        ctx.progress(total, total);
        Ok(total)
    }

    pub fn download(&self, kind: Kind, remote: &str, local: &Path, ctx: &JobContext) -> Result<u64> {
        let storage = self.storage(kind)?;
        self.rt.block_on(async {
            let parent_path = parent_of(remote);
            let name = last_of(remote);
            let parent = resolve(storage, &parent_path, false).await?;
            let objects = storage
                .list_objects(parent)
                .await
                .map_err(|e| Error::Other(format!("MTP list: {e}")))?;
            let object = objects
                .into_iter()
                .find(|o| o.filename.eq_ignore_ascii_case(&name))
                .ok_or_else(|| Error::NotFound(remote.to_string()))?;
            let total = object.size;
            let staging = local.with_extension(format!(
                "{}part",
                local.extension().and_then(|e| e.to_str()).map(|e| format!("{e}.")).unwrap_or_default()
            ));
            let result = async {
                let mut download = storage
                    .download(object.handle, ByteRange::Full)
                    .await
                    .map_err(|e| Error::Other(format!("MTP download: {e}")))?;
                let mut file = tokio::fs::File::create(&staging).await?;
                let mut done = 0u64;
                while let Some(chunk) = download.next_chunk().await {
                    let chunk = chunk.map_err(|e| Error::Other(format!("MTP download: {e}")))?;
                    tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
                    done += chunk.len() as u64;
                    ctx.progress(done, total);
                    ctx.check_cancel()?;
                }
                if done != total {
                    return Err(Error::Other(format!(
                        "download ended early: {done} of {total} bytes"
                    )));
                }
                Ok(done)
            }
            .await;
            match result {
                Ok(done) => {
                    tokio::fs::rename(&staging, local).await?;
                    Ok(done)
                }
                Err(e) => {
                    let _ = tokio::fs::remove_file(&staging).await;
                    Err(e)
                }
            }
        })
    }

    pub fn mkdir(&self, kind: Kind, dir: &str) -> Result<()> {
        let storage = self.storage(kind)?;
        self.rt.block_on(async {
            resolve(storage, dir, true).await?;
            Ok(())
        })
    }

    pub fn delete(&self, kind: Kind, path: &str) -> Result<()> {
        let storage = self.storage(kind)?;
        self.rt.block_on(async {
            let parent = resolve(storage, &parent_of(path), false).await?;
            let name = last_of(path);
            let objects = storage
                .list_objects(parent)
                .await
                .map_err(|e| Error::Other(format!("MTP list: {e}")))?;
            let object = objects
                .into_iter()
                .find(|o| o.filename.eq_ignore_ascii_case(&name))
                .ok_or_else(|| Error::NotFound(path.to_string()))?;
            storage
                .delete(object.handle)
                .await
                .map_err(|e| Error::Other(format!("MTP delete: {e}")))
        })
    }
}

fn join_path(dir: &str, name: &str) -> String {
    let dir = dir.trim_matches('/');
    if dir.is_empty() {
        format!("/{name}")
    } else {
        format!("/{dir}/{name}")
    }
}

fn parent_of(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    match trimmed.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

fn last_of(path: &str) -> String {
    path.trim_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Walk `/a/b/c` from the storage root, optionally creating missing folders.
async fn resolve(
    storage: &Storage,
    path: &str,
    create: bool,
) -> Result<Option<mtp_rs::mtp::ObjectHandle>> {
    let mut parent = None;
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        let objects = storage
            .list_objects(parent)
            .await
            .map_err(|e| Error::Other(format!("MTP list: {e}")))?;
        match objects.into_iter().find(|o| o.filename.eq_ignore_ascii_case(segment)) {
            Some(object) if object.format.is_association() => parent = Some(object.handle),
            Some(_) => return Err(Error::Invalid(format!("{segment} is a file, not a folder"))),
            None if create => {
                let handle = storage
                    .create_folder(parent, segment)
                    .await
                    .map_err(|e| Error::Other(format!("MTP mkdir {segment}: {e}")))?;
                parent = Some(handle);
            }
            None => return Err(Error::NotFound(format!("/{path}"))),
        }
    }
    Ok(parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_responder_storages() {
        assert_eq!(kind_of("SD Card"), Kind::Sd);
        assert_eq!(kind_of("MicroSD card"), Kind::Sd);
        assert_eq!(kind_of("NAND USER"), Kind::NandUser);
        assert_eq!(kind_of("NAND SYSTEM"), Kind::NandSystem);
        assert_eq!(kind_of("SD install"), Kind::SdInstall);
        assert_eq!(kind_of("Install to NAND"), Kind::NandInstall);
        assert_eq!(kind_of("Installed games"), Kind::Installed);
        assert_eq!(kind_of("Saves"), Kind::Saves);
        assert_eq!(kind_of("Album"), Kind::Album);
        assert_eq!(kind_of("Gamecard"), Kind::Gamecard);
    }

    #[test]
    fn paths_round_trip() {
        assert_eq!(join_path("", "a.nsp"), "/a.nsp");
        assert_eq!(join_path("/switch/", "x.nro"), "/switch/x.nro");
        assert_eq!(parent_of("/switch/x.nro"), "switch");
        assert_eq!(parent_of("/x.nro"), "");
        assert_eq!(last_of("/dev/sd/x.nsp"), "x.nsp");
    }
}
