// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosEvent → DDS-DCPS topic bridge (ZeroDDS extension, no OMG item).
//!
//! A [`TopicBridge`] is a `CosEventComm::PushConsumer` that translates
//! every event it receives from a `CosEventChannelAdmin::EventChannel`
//! into one DDS-DCPS topic write. It contributes the two halves the
//! translation needs:
//!
//! * the **topic resolution** — an explicit repository-ID ⇒ topic-name
//!   table, with a deterministic derivation from the repository ID for
//!   every type that is not listed;
//! * the **sample handover** — the [`TopicSink`] trait, implemented by
//!   the caller layer over a real `DataWriter` (the daemon in
//!   `crates/corba-dds-bridge/`). The sink keeps this crate `no_std`
//!   and free of a DCPS dependency.
//!
//! No event is discarded quietly: a repository ID that cannot be
//! resolved and a sink that refuses a sample both produce an error from
//! [`TopicBridge::forward`] and raise the [`TopicBridge::rejected`]
//! counter.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alloc::collections::BTreeMap;

use crate::comm::{AnyEvent, Disconnected, PushConsumer};

/// Reason a bridged event did not reach a DDS topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// The event's repository ID is neither mapped nor derivable, so
    /// there is no topic to write to.
    UnresolvedTopic(String),
    /// The DDS side refused the sample.
    SinkRejected(String),
    /// The bridge is disconnected from its event channel.
    Disconnected,
}

/// DDS side of the bridge: one call per sample.
pub trait TopicSink: Send + Sync {
    /// Writes one sample onto `topic`.
    ///
    /// # Errors
    /// A message describing why the DDS side refused the sample.
    fn write(&self, topic: &str, payload: &[u8]) -> Result<(), String>;
}

/// Derives a DDS topic name from a CORBA repository ID.
///
/// `IDL:demo/Tick:1.0` becomes `demo::Tick`: the `IDL:` prefix and the
/// `:<version>` suffix are dropped, `/` becomes the IDL scope
/// separator. Returns `None` for anything that is not an `IDL:` type
/// identifier, because guessing a topic name there would route samples
/// to an arbitrary topic.
#[must_use]
pub fn derive_topic_name(repository_id: &str) -> Option<String> {
    let body = repository_id.strip_prefix("IDL:")?;
    let scoped = match body.rfind(':') {
        Some(idx) => &body[..idx],
        None => body,
    };
    if scoped.is_empty() {
        return None;
    }
    Some(scoped.replace('/', "::"))
}

/// Repository-ID ⇒ DDS topic name table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicMap {
    entries: BTreeMap<String, String>,
}

impl TopicMap {
    /// Empty table — every type resolves by derivation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps a repository ID onto an explicit topic name.
    pub fn map(&mut self, repository_id: impl Into<String>, topic: impl Into<String>) {
        self.entries.insert(repository_id.into(), topic.into());
    }

    /// The explicit entry for a repository ID, if there is one.
    #[must_use]
    pub fn explicit(&self, repository_id: &str) -> Option<&str> {
        self.entries.get(repository_id).map(String::as_str)
    }

    /// The topic a repository ID resolves to: the explicit entry if
    /// present, otherwise the derived name.
    #[must_use]
    pub fn resolve(&self, repository_id: &str) -> Option<String> {
        match self.explicit(repository_id) {
            Some(t) => Some(t.to_string()),
            None => derive_topic_name(repository_id),
        }
    }

    /// Number of explicit entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if no explicit entry is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `PushConsumer` that writes every received event onto a DDS topic.
pub struct TopicBridge {
    map: TopicMap,
    sink: Arc<dyn TopicSink>,
    connected: AtomicBool,
    forwarded: AtomicUsize,
    rejected: AtomicUsize,
}

impl core::fmt::Debug for TopicBridge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TopicBridge")
            .field("topics", &self.map.len())
            .field("forwarded", &self.forwarded())
            .field("rejected", &self.rejected())
            .finish()
    }
}

impl TopicBridge {
    /// Builds a bridge over a topic table and a DDS sink.
    #[must_use]
    pub fn new(map: TopicMap, sink: Arc<dyn TopicSink>) -> Self {
        Self {
            map,
            sink,
            connected: AtomicBool::new(true),
            forwarded: AtomicUsize::new(0),
            rejected: AtomicUsize::new(0),
        }
    }

