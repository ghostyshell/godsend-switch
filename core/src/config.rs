//! App configuration: Switch hosts, transports and service bases.
//!
//! Persisted as JSON under the platform data directory.

use crate::error::{Error, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// One LAN FTP server on a Switch (sys-ftpd, ftpd, NX-Shell).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Host {
    /// Stable id: `addr:ftp_port`.
    pub id: String,
    pub name: String,
    /// IP address or hostname.
    pub addr: String,
    #[serde(default = "default_ftp_port")]
    pub ftp_port: u16,
    #[serde(default = "default_ftp_user")]
    pub ftp_user: String,
    #[serde(default)]
    pub ftp_pass: String,
}

fn default_ftp_port() -> u16 {
    5000
}
fn default_ftp_user() -> String {
    "anonymous".into()
}

impl Host {
    pub fn new(name: &str, addr: &str) -> Self {
        let addr = addr.trim().to_string();
        Self {
            id: format!("{addr}:{}", default_ftp_port()),
            name: if name.trim().is_empty() {
                addr.clone()
            } else {
                name.trim().to_string()
            },
            addr,
            ftp_port: default_ftp_port(),
            ftp_user: default_ftp_user(),
            ftp_pass: String::new(),
        }
    }

    pub fn refresh_id(&mut self) {
        self.id = format!("{}:{}", self.addr.trim(), self.ftp_port);
    }
}

/// Where an install lands when more than one transport is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallTarget {
    /// MTP: the "SD install" storage.
    Sd,
    /// MTP: the "NAND install" storage.
    Nand,
    /// Tinfoil USB bulk protocol.
    Tinfoil,
    /// Awoo/Tinfoil LAN install on TCP 2000.
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Configured FTP hosts.
    pub hosts: Vec<Host>,
    /// Id of the host the UI works against.
    pub active_host: Option<String>,
    /// Preferred MTP serial, empty = first Switch found.
    pub mtp_serial: String,
    /// Default install transport.
    pub install_target: InstallTarget,
    /// sys-con network controller.
    pub syscon_addr: String,
    pub syscon_port: u16,
    /// Where downloads land. Empty = `<Downloads>/godsend-switch`.
    pub download_dir: String,
    /// Real-Debrid API key. Empty = not connected.
    pub rd_api_key: String,
    /// TorBox API key. Empty = not connected.
    pub tb_api_key: String,
    /// game-search service base URL.
    pub gamesearch_base: String,
    /// Public R2 base for Switch box art.
    pub covers_base: String,
    /// Optional explicit aria2c path. Empty = auto-detect.
    pub aria2_path: String,
    /// Chunk size for FAT32 splits in bytes.
    pub split_bytes: u64,
    /// Port the app's MCP server listens on (127.0.0.1).
    pub mcp_port: u16,
    /// Start the MCP server with the app.
    pub mcp_enabled: bool,
    /// Ask the console to install the file as soon as MTP upload finishes.
    pub auto_install: bool,
    /// Send a neutral sys-con packet before every press.
    pub pad_release_first: bool,
}

fn default_install_target() -> InstallTarget {
    InstallTarget::Sd
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hosts: Vec::new(),
            active_host: None,
            mtp_serial: String::new(),
            install_target: default_install_target(),
            syscon_addr: String::new(),
            syscon_port: 56789,
            download_dir: String::new(),
            rd_api_key: String::new(),
            tb_api_key: String::new(),
            gamesearch_base: "http://localhost:3011".into(),
            covers_base: "https://covers.debrid.in".into(),
            aria2_path: String::new(),
            split_bytes: 4 * 1024 * 1024 * 1024 - 1024 * 1024,
            mcp_port: 3939,
            mcp_enabled: true,
            auto_install: true,
            pad_release_first: true,
        }
    }
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("click", "godsendswitch", "godsend-switch")
}

pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(std::env::temp_dir)
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn jobs_path() -> PathBuf {
    data_dir().join("jobs.json")
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!("config at {} is corrupt ({}), using defaults", path.display(), e);
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn active(&self) -> Option<Host> {
        let id = self.active_host.as_ref()?;
        self.hosts.iter().find(|h| &h.id == id).cloned()
    }

    pub fn require_active(&self) -> Result<Host> {
        self.active()
            .ok_or_else(|| Error::device("no FTP host is configured; add one on the Devices page"))
    }

    /// Merge a partial JSON object into the config. The caller saves.
    pub fn patch(&mut self, patch: serde_json::Value) -> Result<()> {
        let mut base = serde_json::to_value(&*self)?;
        merge(&mut base, patch);
        *self = serde_json::from_value(base)?;
        Ok(())
    }
}

fn merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

/// Best-effort local IPv4 address for the net-install URL we hand to the
/// console. Opened as a UDP socket; no packet is sent.
pub fn local_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_ids_and_defaults() {
        let mut host = Host::new("Living room", "192.168.1.50");
        assert_eq!(host.id, "192.168.1.50:5000");
        assert_eq!(host.ftp_user, "anonymous");
        host.ftp_port = 5001;
        host.refresh_id();
        assert_eq!(host.id, "192.168.1.50:5001");
    }

    #[test]
    fn patch_merges_fields() {
        let mut cfg = Config::default();
        cfg.patch(serde_json::json!({ "mcp_port": 4040, "syscon_addr": "192.168.1.50" }))
            .unwrap();
        assert_eq!(cfg.mcp_port, 4040);
        assert_eq!(cfg.syscon_addr, "192.168.1.50");
        assert_eq!(cfg.install_target, InstallTarget::Sd);
    }

    #[test]
    fn install_target_round_trips() {
        let json = serde_json::to_string(&InstallTarget::Tinfoil).unwrap();
        assert_eq!(json, "\"tinfoil\"");
        assert_eq!(serde_json::from_str::<InstallTarget>("\"network\"").unwrap(), InstallTarget::Network);
    }
}
