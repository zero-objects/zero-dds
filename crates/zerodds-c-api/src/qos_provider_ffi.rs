// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! QoS profiles for the QosProvider of DDS-PSM-Cxx 1.0 §7.6.2.1: a provider
//! loads the DDS-XML QoS libraries a `file:` URI names and answers the QoS of
//! each entity kind for a profile, with the profile's `base_name` inheritance
//! resolved (DDS-XML 1.0 §7.3.2.4.2).
//!
//! The QoS comes back in the C structs the entities' own `get_qos` fill: the
//! caller provides the buffers of USER_DATA, TOPIC_DATA and GROUP_DATA, and
//! a buffer that is too small is `ZERODDS_OUT_OF_RESOURCES` with the length
//! needed in `value_len`. The PARTITION names stay valid until the next call
//! of the provider for that entity kind.

use core::ffi::{c_char, c_int};
use std::ffi::CStr;
use std::sync::Mutex;

use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_xml::qos::{EntityQos, QosLibrary, default_qos};
use zerodds_xml::qos_inheritance::{ResolvedQos, resolve_profile};
use zerodds_xml::qos_parser::parse_qos_libraries;

use crate::ZeroDdsStatus;
use crate::qos_ffi::{
    PartitionOutCache, ZeroDdsDataReaderQos, ZeroDdsDataWriterQos, ZeroDdsDomainParticipantQos,
    ZeroDdsPublisherQos, ZeroDdsSubscriberQos, ZeroDdsTopicQos, dp_qos_to_c, dr_qos_to_c,
    dw_qos_to_c, pub_qos_to_c, sub_qos_to_c, topic_qos_to_c,
};

/// A QosProvider: the QoS libraries of one document, and the profile its
/// `*_qos()` operations without an id use.
pub struct ZeroDdsQosProvider {
    libraries: Vec<QosLibrary>,
    default_profile: Option<String>,
    publisher_partition: Mutex<PartitionOutCache>,
    subscriber_partition: Mutex<PartitionOutCache>,
    writer_partition: Mutex<PartitionOutCache>,
    reader_partition: Mutex<PartitionOutCache>,
}

/// Percent-decodes `text` (RFC 3986 §2.1). `None` for a bad escape or a
/// result that is not UTF-8 when `utf8` is set.
fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// The scheme of a URI (RFC 3986 §3.1), lower-cased, and the rest after the colon.
fn split_scheme(uri: &str) -> Option<(String, &str)> {
    let (scheme, rest) = uri.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic()
        || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    Some((scheme.to_ascii_lowercase(), rest))
}

/// The path a `file:` URI names (RFC 8089 §2): `file:///path`,
/// `file://localhost/path`, `file:/path`; `%XX` escapes are decoded. `None`
/// for another scheme, for a host other than `localhost`, or for a relative
/// path.
fn file_path(uri: &str) -> Option<String> {
    let (scheme, rest) = split_scheme(uri)?;
    if scheme != "file" {
        return None;
    }
    let path = if let Some(after) = rest.strip_prefix("//") {
        let (host, path) = after.split_at(after.find('/')?);
        if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
            return None;
        }
        path
    } else {
        rest
    };
    if !path.starts_with('/') {
        return None;
    }
    let decoded = String::from_utf8(percent_decode(path)?).ok()?;
    // `file:///C:/dir/x.xml` names `C:/dir/x.xml` on Windows (RFC 8089 Appendix E.2).
    let drive = decoded.as_bytes();
    if cfg!(windows) && drive.len() >= 3 && drive[2] == b':' && drive[1].is_ascii_alphabetic() {
        return Some(decoded[1..].to_string());
    }
    Some(decoded)
}