    /// Translates one event and hands the sample to the DDS sink.
    ///
    /// # Errors
    /// [`BridgeError::Disconnected`] after `disconnect_push_consumer`,
    /// [`BridgeError::UnresolvedTopic`] when the repository ID names no
    /// topic, [`BridgeError::SinkRejected`] when the DDS side refuses.
    pub fn forward(&self, event: &AnyEvent) -> Result<String, BridgeError> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(BridgeError::Disconnected);
        }
        let Some(topic) = self.map.resolve(&event.type_id) else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(BridgeError::UnresolvedTopic(event.type_id.clone()));
        };
        match self.sink.write(&topic, &event.data) {
            Ok(()) => {
                self.forwarded.fetch_add(1, Ordering::Relaxed);
                Ok(topic)
            }
            Err(reason) => {
                self.rejected.fetch_add(1, Ordering::Relaxed);
                Err(BridgeError::SinkRejected(reason))
            }
        }
    }

    /// Number of samples handed to the DDS sink.
    #[must_use]
    pub fn forwarded(&self) -> usize {
        self.forwarded.load(Ordering::Relaxed)
    }

    /// Number of events that did not reach a topic.
    #[must_use]
    pub fn rejected(&self) -> usize {
        self.rejected.load(Ordering::Relaxed)
    }
}

impl PushConsumer for TopicBridge {
    /// The channel-facing `push`. Its IDL signature only carries
    /// `Disconnected`, so the precise reason stays in
    /// [`TopicBridge::forward`] and in the [`TopicBridge::rejected`]
    /// counter; an unresolved or refused event is an error here as
    /// well, never a silent success.
    fn push(&self, event: AnyEvent) -> Result<(), Disconnected> {
        self.forward(&event).map(|_| ()).map_err(|_| Disconnected)
    }

    fn disconnect_push_consumer(&self) {
        self.connected.store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::channel::EventChannel;
    use alloc::vec::Vec;
    use std::sync::Mutex;

    /// Records every sample the bridge writes; can be switched into a
    /// refusing state to model a DDS write failure.
    #[derive(Default)]
    struct RecordingSink {
        writes: Mutex<Vec<(String, Vec<u8>)>>,
        refuse: AtomicBool,
    }

    impl RecordingSink {
        fn writes(&self) -> Vec<(String, Vec<u8>)> {
            self.writes.lock().map(|g| g.clone()).unwrap_or_default()
        }
    }

    impl TopicSink for RecordingSink {
        fn write(&self, topic: &str, payload: &[u8]) -> Result<(), String> {
            if self.refuse.load(Ordering::Acquire) {
                return Err("writer not matched".to_string());
            }
            if let Ok(mut g) = self.writes.lock() {
                g.push((topic.to_string(), payload.to_vec()));
            }
            Ok(())
        }
    }

    fn event(type_id: &str, payload: &[u8]) -> AnyEvent {
        AnyEvent::new(type_id.to_string(), payload.to_vec())
    }

    #[test]
    fn derives_topic_name_from_repository_id() {
        assert_eq!(
            derive_topic_name("IDL:demo/Tick:1.0").as_deref(),
            Some("demo::Tick")
        );
        assert_eq!(derive_topic_name("IDL:Tick:1.0").as_deref(), Some("Tick"));
        assert_eq!(
            derive_topic_name("IDL:a/b/C:2.3").as_deref(),
            Some("a::b::C")
        );
        // Not an IDL repository ID — nothing to derive.
        assert!(derive_topic_name("Tick").is_none());
        assert!(derive_topic_name("IDL:").is_none());
        assert!(derive_topic_name("").is_none());
    }

    #[test]
    fn explicit_mapping_wins_over_derivation() {
        let mut map = TopicMap::new();
        assert!(map.is_empty());
        map.map("IDL:demo/Tick:1.0", "LegacyTickTopic");
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.resolve("IDL:demo/Tick:1.0").as_deref(),
            Some("LegacyTickTopic")
        );
        assert_eq!(
            map.resolve("IDL:demo/Other:1.0").as_deref(),
            Some("demo::Other")
        );
    }

