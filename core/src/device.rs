//! Device probing: what is attached, and over which transport.

use crate::config::Config;
use crate::error::Result;
use crate::ftp;
use crate::mtp::{self, StorageOut};
use crate::tinfoil;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeviceStatus {
    /// MTP responder answered and exposed storages.
    pub mtp_ok: bool,
    pub mtp_serial: String,
    pub mtp_product: String,
    pub storages: Vec<StorageOut>,
    /// Tinfoil USB install mode is active.
    pub tinfoil_ok: bool,
    /// FTP host answered.
    pub ftp_ok: bool,
    pub ftp_ms: u64,
    /// sys-con address is configured (reachability is not probed: UDP).
    pub syscon_ok: bool,
    pub message: String,
}

pub fn probe(config: &Config) -> DeviceStatus {
    let mut status = DeviceStatus::default();

    match crate::mtp::MtpSession::open(&config.mtp_serial) {
        Ok(session) => {
            let describe = session.describe();
            status.mtp_ok = true;
            status.mtp_serial = describe.serial;
            status.mtp_product = describe.product;
            status.storages = describe.storages;
        }
        Err(e) => status.message = e.to_string(),
    }

    status.tinfoil_ok = tinfoil::attached().is_some();
    status.syscon_ok = !config.syscon_addr.trim().is_empty();

    if let Some(host) = config.active() {
        let started = Instant::now();
        match ftp::connect(&host) {
            Ok(mut conn) => {
                status.ftp_ok = conn.pwd().is_ok();
                status.ftp_ms = started.elapsed().as_millis() as u64;
                let _ = conn.quit();
            }
            Err(e) => {
                if status.message.is_empty() {
                    status.message = e.to_string();
                }
            }
        }
    }
    status
}

/// True when at least one transport can reach a console.
pub fn any_transport(status: &DeviceStatus) -> bool {
    status.mtp_ok || status.tinfoil_ok || status.ftp_ok
}

/// Scan `<subnet>.1-254` for an FTP port. Candidates need a successful login
/// to be kept, which filters most unrelated services.
pub fn discover(subnet: &str, port: u16, user: &str, pass: &str) -> Result<Vec<crate::config::Host>> {
    let subnet = subnet.trim().trim_end_matches('.');
    if subnet.is_empty() {
        return Err(crate::Error::Invalid("subnet is empty".into()));
    }
    let mut found = Vec::new();
    for last in 1..=254u16 {
        let addr = format!("{subnet}.{last}");
        let target = format!("{addr}:{port}");
        let Ok(mut addrs) = target.to_socket_addrs() else {
            continue;
        };
        let Some(socket) = addrs.next() else { continue };
        if TcpStream::connect_timeout(&socket, Duration::from_millis(120)).is_err() {
            continue;
        }
        let mut host = crate::config::Host::new(&addr, &addr);
        host.ftp_port = port;
        host.ftp_user = user.to_string();
        host.ftp_pass = pass.to_string();
        host.refresh_id();
        if ftp::connect(&host).is_ok() {
            found.push(host);
        }
    }
    Ok(found)
}

/// Storage lookup for callers that already hold a session.
pub fn install_storage(config: &Config, session: &mtp::MtpSession) -> Option<mtp::Kind> {
    let kind = match config.install_target {
        crate::config::InstallTarget::Nand => mtp::Kind::NandInstall,
        _ => mtp::Kind::SdInstall,
    };
    session.has_install(kind).then_some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_status_has_no_transport() {
        let status = DeviceStatus::default();
        assert!(!any_transport(&status));
    }

    #[test]
    fn any_single_transport_counts() {
        let mut status = DeviceStatus::default();
        status.ftp_ok = true;
        assert!(any_transport(&status));
        let mut status = DeviceStatus::default();
        status.tinfoil_ok = true;
        assert!(any_transport(&status));
        let mut status = DeviceStatus::default();
        status.mtp_ok = true;
        assert!(any_transport(&status));
    }
}
