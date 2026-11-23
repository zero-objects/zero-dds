// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The optional URL schemes of CORBA 3.3 Part 2 §7.6.10.7, Table 7.1:
//! `file://` ("a file containing a URL/IOR"), `ftp://` ("a file
//! containing a URL/IOR that is accessible via ftp protocol") and
//! `http://` ("an HTTP URL that returns an object URL/IOR").
//!
//! [`fetch`] returns the text the URL names, without the white space
//! around it; `Orb::string_to_object` converts that text in turn.
//!
//! - `file://[localhost]/<path>` (RFC 8089): a file of this host.
//! - `http://<host>[:<port>]/<path>` (RFC 9110, RFC 9112): an HTTP/1.1
//!   `GET`; the body of a `200` answer, with `Content-Length`, chunked
//!   transfer coding or up to the end of the connection. Path and query
//!   take only the characters of RFC 3986 §3.3 and §3.4 and `%XX`. A
//!   redirect (`301`, `302`, `303`, `307`, `308`) is answered with the
//!   `Location` reference resolved against the request URL (RFC 3986
//!   §5.2), which `string_to_object` converts like any other URL; only
//!   a redirect to another `http` URL is followed.
//! - `ftp://[<user>[:<password>]@]<host>[:<port>]/<path>[;type=<a|i>]`
//!   (RFC 1738 §3.2, RFC 959, RFC 2428): login (`anonymous` without a
//!   user), `TYPE I` (`TYPE A` for `;type=a`), `CWD` for each directory
//!   of the path, passive mode (`EPSV`, then `PASV`), `RETR` of the last
//!   segment. `;type=d` asks for a directory listing, not a file holding
//!   a URL or IOR, and is refused like any other type code.
//!
//! Host names in brackets are IPv6 addresses. The path is `%XX`
//! unescaped. Every network step has a time limit of
//! [`FETCH_TIMEOUT`].

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// The time limit of each connect, read and write of a fetch.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest text a fetch reads: a stringified IOR of many profiles fits
/// many times.
pub const MAX_FETCH_BYTES: usize = 1 << 20;

/// Why a fetch failed, as the `BAD_PARAM` minor code of Part 2 §7.6.10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The scheme is not `file`, `http` or `ftp` (minor 7).
    Scheme(String),
    /// The host, port, user or a `file` URL on another host (minor 8).
    Address(String),
    /// The path does not unescape or has a character a URL does not
    /// carry, an FTP type code other than `a` or `i`, the text is not
    /// UTF-8 or is empty (minor 9).
    Content(String),
    /// The file or server could not be read, or answered with an error
    /// (minor 10).
    Failed(String),
}

impl FetchError {
    /// The `BAD_PARAM` minor code of Part 2 §7.6.10.
    #[must_use]
    pub fn minor(&self) -> u32 {
        match self {
            Self::Scheme(_) => 7,
            Self::Address(_) => 8,
            Self::Content(_) => 9,
            Self::Failed(_) => 10,
        }
    }
}

/// A parsed `file`, `http` or `ftp` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchUrl {
    /// `file://`: the path of a file of this host.
    File {
        /// The unescaped absolute path.
        path: String,
    },
    /// `http://`.
    Http {
        /// The host as written in the URL (an IPv6 address in brackets).
        host: String,
        /// The port, 80 when the URL gives none.
        port: u16,
        /// The path and query as written, `/` when the URL gives none.
        target: String,
    },
    /// `ftp://`.
    Ftp {
        /// The user, `anonymous` when the URL gives none.
        user: String,
        /// The password, `anonymous@` when the URL gives none.
        password: String,
        /// The host as written (an IPv6 address in brackets).
        host: String,
        /// The port, 21 when the URL gives none.
        port: u16,
        /// The unescaped path segments; the last one names the file.
        segments: Vec<String>,
        /// `;type=a`: the file is read with `TYPE A`; otherwise (no type
        /// code or `;type=i`) with `TYPE I`.
        ascii: bool,
    },
}

