// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Resolves `corba.ior.naming_service` (a `corbaname:`/`corbaloc:` URL,
//! CORBA 3.4 Part 2 §13.6.10) and `rebind`s every receiving mapping's
//! servant into it at start, `unbind`s them at a clean stop.
//!
//! The wire codec is `zerodds-corba-cosnaming`'s `wire` module — the
//! same one its servant decodes requests with — so a request this
//! module builds and a reply `zerodds-naming-svc` (or a foreign
//! NameService) sends back agree byte for byte on the operation shape.
//!
//! ## Intermediate contexts
//!
//! A `corbaname:…#App/Trader` URL's fragment is a path: this module
//! creates every missing segment (`bind_new_context` against the root,
//! ignoring `AlreadyBound` — spec §2.5.4.8) before rebinding each
//! mapping's servant under `<path>/<repo_id-operation slug>`. There is
//! no configuration flag to turn this off: creating an already-existing
//! intermediate context is a no-op (`AlreadyBound`, swallowed), so
//! doing it unconditionally is safe on every rebind, including a
//! `SIGHUP`-triggered one.
//!
//! ## Retry
//!
//! `corba.ior.naming_retry_ms` (default 30000) is the *total* budget:
//! exponential backoff from 200 ms, doubled each attempt, capped at
//! 5 s, until the budget is spent — then the start aborts with the
//! last connect/bind error. A NameService that is not up yet when this
//! daemon starts (the common case in a compose-style deployment with
//! no ordering guarantee) gets that whole window rather than one
//! attempt.
//!
//! ## `ssliop:`/`tls:` endpoints
//!
//! `corba.ior.naming_ssliop.ca_file` is the trust root the
//! NameService's own server certificate is checked against — required
//! for an `ssliop:`/`tls:` endpoint, refused otherwise (never a
//! silently-unauthenticated TLS connection). `cert_file`/`key_file`
//! present a client certificate (SSLIOP `EstablishTrustInClient`, for a
//! NameService that requires mutual TLS); both empty presents none.
//!
//! ## `rir:` endpoints
//!
//! `corbaloc:rir:/NameService` — "resolve_initial_references" — has no
//! network address of its own; a real ORB resolves it against its own
//! initial-reference table (populated by e.g. omniORB/TAO's
//! `-ORBInitRef NAME=...` or JacORB's `ORBInitRef.NAME=...`). This
//! daemon is not a full ORB and keeps no such table implicitly, so
//! `rir:` resolves only for a name configured in
//! `corba.ior.initial_references` (CORBA 3.4 Part 2 §13.6.10.1); an
//! unconfigured name is refused by name, not silently treated as
//! `rir:`'s absence of an address.
//!
//! ## What is not built
//!
//! Any endpoint protocol other than `iiop:`/protocol-less, `ssliop:`,
//! `tls:` or `rir:` (resolving to one of the first three) is refused by
//! name.

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_corba_cosnaming::name::{Name, NameComponent};
use zerodds_corba_cosnaming::wire;
use zerodds_corba_giop::{
    Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
    Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, tls};
use zerodds_corba_ior::url::CorbalocEndpoint;
use zerodds_corba_ior::{Ior, parse_corbaloc, parse_corbaname};
use zerodds_wire::{Endianness, Reader};

use crate::bridge_daemon::{DaemonConfig, build_ior, slug};

/// Error resolving or talking to `corba.ior.naming_service`.
#[derive(Debug, Clone)]
pub struct NamingClientError(pub String);

impl core::fmt::Display for NamingClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
struct Resolved {
    endpoints: Vec<CorbalocEndpoint>,
    root_key: Vec<u8>,
    path: Name,
}

