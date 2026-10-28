// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A hand-rolled HTTP/1.1 engine — no framework dependency, the same
//! style as `crates/monitor/src/server.rs` and the accept loops of
//! `crates/coap-bridge`/`crates/websocket-bridge`, extended to what
//! those two don't need: request/response bodies beyond one `read()`
//! call, chunked `Transfer-Encoding` (RFC 7230 §4.1) on the request
//! side, keep-alive (RFC 7230 §6.3), and a `Content-Length` DoS cap.
//!
//! Scope: reads one request at a time off a blocking `Read + Write`
//! stream, and writes one response. A caller loops [`read_request`] /
//! [`write_response`] on the same connection for keep-alive; closes the
//! stream (or a request/response that named `Connection: close`) ends
//! it. Response bodies are always sent as `Content-Length` — every body
//! this daemon serves (XML, WSDL, a SOAP fault) is fully materialized
//! before it is sent, so there is no reason to chunk the output; only
//! an *inbound* request body may arrive chunked, from a client library
//! that does not know its body's length up front.

use std::io::{BufReader, Read, Write};

/// Default cap on a request's `Content-Length` / total chunked body
/// size. DDS-WEB samples are typically small; a body larger than this
/// is refused with `413 Payload Too Large` before it is read into
/// memory.
pub const DEFAULT_MAX_BODY: usize = 8 * 1024 * 1024;

/// Cap on the request line + header block, read before any body
/// handling — bounds a client that never sends a blank line.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// A parsed HTTP/1.x request.
#[derive(Debug, Clone)]
pub struct Request {
    /// `GET`, `POST`, `PUT`, `DELETE`, `HEAD`, or another verb (SOAP
    /// uses `POST` exclusively; the REST PSM's `RestMethod` maps from
    /// this).
    pub method: String,
    /// Request target, exactly as sent (path + optional `?query`), not
    /// yet percent-decoded.
    pub target: String,
    /// `HTTP/1.0` or `HTTP/1.1`.
    pub version: String,
    /// Headers in receipt order; multiple values of the same name are
    /// kept as separate entries (RFC 7230 §3.2.2 allows a single
    /// comma-joined value instead — both are accepted on read, values
    /// are always looked up case-insensitively).
    pub headers: Vec<(String, String)>,
    /// The body, fully read (decoded out of chunked framing if the
    /// request used it).
    pub body: Vec<u8>,
}

impl Request {
    /// The first header value matching `name`, case-insensitively.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// `true` unless the request (or an `HTTP/1.0` request without
    /// `Connection: keep-alive`) asked the connection to close after
    /// this response. RFC 7230 §6.3.
    #[must_use]
    pub fn keep_alive(&self) -> bool {
        let conn = self.header("connection").unwrap_or_default();
        if conn.eq_ignore_ascii_case("close") {
            return false;
        }
        if self.version == "HTTP/1.0" {
            return conn.eq_ignore_ascii_case("keep-alive");
        }
        true
    }

    /// The target's path component (before `?`), still percent-encoded.
    #[must_use]
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or(&self.target)
    }
}

/// A response to write.
#[derive(Debug, Clone)]
pub struct Response {
    /// e.g. `200`.
    pub status: u16,
    /// The reason phrase (`OK`, `Not Found`, ...).
    pub reason: &'static str,
    /// Extra headers beyond `Content-Length`/`Content-Type`/`Connection`,
    /// which [`write_response`] always sets itself.
    pub headers: Vec<(String, String)>,
    /// MIME type; `None` for an empty body (e.g. `204 No Content`).
    pub content_type: Option<String>,
    /// Body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// A response with a body and its content type.
    #[must_use]
    pub fn new(status: u16, reason: &'static str, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            reason,
            headers: Vec::new(),
            content_type: Some(content_type.to_string()),
            body,
        }
    }

    /// A response with no body (`204`/`304`/a `HEAD` answer).
    #[must_use]
    pub fn empty(status: u16, reason: &'static str) -> Self {
        Self {
            status,
            reason,
            headers: Vec::new(),
            content_type: None,
            body: Vec::new(),
        }
    }

    /// Adds a header.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// Read/parse error.