impl FetchUrl {
    /// Parses a `file`, `http` or `ftp` URL. Scheme names are matched
    /// without regard to case.
    ///
    /// # Errors
    /// [`FetchError`] as in its variants.
    pub fn parse(url: &str) -> Result<Self, FetchError> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| FetchError::Scheme(url.to_string()))?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        if scheme.eq_ignore_ascii_case("file") {
            if !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost")) {
                return Err(FetchError::Address(alloc::format!(
                    "{url}: a file of another host"
                )));
            }
            if path.is_empty() {
                return Err(FetchError::Content(alloc::format!("{url}: no path")));
            }
            return Ok(Self::File {
                path: unescape_utf8(path)?,
            });
        }
        if scheme.eq_ignore_ascii_case("http") {
            let (userinfo, host, port) = split_authority(authority, 80)?;
            if userinfo.is_some() {
                return Err(FetchError::Address(alloc::format!(
                    "{url}: user information in an http URL"
                )));
            }
            // RFC 3986 §3.3 to §3.5: what goes into the request line is
            // checked, so that no space, line break or other octet
            // outside the URI syntax reaches it.
            let (target, fragment) = path.split_once('#').unwrap_or((path, ""));
            if !is_uri_text(target) || !is_uri_text(fragment) {
                return Err(FetchError::Content(alloc::format!(
                    "{}: a character outside RFC 3986 in the path or query",
                    url.escape_debug()
                )));
            }
            let target = match target {
                "" => "/".to_string(),
                t => t.to_string(),
            };
            return Ok(Self::Http { host, port, target });
        }
        if scheme.eq_ignore_ascii_case("ftp") {
            let (userinfo, host, port) = split_authority(authority, 21)?;
            let (user, password) = match userinfo {
                None => ("anonymous".to_string(), "anonymous@".to_string()),
                Some(u) => match u.split_once(':') {
                    Some((n, p)) => (unescape_utf8(n)?, unescape_utf8(p)?),
                    None => (unescape_utf8(u)?, String::new()),
                },
            };
            // RFC 1738 §3.2.2: `;type=<typecode>` with `a` (ASCII), `i`
            // (image) or `d` (a directory listing, §3.2.3). A listing is
            // not a file holding a URL or IOR.
            let (path, ascii) = match path.split_once(";type=") {
                None => (path, false),
                Some((p, "i" | "I")) => (p, false),
                Some((p, "a" | "A")) => (p, true),
                Some((_, "d" | "D")) => {
                    return Err(FetchError::Content(alloc::format!(
                        "{url}: `;type=d` names a directory listing, not a file"
                    )));
                }
                Some((_, code)) => {
                    return Err(FetchError::Content(alloc::format!(
                        "{url}: FTP type code `{code}`"
                    )));
                }
            };
            let segments = path
                .trim_start_matches('/')
                .split('/')
                .map(unescape_utf8)
                .collect::<Result<Vec<_>, _>>()?;
            if segments.last().is_none_or(String::is_empty) {
                return Err(FetchError::Content(alloc::format!("{url}: no file name")));
            }
            if user.contains(['\r', '\n'])
                || password.contains(['\r', '\n'])
                || segments.iter().any(|s| s.contains(['\r', '\n']))
            {
                return Err(FetchError::Content(alloc::format!(
                    "{url}: a line break in a command argument"
                )));
            }
            return Ok(Self::Ftp {
                user,
                password,
                host,
                port,
                segments,
                ascii,
            });
        }
        Err(FetchError::Scheme(url.to_string()))
    }
}

/// The text `url` names, trimmed of white space (§7.6.10.7).
///
/// # Errors
/// [`FetchError`]: the minor code of the `BAD_PARAM` `string_to_object`
/// raises.
pub fn fetch(url: &str) -> Result<String, FetchError> {
    let bytes = match FetchUrl::parse(url)? {
        FetchUrl::File { path } => read_file(&path)?,
        FetchUrl::Http { host, port, target } => return http_get(&host, port, &target),
        FetchUrl::Ftp {
            user,
            password,
            host,
            port,
            segments,
            ascii,
        } => ftp_retr(&host, port, &user, &password, &segments, ascii)?,
    };
    into_text(url, &bytes)
}

fn into_text(url: &str, bytes: &[u8]) -> Result<String, FetchError> {
    let text = core::str::from_utf8(bytes)
        .map_err(|_| FetchError::Content(alloc::format!("{url}: the text is not UTF-8")))?
        .trim();
    if text.is_empty() {
        return Err(FetchError::Content(alloc::format!("{url}: empty")));
    }
    Ok(text.to_string())
}

