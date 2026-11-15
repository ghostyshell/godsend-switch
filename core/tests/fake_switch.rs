//! End-to-end test against a fake Switch FTP server (ftpd semantics).
//!
//! Exercises the FTP transport the way the app drives it: connect, list,
//! upload to /switch, download, and split a file for FAT32 on the way in.

use godsend_core::config::Host;
use godsend_core::fm;
use godsend_core::nsp;
use godsend_core::syscon::{PadState, MAGIC};
use godsend_core::transfer::JobContext;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct FakeFtp {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl FakeFtp {
    fn start(root: PathBuf) -> FakeFtp {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ftp");
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let root = root.clone();
                        std::thread::spawn(move || serve(stream, root));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        FakeFtp { port, stop }
    }
}

impl Drop for FakeFtp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn reply(stream: &mut TcpStream, line: &str) {
    let _ = stream.write_all(format!("{line}\r\n").as_bytes());
    let _ = stream.flush();
}

fn map(root: &Path, path: &str) -> PathBuf {
    let clean = path.trim_start_matches('/').replace('\\', "/");
    let mut out = root.to_path_buf();
    for part in clean.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            continue;
        }
        out.push(part);
    }
    out
}

fn serve(mut stream: TcpStream, root: PathBuf) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut data: Option<TcpListener> = None;
    let mut cwd = "/".to_string();
    reply(&mut stream, "220 fake ftpd");
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        let (command, argument) = match line.split_once(' ') {
            Some((c, a)) => (c.to_ascii_uppercase(), a.to_string()),
            None => (line.to_ascii_uppercase(), String::new()),
        };
        match command.as_str() {
            "USER" => reply(&mut stream, "331 need password"),
            "PASS" => reply(&mut stream, "230 logged in"),
            "SYST" => reply(&mut stream, "215 UNIX Type: L8"),
            "FEAT" => {
                let _ = stream.write_all(b"211-Features\r\n MLSD\r\n SIZE\r\n211 End\r\n");
            }
            "TYPE" => reply(&mut stream, "200 binary"),
            "PWD" => reply(&mut stream, &format!("257 \"{cwd}\"")),
            "CWD" => {
                cwd = if argument.starts_with('/') {
                    argument.clone()
                } else {
                    format!("{cwd}/{}", argument.trim_matches('/'))
                };
                reply(&mut stream, "250 ok");
            }
            "PASV" => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = listener.local_addr().unwrap().port();
                data = Some(listener);
                reply(
                    &mut stream,
                    &format!("227 Entering Passive Mode (127,0,0,1,{},{}).", port / 256, port % 256),
                );
            }
            "EPSV" => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = listener.local_addr().unwrap().port();
                data = Some(listener);
                reply(&mut stream, &format!("229 Entering Extended Passive Mode (|||{port}|)"));
            }
            "SIZE" => {
                let path = map(&root, &join(&cwd, &argument));
                match std::fs::metadata(&path) {
                    Ok(meta) => reply(&mut stream, &format!("213 {}", meta.len())),
                    Err(_) => reply(&mut stream, "550 not found"),
                }
            }
            "MLSD" => {
                let path = map(&root, &join(&cwd, &argument));
                let Some(listener) = data.take() else {
                    reply(&mut stream, "425 no data connection");
                    continue;
                };
                reply(&mut stream, "150 opening data connection");
                if let Ok((mut conn, _)) = listener.accept() {
                    if let Ok(rows) = std::fs::read_dir(&path) {
                        for row in rows.flatten() {
                            let meta = row.metadata().unwrap();
                            let kind = if meta.is_dir() { "dir" } else { "file" };
                            let line = format!(
                                "type={kind};size={};modify=20260101000000; {} \r\n",
                                meta.len(),
                                row.file_name().to_string_lossy()
                            );
                            let _ = conn.write_all(line.as_bytes());
                        }
                    }
                }
                reply(&mut stream, "226 done");
            }
            "LIST" => {
                let Some(listener) = data.take() else {
                    reply(&mut stream, "425 no data connection");
                    continue;
                };
                let path = map(&root, &join(&cwd, &argument));
                reply(&mut stream, "150 opening data connection");
                if let Ok((mut conn, _)) = listener.accept() {
                    if let Ok(rows) = std::fs::read_dir(&path) {
                        for row in rows.flatten() {
                            let meta = row.metadata().unwrap();
                            let line = format!(
                                "{}{} 1 owner group {} Jan 01 00:00 {}\r\n",
                                if meta.is_dir() { "d" } else { "-" },
                                "rwxr-xr-x",
                                meta.len(),
                                row.file_name().to_string_lossy()
                            );
                            let _ = conn.write_all(line.as_bytes());
                        }
                    }
                }
                reply(&mut stream, "226 done");
            }
            "MKD" => {
                let _ = std::fs::create_dir_all(map(&root, &join(&cwd, &argument)));
                reply(&mut stream, "257 created");
            }
            "STOR" => {
                let Some(listener) = data.take() else {
                    reply(&mut stream, "425 no data connection");
                    continue;
                };
                reply(&mut stream, "150 ready");
                let path = map(&root, &join(&cwd, &argument));
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok((mut conn, _)) = listener.accept() {
                    let mut file = std::fs::File::create(&path).unwrap();
                    let _ = std::io::copy(&mut conn, &mut file);
                }
                reply(&mut stream, "226 stored");
            }
            "RETR" => {
                let Some(listener) = data.take() else {
                    reply(&mut stream, "425 no data connection");
                    continue;
                };
                let path = map(&root, &join(&cwd, &argument));
                reply(&mut stream, "150 ready");
                if let Ok((mut conn, _)) = listener.accept() {
                    if let Ok(mut file) = std::fs::File::open(&path) {
                        let _ = std::io::copy(&mut file, &mut conn);
                    }
                }
                reply(&mut stream, "226 sent");
            }
            "QUIT" => {
                reply(&mut stream, "221 bye");
                break;
            }
            _ => reply(&mut stream, "502 not implemented"),
        }
    }
}

