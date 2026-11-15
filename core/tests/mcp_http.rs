//! The app-hosted MCP endpoint, exercised over real HTTP.

use godsend_core::config::Config;
use godsend_core::mcp;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn post(port: u16, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out);
    out
}

#[test]
fn mcp_http_answers_initialize_and_tools() {
    let port = free_port();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let handle = std::thread::spawn(move || {
        let _ = mcp::serve_http(Config::default(), port, server_stop);
    });

    // Wait for the listener.
    let mut ready = false;
    for _ in 0..80 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(ready, "MCP server did not start");

    let init = post(
        port,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#,
    );
    assert!(init.starts_with("HTTP/1.1 200"), "{init}");
    assert!(init.contains("godsend-switch"), "{init}");
    assert!(init.contains("2024-11-05"), "{init}");

    let tools = post(port, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert!(tools.contains("install_file"), "{tools}");
    assert!(tools.contains("pad"), "{tools}");

    // A page on another origin (or a DNS-rebinding host) must be refused.
    let foreign = {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let body = r#"{"jsonrpc":"2.0","id":9,"method":"tools/list"}"#;
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: evil.example\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = stream.read_to_string(&mut out);
        out
    };
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");

    let health = {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut out = String::new();
        let _ = stream.read_to_string(&mut out);
        out
    };
    assert!(health.contains("\"ok\":true"), "{health}");

    stop.store(true, Ordering::Relaxed);
    let _ = TcpStream::connect(("127.0.0.1", port));
    let _ = handle.join();
}