fn read_file(path: &str) -> Result<Vec<u8>, FetchError> {
    let file =
        std::fs::File::open(path).map_err(|e| FetchError::Failed(alloc::format!("{path}: {e}")))?;
    read_bounded(file, path)
}

fn read_bounded(r: impl Read, what: &str) -> Result<Vec<u8>, FetchError> {
    let mut out = Vec::new();
    r.take(MAX_FETCH_BYTES as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| FetchError::Failed(alloc::format!("{what}: {e}")))?;
    if out.len() > MAX_FETCH_BYTES {
        return Err(FetchError::Failed(alloc::format!(
            "{what}: longer than {MAX_FETCH_BYTES} bytes"
        )));
    }
    Ok(out)
}

/// `[userinfo@]host[:port]`; an IPv6 host in brackets.
fn split_authority(
    authority: &str,
    default_port: u16,
) -> Result<(Option<&str>, String, u16), FetchError> {
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let bad = |what: &str| FetchError::Address(alloc::format!("{what}: `{authority}`"));
    let (host, port) = if let Some(rest) = hostport.strip_prefix('[') {
        let (v6, after) = rest.split_once(']').ok_or_else(|| bad("no `]`"))?;
        v6.parse::<core::net::Ipv6Addr>()
            .map_err(|_| bad("not an IPv6 address"))?;
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':').ok_or_else(|| bad("after `]`"))?),
        };
        (alloc::format!("[{v6}]"), port)
    } else {
        let (h, p) = match hostport.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (hostport, None),
        };
        if h.is_empty()
            || !h
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        {
            return Err(bad("host"));
        }
        (h.to_string(), p)
    };
    let port = match port {
        None => default_port,
        Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => p
            .parse::<u16>()
            .ok()
            .filter(|&n| n != 0)
            .ok_or_else(|| bad("port"))?,
        Some(_) => return Err(bad("port")),
    };
    Ok((userinfo, host, port))
}

/// `true` when `s` is made of the characters RFC 3986 allows in a path,
/// a query and a fragment (`pchar`, `/`, `?`; §3.3 to §3.5) and `%` only
/// before two hex digits.
fn is_uri_text(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%' {
            if !(b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
            {
                return false;
            }
            i += 3;
            continue;
        }
        if !(c.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?".contains(&c)) {
            return false;
        }
        i += 1;
    }
    true
}

fn unescape_utf8(s: &str) -> Result<String, FetchError> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let v = s
                .get(i + 1..i + 3)
                .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| FetchError::Content(alloc::format!("escape in `{s}`")))?;
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| FetchError::Content(alloc::format!("`{s}` is not UTF-8")))
}

