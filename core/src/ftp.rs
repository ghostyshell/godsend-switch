//! FTP client used to talk to a CFW PS3 (webMAN MOD ftpd, multiMAN, IRISMAN,
//! ManaGunZ or the standalone PS3 FTP Server).
//!
//! The console exposes its filesystem as absolute paths such as
//! `/dev_hdd0/PS3ISO/game.iso` and `/dev_usb000/...`. This module wraps
//! `suppaftp` and adds the two things the app needs beyond a raw client:
//! a tolerant directory listing (MLSD, Unix LIST or DOS LIST) and transfer
//! loops that report progress and honour cancellation.

use crate::config::Host;
use crate::error::{Error, Result};
use crate::transfer::JobContext;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::time::Duration;
use suppaftp::{FtpStream, Mode};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

/// Open and authenticate a connection to one console.
pub fn connect(host: &Host) -> Result<FtpStream> {
    let addr = format!("{}:{}", host.addr, host.ftp_port);
    let socket = addr
        .to_socket_addrs()
        .map_err(|e| Error::Ftp(format!("resolve {addr}: {e}")))?
        .next()
        .ok_or_else(|| Error::Ftp(format!("resolve {addr}: no address")))?;
    let mut ftp = FtpStream::connect_timeout(socket, Duration::from_secs(3))
        .map_err(|e| Error::Ftp(format!("connect {addr}: {e}")))?;
    let _ = ftp.get_ref().set_read_timeout(Some(Duration::from_secs(20)));
    let _ = ftp.get_ref().set_write_timeout(Some(Duration::from_secs(20)));
    ftp.set_mode(Mode::Passive);
    let user = if host.ftp_user.trim().is_empty() {
        "anonymous"
    } else {
        host.ftp_user.trim()
    };
    match ftp.login(user, host.ftp_pass.as_str()) {
        Ok(()) => {}
        Err(first) => {
            // PS3 servers disagree on the anonymous user name; try the other
            // convention before giving up.
            let fallback = if user.eq_ignore_ascii_case("anonymous") { "ftp" } else { "anonymous" };
            ftp.login(fallback, host.ftp_pass.as_str())
                .map_err(|_| Error::Ftp(format!("login {addr} as {user}: {first}")))?;
        }
    }
    ftp.transfer_type(suppaftp::types::FileType::Binary)
        .map_err(|e| Error::Ftp(format!("TYPE I: {e}")))?;
    Ok(ftp)
}

/// List a directory, preferring MLSD and falling back to LIST.
pub fn list(ftp: &mut FtpStream, dir: &str) -> Result<Vec<RemoteEntry>> {
    let base = format!("/{}", dir.trim_matches('/'));
    let mut entries = Vec::new();
    if let Ok(lines) = ftp.mlsd(Some(&base)) {
        for line in lines {
            if let Some(e) = parse_mlsd(&base, &line) {
                entries.push(e);
            }
        }
        if !entries.is_empty() {
            return Ok(sorted(entries));
        }
    }
    let lines = ftp
        .list(Some(&base))
        .map_err(|e| Error::Ftp(format!("LIST {base}: {e}")))?;
    for line in lines {
        if let Some(e) = parse_list(&base, &line) {
            entries.push(e);
        }
    }
    Ok(sorted(entries))
}

