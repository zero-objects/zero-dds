// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The catalog of a participant (spec §4.2 `Catalog`, §4.6 reflection) and the
//! IDL endpoint `GET /idl/<dds_name>` over HTTP/2 and HTTP/1.1 (spec §5.2).

use super::*;

/// The topic services of a participant (§4.6): each mapping's service in
/// its package (`grpc_package` or the default of §5.1), with its DDS topic
/// and type. The reflection service adds itself ([`Reflection::of`]).
pub(crate) fn reflection_catalog(part: &Participant) -> ServiceCatalog {
    let mut catalog = ServiceCatalog::new();
    for t in &part.topics {
        catalog.register(TopicService {
            package: t.package(),
            service: t.grpc_service.clone(),
            dds_topic: t.dds_name.clone(),
            dds_type: t.dds_type.clone(),
        });
    }
    catalog
}

/// The `CatalogResp` of the `Catalog` RPC (spec §4.2, §5.2): a
/// `TopicEntry` per topic of the participant, with the fully-qualified
/// service name.
pub(crate) fn catalog_resp(part: &Participant, idl_base: &str) -> CatalogResp {
    CatalogResp {
        topics: part
            .topics
            .iter()
            .map(|t| TopicEntry {
                dds_name: t.dds_name.clone(),
                dds_type: t.dds_type.clone(),
                grpc_service: t.full_service(),
                idl_url: if t.idl_source.is_some() {
                    format!("{idl_base}{}", percent_encode_segment(&t.dds_name))
                } else {
                    String::new()
                },
            })
            .collect(),
    }
}

/// `<scheme>://<authority>/idl/` (Spec §5.2): `https` over TLS, `http` on
/// the cleartext listener, the authority the client addressed.
pub(crate) fn idl_base(tls: bool, authority: &str) -> String {
    format!("{}://{authority}/idl/", if tls { "https" } else { "http" })
}

