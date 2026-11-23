// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon as a CoAP-to-CoAP proxy (RFC 7252 §5.7).
//!
//! * Forward proxy (§5.7.2): a request with Proxy-Uri, or with Proxy-Scheme
//!   and the Uri-* options, is carried out against the URI it names; one
//!   that names the daemon itself is a local request. Without
//!   `proxy.forward` or for a scheme not in `proxy.schemes` it is
//!   `5.05 Proxying Not Supported`.
//! * Reverse proxy (§5.7.3): the resources under a `proxy.reverse` path are
//!   those under its origin URI.
//! * Options (§5.7.1, §5.4): every option of a proxy request is processed;
//!   an unrecognized Unsafe one is `4.02 Bad Option`, an unrecognized
//!   Safe-to-Forward one is forwarded; an unrecognized Unsafe option in a
//!   response is `5.02 Bad Gateway`, as is a response that cannot be
//!   decoded; no response in time is `5.04 Gateway Timeout`.
//! * Hop-Limit (RFC 8768): decremented, inserted with `proxy.max_hops`
//!   when absent, `5.08 Hop Limit Reached` with the proxy's identifier at
//!   zero.
//! * Cache (§5.6, §5.7.1, §11.2): `2.05` responses to GET, the Max-Age
//!   reduced by the time in the cache, validation with ETag; responses
//!   to `coaps` requests are kept per DTLS identity and served to no one
//!   else.
//! * `coaps` destinations (§5.7, §9): over DTLS with the certificate
//!   checked against `proxy.dtls.ca_file`, or with a pre-shared key; the
//!   client leg must be DTLS too (`proxy.require_dtls_for_coaps`).
//! * Multicast destinations (§8.2.2): the request goes out once as
//!   Non-confirmable, every response within `proxy.multicast_wait_ms` goes
//!   back to the client.
//! * Block-wise transfer (RFC 7959 §2.10): the proxy reassembles both
//!   directions and serves the blocks of a representation itself, at most
//!   the amplification RFC 7252 §11.3 allows to a client that is not
//!   authenticated.
//! * Message layer (§4, §5.2.2): a response that is not there within a
//!   second turns into an empty Acknowledgement and a separate
//!   Confirmable response; a retransmitted request is not carried out
//!   twice (§4.5).

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_bridge_security::{AuthSubject, LiveSecurity};

use crate::blockwise::{BlockReassembler, BlockValue};
use crate::caching_proxy::{CacheKey, CacheLookup, CacheScope, CoapCache};
use crate::codec::{decode, encode};
use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{self, CoapOption, OptionValue, numbers, uint_from_bytes};
use crate::uri::{CoapUri, UriError, parse_coap_uri};

use super::config::ProxySettings;
use super::security::{COAP_OPTION_AUTH_TOKEN, authenticate_coap};
use super::server::{
    block_limit, extract_auth_token, extract_block, make_response, representation, suppressed,
};

/// RFC 7252 §4.8: ACK_TIMEOUT.
const ACK_TIMEOUT: Duration = Duration::from_secs(2);
/// RFC 7252 §4.8: MAX_RETRANSMIT.
const MAX_RETRANSMIT: u32 = 4;
/// How long a Confirmable request waits for a piggybacked response before
/// it is acknowledged empty (§5.2.2; below ACK_TIMEOUT, so that the client
/// does not retransmit).
const PIGGYBACK_WAIT: Duration = Duration::from_millis(1000);
/// RFC 7252 §4.8.2: EXCHANGE_LIFETIME, how long a request is remembered
/// for the deduplication of §4.5.
const EXCHANGE_LIFETIME: Duration = Duration::from_secs(247);
/// The most proxied requests carried out at a time; beyond, `5.03`.
const MAX_WORKERS: usize = 64;
/// The most remembered exchanges.
const MAX_EXCHANGES: usize = 4096;
/// The largest representation the proxy reassembles.
const MAX_REPRESENTATION: usize = 1 << 20;
/// The block size (SZX) of a body the proxy sends to a destination in
/// blocks (1024 octets).
const UPSTREAM_SZX: u8 = 6;
/// How long a DTLS handshake with one address of a host may take before
/// the next address is tried.
#[cfg(feature = "dtls")]
const DTLS_ATTEMPT: Duration = Duration::from_secs(3);
/// The largest payload sent to a client without blocks (§4.6: a message
/// should fit 1152 octets).
const UNBLOCKED_PAYLOAD: usize = 1024;

/// The options of a request the proxy processes itself or forwards
/// knowingly (§5.7.1: "All options present in a proxy request MUST be
/// processed at the proxy").
const FORWARDED_REQUEST_OPTIONS: [u16; 9] = [
    numbers::NO_RESPONSE,
    numbers::IF_MATCH,
    numbers::ETAG,
    numbers::IF_NONE_MATCH,
    numbers::OSCORE,
    numbers::CONTENT_FORMAT,
    numbers::ACCEPT,
    numbers::SIZE2,
    numbers::SIZE1,
];

/// The options of a response the proxy understands.
const KNOWN_RESPONSE_OPTIONS: [u16; 11] = [
    numbers::ETAG,
    numbers::OBSERVE,
    numbers::LOCATION_PATH,
    numbers::OSCORE,
    numbers::CONTENT_FORMAT,
    numbers::MAX_AGE,
    numbers::LOCATION_QUERY,
    numbers::BLOCK2,
    numbers::BLOCK1,
    numbers::SIZE2,
    numbers::SIZE1,
];

/// Where the responses to a request go: the client's UDP endpoint or its
/// DTLS session.
#[derive(Clone)]
pub(super) enum ReplyTo {
    /// Plain UDP.
    Udp {
        /// The daemon's socket the request came in on.
        socket: Arc<UdpSocket>,
        /// The client.
        peer: SocketAddr,
    },
    /// A DTLS session; its thread writes what arrives on `tx`.
    #[cfg(feature = "dtls")]
    Dtls {
        /// The client.
        peer: SocketAddr,
        /// The queue of the session.
        tx: super::server::DtlsTx,
    },
}

impl ReplyTo {
    fn peer(&self) -> SocketAddr {
        match self {
            Self::Udp { peer, .. } => *peer,
            #[cfg(feature = "dtls")]
            Self::Dtls { peer, .. } => *peer,
        }
    }

    fn send(&self, msg: &CoapMessage) -> bool {
        match self {
            Self::Udp { socket, peer } => encode(msg)
                .ok()
                .and_then(|bytes| socket.send_to(&bytes, *peer).ok())
                .is_some(),
            #[cfg(feature = "dtls")]
            Self::Dtls { tx, .. } => tx.send(msg.clone()),
        }
    }
}

/// The client side of a request.
#[derive(Clone)]
pub(super) struct ClientLeg {
    /// Where the responses go.
    pub reply: ReplyTo,
    /// The address the request was sent to: from `IP_PKTINFO` where the
    /// system tells it, else the bound address.
    pub destination: SocketAddr,
    /// The identity the client proved in a DTLS handshake; `None` without
    /// DTLS.
    pub dtls_peer: Option<AuthSubject>,
    /// The size of the request datagram (§11.3).
    pub request_len: usize,
}

/// What became of a request at the proxy.
pub(super) enum Handled {
    /// No proxy request: the daemon serves it, as it came or, when a
    /// Proxy-Uri named the daemon itself, as rewritten here (§5.7.2).
    Local(Option<CoapMessage>),
    /// The proxy answers it (now or from a worker).
    Proxied,
}

/// Why a request to the destination produced no response.
#[derive(Debug)]
enum Failure {
    /// No response in time (§5.7.1: 5.04).
    Timeout,
    /// A response that cannot be processed, or no way to reach the
    /// destination (§5.7.1: 5.02).
    BadGateway(String),
    /// The address refused the datagram (ICMP port unreachable): the next
    /// address of the host is tried, after the last one it is 5.02.
    Unreachable(String),
}

/// A request on its way to its destination.
#[derive(Clone)]
struct Target {
    uri: CoapUri,
    /// For a reverse route: the path segments of the route and of the
    /// origin, to map a Location-Path back (§5.7.3).
    reverse: Option<(Vec<String>, Vec<String>)>,
    /// The options sent to the destination besides the Uri-* ones.
    options: Vec<CoapOption>,
}

enum Route {
    Local(Option<CoapMessage>),
    Reply(CoapCode, String),
    Forward(Box<Target>),
}

/// The work of a proxied request.
struct Job {
    req: CoapMessage,
    target: Target,
    /// The whole body (Block1 reassembled).
    body: Vec<u8>,
    /// The final Block1 of the client, echoed in the final response
    /// (RFC 7959 §2.3).
    block1_echo: Option<BlockValue>,
    /// The cache entry the response belongs to and, for a stale one, the
    /// ETag to validate.
    cache: Option<(CacheKey, Option<Vec<u8>>)>,
    multicast: bool,
    authenticated: bool,
}