fn join(cwd: &str, name: &str) -> String {
    if name.starts_with('/') {
        name.to_string()
    } else {
        format!("{cwd}/{}", name.trim_start_matches('/'))
    }
}

fn host(port: u16) -> Host {
    let mut host = Host::new("Fake Switch", "127.0.0.1");
    host.ftp_port = port;
    host.refresh_id();
    host
}

#[test]
fn ftp_transport_round_trip() {
    let root = std::env::temp_dir().join(format!("godsend-ftp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("switch")).unwrap();
    std::fs::write(root.join("switch/existing.nro"), b"hello").unwrap();
    let server = FakeFtp::start(root.clone());
    let ctx = JobContext::for_test();

    let rows = fm::ftp_list(&host(server.port), "/switch").expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "existing.nro");

    let local = root.join("upload.nsp");
    std::fs::write(&local, vec![7u8; 100_000]).unwrap();
    fm::ftp_upload(&host(server.port), &local, "/switch", 0, &ctx).expect("upload");
    assert_eq!(std::fs::metadata(root.join("switch/upload.nsp")).unwrap().len(), 100_000);

    let back = std::env::temp_dir().join(format!("godsend-back-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&back);
    std::fs::create_dir_all(&back).unwrap();
    fm::ftp_download(&host(server.port), "/switch/existing.nro", &back, &ctx).expect("download");
    assert_eq!(std::fs::read(back.join("existing.nro")).unwrap(), b"hello");

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&back);
}

#[test]
fn ftp_fat32_split_writes_parts_and_cleans_up() {
    let root = std::env::temp_dir().join(format!("godsend-split-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let server = FakeFtp::start(root.clone());
    let ctx = JobContext::for_test();
    let local = root.join("big.nsp");
    std::fs::write(&local, vec![1u8; 300_000]).unwrap();
    fm::ftp_upload(&host(server.port), &local, "/switch", 100_000, &ctx).expect("split upload");
    let mut parts: Vec<_> = std::fs::read_dir(root.join("switch"))
        .unwrap()
        .flatten()
        .map(|r| r.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("big.nsp."))
        .collect();
    parts.sort();
    assert_eq!(parts, vec!["big.nsp.00", "big.nsp.01", "big.nsp.02"]);
    assert_eq!(std::fs::metadata(root.join("switch/big.nsp.02")).unwrap().len(), 100_000);
    assert!(local.exists(), "the source file stays");
    let leftover: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|r| r.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("big.nsp."))
        .collect();
    assert!(leftover.is_empty(), "temporary parts are cleaned up: {leftover:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn nsp_metadata_on_a_synthetic_container() {
    let dir = std::env::temp_dir().join(format!("godsend-nspmeta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Test Game [0100152000022000][v65536].nsp");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PFS0");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.resize(0x100, 0);
    std::fs::write(&path, &bytes).unwrap();
    let info = nsp::info(&path).unwrap();
    assert_eq!(info.format, nsp::Format::Nsp);
    assert_eq!(info.title_id.as_deref(), Some("0100152000022000"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn syscon_packet_is_what_the_console_reads() {
    let packet = PadState::with(&[godsend_core::syscon::Button::A]).to_packet();
    assert_eq!(&packet[0..4], &MAGIC.to_le_bytes());
    assert_eq!(packet.len(), 44);
    assert_eq!(u32::from_le_bytes(packet[8..12].try_into().unwrap()), 1 << 2);
}

#[test]
fn fake_server_speaks_plain_ftp() {
    use std::io::{Read, Write};
    let root = std::env::temp_dir().join(format!("godsend-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let server = FakeFtp::start(root.clone());
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let greeting = String::from_utf8_lossy(&buf[..n]).to_string();
    assert!(greeting.starts_with("220"), "{greeting}");
    stream.write_all(b"USER anonymous\r\n").unwrap();
    let n = stream.read(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).starts_with("331"));
    let _ = std::fs::remove_dir_all(&root);
}
