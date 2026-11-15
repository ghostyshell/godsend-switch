//! Awoo/Tinfoil LAN install.
//!
//! The console listens on TCP 2000. The host sends one length-prefixed list of
//! newline-separated HTTP URLs, then serves those files with Range support.
//! The console answers with a single 0x00 byte when it finishes.

use crate::error::{Error, Result};
use crate::transfer::JobContext;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const PORT: u16 = 2000;

/// Percent-encode a path segment for the URL we hand to the console.
pub fn url_name(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Parse a `Range: bytes=a-b` header against a file size. Returns inclusive
/// start and end, or None for a whole-file request. An inverted range
/// (`bytes=500-100`) is rejected rather than underflowing the length.
pub fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = if end.trim().is_empty() {
        size.saturating_sub(1)
    } else {
        end.trim().parse().ok()?
    };
    if start >= size || end < start {
        return None;
    }
    Some((start, end.min(size.saturating_sub(1))))
}

/// Response head for one file request.
pub fn response_head(size: u64, range: Option<(u64, u64)>) -> String {
    match range {
        Some((start, end)) => format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{size}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            end - start + 1
        ),
        None => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {size}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
        ),
    }
}

struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(files: Vec<PathBuf>, stop: Arc<AtomicBool>, ctx: JobContext) -> Result<Server> {
    let listener = TcpListener::bind(("0.0.0.0", 0))?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let worker_stop = stop.clone();
    let handle = std::thread::spawn(move || {
        while !worker_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                    let files = files.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let _ = serve_one(stream, &files, &ctx);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
    });
    Ok(Server { port, stop, thread: Some(handle) })
}

fn serve_one(stream: TcpStream, files: &[PathBuf], ctx: &JobContext) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut range_header = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("range") {
                range_header = value.trim().to_string();
            }
        }
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let index: usize = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .and_then(|i| i.parse().ok())
        .ok_or_else(|| Error::Invalid(format!("bad install path {path}")))?;
    let file_path = files.get(index).ok_or_else(|| Error::NotFound(path.to_string()))?;
    let size = std::fs::metadata(file_path)?.len();
    let range = parse_range(&range_header, size);
    let mut stream = stream;
    stream.write_all(response_head(size, range).as_bytes())?;
    let mut file = File::open(file_path)?;
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    file.seek(SeekFrom::Start(start))?;
    let mut left = end.saturating_sub(start) + 1;
    let mut buf = vec![0u8; 1024 * 1024];
    while left > 0 {
        if ctx.check_cancel().is_err() {
            break;
        }
        let want = (left as usize).min(buf.len());
        let got = file.read(&mut buf[..want])?;
        if got == 0 {
            break;
        }
        stream.write_all(&buf[..got])?;
        left -= got as u64;
    }
    Ok(())
}

/// Send the URL list to the console and serve until it acknowledges.
pub fn install(files: &[PathBuf], addr: &str, port: u16, ctx: &JobContext) -> Result<()> {
    if files.is_empty() {
        return Err(Error::Invalid("no files selected".into()));
    }
    if addr.trim().is_empty() {
        return Err(Error::device("console address is not set"));
    }
    let ip = crate::config::local_ip()
        .ok_or_else(|| Error::Other("no local IPv4 address; connect to the LAN".into()))?;
    let server = serve(files.to_vec(), Arc::new(AtomicBool::new(false)), ctx.clone())?;
    let urls: Vec<String> = files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let name = url_name(&path.file_name().unwrap_or_default().to_string_lossy());
            format!("http://{ip}:{}/{index}/{name}", server.port)
        })
        .collect();
    let body = urls.join("\n");
    let target = format!("{}:{}", addr.trim(), if port == 0 { PORT } else { port });
    ctx.message(format!("sending {} URL(s) to {target}", urls.len()));
    let mut stream = TcpStream::connect(&target)
        .map_err(|e| Error::Other(format!("connect {target}: {e}")))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    // The first byte is Awoo's 0x00 ack, sent when it accepts the list, not
    // when the install finishes. Keep serving until the console closes the
    // connection or the link goes idle, and only then report completion.
    let mut buf = [0u8; 256];
    let mut idle = Duration::from_secs(0);
    loop {
        ctx.check_cancel()?;
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                idle = Duration::from_secs(0);
                continue;
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                idle += Duration::from_secs(2);
                if idle > Duration::from_secs(3600) {
                    return Err(Error::Other("console stopped answering the install".into()));
                }
                continue;
            }
            Err(e) => return Err(Error::Other(format!("install link: {e}"))),
        }
    }
    ctx.progress(1, 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_names_for_urls() {
        assert_eq!(url_name("Game [0100].nsp"), "Game%20%5B0100%5D.nsp");
        assert_eq!(url_name("plain-nsp_1.2.3~x"), "plain-nsp_1.2.3~x");
    }

    #[test]
    fn parses_range_headers() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-", 1000), Some((100, 999)));
        assert_eq!(parse_range("bytes=990-5000", 1000), Some((990, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=500-100", 1000), None);
    }

    #[test]
    fn builds_range_and_full_heads() {
        let partial = response_head(1000, Some((0, 99)));
        assert!(partial.starts_with("HTTP/1.1 206"));
        assert!(partial.contains("Content-Length: 100"));
        assert!(partial.contains("Content-Range: bytes 0-99/1000"));
        let full = response_head(1000, None);
        assert!(full.starts_with("HTTP/1.1 200"));
        assert!(full.contains("Content-Length: 1000"));
    }
}