/// A remembered request (§4.5).
struct Exchange {
    at: Instant,
    /// What acknowledged it: the piggybacked response or the empty ACK.
    ack: Option<CoapMessage>,
}

/// Block1 bodies being collected, by client, token and target URI.
type Block1Key = (SocketAddr, Vec<u8>, String);

/// The proxy of one daemon.
pub(super) struct Proxy {
    settings: ProxySettings,
    /// The reverse routes: local path segments and origin.
    reverse: Vec<(Vec<String>, CoapUri)>,
    cache: CoapCache,
    mid: Arc<AtomicU16>,
    security: Arc<LiveSecurity>,
    /// The ports of the daemon (§5.7.2: an authority naming one of them
    /// with an address of the daemon is the daemon itself).
    plain_port: u16,
    dtls_port: Option<u16>,
    /// The bound address, unspecified for "any".
    bind_ip: IpAddr,
    /// The identifier in a 5.08 diagnostic (RFC 8768 §4).
    identifier: String,
    workers: AtomicUsize,
    exchanges: Mutex<BTreeMap<(SocketAddr, u16), Exchange>>,
    confirmations: Mutex<BTreeMap<(SocketAddr, u16), mpsc::Sender<bool>>>,
    block1: Mutex<BTreeMap<Block1Key, (Instant, BlockReassembler)>>,
    /// What the proxy proves and whom it trusts towards a `coaps`
    /// destination; the server name is set per request.
    #[cfg(feature = "dtls")]
    dtls: Option<zerodds_dtls_openssl::ClientConfig>,
}

impl Proxy {
    /// The proxy for `settings`; `topics` are the daemon's own resource
    /// names, which no reverse route may cover.
    ///
    /// # Errors
    /// A reverse route that covers a resource of the daemon, an origin that
    /// is no `coap`/`coaps` URI, or DTLS credentials that cannot be loaded.
    pub(super) fn new(
        settings: ProxySettings,
        topics: &[String],
        mid: Arc<AtomicU16>,
        security: Arc<LiveSecurity>,
        bind: SocketAddr,
        dtls_port: Option<u16>,
    ) -> Result<Self, String> {
        let mut reverse = Vec::new();
        for (path, origin) in &settings.reverse {
            let local: Vec<String> = path.split('/').map(str::to_string).collect();
            if path.is_empty() {
                return Err("proxy.reverse: an empty path would cover every resource".into());
            }
            for own in topics
                .iter()
                .map(String::as_str)
                .chain([".well-known/core"])
            {
                let own: Vec<&str> = own.split('/').collect();
                let covers = own.len() >= local.len()
                    && own.iter().zip(&local).all(|(a, b)| *a == b.as_str());
                let covered = local.len() > own.len()
                    && own.iter().zip(&local).all(|(a, b)| *a == b.as_str());
                if covers || covered {
                    return Err(format!(
                        "proxy.reverse: the path `{path}` overlaps the resource `{}`",
                        own.join("/")
                    ));
                }
            }
            let origin =
                parse_coap_uri(origin).map_err(|e| format!("proxy.reverse `{origin}`: {e}"))?;
            reverse.push((local, origin));
        }
        let identifier = settings
            .names
            .first()
            .cloned()
            .unwrap_or_else(|| SocketAddr::new(bind.ip(), bind.port()).to_string())
            .replace(' ', "");
        #[cfg(feature = "dtls")]
        let dtls = {
            let trusted = crate::dtls_transport::parse_key_names(&settings.dtls_rpk_trusted)
                .map_err(|e| format!("proxy.dtls.rpk_trusted: {e}"))?;
            let usable = settings.dtls_psk.is_some()
                || settings.dtls_ca_file.is_some()
                || !trusted.is_empty();
            usable.then(|| zerodds_dtls_openssl::ClientConfig {
                psk: settings
                    .dtls_psk
                    .as_ref()
                    .map(|(id, key)| (id.clone(), key.clone())),
                certificate: settings
                    .dtls_cert_file
                    .clone()
                    .zip(settings.dtls_key_file.clone())
                    .map(|(c, k)| zerodds_dtls_openssl::CertificateFiles {
                        cert_file: c.into(),
                        key_file: k.into(),
                    }),
                raw_public_key: settings.dtls_rpk_key_file.clone().map(Into::into),
                ca_file: settings.dtls_ca_file.clone().map(Into::into),
                trusted_server_keys: trusted,
                server_name: None,
                cipher_suites: Vec::new(),
            })
        };
        Ok(Self {
            cache: CoapCache::new(settings.cache_entries),
            settings,
            reverse,
            mid,
            security,
            plain_port: bind.port(),
            dtls_port,
            bind_ip: bind.ip(),
            identifier,
            workers: AtomicUsize::new(0),
            exchanges: Mutex::new(BTreeMap::new()),
            confirmations: Mutex::new(BTreeMap::new()),
            block1: Mutex::new(BTreeMap::new()),
            #[cfg(feature = "dtls")]
            dtls,
        })
    }

    /// An empty Acknowledgement (`reset` false) or Reset from `peer` for
    /// the message `mid`: ends the retransmission of a separate response.
    /// `true` when it was one.
    pub(super) fn confirm(&self, peer: SocketAddr, mid: u16, reset: bool) -> bool {
        let waiting = self
            .confirmations
            .lock()
            .ok()
            .and_then(|mut c| c.remove(&(peer, mid)));
        match waiting {
            Some(tx) => {
                let _ = tx.send(!reset);
                true
            }
            None => false,
        }
    }

    /// Takes a request; see [`Handled`].
    pub(super) fn handle(self: &Arc<Self>, req: &CoapMessage, leg: ClientLeg) -> Handled {
        let target = match self.route(req, &leg) {
            Route::Local(rewritten) => return Handled::Local(rewritten),
            Route::Reply(code, diagnostic) => {
                self.reply_now(req, &leg, code, diagnostic.into_bytes());
                return Handled::Proxied;
            }
            Route::Forward(target) => *target,
        };
        let peer = leg.reply.peer();
        let confirmable = matches!(req.message_type, MessageType::Confirmable);
        // §4.5: a request seen before is carried out once; a Confirmable
        // one gets the same acknowledgement again.
        if let Ok(mut exchanges) = self.exchanges.lock() {
            if let Some(seen) = exchanges.get(&(peer, req.message_id)) {
                if let (true, Some(ack)) = (confirmable, &seen.ack) {
                    leg.reply.send(ack);
                }
                return Handled::Proxied;
            }
            let now = Instant::now();
            exchanges.retain(|_, e| now.duration_since(e.at) < EXCHANGE_LIFETIME);
            if exchanges.len() >= MAX_EXCHANGES {
                if let Some(oldest) = exchanges.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| *k) {
                    exchanges.remove(&oldest);
                }
            }
            exchanges.insert((peer, req.message_id), Exchange { at: now, ack: None });
        }

        // The proxy forwards for the clients the bridge lets in (§7.2 of
        // the bridge specification): the auth token or the DTLS identity.
        let token = extract_auth_token(req);
        if authenticate_coap(
            &self.security.current().auth,
            token.as_deref(),
            leg.dtls_peer.as_ref(),
        )
        .is_err()
        {
            self.reply_now(req, &leg, CoapCode::new(4, 1), b"unauthorized".to_vec());
            return Handled::Proxied;
        }
        if target.uri.secure {
            // §5.7: the request for a "coaps" URI "SHOULD be sent using
            // DTLS" to the proxy; §11.2: what the proxy keeps of it is no
            // less protected.
            if self.settings.require_dtls_for_coaps && leg.dtls_peer.is_none() {
                self.reply_now(
                    req,
                    &leg,
                    CoapCode::new(4, 1),
                    b"a coaps destination needs DTLS to the proxy".to_vec(),
                );
                return Handled::Proxied;
            }
            if !self.coaps_ready() {
                self.reply_now(
                    req,
                    &leg,
                    CoapCode::new(5, 5),
                    b"no DTLS credentials for coaps destinations".to_vec(),
                );
                return Handled::Proxied;
            }
        }
        let protected = req.options.iter().any(|o| o.number == numbers::OSCORE);
        let authenticated = leg.dtls_peer.is_some() || token.is_some() || protected;

