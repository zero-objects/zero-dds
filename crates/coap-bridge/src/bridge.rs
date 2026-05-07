// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP↔DDS-Topic-Bridge.
//!
//! Mappt CoAP-Resources auf DDS-Topics:
//!
//! * `GET /dds/<topic>` — liefert Discovery-Liste (Instances).
//! * `GET /dds/<topic>/<instance-key>` — read-Sample.
//! * `POST /dds/<topic>` — register-instance + write-Sample.
//! * `PUT /dds/<topic>/<instance-key>` — update-Sample (write).
//! * `DELETE /dds/<topic>/<instance-key>` — dispose.
//! * `Observe = 0` auf `/dds/<topic>/<instance-key>` — Subscribe; jede
//!   Notification entspricht einem DataReader-Sample.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::message::CoapCode;
use crate::observe::ObserveRegistry;

/// Bridge-Operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeOp {
    /// `GET /dds/<topic>` — Discovery.
    DiscoverInstances {
        /// Topic-Name.
        topic: String,
    },
    /// `GET /dds/<topic>/<key>` — Read.
    Read {
        /// Topic-Name.
        topic: String,
        /// Instance-Key (BLOB).
        key: Vec<u8>,
    },
    /// `POST /dds/<topic>` — Write (register-instance + write).
    Write {
        /// Topic-Name.
        topic: String,
        /// Sample-Payload (CDR-encoded).
        payload: Vec<u8>,
    },
    /// `PUT /dds/<topic>/<key>` — Update.
    Update {
        /// Topic-Name.
        topic: String,
        /// Instance-Key.
        key: Vec<u8>,
        /// Sample-Payload.
        payload: Vec<u8>,
    },
    /// `DELETE /dds/<topic>/<key>` — Dispose.
    Dispose {
        /// Topic-Name.
        topic: String,
        /// Instance-Key.
        key: Vec<u8>,
    },
    /// Observe-Register (Spec RFC 7641 §3.1).
    ObserveRegister {
        /// Topic.
        topic: String,
        /// Instance-Key.
        key: Vec<u8>,
        /// Token.
        token: Vec<u8>,
    },
    /// Observe-Deregister.
    ObserveDeregister {
        /// Topic.
        topic: String,
        /// Token.
        token: Vec<u8>,
    },
}

/// Bridge-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// URI ist nicht im erwarteten Schema `/dds/<topic>[/<key>]`.
    BadPath(String),
    /// Operation nicht erlaubt fuer den HTTP-aequivalenten Code.
    UnsupportedMethod(CoapCode),
}

impl core::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadPath(s) => write!(f, "bad path: {s}"),
            Self::UnsupportedMethod(c) => write!(f, "unsupported method: {c:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

/// Parst einen Resource-Path zu (topic, optional key).
///
/// # Errors
/// `BadPath` wenn der Pfad nicht mit `dds/` beginnt oder mehr als 2
/// Komponenten hat.
pub fn parse_dds_path(path: &str) -> Result<(String, Option<Vec<u8>>), BridgeError> {
    let stripped = path.trim_start_matches('/');
    let mut parts = stripped.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("dds"), Some(topic), None) if !topic.is_empty() => Ok((topic.to_string(), None)),
        (Some("dds"), Some(topic), Some(key)) if !topic.is_empty() => {
            // Spec wir akzeptieren key als raw UTF-8-Bytes (Caller kann
            // hex-prefix verwenden); fuer reine Hex-Keys liefern wir
            // automatisches Decoding.
            let bytes = if let Some(rest) = key.strip_prefix("0x") {
                hex_decode(rest).unwrap_or_else(|_| key.as_bytes().to_vec())
            } else {
                key.as_bytes().to_vec()
            };
            Ok((topic.to_string(), Some(bytes)))
        }
        _ => Err(BridgeError::BadPath(path.into())),
    }
}