#[derive(Debug)]
pub enum HttpError {
    /// The peer closed the connection before a request arrived — not
    /// an error on a keep-alive loop's next iteration.
    ConnectionClosed,
    /// The request line or headers did not parse.
    Malformed(String),
    /// `Content-Length` or the chunked body exceeded the cap.
    BodyTooLarge,
    /// The header block exceeded [`MAX_HEADER_BYTES`] without a blank
    /// line.
    HeaderTooLarge,
    /// Underlying I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionClosed => f.write_str("connection closed"),
            Self::Malformed(m) => write!(f, "malformed request: {m}"),
            Self::BodyTooLarge => f.write_str("request body too large"),
            Self::HeaderTooLarge => f.write_str("request header block too large"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<std::io::Error> for HttpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads one HTTP/1.x request off `stream`, decoding a chunked body if
/// `Transfer-Encoding: chunked` is present, or reading exactly
/// `Content-Length` bytes otherwise. Neither may exceed `max_body`.
///
/// # Errors
/// See [`HttpError`]. [`HttpError::ConnectionClosed`] specifically means
/// zero bytes arrived before any request line — the expected way a
/// keep-alive loop's next read ends.
pub fn read_request<R: Read>(
    stream: &mut BufReader<R>,
    max_body: usize,
) -> Result<Request, HttpError> {
    let head = read_header_block(stream)?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpError::Malformed("empty request".into()))?;
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("missing method".into()))?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| HttpError::Malformed("missing target".into()))?
        .to_string();
    let version = parts.next().unwrap_or("HTTP/1.1").trim().to_string();
    if !matches!(version.as_str(), "HTTP/1.0" | "HTTP/1.1") {
        return Err(HttpError::Malformed(format!(
            "unsupported version {version}"
        )));
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| HttpError::Malformed(format!("malformed header: {line}")))?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }

    let req_no_body = Request {
        method,
        target,
        version,
        headers,
        body: Vec::new(),
    };

    let chunked = req_no_body
        .header("transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
    let body = if chunked {
        read_chunked_body(stream, max_body)?
    } else if let Some(len) = req_no_body.header("content-length") {
        let len: usize = len
            .trim()
            .parse()
            .map_err(|_| HttpError::Malformed(format!("bad content-length: {len}")))?;
        if len > max_body {
            return Err(HttpError::BodyTooLarge);
        }
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf)?;
        buf
    } else {
        Vec::new()
    };

    Ok(Request {
        body,
        ..req_no_body
    })
}

/// Reads bytes up to and including the blank line that ends the
/// request-line + header block, capped at [`MAX_HEADER_BYTES`].
fn read_header_block<R: Read>(stream: &mut BufReader<R>) -> Result<String, HttpError> {
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            if raw.is_empty() {
                return Err(HttpError::ConnectionClosed);
            }
            return Err(HttpError::Malformed("connection closed mid-header".into()));
        }
        raw.push(byte[0]);
        if raw.len() > MAX_HEADER_BYTES {
            return Err(HttpError::HeaderTooLarge);
        }
        if raw.ends_with(b"\r\n\r\n") {
            raw.truncate(raw.len() - 4);
            break;
        }
    }
    String::from_utf8(raw).map_err(|e| HttpError::Malformed(format!("header not UTF-8: {e}")))
}

/// RFC 7230 §4.1 chunked transfer coding.
fn read_chunked_body<R: Read>(
    stream: &mut BufReader<R>,
    max_body: usize,
) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::new();
    loop {
        let size_line = read_line(stream)?;
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|_| HttpError::Malformed(format!("bad chunk size: {size_str}")))?;
        if size == 0 {
            // Trailer headers (RFC 7230 §4.1.2), if any, up to the final
            // blank line; not surfaced to the caller.
            loop {
                let trailer = read_line(stream)?;
                if trailer.is_empty() {
                    break;
                }
            }
            break;
        }
        if body.len() + size > max_body {
            return Err(HttpError::BodyTooLarge);
        }
        let mut chunk = vec![0u8; size];
        stream.read_exact(&mut chunk)?;
        body.extend_from_slice(&chunk);
        // Trailing CRLF after each chunk's data.
        let mut crlf = [0u8; 2];
        stream.read_exact(&mut crlf)?;
        if &crlf != b"\r\n" {
            return Err(HttpError::Malformed("missing chunk CRLF".into()));
        }
    }
    Ok(body)
}

fn read_line<R: Read>(stream: &mut BufReader<R>) -> Result<String, HttpError> {
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            return Err(HttpError::Malformed("connection closed mid-chunk".into()));
        }
        if byte[0] == b'\n' {
            if raw.last() == Some(&b'\r') {
                raw.pop();
            }
            break;
        }
        raw.push(byte[0]);
        if raw.len() > MAX_HEADER_BYTES {
            return Err(HttpError::HeaderTooLarge);
        }
    }
    String::from_utf8(raw).map_err(|e| HttpError::Malformed(format!("chunk line not UTF-8: {e}")))
}

