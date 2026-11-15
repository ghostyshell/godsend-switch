//! MCP server: a small JSON-RPC surface over the same engine the app uses.
//!
//! The desktop app hosts it on 127.0.0.1 (starts with the app); the CLI can
//! also run it over stdio. Tools are stateless calls into `core`.

use crate::config::{Config, InstallTarget};
use crate::error::{Error, Result};
use crate::mtp::{Kind, MtpSession};
use crate::transfer::JobContext;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn kind_from_str(text: &str) -> Result<Kind> {
    Ok(match text.trim().to_ascii_lowercase().as_str() {
        "sd" => Kind::Sd,
        "nand" | "nand_user" => Kind::NandUser,
        "nand_system" => Kind::NandSystem,
        "sd_install" | "install" => Kind::SdInstall,
        "nand_install" => Kind::NandInstall,
        "installed" | "games" => Kind::Installed,
        "saves" => Kind::Saves,
        "album" => Kind::Album,
        "gamecard" => Kind::Gamecard,
        other => return Err(Error::Invalid(format!("unknown storage '{other}'"))),
    })
}

fn target_from_str(text: &str) -> Result<InstallTarget> {
    Ok(match text.trim().to_ascii_lowercase().as_str() {
        "sd" => InstallTarget::Sd,
        "nand" => InstallTarget::Nand,
        "tinfoil" | "usb" => InstallTarget::Tinfoil,
        "network" | "lan" => InstallTarget::Network,
        other => return Err(Error::Invalid(format!("unknown install target '{other}'"))),
    })
}

pub fn tool_specs() -> Value {
    let storage = |description: &str| {
        json!({
            "type": "string",
            "description": description,
            "enum": ["sd", "nand_user", "nand_system", "sd_install", "nand_install", "installed", "saves", "album", "gamecard"]
        })
    };
    json!([
        {
            "name": "switch_status",
            "description": "Probe the console: MTP storages, Tinfoil USB mode, FTP host and sys-con config.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "doctor",
            "description": "Run the app's preflight checks and return each check with ok/detail.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "list_storage",
            "description": "List files in an MTP storage (SD card, install folders, installed games, saves, album).",
            "inputSchema": {"type": "object", "properties": {
                "storage": storage("Storage unit to read."),
                "path": {"type": "string", "description": "Absolute path inside the storage, e.g. /switch. Default /."}
            }, "required": ["storage"]}
        },
        {
            "name": "library",
            "description": "List installed titles reported by the MTP responder, or local packages in the download folder.",
            "inputSchema": {"type": "object", "properties": {
                "source": {"type": "string", "enum": ["installed", "local"], "description": "Default installed."}
            }}
        },
        {
            "name": "install_file",
            "description": "Send a local NSP/NSZ/XCI/XCZ/NRO to the console using the configured or given transport.",
            "inputSchema": {"type": "object", "properties": {
                "path": {"type": "string", "description": "Local file path."},
                "via": {"type": "string", "enum": ["sd", "nand", "tinfoil", "network"], "description": "Override the configured target."}
            }, "required": ["path"]}
        },
        {
            "name": "list_storages",
            "description": "List every storage unit the MTP responder exposes with free/total space.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "pad",
            "description": "Drive the sys-con virtual pad. Press buttons, set sticks, or detach the pad.",
            "inputSchema": {"type": "object", "properties": {
                "buttons": {"type": "array", "items": {"type": "string"}, "description": "Buttons held in this packet, e.g. [\"a\"] or [\"l\",\"r\"]."},
                "left": {"type": "array", "items": {"type": "number"}, "description": "Left stick x,y in -1..1."},
                "right": {"type": "array", "items": {"type": "number"}, "description": "Right stick x,y in -1..1."},
                "hold_ms": {"type": "integer", "description": "Press, hold, release for this long. 0 leaves buttons held."},
                "release": {"type": "boolean", "description": "Detach the pad."}
            }}
        },
        {
            "name": "search_games",
            "description": "Search the configured game index for a title.",
            "inputSchema": {"type": "object", "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer", "description": "Default 10."}
            }, "required": ["query"]}
        },
        {
            "name": "homebrew",
            "description": "List the NRO homebrew catalog, optionally installing one.",
            "inputSchema": {"type": "object", "properties": {
                "refresh": {"type": "boolean", "description": "Re-query GitHub releases."},
                "install": {"type": "string", "description": "Catalog id to install, e.g. sys-con."}
            }}
        },
        {
            "name": "nsp_split",
            "description": "Split an NSP/XCI for FAT32 into .00/.01 parts.",
            "inputSchema": {"type": "object", "properties": {
                "path": {"type": "string"},
                "chunk_mb": {"type": "integer", "description": "Default 4095."}
            }, "required": ["path"]}
        },
        {
            "name": "nsp_merge",
            "description": "Merge a .00/.01 split set back into one file.",
            "inputSchema": {"type": "object", "properties": {
                "path": {"type": "string", "description": "The base path, e.g. /x/game.nsp."},
                "out": {"type": "string", "description": "Output path."}
            }, "required": ["path", "out"]}
        }
    ])
}

