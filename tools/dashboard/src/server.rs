// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mini-HTTP-Server fuer das Dashboard. Pure-Rust, keine externen
//! Crates. Reicht fuer Localhost-Tools — kein production-grade
//! HTTP/1.1, kein chunked, kein keep-alive.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::state::{DashboardState, RecordingStatus};
use crate::web::INDEX_HTML;

/// Server-Fehler.
#[derive(Debug)]
pub enum ServerError {
    /// Bind/Accept/IO-Fehler.
    Io(std::io::Error),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Wie [`route`] aber mit Body fuer POST-Inject.
pub fn route_with_body(
    method: &str,
    path: &str,
    body: &str,
    state: &DashboardState,
) -> (u16, &'static str, String) {
    if method == "POST" {
        match path {
            "/api/inject/topics" => match state.inject_topics_json(body) {
                Ok(n) => (200, "application/json", format!(r#"{{"injected":{n}}}"#)),
                Err(e) => (400, "text/plain", format!("inject topics: {e}\n")),
            },
            "/api/inject/participants" => match state.inject_participants_json(body) {
                Ok(n) => (200, "application/json", format!(r#"{{"injected":{n}}}"#)),
                Err(e) => (400, "text/plain", format!("inject participants: {e}\n")),
            },
            "/api/inject/histograms" => match state.inject_histograms_json(body) {
                Ok(n) => (200, "application/json", format!(r#"{{"injected":{n}}}"#)),
                Err(e) => (400, "text/plain", format!("inject histograms: {e}\n")),
            },
            _ => route(method, path, state),
        }
    } else {
        route(method, path, state)
    }
}

/// Routet eine HTTP-Request-Line. Returnt `(status, content_type, body)`.
pub fn route(method: &str, path: &str, state: &DashboardState) -> (u16, &'static str, String) {
    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            (200, "text/html; charset=utf-8", INDEX_HTML.into())
        }
        ("GET", "/api/topics") => (200, "application/json", state.topics_json()),
        ("GET", "/api/participants") => (200, "application/json", state.participants_json()),
        ("GET", "/api/histograms") => (200, "application/json", state.histograms_json()),
        ("GET", "/api/graph") => (200, "application/json", state.graph_json()),
        ("GET", "/api/recording") => (200, "application/json", state.recording_json()),
        ("POST", "/api/recording/toggle") => {
            // Phase-A: toggle nur in-state. Phase-B: Hook zum
            // zerodds-recorder, der live in eine zddsrec-Datei schreibt.
            let cur_json = state.recording_json();
            let was_active = cur_json.contains(r#""active":true"#);
            state.set_recording(RecordingStatus {
                active: !was_active,
                output_path: if was_active {
                    None
                } else {
                    Some("/tmp/zerodds-live.zddsrec".into())
                },
                frames: 0,
            });
            (200, "application/json", state.recording_json())
        }
        _ => (404, "text/plain", "not found\n".into()),
    }
}

/// Formatiert eine HTTP-Response.
fn format_response(status: u16, content_type: &str, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{body}",
        len = body.len()
    )
}

/// Parsed die Request-Line aus dem Request-Header.
fn parse_request_line(buf: &str) -> Option<(String, String)> {
    let line = buf.lines().next()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    Some((parts[0].into(), parts[1].into()))
}

fn handle_connection(mut stream: TcpStream, state: Arc<DashboardState>) {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let mut buf = [0u8; 65_536];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let (method, path) = match parse_request_line(&req) {
        Some(p) => p,
        None => return,
    };
    // Body extrahieren (alles nach \r\n\r\n).
    let body = req.find("\r\n\r\n").map(|i| &req[i + 4..]).unwrap_or("");
    let (status, ctype, body) = route_with_body(&method, &path, body, &state);
    let resp = format_response(status, ctype, &body);
    let _ = stream.write_all(resp.as_bytes());
}

/// Blockierender Server-Loop. Akzeptiert Connections und beantwortet
/// jede in einem eigenen Thread (kein Async-Runtime, einfach).
///
/// # Errors
/// IO-Fehler beim TCP-Bind/Accept.
pub fn run_blocking(addr: SocketAddr, state: Arc<DashboardState>) -> Result<(), ServerError> {
    let listener = TcpListener::bind(addr).map_err(ServerError::Io)?;
    println!("zerodds-dashboard: listening on http://{addr}/");
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let st = Arc::clone(&state);
        std::thread::spawn(move || handle_connection(stream, st));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;
    use crate::state::TopicInfo;

    #[test]
    fn route_serves_index_html() {
        let s = DashboardState::new();
        let (code, ct, body) = route("GET", "/", &s);
        assert_eq!(code, 200);
        assert_eq!(ct, "text/html; charset=utf-8");
        assert!(body.contains("<title>ZeroDDS Dashboard</title>"));
    }

    #[test]
    fn route_serves_topics_json() {
        let s = DashboardState::new();
        s.set_topics(vec![TopicInfo {
            name: "/x".into(),
            type_name: "T".into(),
            publishers: 0,
            subscribers: 0,
            sample_rate_hz: 0.0,
        }]);
        let (code, ct, body) = route("GET", "/api/topics", &s);
        assert_eq!(code, 200);
        assert_eq!(ct, "application/json");
        assert!(body.contains(r#""name":"/x""#));
    }

    #[test]
    fn route_404_for_unknown() {
        let s = DashboardState::new();
        let (code, _, _) = route("GET", "/api/unknown", &s);
        assert_eq!(code, 404);
    }

    #[test]
    fn route_post_toggles_recording() {
        let s = DashboardState::new();
        let (code, _, body1) = route("POST", "/api/recording/toggle", &s);
        assert_eq!(code, 200);
        assert!(body1.contains(r#""active":true"#));
        let (_, _, body2) = route("POST", "/api/recording/toggle", &s);
        assert!(body2.contains(r#""active":false"#));
    }

    #[test]
    fn parse_request_line_smoke() {
        let r = "GET /api/topics HTTP/1.1\r\nHost: x\r\n\r\n";
        let (m, p) = parse_request_line(r).unwrap();
        assert_eq!(m, "GET");
        assert_eq!(p, "/api/topics");
    }

    #[test]
    fn format_response_includes_status_and_length() {
        let r = format_response(200, "application/json", "{}");
        assert!(r.starts_with("HTTP/1.1 200 OK"));
        assert!(r.contains("Content-Length: 2"));
        assert!(r.contains("Cache-Control: no-store"));
    }

    #[test]
    fn format_response_404() {
        let r = format_response(404, "text/plain", "no\n");
        assert!(r.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn route_inject_topics_accepts_json() {
        let s = DashboardState::new();
        let body = r#"[{"name":"/x","type_name":"T","publishers":1,"subscribers":2,"sample_rate_hz":50.0}]"#;
        let (code, _, resp) = route_with_body("POST", "/api/inject/topics", body, &s);
        assert_eq!(code, 200);
        assert!(resp.contains(r#""injected":1"#));
        // Datei laesst sich nun via GET wieder lesen.
        let (_, _, get_body) = route("GET", "/api/topics", &s);
        assert!(get_body.contains(r#""name":"/x""#));
    }

    #[test]
    fn route_inject_topics_rejects_invalid_json() {
        let s = DashboardState::new();
        let (code, _, _) = route_with_body("POST", "/api/inject/topics", "not json", &s);
        assert_eq!(code, 400);
    }
}