fn sorted(mut entries: Vec<RemoteEntry>) -> Vec<RemoteEntry> {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

/// `type=file;size=123;modify=20200101120000; name`
pub fn parse_mlsd(dir: &str, line: &str) -> Option<RemoteEntry> {
    let (facts, name) = line.split_once(' ')?;
    let name = name.trim();
    // Skip "." and ".." and anything after a symlink arrow.
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let name = name.split(" -> ").next().unwrap_or(name).to_string();
    let mut is_dir = false;
    let mut size = 0u64;
    for fact in facts.split(';') {
        if let Some(kind) = fact.strip_prefix("type=") {
            is_dir = kind.eq_ignore_ascii_case("dir") || kind.eq_ignore_ascii_case("cdir") || kind.eq_ignore_ascii_case("pdir");
        } else if let Some(value) = fact.strip_prefix("size=") {
            size = value.parse().unwrap_or(0);
        }
    }
    Some(RemoteEntry {
        path: join_remote(dir, &name),
        name,
        is_dir,
        size_bytes: size,
    })
}

/// Unix `-rw-r--r-- 1 root root 1234 Jan 01 12:00 name` or DOS `01-01-20  12:00PM <DIR> name`.
pub fn parse_list(dir: &str, line: &str) -> Option<RemoteEntry> {
    let line = line.trim_end_matches(['\r', '\n']).trim();
    if line.is_empty() || line.starts_with("total ") {
        return None;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        return None;
    }
    let first = tokens[0];
    let (is_dir, size, name_start) = if first.starts_with('d') {
        (true, tokens.get(4).and_then(|s| s.parse().ok()).unwrap_or(0), 8usize)
    } else if first.starts_with('-') || first.starts_with('l') {
        (false, tokens.get(4).and_then(|s| s.parse().ok()).unwrap_or(0), 8usize)
    } else if first.len() == 8 && first.as_bytes().get(2) == Some(&b'-') {
        // DOS date `01-01-20`; `01-01-20  12:00PM  <DIR>  name`
        let dir = tokens.get(2).map(|t| t.eq_ignore_ascii_case("<DIR>")).unwrap_or(false);
        let size = tokens.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (dir, size, 3usize)
    } else {
        // Some servers return just `name`; treat as a file of unknown size.
        if tokens.len() == 1 {
            return Some(RemoteEntry {
                path: join_remote(dir, tokens[0]),
                name: tokens[0].to_string(),
                is_dir: false,
                size_bytes: 0,
            });
        }
        return None;
    };
    if tokens.len() <= name_start {
        return None;
    }
    let name = tokens[name_start..].join(" ");
    let name = name.split(" -> ").next().unwrap_or(&name).to_string();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    Some(RemoteEntry {
        path: join_remote(dir, &name),
        name,
        is_dir,
        size_bytes: size,
    })
}

pub fn join_remote(dir: &str, name: &str) -> String {
    format!("/{}/{}", dir.trim_matches('/'), name.trim_matches('/'))
}

/// True when `path` is a directory. Uses CWD, which every PS3 server supports.
pub fn is_dir(ftp: &mut FtpStream, path: &str) -> bool {
    if ftp.cwd(path).is_ok() {
        let _ = ftp.cdup();
        true
    } else {
        false
    }
}

/// Create `path` and every missing parent, ignoring "already exists".
pub fn mkdir_all(ftp: &mut FtpStream, path: &str) -> Result<()> {
    let mut current = String::new();
    for part in path.trim_matches('/').split('/') {
        if part.is_empty() {
            continue;
        }
        current.push('/');
        current.push_str(part);
        let _ = ftp.mkdir(&current);
    }
    Ok(())
}

/// Download `remote` into `local`, reporting progress.
pub fn download(
    ftp: &mut FtpStream,
    remote: &str,
    local: &std::path::Path,
    ctx: &JobContext,
) -> Result<u64> {
    let total = ftp.size(remote).unwrap_or(0) as u64;
    if let Some(parent) = local.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut stream = ftp
        .retr_as_stream(remote)
        .map_err(|e| Error::Ftp(format!("RETR {remote}: {e}")))?;
    let mut file = std::fs::File::create(local)?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        ctx.check_cancel()?;
        let read = stream
            .read(&mut buf)
            .map_err(|e| Error::Ftp(format!("read {remote}: {e}")))?;
        if read == 0 {
            break;
        }
        file.write_all(&buf[..read])?;
        done += read as u64;
        if last.elapsed().as_millis() > 120 {
            ctx.progress(done, total);
            last = std::time::Instant::now();
        }
    }
    ftp.finalize_retr_stream(stream)
        .map_err(|e| Error::Ftp(format!("RETR finish {remote}: {e}")))?;
    file.flush()?;
    ctx.progress(done, total.max(done));
    Ok(done)
}

