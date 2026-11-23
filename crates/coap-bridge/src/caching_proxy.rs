// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP caching + proxying per RFC 7252 §2.3 / §5.6 / §5.7 +
//! HTTP cross-protocol mapping per RFC 7252 §10.
//!
//! Here we provide the configuration layer + state tracking. The
//! actual HTTP translation is on the caller side (e.g. via
//! `crates/grpc-bridge/src/server.rs` as an HTTP adapter).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Mutex;
use std::time::Instant;

use crate::message::{CoapCode, CoapMessage};
use crate::option::{CoapOption, OptionValue, numbers, uint_from_bytes};

// ---------------------------------------------------------------------------
// §5.6 Caching — Freshness + Validation
// ---------------------------------------------------------------------------

/// Spec §5.6.1 — Default Max-Age = 60 seconds.
pub const DEFAULT_MAX_AGE: Duration = Duration::from_secs(60);

/// Cache entry: response + freshness lifetime + ETag.
struct CacheEntry {
    response: CoapMessage,
    inserted: Instant,
    max_age: Duration,
    etag: Option<Vec<u8>>,
    /// The request URI the response belongs to, for the invalidation of
    /// §5.9.1.1, §5.9.1.2 and §5.9.1.4; empty for an entry stored by key.
    uri: String,
}

/// Who may be served a stored response (RFC 7252 §11.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CacheScope {
    /// The response to a `coap` request: every client the proxy forwards
    /// such a request for.
    Shared,
    /// The response to a `coaps` request, which is "never public" (§11.2):
    /// only the client that proved this identity in its DTLS handshake,
    /// the same access decision that led to the entry.
    Private(String),
}

/// The key a response is stored under (RFC 7252 §5.6, §5.7.1): the
/// scope, the method, the request URI and the request options that are
/// part of the Cache-Key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    /// See [`CacheScope`].
    pub scope: CacheScope,
    /// The method code.
    pub method: u8,
    /// The request URI (§6.5), standing for the Uri-Host, Uri-Port,
    /// Uri-Path and Uri-Query options.
    pub uri: String,
    /// The other request options that are part of the Cache-Key (§5.4.6:
    /// not NoCacheKey), as number and value, in the order of the request.
    pub options: Vec<(u16, Vec<u8>)>,
}

impl CacheKey {
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.scope {
            CacheScope::Shared => out.push(0),
            CacheScope::Private(identity) => {
                out.push(1);
                push_field(&mut out, identity.as_bytes());
            }
        }
        out.push(self.method);
        push_field(&mut out, self.uri.as_bytes());
        for (number, value) in &self.options {
            out.extend_from_slice(&number.to_be_bytes());
            push_field(&mut out, value);
        }
        out
    }
}

fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// The Max-Age of a response (§5.10.5: 60 seconds when absent).
#[must_use]
pub fn max_age_of(response: &CoapMessage) -> Duration {
    response
        .options
        .iter()
        .find(|o| o.number == numbers::MAX_AGE)
        .and_then(|o| match &o.value {
            OptionValue::Uint(v) => Some(*v),
            OptionValue::Opaque(b) => uint_from_bytes(b),
            _ => None,
        })
        .map_or(DEFAULT_MAX_AGE, Duration::from_secs)
}

fn etag_of(response: &CoapMessage) -> Option<Vec<u8>> {
    response
        .options
        .iter()
        .find(|o| o.number == numbers::ETAG)
        .map(|o| o.value.to_wire_bytes())
}

/// `response` with a Max-Age option of `remaining` whole seconds in place
/// of the one it had.
fn with_max_age(mut response: CoapMessage, remaining: Duration) -> CoapMessage {
    response.options.retain(|o| o.number != numbers::MAX_AGE);
    response.options.push(CoapOption::new(
        numbers::MAX_AGE,
        OptionValue::Uint(remaining.as_secs()),
    ));
    response
}

impl core::fmt::Debug for CacheEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CacheEntry")
            .field("max_age_secs", &self.max_age.as_secs())
            .field("has_etag", &self.etag.is_some())
            .finish()
    }
}

/// CoAP response cache per RFC 7252 §5.6.
#[derive(Default)]
pub struct CoapCache {
    /// Cache key = (method code, full request path) → CacheEntry.
    entries: Mutex<BTreeMap<Vec<u8>, CacheEntry>>,
    /// Spec §5.6.x — DoS cap.
    pub max_entries: usize,
}

