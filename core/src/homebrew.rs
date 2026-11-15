//! Homebrew catalog: GitHub projects that publish ready-to-run NROs.
//!
//! A refresh is one API call per project; the newest release's NRO asset is
//! recorded. Installing downloads the NRO and pushes it to /switch on the SD
//! card (MTP first, FTP as the fallback).

use crate::config::Config;
use crate::error::{Error, Result};
use crate::mtp::{Kind, MtpSession};
use crate::transfer::JobContext;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub repo: String,
    /// Filename fragment that identifies the NRO in a release.
    pub asset: String,
    #[serde(default)]
    pub latest_tag: Option<String>,
    #[serde(default)]
    pub nro_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
}

fn entry(id: &str, name: &str, description: &str, category: &str, repo: &str, asset: &str) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: category.into(),
        repo: repo.into(),
        asset: asset.into(),
        latest_tag: None,
        nro_url: None,
        published_at: None,
        size_bytes: 0,
    }
}

pub fn bundled_catalog() -> Vec<CatalogEntry> {
    vec![
        entry(
            "hb-appstore",
            "Homebrew App Store",
            "Browse and install homebrew from the console itself.",
            "store",
            "fortheusers/hb-appstore",
            "appstore.nro",
        ),
        entry(
            "ftpd",
            "ftpd",
            "Minimal FTP server for the SD card. The app's FTP transport talks to this.",
            "tool",
            "mtheall/ftpd",
            "ftpd.nro",
        ),
        entry(
            "sys-con",
            "sys-con",
            "USB and network controller support. Set network_controller=1 to use the app's virtual pad.",
            "system",
            "o0Zz/sys-con",
            "sys-con.nro",
        ),
        entry(
            "nx-shell",
            "NX-Shell",
            "File manager with copy, move, delete and archive support.",
            "tool",
            "joel16/NX-Shell",
            "NX-Shell.nro",
        ),
        entry(
            "dbi",
            "DBI",
            "Installer and MTP responder. Its MTP mode is the app's primary USB transport.",
            "installer",
            "rashevskyv/dbi",
            "DBI.nro",
        ),
        entry(
            "awoo",
            "Awoo Installer",
            "Installs NSP, NSZ, XCI and XCZ over USB or the LAN.",
            "installer",
            "Huntereb/Awoo-Installer",
            "Awoo-Installer.nro",
        ),
        entry(
            "sys-ftpd",
            "sys-ftpd",
            "Background FTP server, so FTP stays available while a game runs.",
            "tool",
            "cathery/sys-ftpd",
            "sys-ftpd.nro",
        ),
        entry(
            "nxthemes",
            "NXThemes Installer",
            "Installs custom themes from the SD card.",
            "system",
            "exelix11/SwitchThemeInjector",
            "NXThemesInstaller.nro",
        ),
    ]
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(&format!("godsend-switch/{}", crate::VERSION))
        .build()
}

fn latest_release(repo: &str) -> Result<serde_json::Value> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = agent()
        .get(&url)
        .call()
        .map_err(|e| Error::Http(format!("{repo}: {e}")))?
        .into_string()
        .map_err(|e| Error::Http(format!("{repo}: {e}")))?;
    serde_json::from_str(&body).map_err(Error::from)
}

fn pick_asset(release: &serde_json::Value, fragment: &str) -> Option<(String, String, String, u64)> {
    let tag = release.get("tag_name")?.as_str()?.to_string();
    let published = release
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let assets = release.get("assets")?.as_array()?;
    let mut best: Option<&serde_json::Value> = None;
    for asset in assets {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if !name.to_ascii_lowercase().ends_with(".nro") {
            continue;
        }
        let matched = name.eq_ignore_ascii_case(fragment)
            || name.to_ascii_lowercase().contains(&fragment.to_ascii_lowercase());
        if matched {
            best = Some(asset);
            break;
        }
        if best.is_none() {
            best = Some(asset);
        }
    }
    let asset = best?;
    Some((
        tag,
        asset.get("browser_download_url")?.as_str()?.to_string(),
        published,
        asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
    ))
}