fn parse_naming_url(url: &str) -> Result<Resolved, NamingClientError> {
    if let Ok(cn) = parse_corbaname(url) {
        let path = if cn.stringified_name.is_empty() {
            Vec::new()
        } else {
            zerodds_corba_cosnaming::stringified::string_to_name(&cn.stringified_name)
                .map_err(|e| NamingClientError(format!("{url}: fragment name: {e:?}")))?
        };
        return Ok(Resolved {
            endpoints: cn.address.endpoints,
            root_key: cn.address.object_key,
            path,
        });
    }
    let loc = parse_corbaloc(url).map_err(|e| {
        NamingClientError(format!("{url}: not a corbaname: or corbaloc: URL ({e:?})"))
    })?;
    Ok(Resolved {
        endpoints: loc.endpoints,
        root_key: loc.object_key,
        path: Vec::new(),
    })
}

/// As [`parse_naming_url`], additionally resolving a `rir:` endpoint
/// against `initial_references` (see this module's own doc). Recurses
/// exactly once — the mapped URL is parsed with [`parse_naming_url`]
/// directly, not fed back through `rir:` resolution again, so a
/// misconfigured `initial_references` entry cannot loop.
fn resolve_naming_service(
    url: &str,
    initial_references: &std::collections::BTreeMap<String, String>,
) -> Result<Resolved, NamingClientError> {
    let resolved = parse_naming_url(url)?;
    let Some(rir) = resolved
        .endpoints
        .iter()
        .find(|e| e.protocol.eq_ignore_ascii_case("rir"))
    else {
        return Ok(resolved);
    };
    let _ = rir; // the endpoint itself carries no address; the object key does
    let reference_name = String::from_utf8_lossy(&resolved.root_key).into_owned();
    let mapped_url = initial_references.get(&reference_name).ok_or_else(|| {
        NamingClientError(format!(
            "{url}: rir:/{reference_name} needs \
             corba.ior.initial_references.{reference_name}"
        ))
    })?;
    let mapped = parse_naming_url(mapped_url)?;
    let mut path = mapped.path;
    path.extend(resolved.path);
    Ok(Resolved {
        endpoints: mapped.endpoints,
        root_key: mapped.root_key,
        path,
    })
}

/// A resolved endpoint together with the transport it is reached over.
#[derive(Debug, PartialEq, Eq)]
enum Transport {
    Plain,
    Ssliop,
}

fn pick_endpoint(
    endpoints: &[CorbalocEndpoint],
) -> Result<(&CorbalocEndpoint, Transport), NamingClientError> {
    if let Some(e) = endpoints
        .iter()
        .find(|e| e.protocol.is_empty() || e.protocol.eq_ignore_ascii_case("iiop"))
    {
        return Ok((e, Transport::Plain));
    }
    if let Some(e) = endpoints.iter().find(|e| {
        e.protocol.eq_ignore_ascii_case("ssliop") || e.protocol.eq_ignore_ascii_case("tls")
    }) {
        return Ok((e, Transport::Ssliop));
    }
    let protocols: Vec<&str> = endpoints.iter().map(|e| e.protocol.as_str()).collect();
    Err(NamingClientError(format!(
        "corba.ior.naming_service names no iiop:/ssliop:/tls: endpoint (only {protocols:?})"
    )))
}

/// How to reach `corba.ior.naming_service` — either a plain [`Connector`]
/// or one carrying an SSLIOP client config, built once per
/// [`rebind_once`]/[`unbind_all`] call from `cfg.naming_ssliop_*`.
enum NamingConnector {
    Plain(Connector),
    Ssliop {
        connector: Connector,
        sni: String,
        config: Arc<rustls::ClientConfig>,
    },
}

