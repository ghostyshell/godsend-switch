//! Preflight checks shown on the Console page and by `godsend-switch doctor`.

use crate::config::Config;
use crate::device::{self, DeviceStatus};
use crate::mtp::Kind;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub fn run(config: &Config) -> Vec<Check> {
    let status = device::probe(config);
    checks(config, &status)
}

pub fn checks(config: &Config, status: &DeviceStatus) -> Vec<Check> {
    let mut out = Vec::new();
    out.push(if status.mtp_ok {
        Check {
            name: "MTP responder".into(),
            ok: true,
            detail: format!(
                "{} ({} storages){}",
                status.mtp_product,
                status.storages.len(),
                if status.mtp_serial.is_empty() { String::new() } else { format!(" - serial {}", status.mtp_serial) }
            ),
        }
    } else {
        Check {
            name: "MTP responder".into(),
            ok: false,
            detail: "start DBI's MTP responder and connect USB".into(),
        }
    });

    out.push(Check {
        name: "install storage".into(),
        ok: status.mtp_ok
            && (status.storages.iter().any(|s| s.kind == Kind::SdInstall)
                || status.storages.iter().any(|s| s.kind == Kind::NandInstall)),
        detail: if status.mtp_ok {
            "SD install / NAND install visible over MTP".into()
        } else {
            "needs the MTP responder".into()
        },
    });

    out.push(if status.tinfoil_ok {
        Check {
            name: "Tinfoil USB".into(),
            ok: true,
            detail: "console is in USB install mode".into(),
        }
    } else {
        Check {
            name: "Tinfoil USB".into(),
            ok: false,
            detail: "optional: open a Tinfoil-family installer and pick USB".into(),
        }
    });

    match config.active() {
        Some(host) => out.push(Check {
            name: "FTP host".into(),
            ok: status.ftp_ok,
            detail: if status.ftp_ok {
                format!("{} ms", status.ftp_ms)
            } else {
                format!("{}:{} did not answer", host.addr, host.ftp_port)
            },
        }),
        None => out.push(Check {
            name: "FTP host".into(),
            ok: false,
            detail: "optional: add one on the Devices page for file transfers".into(),
        }),
    }

    out.push(Check {
        name: "sys-con pad".into(),
        ok: status.syscon_ok,
        detail: if status.syscon_ok {
            format!("{}:{}", config.syscon_addr, config.syscon_port)
        } else {
            "optional: set the console address in Settings and enable network_controller=1".into()
        },
    });

    let dir = crate::aria2::download_dir(config);
    let writable = std::fs::create_dir_all(&dir).is_ok()
        && std::fs::write(dir.join(".probe"), b"ok").is_ok();
    let _ = std::fs::remove_file(dir.join(".probe"));
    out.push(Check {
        name: "Download folder".into(),
        ok: writable,
        detail: dir.to_string_lossy().to_string(),
    });

    out.push(match crate::aria2::find_aria2(&config.aria2_path) {
        Some(path) => Check {
            name: "aria2c".into(),
            ok: true,
            detail: path.to_string_lossy().to_string(),
        },
        None => Check {
            name: "aria2c".into(),
            ok: false,
            detail: "optional: downloads fall back to a single connection".into(),
        },
    });

    let rd = !config.rd_api_key.trim().is_empty();
    let tb = !config.tb_api_key.trim().is_empty();
    out.push(Check {
        name: "Debrid".into(),
        ok: rd || tb,
        detail: match (rd, tb) {
            (true, true) => "Real-Debrid and TorBox connected".into(),
            (true, false) => "Real-Debrid connected".into(),
            (false, true) => "TorBox connected".into(),
            (false, false) => "optional: keys unlock debrid download links".into(),
        },
    });

    out.push(Check {
        name: "MCP server".into(),
        ok: config.mcp_enabled,
        detail: if config.mcp_enabled {
            format!("http://127.0.0.1:{}/mcp", config.mcp_port)
        } else {
            "off (Settings)".into()
        },
    });

    out
}

pub fn human_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
    }

    #[test]
    fn missing_console_is_reported_not_panicked() {
        let config = Config::default();
        let status = DeviceStatus::default();
        let checks = checks(&config, &status);
        assert!(checks.iter().any(|c| c.name == "MTP responder" && !c.ok));
        assert!(checks.iter().any(|c| c.name == "MCP server"));
    }
}