/// A connection to `host` (an IPv6 address in brackets) and `port`, each
/// resolved address tried in turn.
fn connect(host: &str, port: u16) -> Result<TcpStream, FetchError> {
    let name = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = (name, port)
        .to_socket_addrs()
        .map_err(|e| FetchError::Address(alloc::format!("{host}: {e}")))?
        .collect();
    let mut last = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, FETCH_TIMEOUT) {
            Ok(s) => {
                s.set_read_timeout(Some(FETCH_TIMEOUT))
                    .and_then(|()| s.set_write_timeout(Some(FETCH_TIMEOUT)))
                    .map_err(|e| FetchError::Failed(alloc::format!("{host}:{port}: {e}")))?;
                return Ok(s);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(FetchError::Failed(alloc::format!(
        "{host}:{port}: {}",
        last.map_or_else(|| "no address".to_string(), |e| e.to_string())
    )))
}

// ---- HTTP ------------------------------------------------------------------

/// An HTTP/1.1 `GET` of `target` (RFC 9112): the body of a `200`, or the
/// `Location` of a redirect as the text to convert next.
fn http_get(host: &str, port: u16, target: &str) -> Result<String, FetchError> {
    let what = alloc::format!("http://{host}:{port}{target}");
    let mut stream = connect(host, port)?;
    let host_header = if port == 80 {
        host.to_string()
    } else {
        alloc::format!("{host}:{port}")
    };
    let request = alloc::format!(
        "GET {target} HTTP/1.1\r\nHost: {host_header}\r\nAccept: */*\r\n\
         User-Agent: zerodds-corba-orb\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| FetchError::Failed(alloc::format!("{what}: {e}")))?;
    let mut reader = BufReader::new(stream);
    let status_line = read_line(&mut reader, &what)?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    let status = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") || status.len() != 3 {
        return Err(FetchError::Failed(alloc::format!(
            "{what}: not an HTTP/1 answer: `{status_line}`"
        )));
    }
    let mut content_length = None;
    let mut chunked = false;
    let mut location = None;
    loop {
        let line = read_line(&mut reader, &what)?;
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(FetchError::Failed(alloc::format!(
                "{what}: header line `{line}`"
            )));
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            // RFC 9110 §8.6: `Content-Length = 1*DIGIT`.
            content_length = Some(
                (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| value.parse::<usize>().ok())
                    .flatten()
                    .ok_or_else(|| {
                        FetchError::Failed(alloc::format!("{what}: Content-Length `{value}`"))
                    })?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value
                .rsplit(',')
                .next()
                .is_some_and(|c| c.trim().eq_ignore_ascii_case("chunked"));
            if !chunked {
                return Err(FetchError::Failed(alloc::format!(
                    "{what}: transfer coding `{value}`"
                )));
            }
        } else if name.eq_ignore_ascii_case("location") {
            location = Some(value.to_string());
        }
    }
    match status {
        "200" => {}
        "301" | "302" | "303" | "307" | "308" => {
            let to = location.ok_or_else(|| {
                FetchError::Failed(alloc::format!("{what}: redirect without Location"))
            })?;
            return resolve_redirect(&host_header, target, &to)
                .map_err(|why| FetchError::Failed(alloc::format!("{what}: {why}")));
        }
        other => {
            return Err(FetchError::Failed(alloc::format!("{what}: status {other}")));
        }
    }
    let body = if chunked {
        read_chunked(&mut reader, &what)?
    } else if let Some(n) = content_length {
        if n > MAX_FETCH_BYTES {
            return Err(FetchError::Failed(alloc::format!(
                "{what}: longer than {MAX_FETCH_BYTES} bytes"
            )));
        }
        let mut body = alloc::vec![0; n];
        reader
            .read_exact(&mut body)
            .map_err(|e| FetchError::Failed(alloc::format!("{what}: {e}")))?;
        body
    } else {
        read_bounded(reader, &what)?
    };
    into_text(&what, &body)
}

/// The parts of a URI reference (RFC 3986 §4.1) without its fragment.
struct UriReference<'a> {
    scheme: Option<&'a str>,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
}

impl<'a> UriReference<'a> {
    /// Splits `s` as the regular expression of RFC 3986 Appendix B does.
    fn split(s: &'a str) -> Self {
        let s = s.split_once('#').map_or(s, |(before, _)| before);
        let (scheme, rest) = match s.split_once(':') {
            Some((scheme, rest)) if is_scheme(scheme) => (Some(scheme), rest),
            _ => (None, s),
        };
        let (authority, rest) = match rest.strip_prefix("//") {
            Some(r) => {
                let end = r.find(['/', '?']).unwrap_or(r.len());
                (Some(&r[..end]), &r[end..])
            }
            None => (None, rest),
        };
        let (path, query) = match rest.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (rest, None),
        };
        Self {
            scheme,
            authority,
            path,
            query,
        }
    }
}

/// `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )` (RFC 3986 §3.1).
fn is_scheme(s: &str) -> bool {
    let mut b = s.bytes();
    b.next().is_some_and(|c| c.is_ascii_alphabetic())
        && b.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
}

/// The URL a redirect leads to: the `Location` reference (RFC 9110
/// §10.2.2, a URI reference) resolved against the request URL
/// `http://<authority><target>` after RFC 3986 §5.2.2, without a
/// fragment. Only an `http` URL with a host is followed: a `Location`
/// of another scheme would make a server name a local file or another
/// protocol.
///
/// # Errors
/// Why the redirect is not followed.
fn resolve_redirect(authority: &str, target: &str, location: &str) -> Result<String, String> {
    let r = UriReference::split(location);
    let (base_path, base_query) = match target.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (target, None),
    };
    if r.scheme
        .is_some_and(|scheme| !scheme.eq_ignore_ascii_case("http"))
    {
        return Err(alloc::format!(
            "a redirect to `{location}`, which is no http URL"
        ));
    }
    let (authority, path, query) = if let Some(a) = r.authority {
        (a, remove_dot_segments(r.path), r.query)
    } else if r.scheme.is_some() {
        return Err(alloc::format!("a redirect to `{location}` without a host"));
    } else if r.path.is_empty() {
        (authority, base_path.to_string(), r.query.or(base_query))
    } else if r.path.starts_with('/') {
        (authority, remove_dot_segments(r.path), r.query)
    } else {
        // §5.2.3: the base path up to its last `/`, then the reference.
        let dir = base_path.rfind('/').map_or("/", |i| &base_path[..=i]);
        (
            authority,
            remove_dot_segments(&alloc::format!("{dir}{}", r.path)),
            r.query,
        )
    };
    if authority.is_empty() {
        return Err(alloc::format!("a redirect to `{location}` without a host"));
    }
    let mut url = alloc::format!("http://{authority}{path}");
    if let Some(q) = query {
        url.push('?');
        url.push_str(q);
    }
    Ok(url)
}

