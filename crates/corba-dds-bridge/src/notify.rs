// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotification-Mapping (DDS-Topic ↔ CORBA-Notification-Channel).
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §4.4 (= OMG CosNotification 1.1).
//!
//! Diese Schicht liefert die **Mapping-Tabelle** und die **Translation
//! Helpers**, NICHT die volle Notify-Service-Implementierung. Der
//! konkrete `EventChannel`-IDL-Adapter lebt in `crates/corba-cos-event`.
//!
//! ZeroDDS-Mapping-Entscheidungen (RC1):
//! * Ein DDS-Topic ⇔ ein Notify-Channel.
//! * `StructuredEvent` (Spec §2.3.4) ist die Standard-Wire-Form;
//!   `untyped EventTypes` werden auf `_GenericEvent` abgebildet.
//! * QoS-Property-Filter werden best-effort auf DDS-QoS-Policies
//!   uebersetzt — `MaximumBatchSize` → DDS-Lifespan; `Persistence` →
//!   DDS-Durability. Spec §4.4-Tabelle 7.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Eine `StructuredEvent` (CosNotification §2.3.4) reduziert auf die
/// drei wire-tragenden Felder. Volle Notify-Header-Felder
/// (`event_type.domain_name`/`event_type.type_name`/`event_name`) werden
/// als `EventName` aggregiert.
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

/// Notify-Channel-Mapping: pro DDS-Topic ein Notify-Channel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotifyChannelMap {
    /// `dds_topic_name` ⇒ `notify_channel_name`.
    topics: BTreeMap<String, String>,
}

impl NotifyChannelMap {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriere ein Mapping.
    pub fn map(&mut self, dds_topic: impl Into<String>, channel: impl Into<String>) {
        self.topics.insert(dds_topic.into(), channel.into());
    }

    /// Lookup Channel-Name fuer einen DDS-Topic.
    #[must_use]
    pub fn channel_for(&self, dds_topic: &str) -> Option<&str> {
        self.topics.get(dds_topic).map(String::as_str)
    }

    /// Reverse-Lookup DDS-Topic fuer einen Channel-Name.
    #[must_use]
    pub fn topic_for_channel(&self, channel: &str) -> Option<&str> {
        self.topics
            .iter()
            .find(|(_, c)| c.as_str() == channel)
            .map(|(t, _)| t.as_str())
    }

    /// Anzahl Mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.topics.len()
    }

    /// Leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }
}

/// Translation: DDS-Sample (Bytes) → StructuredEvent.
///
/// Spec §4.4: das ZeroDDS-Mapping nutzt den DDS-Topic-Namen als
/// `event_type.type_name` und `dds.publish` als `event_name`. Das
/// payload wandert in `remainder_body`.
#[must_use]
pub fn dds_to_structured_event(topic: &str, payload: &[u8]) -> StructuredEventLite {
    StructuredEventLite {
        event_type: format!("zerodds::{topic}"),
        event_name: "dds.publish".into(),
        filterable: Vec::new(),
        remainder_body: payload.to_vec(),
    }
}

/// Translation: StructuredEvent → DDS-Sample (Bytes).
///
/// Identitaets-Operation auf `remainder_body`. `filterable`-Felder
/// werden RC1 nicht an DDS-Properties durchgereicht; das ist
/// RC2-Backlog.
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
