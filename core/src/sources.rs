//! Store sources: the game-search debrid index (PS3 catalogue) and Internet
//! Archive PS3 sets.

use crate::error::Result;
use serde::{Deserialize, Serialize};

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(25))
        .build()
}

// ---------- game-search ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLink {
    #[serde(default)]
    pub id: String,
    pub host: String,
    pub url: String,
    #[serde(rename = "linkType", default)]
    pub link_type: String, // ddl | magnet | torrent | source
    #[serde(rename = "supportedBy", default)]
    pub supported_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameHit {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "gameSlug", alias = "slug")]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "linkType", default)]
    pub link_type: String,
    #[serde(rename = "supportedBy", default)]
    pub supported_by: Vec<String>,
    #[serde(rename = "rawgCover", default)]
    pub rawg_cover: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(rename = "rawgRating", default)]
    pub rating: Option<f64>,
    #[serde(rename = "rawgReleased", default)]
    pub released: Option<String>,
    #[serde(rename = "linkCount", default)]
    pub link_count: u32,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<GameLink>,
}

/// The public site serves `/api/games`; the Go service serves `/games`.
fn games_url(base: &str, query: &str, limit: u32) -> String {
    let base = base.trim_end_matches('/');
    let path = if base.ends_with("/api") { "games" } else { "games" };
    let mut url = format!("{base}/{path}?platform=switch&limit={limit}");
    if !query.is_empty() {
        url.push_str(&format!("&q={}", urlencode(query)));
    }
    url
}

/// Browse or search the index for PS3 games.
pub fn gamesearch(base: &str, query: &str, limit: u32) -> Result<Vec<GameHit>> {
    let body: serde_json::Value = agent().get(&games_url(base, query, limit)).call()?.into_json()?;
    let list = body
        .get("games")
        .and_then(|g| g.as_array())
        .or_else(|| body.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for value in list {
        let Ok(mut hit) = serde_json::from_value::<GameHit>(value) else {
            continue;
        };
        if hit.title.is_empty() {
            hit.title = hit.slug.replace('-', " ");
        }
        if hit.id.is_empty() {
            hit.id = hit.slug.clone();
        }
        out.push(hit);
    }
    Ok(out)
}

/// Fetch one game's full link list. Works against the Go service; against the
/// public frontend the endpoint is protected, so the caller gets a clear
/// "sign in / point the app at the backend" path instead of a silent empty.
pub fn gamesearch_detail(base: &str, slug: &str) -> Result<GameHit> {
    let base = base.trim_end_matches('/');
    let url = format!("{base}/games/{}", urlencode(slug));
    let value: serde_json::Value = agent().get(&url).call()?.into_json()?;
    let game = value.get("game").cloned().unwrap_or_else(|| value.clone());
    let mut hit: GameHit = serde_json::from_value(game)?;
    if hit.title.is_empty() {
        hit.title = hit.slug.replace('-', " ");
    }
    if let Some(links) = value.get("links").or_else(|| value.get("game_links")) {
        hit.links = serde_json::from_value(links.clone()).unwrap_or_default();
    }
    Ok(hit)
}

// ---------- Internet Archive ----------

#[derive(Debug, Clone, Serialize)]
pub struct IaItem {
    pub id: String,
    pub title: String,
    pub files_count: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IaFile {
    pub name: String,
    pub size_bytes: u64,
    pub url: String,
}

pub fn ia_search(query: &str, rows: u32) -> Result<Vec<IaItem>> {
    let q = if query.trim().is_empty() {
        "PS3".to_string()
    } else {
        format!("{query} AND PS3")
    };
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=item_size&fl%5B%5D=files_count&rows={}&page=1&output=json",
        urlencode(&format!("{q} AND mediatype:(software)")),
        rows.clamp(1, 100)
    );
    let value: serde_json::Value = agent().get(&url).call()?.into_json()?;
    let docs = value
        .pointer("/response/docs")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for doc in docs {
        let id = doc.get("identifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        out.push(IaItem {
            title: doc.get("title").and_then(|t| t.as_str()).unwrap_or(&id).to_string(),
            files_count: doc.get("files_count").and_then(|f| f.as_u64()).unwrap_or(0),
            size_bytes: doc
                .get("item_size")
                .and_then(|s| s.as_u64())
                .or_else(|| doc.get("item_size").and_then(|s| s.as_str()).and_then(|s| s.parse().ok()))
                .unwrap_or(0),
            id,
        });
    }
    Ok(out)
}

pub fn ia_seed_queries() -> Vec<&'static str> {
    vec![
        "Nintendo Switch NSP collection",
        "Switch game archive",
        "Switch homebrew",
        "Switch XCI",
    ]
}

pub fn ia_item_files(id: &str) -> Result<Vec<IaFile>> {
    let url = format!("https://archive.org/metadata/{}", urlencode(id));
    let value: serde_json::Value = agent().get(&url).call()?.into_json()?;
    let files = value.get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default();
    let server = value
        .get("server")
        .and_then(|s| s.as_str())
        .unwrap_or("archive.org");
    let dir = value.get("dir").and_then(|d| d.as_str()).unwrap_or(id);
    let mut out = Vec::new();
    for file in files {
        let name = file.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let lower = name.to_lowercase();
        if !(lower.ends_with(".nsp")
            || lower.ends_with(".xci")
            || lower.ends_with(".nsz")
            || lower.ends_with(".xcz")
            || lower.ends_with(".nro")
            || lower.ends_with(".zip"))
        {
            continue;
        }
        let size = file
            .get("size")
            .and_then(|s| s.as_u64())
            .or_else(|| file.get("size").and_then(|s| s.as_str()).and_then(|s| s.parse().ok()))
            .unwrap_or(0);
        out.push(IaFile {
            url: format!("https://{server}/{dir}/{}", urlencode(&name)),
            name,
            size_bytes: size,
        });
    }
    out.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    Ok(out)
}

pub fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding() {
        assert_eq!(urlencode("PS3 ISO"), "PS3%20ISO");
        assert_eq!(urlencode("a+b/c"), "a%2Bb%2Fc");
    }

    #[test]
    fn parses_a_public_games_row() {
        let value = serde_json::json!({
            "slug": "god-of-war-collection",
            "title": "God of War Collection",
            "platform": "switch",
            "source": "minerva",
            "linkCount": 1,
            "rawgCover": "https://x/cover.jpg"
        });
        let hit: GameHit = serde_json::from_value(value).unwrap();
        assert_eq!(hit.slug, "god-of-war-collection");
        assert_eq!(hit.link_count, 1);
        assert_eq!(hit.rawg_cover.as_deref(), Some("https://x/cover.jpg"));
    }
}