/// `remove_dot_segments` of RFC 3986 §5.2.4.
fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut output = String::with_capacity(path.len());
    let pop = |output: &mut String| match output.rfind('/') {
        Some(i) => output.truncate(i),
        None => output.clear(),
    };
    while !input.is_empty() {
        if input.starts_with("../") {
            input.replace_range(..3, "");
        } else if input.starts_with("./") {
            input.replace_range(..2, "");
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/");
        } else if input == "/." {
            input = "/".to_string();
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/");
            pop(&mut output);
        } else if input == "/.." {
            input = "/".to_string();
            pop(&mut output);
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            let from = usize::from(input.starts_with('/'));
            let end = input[from..].find('/').map_or(input.len(), |i| i + from);
            output.push_str(&input[..end]);
            input.replace_range(..end, "");
        }
    }
    output
}

/// One line without its CRLF (or LF).
fn read_line(r: &mut impl BufRead, what: &str) -> Result<String, FetchError> {
    let mut line = Vec::new();
    r.take(8192)
        .read_until(b'\n', &mut line)
        .map_err(|e| FetchError::Failed(alloc::format!("{what}: {e}")))?;
    if line.last() != Some(&b'\n') {
        return Err(FetchError::Failed(alloc::format!(
            "{what}: the connection ended inside a line"
        )));
    }
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line)
        .map_err(|_| FetchError::Failed(alloc::format!("{what}: a line is not UTF-8")))
}

/// The chunked transfer coding (RFC 9112 §7.1). A chunk size is hex
/// digits (`chunk-size = 1*HEXDIG`); the body stays within
/// [`MAX_FETCH_BYTES`] whatever size the server announces.
fn read_chunked(r: &mut impl BufRead, what: &str) -> Result<Vec<u8>, FetchError> {
    let mut body = Vec::new();
    loop {
        let line = read_line(r, what)?;
        let size = line.split(';').next().unwrap_or_default().trim();
        let n = (!size.is_empty() && size.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| usize::from_str_radix(size, 16).ok())
            .flatten()
            .ok_or_else(|| FetchError::Failed(alloc::format!("{what}: chunk size `{size}`")))?;
        if n == 0 {
            // Trailer fields up to the empty line.
            while !read_line(r, what)?.is_empty() {}
            return Ok(body);
        }
        if n > MAX_FETCH_BYTES - body.len() {
            return Err(FetchError::Failed(alloc::format!(
                "{what}: longer than {MAX_FETCH_BYTES} bytes"
            )));
        }
        let start = body.len();
        body.resize(start + n, 0);
        r.read_exact(&mut body[start..])
            .map_err(|e| FetchError::Failed(alloc::format!("{what}: {e}")))?;
        if !read_line(r, what)?.is_empty() {
            return Err(FetchError::Failed(alloc::format!(
                "{what}: no line end after a chunk"
            )));
        }
    }
}

// ---- FTP -------------------------------------------------------------------

/// The control connection of an FTP session (RFC 959).
struct Ftp {
    reader: BufReader<TcpStream>,
    what: String,
}

impl Ftp {
    /// One reply: its code and its last line; a multi-line reply
    /// (`ddd-` ... `ddd `) is read to its end (RFC 959 §4.2).
    fn reply(&mut self) -> Result<(u16, String), FetchError> {
        let first = read_line(&mut self.reader, &self.what)?;
        let code = first
            .get(..3)
            .and_then(|c| c.parse::<u16>().ok())
            .ok_or_else(|| FetchError::Failed(alloc::format!("{}: reply `{first}`", self.what)))?;
        if first.as_bytes().get(3) == Some(&b'-') {
            let end = alloc::format!("{code} ");
            loop {
                let line = read_line(&mut self.reader, &self.what)?;
                if line.starts_with(&end) {
                    return Ok((code, line));
                }
            }
        }
        Ok((code, first))
    }