fn cache_path() -> PathBuf {
    crate::config::cache_dir().join("homebrew.json")
}

pub fn catalog(config: &Config, refresh: bool) -> Vec<CatalogEntry> {
    let mut entries = bundled_catalog();
    if !refresh {
        let fresh = std::fs::metadata(cache_path())
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .map(|age| age < std::time::Duration::from_secs(24 * 3600))
            .unwrap_or(false);
        if fresh {
            if let Ok(bytes) = std::fs::read(cache_path()) {
                if let Ok(cached) = serde_json::from_slice::<Vec<CatalogEntry>>(&bytes) {
                    if cached.len() == entries.len()
                        && cached.iter().all(|row| entries.iter().any(|e| e.id == row.id))
                    {
                        return cached;
                    }
                }
            }
        }
    }
    let _ = config;
    for row in &mut entries {
        if let Ok(release) = latest_release(&row.repo) {
            if let Some((tag, url, published, size)) = pick_asset(&release, &row.asset) {
                row.latest_tag = Some(tag);
                row.nro_url = Some(url);
                row.published_at = Some(published);
                row.size_bytes = size;
            }
        }
    }
    if let Some(parent) = cache_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&entries) {
        let _ = std::fs::write(cache_path(), bytes);
    }
    entries
}

/// Download a catalog entry and push it to /switch.
pub fn install(config: &Config, id: &str, ctx: &JobContext) -> Result<PathBuf> {
    let row = catalog(config, false)
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| Error::NotFound(format!("unknown homebrew '{id}'")))?;
    let url = row
        .nro_url
        .ok_or_else(|| Error::Other(format!("{} has no NRO asset in its latest release", row.name)))?;
    let dir = crate::aria2::download_dir(config);
    let name = url.rsplit('/').next().unwrap_or("homebrew.nro");
    let local = crate::aria2::download(&url, &dir, name, config, ctx)?;
    push(config, &local, ctx)?;
    Ok(local)
}

/// Push an NRO (or any file) to /switch, MTP first then FTP.
pub fn push(config: &Config, local: &std::path::Path, ctx: &JobContext) -> Result<String> {
    match MtpSession::open(&config.mtp_serial) {
        Ok(session) => {
            session.upload(Kind::Sd, "/switch", local, ctx)?;
            Ok("/switch".into())
        }
        Err(mtp_error) => {
            let host = config.active().ok_or(mtp_error)?;
            crate::fm::ftp_upload(&host, local, "/switch", 0, ctx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_assets_are_nros() {
        let rows = bundled_catalog();
        let mut ids: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), rows.len());
        assert!(rows.iter().all(|r| r.asset.to_ascii_lowercase().ends_with(".nro")));
        assert!(rows.iter().any(|r| r.id == "sys-con"));
        assert!(rows.iter().any(|r| r.id == "dbi"));
    }

    #[test]
    fn picks_the_named_nro_from_a_release() {
        let release = serde_json::json!({
            "tag_name": "v1.2.3",
            "published_at": "2026-11-01T00:00:00Z",
            "assets": [
                { "name": "readme.txt", "browser_download_url": "https://x/readme.txt", "size": 1 },
                { "name": "ftpd.nro", "browser_download_url": "https://x/ftpd.nro", "size": 42 }
            ]
        });
        let (tag, url, _, size) = pick_asset(&release, "ftpd.nro").unwrap();
        assert_eq!(tag, "v1.2.3");
        assert_eq!(url, "https://x/ftpd.nro");
        assert_eq!(size, 42);
    }

    #[test]
    fn skips_non_nro_assets() {
        let release = serde_json::json!({
            "tag_name": "v1",
            "assets": [{ "name": "app.zip", "browser_download_url": "https://x/app.zip", "size": 1 }]
        });
        assert!(pick_asset(&release, "app.nro").is_none());
    }
}
