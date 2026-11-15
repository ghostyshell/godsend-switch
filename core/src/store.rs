//! Store download flow: pick a link, resolve it through debrid when a key is
//! configured, download with aria2, leave the file in the download folder.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::sources::{GameHit, GameLink};
use crate::transfer::JobContext;
use std::path::PathBuf;

/// Torrents first (debrid providers accelerate them), then DDL.
fn pick_link(hit: &GameHit) -> Option<&GameLink> {
    hit.links
        .iter()
        .filter(|l| l.link_type != "source" && !l.url.is_empty())
        .max_by_key(|l| {
            let torrent_bonus = if l.link_type == "ddl" { 0 } else { 2 };
            l.supported_by.len() * 10 + torrent_bonus
        })
}

pub fn download_hit(hit: &GameHit, config: &Config, ctx: &JobContext) -> Result<PathBuf> {
    let link = pick_link(hit).ok_or_else(|| {
        Error::NotFound(format!(
            "no download links for {} (set the game-search backend in Settings and refresh)",
            hit.title
        ))
    })?;
    ctx.message(format!("resolving via {} ({})", link.host, link.link_type));
    let direct = crate::debrid::resolve(
        &link.url,
        &link.supported_by,
        &config.rd_api_key,
        &config.tb_api_key,
    )?;
    let name = file_name_for(hit, &direct);
    download_url(&direct, &name, config, ctx)
}

pub fn download_url(url: &str, name: &str, config: &Config, ctx: &JobContext) -> Result<PathBuf> {
    let dest_dir = crate::aria2::download_dir(config);
    crate::aria2::download(url, &dest_dir, name, config, ctx)
}

pub fn file_name_for(hit: &GameHit, direct_url: &str) -> String {
    let path = direct_url.split('?').next().unwrap_or(direct_url);
    let candidate = path.rsplit('/').next().unwrap_or("");
    if candidate.len() > 4 && candidate.contains('.') {
        return safe_name(candidate);
    }
    let ext = if direct_url.starts_with("magnet:") || hit.link_type == "magnet" {
        "torrent"
    } else if direct_url.contains(".pkg") {
        "pkg"
    } else if direct_url.contains(".iso") {
        "iso"
    } else {
        "zip"
    };
    format!("{}.{}", safe_name(&hit.title), ext)
}

/// Filesystem-safe name keeping the extension.
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "download".into()
    } else {
        cleaned.chars().take(180).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(links: Vec<GameLink>) -> GameHit {
        GameHit {
            id: "1".into(),
            slug: "g".into(),
            title: "God of War: Collection".into(),
            platform: "switch".into(),
            source: "minerva".into(),
            host: String::new(),
            url: String::new(),
            link_type: "ddl".into(),
            supported_by: vec![],
            rawg_cover: None,
            cover: None,
            rating: None,
            released: None,
            link_count: links.len() as u32,
            size: None,
            links,
        }
    }

    #[test]
    fn prefers_torrents_and_skips_source_pages() {
        let h = hit(vec![
            GameLink { id: "s".into(), host: "src".into(), url: "https://post".into(), link_type: "source".into(), supported_by: vec![] },
            GameLink { id: "d".into(), host: "fh".into(), url: "https://fh/f.iso".into(), link_type: "ddl".into(), supported_by: vec![] },
            GameLink { id: "m".into(), host: "tb".into(), url: "magnet:?xt=1".into(), link_type: "magnet".into(), supported_by: vec!["torbox".into(), "realdebrid".into()] },
        ]);
        assert_eq!(pick_link(&h).unwrap().id, "m");
    }

    #[test]
    fn names_files_safely() {
        assert_eq!(file_name_for(&hit(vec![]), "https://h/God%20of%20War.iso?x=1"), "God%20of%20War.iso");
        assert_eq!(safe_name("a/b:c"), "a_b_c");
        assert_eq!(file_name_for(&hit(vec![]), "https://h/get"), "God of War_ Collection.zip");
    }
}