impl core::fmt::Debug for CoapCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.entries.lock().map_or(0, |g| g.len());
        f.debug_struct("CoapCache")
            .field("count", &n)
            .field("max_entries", &self.max_entries)
            .finish()
    }
}

/// Cache lookup result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLookup {
    /// No entry.
    Miss,
    /// Fresh entry — usable directly as a response.
    Fresh(CoapMessage),
    /// Entry present but stale — validation via ETag required.
    Stale {
        /// Cached response (can be reused if the server returns 2.03
        /// Valid).
        response: CoapMessage,
        /// ETag for the If-None-Match header in the validation request.
        etag: Vec<u8>,
    },
}

impl CoapCache {
    /// Constructor with DoS cap.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
            max_entries,
        }
    }

    /// Spec §5.6.1 — Stash a Response.
    pub fn store(
        &self,
        key: Vec<u8>,
        response: CoapMessage,
        max_age: Duration,
        etag: Option<Vec<u8>>,
    ) {
        self.insert(key, response, max_age, etag, String::new());
    }

    fn insert(
        &self,
        key: Vec<u8>,
        response: CoapMessage,
        max_age: Duration,
        etag: Option<Vec<u8>>,
        uri: String,
    ) {
        if self.max_entries == 0 {
            return;
        }
        if let Ok(mut g) = self.entries.lock() {
            if g.len() >= self.max_entries && !g.contains_key(&key) {
                // The entry stored longest ago makes room.
                if let Some(oldest) = g
                    .iter()
                    .min_by_key(|(_, e)| e.inserted)
                    .map(|(k, _)| k.clone())
                {
                    g.remove(&oldest);
                }
            }
            g.insert(
                key,
                CacheEntry {
                    response,
                    inserted: Instant::now(),
                    max_age,
                    etag,
                    uri,
                },
            );
        }
    }

    /// Stores the response to the request of `key` when it may be reused
    /// (§5.6): a `2.05 Content` that is fresh for a while (Max-Age, 60
    /// seconds without one) or can be validated (ETag). The token, the
    /// message ID and the Max-Age are not kept; [`Self::serve`] sets the
    /// Max-Age that is left. `false` when nothing is stored.
    pub fn store_response(&self, key: &CacheKey, response: &CoapMessage) -> bool {
        if response.code != CoapCode::CONTENT {
            return false;
        }
        let max_age = max_age_of(response);
        let etag = etag_of(response);
        if max_age.is_zero() && etag.is_none() {
            return false;
        }
        let mut stored = response.clone();
        stored.token.clear();
        stored.message_id = 0;
        stored.options.retain(|o| o.number != numbers::MAX_AGE);
        self.insert(key.bytes(), stored, max_age, etag, key.uri.clone());
        true
    }

    /// The stored response for the request of `key` (§5.6.1, §5.7.1): a
    /// fresh one with the Max-Age reduced by the time it spent in the
    /// cache ("MUST NOT extend the max-age originally set by the
    /// server"), a stale one with its ETag for validation (§5.6.2), or
    /// nothing. An entry of another [`CacheScope`] is never served.
    pub fn serve(&self, key: &CacheKey) -> CacheLookup {
        let g = match self.entries.lock() {
            Ok(g) => g,
            Err(_) => return CacheLookup::Miss,
        };
        let Some(entry) = g.get(&key.bytes()) else {
            return CacheLookup::Miss;
        };
        let age = entry.inserted.elapsed();
        if age < entry.max_age {
            CacheLookup::Fresh(with_max_age(
                entry.response.clone(),
                entry.max_age.saturating_sub(age),
            ))
        } else if let Some(etag) = entry.etag.clone() {
            CacheLookup::Stale {
                response: entry.response.clone(),
                etag,
            }
        } else {
            CacheLookup::Miss
        }
    }

    /// A `2.03 Valid` for the stored response of `key` (§5.9.1.3): the
    /// entry is fresh again for the Max-Age of `valid` ("the cache MUST
    /// update the stored response with the value of the Max-Age Option
    /// included in the response") and is returned with it. `None` when
    /// there is no entry or `valid` names another ETag.
    pub fn revalidated(&self, key: &CacheKey, valid: &CoapMessage) -> Option<CoapMessage> {
        let mut g = self.entries.lock().ok()?;
        let entry = g.get_mut(&key.bytes())?;
        if entry.etag.is_none() || etag_of(valid) != entry.etag {
            return None;
        }
        entry.inserted = Instant::now();
        entry.max_age = max_age_of(valid);
        Some(with_max_age(entry.response.clone(), entry.max_age))
    }

    /// Marks every stored response for `uri` as not fresh, whatever its
    /// scope and options (§5.9.1.1 2.01, §5.9.1.2 2.02, §5.9.1.4 2.04: "a
    /// cache MUST mark any stored response for the [...] resource as not
    /// fresh"). An entry with an ETag can still be validated.
    pub fn invalidate(&self, uri: &str) {
        if let Ok(mut g) = self.entries.lock() {
            for entry in g.values_mut().filter(|e| e.uri == uri) {
                entry.max_age = Duration::ZERO;
            }
        }
    }

    /// Spec §5.6.1 / §5.6.2 — Lookup with freshness check.
    pub fn lookup(&self, key: &[u8]) -> CacheLookup {
        let g = match self.entries.lock() {
            Ok(g) => g,
            Err(_) => return CacheLookup::Miss,
        };
        let Some(entry) = g.get(key) else {
            return CacheLookup::Miss;
        };
        if entry.inserted.elapsed() < entry.max_age {
            CacheLookup::Fresh(entry.response.clone())
        } else if let Some(etag) = entry.etag.clone() {
            CacheLookup::Stale {
                response: entry.response.clone(),
                etag,
            }
        } else {
            // Stale without ETag → cannot validate, treat as Miss.
            CacheLookup::Miss
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |g| g.len())
    }

    /// `true` when the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Spec §5.6.x — Evict expired (without ETag for revalidation).
    pub fn evict_expired_without_etag(&self) -> usize {
        if let Ok(mut g) = self.entries.lock() {
            let before = g.len();
            g.retain(|_, e| e.inserted.elapsed() < e.max_age || e.etag.is_some());
            before - g.len()
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// §5.7 Proxying — Forward-Proxy-Configuration
// ---------------------------------------------------------------------------

/// Spec §5.7.2 — Proxy mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    /// `Forward-Proxy` — client references the proxy via the `Proxy-Uri` option.
    Forward,
    /// `Reverse-Proxy` — the proxy is transparent to the client.
    Reverse,
}

/// Spec §5.7.x — Proxy configuration.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Mode (Forward/Reverse).
    pub mode: ProxyMode,
    /// `true` when the proxy forwards CoAP↔CoAP.
    pub coap_to_coap: bool,
    /// `true` when the proxy translates CoAP↔HTTP (Spec §10).
    pub http_translation: bool,
    /// Maximum hop count (Spec §5.7.1: protection against forwarding loops).
    pub max_hops: u8,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            mode: ProxyMode::Forward,
            coap_to_coap: true,
            http_translation: false,
            // Spec §5.7.1 — recommended ≤ 16 hops.
            max_hops: 16,
        }
    }
}

