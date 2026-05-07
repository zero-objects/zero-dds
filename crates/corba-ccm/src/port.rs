// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Port-Registry — Receptacle/Facet/Event-Connections zur Laufzeit.
//!
//! Spec §8.1.4: jede Component-Instance hat eine Connection-Tabelle,
//! die Ports an konkrete Object-References bzw. Event-Streams bindet.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use std::sync::Mutex;

/// Connection-Id — vom Container vergeben (Spec §8.1.4.5).
pub type ConnectionId = u64;

/// Event-Stream-Identifier — verbindet Source mit Sink ueber einen
/// Push/Pull-Channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventStream {
    /// Stream-Name (typisch Event-Type-Repository-ID).
    pub name: String,
    /// Eindeutige Connection-Id.
    pub connection_id: ConnectionId,
}

#[derive(Debug, Default)]
struct PortRegistryInner {
    receptacle_connections: BTreeMap<(String, String), Vec<ConnectionId>>,
    event_streams: BTreeMap<String, Vec<EventStream>>,
    ior_by_id: BTreeMap<ConnectionId, Vec<u8>>,
}

/// Port-Registry pro Component-Instance.
#[derive(Debug, Default)]
pub struct PortRegistry {
    inner: Mutex<PortRegistryInner>,
    seq: AtomicU64,
}

impl PortRegistry {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(PortRegistryInner::default()),
            seq: AtomicU64::new(1),
        }
    }

    /// `connect` — Spec §8.1.4.5. Bindet einen Receptacle an einen
    /// Object-Reference (als opaque IOR-Bytes). Liefert die
    /// `ConnectionId`.
    pub fn connect_receptacle(
        &self,
        instance_id: &str,
        receptacle: &str,
        ior_bytes: Vec<u8>,
    ) -> ConnectionId {
        let id = self.seq.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut inner) = self.inner.lock() {
            inner
                .receptacle_connections
                .entry((instance_id.to_string(), receptacle.to_string()))
                .or_default()
                .push(id);
            inner.ior_by_id.insert(id, ior_bytes);
        }
        id
    }

    /// `disconnect` — Spec §8.1.4.6.
    pub fn disconnect_receptacle(
        &self,
        instance_id: &str,
        receptacle: &str,
        connection_id: ConnectionId,
    ) -> bool {
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        let key = (instance_id.to_string(), receptacle.to_string());
        let removed = if let Some(list) = inner.receptacle_connections.get_mut(&key) {
            let before = list.len();
            list.retain(|i| *i != connection_id);
            before != list.len()
        } else {
            false
        };
        if removed {
            inner.ior_by_id.remove(&connection_id);
        }
        removed
    }

    /// `get_connections` — Liste von ConnectionIds + IOR-Bytes.
    #[must_use]
    pub fn get_connections(
        &self,
        instance_id: &str,
        receptacle: &str,
    ) -> Vec<(ConnectionId, Vec<u8>)> {
        let inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let ids = inner
            .receptacle_connections
            .get(&(instance_id.to_string(), receptacle.to_string()))
            .cloned()
            .unwrap_or_default();
        ids.iter()
            .filter_map(|id| inner.ior_by_id.get(id).map(|b| (*id, b.clone())))
            .collect()
    }

    /// `subscribe` — Event-Sink-zu-Source-Bindung.
    pub fn subscribe_event_sink(&self, source: &str, sink_name: String) -> ConnectionId {
        let id = self.seq.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut inner) = self.inner.lock() {
            inner
                .event_streams
                .entry(source.to_string())
                .or_default()
                .push(EventStream {
                    name: sink_name,
                    connection_id: id,
                });
        }
        id
    }

    /// Liste aller Event-Sinks, die an einer Event-Source haengen.
    #[must_use]
    pub fn list_event_sinks(&self, source: &str) -> Vec<EventStream> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.event_streams.get(source).cloned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn connect_and_get_connections() {
        let r = PortRegistry::new();
        let id1 = r.connect_receptacle("inst-1", "market_feed", alloc::vec![1, 2]);
        let id2 = r.connect_receptacle("inst-1", "market_feed", alloc::vec![3, 4]);
        let cons = r.get_connections("inst-1", "market_feed");
        assert_eq!(cons.len(), 2);
        assert!(
            cons.iter()
                .any(|(i, b)| *i == id1 && b == &alloc::vec![1, 2])
        );
        assert!(
            cons.iter()
                .any(|(i, b)| *i == id2 && b == &alloc::vec![3, 4])
        );
    }

    #[test]
    fn disconnect_removes_only_target_id() {
        let r = PortRegistry::new();
        let id1 = r.connect_receptacle("inst-1", "p", alloc::vec![1]);
        let _id2 = r.connect_receptacle("inst-1", "p", alloc::vec![2]);
        assert!(r.disconnect_receptacle("inst-1", "p", id1));
        assert_eq!(r.get_connections("inst-1", "p").len(), 1);
    }

    #[test]
    fn disconnect_unknown_returns_false() {
        let r = PortRegistry::new();
        assert!(!r.disconnect_receptacle("inst-1", "p", 999));
    }

    #[test]
    fn event_sink_subscription_round_trip() {
        let r = PortRegistry::new();
        let _ = r.subscribe_event_sink("Trade", "alarm_sink".into());
        let _ = r.subscribe_event_sink("Trade", "metrics_sink".into());
        let sinks = r.list_event_sinks("Trade");
        assert_eq!(sinks.len(), 2);
    }

    #[test]
    fn list_event_sinks_unknown_source_empty() {
        let r = PortRegistry::new();
        assert!(r.list_event_sinks("nonexistent").is_empty());
    }
}