    /// Sends `command` and reads its reply.
    fn command(&mut self, command: &str) -> Result<(u16, String), FetchError> {
        let line = alloc::format!("{command}\r\n");
        self.reader
            .get_mut()
            .write_all(line.as_bytes())
            .map_err(|e| FetchError::Failed(alloc::format!("{}: {e}", self.what)))?;
        self.reply()
    }

    /// `command` must be answered with a code in `ok`.
    fn expect(&mut self, command: &str, ok: &[u16]) -> Result<(u16, String), FetchError> {
        let (code, text) = self.command(command)?;
        if ok.contains(&code) {
            Ok((code, text))
        } else {
            let shown = command.split(' ').next().unwrap_or(command);
            Err(FetchError::Failed(alloc::format!(
                "{}: {shown}: {text}",
                self.what
            )))
        }
    }

    /// The data connection of passive mode: `EPSV` (RFC 2428), then
    /// `PASV`; both connect to the address of the control connection.
    fn data_connection(&mut self) -> Result<TcpStream, FetchError> {
        let peer = self
            .reader
            .get_ref()
            .peer_addr()
            .map_err(|e| FetchError::Failed(alloc::format!("{}: {e}", self.what)))?;
        let (code, text) = self.command("EPSV")?;
        let port = if code == 229 {
            // `229 Entering Extended Passive Mode (|||port|)`.
            text.split('(')
                .nth(1)
                .and_then(|t| t.split(')').next())
                .and_then(|t| {
                    let d = t.chars().next()?;
                    t.split(d).nth(3)?.parse::<u16>().ok()
                })
                .ok_or_else(|| FetchError::Failed(alloc::format!("{}: {text}", self.what)))?
        } else {
            let (_, text) = self.expect("PASV", &[227])?;
            // `227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)`.
            let numbers: Vec<u16> = text
                .split('(')
                .nth(1)
                .and_then(|t| t.split(')').next())
                .map(|t| t.split(',').filter_map(|n| n.trim().parse().ok()).collect())
                .unwrap_or_default();
            if numbers.len() != 6 || numbers[4] > 255 || numbers[5] > 255 {
                return Err(FetchError::Failed(alloc::format!("{}: {text}", self.what)));
            }
            numbers[4] * 256 + numbers[5]
        };
        let addr = SocketAddr::new(peer.ip(), port);
        let s = TcpStream::connect_timeout(&addr, FETCH_TIMEOUT)
            .map_err(|e| FetchError::Failed(alloc::format!("{}: data {addr}: {e}", self.what)))?;
        s.set_read_timeout(Some(FETCH_TIMEOUT))
            .map_err(|e| FetchError::Failed(alloc::format!("{}: {e}", self.what)))?;
        Ok(s)
    }
}