// ---------------------------------------------------------------------------
// §10 HTTP cross-protocol mapping
// ---------------------------------------------------------------------------

/// Spec §10.1 — Map HTTP status codes onto CoAP codes.
///
/// Returns the CoAP code tuple `(class, detail)` for HTTP status codes
/// per §10.1 Table 8.
#[must_use]
pub fn http_status_to_coap(http_status: u16) -> Option<(u8, u8)> {
    match http_status {
        200 => Some((2, 5)),  // 2.05 Content (GET)
        201 => Some((2, 1)),  // 2.01 Created
        204 => Some((2, 4)),  // 2.04 Changed (PUT/POST/DELETE)
        400 => Some((4, 0)),  // 4.00 Bad Request
        401 => Some((4, 1)),  // 4.01 Unauthorized
        403 => Some((4, 3)),  // 4.03 Forbidden
        404 => Some((4, 4)),  // 4.04 Not Found
        405 => Some((4, 5)),  // 4.05 Method Not Allowed
        406 => Some((4, 6)),  // 4.06 Not Acceptable
        412 => Some((4, 12)), // 4.12 Precondition Failed
        413 => Some((4, 13)), // 4.13 Request Entity Too Large
        415 => Some((4, 15)), // 4.15 Unsupported Content-Format
        500 => Some((5, 0)),  // 5.00 Internal Server Error
        501 => Some((5, 1)),  // 5.01 Not Implemented
        502 => Some((5, 2)),  // 5.02 Bad Gateway
        503 => Some((5, 3)),  // 5.03 Service Unavailable
        504 => Some((5, 4)),  // 5.04 Gateway Timeout
        _ => None,
    }
}