impl NamingConnector {
    fn build(
        cfg: &DaemonConfig,
        endpoint: &CorbalocEndpoint,
        transport: &Transport,
    ) -> Result<Self, NamingClientError> {
        match transport {
            Transport::Plain => Ok(Self::Plain(Connector::new(ConnectorConfig::default()))),
            Transport::Ssliop => {
                if cfg.naming_ssliop_ca_file.is_empty() {
                    return Err(NamingClientError(
                        "an ssliop:/tls: corba.ior.naming_service needs \
                         corba.ior.naming_ssliop.ca_file"
                            .into(),
                    ));
                }
                let ca_pem = std::fs::read(&cfg.naming_ssliop_ca_file).map_err(|e| {
                    NamingClientError(format!(
                        "corba.ior.naming_ssliop.ca_file {}: {e}",
                        cfg.naming_ssliop_ca_file
                    ))
                })?;
                let (cert_pem, key_pem) = if cfg.naming_ssliop_cert_file.is_empty() {
                    (Vec::new(), Vec::new())
                } else {
                    let cert = std::fs::read(&cfg.naming_ssliop_cert_file).map_err(|e| {
                        NamingClientError(format!(
                            "corba.ior.naming_ssliop.cert_file {}: {e}",
                            cfg.naming_ssliop_cert_file
                        ))
                    })?;
                    let key = std::fs::read(&cfg.naming_ssliop_key_file).map_err(|e| {
                        NamingClientError(format!(
                            "corba.ior.naming_ssliop.key_file {}: {e}",
                            cfg.naming_ssliop_key_file
                        ))
                    })?;
                    (cert, key)
                };
                let config = tls::load_client_config(&ca_pem, &cert_pem, &key_pem)
                    .map_err(|e| NamingClientError(format!("naming_ssliop: {e:?}")))?;
                Ok(Self::Ssliop {
                    connector: Connector::new(ConnectorConfig::default()),
                    sni: endpoint.host.clone(),
                    config,
                })
            }
        }
    }

    fn connect(
        &self,
        host: &str,
        port: u16,
    ) -> Result<zerodds_corba_iiop::connector::PooledConnection, zerodds_corba_iiop::IiopError>
    {
        match self {
            Self::Plain(c) => c.connect(host, port),
            Self::Ssliop {
                connector,
                sni,
                config,
            } => connector.connect_tls(host, port, sni, Arc::clone(config)),
        }
    }
}

/// One round trip: `operation` against `object_key`, returns the reply
/// and the byte order it is in.
fn call(
    connector: &NamingConnector,
    host: &str,
    port: u16,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
    request_id: u32,
) -> Result<(Reply, Endianness), NamingClientError> {
    let mut pooled = connector
        .connect(host, port)
        .map_err(|e| NamingClientError(format!("connect {host}:{port}: {e:?}")))?;
    let conn = pooled
        .connection()
        .ok_or_else(|| NamingClientError("connection unavailable".into()))?;
    let req = Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(object_key.to_vec()),
        operation: operation.to_string(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body,
    };
    conn.write_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .map_err(|e| NamingClientError(format!("{operation}: write: {e:?}")))?;
    let (msg, endianness, _version) = conn
        .read_message_full()
        .map_err(|e| NamingClientError(format!("{operation}: read: {e:?}")))?;
    match msg {
        Message::Reply(r) => Ok((r, endianness)),
        other => Err(NamingClientError(format!(
            "{operation}: expected a Reply, got {:?}",
            other.message_type()
        ))),
    }
}

fn is_already_bound(reply: &Reply, endianness: Endianness) -> bool {
    if reply.reply_status != ReplyStatusType::UserException {
        return false;
    }
    let mut r = Reader::new(&reply.body, endianness);
    r.read_string()
        .is_ok_and(|repo_id| repo_id == wire::REPO_ID_ALREADY_BOUND)
}

fn exception_text(reply: &Reply, endianness: Endianness) -> String {
    if reply.reply_status == ReplyStatusType::UserException {
        let mut r = Reader::new(&reply.body, endianness);
        return r.read_string().unwrap_or_else(|_| "<unreadable>".into());
    }
    String::from_utf8_lossy(&reply.body)
        .split("IDL:omg.org/CORBA/")
        .nth(1)
        .and_then(|t| t.split(':').next())
        .map(|s| format!("CORBA::{s}"))
        .unwrap_or_else(|| "<unknown>".into())
}

fn create_intermediate_contexts(
    connector: &NamingConnector,
    host: &str,
    port: u16,
    root_key: &[u8],
    path: &Name,
    next_id: &mut u32,
) -> Result<(), NamingClientError> {
    for depth in 1..=path.len() {
        let prefix: Name = path[..depth].to_vec();
        let body = wire::encode_name_request(&prefix, Endianness::Little)
            .map_err(|e| NamingClientError(format!("{e}")))?;
        let (reply, endianness) = call(
            connector,
            host,
            port,
            root_key,
            "bind_new_context",
            body,
            *next_id,
        )?;
        *next_id += 1;
        if reply.reply_status == ReplyStatusType::NoException
            || is_already_bound(&reply, endianness)
        {
            continue;
        }
        return Err(NamingClientError(format!(
            "bind_new_context {:?}: {}",
            prefix
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
                .join("/"),
            exception_text(&reply, endianness)
        )));
    }
    Ok(())
}