/// The text a `data:` URI carries (RFC 2397): `data:[<media type>][;base64],<data>`.
/// Only XML and plain text media types are documents the provider reads.
fn data_document(uri: &str) -> Result<String, ZeroDdsStatus> {
    use base64::Engine as _;
    let Some((scheme, rest)) = split_scheme(uri) else {
        return Err(ZeroDdsStatus::BadParameter);
    };
    if scheme != "data" {
        return Err(ZeroDdsStatus::BadParameter);
    }
    let Some((meta, data)) = rest.split_once(',') else {
        return Err(ZeroDdsStatus::BadParameter);
    };
    let mut parts = meta.split(';');
    let media = parts.next().unwrap_or("").to_ascii_lowercase();
    let is_base64 = parts.any(|p| p.eq_ignore_ascii_case("base64"));
    if !(media.is_empty()
        || media == "text/xml"
        || media == "application/xml"
        || media == "text/plain"
        || media.ends_with("+xml"))
    {
        return Err(ZeroDdsStatus::Unsupported);
    }
    let bytes = if is_base64 {
        let raw = percent_decode(data).ok_or(ZeroDdsStatus::BadParameter)?;
        base64::engine::general_purpose::STANDARD
            .decode(raw)
            .map_err(|_| ZeroDdsStatus::BadParameter)?
    } else {
        percent_decode(data).ok_or(ZeroDdsStatus::BadParameter)?
    };
    String::from_utf8(bytes).map_err(|_| ZeroDdsStatus::BadParameter)
}

/// Fetches `http://` or `https://host[:port]/path` with one HTTP/1.1 GET (RFC 9112); a `200`
/// answer is the document. No redirects are followed. `https:` runs the request over TLS 1.2
/// or 1.3 (rustls), the server's certificate checked against the webpki roots (RFC 9110 §4.3.3).
fn http_document(uri: &str) -> Result<String, ZeroDdsStatus> {
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let bad = ZeroDdsStatus::BadParameter;
    let (scheme, rest) = split_scheme(uri).ok_or(bad)?;
    let tls = scheme == "https";
    let default_port = if tls { 443 } else { 80 };
    let after = rest.strip_prefix("//").ok_or(bad)?;
    let (authority, target) = match after.find(['/', '?']) {
        Some(at) => (&after[..at], &after[at..]),
        None => (after, "/"),
    };
    let target = if target.starts_with('/') {
        target.to_string()
    } else {
        format!("/{target}")
    };
    if authority.is_empty() || authority.contains('@') {
        return Err(bad);
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (addr, tail) = v6.split_once(']').ok_or(bad)?;
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse::<u16>().map_err(|_| bad)?,
            None if tail.is_empty() => default_port,
            None => return Err(bad),
        };
        (addr.to_string(), port)
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        (h.to_string(), p.parse::<u16>().map_err(|_| bad)?)
    } else {
        (authority.to_string(), default_port)
    };
    let timeout = Duration::from_secs(10);
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| bad)?
        .next()
        .ok_or(bad)?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(|_| bad)?;
    stream.set_read_timeout(Some(timeout)).map_err(|_| bad)?;
    stream.set_write_timeout(Some(timeout)).map_err(|_| bad)?;
    let request = format!(
        "GET {target} HTTP/1.1\r\nHost: {authority}\r\nAccept: application/xml, text/xml\r\nConnection: close\r\n\r\n"
    );
    let mut response = Vec::new();
    if tls {
        let name = rustls_pki_types::ServerName::try_from(host.clone()).map_err(|_| bad)?;
        let connection = rustls::ClientConnection::new(tls_config()?, name).map_err(|_| bad)?;
        let mut stream = rustls::StreamOwned::new(connection, stream);
        stream.write_all(request.as_bytes()).map_err(|_| bad)?;
        match stream.read_to_end(&mut response) {
            Ok(_) => {}
            // The server closed without close_notify. What was read is complete only if the
            // length the response announced is there; checked below.
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {}
            Err(_) => return Err(bad),
        }
    } else {
        stream.write_all(request.as_bytes()).map_err(|_| bad)?;
        stream.read_to_end(&mut response).map_err(|_| bad)?;
    }

    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or(bad)?;
    let head = core::str::from_utf8(&response[..split]).map_err(|_| bad)?;
    let body = &response[split + 4..];
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or(bad)?;
    if status_line.split_whitespace().nth(1) != Some("200") {
        return Err(bad);
    }
    let chunked = lines.any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    });
    let announced = head.split("\r\n").skip(1).find_map(|l| {
        l.split_once(':').and_then(|(k, v)| {
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
    });
    if !chunked && announced.is_some_and(|n| body.len() < n) {
        return Err(bad);
    }
    let bytes = if chunked {
        dechunk(body).ok_or(bad)?
    } else {
        body.to_vec()
    };
    String::from_utf8(bytes).map_err(|_| bad)
}

/// The TLS client configuration: TLS 1.2 and 1.3, the ring provider, the webpki roots.
fn tls_config() -> Result<std::sync::Arc<rustls::ClientConfig>, ZeroDdsStatus> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| ZeroDdsStatus::Error)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(std::sync::Arc::new(config))
}

