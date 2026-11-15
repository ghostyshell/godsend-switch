//! Games, homebrew and local packages.

use crate::error::Result;
use crate::mtp::{Kind, MtpSession, RemoteEntry};
use crate::nsp;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// A title the console reports as installed.
    Installed,
    /// A package on the SD card or in the download folder.
    Package,
    /// An NRO homebrew app.
    Homebrew,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub title_id: Option<String>,
    pub size: u64,
    pub kind: EntryKind,
    pub format: nsp::Format,
    /// Storage label or "local".
    pub location: String,
    /// MTP path or local path.
    pub path: String,
    pub cover: Option<String>,
}

pub fn cover_url(base: &str, title_id: &str) -> Option<String> {
    let base = base.trim().trim_end_matches('/');
    let id = title_id.trim().to_uppercase();
    if base.is_empty() || id.len() != 16 {
        return None;
    }
    Some(format!("{base}/switch/repo/COV/{id}.JPG"))
}

fn entry_from_remote(
    row: &RemoteEntry,
    kind: EntryKind,
    location: &str,
    covers: &str,
) -> Entry {
    let base = nsp::strip_part(&row.name);
    let title_id = nsp::title_id_from_name(&base);
    Entry {
        name: base,
        title_id: title_id.clone(),
        size: row.size,
        kind,
        format: nsp::format_from_name(&row.name),
        location: location.to_string(),
        path: row.path.clone(),
        cover: title_id.as_deref().and_then(|id| cover_url(covers, id)),
    }
}

/// What the console's MTP responder reports as installed.
pub fn installed(session: &MtpSession, covers: &str) -> Result<Vec<Entry>> {
    let label = Kind::Installed.label();
    let rows = session.list(Kind::Installed, "/")?;
    let mut out: Vec<Entry> = rows
        .iter()
        .filter(|r| !r.is_dir)
        .map(|r| entry_from_remote(r, EntryKind::Installed, label, covers))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// NROs and packages under a directory on the SD card.
pub fn sd_homebrew(session: &MtpSession, dir: &str, covers: &str) -> Result<Vec<Entry>> {
    let label = Kind::Sd.label();
    let rows = session.list(Kind::Sd, dir)?;
    let mut out: Vec<Entry> = rows
        .iter()
        .filter(|r| !r.is_dir)
        .map(|r| entry_from_remote(r, EntryKind::Homebrew, label, covers))
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Local packages: NSP, XCI, NSZ, XCZ, NRO, including split parts.
pub fn local(dir: &Path, covers: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let Ok(rows) = std::fs::read_dir(dir) else {
        return out;
    };
    for row in rows.flatten() {
        let path = row.path();
        if !path.is_file() {
            continue;
        }
        let name = row.file_name().to_string_lossy().to_string();
        let format = nsp::format_from_name(&name);
        if format == nsp::Format::Unknown {
            continue;
        }
        if nsp::part_index(&name).is_some() {
            continue; // listed through the first part
        }
        let size = row.metadata().map(|m| m.len()).unwrap_or(0);
        let base = nsp::strip_part(&name);
        let title_id = nsp::title_id_from_name(&base);
        out.push(Entry {
            name: base,
            title_id: title_id.clone(),
            size,
            kind: if format == nsp::Format::Nro { EntryKind::Homebrew } else { EntryKind::Package },
            format,
            location: "local".into(),
            path: path.to_string_lossy().to_string(),
            cover: title_id.as_deref().and_then(|id| cover_url(covers, id)),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Folders under `dir` on the SD card (e.g. /switch entries).
pub fn sd_folders(session: &MtpSession, dir: &str) -> Result<Vec<String>> {
    Ok(session
        .list(Kind::Sd, dir)?
        .into_iter()
        .filter(|r| r.is_dir)
        .map(|r| r.name)
        .collect())
}

/// Choose the file that should be sent for a local path (first split part).
pub fn package_for_install(path: &Path) -> PathBuf {
    if nsp::part_index(&path.file_name().unwrap_or_default().to_string_lossy()).is_some() {
        if let Some(parent) = path.parent() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let base = nsp::strip_part(&name);
            return parent.join(base);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_urls_need_a_full_id() {
        assert_eq!(
            cover_url("https://covers.debrid.in/", "0100152000022000").as_deref(),
            Some("https://covers.debrid.in/switch/repo/COV/0100152000022000.JPG")
        );
        assert_eq!(cover_url("https://covers.debrid.in", "0100"), None);
        assert_eq!(cover_url("", "0100152000022000"), None);
    }

    #[test]
    fn local_scan_reads_titles_and_skips_parts() {
        let dir = std::env::temp_dir().join(format!("godsend-lib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Game [0100152000022000][v0].nsp"), b"x").unwrap();
        std::fs::write(dir.join("Game [0100152000022000][v0].nsp.00"), b"x").unwrap();
        std::fs::write(dir.join("tool.nro"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let entries = local(&dir, "https://covers.debrid.in");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Game [0100152000022000][v0].nsp");
        assert_eq!(entries[0].title_id.as_deref(), Some("0100152000022000"));
        assert_eq!(entries[1].kind, EntryKind::Homebrew);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn split_parts_resolve_to_the_base() {
        assert_eq!(
            package_for_install(Path::new("/x/game.nsp.03")),
            PathBuf::from("/x/game.nsp")
        );
        assert_eq!(
            package_for_install(Path::new("/x/game.nsp")),
            PathBuf::from("/x/game.nsp")
        );
    }
}
