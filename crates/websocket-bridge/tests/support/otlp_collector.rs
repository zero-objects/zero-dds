// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// An OTLP/HTTP collector for the e2e tests of the daemons' spans (spec
// §8.3), shared by `#[path]` include (websocket-bridge, mqtt-bridge).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// An OTLP/HTTP collector on a port of its own: the spans of every
/// `POST /v1/traces` it receives, as the JSON objects of the request body.
pub fn otlp_collector() -> (String, Arc<Mutex<Vec<serde_json::Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let spans: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
    let sink = Arc::clone(&spans);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut acc = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                if let Some(i) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => acc.extend_from_slice(&buf[..n]),
                    _ => break None,
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&acc[..head_end]).to_ascii_lowercase();
            let length: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            while acc.len() < head_end + length {
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => acc.extend_from_slice(&buf[..n]),
                    _ => break,
                }
            }
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            if !head.starts_with("post /v1/traces ") {
                continue;
            }
            let body: serde_json::Value =
                serde_json::from_slice(&acc[head_end..]).expect("an OTLP/JSON body");
            let mut sink = sink.lock().unwrap();
            for resource in body["resourceSpans"].as_array().into_iter().flatten() {
                for scope in resource["scopeSpans"].as_array().into_iter().flatten() {
                    for span in scope["spans"].as_array().into_iter().flatten() {
                        sink.push(span.clone());
                    }
                }
            }
        }
    });
    (endpoint, spans)
}

/// The value of the attribute `key` of an OTLP/JSON span.
pub fn span_attribute<'a>(span: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    span["attributes"]
        .as_array()?
        .iter()
        .find(|a| a["key"] == key)?["value"]["stringValue"]
        .as_str()
}