pub struct Mcp {
    pub config: Config,
}

impl Mcp {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Handle one JSON-RPC message. Notifications return `Value::Null`.
    pub fn handle(&self, request: &Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        if id.is_null() {
            return Value::Null;
        }
        match method {
            "initialize" => ok(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "godsend-switch", "version": crate::VERSION}
                }),
            ),
            "ping" => ok(id, json!({})),
            "tools/list" => ok(id, json!({"tools": tool_specs()})),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match self.call(name, &args) {
                    Ok(value) => ok(
                        id,
                        json!({
                            "content": [{"type": "text", "text": value.to_string()}],
                            "isError": false
                        }),
                    ),
                    Err(e) => ok(
                        id,
                        json!({
                            "content": [{"type": "text", "text": e.to_string()}],
                            "isError": true
                        }),
                    ),
                }
            }
            other => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("unknown method {other}")}
            }),
        }
    }

    fn call(&self, name: &str, args: &Value) -> Result<Value> {
        let ctx = JobContext::for_test();
        match name {
            "switch_status" => {
                let status = crate::device::probe(&self.config);
                Ok(serde_json::to_value(status)?)
            }
            "doctor" => Ok(serde_json::to_value(crate::doctor::run(&self.config))?),
            "list_storages" => {
                let session = MtpSession::open(&self.config.mtp_serial)?;
                Ok(serde_json::to_value(session.storages())?)
            }
            "list_storage" => {
                let kind = kind_from_str(args.get("storage").and_then(|v| v.as_str()).unwrap_or("sd"))?;
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                let session = MtpSession::open(&self.config.mtp_serial)?;
                Ok(serde_json::to_value(session.list(kind, path)?)?)
            }
            "library" => {
                let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("installed");
                if source == "local" {
                    let dir = crate::aria2::download_dir(&self.config);
                    return Ok(serde_json::to_value(crate::library::local(&dir, &self.config.covers_base))?);
                }
                let session = MtpSession::open(&self.config.mtp_serial)?;
                Ok(serde_json::to_value(crate::library::installed(&session, &self.config.covers_base)?)?)
            }
            "install_file" => {
                let path = PathBuf::from(
                    args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Invalid("path is required".into()))?,
                );
                let mut config = self.config.clone();
                if let Some(via) = args.get("via").and_then(|v| v.as_str()) {
                    config.install_target = target_from_str(via)?;
                }
                let report = crate::install::install(&config, &path, &ctx)?;
                Ok(serde_json::to_value(report)?)
            }
            "pad" => self.pad(args),
            "search_games" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or_default();
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
                let hits = crate::sources::gamesearch(&self.config.gamesearch_base, query, limit)?;
                Ok(serde_json::to_value(hits)?)
            }
            "homebrew" => {
                if let Some(id) = args.get("install").and_then(|v| v.as_str()) {
                    let path = crate::homebrew::install(&self.config, id, &ctx)?;
                    return Ok(json!({"installed": id, "path": path.to_string_lossy()}));
                }
                let refresh = args.get("refresh").and_then(|v| v.as_bool()).unwrap_or(false);
                Ok(serde_json::to_value(crate::homebrew::catalog(&self.config, refresh))?)
            }
            "nsp_split" => {
                let path = PathBuf::from(
                    args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Invalid("path is required".into()))?,
                );
                let chunk = args.get("chunk_mb").and_then(|v| v.as_u64()).unwrap_or(4095) * 1024 * 1024;
                let parts = crate::nsp::split(&path, chunk, &ctx)?;
                Ok(json!({"parts": parts.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>()}))
            }
            "nsp_merge" => {
                let base = PathBuf::from(
                    args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Invalid("path is required".into()))?,
                );
                let out = PathBuf::from(
                    args.get("out")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| Error::Invalid("out is required".into()))?,
                );
                let parts = crate::nsp::find_parts(&base);
                let bytes = crate::nsp::merge(&parts, &out, &ctx)?;
                Ok(json!({"out": out.to_string_lossy(), "bytes": bytes}))
            }
            other => Err(Error::Invalid(format!("unknown tool '{other}'"))),
        }
    }

    fn pad(&self, args: &Value) -> Result<Value> {
        use crate::syscon::{self, Button, PadState};
        let addr = self.config.syscon_addr.clone();
        let port = self.config.syscon_port;
        if args.get("release").and_then(|v| v.as_bool()).unwrap_or(false) {
            syscon::disconnect(&addr, port)?;
            return Ok(json!({"connected": false}));
        }
        let mut state = PadState::default();
        for name in args
            .get("buttons")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let name = name.as_str().ok_or_else(|| Error::Invalid("buttons must be strings".into()))?;
            state.buttons |= Button::parse(name)?.mask();
        }
        for (side, key) in [("l", "left"), ("r", "right")] {
            if let Some(values) = args.get(key).and_then(|v| v.as_array()) {
                let x = values.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = values.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                state = state.with_stick(side, x, y)?;
            }
        }
        let hold = args.get("hold_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        if hold > 0 {
            syscon::send(&addr, port, &state)?;
            std::thread::sleep(std::time::Duration::from_millis(hold.clamp(30, 5000)));
            syscon::send(&addr, port, &PadState::default())?;
            return Ok(json!({"connected": true, "released": true}));
        }
        syscon::send(&addr, port, &state)?;
        Ok(json!({"connected": true, "buttons": state.buttons}))
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Stdio MCP loop for the CLI.
pub fn run_stdio(config: Config) -> Result<()> {
    let mcp = Mcp::new(config);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let response = mcp.handle(&request);
        if !response.is_null() {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Host the MCP endpoint over HTTP on 127.0.0.1 until `stop` is set.
pub fn serve_http(config: Config, port: u16, stop: Arc<AtomicBool>) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| Error::Other(format!("MCP bind 127.0.0.1:{port}: {e}")))?;
    listener.set_nonblocking(true)?;
    let mcp = Arc::new(Mcp::new(config));
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let mcp = mcp.clone();
                std::thread::spawn(move || {
                    let _ = serve_connection(stream, &mcp);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(Error::Other(format!("MCP accept: {e}"))),
        }
    }
    Ok(())
}