fn rebind_once(
    cfg: &DaemonConfig,
    advertise_host: &str,
    resolved: &Resolved,
) -> Result<Vec<Name>, NamingClientError> {
    let (endpoint, transport) = pick_endpoint(&resolved.endpoints)?;
    // `endpoint.host`/`endpoint.port` are passed through as-is (not
    // pre-resolved to a single `SocketAddr`) so every `call()` below
    // re-resolves and — via `TcpStream::connect`'s own multi-address
    // fallback — tries every address a hostname resolves to, not just
    // the first one. A hostname like "localhost" commonly resolves to
    // both an IPv6 and an IPv4 loopback address; a NameService bound to
    // only one family must still be reachable by name, not just by IP
    // literal. Collapsing to `.to_socket_addrs().next()` up front (the
    // prior behaviour here) silently pinned the connection to whichever
    // address the resolver listed first, so a target listening only on
    // the other family was refused even though it was reachable.
    let connector = NamingConnector::build(cfg, endpoint, &transport)?;
    let mut next_id = 1u32;

    create_intermediate_contexts(
        &connector,
        &endpoint.host,
        endpoint.port,
        &resolved.root_key,
        &resolved.path,
        &mut next_id,
    )?;

    let mut bound = Vec::new();
    for m in cfg.mappings.iter().filter(|m| m.receives_giop()) {
        let ior: Ior = build_ior(cfg, m, advertise_host).map_err(NamingClientError)?;
        let leaf = NameComponent::new(slug(&m.repo_id, &m.operation));
        let mut full_name = resolved.path.clone();
        full_name.push(leaf);
        // `rebind` walks a compound name against the root context
        // itself (the NamingContext implementation resolves every
        // intermediate component along the way), so the request names
        // the full path against `root_key`, not a single component
        // against an intermediate context's own key.
        let body = wire::encode_bind_request(&full_name, &ior, Endianness::Little)
            .map_err(|e| NamingClientError(format!("{e}")))?;
        let (reply, endianness) = call(
            &connector,
            &endpoint.host,
            endpoint.port,
            &resolved.root_key,
            "rebind",
            body,
            next_id,
        )?;
        next_id += 1;
        if reply.reply_status != ReplyStatusType::NoException {
            return Err(NamingClientError(format!(
                "rebind {} {}: {}",
                m.repo_id,
                m.operation,
                exception_text(&reply, endianness)
            )));
        }
        bound.push(full_name);
    }
    Ok(bound)
}

/// Resolves `cfg.naming_service` and `rebind`s every receiving mapping's
/// current IOR into it, retrying with backoff for up to
/// `cfg.naming_retry_ms` in total. Returns the full [`Name`] of every
/// binding made (for [`unbind_all`]) — empty when `naming_service` is
/// not configured, which is not an error.
///
/// # Errors
/// [`NamingClientError`] once the retry budget is spent without a
/// successful rebind of every mapping.
pub fn rebind_all(
    cfg: &DaemonConfig,
    advertise_host: &str,
) -> Result<Vec<Name>, NamingClientError> {
    if cfg.naming_service.is_empty() {
        return Ok(Vec::new());
    }
    let resolved = resolve_naming_service(&cfg.naming_service, &cfg.naming_initial_references)?;
    let budget = Duration::from_millis(cfg.naming_retry_ms);
    let start = Instant::now();
    let mut backoff = Duration::from_millis(200);
    loop {
        match rebind_once(cfg, advertise_host, &resolved) {
            Ok(names) => return Ok(names),
            Err(e) => {
                let elapsed = start.elapsed();
                if elapsed >= budget {
                    return Err(NamingClientError(format!(
                        "corba.ior.naming_service: giving up after {}ms: {e}",
                        budget.as_millis()
                    )));
                }
                std::thread::sleep(backoff.min(budget - elapsed));
                backoff = (backoff * 2).min(Duration::from_secs(5));
            }
        }
    }
}

