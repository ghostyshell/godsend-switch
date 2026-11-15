//! Switch package files: format sniffing, title ids and FAT32 split sets.
//!
//! Nothing here decrypts anything. An NSP is a PFS0 container; its title id
//! is not in the PFS0 header, so we take it from the conventional
//! `Name [0100xxxxxxxxxxxx][v0].nsp` filename. NSZ/XCZ are zstd-compressed
//! containers and are recognised, not decoded.

use crate::error::{Error, Result};
use crate::transfer::JobContext;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// 4 GiB minus 1 MiB, the FAT32-safe chunk DBI and Tinfoil expect.
pub const DEFAULT_CHUNK: u64 = 4 * 1024 * 1024 * 1024 - 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Nsp,
    Xci,
    Nsz,
    Xcz,
    Nro,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub format: Format,
    pub title_id: Option<String>,
    /// First part count when this is `name.nsp.00` style split set.
    pub parts: usize,
    /// True when the file is a split part rather than a whole image.
    pub is_part: bool,
}

pub fn zstd_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == [0x28, 0xB5, 0x2F, 0xFD]
}

/// PFS0 header fields when the bytes start with one.
pub fn pfs0_header(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 12 || &bytes[0..4] != b"PFS0" {
        return None;
    }
    let count = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let strings = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    Some((count, strings))
}

/// XCI files carry the string `HEAD` at offset 0x100.
pub fn xci_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 0x104 && &bytes[0x100..0x104] == b"HEAD"
}

pub fn format_from_name(name: &str) -> Format {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".nsp") || lower.contains(".nsp.") {
        Format::Nsp
    } else if lower.ends_with(".xci") || lower.contains(".xci.") {
        Format::Xci
    } else if lower.ends_with(".nsz") || lower.contains(".nsz.") {
        Format::Nsz
    } else if lower.ends_with(".xcz") || lower.contains(".xcz.") {
        Format::Xcz
    } else if lower.ends_with(".nro") {
        Format::Nro
    } else {
        Format::Unknown
    }
}