        // RFC 7959 §2.10: the proxy collects a Block1 body before it
        // forwards it whole.
        let mut body = req.payload.clone();
        let mut block1_echo = None;
        if let Some(b1) = extract_block(req, numbers::BLOCK1) {
            match self.collect_block1(peer, req, &target, b1) {
                Ok(Some(whole)) => {
                    body = whole;
                    block1_echo = Some(b1);
                }
                Ok(None) => {
                    let mut resp = make_response(req, CoapCode::new(2, 31), Vec::new());
                    resp.options.push(CoapOption::block1(b1.num, true, b1.szx));
                    self.deliver(req, &leg, vec![resp], false);
                    return Handled::Proxied;
                }
                Err((code, diagnostic)) => {
                    self.reply_now(req, &leg, code, diagnostic.into_bytes());
                    return Handled::Proxied;
                }
            }
        }

        let multicast = target
            .uri
            .host
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_multicast());
        let mut cache = None;
        if let Some(key) = self.cache_key(req, &target, &leg, multicast) {
            match self.cache.serve(&key) {
                CacheLookup::Fresh(stored) => {
                    let resp = self.for_client(req, &leg, stored, authenticated);
                    self.deliver(req, &leg, vec![resp], false);
                    return Handled::Proxied;
                }
                CacheLookup::Stale { etag, .. } => cache = Some((key, Some(etag))),
                CacheLookup::Miss => cache = Some((key, None)),
            }
        }

        if self.workers.fetch_add(1, Ordering::SeqCst) >= MAX_WORKERS {
            self.workers.fetch_sub(1, Ordering::SeqCst);
            let mut resp = make_response(req, CoapCode::new(5, 3), b"proxy busy".to_vec());
            resp.options
                .push(CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(5)));
            self.deliver(req, &leg, vec![resp], false);
            return Handled::Proxied;
        }
        let job = Job {
            req: req.clone(),
            target,
            body,
            block1_echo,
            cache,
            multicast,
            authenticated,
        };
        let proxy = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("coap-proxy".into())
            .spawn(move || proxy.run(job, leg));
        if spawned.is_err() {
            self.workers.fetch_sub(1, Ordering::SeqCst);
        }
        Handled::Proxied
    }

    /// Whether a `coaps` destination can be reached with checked DTLS.
    fn coaps_ready(&self) -> bool {
        #[cfg(feature = "dtls")]
        {
            self.dtls.is_some()
        }
        #[cfg(not(feature = "dtls"))]
        {
            false
        }
    }

    fn route(&self, req: &CoapMessage, leg: &ClientLeg) -> Route {
        let proxy_uri: Vec<&CoapOption> = options(req, numbers::PROXY_URI).collect();
        let proxy_scheme: Vec<&CoapOption> = options(req, numbers::PROXY_SCHEME).collect();
        if proxy_uri.is_empty() && proxy_scheme.is_empty() {
            return self.reverse_route(req).unwrap_or(Route::Local(None));
        }
        // §5.4.5: a second occurrence of these critical options is an
        // unrecognized critical option.
        if proxy_uri.len() > 1 || proxy_scheme.len() > 1 {
            return Route::Reply(
                CoapCode::new(4, 2),
                "Proxy-Uri or Proxy-Scheme twice".into(),
            );
        }
        let uri = if let Some(o) = proxy_uri.first() {
            // §5.10.2: Proxy-Uri takes precedence over the Uri-* options.
            let Some(text) = string_value(o) else {
                return Route::Reply(CoapCode::BAD_REQUEST, "Proxy-Uri is no UTF-8".into());
            };
            match parse_coap_uri(&text) {
                Ok(uri) => uri,
                Err(UriError::InvalidScheme) => {
                    return Route::Reply(
                        CoapCode::new(5, 5),
                        "only coap and coaps URIs are proxied".into(),
                    );
                }
                Err(e) => return Route::Reply(CoapCode::BAD_REQUEST, format!("Proxy-Uri: {e}")),
            }
        } else {
            // §5.10.2 with §6.5: the URI from Proxy-Scheme, Uri-Host (else
            // the request's destination), Uri-Port (else its port),
            // Uri-Path and Uri-Query.
            let scheme = proxy_scheme
                .first()
                .and_then(|o| string_value(o))
                .unwrap_or_default()
                .to_ascii_lowercase();
            let secure = match scheme.as_str() {
                "coap" => false,
                "coaps" => true,
                _ => {
                    return Route::Reply(
                        CoapCode::new(5, 5),
                        "only coap and coaps URIs are proxied".into(),
                    );
                }
            };
            let host = options(req, numbers::URI_HOST)
                .next()
                .and_then(string_value)
                .map_or_else(
                    || leg.destination.ip().to_string(),
                    |h| h.to_ascii_lowercase(),
                );
            let host = host
                .strip_prefix('[')
                .and_then(|h| h.strip_suffix(']'))
                .map_or(host.clone(), str::to_string);
            let port = options(req, numbers::URI_PORT)
                .next()
                .and_then(uint_value)
                .and_then(|p| u16::try_from(p).ok())
                .unwrap_or(leg.destination.port());
            CoapUri {
                secure,
                host,
                port,
                path_segments: options(req, numbers::URI_PATH)
                    .filter_map(string_value)
                    .collect(),
                query_params: options(req, numbers::URI_QUERY)
                    .filter_map(string_value)
                    .collect(),
            }
        };
        if self.is_self(&uri, leg) {
            return Route::Local(Some(local_request(req, &uri)));
        }
        let scheme = if uri.secure { "coaps" } else { "coap" };
        if !self.settings.forward || !self.settings.schemes.iter().any(|s| s == scheme) {
            return Route::Reply(CoapCode::new(5, 5), "proxying not supported".into());
        }
        self.target(req, uri, None)
    }

    /// §5.7.2: "If the authority (host and port) is recognized as
    /// identifying the proxy endpoint itself [...], then the request MUST
    /// be treated as a local (non-proxied) request."
    fn is_self(&self, uri: &CoapUri, leg: &ClientLeg) -> bool {
        let port = if uri.secure {
            self.dtls_port
        } else {
            Some(self.plain_port)
        };
        if port != Some(uri.port) {
            return false;
        }
        if self
            .settings
            .names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&uri.host))
        {
            return true;
        }
        let Ok(ip) = uri.host.parse::<IpAddr>() else {
            return false;
        };
        ip == leg.destination.ip()
            || (!self.bind_ip.is_unspecified() && ip == self.bind_ip)
            || (self.bind_ip.is_unspecified() && ip.is_loopback())
    }

    /// §5.7.3: a request under a reverse route goes to its origin.
    fn reverse_route(&self, req: &CoapMessage) -> Option<Route> {
        let segments: Vec<String> = options(req, numbers::URI_PATH)
            .filter_map(string_value)
            .collect();
        let (local, origin) = self
            .reverse
            .iter()
            .find(|(local, _)| segments.starts_with(local))?;
        let mut uri = origin.clone();
        uri.path_segments
            .extend(segments[local.len()..].iter().cloned());
        uri.query_params
            .extend(options(req, numbers::URI_QUERY).filter_map(string_value));
        Some(self.target(
            req,
            uri,
            Some((local.clone(), origin.path_segments.clone())),
        ))
    }

    /// The options of the request to the destination (§5.7.1, §5.7.2).
    fn target(
        &self,
        req: &CoapMessage,
        uri: CoapUri,
        reverse: Option<(Vec<String>, Vec<String>)>,
    ) -> Route {
        let mut forwarded = Vec::new();
        let mut hop_limit = None;
        let mut seen = BTreeSet::new();
        for o in &req.options {
            let n = o.number;
            if !seen.insert(n) && !option::is_repeatable(n) {
                // §5.4.5: "each supernumerary option occurrence [...] MUST
                // be treated like an unrecognized option".
                if option::is_critical(n) {
                    return Route::Reply(CoapCode::new(4, 2), format!("option {n} twice"));
                }
                continue;
            }
            match n {
                // Consumed (§5.7.2) or the proxy's own (§7.2 of the bridge
                // specification).
                numbers::PROXY_URI
                | numbers::PROXY_SCHEME
                | numbers::URI_HOST
                | numbers::URI_PORT
                | numbers::URI_PATH
                | numbers::URI_QUERY
                | COAP_OPTION_AUTH_TOKEN => {}
                numbers::HOP_LIMIT => {
                    // RFC 8768 §3: "between 1 and 255 inclusive [...]
                    // MUST be rejected [...] using 4.00 (Bad Request)".
                    match uint_value(o) {
                        Some(v @ 1..=255) => hop_limit = u8::try_from(v).ok(),
                        _ => {
                            return Route::Reply(
                                CoapCode::BAD_REQUEST,
                                "Hop-Limit out of 1..=255".into(),
                            );
                        }
                    }
                }
                // The proxy serves no observation (RFC 7641 §2: a response
                // without Observe tells the client so) and handles the
                // blocks of a transfer itself (RFC 7959 §2.10).
                numbers::OBSERVE | numbers::BLOCK1 | numbers::BLOCK2 => {}
                n if FORWARDED_REQUEST_OPTIONS.contains(&n) => forwarded.push(o.clone()),
                // §5.7.1: "Unsafe options in a request that are not
                // recognized by the proxy MUST lead to a 4.02".
                n if option::is_unsafe(n) => {
                    return Route::Reply(CoapCode::new(4, 2), format!("unsafe option {n}"));
                }
                // "A CoAP-to-CoAP proxy MUST forward to the origin server
                // all Safe-to-Forward options that it does not recognize."
                _ => forwarded.push(o.clone()),
            }
        }
        // RFC 8768 §3: decremented, or inserted with the initial value.
        let next = match hop_limit {
            Some(v) => v - 1,
            None => self.settings.max_hops,
        };
        if next == 0 {
            super::log::event(super::log::Level::Warn, "proxy_hop_limit")
                .field("coap_uri", &uri.to_uri())
                .emit();
            return Route::Reply(CoapCode::new(5, 8), self.identifier.clone());
        }
        forwarded.push(CoapOption::new(
            numbers::HOP_LIMIT,
            OptionValue::Uint(u64::from(next)),
        ));
        Route::Forward(Box::new(Target {
            uri,
            reverse,
            options: forwarded,
        }))
    }

    /// The cache entry of a request, when its response may be kept: a GET
    /// that is not OSCORE-protected, validates no ETag of the client and
    /// goes to a single destination; a `coaps` one only for a client with
    /// a DTLS identity, under that identity (§11.2).
    fn cache_key(
        &self,
        req: &CoapMessage,
        target: &Target,
        leg: &ClientLeg,
        multicast: bool,
    ) -> Option<CacheKey> {
        if req.code != CoapCode::GET
            || multicast
            || target
                .options
                .iter()
                .any(|o| o.number == numbers::OSCORE || o.number == numbers::ETAG)
        {
            return None;
        }
        let scope = if target.uri.secure {
            CacheScope::Private(leg.dtls_peer.as_ref()?.name.clone())
        } else {
            CacheScope::Shared
        };
        Some(CacheKey {
            scope,
            method: req.code.to_byte(),
            uri: target.uri.to_uri(),
            options: target
                .options
                .iter()
                .filter(|o| !option::is_nocachekey(o.number))
                .map(|o| (o.number, o.value.to_wire_bytes()))
                .collect(),
        })
    }

    /// RFC 7959 §2.5 at the proxy: the blocks of a body, the whole body
    /// with the last one.
    fn collect_block1(
        &self,
        peer: SocketAddr,
        req: &CoapMessage,
        target: &Target,
        b1: BlockValue,
    ) -> Result<Option<Vec<u8>>, (CoapCode, String)> {
        let key = (peer, req.token.clone(), target.uri.to_uri());
        let mut map = self
            .block1
            .lock()
            .map_err(|_| (CoapCode::INTERNAL_SERVER_ERROR, "block1".to_string()))?;
        let now = Instant::now();
        map.retain(|_, (at, _)| now.duration_since(*at) < EXCHANGE_LIFETIME);
        if b1.num == 0 {
            map.remove(&key);
        }
        let size = b1
            .block_size()
            .map_err(|_| (CoapCode::BAD_REQUEST, "Block1 SZX 7".to_string()))?;
        let (at, entry) = map
            .entry(key.clone())
            .or_insert_with(|| (now, BlockReassembler::new(size)));
        *at = now;
        // RFC 7959 §2.5: a block out of order is 4.08 Request Entity
        // Incomplete.
        entry
            .accept(b1, &req.payload)
            .map_err(|e| (CoapCode::new(4, 8), e.to_string()))?;
        if entry.len() > MAX_REPRESENTATION {
            map.remove(&key);
            return Err((CoapCode::new(4, 13), "body too large".to_string()));
        }
        if entry.is_complete() {
            Ok(map.remove(&key).map(|(_, r)| r.into_payload()))
        } else {
            Ok(None)
        }
    }

    fn reply_now(&self, req: &CoapMessage, leg: &ClientLeg, code: CoapCode, diagnostic: Vec<u8>) {
        let resp = make_response(req, code, diagnostic);
        self.deliver(req, leg, vec![resp], false);
    }

    /// Carries out a request against its destination and answers the
    /// client; on a worker thread.
    fn run(self: Arc<Self>, job: Job, leg: ClientLeg) {
        let req = &job.req;
        let peer = leg.reply.peer();
        let acked = AtomicBool::new(false);
        let confirmable = matches!(req.message_type, MessageType::Confirmable);
        // §5.2.2: no response within the wait — an empty ACK now, the
        // response separately later.
        let slow = || {
            if confirmable && !acked.swap(true, Ordering::SeqCst) {
                let ack = CoapMessage::new(
                    MessageType::Acknowledgement,
                    CoapCode::EMPTY,
                    req.message_id,
                );
                leg.reply.send(&ack);
                self.remember_ack(peer, req.message_id, ack);
            }
        };
        let deadline = Instant::now() + Duration::from_millis(self.settings.timeout_ms);
        let slow_at = Instant::now() + PIGGYBACK_WAIT;
        let fetched = if job.multicast {
            self.fetch_multicast(&job, &slow, slow_at)
        } else if job.target.uri.secure {
            self.fetch_dtls(&job, &slow, slow_at, deadline)
        } else {
            self.fetch_udp(&job, &slow, slow_at, deadline)
        };
        let responses = match fetched {
            Ok(responses) => responses
                .into_iter()
                .map(|r| self.answer_from_destination(&job, &leg, r))
                .collect(),
            Err(Failure::Timeout) => vec![make_response(
                req,
                CoapCode::new(5, 4),
                b"no response from the destination".to_vec(),
            )],
            Err(Failure::BadGateway(diagnostic) | Failure::Unreachable(diagnostic)) => {
                super::log::event(super::log::Level::Warn, "proxy_bad_gateway")
                    .field("coap_uri", &job.target.uri.to_uri())
                    .field("reason", &diagnostic)
                    .emit();
                vec![make_response(
                    req,
                    CoapCode::new(5, 2),
                    diagnostic.into_bytes(),
                )]
            }
        };
        self.deliver(req, &leg, responses, acked.load(Ordering::SeqCst));
        self.workers.fetch_sub(1, Ordering::SeqCst);
    }

    /// A response of the destination on its way to the client (§5.7.1,
    /// RFC 8768 §4, §5.6, §5.7.3).
    fn answer_from_destination(
        &self,
        job: &Job,
        leg: &ClientLeg,
        mut resp: CoapMessage,
    ) -> CoapMessage {
        let req = &job.req;
        // "Unsafe options in a response that are not recognized by the
        // CoAP-to-CoAP proxy server MUST lead to a 5.02".
        if let Some(n) = resp
            .options
            .iter()
            .map(|o| o.number)
            .find(|n| !KNOWN_RESPONSE_OPTIONS.contains(n) && option::is_unsafe(*n))
        {
            return make_response(
                req,
                CoapCode::new(5, 2),
                format!("unsafe option {n} in the response").into_bytes(),
            );
        }
        if resp.code == CoapCode::new(5, 8) {
            // RFC 8768 §4: the identifiers in the diagnostic, this proxy's
            // put in front; its own already there is a loop.
            let ids = String::from_utf8_lossy(&resp.payload).to_string();
            if ids.split(' ').any(|id| id == self.identifier) {
                super::log::event(super::log::Level::Warn, "proxy_loop")
                    .field("reason", &ids)
                    .emit();
                return make_response(
                    req,
                    CoapCode::new(5, 2),
                    format!("request loop through {ids}").into_bytes(),
                );
            }
            let joined = format!("{} {ids}", self.identifier);
            if joined.len() <= UNBLOCKED_PAYLOAD {
                resp.payload = joined.into_bytes();
            }
        }
        resp.options.retain(|o| o.number != numbers::OBSERVE);
        // §5.7.3: a Location-Path in the origin's namespace in the
        // proxy's.
        if let Some((local, origin)) = &job.target.reverse {
            let location: Vec<String> = options(&resp, numbers::LOCATION_PATH)
                .filter_map(string_value)
                .collect();
            if !location.is_empty() && location.starts_with(origin) {
                let mapped: Vec<String> = local
                    .iter()
                    .cloned()
                    .chain(location[origin.len()..].iter().cloned())
                    .collect();
                resp.options.retain(|o| o.number != numbers::LOCATION_PATH);
                resp.options.extend(
                    mapped
                        .into_iter()
                        .map(|s| CoapOption::new(numbers::LOCATION_PATH, OptionValue::String(s))),
                );
            }
        }
        // §5.9.1.1, §5.9.1.2, §5.9.1.4: a created, deleted or changed
        // resource is no longer fresh in the cache.
        if matches!(
            (resp.code.class, resp.code.detail),
            (2, 1) | (2, 2) | (2, 4)
        ) {
            self.cache.invalidate(&job.target.uri.to_uri());
        }
        if let Some((key, validating)) = &job.cache {
            if resp.code == CoapCode::VALID && validating.is_some() {
                match self.cache.revalidated(key, &resp) {
                    Some(stored) => resp = stored,
                    None => {
                        return make_response(
                            req,
                            CoapCode::new(5, 2),
                            b"2.03 for an ETag the proxy did not ask for".to_vec(),
                        );
                    }
                }
            } else {
                self.cache.store_response(key, &resp);
            }
        }
        if let Some(b1) = job.block1_echo {
            resp.options.push(CoapOption::block1(b1.num, false, b1.szx));
        }
        self.for_client(req, leg, resp, job.authenticated)
    }

    /// The response as the client gets it: a representation larger than
    /// the client may receive at once (§11.3, §4.6), or a block the
    /// client asked for, cut into Block2 blocks (RFC 7959 §2.4).
    fn for_client(
        &self,
        req: &CoapMessage,
        leg: &ClientLeg,
        resp: CoapMessage,
        authenticated: bool,
    ) -> CoapMessage {
        if resp.code != CoapCode::CONTENT || req.code != CoapCode::GET {
            return resp;
        }
        let limit = block_limit(authenticated, leg.request_len);
        let fits = resp.payload.len() <= limit.map_or(UNBLOCKED_PAYLOAD, |szx| 16usize << szx)
            && resp.payload.len() <= UNBLOCKED_PAYLOAD;
        if fits && extract_block(req, numbers::BLOCK2).is_none() {
            return resp;
        }
        let limit = Some(limit.unwrap_or(6));
        let cut = representation(req, resp.payload.clone(), limit);
        if cut.code != CoapCode::CONTENT {
            return cut;
        }
        let mut out = resp;
        out.payload = cut.payload;
        out.options
            .retain(|o| !matches!(o.number, numbers::ETAG | numbers::BLOCK2 | numbers::SIZE2));
        out.options.extend(cut.options);
        out
    }

    /// Sends the responses to the client: the first piggybacked unless
    /// the request is already acknowledged, then as a separate
    /// Confirmable response (§5.2.2); to a Non-confirmable request and
    /// every further response (§8.2.2) Non-confirmable.
    fn deliver(
        &self,
        req: &CoapMessage,
        leg: &ClientLeg,
        responses: Vec<CoapMessage>,
        acked: bool,
    ) {
        let peer = leg.reply.peer();
        let confirmable = matches!(req.message_type, MessageType::Confirmable);
        for (i, mut resp) in responses.into_iter().enumerate() {
            resp.token = req.token.clone();
            // RFC 7967 §2: the proxy interprets the client's No-Response
            // (Unsafe-to-Forward); a suppressed response does not leave, a
            // Confirmable request still gets its acknowledgement.
            if suppressed(req, resp.code) {
                if i == 0 && confirmable && !acked {
                    let ack = CoapMessage::new(
                        MessageType::Acknowledgement,
                        CoapCode::EMPTY,
                        req.message_id,
                    );
                    leg.reply.send(&ack);
                    self.remember_ack(peer, req.message_id, ack);
                }
                continue;
            }
            if i == 0 && confirmable && !acked {
                resp.message_type = MessageType::Acknowledgement;
                resp.message_id = req.message_id;
                leg.reply.send(&resp);
                self.remember_ack(peer, req.message_id, resp);
            } else if i == 0 && confirmable {
                resp.message_type = MessageType::Confirmable;
                resp.message_id = self.mid.fetch_add(1, Ordering::SeqCst);
                self.send_confirmable(&leg.reply, &resp);
            } else {
                resp.message_type = MessageType::NonConfirmable;
                resp.message_id = self.mid.fetch_add(1, Ordering::SeqCst);
                leg.reply.send(&resp);
            }
        }
    }

    fn remember_ack(&self, peer: SocketAddr, mid: u16, ack: CoapMessage) {
        if let Ok(mut exchanges) = self.exchanges.lock() {
            if let Some(e) = exchanges.get_mut(&(peer, mid)) {
                e.ack = Some(ack);
            }
        }
    }

    /// §4.2: a Confirmable message retransmitted with exponential back-off
    /// until it is acknowledged or reset.
    fn send_confirmable(&self, reply: &ReplyTo, msg: &CoapMessage) {
        let (tx, rx) = mpsc::channel();
        let key = (reply.peer(), msg.message_id);
        if let Ok(mut c) = self.confirmations.lock() {
            c.insert(key, tx);
        }
        let mut timeout = initial_timeout();
        for _ in 0..=MAX_RETRANSMIT {
            reply.send(msg);
            if rx.recv_timeout(timeout).is_ok() {
                return;
            }
            timeout *= 2;
        }
        if let Ok(mut c) = self.confirmations.lock() {
            c.remove(&key);
        }
    }

    /// The request to the destination: the Uri-Host when the host is no
    /// IP address (§6.4 step 5; the port is the destination's, so no
    /// Uri-Port), the path and query, the forwarded options.
    fn upstream_request(
        &self,
        job: &Job,
        message_type: MessageType,
        code: CoapCode,
        payload: Vec<u8>,
        extra: Vec<CoapOption>,
    ) -> CoapMessage {
        let mut msg = CoapMessage::new(message_type, code, random_u16());
        msg.token = random_token();
        let uri = &job.target.uri;
        if uri.host.parse::<IpAddr>().is_err() {
            msg.options.push(CoapOption::new(
                numbers::URI_HOST,
                OptionValue::String(uri.host.clone()),
            ));
        }
        for segment in &uri.path_segments {
            msg.options.push(CoapOption::uri_path(segment.clone()));
        }
        for arg in &uri.query_params {
            msg.options.push(CoapOption::new(
                numbers::URI_QUERY,
                OptionValue::String(arg.clone()),
            ));
        }
        msg.options.extend(job.target.options.iter().cloned());
        msg.options.extend(extra);
        msg.payload = payload;
        msg
    }

    /// The request with its body (in Block1 blocks when it is larger than
    /// one, RFC 7959 §2.5) and the whole response (Block2 blocks
    /// reassembled for a GET, §2.4).
    fn exchange_all(
        &self,
        job: &Job,
        up: &mut dyn Upstream,
        slow: &dyn Fn(),
        slow_at: Instant,
    ) -> Result<Vec<CoapMessage>, Failure> {
        let req = &job.req;
        let validate: Vec<CoapOption> = job
            .cache
            .as_ref()
            .and_then(|(_, etag)| etag.clone())
            .map(|e| vec![CoapOption::new(numbers::ETAG, OptionValue::Opaque(e))])
            .unwrap_or_default();
        let mut resp = if job.body.len() > UNBLOCKED_PAYLOAD {
            let mut szx = UPSTREAM_SZX;
            let mut start = 0usize;
            loop {
                let size = 16usize << szx;
                let end = (start + size).min(job.body.len());
                let more = end < job.body.len();
                let num = u32::try_from(start / size).unwrap_or(u32::MAX);
                let msg = self.upstream_request(
                    job,
                    MessageType::Confirmable,
                    req.code,
                    job.body[start..end].to_vec(),
                    vec![CoapOption::block1(num, more, szx)],
                );
                let r = up.exchange(&msg, slow, slow_at)?;
                if r.code == CoapCode::new(2, 31) && more {
                    // §2.5: the server may ask for smaller blocks.
                    if let Some(b) = extract_block(&r, numbers::BLOCK1) {
                        szx = szx.min(b.szx);
                    }
                    start = end;
                    continue;
                }
                break r;
            }
        } else {
            let msg = self.upstream_request(
                job,
                MessageType::Confirmable,
                req.code,
                job.body.clone(),
                validate,
            );
            up.exchange(&msg, slow, slow_at)?
        };
        if let Some(first) = extract_block(&resp, numbers::BLOCK2) {
            if req.code == CoapCode::GET && resp.code == CoapCode::CONTENT && first.more {
                let etag = option_bytes(&resp, numbers::ETAG);
                let mut body = std::mem::take(&mut resp.payload);
                let mut num = first.num + 1;
                loop {
                    let msg = self.upstream_request(
                        job,
                        MessageType::Confirmable,
                        CoapCode::GET,
                        Vec::new(),
                        vec![CoapOption::block2(num, false, first.szx)],
                    );
                    let next = up.exchange(&msg, slow, slow_at)?;
                    if next.code != CoapCode::CONTENT {
                        return Ok(vec![next]);
                    }
                    if option_bytes(&next, numbers::ETAG) != etag {
                        return Err(Failure::BadGateway(
                            "the representation changed between its blocks".into(),
                        ));
                    }
                    body.extend_from_slice(&next.payload);
                    if body.len() > MAX_REPRESENTATION {
                        return Err(Failure::BadGateway("representation too large".into()));
                    }
                    match extract_block(&next, numbers::BLOCK2) {
                        Some(b) if b.more => num = b.num + 1,
                        _ => break,
                    }
                }
                resp.payload = body;
                resp.options
                    .retain(|o| o.number != numbers::BLOCK2 && o.number != numbers::SIZE2);
            }
        }
        Ok(vec![resp])
    }

    fn fetch_udp(
        &self,
        job: &Job,
        slow: &dyn Fn(),
        slow_at: Instant,
        deadline: Instant,
    ) -> Result<Vec<CoapMessage>, Failure> {
        // The addresses of the host in turn while one refuses (a name
        // like "localhost" gives ::1 before 127.0.0.1).
        let mut refused = String::from("no address");
        for dest in resolve(&job.target.uri)? {
            let socket = UdpSocket::bind(unspecified(dest))
                .and_then(|s| s.connect(dest).map(|()| s))
                .map_err(|e| Failure::BadGateway(format!("socket: {e}")))?;
            let mut up = UdpUpstream {
                socket,
                dest,
                deadline,
            };
            match self.exchange_all(job, &mut up, slow, slow_at) {
                Err(Failure::Unreachable(e)) => refused = e,
                other => return other,
            }
        }
        Err(Failure::BadGateway(refused))
    }

    /// §8.2.2: the request once as Non-confirmable to the group, every
    /// response within the wait back to the client.
    fn fetch_multicast(
        &self,
        job: &Job,
        slow: &dyn Fn(),
        slow_at: Instant,
    ) -> Result<Vec<CoapMessage>, Failure> {
        let dest = resolve(&job.target.uri)?
            .into_iter()
            .next()
            .ok_or_else(|| Failure::BadGateway("no address".into()))?;
        let socket = UdpSocket::bind(unspecified(dest))
            .map_err(|e| Failure::BadGateway(format!("socket: {e}")))?;
        let msg = self.upstream_request(
            job,
            MessageType::NonConfirmable,
            job.req.code,
            job.body.clone(),
            Vec::new(),
        );
        let bytes = encode(&msg).map_err(|e| Failure::BadGateway(format!("encode: {e}")))?;
        socket
            .send_to(&bytes, dest)
            .map_err(|e| Failure::BadGateway(format!("send: {e}")))?;
        let until = Instant::now() + Duration::from_millis(self.settings.multicast_wait_ms);
        let mut responses = Vec::new();
        let mut buf = [0u8; 65535];
        let mut slowed = false;
        loop {
            let now = Instant::now();
            if !slowed && now >= slow_at {
                slow();
                slowed = true;
            }
            if now >= until {
                break;
            }
            let wait = until
                .min(if slowed { until } else { slow_at })
                .saturating_duration_since(now);
            let _ = socket.set_read_timeout(Some(wait.max(Duration::from_millis(1))));
            let Ok((n, from)) = socket.recv_from(&mut buf) else {
                continue;
            };
            let Ok(r) = decode(&buf[..n]) else {
                continue;
            };
            if r.token != msg.token || r.code.is_request() || r.code.is_empty() {
                continue;
            }
            if matches!(r.message_type, MessageType::Confirmable) {
                let ack =
                    CoapMessage::new(MessageType::Acknowledgement, CoapCode::EMPTY, r.message_id);
                if let Ok(b) = encode(&ack) {
                    let _ = socket.send_to(&b, from);
                }
            }
            responses.push(r);
        }
        if responses.is_empty() {
            Err(Failure::Timeout)
        } else {
            Ok(responses)
        }
    }

    #[cfg(feature = "dtls")]
    fn fetch_dtls(
        &self,
        job: &Job,
        slow: &dyn Fn(),
        slow_at: Instant,
        deadline: Instant,
    ) -> Result<Vec<CoapMessage>, Failure> {
        let addresses = resolve(&job.target.uri)?;
        let mut config = self
            .dtls
            .clone()
            .ok_or_else(|| Failure::BadGateway("no DTLS credentials".into()))?;
        // §9.1.3.3: the certificate names the host of the URI.
        config.server_name = Some(job.target.uri.host.clone());
        let mut session = None;
        let mut failed = String::from("no address");
        for (i, dest) in addresses.iter().enumerate() {
            // Every address but the last gets a few seconds, then the next
            // one is tried (a name like "localhost" gives ::1 first).
            let left = deadline.saturating_duration_since(Instant::now());
            let last = i + 1 == addresses.len();
            let budget = if last { left } else { left.min(DTLS_ATTEMPT) };
            if budget.is_zero() {
                return Err(Failure::Timeout);
            }
            let (tx, rx) = mpsc::channel();
            let attempt = std::thread::scope(|scope| {
                let config = &config;
                scope.spawn(move || {
                    let _ = tx.send(crate::dtls_transport::DtlsCoapClient::connect(
                        *dest, config, budget,
                    ));
                });
                // The handshake runs; §5.2.2 still acknowledges the client
                // when it takes a while.
                loop {
                    let wait = slow_at
                        .saturating_duration_since(Instant::now())
                        .clamp(Duration::from_millis(10), Duration::from_millis(100));
                    match rx.recv_timeout(wait) {
                        Ok(r) => break Some(r),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if Instant::now() >= slow_at {
                                slow();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break None,
                    }
                }
            });
            match attempt {
                Some(Ok(s)) => {
                    session = Some(s);
                    break;
                }
                Some(Err(crate::dtls_transport::DtlsError::Io(e)))
                    if e.kind() == std::io::ErrorKind::TimedOut && last =>
                {
                    return Err(Failure::Timeout);
                }
                Some(Err(e)) => failed = format!("DTLS to {dest}: {e}"),
                None => failed = format!("DTLS to {dest}: no handshake"),
            }
        }
        let Some(session) = session else {
            return Err(Failure::BadGateway(failed));
        };
        let mut up = DtlsUpstream { session, deadline };
        let out = self.exchange_all(job, &mut up, slow, slow_at);
        up.session.close();
        out
    }

    #[cfg(not(feature = "dtls"))]
    fn fetch_dtls(
        &self,
        _job: &Job,
        _slow: &dyn Fn(),
        _slow_at: Instant,
        _deadline: Instant,
    ) -> Result<Vec<CoapMessage>, Failure> {
        Err(Failure::BadGateway("this build has no DTLS".into()))
    }
}