/// `unbind`s every name [`rebind_all`] bound, best effort: a failure
/// (the NameService already gone, a network error) is not propagated —
/// the daemon is stopping either way, and a stale binding is a
/// NameService-side cleanup question, not one this call can still act
/// on. Does nothing when `cfg.naming_service` is empty.
pub fn unbind_all(cfg: &DaemonConfig, names: &[Name]) {
    if cfg.naming_service.is_empty() || names.is_empty() {
        return;
    }
    let Ok(resolved) = resolve_naming_service(&cfg.naming_service, &cfg.naming_initial_references)
    else {
        return;
    };
    let Ok((endpoint, transport)) = pick_endpoint(&resolved.endpoints) else {
        return;
    };
    let Ok(connector) = NamingConnector::build(cfg, endpoint, &transport) else {
        return;
    };
    for (i, name) in names.iter().enumerate() {
        let Ok(body) = wire::encode_name_request(name, Endianness::Little) else {
            continue;
        };
        let _ = call(
            &connector,
            &endpoint.host,
            endpoint.port,
            &resolved.root_key,
            "unbind",
            body,
            u32::try_from(i).unwrap_or(0) + 1,
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn no_refs() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn resolve_corbaloc_has_an_empty_path() {
        let r = resolve_naming_service("corbaloc::host:2809/NameService", &no_refs()).unwrap();
        assert!(r.path.is_empty());
        assert_eq!(r.root_key, b"NameService");
    }

    #[test]
    fn resolve_corbaname_splits_the_fragment_into_a_path() {
        let r = resolve_naming_service("corbaname::host:2809/NameService#App/Trader", &no_refs())
            .unwrap();
        assert_eq!(r.path.len(), 2);
        assert_eq!(r.path[0].id, "App");
        assert_eq!(r.path[1].id, "Trader");
    }

    #[test]
    fn resolve_corbaname_without_a_fragment_has_an_empty_path() {
        let r = resolve_naming_service("corbaname::host:2809/NameService", &no_refs()).unwrap();
        assert!(r.path.is_empty());
    }

    /// `rir:` with no matching `corba.ior.initial_references` entry is
    /// refused by name, not silently treated as an address-less no-op.
    #[test]
    fn an_unconfigured_rir_reference_is_refused_by_name() {
        let err = resolve_naming_service("corbaloc:rir:/NameService", &no_refs()).unwrap_err();
        assert!(err.0.contains("initial_references.NameService"));
    }

    /// `rir:` with a configured `initial_references` entry resolves to
    /// that entry's own endpoint/object-key, keeping the `rir:` URL's
    /// own fragment path (if any).
    #[test]
    fn a_configured_rir_reference_resolves_through_initial_references() {
        let mut refs = BTreeMap::new();
        refs.insert(
            "NameService".to_string(),
            "corbaloc::ns.example:2809/NS".to_string(),
        );
        let r = resolve_naming_service("corbaname:rir:/NameService#App/Trader", &refs).unwrap();
        assert_eq!(r.root_key, b"NS");
        assert_eq!(r.endpoints[0].host, "ns.example");
        assert_eq!(r.endpoints[0].port, 2809);
        assert_eq!(r.path.len(), 2);
        assert_eq!(r.path[0].id, "App");
    }

    /// A `rir:` entry that maps to a name other than the one it names is
    /// still resolved by the *referenced* name, and a lookup miss for a
    /// *different* configured name is still refused.
    #[test]
    fn rir_resolution_is_looked_up_by_the_urls_own_reference_name() {
        let mut refs = BTreeMap::new();
        refs.insert(
            "TradingService".to_string(),
            "corbaloc::ts.example:2809/TS".to_string(),
        );
        let err = resolve_naming_service("corbaloc:rir:/NameService", &refs).unwrap_err();
        assert!(err.0.contains("initial_references.NameService"));
    }

    #[test]
    fn a_url_that_is_neither_scheme_is_refused() {
        let err = resolve_naming_service("http://x", &no_refs()).unwrap_err();
        assert!(err.0.contains("corbaname") || err.0.contains("corbaloc"));
    }

    #[test]
    fn pick_endpoint_prefers_iiop_over_ssliop_when_both_are_offered() {
        let r =
            resolve_naming_service("corbaloc:iiop:1.2@a:1,ssliop:1.2@b:2/NS", &no_refs()).unwrap();
        let (endpoint, transport) = pick_endpoint(&r.endpoints).unwrap();
        assert_eq!(endpoint.host, "a");
        assert_eq!(transport, Transport::Plain);
    }

    #[test]
    fn pick_endpoint_falls_back_to_ssliop_when_no_iiop_endpoint_exists() {
        let r = resolve_naming_service("corbaloc:ssliop:1.2@b:2/NS", &no_refs()).unwrap();
        let (endpoint, transport) = pick_endpoint(&r.endpoints).unwrap();
        assert_eq!(endpoint.host, "b");
        assert_eq!(transport, Transport::Ssliop);
    }

    #[test]
    fn pick_endpoint_accepts_tls_as_a_synonym_for_ssliop() {
        let r = resolve_naming_service("corbaloc:tls:1.2@b:2/NS", &no_refs()).unwrap();
        let (_endpoint, transport) = pick_endpoint(&r.endpoints).unwrap();
        assert_eq!(transport, Transport::Ssliop);
    }

    #[test]
    fn pick_endpoint_refuses_an_unsupported_protocol() {
        let r = resolve_naming_service("corbaloc:corbaloc-iiop:1.2@b:2/NS", &no_refs()).unwrap();
        let err = pick_endpoint(&r.endpoints).unwrap_err();
        assert!(err.0.contains("iiop"));
    }

    /// An `ssliop:` `corba.ior.naming_service` without
    /// `corba.ior.naming_ssliop.ca_file` is refused, not attempted
    /// without transport security.
    #[test]
    fn ssliop_naming_service_without_ca_file_is_refused() {
        let cfg = DaemonConfig {
            naming_service: "corbaloc:ssliop:1.2@127.0.0.1:1/NS".into(),
            naming_retry_ms: 0,
            ..Default::default()
        };
        let err = rebind_all(&cfg, "127.0.0.1").unwrap_err();
        assert!(err.0.contains("naming_ssliop.ca_file"));
    }

    #[test]
    fn rebind_all_with_no_naming_service_configured_does_nothing() {
        let cfg = DaemonConfig::default();
        assert!(rebind_all(&cfg, "127.0.0.1").unwrap().is_empty());
    }

    #[test]
    fn unbind_all_with_no_naming_service_configured_does_not_panic() {
        let cfg = DaemonConfig::default();
        unbind_all(&cfg, &[vec![NameComponent::new("x")]]);
    }

    /// The retry budget is honored: an unreachable NameService gives up
    /// once the whole `naming_retry_ms` window elapses, not on the
    /// first failed connect.
    #[test]
    fn rebind_all_retries_for_the_configured_budget() {
        use crate::bridge_daemon::{Direction, Mapping, TopicRef};
        let cfg = DaemonConfig {
            naming_service: "corbaloc::127.0.0.1:1/NameService".into(), // nothing listens on port 1
            naming_retry_ms: 350,
            mappings: vec![Mapping {
                repo_id: "IDL:x/Y:1.0".into(),
                operation: "op".into(),
                direction: Direction::CorbaToDds,
                request_topic: TopicRef::default(),
                reply_topic: TopicRef::default(),
                notify_topic: TopicRef::default(),
                timeout_ms: 1000,
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                targets: Vec::new(),
            }],
            ..Default::default()
        };
        let start = Instant::now();
        let err = rebind_all(&cfg, "127.0.0.1").unwrap_err();
        assert!(start.elapsed() >= Duration::from_millis(300));
        assert!(err.0.contains("giving up"));
    }
}