/// The payload of an HTTP/1.1 chunked body (RFC 9112 §7.1). `None` when it is malformed.
fn dechunk(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = body.windows(2).position(|w| w == b"\r\n")?;
        let size_text = core::str::from_utf8(&body[..end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        body = &body[end + 2..];
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(body.get(..size)?);
        body = body.get(size + 2..)?;
    }
}

/// The document `uri` names. The scheme decides the protocol, the document is
/// DDS-XML (DDS-PSM-Cxx 1.0 §7.6.2.1): `file:` (RFC 8089), `data:` (RFC 2397)
/// and `http:`/`https:` (RFC 9110). A scheme no specification defines is
/// unknown: `ZERODDS_UNSUPPORTED`. A URI that is no URI is `ZERODDS_BAD_PARAMETER`, so
/// is one that cannot be read.
pub(crate) fn load_document(uri: &str) -> Result<String, ZeroDdsStatus> {
    let Some((scheme, _)) = split_scheme(uri) else {
        return Err(ZeroDdsStatus::BadParameter);
    };
    match scheme.as_str() {
        "file" => {
            let path = file_path(uri).ok_or(ZeroDdsStatus::BadParameter)?;
            std::fs::read_to_string(path).map_err(|_| ZeroDdsStatus::BadParameter)
        }
        "data" => data_document(uri),
        "http" | "https" => http_document(uri),
        _ => Err(ZeroDdsStatus::Unsupported),
    }
}

/// Creates a provider from the QoS libraries the URI `uri` names (`file:`, `data:`, `http:`, `https:`);
/// `profile` (`"Library::Profile"`, or NULL) is the profile the provider
/// answers for when a call names none. Returns NULL and sets `*out_status`
/// (if not NULL) otherwise: `ZERODDS_UNSUPPORTED` for a scheme no
/// specification defines (DDS-PSM-Cxx 1.0 §2.1: an implementation without the
/// source answers UNSUPPORTED), `ZERODDS_BAD_PARAMETER` for a document that
/// cannot be read or is not a DDS-XML QoS document, or a profile it does not have.
///
/// # Safety
/// `uri` is a NUL-terminated string, `profile` is one or NULL, `out_status`
/// is writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_create(
    uri: *const c_char,
    profile: *const c_char,
    out_status: *mut c_int,
) -> *mut ZeroDdsQosProvider {
    let fail = |status: ZeroDdsStatus| {
        if !out_status.is_null() {
            // SAFETY: writable per the caller's contract.
            unsafe { *out_status = status as c_int };
        }
        core::ptr::null_mut()
    };
    if uri.is_null() {
        return fail(ZeroDdsStatus::BadParameter);
    }
    // SAFETY: NUL-terminated per the caller's contract.
    let Ok(uri) = unsafe { CStr::from_ptr(uri) }.to_str() else {
        return fail(ZeroDdsStatus::InvalidUtf8);
    };
    let profile = if profile.is_null() {
        None
    } else {
        // SAFETY: NUL-terminated per the caller's contract.
        match unsafe { CStr::from_ptr(profile) }.to_str() {
            Ok(p) if !p.is_empty() => Some(p.to_string()),
            Ok(_) => None,
            Err(_) => return fail(ZeroDdsStatus::InvalidUtf8),
        }
    };
    let xml = match load_document(uri) {
        Ok(xml) => xml,
        Err(status) => return fail(status),
    };
    let Ok(libraries) = parse_qos_libraries(&xml) else {
        return fail(ZeroDdsStatus::BadParameter);
    };
    let provider = ZeroDdsQosProvider {
        libraries,
        default_profile: profile,
        publisher_partition: Mutex::default(),
        subscriber_partition: Mutex::default(),
        writer_partition: Mutex::default(),
        reader_partition: Mutex::default(),
    };
    // A default profile the document does not have is refused now.
    if let Some(p) = provider.default_profile.clone() {
        if provider.resolve(Some(&p)).is_err() {
            return fail(ZeroDdsStatus::BadParameter);
        }
    }
    Box::into_raw(Box::new(provider))
}