fn serve_connection(stream: TcpStream, mcp: &Mcp) -> Result<()> {
    let mut stream = stream;
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or("/").to_string();
    if method == "GET" && path == "/health" {
        respond(&mut stream, 200, &json!({"ok": true, "server": "godsend-switch"}).to_string())?;
        return Ok(());
    }
    if method != "POST" {
        respond(&mut stream, 405, "{\"error\":\"POST /mcp\"}")?;
        return Ok(());
    }
    let mut length = 0usize;
    let mut host = String::new();
    let mut origin = String::new();
    let mut content_type = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("host") {
                host = value;
            } else if name.eq_ignore_ascii_case("origin") {
                origin = value;
            } else if name.eq_ignore_ascii_case("content-type") {
                content_type = value;
            }
        }
    }
    // This endpoint can press buttons and write files. Only let the app's own
    // loopback page drive it: reject DNS rebinding (foreign Host) and simple
    // cross-site POSTs (foreign Origin, non-JSON content type).
    if !loopback_host(&host) {
        respond(&mut stream, 403, "{\"error\":\"loopback host only\"}")?;
        return Ok(());
    }
    if !origin.is_empty() && !loopback_origin(&origin) {
        respond(&mut stream, 403, "{\"error\":\"cross-origin request refused\"}")?;
        return Ok(());
    }
    if !content_type.is_empty() && !content_type.to_ascii_lowercase().starts_with("application/json") {
        respond(&mut stream, 415, "{\"error\":\"application/json required\"}")?;
        return Ok(());
    }
    if length > 8 * 1024 * 1024 {
        respond(&mut stream, 413, "{\"error\":\"request too large\"}")?;
        return Ok(());
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let response = mcp.handle(&request);
    if response.is_null() {
        respond(&mut stream, 202, "")?;
    } else {
        respond(&mut stream, 200, &response.to_string())?;
    }
    Ok(())
}

