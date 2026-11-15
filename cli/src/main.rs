//! GODsend Switch command line.
//!
//! Talks to a Switch over MTP, Tinfoil USB, the LAN install protocol and FTP.

use godsend_core::config::{Config, Host, InstallTarget};
use godsend_core::device;
use godsend_core::doctor;
use godsend_core::fm;
use godsend_core::homebrew;
use godsend_core::install;
use godsend_core::library;
use godsend_core::mcp;
use godsend_core::mtp::MtpSession;
use godsend_core::nsp;
use godsend_core::syscon::{self, Button, PadState};
use godsend_core::transfer::JobContext;
use godsend_core::{Error, Result};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" || args[0] == "help" {
        usage();
        return;
    }
    if let Err(err) = run(&args) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn usage() {
    println!(
        r#"GODsend Switch {version}

Usage: godsend-switch <command> [options]

Hosts and status
  hosts                          list FTP hosts
  add-host <addr> [--name N] [--user U] [--pass P] [--ftp-port N]
  rm-host <id>                   remove a host
  use <id>                       make a host active
  discover [--subnet 192.168.1] [--port 5000]
  status                         probe MTP, Tinfoil USB, FTP, sys-con
  doctor                         preflight checks
  storages                       list MTP storage units with free space

Files and library
  ls <storage> [path]            list a storage path (sd, installed, saves, ...)
  push <local> <storage> <dir>   upload a file or folder over MTP
  pull <storage> <path> [dir]    download a file over MTP
  library [--local]              installed titles, or local packages

Games and installs
  install <file> [--via sd|nand|tinfoil|network]
  tinfoil <file...>              USB install via Tinfoil/Awoo
  netinstall <file>              LAN install on TCP 2000
  homebrew [--refresh]           list the NRO catalog
  homebrew-install <id>          download and push an NRO to /switch

Virtual pad (sys-con)
  pad <button>                   press and release
  stick <l|r> <x> <y>            tilt a stick, -1..1
  pad-release                    detach the pad

Conversions
  inspect <path>                 format, title id, split parts
  split <file> [--chunk-mb 4095] split for FAT32
  merge <base> <out>             join <base>.00, <base>.01, ...

Other
  mcp                            run the MCP server over stdio
"#,
        version = godsend_core::VERSION
    );
}