    #[test]
    fn push_writes_the_payload_onto_the_resolved_topic() {
        let sink = Arc::new(RecordingSink::default());
        let mut map = TopicMap::new();
        map.map("IDL:demo/Tick:1.0", "TickTopic");
        let bridge = TopicBridge::new(map, Arc::clone(&sink) as Arc<dyn TopicSink>);

        bridge.push(event("IDL:demo/Tick:1.0", &[1, 2])).unwrap();
        bridge.push(event("IDL:demo/Other:1.0", &[3])).unwrap();

        assert_eq!(
            sink.writes(),
            alloc::vec![
                ("TickTopic".to_string(), alloc::vec![1, 2]),
                ("demo::Other".to_string(), alloc::vec![3]),
            ]
        );
        assert_eq!(bridge.forwarded(), 2);
        assert_eq!(bridge.rejected(), 0);
    }

    #[test]
    fn unresolvable_repository_id_is_reported_not_dropped() {
        let sink = Arc::new(RecordingSink::default());
        let bridge = TopicBridge::new(TopicMap::new(), Arc::clone(&sink) as Arc<dyn TopicSink>);

        let err = bridge.forward(&event("NotAnIdlId", &[1])).unwrap_err();
        assert_eq!(err, BridgeError::UnresolvedTopic("NotAnIdlId".to_string()));
        // The channel-facing `push` fails too — the event is not
        // acknowledged as delivered.
        assert_eq!(
            bridge.push(event("NotAnIdlId", &[1])).unwrap_err(),
            Disconnected
        );
        assert!(sink.writes().is_empty());
        assert_eq!(bridge.rejected(), 2);
        assert_eq!(bridge.forwarded(), 0);
    }

    #[test]
    fn sink_failure_is_reported_not_swallowed() {
        let sink = Arc::new(RecordingSink::default());
        sink.refuse.store(true, Ordering::Release);
        let bridge = TopicBridge::new(TopicMap::new(), Arc::clone(&sink) as Arc<dyn TopicSink>);

        let err = bridge
            .forward(&event("IDL:demo/Tick:1.0", &[1]))
            .unwrap_err();
        assert_eq!(
            err,
            BridgeError::SinkRejected("writer not matched".to_string())
        );
        assert_eq!(bridge.rejected(), 1);
        assert_eq!(bridge.forwarded(), 0);
    }

    #[test]
    fn disconnect_stops_forwarding() {
        let sink = Arc::new(RecordingSink::default());
        let bridge = TopicBridge::new(TopicMap::new(), Arc::clone(&sink) as Arc<dyn TopicSink>);
        bridge.push(event("IDL:demo/Tick:1.0", &[1])).unwrap();
        bridge.disconnect_push_consumer();

        assert_eq!(
            bridge
                .forward(&event("IDL:demo/Tick:1.0", &[2]))
                .unwrap_err(),
            BridgeError::Disconnected
        );
        assert_eq!(sink.writes().len(), 1);
    }

    /// End to end: a supplier pushes into a `CosEventChannelAdmin`
    /// channel, the channel fans the event out, and the bridge turns it
    /// into a DDS topic write.
    #[test]
    fn event_channel_fan_out_reaches_the_dds_topic() {
        let sink = Arc::new(RecordingSink::default());
        let mut map = TopicMap::new();
        map.map("IDL:demo/Tick:1.0", "TickTopic");
        let bridge = Arc::new(TopicBridge::new(
            map,
            Arc::clone(&sink) as Arc<dyn TopicSink>,
        ));

        let chan = EventChannel::new();
        chan.for_consumers()
            .obtain_push_supplier()
            .connect_push_consumer(Arc::clone(&bridge) as Arc<dyn PushConsumer>)
            .unwrap();
        let supplier = chan.for_suppliers().obtain_push_consumer();
        supplier.connect_push_supplier().unwrap();

        supplier.push(event("IDL:demo/Tick:1.0", &[9])).unwrap();

        assert_eq!(
            sink.writes(),
            alloc::vec![("TickTopic".to_string(), alloc::vec![9])]
        );
        assert_eq!(bridge.forwarded(), 1);
    }
}
