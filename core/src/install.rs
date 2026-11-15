//! Putting files on the console, over whichever transport is selected.

use crate::config::{Config, InstallTarget};
use crate::error::{Error, Result};
use crate::mtp::{Kind, MtpSession};
use crate::netinstall;
use crate::nsp;
use crate::tinfoil;
use crate::transfer::JobContext;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct InstallReport {
    pub transport: String,
    pub destination: String,
    pub bytes: u64,
}

/// Send one local file over the configured transport.
pub fn install(config: &Config, path: &Path, ctx: &JobContext) -> Result<InstallReport> {
    let path = &crate::library::package_for_install(path);
    let info = nsp::info(path)?;
    if info.format == nsp::Format::Nro {
        return install_nro(config, path, ctx);
    }
    match config.install_target {
        InstallTarget::Sd | InstallTarget::Nand => {
            let kind = if config.install_target == InstallTarget::Nand {
                Kind::NandInstall
            } else {
                Kind::SdInstall
            };
            let session = MtpSession::open(&config.mtp_serial)?;
            if !session.has_install(kind) {
                return Err(Error::device(format!(
                    "the responder has no '{}' storage; use Tinfoil or network install instead",
                    kind.label()
                )));
            }
            let bytes = session.upload(kind, "/", path, ctx)?;
            Ok(InstallReport {
                transport: "mtp".into(),
                destination: kind.label().into(),
                bytes,
            })
        }
        InstallTarget::Tinfoil => {
            tinfoil::install(&[path.to_path_buf()], ctx)?;
            Ok(InstallReport {
                transport: "tinfoil".into(),
                destination: "USB install".into(),
                bytes: std::fs::metadata(path)?.len(),
            })
        }
        InstallTarget::Network => {
            let host = config.require_active()?;
            netinstall::install(&[path.to_path_buf()], &host.addr, netinstall::PORT, ctx)?;
            Ok(InstallReport {
                transport: "network".into(),
                destination: format!("{}:{}", host.addr, netinstall::PORT),
                bytes: std::fs::metadata(path)?.len(),
            })
        }
    }
}

/// NRO homebrew always belongs in /switch on the SD card.
fn install_nro(config: &Config, path: &Path, ctx: &JobContext) -> Result<InstallReport> {
    let session = MtpSession::open(&config.mtp_serial)?;
    let bytes = session.upload(Kind::Sd, "/switch", path, ctx)?;
    Ok(InstallReport {
        transport: "mtp".into(),
        destination: "/switch".into(),
        bytes,
    })
}

/// Install several files over one transport session where possible.
pub fn install_many(config: &Config, files: &[std::path::PathBuf], ctx: &JobContext) -> Result<Vec<InstallReport>> {
    let mut reports = Vec::new();
    for file in files {
        ctx.check_cancel()?;
        reports.push(install(config, file, ctx)?);
    }
    Ok(reports)
}

/// Where a local file would go, for the UI.
pub fn destination(config: &Config, path: &Path) -> String {
    match nsp::info(path) {
        Ok(info) if info.format == nsp::Format::Nro => "/switch (SD)".into(),
        _ => match config.install_target {
            InstallTarget::Sd => "SD install".into(),
            InstallTarget::Nand => "NAND install".into(),
            InstallTarget::Tinfoil => "Tinfoil USB".into(),
            InstallTarget::Network => "Network install".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_follows_the_target() {
        let mut config = Config::default();
        config.install_target = InstallTarget::Nand;
        assert_eq!(destination(&config, Path::new("/x/game.nsp")), "NAND install");
        config.install_target = InstallTarget::Tinfoil;
        assert_eq!(destination(&config, Path::new("/x/game.nsp")), "Tinfoil USB");
    }
}