/// Mappt einen CoAP-Method-Code + Path zu einer Bridge-Operation.
///
/// # Errors
/// `BadPath` / `UnsupportedMethod`.
pub fn map_method(
    code: CoapCode,
    path: &str,
    payload: Vec<u8>,
    observe_action: Option<u8>,
    token: Option<Vec<u8>>,
) -> Result<BridgeOp, BridgeError> {
    let (topic, key) = parse_dds_path(path)?;

    // Observe-Action bei GET (Spec RFC 7641 §2): 0 = register, 1 = deregister.
    if let (CoapCode::GET, Some(action)) = (code, observe_action) {
        let token = token.unwrap_or_default();
        if action == 0 {
            return Ok(BridgeOp::ObserveRegister {
                topic,
                key: key.unwrap_or_default(),
                token,
            });
        }
        if action == 1 {
            return Ok(BridgeOp::ObserveDeregister { topic, token });
        }
    }
    match (code, key) {
        (CoapCode::GET, None) => Ok(BridgeOp::DiscoverInstances { topic }),
        (CoapCode::GET, Some(key)) => Ok(BridgeOp::Read { topic, key }),
        (CoapCode::POST, _) => Ok(BridgeOp::Write { topic, payload }),
        (CoapCode::PUT, Some(key)) => Ok(BridgeOp::Update {
            topic,
            key,
            payload,
        }),
        (CoapCode::DELETE, Some(key)) => Ok(BridgeOp::Dispose { topic, key }),
        (c, _) => Err(BridgeError::UnsupportedMethod(c)),
    }
}

/// Bridge-State — verkapselt Observe-Registry + In-Memory-Sample-Store
/// (Production-Caller plugt einen echten DataWriter/Reader ein).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoapDdsBridge {
    samples: BTreeMap<(String, Vec<u8>), Vec<u8>>,
    observers: ObserveRegistry,
}

impl CoapDdsBridge {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Aktueller Sample-Count.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Observer-Anzahl.
    #[must_use]
    pub fn observer_count(&self) -> usize {
        self.observers.observer_count()
    }

    /// Observer-Registry-Reference (fuer Notification-Loop).
    #[must_use]
    pub fn observers(&self) -> &ObserveRegistry {
        &self.observers
    }

    /// Wende eine Bridge-Op an.
    pub fn apply(&mut self, op: BridgeOp, endpoint: Vec<u8>) -> Vec<u8> {
        match op {
            BridgeOp::DiscoverInstances { topic } => {
                let instances: Vec<&Vec<u8>> = self
                    .samples
                    .keys()
                    .filter(|(t, _)| t == &topic)
                    .map(|(_, k)| k)
                    .collect();
                let mut out = Vec::new();
                for k in instances {
                    out.extend_from_slice(k);
                    out.push(b'\n');
                }
                out
            }
            BridgeOp::Read { topic, key } => {
                self.samples.get(&(topic, key)).cloned().unwrap_or_default()
            }
            BridgeOp::Write { topic, payload } => {
                // Use payload-prefix as auto-key when no explicit key given.
                let key = payload.iter().take(8).copied().collect::<Vec<u8>>();
                let _ = self
                    .samples
                    .insert((topic.clone(), key.clone()), payload.clone());
                let _ = self.observers.next_seq(&topic);
                payload
            }
            BridgeOp::Update {
                topic,
                key,
                payload,
            } => {
                let _ = self
                    .samples
                    .insert((topic.clone(), key.clone()), payload.clone());
                let _ = self.observers.next_seq(&topic);
                payload
            }
            BridgeOp::Dispose { topic, key } => {
                self.samples.remove(&(topic.clone(), key));
                let _ = self.observers.next_seq(&topic);
                Vec::new()
            }
            BridgeOp::ObserveRegister {
                topic,
                key: _,
                token,
            } => {
                self.observers.register(topic, token, endpoint);
                Vec::new()
            }
            BridgeOp::ObserveDeregister { topic, token } => {
                self.observers.deregister(&topic, &token);
                Vec::new()
            }
        }
    }
}