/// `RETR` of the last of `segments` after a `CWD` for each one before it
/// (RFC 1738 §3.2.2), with `TYPE A` when `ascii`, else `TYPE I`.
fn ftp_retr(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    segments: &[String],
    ascii: bool,
) -> Result<Vec<u8>, FetchError> {
    let what = alloc::format!("ftp://{host}:{port}/{}", segments.join("/"));
    let mut ftp = Ftp {
        reader: BufReader::new(connect(host, port)?),
        what,
    };
    let (code, text) = ftp.reply()?;
    if code != 220 {
        return Err(FetchError::Failed(alloc::format!("{}: {text}", ftp.what)));
    }
    let (code, _) = ftp.expect(&alloc::format!("USER {user}"), &[230, 331])?;
    if code == 331 {
        ftp.expect(&alloc::format!("PASS {password}"), &[230, 202])?;
    }
    ftp.expect(if ascii { "TYPE A" } else { "TYPE I" }, &[200])?;
    let (file, dirs) = segments
        .split_last()
        .ok_or_else(|| FetchError::Content(alloc::format!("{}: no file name", ftp.what)))?;
    for dir in dirs {
        ftp.expect(&alloc::format!("CWD {dir}"), &[250])?;
    }
    let data = ftp.data_connection()?;
    ftp.expect(&alloc::format!("RETR {file}"), &[125, 150])?;
    let body = read_bounded(data, &ftp.what)?;
    ftp.reply().and_then(|(code, text)| {
        if code == 226 || code == 250 {
            Ok(())
        } else {
            Err(FetchError::Failed(alloc::format!("{}: {text}", ftp.what)))
        }
    })?;
    // The answer to QUIT does not change the outcome.
    let _ = ftp.command("QUIT");
    Ok(body)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn corba_p2_7_6_10_7_the_three_url_forms_parse() {
        assert_eq!(
            FetchUrl::parse("file:///tmp/a%20b.ior").unwrap(),
            FetchUrl::File {
                path: "/tmp/a b.ior".into()
            }
        );
        assert_eq!(
            FetchUrl::parse("FILE://localhost/x").unwrap(),
            FetchUrl::File { path: "/x".into() }
        );
        assert_eq!(
            FetchUrl::parse("http://[::1]:8080/ns.ior?x=1#f").unwrap(),
            FetchUrl::Http {
                host: "[::1]".into(),
                port: 8080,
                target: "/ns.ior?x=1".into()
            }
        );
        assert_eq!(
            FetchUrl::parse("http://h.example").unwrap(),
            FetchUrl::Http {
                host: "h.example".into(),
                port: 80,
                target: "/".into()
            }
        );
        assert_eq!(
            FetchUrl::parse("ftp://u:p%40ss@h:2121/pub/refs/ns.ior;type=i").unwrap(),
            FetchUrl::Ftp {
                user: "u".into(),
                password: "p@ss".into(),
                host: "h".into(),
                port: 2121,
                segments: alloc::vec!["pub".into(), "refs".into(), "ns.ior".into()],
                ascii: false,
            }
        );
        assert_eq!(
            FetchUrl::parse("ftp://h/ns.ior;type=A").unwrap(),
            FetchUrl::Ftp {
                user: "anonymous".into(),
                password: "anonymous@".into(),
                host: "h".into(),
                port: 21,
                segments: alloc::vec!["ns.ior".into()],
                ascii: true,
            }
        );
        assert_eq!(
            FetchUrl::parse("ftp://h/ns.ior").unwrap(),
            FetchUrl::Ftp {
                user: "anonymous".into(),
                password: "anonymous@".into(),
                host: "h".into(),
                port: 21,
                segments: alloc::vec!["ns.ior".into()],
                ascii: false,
            }
        );
    }

    #[test]
    fn corba_p2_7_6_10_7_bad_urls_carry_their_minor_code() {
        let minor = |u: &str| FetchUrl::parse(u).unwrap_err().minor();
        assert_eq!(minor("gopher://h/x"), 7);
        assert_eq!(minor("file:/x"), 7);
        assert_eq!(minor("file://other.example/x"), 8);
        assert_eq!(minor("http://h:0/x"), 8);
        assert_eq!(minor("http://h:x/x"), 8);
        assert_eq!(minor("http://u@h/x"), 8);
        assert_eq!(minor("http://[::1/x"), 8);
        assert_eq!(minor("file://"), 9);
        assert_eq!(minor("file:///x%2"), 9);
        assert_eq!(minor("ftp://h/"), 9);
        assert_eq!(minor("ftp://h/a%0d%0aDELE%20b"), 9);
        // RFC 1738 §3.2.2: a directory listing and unknown type codes.
        assert_eq!(minor("ftp://h/refs/ns.ior;type=d"), 9);
        assert_eq!(minor("ftp://h/refs/;type=D"), 9);
        assert_eq!(minor("ftp://h/refs/ns.ior;type=x"), 9);
        assert_eq!(minor("ftp://h/refs/ns.ior;type="), 9);
        assert_eq!(minor("ftp://h/refs/ns.ior;type=ia"), 9);
        // RFC 3986: no space, line break, other control octet or bad
        // escape in the request target of an http URL.
        for bad in [
            "http://h/a\r\nX-Injected: 1",
            "http://h/a b",
            "http://h/a\tb",
            "http://h/a?q=\n",
            "http://h/a%zz",
            "http://h/a%4",
            "http://h/a\"b",
            "http://h/a<b>",
            "http://h/a#f g",
            "http://h/\u{e4}",
        ] {
            assert_eq!(minor(bad), 9, "{bad:?}");
        }
        assert!(FetchUrl::parse("http://h/a%20b?x=1&y=a:b@c/d;e#f/g?").is_ok());
    }

    /// RFC 9110 §10.2.2: `Location` is a URI reference, resolved against
    /// the request URL after RFC 3986 §5.2. The normal and abnormal
    /// examples of RFC 3986 §5.4 with the base `http://a/b/c/d;p?q`
    /// (without their fragments, which a fetch drops).
    #[test]
    fn corba_p2_7_6_10_7_a_redirect_is_resolved_against_the_request_url() {
        let resolve = |r: &str| resolve_redirect("a", "/b/c/d;p?q", r).unwrap();
        for (reference, expected) in [
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("#s", "http://a/b/c/d;p?q"),
            ("g#s", "http://a/b/c/g"),
            ("g?y#s", "http://a/b/c/g?y"),
            (";x", "http://a/b/c/;x"),
            ("g;x", "http://a/b/c/g;x"),
            ("g;x?y#s", "http://a/b/c/g;x?y"),
            ("", "http://a/b/c/d;p?q"),
            (".", "http://a/b/c/"),
            ("./", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../", "http://a/"),
            ("../../g", "http://a/g"),
            ("../../../g", "http://a/g"),
            ("../../../../g", "http://a/g"),
            ("/./g", "http://a/g"),
            ("/../g", "http://a/g"),
            ("g.", "http://a/b/c/g."),
            (".g", "http://a/b/c/.g"),
            ("g..", "http://a/b/c/g.."),
            ("..g", "http://a/b/c/..g"),
            ("./../g", "http://a/b/g"),
            ("./g/.", "http://a/b/c/g/"),
            ("g/./h", "http://a/b/c/g/h"),
            ("g/../h", "http://a/b/c/h"),
            ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
            ("g;x=1/../y", "http://a/b/c/y"),
            ("g?y/./x", "http://a/b/c/g?y/./x"),
            ("g?y/../x", "http://a/b/c/g?y/../x"),
            ("g#s/./x", "http://a/b/c/g"),
            ("g#s/../x", "http://a/b/c/g"),
            ("HTTP://h2:81/x/../y?z", "http://h2:81/y?z"),
        ] {
            assert_eq!(resolve(reference), expected, "{reference:?}");
        }
        // Another scheme, an http URL without a host.
        for refused in [
            "file:///etc/passwd",
            "https://a/x",
            "ftp://a/x",
            "corbaloc::a/k",
            "http:g",
            "http:///g",
            "///g",
        ] {
            assert!(
                resolve_redirect("a", "/b/c/d;p?q", refused).is_err(),
                "{refused}"
            );
        }
    }

    /// RFC 9112 §7.1: chunks up to the last chunk and its trailer; a
    /// chunk size is hex digits, and a size beyond what a fetch reads,
    /// however large, is refused before anything is allocated.
    #[test]
    fn corba_p2_7_6_10_7_a_chunked_body_is_read_within_its_limit() {
        let read = |wire: &str| read_chunked(&mut std::io::Cursor::new(wire.as_bytes()), "t");
        assert_eq!(
            read("4\r\nIOR:\r\n3;ext=1\r\n010\r\n0\r\nX-T: 1\r\n\r\n").unwrap(),
            b"IOR:010".to_vec()
        );
        for wire in [
            // The size the review measured: past usize after one octet.
            "1\r\nA\r\nffffffffffffffff\r\n",
            "1\r\nA\r\nfffffffffffffffe\r\n",
            "100001\r\n",
            "1\r\nA\r\nfffff\r\n",
            "+1\r\nA\r\n0\r\n\r\n",
            "\r\n",
            "1x\r\nA\r\n0\r\n\r\n",
        ] {
            assert!(matches!(read(wire), Err(FetchError::Failed(_))), "{wire:?}");
        }
        // Exactly the limit is read.
        let mut full = alloc::format!("{MAX_FETCH_BYTES:x}\r\n");
        full.push_str(&"a".repeat(MAX_FETCH_BYTES));
        full.push_str("\r\n0\r\n\r\n");
        assert_eq!(read(&full).unwrap().len(), MAX_FETCH_BYTES);
    }

    #[test]
    fn corba_p2_7_6_10_7_a_missing_file_is_minor_10() {
        let e = fetch("file:///nonexistent/zerodds/ns.ior").unwrap_err();
        assert_eq!(e.minor(), 10);
    }
}
