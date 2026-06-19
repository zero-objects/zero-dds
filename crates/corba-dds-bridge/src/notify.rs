// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotification mapping (DDS topic ↔ CORBA notification channel).
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §4.4 (= OMG CosNotification 1.1).
//!
//! This layer provides the **mapping table** and the **translation
//! helpers**, NOT the full notify-service implementation. The concrete
//! `EventChannel` IDL adapter lives in `crates/corba-cos-event`.
//!
//! ZeroDDS mapping decisions (RC1):
//! * One DDS topic ⇔ one notify channel.
//! * `StructuredEvent` (spec §2.3.4) is the standard wire form;
//!   `untyped EventTypes` are mapped onto `_GenericEvent`.
//! * QoS property filters are translated best-effort onto DDS QoS
//!   policies — `MaximumBatchSize` → DDS lifespan; `Persistence` →
//!   DDS durability. Spec §4.4 table 7.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// A `StructuredEvent` (CosNotification §2.3.4) reduced to the three
/// wire-bearing fields. The full notify header fields
/// (`event_type.domain_name`/`event_type.type_name`/`event_name`) are
/// aggregated as `EventName`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StructuredEventLite {
    /// `domain_name::type_name` (CosNotification EventType).
    pub event_type: String,
    /// `event_name` (free-form).
    pub event_name: String,
    /// `filterable_data` (NameValue-Sequence).
    pub filterable: Vec<(String, Vec<u8>)>,
    /// `remainder_of_body` (Any → CDR-Bytes).
    pub remainder_body: Vec<u8>,
}

/// Notify channel mapping: one notify channel per DDS topic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotifyChannelMap {
    /// `dds_topic_name` ⇒ `notify_channel_name`.
    topics: BTreeMap<String, String>,
}

impl NotifyChannelMap {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a mapping.
    pub fn map(&mut self, dds_topic: impl Into<String>, channel: impl Into<String>) {
        self.topics.insert(dds_topic.into(), channel.into());
    }

    /// Looks up the channel name for a DDS topic.
    #[must_use]
    pub fn channel_for(&self, dds_topic: &str) -> Option<&str> {
        self.topics.get(dds_topic).map(String::as_str)
    }

    /// Reverse lookup of the DDS topic for a channel name.
    #[must_use]
    pub fn topic_for_channel(&self, channel: &str) -> Option<&str> {
        self.topics
            .iter()
            .find(|(_, c)| c.as_str() == channel)
            .map(|(t, _)| t.as_str())
    }

    /// Number of mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.topics.len()
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }
}

/// Translation: DDS sample (bytes) → StructuredEvent.
///
/// Spec §4.4: the ZeroDDS mapping uses the DDS topic name as
/// `event_type.type_name` and `dds.publish` as `event_name`. The
/// payload goes into `remainder_body`.
#[must_use]
pub fn dds_to_structured_event(topic: &str, payload: &[u8]) -> StructuredEventLite {
    StructuredEventLite {
        event_type: format!("zerodds::{topic}"),
        event_name: "dds.publish".into(),
        filterable: Vec::new(),
        remainder_body: payload.to_vec(),
    }
}

/// Translation: StructuredEvent → DDS sample (bytes).
///
/// Identity operation on `remainder_body`. In RC1 the `filterable`
/// fields are not forwarded to DDS properties; that is RC2 backlog.
#[must_use]
pub fn structured_event_to_dds(event: &StructuredEventLite) -> Vec<u8> {
    event.remainder_body.clone()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn map_lookup_round_trip() {
        let mut m = NotifyChannelMap::new();
        m.map("Trade", "ChannelA");
        m.map("Quote", "ChannelB");
        assert_eq!(m.channel_for("Trade"), Some("ChannelA"));
        assert_eq!(m.topic_for_channel("ChannelB"), Some("Quote"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn unknown_channel_returns_none() {
        let m = NotifyChannelMap::new();
        assert_eq!(m.channel_for("nope"), None);
        assert!(m.is_empty());
    }

    #[test]
    fn dds_to_event_carries_payload_in_remainder() {
        let ev = dds_to_structured_event("Trade", b"AAPL@200");
        assert_eq!(ev.event_type, "zerodds::Trade");
        assert_eq!(ev.event_name, "dds.publish");
        assert_eq!(ev.remainder_body, b"AAPL@200");
    }

    #[test]
    fn round_trip_dds_event_dds() {
        let payload = b"hello".to_vec();
        let ev = dds_to_structured_event("X", &payload);
        let back = structured_event_to_dds(&ev);
        assert_eq!(back, payload);
    }
}