fn loopback_host(host: &str) -> bool {
    let name = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    name == "127.0.0.1" || name == "localhost" || name == "[::1]"
}

fn loopback_origin(origin: &str) -> bool {
    let lower = origin.to_ascii_lowercase();
    lower.starts_with("http://127.0.0.1") || lower.starts_with("http://localhost") || lower.starts_with("https://127.0.0.1")
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        403 => "Forbidden",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_and_tool_list() {
        let mcp = Mcp::new(Config::default());
        let init = mcp.handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        let list = mcp.handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        let tools = list["result"]["tools"].as_array().unwrap();
        let names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"switch_status"));
        assert!(names.contains(&"install_file"));
        assert!(names.contains(&"pad"));
    }

    #[test]
    fn notifications_get_no_reply() {
        let mcp = Mcp::new(Config::default());
        let response = mcp.handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        assert!(response.is_null());
    }

    #[test]
    fn unknown_method_is_an_error() {
        let mcp = Mcp::new(Config::default());
        let response = mcp.handle(&json!({"jsonrpc":"2.0","id":7,"method":"nope"}));
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn tool_errors_are_content_not_crashes() {
        let mcp = Mcp::new(Config::default());
        let response = mcp.handle(&json!({
            "jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"list_storage","arguments":{"storage":"sd"}}
        }));
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"].as_str().unwrap().contains("MTP"));
    }

    #[test]
    fn only_loopback_hosts_and_origins_are_accepted() {
        assert!(loopback_host("127.0.0.1:3939"));
        assert!(loopback_host("localhost:3939"));
        assert!(!loopback_host("evil.example:3939"));
        assert!(!loopback_host(""));
        assert!(loopback_origin("http://127.0.0.1:3939"));
        assert!(loopback_origin("http://localhost"));
        assert!(!loopback_origin("https://evil.example"));
    }

    #[test]
    fn storage_names_map_to_kinds() {
        assert_eq!(kind_from_str("sd_install").unwrap(), Kind::SdInstall);
        assert_eq!(kind_from_str("nand").unwrap(), Kind::NandUser);
        assert!(kind_from_str("floppy").is_err());
        assert_eq!(target_from_str("tinfoil").unwrap(), InstallTarget::Tinfoil);
    }
}