/// Upload `local` to `remote`, reporting progress.
pub fn upload(
    ftp: &mut FtpStream,
    local: &std::path::Path,
    remote: &str,
    ctx: &JobContext,
) -> Result<u64> {
    let total = std::fs::metadata(local)?.len();
    if let Some(parent) = remote.rsplit_once('/').map(|(p, _)| p) {
        mkdir_all(ftp, parent)?;
    }
    let mut file = std::fs::File::open(local)?;
    let mut stream = ftp
        .put_with_stream(remote)
        .map_err(|e| Error::Ftp(format!("STOR {remote}: {e}")))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        ctx.check_cancel()?;
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        stream
            .write_all(&buf[..read])
            .map_err(|e| Error::Ftp(format!("write {remote}: {e}")))?;
        done += read as u64;
        if last.elapsed().as_millis() > 120 {
            ctx.progress(done, total);
            last = std::time::Instant::now();
        }
    }
    ftp.finalize_put_stream(stream)
        .map_err(|e| Error::Ftp(format!("STOR finish {remote}: {e}")))?;
    ctx.progress(done, total.max(done));
    Ok(done)
}

/// Read at most `len` bytes from the start of a remote file (PARAM.SFO, icons).
pub fn read_head(ftp: &mut FtpStream, remote: &str, max: usize) -> Result<Vec<u8>> {
    let mut stream = ftp
        .retr_as_stream(remote)
        .map_err(|e| Error::Ftp(format!("RETR {remote}: {e}")))?;
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    while out.len() < max {
        let read = stream
            .read(&mut buf)
            .map_err(|e| Error::Ftp(format!("read {remote}: {e}")))?;
        if read == 0 {
            break;
        }
        let take = read.min(max - out.len());
        out.extend_from_slice(&buf[..take]);
        if take < read {
            break;
        }
    }
    let _ = ftp.finalize_retr_stream(stream);
    Ok(out)
}

/// Delete a file or (recursively) a directory.
pub fn remove(ftp: &mut FtpStream, path: &str, is_dir: bool) -> Result<()> {
    if is_dir {
        for entry in list(ftp, path)? {
            remove(ftp, &entry.path, entry.is_dir)?;
        }
        ftp.rmdir(path).map_err(|e| Error::Ftp(format!("RMD {path}: {e}")))?;
    } else {
        ftp.rm(path).map_err(|e| Error::Ftp(format!("DELE {path}: {e}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mlsd_facts() {
        let e = parse_mlsd("/dev_hdd0/PS3ISO", "type=file;size=1234;modify=20200101; Game One.iso").unwrap();
        assert_eq!(e.name, "Game One.iso");
        assert_eq!(e.path, "/dev_hdd0/PS3ISO/Game One.iso");
        assert_eq!(e.size_bytes, 1234);
        assert!(!e.is_dir);
        let d = parse_mlsd("/dev_hdd0", "type=dir;modify=20200101; PS3ISO").unwrap();
        assert!(d.is_dir);
    }

    #[test]
    fn parses_unix_and_dos_list() {
        let f = parse_list(
            "/dev_hdd0/GAMES",
            "-rw-r--r--    1 root     root      4096 Jan 01 12:00 Game Folder Name",
        )
        .unwrap();
        assert_eq!(f.name, "Game Folder Name");
        assert_eq!(f.size_bytes, 4096);
        assert!(!f.is_dir);

        let d = parse_list("/dev_hdd0", "drwxr-xr-x    2 root     root         0 Jan 01 12:00 PS3ISO").unwrap();
        assert!(d.is_dir);

        let dos = parse_list("/dev_hdd0", "01-01-20  12:00PM  <DIR>  PSXISO").unwrap();
        assert!(dos.is_dir);
        assert_eq!(dos.name, "PSXISO");
    }

    #[test]
    fn skips_noise() {
        assert!(parse_list("/", "total 42").is_none());
        assert!(parse_list("/", "").is_none());
        assert!(parse_mlsd("/", "type=dir; .").is_none());
    }
}