/// Find a 16 character title id, preferring the `[0100...]` filename slot.
pub fn title_id_from_name(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i + 18 <= bytes.len() {
        if bytes[i] == b'[' && bytes[i + 17] == b']' {
            if let Ok(text) = std::str::from_utf8(&bytes[i + 1..i + 17]) {
                if text.starts_with("0100") && text.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(text.to_ascii_lowercase());
                }
            }
        }
        i += 1;
    }
    // Fall back to a bare id in the name, e.g. 0100152000022000.nsp.
    for start in 0..bytes.len().saturating_sub(15) {
        if let Ok(text) = std::str::from_utf8(&bytes[start..start + 16]) {
            if text.starts_with("0100") && text.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(text.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Part index for `name.nsp.02`, or None for a whole file. The base must end
/// in a package extension, so an ordinary `Game.07` file is not mistaken for
/// a split part.
pub fn part_index(name: &str) -> Option<usize> {
    let (base, ext) = name.rsplit_once('.')?;
    if ext.len() != 2 || !ext.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let lower = base.to_ascii_lowercase();
    if [".nsp", ".xci", ".nsz", ".xcz", ".nro", ".iso"]
        .iter()
        .any(|package| lower.ends_with(package))
    {
        ext.parse().ok()
    } else {
        None
    }
}

/// Strip a `.NN` part suffix: `game.nsp.01` -> `game.nsp`.
pub fn strip_part(name: &str) -> String {
    if part_index(name).is_some() {
        if let Some((base, _)) = name.rsplit_once('.') {
            return base.to_string();
        }
    }
    name.to_string()
}

pub fn info(path: &Path) -> Result<FileInfo> {
    let meta = std::fs::metadata(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut head = [0u8; 0x120];
    let mut file = File::open(path)?;
    let mut read = 0usize;
    while read < head.len() {
        let got = file.read(&mut head[read..])?;
        if got == 0 {
            break;
        }
        read += got;
    }
    let bytes = &head[..read];
    let by_name = format_from_name(&name);
    let format = if zstd_magic(bytes) {
        if by_name == Format::Xcz { Format::Xcz } else { Format::Nsz }
    } else if pfs0_header(bytes).is_some() {
        Format::Nsp
    } else if xci_magic(bytes) {
        Format::Xci
    } else {
        by_name
    };
    let base = strip_part(&name);
    let parts = if part_index(&name).is_some() {
        find_parts(&path.with_file_name(&base)).len()
    } else {
        0
    };
    Ok(FileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        size: meta.len(),
        format,
        title_id: title_id_from_name(&base),
        parts,
        is_part: part_index(path.file_name().unwrap_or_default().to_string_lossy().as_ref()).is_some(),
    })
}

pub fn part_path(base: &Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.{:02}", base.to_string_lossy(), index))
}

/// Split into `<name>.00`, `<name>.01`, ... and return the parts.
pub fn split(src: &Path, chunk: u64, ctx: &JobContext) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    let result = split_into(src, chunk, ctx, &mut created);
    if result.is_err() {
        for part in &created {
            let _ = std::fs::remove_file(part);
        }
    }
    result
}

fn split_into(
    src: &Path,
    chunk: u64,
    ctx: &JobContext,
    created: &mut Vec<PathBuf>,
) -> Result<Vec<PathBuf>> {
    if chunk == 0 {
        return Err(Error::Invalid("split size must be greater than zero".into()));
    }
    let total = std::fs::metadata(src)?.len();
    let mut input = File::open(src)?;
    let mut parts = Vec::new();
    let mut done = 0u64;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut index = 0usize;
    ctx.progress(0, total);
    while done < total {
        ctx.check_cancel()?;
        let part = part_path(src, index);
        let mut output = File::create(&part)?;
        let mut left = (total - done).min(chunk);
        while left > 0 {
            ctx.check_cancel()?;
            let want = left.min(buf.len() as u64) as usize;
            let got = input.read(&mut buf[..want])?;
            if got == 0 {
                return Err(Error::Invalid(format!("{} ended early", src.display())));
            }
            output.write_all(&buf[..got])?;
            left -= got as u64;
            done += got as u64;
            ctx.progress(done, total);
        }
        drop(output);
        created.push(part.clone());
        parts.push(part);
        index += 1;
    }
    ctx.progress(total, total);
    Ok(parts)
}

/// Find `base.00`, `base.01`, ... in order.
pub fn find_parts(base: &Path) -> Vec<PathBuf> {
    let mut parts = Vec::new();
    for index in 0..100 {
        let part = part_path(base, index);
        if part.is_file() {
            parts.push(part);
        } else {
            break;
        }
    }
    parts
}

/// Join a split set into `dest`.
pub fn merge(parts: &[PathBuf], dest: &Path, ctx: &JobContext) -> Result<u64> {
    if parts.is_empty() {
        return Err(Error::Invalid("no split parts to merge".into()));
    }
    if part_index(&dest.file_name().unwrap_or_default().to_string_lossy()).is_some() {
        return Err(Error::Invalid(
            "output path is a split part; choose a whole package name".into(),
        ));
    }
    let total: u64 = parts
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let staging = PathBuf::from(format!("{}.part", dest.to_string_lossy()));
    let result = (|| -> Result<u64> {
        let mut output = File::create(&staging)?;
        let mut done = 0u64;
        let mut buf = vec![0u8; 1024 * 1024];
        ctx.progress(0, total);
        for part in parts {
            let mut input = File::open(part)?;
            loop {
                ctx.check_cancel()?;
                let got = input.read(&mut buf)?;
                if got == 0 {
                    break;
                }
                output.write_all(&buf[..got])?;
                done += got as u64;
                ctx.progress(done, total);
            }
        }
        output.flush()?;
        Ok(done)
    })();
    match result {
        Ok(done) => {
            std::fs::rename(&staging, dest)?;
            Ok(done)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staging);
            Err(e)
        }
    }
}

/// Read the first `max` bytes of a split set as one file.
pub fn read_head(parts: &[PathBuf], max: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(max);
    for part in parts {
        let mut file = File::open(part)?;
        let mut chunk = vec![0u8; max.saturating_sub(out.len()).min(0x10000)];
        let got = file.read(&mut chunk)?;
        out.extend_from_slice(&chunk[..got]);
        if out.len() >= max {
            break;
        }
    }
    Ok(out)
}

/// Offset of the first byte after `parts[..index]`, for range reads.
pub fn part_offset(parts: &[PathBuf], index: usize) -> u64 {
    parts
        .iter()
        .take(index)
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

pub fn seek_hint(path: &Path) -> Result<u64> {
    Ok(File::open(path)?.metadata()?.len())
}

pub fn read_at(path: &Path, offset: u64, len: usize) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    let got = file.read(&mut buf)?;
    buf.truncate(got);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pfs0() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PFS0");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_pfs0_header() {
        assert_eq!(pfs0_header(&pfs0()), Some((2, 16)));
        assert_eq!(pfs0_header(b"PFS0\0\0"), None);
        assert_eq!(pfs0_header(b"NOPE\0\0\0\0\0\0\0\0"), None);
    }

    #[test]
    fn detects_xci_magic() {
        let mut bytes = vec![0u8; 0x104];
        bytes[0x100..0x104].copy_from_slice(b"HEAD");
        assert!(xci_magic(&bytes));
        assert!(!xci_magic(&pfs0()));
    }

    #[test]
    fn title_ids_from_filenames() {
        assert_eq!(
            title_id_from_name("Mario Kart 8 Deluxe [0100152000022000][v65536].nsp").as_deref(),
            Some("0100152000022000")
        );
        assert_eq!(
            title_id_from_name("0100000000010000.nsp").as_deref(),
            Some("0100000000010000")
        );
        assert_eq!(title_id_from_name("homebrew.nro"), None);
        assert_eq!(title_id_from_name("[NOTATITLEID0000].nsp"), None);
    }

    #[test]
    fn part_names() {
        assert_eq!(part_index("game.nsp.01"), Some(1));
        assert_eq!(part_index("game.nsp"), None);
        assert_eq!(part_index("Game.07"), None);
        assert_eq!(strip_part("game.nsp.02"), "game.nsp");
        assert_eq!(strip_part("game.nsp"), "game.nsp");
        assert_eq!(part_path(Path::new("/x/game.nsp"), 3), PathBuf::from("/x/game.nsp.03"));
    }

    #[test]
    fn merge_refuses_to_overwrite_a_part() {
        let dir = std::env::temp_dir().join(format!("godsend-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("game.nsp.00");
        std::fs::write(&part, b"abc").unwrap();
        let ctx = crate::transfer::JobContext::for_test();
        let parts = vec![part.clone()];
        assert!(merge(&parts, &part, &ctx).is_err());
        assert_eq!(std::fs::read(&part).unwrap(), b"abc");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn split_and_merge_round_trip() {
        let dir = std::env::temp_dir().join(format!("godsend-nsp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("test.nsp");
        let data: Vec<u8> = (0..(3 * 1024 * 1024 + 7)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&src, &data).unwrap();
        let ctx = crate::transfer::JobContext::for_test();
        let parts = split(&src, 1024 * 1024, &ctx).unwrap();
        assert_eq!(parts.len(), 4);
        let found = find_parts(&src);
        assert_eq!(found.len(), parts.len());
        let out = dir.join("joined.nsp");
        merge(&found, &out, &ctx).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), data);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