/// One request and its response on the way to a destination.
trait Upstream {
    /// Sends `msg` (Confirmable, retransmitted per §4.2) and returns the
    /// response, piggybacked or separate (§5.2).
    fn exchange(
        &mut self,
        msg: &CoapMessage,
        slow: &dyn Fn(),
        slow_at: Instant,
    ) -> Result<CoapMessage, Failure>;
}

struct UdpUpstream {
    socket: UdpSocket,
    dest: SocketAddr,
    deadline: Instant,
}

impl Upstream for UdpUpstream {
    fn exchange(
        &mut self,
        msg: &CoapMessage,
        slow: &dyn Fn(),
        slow_at: Instant,
    ) -> Result<CoapMessage, Failure> {
        let bytes = encode(msg).map_err(|e| Failure::BadGateway(format!("encode: {e}")))?;
        let mut timeout = initial_timeout();
        let mut next_send = Instant::now();
        let mut sent = 0u32;
        let mut acknowledged = false;
        let mut buf = [0u8; 65535];
        loop {
            let now = Instant::now();
            if now >= slow_at {
                slow();
            }
            if now >= self.deadline {
                return Err(Failure::Timeout);
            }
            if !acknowledged && now >= next_send && sent <= MAX_RETRANSMIT {
                match self.socket.send(&bytes) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                        return Err(Failure::Unreachable(format!("{}: {e}", self.dest)));
                    }
                    Err(e) => return Err(Failure::BadGateway(format!("send: {e}"))),
                }
                sent += 1;
                next_send = now + timeout;
                timeout *= 2;
            }
            let mut wake = self.deadline;
            if !acknowledged && sent <= MAX_RETRANSMIT {
                wake = wake.min(next_send);
            }
            if now < slow_at {
                wake = wake.min(slow_at);
            }
            let wait = wake
                .saturating_duration_since(now)
                .max(Duration::from_millis(1));
            let _ = self.socket.set_read_timeout(Some(wait));
            let n = match self.socket.recv(&mut buf) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    return Err(Failure::Unreachable(format!("{}: {e}", self.dest)));
                }
                Err(_) => continue,
            };
            let data = &buf[..n];
            let r = match decode(data) {
                Ok(r) => r,
                Err(e) => {
                    // Ours when it carries our message ID (§5.7.1: "message
                    // format errors" are 5.02); else not for this exchange.
                    if data.len() >= 4 && u16::from_be_bytes([data[2], data[3]]) == msg.message_id {
                        return Err(Failure::BadGateway(format!("undecodable response: {e}")));
                    }
                    continue;
                }
            };
            match r.message_type {
                MessageType::Acknowledgement if r.message_id == msg.message_id => {
                    if r.code.is_empty() {
                        acknowledged = true;
                        continue;
                    }
                    if r.token == msg.token {
                        return Ok(r);
                    }
                    return Err(Failure::BadGateway("response with another token".into()));
                }
                MessageType::Reset if r.message_id == msg.message_id => {
                    return Err(Failure::BadGateway(
                        "the destination reset the request".into(),
                    ));
                }
                MessageType::Confirmable | MessageType::NonConfirmable
                    if r.token == msg.token && !r.code.is_request() && !r.code.is_empty() =>
                {
                    if matches!(r.message_type, MessageType::Confirmable) {
                        let ack = CoapMessage::new(
                            MessageType::Acknowledgement,
                            CoapCode::EMPTY,
                            r.message_id,
                        );
                        if let Ok(b) = encode(&ack) {
                            let _ = self.socket.send(&b);
                        }
                    }
                    return Ok(r);
                }
                MessageType::Confirmable => {
                    // §4.2: a Confirmable message the proxy cannot process
                    // is reset.
                    let rst = CoapMessage::new(MessageType::Reset, CoapCode::EMPTY, r.message_id);
                    if let Ok(b) = encode(&rst) {
                        let _ = self.socket.send(&b);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(feature = "dtls")]
struct DtlsUpstream {
    session: crate::dtls_transport::DtlsCoapSession,
    deadline: Instant,
}

#[cfg(feature = "dtls")]
impl Upstream for DtlsUpstream {
    fn exchange(
        &mut self,
        msg: &CoapMessage,
        slow: &dyn Fn(),
        slow_at: Instant,
    ) -> Result<CoapMessage, Failure> {
        let mut timeout = initial_timeout();
        let mut sent = 0u32;
        let mut acknowledged = false;
        let mut next_send = Instant::now();
        loop {
            let now = Instant::now();
            if now >= slow_at {
                slow();
            }
            if now >= self.deadline {
                return Err(Failure::Timeout);
            }
            if !acknowledged && now >= next_send && sent <= MAX_RETRANSMIT {
                self.session
                    .send_message(msg)
                    .map_err(|e| Failure::BadGateway(format!("DTLS send: {e}")))?;
                sent += 1;
                next_send = now + timeout;
                timeout *= 2;
            }
            let mut wake = self.deadline;
            if !acknowledged && sent <= MAX_RETRANSMIT {
                wake = wake.min(next_send);
            }
            if now < slow_at {
                wake = wake.min(slow_at);
            }
            let wait = wake
                .saturating_duration_since(now)
                .max(Duration::from_millis(1));
            let r = match self.session.recv_message(wait) {
                Ok(None) => continue,
                Ok(Some(r)) => r,
                Err(crate::dtls_transport::DtlsError::Codec(e)) => {
                    return Err(Failure::BadGateway(format!("undecodable response: {e}")));
                }
                Err(e) => return Err(Failure::BadGateway(format!("DTLS: {e}"))),
            };
            match r.message_type {
                MessageType::Acknowledgement if r.message_id == msg.message_id => {
                    if r.code.is_empty() {
                        acknowledged = true;
                        continue;
                    }
                    return Ok(r);
                }
                MessageType::Reset if r.message_id == msg.message_id => {
                    return Err(Failure::BadGateway(
                        "the destination reset the request".into(),
                    ));
                }
                MessageType::Confirmable | MessageType::NonConfirmable
                    if r.token == msg.token && !r.code.is_request() && !r.code.is_empty() =>
                {
                    if matches!(r.message_type, MessageType::Confirmable) {
                        let ack = CoapMessage::new(
                            MessageType::Acknowledgement,
                            CoapCode::EMPTY,
                            r.message_id,
                        );
                        let _ = self.session.send_message(&ack);
                    }
                    return Ok(r);
                }
                _ => {}
            }
        }
    }
}

/// The request as the daemon serves it when its Proxy-Uri or Proxy-Scheme
/// names the daemon itself: the path and query of the URI as Uri-Path and
/// Uri-Query, the proxy options gone (§5.7.2).
fn local_request(req: &CoapMessage, uri: &CoapUri) -> CoapMessage {
    let mut out = req.clone();
    out.options.retain(|o| {
        !matches!(
            o.number,
            numbers::PROXY_URI
                | numbers::PROXY_SCHEME
                | numbers::URI_HOST
                | numbers::URI_PORT
                | numbers::URI_PATH
                | numbers::URI_QUERY
        )
    });
    for segment in &uri.path_segments {
        out.options.push(CoapOption::uri_path(segment.clone()));
    }
    for arg in &uri.query_params {
        out.options.push(CoapOption::new(
            numbers::URI_QUERY,
            OptionValue::String(arg.clone()),
        ));
    }
    out
}

fn options(msg: &CoapMessage, number: u16) -> impl Iterator<Item = &CoapOption> {
    msg.options.iter().filter(move |o| o.number == number)
}

fn string_value(o: &CoapOption) -> Option<String> {
    match &o.value {
        OptionValue::String(s) => Some(s.clone()),
        OptionValue::Opaque(b) => String::from_utf8(b.clone()).ok(),
        OptionValue::Empty => Some(String::new()),
        OptionValue::Uint(_) => None,
    }
}

fn uint_value(o: &CoapOption) -> Option<u64> {
    match &o.value {
        OptionValue::Uint(v) => Some(*v),
        OptionValue::Opaque(b) => uint_from_bytes(b),
        OptionValue::Empty => Some(0),
        OptionValue::String(_) => None,
    }
}

fn option_bytes(msg: &CoapMessage, number: u16) -> Option<Vec<u8>> {
    options(msg, number).next().map(|o| o.value.to_wire_bytes())
}

/// The addresses of the destination of `uri`, in the order of the
/// resolver.
fn resolve(uri: &CoapUri) -> Result<Vec<SocketAddr>, Failure> {
    if let Ok(ip) = uri.host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, uri.port)]);
    }
    let all: Vec<SocketAddr> = (uri.host.as_str(), uri.port)
        .to_socket_addrs()
        .map_err(|e| Failure::BadGateway(format!("cannot resolve {}: {e}", uri.host)))?
        .collect();
    if all.is_empty() {
        return Err(Failure::BadGateway(format!("no address for {}", uri.host)));
    }
    Ok(all)
}

