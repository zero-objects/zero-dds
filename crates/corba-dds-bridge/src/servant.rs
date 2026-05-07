// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridge-Servant — verarbeitet GIOP-Requests und konvertiert sie
//! in DDS-Sample-Publish-Operationen.
//!
//! Der `BridgeServant` ist `Servant`-kompatibel (`crates/corba-poa/`)
//! und kann in einem POA als Default-Servant oder per
//! `activate_object_with_id` registriert werden.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use std::sync::Mutex;

use zerodds_corba_poa::Servant;

use crate::mapping::{BridgeMapping, BridgeRoute, OperationMapping};

/// Aufruf-Trait, den der Bridge-Servant fuer die DDS-Seite einbindet.
/// Caller implementiert das gegen `crates/dcps/`.
pub trait DdsPublishSink: Send + Sync {
    /// Publiziert ein Sample auf einem Topic.
    fn publish(&self, topic: &str, sample_bytes: &[u8]);
    /// Wartet auf den naechsten Reply-Sample auf dem Reply-Topic.
    /// Liefert `None` bei Timeout.
    fn await_reply(&self, topic: &str, request_id: u64) -> Option<Vec<u8>>;
}

/// Bridge-Servant.
pub struct BridgeServant {
    /// Repository-ID des bedienten Object-Types.
    repository_id: String,
    /// Object-Key des bedienten Servants.
    object_key: Vec<u8>,
    /// Bridge-Mapping (geshared).
    mapping: alloc::sync::Arc<Mutex<BridgeMapping>>,
    /// DDS-Sink fuer Publish + Reply-Wait.
    sink: alloc::sync::Arc<dyn DdsPublishSink>,
    /// Monoton wachsender Request-ID-Counter.
    next_request_id: AtomicU64,
}

impl core::fmt::Debug for BridgeServant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BridgeServant")
            .field("repository_id", &self.repository_id)
            .field("object_key_len", &self.object_key.len())
            .finish()
    }
}

impl BridgeServant {
    /// Konstruktor.
    #[must_use]
    pub fn new(
        repository_id: String,
        object_key: Vec<u8>,
        mapping: alloc::sync::Arc<Mutex<BridgeMapping>>,
        sink: alloc::sync::Arc<dyn DdsPublishSink>,
    ) -> Self {
        Self {
            repository_id,
            object_key,
            mapping,
            sink,
            next_request_id: AtomicU64::new(1),
        }
    }

    /// Liefert die Operation-Mapping-Tabelle aus dem geshared
    /// Mapping (Helper fuer Tests + Diagnose).
    fn lookup_operation_mapping(&self, operation: &str) -> Option<OperationMapping> {
        let m = self.mapping.lock().ok()?;
        let route: &BridgeRoute = m.lookup(&self.repository_id, &self.object_key)?;
        route.operation(operation).cloned()
    }
}

impl Servant for BridgeServant {
    fn primary_interface(&self) -> String {
        self.repository_id.clone()
    }

    fn invoke(&self, operation: &str, request_body: &[u8]) -> Vec<u8> {
        let Some(mapping) = self.lookup_operation_mapping(operation) else {
            // Spec §11.3: BAD_OPERATION wird vom Caller-POA als Reply
            // mit SystemException kodiert. Hier nur leere Reply.
            return Vec::new();
        };
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        // Sample-Bytes = request_id (8 Bytes BE) + body. Caller-Layer
        // dekodiert das Topic-Type-Schema.
        let mut sample = Vec::with_capacity(8 + request_body.len());
        sample.extend_from_slice(&request_id.to_be_bytes());
        sample.extend_from_slice(request_body);
        self.sink.publish(&mapping.request_topic, &sample);
        // Wenn Reply-Topic leer, keine Reply (fire-and-forget).
        if mapping.reply_topic.is_empty() {
            return Vec::new();
        }
        self.sink
            .await_reply(&mapping.reply_topic, request_id)
            .unwrap_or_default()
    }
}