fn hex_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    if s.len() % 2 != 0 {
        return Err("odd hex length");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_value(bytes[i])?;
        let lo = hex_value(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_value(c: u8) -> Result<u8, &'static str> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err("not hex"),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_topic_only() {
        let (t, k) = parse_dds_path("/dds/Trade").unwrap();
        assert_eq!(t, "Trade");
        assert!(k.is_none());
    }

    #[test]
    fn parses_topic_with_key() {
        let (t, k) = parse_dds_path("/dds/Trade/AAPL").unwrap();
        assert_eq!(t, "Trade");
        assert_eq!(k.unwrap(), b"AAPL");
    }

    #[test]
    fn parses_hex_key() {
        let (_, k) = parse_dds_path("/dds/Trade/0xCAFE").unwrap();
        assert_eq!(k.unwrap(), alloc::vec![0xca, 0xfe]);
    }

    #[test]
    fn rejects_non_dds_path() {
        assert!(parse_dds_path("/other/x").is_err());
        assert!(parse_dds_path("/dds/").is_err());
    }

    #[test]
    fn map_method_get_topic_is_discover() {
        let op = map_method(CoapCode::GET, "/dds/T", alloc::vec![], None, None).unwrap();
        assert!(matches!(op, BridgeOp::DiscoverInstances { .. }));
    }

    #[test]
    fn map_method_get_with_key_is_read() {
        let op = map_method(CoapCode::GET, "/dds/T/k", alloc::vec![], None, None).unwrap();
        assert!(matches!(op, BridgeOp::Read { .. }));
    }

    #[test]
    fn map_method_post_is_write() {
        let op = map_method(CoapCode::POST, "/dds/T", alloc::vec![1, 2, 3], None, None).unwrap();
        assert!(matches!(op, BridgeOp::Write { .. }));
    }

    #[test]
    fn map_method_put_is_update() {
        let op = map_method(CoapCode::PUT, "/dds/T/k", alloc::vec![1], None, None).unwrap();
        assert!(matches!(op, BridgeOp::Update { .. }));
    }

    #[test]
    fn map_method_delete_is_dispose() {
        let op = map_method(CoapCode::DELETE, "/dds/T/k", alloc::vec![], None, None).unwrap();
        assert!(matches!(op, BridgeOp::Dispose { .. }));
    }

    #[test]
    fn map_method_get_observe_register() {
        let op = map_method(
            CoapCode::GET,
            "/dds/T/k",
            alloc::vec![],
            Some(0),
            Some(alloc::vec![1, 2]),
        )
        .unwrap();
        assert!(matches!(op, BridgeOp::ObserveRegister { .. }));
    }

    #[test]
    fn map_method_get_observe_deregister() {
        let op = map_method(
            CoapCode::GET,
            "/dds/T/k",
            alloc::vec![],
            Some(1),
            Some(alloc::vec![1]),
        )
        .unwrap();
        assert!(matches!(op, BridgeOp::ObserveDeregister { .. }));
    }

    #[test]
    fn bridge_apply_write_then_read_round_trip() {
        let mut b = CoapDdsBridge::new();
        let _ = b.apply(
            BridgeOp::Update {
                topic: "T".into(),
                key: alloc::vec![1],
                payload: alloc::vec![10, 20],
            },
            alloc::vec![],
        );
        let r = b.apply(
            BridgeOp::Read {
                topic: "T".into(),
                key: alloc::vec![1],
            },
            alloc::vec![],
        );
        assert_eq!(r, alloc::vec![10, 20]);
    }

    #[test]
    fn bridge_dispose_removes_sample() {
        let mut b = CoapDdsBridge::new();
        let _ = b.apply(
            BridgeOp::Update {
                topic: "T".into(),
                key: alloc::vec![1],
                payload: alloc::vec![1],
            },
            alloc::vec![],
        );
        assert_eq!(b.sample_count(), 1);
        let _ = b.apply(
            BridgeOp::Dispose {
                topic: "T".into(),
                key: alloc::vec![1],
            },
            alloc::vec![],
        );
        assert_eq!(b.sample_count(), 0);
    }

    #[test]
    fn bridge_observe_register_increments_observer_count() {
        let mut b = CoapDdsBridge::new();
        let _ = b.apply(
            BridgeOp::ObserveRegister {
                topic: "T".into(),
                key: alloc::vec![],
                token: alloc::vec![1],
            },
            alloc::vec![],
        );
        assert_eq!(b.observer_count(), 1);
    }

    #[test]
    fn bridge_observe_deregister_decrements() {
        let mut b = CoapDdsBridge::new();
        let _ = b.apply(
            BridgeOp::ObserveRegister {
                topic: "T".into(),
                key: alloc::vec![],
                token: alloc::vec![1],
            },
            alloc::vec![],
        );
        let _ = b.apply(
            BridgeOp::ObserveDeregister {
                topic: "T".into(),
                token: alloc::vec![1],
            },
            alloc::vec![],
        );
        assert_eq!(b.observer_count(), 0);
    }

    #[test]
    fn bridge_discover_lists_keys() {
        let mut b = CoapDdsBridge::new();
        let _ = b.apply(
            BridgeOp::Update {
                topic: "T".into(),
                key: alloc::vec![1],
                payload: alloc::vec![1],
            },
            alloc::vec![],
        );
        let r = b.apply(
            BridgeOp::DiscoverInstances { topic: "T".into() },
            alloc::vec![],
        );
        assert!(!r.is_empty());
    }
}