/// Spec §10.1 — Inverse mapping (CoAP code to HTTP status).
#[must_use]
pub fn coap_to_http_status(class: u8, detail: u8) -> Option<u16> {
    match (class, detail) {
        (2, 1) => Some(201),
        (2, 4) => Some(204),
        (2, 5) => Some(200),
        (4, 0) => Some(400),
        (4, 1) => Some(401),
        (4, 3) => Some(403),
        (4, 4) => Some(404),
        (4, 5) => Some(405),
        (4, 6) => Some(406),
        (4, 12) => Some(412),
        (4, 13) => Some(413),
        (4, 15) => Some(415),
        (5, 0) => Some(500),
        (5, 1) => Some(501),
        (5, 2) => Some(502),
        (5, 3) => Some(503),
        (5, 4) => Some(504),
        _ => None,
    }
}

/// Spec §10.2 — HTTP method to CoAP method.
///
/// Returns `(class=0, detail)` for GET/POST/PUT/DELETE; `None` for
/// HTTP-specific methods (HEAD/OPTIONS/PATCH/etc.) — these are
/// rejected by the proxy.
#[must_use]
pub fn http_method_to_coap(method: &str) -> Option<u8> {
    match method.to_uppercase().as_str() {
        "GET" => Some(1),
        "POST" => Some(2),
        "PUT" => Some(3),
        "DELETE" => Some(4),
        _ => None,
    }
}