/// Hilfs-Konstruktor: registriert eine `BridgeServant`-Box.
#[must_use]
pub fn boxed(servant: BridgeServant) -> Box<dyn Servant> {
    Box::new(servant)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::mapping::{BridgeRoute, Direction, OperationMapping, TopicQosRef};
    use alloc::sync::Arc;
    use core::sync::atomic::AtomicUsize;

    struct TestSink {
        published_topic: Mutex<String>,
        published_bytes: Mutex<Vec<u8>>,
        reply_called: AtomicUsize,
        reply_bytes: Vec<u8>,
    }

    impl DdsPublishSink for TestSink {
        fn publish(&self, topic: &str, sample: &[u8]) {
            if let Ok(mut t) = self.published_topic.lock() {
                *t = topic.to_string();
            }
            if let Ok(mut b) = self.published_bytes.lock() {
                *b = sample.to_vec();
            }
        }
        fn await_reply(&self, _topic: &str, _request_id: u64) -> Option<Vec<u8>> {
            self.reply_called
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            Some(self.reply_bytes.clone())
        }
    }

    fn build() -> (BridgeServant, Arc<TestSink>) {
        let mut mapping = BridgeMapping::new();
        mapping.add_route(BridgeRoute {
            repository_id: "IDL:demo/Echo:1.0".into(),
            object_key: alloc::vec![0xab],
            direction: Direction::Bidirectional,
            operations: alloc::vec![OperationMapping {
                operation: "ping".into(),
                request_topic: "Echo/ping/Req".into(),
                reply_topic: "Echo/ping/Rep".into(),
                qos: TopicQosRef::default(),
            }],
        });
        let sink = Arc::new(TestSink {
            published_topic: Mutex::new(String::new()),
            published_bytes: Mutex::new(Vec::new()),
            reply_called: AtomicUsize::new(0),
            reply_bytes: alloc::vec![1, 2, 3, 4],
        });
        let servant = BridgeServant::new(
            "IDL:demo/Echo:1.0".into(),
            alloc::vec![0xab],
            Arc::new(Mutex::new(mapping)),
            sink.clone(),
        );
        (servant, sink)
    }

    #[test]
    fn invoke_publishes_sample_with_request_id_prefix() {
        let (s, sink) = build();
        let _ = s.invoke("ping", &[0xff, 0xee]);
        let topic = sink.published_topic.lock().unwrap().clone();
        assert_eq!(topic, "Echo/ping/Req");
        let bytes = sink.published_bytes.lock().unwrap().clone();
        assert_eq!(bytes.len(), 8 + 2);
        // Request-ID = 1 (start), BE.
        assert_eq!(&bytes[0..8], &1u64.to_be_bytes());
        // Body danach.
        assert_eq!(&bytes[8..], &[0xff, 0xee]);
    }

    #[test]
    fn invoke_returns_reply_bytes() {
        let (s, sink) = build();
        let r = s.invoke("ping", &[]);
        assert_eq!(r, alloc::vec![1, 2, 3, 4]);
        assert!(
            sink.reply_called
                .load(core::sync::atomic::Ordering::Relaxed)
                >= 1
        );
    }

    #[test]
    fn fire_and_forget_returns_empty_without_calling_await() {
        let mut mapping = BridgeMapping::new();
        mapping.add_route(BridgeRoute {
            repository_id: "IDL:demo/Echo:1.0".into(),
            object_key: alloc::vec![0xab],
            direction: Direction::CorbaToDds,
            operations: alloc::vec![OperationMapping {
                operation: "log".into(),
                request_topic: "Echo/log/Req".into(),
                reply_topic: "".into(), // fire-and-forget
                qos: TopicQosRef::default(),
            }],
        });
        let sink = Arc::new(TestSink {
            published_topic: Mutex::new(String::new()),
            published_bytes: Mutex::new(Vec::new()),
            reply_called: AtomicUsize::new(0),
            reply_bytes: alloc::vec![],
        });
        let s = BridgeServant::new(
            "IDL:demo/Echo:1.0".into(),
            alloc::vec![0xab],
            Arc::new(Mutex::new(mapping)),
            sink.clone(),
        );
        let r = s.invoke("log", &[7, 8]);
        assert!(r.is_empty());
        assert_eq!(
            sink.reply_called
                .load(core::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn unknown_operation_returns_empty() {
        let (s, _sink) = build();
        let r = s.invoke("unknown", &[]);
        assert!(r.is_empty());
    }

    #[test]
    fn primary_interface_matches_repository_id() {
        let (s, _) = build();
        assert_eq!(s.primary_interface(), "IDL:demo/Echo:1.0");
    }
}