/// Releases a provider. NULL is ignored.
///
/// # Safety
/// `p` is NULL or a provider from `zerodds_qosprovider_create`, not used
/// afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_destroy(p: *mut ZeroDdsQosProvider) {
    if !p.is_null() {
        // SAFETY: from `Box::into_raw` per the caller's contract.
        drop(unsafe { Box::from_raw(p) });
    }
}

impl ZeroDdsQosProvider {
    /// The resolved QoS of `id` (`"Library::Profile"`, or a profile name of
    /// any library), of the default profile, or of the only profile the
    /// documents have.
    fn resolve(&self, id: Option<&str>) -> Result<ResolvedQos, ZeroDdsStatus> {
        let wanted = id.or(self.default_profile.as_deref());
        let path = match wanted {
            Some(w) if w.contains("::") => w.to_string(),
            Some(name) => self
                .libraries
                .iter()
                .find(|l| l.profile(name).is_some())
                .map(|l| format!("{}::{name}", l.name))
                .ok_or(ZeroDdsStatus::BadParameter)?,
            None => {
                let mut all = self.libraries.iter().flat_map(|l| {
                    l.profiles
                        .iter()
                        .map(move |p| format!("{}::{}", l.name, p.name))
                });
                match (all.next(), all.next()) {
                    (Some(only), None) => only,
                    _ => return Err(ZeroDdsStatus::BadParameter),
                }
            }
        };
        resolve_profile(&self.libraries, &path).map_err(|_| ZeroDdsStatus::BadParameter)
    }
}

/// `id` as an optional string: NULL or empty is none.
///
/// # Safety
/// `id` is NULL or a NUL-terminated string that outlives the result.
unsafe fn id_of<'a>(id: *const c_char) -> Result<Option<&'a str>, ZeroDdsStatus> {
    if id.is_null() {
        return Ok(None);
    }
    // SAFETY: NUL-terminated per the caller's contract.
    match unsafe { CStr::from_ptr(id) }.to_str() {
        Ok("") => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(_) => Err(ZeroDdsStatus::InvalidUtf8),
    }
}

macro_rules! apply {
    ($q:ident, $e:expr, $($field:ident),+) => {
        if let Some(e) = $e {
            $( if let Some(p) = e.$field.clone() { $q.$field = p; } )+
        }
    };
}

fn participant_of(e: Option<&EntityQos>) -> DomainParticipantQos {
    let mut q = DomainParticipantQos::default();
    apply!(q, e, user_data, entity_factory);
    q
}

fn topic_of(e: Option<&EntityQos>) -> TopicQos {
    let mut q = TopicQos::default();
    apply!(
        q,
        e,
        durability,
        durability_service,
        deadline,
        latency_budget,
        liveliness,
        reliability,
        destination_order,
        history,
        resource_limits,
        transport_priority,
        lifespan,
        ownership,
        topic_data
    );
    q
}