/// Spec §10.2 — Inverse mapping (CoAP method to HTTP).
#[must_use]
pub fn coap_method_to_http(method_detail: u8) -> Option<&'static str> {
    match method_detail {
        1 => Some("GET"),
        2 => Some("POST"),
        3 => Some("PUT"),
        4 => Some("DELETE"),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::message::MessageType;

    fn sample_response() -> CoapMessage {
        CoapMessage::new(MessageType::Acknowledgement, CoapCode::CONTENT, 1)
    }

    // §5.6 Cache
    #[test]
    fn coap_2_3_empty_cache_returns_miss() {
        let c = CoapCache::new(10);
        assert_eq!(c.lookup(b"key"), CacheLookup::Miss);
        assert!(c.is_empty());
    }

    #[test]
    fn coap_2_3_store_then_fresh_lookup() {
        let c = CoapCache::new(10);
        c.store(
            b"key".to_vec(),
            sample_response(),
            Duration::from_secs(60),
            None,
        );
        assert_eq!(c.len(), 1);
        assert!(matches!(c.lookup(b"key"), CacheLookup::Fresh(_)));
    }

    #[test]
    fn coap_2_3_stale_with_etag_yields_validation_path() {
        let c = CoapCache::new(10);
        c.store(
            b"key".to_vec(),
            sample_response(),
            Duration::from_millis(1),
            Some(b"etag-1".to_vec()),
        );
        std::thread::sleep(Duration::from_millis(20));
        match c.lookup(b"key") {
            CacheLookup::Stale { etag, .. } => assert_eq!(etag, b"etag-1"),
            other => panic!("expected Stale, got {other:?}"),
        }
    }

    #[test]
    fn coap_2_3_stale_without_etag_yields_miss() {
        let c = CoapCache::new(10);
        c.store(
            b"key".to_vec(),
            sample_response(),
            Duration::from_millis(1),
            None,
        );
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(c.lookup(b"key"), CacheLookup::Miss);
    }

    #[test]
    fn coap_2_3_cap_evicts_oldest() {
        let c = CoapCache::new(2);
        c.store(
            b"a".to_vec(),
            sample_response(),
            Duration::from_secs(60),
            None,
        );
        c.store(
            b"b".to_vec(),
            sample_response(),
            Duration::from_secs(60),
            None,
        );
        c.store(
            b"c".to_vec(),
            sample_response(),
            Duration::from_secs(60),
            None,
        );
        assert_eq!(c.len(), 2);
        // The entry stored longest ago made room.
        assert_eq!(c.lookup(b"a"), CacheLookup::Miss);
        assert!(matches!(c.lookup(b"c"), CacheLookup::Fresh(_)));
    }

    fn content(max_age: Option<u64>, etag: Option<&[u8]>) -> CoapMessage {
        let mut m = sample_response();
        m.token = b"tk".to_vec();
        m.payload = b"22.5".to_vec();
        if let Some(v) = max_age {
            m.options
                .push(CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(v)));
        }
        if let Some(e) = etag {
            m.options.push(CoapOption::new(
                numbers::ETAG,
                OptionValue::Opaque(e.to_vec()),
            ));
        }
        m
    }

    fn key(scope: CacheScope, uri: &str) -> CacheKey {
        CacheKey {
            scope,
            method: 1,
            uri: uri.to_string(),
            options: Vec::new(),
        }
    }

    fn served_max_age(lookup: &CacheLookup) -> Option<u64> {
        match lookup {
            CacheLookup::Fresh(m) => m
                .options
                .iter()
                .find(|o| o.number == numbers::MAX_AGE)
                .and_then(|o| match o.value {
                    OptionValue::Uint(v) => Some(v),
                    _ => None,
                }),
            _ => None,
        }
    }

    #[test]
    fn coap_5_7_1_a_cached_response_never_extends_the_max_age() {
        // RFC 7252 §5.7.1: "proxy-max-age = original-max-age - cache-age".
        let c = CoapCache::new(10);
        let k = key(CacheScope::Shared, "coap://o/t");
        assert!(c.store_response(&k, &content(Some(2), None)));
        assert_eq!(served_max_age(&c.serve(&k)), Some(1));
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(served_max_age(&c.serve(&k)), Some(0));
        std::thread::sleep(Duration::from_millis(1000));
        assert_eq!(c.serve(&k), CacheLookup::Miss);
        // Max-Age 0 without an ETag is nothing to keep; the default is 60.
        assert!(!c.store_response(&k, &content(Some(0), None)));
        let d = key(CacheScope::Shared, "coap://o/d");
        assert!(c.store_response(&d, &content(None, None)));
        assert_eq!(served_max_age(&c.serve(&d)), Some(59));
        // The stored response keeps no token of the request it answered.
        match c.serve(&d) {
            CacheLookup::Fresh(m) => assert!(m.token.is_empty()),
            other => panic!("expected Fresh, got {other:?}"),
        }
    }

    #[test]
    fn coap_11_2_a_coaps_response_is_served_only_to_the_same_identity() {
        // §11.2: responses to "coaps" requests "are never "public" and thus
        // MUST NOT be reused for shared caching"; cached values are not
        // available to requests with lesser transport security.
        let c = CoapCache::new(10);
        let alice = key(CacheScope::Private("mtls:alice".into()), "coaps://o/t");
        assert!(c.store_response(&alice, &content(Some(60), None)));
        assert!(matches!(c.serve(&alice), CacheLookup::Fresh(_)));
        let bob = key(CacheScope::Private("mtls:bob".into()), "coaps://o/t");
        assert_eq!(c.serve(&bob), CacheLookup::Miss);
        let plain = key(CacheScope::Shared, "coaps://o/t");
        assert_eq!(c.serve(&plain), CacheLookup::Miss);
    }

    #[test]
    fn coap_5_9_1_3_a_valid_response_refreshes_the_entry() {
        let c = CoapCache::new(10);
        let k = key(CacheScope::Shared, "coap://o/t");
        assert!(c.store_response(&k, &content(Some(0), Some(b"e1"))));
        assert!(matches!(c.serve(&k), CacheLookup::Stale { .. }));
        let mut valid = CoapMessage::new(MessageType::Acknowledgement, CoapCode::VALID, 9);
        valid.options.push(CoapOption::new(
            numbers::ETAG,
            OptionValue::Opaque(b"e2".to_vec()),
        ));
        assert!(c.revalidated(&k, &valid).is_none());
        valid.options = vec![
            CoapOption::new(numbers::ETAG, OptionValue::Opaque(b"e1".to_vec())),
            CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(30)),
        ];
        let again = c.revalidated(&k, &valid).expect("refreshed");
        assert_eq!(again.payload, b"22.5");
        assert!(served_max_age(&c.serve(&k)).is_some_and(|s| (29..=30).contains(&s)));
    }

    #[test]
    fn coap_5_9_1_a_change_marks_the_stored_responses_not_fresh() {
        // §5.9.1.4: on 2.04 Changed "a cache MUST mark any stored response
        // for the changed resource as not fresh".
        let c = CoapCache::new(10);
        let k = key(CacheScope::Shared, "coap://o/t");
        let p = key(CacheScope::Private("psk:x".into()), "coap://o/t");
        let other = key(CacheScope::Shared, "coap://o/u");
        for k in [&k, &p, &other] {
            assert!(c.store_response(k, &content(Some(60), None)));
        }
        c.invalidate("coap://o/t");
        assert_eq!(c.serve(&k), CacheLookup::Miss);
        assert_eq!(c.serve(&p), CacheLookup::Miss);
        assert!(matches!(c.serve(&other), CacheLookup::Fresh(_)));
    }

    #[test]
    fn coap_2_3_evict_expired_without_etag_keeps_revalidatable() {
        let c = CoapCache::new(10);
        c.store(
            b"a".to_vec(),
            sample_response(),
            Duration::from_millis(1),
            None,
        );
        c.store(
            b"b".to_vec(),
            sample_response(),
            Duration::from_millis(1),
            Some(b"e".to_vec()),
        );
        std::thread::sleep(Duration::from_millis(20));
        let evicted = c.evict_expired_without_etag();
        assert_eq!(evicted, 1);
        assert_eq!(c.len(), 1);
    }

    // §5.7 Proxy
    #[test]
    fn coap_2_3_proxy_config_default_is_forward_coap_to_coap() {
        let c = ProxyConfig::default();
        assert_eq!(c.mode, ProxyMode::Forward);
        assert!(c.coap_to_coap);
        assert!(!c.http_translation);
        assert_eq!(c.max_hops, 16);
    }

    // §10.1 HTTP-Status-Mapping
    #[test]
    fn coap_10_http_200_maps_to_coap_2_05() {
        assert_eq!(http_status_to_coap(200), Some((2, 5)));
    }

    #[test]
    fn coap_10_http_404_maps_to_coap_4_04() {
        assert_eq!(http_status_to_coap(404), Some((4, 4)));
    }

    #[test]
    fn coap_10_http_500_maps_to_coap_5_00() {
        assert_eq!(http_status_to_coap(500), Some((5, 0)));
    }

    #[test]
    fn coap_10_http_unknown_returns_none() {
        assert!(http_status_to_coap(999).is_none());
    }

    #[test]
    fn coap_10_the_status_codes_map_both_ways() {
        // RFC 7252 §12.1.2 names the CoAP codes after the HTTP ones.
        for (h, c) in [
            (200u16, (2u8, 5u8)),
            (201, (2, 1)),
            (204, (2, 4)),
            (400, (4, 0)),
            (401, (4, 1)),
            (403, (4, 3)),
            (404, (4, 4)),
            (405, (4, 5)),
            (412, (4, 12)),
            (500, (5, 0)),
            (503, (5, 3)),
        ] {
            assert_eq!(http_status_to_coap(h), Some(c));
            assert_eq!(coap_to_http_status(c.0, c.1), Some(h));
        }
    }

    // §10.2 HTTP-Method-Mapping
    #[test]
    fn coap_10_http_get_maps_to_coap_1() {
        assert_eq!(http_method_to_coap("GET"), Some(1));
        assert_eq!(http_method_to_coap("get"), Some(1));
    }

    #[test]
    fn coap_10_http_unknown_method_returns_none() {
        assert!(http_method_to_coap("HEAD").is_none());
        assert!(http_method_to_coap("PATCH").is_none());
    }

    #[test]
    fn coap_10_the_methods_map_both_ways() {
        // RFC 7252 §12.1.1: GET 0.01, POST 0.02, PUT 0.03, DELETE 0.04.
        for (m, d) in [("GET", 1u8), ("POST", 2), ("PUT", 3), ("DELETE", 4)] {
            assert_eq!(http_method_to_coap(m), Some(d));
            assert_eq!(coap_method_to_http(d), Some(m));
        }
    }
}