fn unspecified(dest: SocketAddr) -> SocketAddr {
    if dest.is_ipv4() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
    } else {
        SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
    }
}

/// §4.8: between ACK_TIMEOUT and ACK_TIMEOUT * ACK_RANDOM_FACTOR (1.5).
fn initial_timeout() -> Duration {
    let fraction = f64::from(random_u16()) / f64::from(u16::MAX);
    ACK_TIMEOUT.mul_f64(1.0 + 0.5 * fraction)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    if getrandom::getrandom(&mut out).is_err() {
        // No system randomness: the clock, which §5.3.1 does not forbid
        // over a protected leg and which is unique per request.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        for (i, b) in out.iter_mut().enumerate() {
            *b = (nanos >> (8 * (i % 16))) as u8;
        }
    }
    out
}

/// §5.3.1: a token with 64 random bits for every request of the proxy.
fn random_token() -> Vec<u8> {
    random_bytes::<8>().to_vec()
}

fn random_u16() -> u16 {
    u16::from_be_bytes(random_bytes::<2>())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn proxy(settings: ProxySettings) -> Proxy {
        let security = Arc::new(LiveSecurity::new(
            super::super::security::ctx_from_daemon_config(
                &super::super::config::DaemonConfig::default(),
            )
            .expect("security"),
        ));
        Proxy::new(
            settings,
            &["sensors/temp".to_string()],
            Arc::new(AtomicU16::new(1)),
            security,
            "127.0.0.1:5683".parse().expect("addr"),
            Some(5684),
        )
        .expect("proxy")
    }

    fn leg() -> ClientLeg {
        ClientLeg {
            reply: ReplyTo::Udp {
                socket: Arc::new(UdpSocket::bind("127.0.0.1:0").expect("socket")),
                peer: "127.0.0.1:40000".parse().expect("addr"),
            },
            destination: "127.0.0.1:5683".parse().expect("addr"),
            dtls_peer: None,
            request_len: 40,
        }
    }

    fn get(options: Vec<CoapOption>) -> CoapMessage {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 7);
        m.token = b"tk".to_vec();
        m.options = options;
        m
    }

    fn proxy_uri(uri: &str) -> CoapOption {
        CoapOption::new(numbers::PROXY_URI, OptionValue::String(uri.to_string()))
    }

    fn forwarding() -> ProxySettings {
        ProxySettings {
            forward: true,
            ..ProxySettings::default()
        }
    }

    #[test]
    fn coap_5_3_1_the_proxy_tokens_come_from_the_system_randomness() {
        // RFC 7252 §5.3.1 ("at least 32 bits of randomness") and §11.6 (no
        // weak source where good entropy is needed): 64 bits from the
        // operating system for every request of the proxy.
        let tokens: BTreeSet<Vec<u8>> = (0..1000).map(|_| random_token()).collect();
        assert_eq!(tokens.len(), 1000);
        assert!(tokens.iter().all(|t| t.len() == 8));
        let mut probe = [0u8; 8];
        assert!(getrandom::getrandom(&mut probe).is_ok());
    }

    #[test]
    fn coap_5_7_2_without_forwarding_a_proxy_request_is_5_05() {
        let p = proxy(ProxySettings::default());
        match p.route(&get(vec![proxy_uri("coap://192.0.2.1/x")]), &leg()) {
            Route::Reply(code, _) => assert_eq!(code, CoapCode::new(5, 5)),
            _ => panic!("expected 5.05"),
        }
        let p = proxy(forwarding());
        match p.route(&get(vec![proxy_uri("http://192.0.2.1/x")]), &leg()) {
            Route::Reply(code, _) => assert_eq!(code, CoapCode::new(5, 5)),
            _ => panic!("expected 5.05 for http"),
        }
    }

    #[test]
    fn coap_5_7_2_a_uri_naming_the_proxy_itself_is_a_local_request() {
        let p = proxy(ProxySettings::default());
        let req = get(vec![proxy_uri("coap://127.0.0.1:5683/sensors/temp?x=1")]);
        match p.route(&req, &leg()) {
            Route::Local(Some(local)) => {
                assert!(local.options.iter().all(|o| o.number != numbers::PROXY_URI));
                let path: Vec<String> = options(&local, numbers::URI_PATH)
                    .filter_map(string_value)
                    .collect();
                assert_eq!(path, vec!["sensors".to_string(), "temp".into()]);
            }
            _ => panic!("expected a local request"),
        }
        // Proxy-Scheme without Uri-Host addresses the request's destination.
        let req = get(vec![
            CoapOption::new(numbers::PROXY_SCHEME, OptionValue::String("coap".into())),
            CoapOption::uri_path("sensors"),
        ]);
        assert!(matches!(p.route(&req, &leg()), Route::Local(Some(_))));
    }

    #[test]
    fn coap_5_7_1_an_unrecognized_unsafe_option_is_4_02_and_a_safe_one_is_forwarded() {
        let p = proxy(forwarding());
        // 2050: elective, unsafe (bit 1 set).
        let unsafe_req = get(vec![
            proxy_uri("coap://192.0.2.1/x"),
            CoapOption::new(2050, OptionValue::Opaque(vec![1])),
        ]);
        match p.route(&unsafe_req, &leg()) {
            Route::Reply(code, _) => assert_eq!(code, CoapCode::new(4, 2)),
            _ => panic!("expected 4.02"),
        }
        // 2048: elective, safe-to-forward.
        let safe_req = get(vec![
            proxy_uri("coap://192.0.2.1/x"),
            CoapOption::new(2048, OptionValue::Opaque(vec![1])),
            CoapOption::new(
                COAP_OPTION_AUTH_TOKEN,
                OptionValue::String("Bearer t".into()),
            ),
        ]);
        match p.route(&safe_req, &leg()) {
            Route::Forward(t) => {
                assert!(t.options.iter().any(|o| o.number == 2048));
                assert!(
                    t.options
                        .iter()
                        .all(|o| o.number != numbers::PROXY_URI
                            && o.number != COAP_OPTION_AUTH_TOKEN)
                );
                assert_eq!(t.uri.path_segments, vec!["x".to_string()]);
            }
            _ => panic!("expected a forward"),
        }
    }

    #[test]
    fn rfc8768_3_the_hop_limit_is_decremented_inserted_and_reached() {
        let p = proxy(forwarding());
        let hop = |v: u64| CoapOption::new(numbers::HOP_LIMIT, OptionValue::Uint(v));
        let hop_of = |route: Route| match route {
            Route::Forward(t) => t
                .options
                .iter()
                .find(|o| o.number == numbers::HOP_LIMIT)
                .and_then(uint_value),
            _ => None,
        };
        assert_eq!(
            hop_of(p.route(&get(vec![proxy_uri("coap://192.0.2.1/x")]), &leg())),
            Some(16)
        );
        assert_eq!(
            hop_of(p.route(&get(vec![proxy_uri("coap://192.0.2.1/x"), hop(5)]), &leg())),
            Some(4)
        );
        match p.route(&get(vec![proxy_uri("coap://192.0.2.1/x"), hop(1)]), &leg()) {
            Route::Reply(code, diagnostic) => {
                assert_eq!(code, CoapCode::new(5, 8));
                assert!(!diagnostic.contains(' '));
            }
            _ => panic!("expected 5.08"),
        }
        match p.route(&get(vec![proxy_uri("coap://192.0.2.1/x"), hop(0)]), &leg()) {
            Route::Reply(code, _) => assert_eq!(code, CoapCode::BAD_REQUEST),
            _ => panic!("expected 4.00"),
        }
    }

    #[test]
    fn coap_5_7_3_a_reverse_route_maps_the_path_to_the_origin() {
        let p = proxy(ProxySettings {
            reverse: vec![("far".into(), "coap://192.0.2.7:5700/base".into())],
            ..ProxySettings::default()
        });
        let mut req = get(vec![
            CoapOption::uri_path("far"),
            CoapOption::uri_path("a b"),
        ]);
        req.options.push(CoapOption::new(
            numbers::URI_QUERY,
            OptionValue::String("q=1".into()),
        ));
        match p.route(&req, &leg()) {
            Route::Forward(t) => {
                assert_eq!(t.uri.to_uri(), "coap://192.0.2.7:5700/base/a%20b?q=1");
                assert!(t.reverse.is_some());
            }
            _ => panic!("expected the origin"),
        }
        assert!(matches!(
            p.route(&get(vec![CoapOption::uri_path("near")]), &leg()),
            Route::Local(None)
        ));
        // A reverse path may not cover a resource of the daemon.
        let security = Arc::new(LiveSecurity::new(
            super::super::security::ctx_from_daemon_config(
                &super::super::config::DaemonConfig::default(),
            )
            .expect("security"),
        ));
        assert!(
            Proxy::new(
                ProxySettings {
                    reverse: vec![("sensors".into(), "coap://192.0.2.7/".into())],
                    ..ProxySettings::default()
                },
                &["sensors/temp".to_string()],
                Arc::new(AtomicU16::new(1)),
                security,
                "127.0.0.1:5683".parse().expect("addr"),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn coap_11_2_a_coaps_response_is_cached_only_under_the_dtls_identity() {
        let p = proxy(forwarding());
        let req = get(vec![proxy_uri("coaps://192.0.2.1/x")]);
        let Route::Forward(target) = p.route(&req, &leg()) else {
            panic!("expected a forward");
        };
        // Without a DTLS identity nothing is kept.
        assert!(p.cache_key(&req, &target, &leg(), false).is_none());
        let mut dtls = leg();
        dtls.dtls_peer = Some(AuthSubject::new("psk:alice"));
        let key = p.cache_key(&req, &target, &dtls, false).expect("key");
        assert_eq!(key.scope, CacheScope::Private("psk:alice".into()));
        let plain = get(vec![proxy_uri("coap://192.0.2.1/x")]);
        let Route::Forward(target) = p.route(&plain, &leg()) else {
            panic!("expected a forward");
        };
        let key = p.cache_key(&plain, &target, &leg(), false).expect("key");
        assert_eq!(key.scope, CacheScope::Shared);
        // A multicast destination or an OSCORE request is not cached.
        assert!(p.cache_key(&plain, &target, &leg(), true).is_none());
    }
}