/// Writes `response` to `stream` as `Content-Length`-framed
/// HTTP/1.1, honoring `keep_alive` in the `Connection` header.
///
/// # Errors
/// I/O error writing to the stream.
pub fn write_response<W: Write>(
    stream: &mut W,
    response: &Response,
    keep_alive: bool,
) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", response.status, response.reason);
    if let Some(ct) = &response.content_type {
        out.push_str(&format!("Content-Type: {ct}\r\n"));
    }
    out.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    out.push_str(&format!(
        "Connection: {}\r\n",
        if keep_alive { "keep-alive" } else { "close" }
    ));
    for (name, value) in &response.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    stream.write_all(out.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(raw: &[u8]) -> Request {
        let mut r = BufReader::new(Cursor::new(raw.to_vec()));
        read_request(&mut r, DEFAULT_MAX_BODY).expect("parses")
    }

    #[test]
    fn parses_simple_get_with_no_body() {
        let req = parse(b"GET /dds/rest1/applications HTTP/1.1\r\nHost: x\r\n\r\n");
        assert_eq!(req.method, "GET");
        assert_eq!(req.target, "/dds/rest1/applications");
        assert!(req.body.is_empty());
        assert!(req.keep_alive());
    }

    #[test]
    fn parses_content_length_body() {
        let req = parse(b"POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello");
        assert_eq!(req.body, b"hello");
    }

    #[test]
    fn parses_chunked_body() {
        let raw = b"POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        let req = parse(raw);
        assert_eq!(req.body, b"Wikipedia");
    }

    #[test]
    fn chunked_body_with_trailer_headers() {
        let raw = b"POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\nX-Trailer: y\r\n\r\n";
        let req = parse(raw);
        assert_eq!(req.body, b"abc");
    }

    #[test]
    fn content_length_over_cap_is_refused() {
        let mut r = BufReader::new(Cursor::new(
            b"POST /x HTTP/1.1\r\nContent-Length: 100\r\n\r\n".to_vec(),
        ));
        let err = read_request(&mut r, 10).unwrap_err();
        assert!(matches!(err, HttpError::BodyTooLarge));
    }

    #[test]
    fn chunked_body_over_cap_is_refused() {
        let raw = b"POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n01234567\r\n8\r\n89abcdef\r\n0\r\n\r\n";
        let mut r = BufReader::new(Cursor::new(raw.to_vec()));
        let err = read_request(&mut r, 10).unwrap_err();
        assert!(matches!(err, HttpError::BodyTooLarge));
    }

    #[test]
    fn empty_stream_is_connection_closed_not_malformed() {
        let mut r = BufReader::new(Cursor::new(Vec::new()));
        let err = read_request(&mut r, DEFAULT_MAX_BODY).unwrap_err();
        assert!(matches!(err, HttpError::ConnectionClosed));
    }

    #[test]
    fn http_1_0_without_keep_alive_header_closes() {
        let req = parse(b"GET /x HTTP/1.0\r\n\r\n");
        assert!(!req.keep_alive());
    }

    #[test]
    fn http_1_0_with_keep_alive_header_stays_open() {
        let req = parse(b"GET /x HTTP/1.0\r\nConnection: keep-alive\r\n\r\n");
        assert!(req.keep_alive());
    }

    #[test]
    fn http_1_1_with_connection_close_closes() {
        let req = parse(b"GET /x HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(!req.keep_alive());
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let req = parse(b"GET /x HTTP/1.1\r\nContent-Type: text/plain\r\n\r\n");
        assert_eq!(req.header("content-type"), Some("text/plain"));
        assert_eq!(req.header("CONTENT-TYPE"), Some("text/plain"));
    }

    #[test]
    fn path_strips_query_string() {
        let req = parse(b"GET /a/b?x=1 HTTP/1.1\r\n\r\n");
        assert_eq!(req.path(), "/a/b");
    }

    #[test]
    fn write_response_round_trip() {
        let mut out = Vec::new();
        let resp = Response::new(200, "OK", "text/plain", b"hi".to_vec());
        write_response(&mut out, &resp, true).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.contains("Connection: keep-alive\r\n"));
        assert!(text.ends_with("\r\n\r\nhi"));
    }

    #[test]
    fn write_response_closes_when_not_keep_alive() {
        let mut out = Vec::new();
        let resp = Response::empty(204, "No Content");
        write_response(&mut out, &resp, false).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains("Content-Length: 0\r\n"));
    }
}