fn publisher_of(e: Option<&EntityQos>) -> PublisherQos {
    let mut q = PublisherQos::default();
    apply!(q, e, presentation, partition, group_data, entity_factory);
    q
}

fn subscriber_of(e: Option<&EntityQos>) -> SubscriberQos {
    let mut q = SubscriberQos::default();
    apply!(q, e, presentation, partition, group_data, entity_factory);
    q
}

fn writer_of(e: Option<&EntityQos>) -> DataWriterQos {
    e.map_or_else(DataWriterQos::default, |e| e.into_writer_qos().into())
}

fn reader_of(e: Option<&EntityQos>) -> DataReaderQos {
    e.map_or_else(DataReaderQos::default, |e| e.into_reader_qos().into())
}

/// The resolved QoS container of the profile `id` for one entity kind.
///
/// # Safety
/// `p` is NULL or a live provider, `id` NULL or NUL-terminated.
unsafe fn resolved_for<'a>(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
) -> Result<(&'a ZeroDdsQosProvider, ResolvedQos), c_int> {
    if p.is_null() {
        return Err(ZeroDdsStatus::BadParameter as c_int);
    }
    // SAFETY: live per the caller's contract.
    let provider = unsafe { &*p };
    // SAFETY: NUL-terminated or NULL per the caller's contract.
    let id = unsafe { id_of(id) }.map_err(|status| status as c_int)?;
    let resolved = provider.resolve(id).map_err(|status| status as c_int)?;
    Ok((provider, resolved))
}

/// The QoS of a DomainParticipant for the profile `id` (NULL: the
/// provider's default profile). USER_DATA goes to the caller's buffer.
///
/// # Safety
/// `p` is a live provider, `id` NULL or NUL-terminated, `out` a QoS struct
/// with the caller's buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_participant_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((_, r)) => unsafe {
            dp_qos_to_c(&participant_of(default_qos(&r.domainparticipant_qos)), out)
        },
        Err(status) => status,
    }
}

/// The QoS of a Topic for the profile `id`. TOPIC_DATA goes to the caller's
/// buffer.
///
/// # Safety
/// As [`zerodds_qosprovider_participant_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_topic_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsTopicQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((_, r)) => unsafe { topic_qos_to_c(&topic_of(default_qos(&r.topic_qos)), out) },
        Err(status) => status,
    }
}

/// The QoS of a Publisher for the profile `id`. GROUP_DATA goes to the
/// caller's buffer.
///
/// # Safety
/// As [`zerodds_qosprovider_participant_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_publisher_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsPublisherQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((provider, r)) => unsafe {
            pub_qos_to_c(
                &publisher_of(default_qos(&r.publisher_qos)),
                out,
                &provider.publisher_partition,
            )
        },
        Err(status) => status,
    }
}

/// The QoS of a Subscriber for the profile `id`. GROUP_DATA goes to the
/// caller's buffer.
///
/// # Safety
/// As [`zerodds_qosprovider_participant_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_subscriber_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsSubscriberQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((provider, r)) => unsafe {
            sub_qos_to_c(
                &subscriber_of(default_qos(&r.subscriber_qos)),
                out,
                &provider.subscriber_partition,
            )
        },
        Err(status) => status,
    }
}

/// The QoS of a DataWriter for the profile `id`. USER_DATA, TOPIC_DATA and
/// GROUP_DATA go to the caller's buffers.
///
/// # Safety
/// As [`zerodds_qosprovider_participant_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_datawriter_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsDataWriterQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((provider, r)) => unsafe {
            dw_qos_to_c(
                &writer_of(default_qos(&r.datawriter_qos)),
                out,
                &provider.writer_partition,
            )
        },
        Err(status) => status,
    }
}

