// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP caching + proxying per RFC 7252 §2.3 / §5.6 / §5.7 +
//! HTTP cross-protocol mapping per RFC 7252 §10.
//!
//! Here we provide the configuration layer + state tracking. The
//! actual HTTP translation is on the caller side (e.g. via
//! `crates/grpc-bridge/src/server.rs` as an HTTP adapter).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Mutex;
use std::time::Instant;

use crate::message::CoapMessage;

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
        if let Ok(mut g) = self.entries.lock() {
            if g.len() >= self.max_entries {
                // FIFO eviction when the cap is reached.
                if let Some(first) = g.keys().next().cloned() {
                    g.remove(&first);
                }
            }
            g.insert(
                key,
                CacheEntry {
                    response,
                    inserted: Instant::now(),
                    max_age,
                    etag,
                },
            );
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
    use crate::message::{CoapCode, MessageType};

    fn sample_response() -> CoapMessage {
        CoapMessage::new(MessageType::Acknowledgement, CoapCode::CONTENT, 1)
    }

    // §5.6 Cache
    #[test]
    fn empty_cache_returns_miss() {
        let c = CoapCache::new(10);
        assert_eq!(c.lookup(b"key"), CacheLookup::Miss);
        assert!(c.is_empty());
    }

    #[test]
    fn store_then_fresh_lookup() {
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
    fn stale_with_etag_yields_validation_path() {
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
    fn stale_without_etag_yields_miss() {
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
    fn cap_evicts_oldest() {
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
    }

    #[test]
    fn evict_expired_without_etag_keeps_revalidatable() {
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
    fn proxy_config_default_is_forward_coap_to_coap() {
        let c = ProxyConfig::default();
        assert_eq!(c.mode, ProxyMode::Forward);
        assert!(c.coap_to_coap);
        assert!(!c.http_translation);
        assert_eq!(c.max_hops, 16);
    }

    // §10.1 HTTP-Status-Mapping
    #[test]
    fn http_200_maps_to_coap_2_05() {
        assert_eq!(http_status_to_coap(200), Some((2, 5)));
    }

    #[test]
    fn http_404_maps_to_coap_4_04() {
        assert_eq!(http_status_to_coap(404), Some((4, 4)));
    }

    #[test]
    fn http_500_maps_to_coap_5_00() {
        assert_eq!(http_status_to_coap(500), Some((5, 0)));
    }

    #[test]
    fn http_unknown_returns_none() {
        assert!(http_status_to_coap(999).is_none());
    }

    #[test]
    fn coap_to_http_round_trip() {
        for h in [200u16, 201, 204, 400, 401, 403, 404, 405, 412, 500, 503] {
            let (c, d) = http_status_to_coap(h).expect("ok");
            assert_eq!(coap_to_http_status(c, d), Some(h));
        }
    }

    // §10.2 HTTP-Method-Mapping
    #[test]
    fn http_get_maps_to_coap_1() {
        assert_eq!(http_method_to_coap("GET"), Some(1));
        assert_eq!(http_method_to_coap("get"), Some(1));
    }

    #[test]
    fn http_unknown_method_returns_none() {
        assert!(http_method_to_coap("HEAD").is_none());
        assert!(http_method_to_coap("PATCH").is_none());
    }

    #[test]
    fn coap_method_round_trip() {
        for m in ["GET", "POST", "PUT", "DELETE"] {
            let d = http_method_to_coap(m).expect("ok");
            assert_eq!(coap_method_to_http(d), Some(m));
        }
    }
}