/// A DDS topic name as one path segment (RFC 3986 §3.3): the unreserved
/// characters, the sub-delimiters, `:` and `@` stay, every other octet is
/// `%XX`.
pub(crate) fn percent_encode_segment(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The text of a percent-encoded path segment; `None` for a malformed
/// escape or octets that are not UTF-8.
pub(crate) fn percent_decode_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = segment.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The answer to `GET /idl/<dds_name>` (Spec §5.2): the IDL source of the
/// topic as `text/x-omg-idl`; 401 when the authentication of §7.2 fails,
/// 404 for a topic the participant does not map or one without
/// `idl_file`, 400 for a malformed name.
pub(crate) fn idl_answer(
    part: &Participant,
    path: &str,
    auth: Result<AuthSubject, crate::bridge_security::AuthError>,
    metrics: &DaemonMetrics,
) -> (u16, &'static str, String) {
    const TEXT: &str = "text/plain; charset=utf-8";
    if let Err(e) = auth {
        metrics.spec.auth_failure(&e);
        event(Level::Warn, "auth_failed")
            .text("path", path)
            .text("err", &e.to_string())
            .emit();
        return (401, TEXT, format!("{e}\n"));
    }
    let segment = path
        .strip_prefix("/idl/")
        .unwrap_or("")
        .split(['?', '#'])
        .next()
        .unwrap_or("");
    let Some(name) = percent_decode_segment(segment) else {
        return (400, TEXT, format!("malformed topic name {segment:?}\n"));
    };
    match part.topics.iter().find(|t| t.dds_name == name) {
        Some(TopicMapping {
            idl_source: Some(idl),
            ..
        }) => (200, "text/x-omg-idl", idl.clone()),
        Some(_) => (404, TEXT, format!("{name} has no idl_file\n")),
        None => (404, TEXT, format!("{name} is not mapped\n")),
    }
}

/// Spec §5.2: an HTTP/1.1 connection — on the cleartext listener one
/// whose first octets are an HTTP/1.1 request line (not the HTTP/2
/// preface), on the TLS listener one that negotiated ALPN `http/1.1` (or
/// none, with a request line). `GET /idl/<dds_name>` is answered as by
/// [`idl_answer`], authenticated with the `Authorization` header or the
/// client certificate's subject (`mtls_subject`); another path is 404,
/// another method 405. One request, then the connection closes.
pub(crate) fn serve_http1<S: Read + Write>(
    mut stream: S,
    start: &[u8],
    env: &ConnEnv<'_>,
    part: &Participant,
    mtls_subject: Option<AuthSubject>,
    peer: &str,
) -> std::io::Result<()> {
    const LIMIT: usize = 16 * 1024;
    let opened = Clock::now();
    let mut head = start.to_vec();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        if head.len() > LIMIT {
            stream.write_all(
                b"HTTP/1.1 431 Request Header Fields Too Large\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )?;
            return Ok(());
        }
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        head.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let metadata: Vec<(String, String)> = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let (status, ctype, body) = if method != "GET" {
        (405, "text/plain; charset=utf-8", "GET only\n".to_string())
    } else if target.starts_with("/idl/") {
        let ctx = env.live.current();
        idl_answer(
            part,
            target,
            authenticate_grpc(&ctx.auth, &metadata, mtls_subject),
            env.metrics,
        )
    } else {
        (404, "text/plain; charset=utf-8", String::new())
    };
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    log_http(peer, "http/1.1", method, target, status, body.len(), opened);
    let mut resp = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    resp.extend_from_slice(body.as_bytes());
    stream.write_all(&resp)?;
    stream.flush()
}

/// Spec §8.1: the `http` line of a request to the IDL endpoint of §5.2,
/// over HTTP/1.1 or HTTP/2 (`protocol` `http/1.1` or `h2`): `info` for a
/// 2xx answer, `warn` for another.
pub(crate) fn log_http(
    peer: &str,
    protocol: &str,
    method: &str,
    path: &str,
    status: u16,
    bytes: usize,
    opened: Clock,
) {
    event(
        if (200..300).contains(&status) {
            Level::Info
        } else {
            Level::Warn
        },
        "http",
    )
    .text("peer", peer)
    .text("protocol", protocol)
    .text("method", method)
    .text("path", path)
    .num("http_status", u64::from(status))
    .num("bytes", bytes as u64)
    .num("latency_us", opened.elapsed_us())
    .emit();
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::bridge_daemon::test_support::participant;
    use crate::bridge_daemon::*;

    /// §4.2, §5.2: the `Catalog` RPC answers a `CatalogResp` with a
    /// `TopicEntry` per topic, the service fully qualified.
    #[test]
    fn grpcbridge_5_2_build_catalog_emits_topics() {
        let cfg = DaemonConfig {
            topics: vec![
                TopicMapping {
                    dds_name: "Foo::Bar".into(),
                    grpc_service: "FooBarStream".into(),
                    direction: "bidir".into(),
                    ..TopicMapping::default()
                },
                TopicMapping {
                    dds_name: "Chat::Message".into(),
                    dds_type: "chat::Message".into(),
                    grpc_service: "ChatMessageStream".into(),
                    grpc_package: "zerodds.chat.v1".into(),
                    ..TopicMapping::default()
                },
            ],
            ..DaemonConfig::default()
        };
        let wire = catalog_resp(&participant(&cfg), "http://h:1/idl/").encode();
        let cat = CatalogResp::decode(&wire).unwrap();
        assert_eq!(cat.topics.len(), 2);
        assert_eq!(cat.topics[0].dds_name, "Foo::Bar");
        assert_eq!(cat.topics[0].dds_type, dds_pump::BRIDGE_TYPE_NAME);
        assert_eq!(cat.topics[0].grpc_service, "zerodds.foo.v1.FooBarStream");
        assert_eq!(cat.topics[1].dds_type, "chat::Message");
        assert_eq!(
            cat.topics[1].grpc_service,
            "zerodds.chat.v1.ChatMessageStream"
        );
    }
}