/// The QoS of a DataReader for the profile `id`. USER_DATA, TOPIC_DATA and
/// GROUP_DATA go to the caller's buffers.
///
/// # Safety
/// As [`zerodds_qosprovider_participant_qos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_qosprovider_datareader_qos(
    p: *mut ZeroDdsQosProvider,
    id: *const c_char,
    out: *mut ZeroDdsDataReaderQos,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { resolved_for(p, id) } {
        // SAFETY: the out buffers are the caller's, per the function's contract.
        Ok((provider, r)) => unsafe {
            dr_qos_to_c(
                &reader_of(default_qos(&r.datareader_qos)),
                out,
                &provider.reader_partition,
            )
        },
        Err(status) => status,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The status as its C code, which has `PartialEq`.
    fn doc(r: Result<String, ZeroDdsStatus>) -> Result<String, c_int> {
        r.map_err(|e| e as c_int)
    }

    #[test]
    fn file_uris_name_a_path_and_other_schemes_do_not() {
        assert_eq!(file_path("file:///a/b.xml").as_deref(), Some("/a/b.xml"));
        assert_eq!(file_path("file:/a/b.xml").as_deref(), Some("/a/b.xml"));
        assert_eq!(
            file_path("file://localhost/a/b%20c.xml").as_deref(),
            Some("/a/b c.xml")
        );
        assert_eq!(file_path("FILE:///a/b.xml").as_deref(), Some("/a/b.xml"));
        assert_eq!(file_path("file://other-host/a.xml"), None);
        assert_eq!(file_path("file:relative.xml"), None);
        assert_eq!(file_path("http://host/a.xml"), None);
        assert_eq!(file_path("/a/b.xml"), None);
    }

    #[test]
    fn a_data_uri_carries_the_document_percent_encoded_or_in_base64() {
        assert_eq!(
            doc(data_document("data:,%3Ca%2F%3E")).as_deref(),
            Ok("<a/>")
        );
        assert_eq!(
            doc(data_document("data:text/xml;base64,PGEvPg==")).as_deref(),
            Ok("<a/>")
        );
        assert_eq!(
            doc(data_document("data:application/octet-stream,abc")),
            Err(ZeroDdsStatus::Unsupported as c_int)
        );
        assert_eq!(
            doc(data_document("data:text/xml;base64,@@@")),
            Err(ZeroDdsStatus::BadParameter as c_int)
        );
        assert_eq!(
            doc(data_document("data:text/xml")),
            Err(ZeroDdsStatus::BadParameter as c_int)
        );
    }

    #[test]
    fn the_scheme_decides_the_protocol() {
        // https: is a protocol of its own, not an unknown scheme: a closed port is a document
        // that cannot be read.
        assert_eq!(
            doc(load_document("https://127.0.0.1:1/a.xml")),
            Err(ZeroDdsStatus::BadParameter as c_int)
        );
        assert_eq!(
            doc(load_document("ftp://h/a.xml")),
            Err(ZeroDdsStatus::Unsupported as c_int)
        );
        assert_eq!(
            doc(load_document("no scheme")),
            Err(ZeroDdsStatus::BadParameter as c_int)
        );
        assert_eq!(
            doc(load_document("data:,%3Ca%2F%3E")).as_deref(),
            Ok("<a/>")
        );
    }

    #[test]
    fn a_chunked_body_is_joined() {
        assert_eq!(
            dechunk(b"4\r\nWiki\r\n5;ext=1\r\npedia\r\n0\r\n\r\n").as_deref(),
            Some(&b"Wikipedia"[..])
        );
        assert_eq!(dechunk(b"zz\r\n"), None);
    }

    #[test]
    fn an_http_uri_is_fetched_with_one_get() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().expect("accept");
            let mut request = [0u8; 512];
            let n = conn.read(&mut request).expect("read");
            let text = String::from_utf8_lossy(&request[..n]).into_owned();
            assert!(text.starts_with("GET /q.xml HTTP/1.1\r\n"), "{text}");
            conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n<a/>")
                .expect("write");
        });
        let got = doc(http_document(&format!("http://127.0.0.1:{port}/q.xml")));
        server.join().expect("server");
        assert_eq!(got.as_deref(), Ok("<a/>"));
    }
}