fn run(args: &[String]) -> Result<()> {
    let mut config = Config::load();
    let cmd = args[0].as_str();
    match cmd {
        "hosts" => {
            for host in &config.hosts {
                let marker = if config.active_host.as_deref() == Some(&host.id) { "*" } else { " " };
                println!(
                    "{marker} {:<24} {:<20} ftp :{}",
                    host.id, host.name, host.ftp_port
                );
            }
            if config.hosts.is_empty() {
                println!("no hosts yet: godsend-switch add-host 192.168.1.50");
            }
            Ok(())
        }
        "add-host" => {
            let addr = args.get(1).ok_or_else(|| Error::Invalid("add-host <addr>".into()))?;
            let name = flag(args, "--name").unwrap_or_else(|| addr.clone());
            let mut host = Host::new(&name, addr);
            if let Some(user) = flag(args, "--user") {
                host.ftp_user = user;
            }
            if let Some(pass) = flag(args, "--pass") {
                host.ftp_pass = pass;
            }
            if let Some(port) = flag(args, "--ftp-port") {
                host.ftp_port = port.parse().map_err(|_| Error::Invalid("bad port".into()))?;
                host.refresh_id();
            }
            if !config.hosts.iter().any(|h| h.id == host.id) {
                config.hosts.push(host.clone());
            }
            if config.active_host.is_none() {
                config.active_host = Some(host.id.clone());
            }
            config.save()?;
            println!("saved {}", host.id);
            Ok(())
        }
        "rm-host" => {
            let id = args.get(1).ok_or_else(|| Error::Invalid("rm-host <id>".into()))?;
            config.hosts.retain(|h| &h.id != id);
            if config.active_host.as_deref() == Some(id.as_str()) {
                config.active_host = config.hosts.first().map(|h| h.id.clone());
            }
            config.save()?;
            Ok(())
        }
        "use" => {
            let id = args.get(1).ok_or_else(|| Error::Invalid("use <id>".into()))?;
            if !config.hosts.iter().any(|h| &h.id == id) {
                return Err(Error::NotFound(id.clone()));
            }
            config.active_host = Some(id.clone());
            config.save()?;
            Ok(())
        }
        "discover" => {
            let subnet = flag(args, "--subnet").unwrap_or_else(|| "192.168.1".into());
            let port = flag(args, "--port").and_then(|v| v.parse().ok()).unwrap_or(5000);
            let found = device::discover(&subnet, port, "anonymous", "")?;
            println!("found {}", found.len());
            for host in found {
                println!("{}", host.id);
            }
            Ok(())
        }
        "status" => {
            let status = device::probe(&config);
            println!("MTP      {}", if status.mtp_ok { &status.mtp_product } else { "not attached" });
            for storage in &status.storages {
                println!(
                    "  {:<14} {} free",
                    storage.description,
                    doctor::human_bytes(storage.free_bytes)
                );
            }
            println!("Tinfoil  {}", if status.tinfoil_ok { "USB install mode" } else { "not attached" });
            println!("FTP      {}", if status.ftp_ok { "ok" } else { "offline" });
            println!("sys-con  {}", if status.syscon_ok { &config.syscon_addr } else { "not configured" });
            if !status.message.is_empty() {
                println!("note: {}", status.message);
            }
            Ok(())
        }
        "doctor" => {
            for check in doctor::run(&config) {
                println!("[{}] {:<18} {}", if check.ok { "ok" } else { "--" }, check.name, check.detail);
            }
            Ok(())
        }
        "storages" => {
            let session = MtpSession::open(&config.mtp_serial)?;
            for storage in session.storages() {
                println!(
                    "{:<14} {:<12} {} free of {}",
                    storage.description,
                    storage.kind.label(),
                    doctor::human_bytes(storage.free_bytes),
                    doctor::human_bytes(storage.total_bytes)
                );
            }
            Ok(())
        }
        "ls" => {
            let kind = mcp::kind_from_str(args.get(1).ok_or_else(|| Error::Invalid("ls <storage> [path]".into()))?)?;
            let path = args.get(2).map(String::as_str).unwrap_or("/");
            let session = MtpSession::open(&config.mtp_serial)?;
            for row in session.list(kind, path)? {
                println!(
                    "{}{:<48} {}",
                    if row.is_dir { "d " } else { "  " },
                    row.name,
                    if row.is_dir { String::new() } else { doctor::human_bytes(row.size) }
                );
            }
            Ok(())
        }
        "push" => {
            let local = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("push <local> <storage> <dir>".into()))?);
            let kind = mcp::kind_from_str(args.get(2).ok_or_else(|| Error::Invalid("push <local> <storage> <dir>".into()))?)?;
            let dir = args.get(3).ok_or_else(|| Error::Invalid("push <local> <storage> <dir>".into()))?;
            let ctx = JobContext::for_test();
            let session = MtpSession::open(&config.mtp_serial)?;
            let bytes = fm::mtp_upload(&session, kind, dir, &local, &ctx)?;
            println!("uploaded {} to {} {}", doctor::human_bytes(bytes), kind.label(), dir);
            Ok(())
        }
        "pull" => {
            let kind = mcp::kind_from_str(args.get(1).ok_or_else(|| Error::Invalid("pull <storage> <path> [dir]".into()))?)?;
            let remote = args.get(2).ok_or_else(|| Error::Invalid("pull <storage> <path> [dir]".into()))?;
            let local = PathBuf::from(args.get(3).cloned().unwrap_or_else(|| ".".into()));
            std::fs::create_dir_all(&local)?;
            let session = MtpSession::open(&config.mtp_serial)?;
            let target = fm::mtp_download(&session, kind, remote, &local, &JobContext::for_test())?;
            println!("{}", target.display());
            Ok(())
        }
        "library" => {
            if args.iter().any(|a| a == "--local") {
                let dir = godsend_core::aria2::download_dir(&config);
                for entry in library::local(&dir, &config.covers_base) {
                    println!("{:<50} {:>10} {:?}", entry.name, doctor::human_bytes(entry.size), entry.format);
                }
                return Ok(());
            }
            let session = MtpSession::open(&config.mtp_serial)?;
            for entry in library::installed(&session, &config.covers_base)? {
                println!("{:<50} {}", entry.name, entry.title_id.unwrap_or_default());
            }
            Ok(())
        }
        "install" => {
            let path = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("install <file>".into()))?);
            if let Some(via) = flag(args, "--via") {
                config.install_target = match via.as_str() {
                    "sd" => InstallTarget::Sd,
                    "nand" => InstallTarget::Nand,
                    "tinfoil" | "usb" => InstallTarget::Tinfoil,
                    "network" | "lan" => InstallTarget::Network,
                    other => return Err(Error::Invalid(format!("unknown target {other}"))),
                };
            }
            let report = install::install(&config, &path, &JobContext::for_test())?;
            println!(
                "{} -> {} ({})",
                report.transport,
                report.destination,
                doctor::human_bytes(report.bytes)
            );
            Ok(())
        }
        "tinfoil" => {
            let files: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
            godsend_core::tinfoil::install(&files, &JobContext::for_test())
        }
        "netinstall" => {
            let file = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("netinstall <file>".into()))?);
            let host = config.require_active()?;
            godsend_core::netinstall::install(&[file], &host.addr, godsend_core::netinstall::PORT, &JobContext::for_test())
        }
        "pad" => {
            let name = args.get(1).ok_or_else(|| Error::Invalid("pad <button>".into()))?;
            syscon::tap(&config.syscon_addr, config.syscon_port, Button::parse(name)?, 90)
        }
        "stick" => {
            let side = args.get(1).ok_or_else(|| Error::Invalid("stick <l|r> <x> <y>".into()))?;
            let x: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let y: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let state = PadState::default().with_stick(side, x, y)?;
            syscon::send(&config.syscon_addr, config.syscon_port, &state)
        }
        "pad-release" => syscon::disconnect(&config.syscon_addr, config.syscon_port),
        "homebrew" => {
            let refresh = args.iter().any(|a| a == "--refresh");
            for row in homebrew::catalog(&config, refresh) {
                println!(
                    "{:<16} {:<24} {:<12} {}",
                    row.id,
                    row.name,
                    row.latest_tag.unwrap_or_else(|| "-".into()),
                    row.nro_url.unwrap_or_else(|| "unavailable".into())
                );
            }
            Ok(())
        }
        "homebrew-install" => {
            let id = args.get(1).ok_or_else(|| Error::Invalid("homebrew-install <id>".into()))?;
            let path = homebrew::install(&config, id, &JobContext::for_test())?;
            println!("{}", path.display());
            Ok(())
        }
        "inspect" => {
            let path = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("inspect <path>".into()))?);
            let info = nsp::info(&path)?;
            println!("name      {}", info.name);
            println!("format    {:?}", info.format);
            println!("size      {}", doctor::human_bytes(info.size));
            println!("title id  {}", info.title_id.unwrap_or_else(|| "-".into()));
            println!("parts     {}", info.parts);
            Ok(())
        }
        "split" => {
            let path = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("split <file>".into()))?);
            let mb = flag(args, "--chunk-mb").and_then(|v| v.parse().ok()).unwrap_or(4095u64);
            let parts = nsp::split(&path, mb * 1024 * 1024, &JobContext::for_test())?;
            for part in parts {
                println!("{}", part.display());
            }
            Ok(())
        }
        "merge" => {
            let base = PathBuf::from(args.get(1).ok_or_else(|| Error::Invalid("merge <base> <out>".into()))?);
            let out = PathBuf::from(args.get(2).ok_or_else(|| Error::Invalid("merge <base> <out>".into()))?);
            let parts = nsp::find_parts(&base);
            let bytes = nsp::merge(&parts, &out, &JobContext::for_test())?;
            println!("{} ({})", out.display(), doctor::human_bytes(bytes));
            Ok(())
        }
        "mcp" => mcp::run_stdio(config),
        other => Err(Error::Invalid(format!("unknown command '{other}' (try --help)"))),
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}
