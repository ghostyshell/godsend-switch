//! File operations: local disk, MTP storages and FTP hosts.

use crate::config::Host;
use crate::error::{Error, Result};
use crate::ftp;
use crate::mtp::{Kind, MtpSession, RemoteEntry};
use crate::transfer::JobContext;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct LocalEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Read a local folder, hiding dotfiles and AppleDouble noise.
pub fn local_list(dir: &Path) -> Result<Vec<LocalEntry>> {
    let mut out = Vec::new();
    for row in std::fs::read_dir(dir)?.flatten() {
        let name = row.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with("._") {
            continue;
        }
        let meta = row.metadata()?;
        out.push(LocalEntry {
            name,
            path: row.path().to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
        });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

pub fn local_mkdir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    Ok(())
}

pub fn local_delete(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Read a folder on an MTP storage.
pub fn mtp_list(session: &MtpSession, kind: Kind, dir: &str) -> Result<Vec<RemoteEntry>> {
    session.list(kind, dir)
}

/// Upload one local file or a folder tree into an MTP storage.
pub fn mtp_upload(
    session: &MtpSession,
    kind: Kind,
    remote_dir: &str,
    local: &Path,
    ctx: &JobContext,
) -> Result<u64> {
    if local.is_dir() {
        let name = local
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| Error::Invalid("folder has no name".into()))?;
        let base = format!("{}/{}", remote_dir.trim_end_matches('/'), name);
        let mut total = 0u64;
        for entry in walk(local)? {
            if entry.dir {
                session.mkdir(kind, &entry.remote(&base))?;
            } else {
                let remote = entry.remote(&base);
                total += session.upload(kind, &dir_of(&remote), &entry.path, ctx)?;
            }
        }
        Ok(total)
    } else {
        session.upload(kind, remote_dir, local, ctx)
    }
}

pub fn mtp_download(
    session: &MtpSession,
    kind: Kind,
    remote: &str,
    local_dir: &Path,
    ctx: &JobContext,
) -> Result<PathBuf> {
    let name = remote.rsplit('/').next().unwrap_or("download");
    let target = local_dir.join(name);
    session.download(kind, remote, &target, ctx)?;
    Ok(target)
}

/// List an FTP directory.
pub fn ftp_list(host: &Host, dir: &str) -> Result<Vec<ftp::RemoteEntry>> {
    let mut conn = ftp::connect(host)?;
    let result = ftp::list(&mut conn, dir);
    let _ = conn.quit();
    result
}

/// Upload to an FTP host, splitting files for a FAT32 target when needed.
pub fn ftp_upload(
    host: &Host,
    local: &Path,
    remote_dir: &str,
    split_bytes: u64,
    ctx: &JobContext,
) -> Result<String> {
    let mut conn = ftp::connect(host)?;
    let result = (|| -> Result<String> {
        if local.is_dir() {
            push_folder(&mut conn, local, remote_dir, ctx)
        } else {
            push_file(&mut conn, local, remote_dir, split_bytes, ctx)
        }
    })();
    let _ = conn.quit();
    result
}

pub fn ftp_download(host: &Host, remote: &str, local_dir: &Path, ctx: &JobContext) -> Result<PathBuf> {
    let mut conn = ftp::connect(host)?;
    let name = remote.rsplit('/').next().unwrap_or("download");
    let target = local_dir.join(name);
    let result = ftp::download(&mut conn, remote, &target, ctx).map(|_| target);
    let _ = conn.quit();
    result
}

fn push_file(
    conn: &mut suppaftp::FtpStream,
    local: &Path,
    remote_dir: &str,
    split_bytes: u64,
    ctx: &JobContext,
) -> Result<String> {
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("file has no name".into()))?;
    let target = format!("{}/{}", remote_dir.trim_end_matches('/'), name);
    ftp::mkdir_all(conn, remote_dir)?;
    let size = std::fs::metadata(local)?.len();
    if split_bytes > 0 && size > split_bytes {
        ctx.message(format!("splitting {name} for FAT32"));
        let parts = crate::nsp::split(local, split_bytes, ctx)?;
        for part in &parts {
            let part_name = part.file_name().unwrap_or_default().to_string_lossy();
            let remote = format!("{}/{}", remote_dir.trim_end_matches('/'), part_name);
            if let Err(e) = ftp::upload(conn, part, &remote, ctx) {
                for leftover in &parts {
                    let _ = std::fs::remove_file(leftover);
                }
                return Err(e);
            }
            let _ = std::fs::remove_file(part);
        }
        return Ok(target);
    }
    ftp::upload(conn, local, &target, ctx)?;
    Ok(target)
}

fn push_folder(
    conn: &mut suppaftp::FtpStream,
    local: &Path,
    remote_parent: &str,
    ctx: &JobContext,
) -> Result<String> {
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| Error::Invalid("folder has no name".into()))?;
    let base = format!("{}/{}", remote_parent.trim_end_matches('/'), name);
    let mut done = 0u64;
    for entry in walk(local)? {
        ctx.check_cancel()?;
        let remote = entry.remote(&base);
        if entry.dir {
            ftp::mkdir_all(conn, &remote)?;
        } else {
            ftp::upload(conn, &entry.path, &remote, ctx)?;
            done += std::fs::metadata(&entry.path).map(|m| m.len()).unwrap_or(0);
            ctx.progress(done, done);
        }
    }
    Ok(base)
}

struct WalkEntry {
    path: PathBuf,
    dir: bool,
    relative: PathBuf,
}

impl WalkEntry {
    fn remote(&self, base: &str) -> String {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            self.relative.to_string_lossy().replace('\\', "/")
        )
    }
}

fn dir_of(remote: &str) -> String {
    match remote.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

fn walk(root: &Path) -> Result<Vec<WalkEntry>> {
    fn visit(dir: &Path, root: &Path, out: &mut Vec<WalkEntry>) -> Result<()> {
        for row in std::fs::read_dir(dir)?.flatten() {
            let name = row.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.starts_with("._") {
                continue;
            }
            let path = row.path();
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let file_type = row.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let is_dir = file_type.is_dir();
            out.push(WalkEntry { path: path.clone(), dir: is_dir, relative });
            if is_dir {
                visit(&path, root, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_listing_hides_noise() {
        let dir = std::env::temp_dir().join(format!("godsend-fm-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.nsp"), b"x").unwrap();
        std::fs::write(dir.join(".hidden"), b"x").unwrap();
        std::fs::write(dir.join("._a.nsp"), b"x").unwrap();
        let rows = local_list(&dir).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "sub");
        assert_eq!(rows[1].name, "a.nsp");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn walk_is_relative_and_prunes_noise() {
        let dir = std::env::temp_dir().join(format!("godsend-walk-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/x.nro"), b"x").unwrap();
        std::fs::write(dir.join(".DS_Store"), b"x").unwrap();
        let rows = walk(&dir).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].remote("/switch"), "/switch/sub/x.nro");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
