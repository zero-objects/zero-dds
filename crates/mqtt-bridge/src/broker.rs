// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT-5.0 Broker-Logic — Spec §3 + §4.
//!
//! In-Memory-Broker mit:
//! * Session-Persistence (Spec §4.1).
//! * Subscription-Tabelle mit Wildcard-Matching (§4.7).
//! * Retained-Messages (§3.3.1.3).
//! * Will-Message-Delivery (§3.1.2.5, §3.1.3.2).
//! * QoS 0/1/2 mit Packet-Identifier-Tracking (§4.6).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::topic_filter::{matches, validate_filter};

/// QoS-Level (Spec §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QoS {
    /// `0` At-Most-Once.
    AtMostOnce = 0,
    /// `1` At-Least-Once.
    AtLeastOnce = 1,
    /// `2` Exactly-Once.
    ExactlyOnce = 2,
}

impl QoS {
    /// Wire-Wert.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `u8 -> QoS`.
    ///
    /// # Errors
    /// `()` wenn Wert nicht 0/1/2.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::AtMostOnce),
            1 => Ok(Self::AtLeastOnce),
            2 => Ok(Self::ExactlyOnce),
            _ => Err(()),
        }
    }
}

/// Will-Message — vom Client beim CONNECT registriert. Spec §3.1.2.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Will {
    /// Will-Topic.
    pub topic: String,
    /// Will-Payload.
    pub payload: Vec<u8>,
    /// Will-QoS.
    pub qos: QoS,
    /// Will-Retain-Flag.
    pub retain: bool,
}

/// Subscription-Eintrag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// Filter (mit Wildcards).
    pub filter: String,
    /// Maximum-QoS fuer diese Subscription.
    pub max_qos: QoS,
    /// `No-Local`-Flag (Spec §3.8.3.1).
    pub no_local: bool,
    /// `Retain-As-Published` (Spec §3.8.3.1).
    pub retain_as_published: bool,
}

/// Session-State pro Client. Spec §4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    /// Client-Id.
    pub client_id: String,
    /// `Clean-Start`-Flag aus dem letzten CONNECT.
    pub clean_start: bool,
    /// Subscriptions.
    pub subscriptions: Vec<Subscription>,
    /// Will (optional).
    pub will: Option<Will>,
    /// Naechster Packet-Identifier fuer ausgehende QoS>=1-Messages.
    pub next_packet_id: u16,
    /// Pending QoS-1/2 Acks (Spec §4.6) — Packet-Id → Topic.
    pub in_flight: BTreeMap<u16, String>,
}

impl Session {
    /// Konstruktor.
    #[must_use]
    pub fn new(client_id: String, clean_start: bool) -> Self {
        Self {
            client_id,
            clean_start,
            subscriptions: Vec::new(),
            will: None,
            next_packet_id: 1,
            in_flight: BTreeMap::new(),
        }
    }

    /// Vergibt einen frischen Packet-Identifier.
    pub fn allocate_packet_id(&mut self, topic: String) -> u16 {
        let id = self.next_packet_id;
        self.next_packet_id = self.next_packet_id.wrapping_add(1);
        if self.next_packet_id == 0 {
            self.next_packet_id = 1;
        }
        self.in_flight.insert(id, topic);
        id
    }

    /// Markiert eine Packet-Id als acknowledged. Spec §4.6.
    pub fn ack_packet_id(&mut self, id: u16) -> bool {
        self.in_flight.remove(&id).is_some()
    }
}

/// Retained-Message — pro Topic ein Eintrag (Spec §3.3.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedMessage {
    /// Topic-Name (ohne Wildcards).
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS bei dem die Message published wurde.
    pub qos: QoS,
}

/// Broker-State.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Broker {
    sessions: BTreeMap<String, Session>,
    retained: BTreeMap<String, RetainedMessage>,
}

/// Eine zu liefernde PUBLISH-Message zu einem Subscriber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEnvelope {
    /// Ziel-Client-Id.
    pub client_id: String,
    /// Topic-Name (so wie publisher es geschickt hat).
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS = min(publish.qos, subscription.max_qos) per Spec §3.8.3.
    pub qos: QoS,
    /// `Retain`-Flag fuer diesen Subscriber (siehe `retain_as_published`).
    pub retain: bool,
}

impl Broker {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// CONNECT (Spec §3.1) — bringt eine Session online.
    /// `clean_start=true` verwirft eine bestehende Session.
    pub fn connect(&mut self, client_id: String, clean_start: bool, will: Option<Will>) {
        let entry = self
            .sessions
            .entry(client_id.clone())
            .or_insert_with(|| Session::new(client_id.clone(), clean_start));
        if clean_start {
            entry.subscriptions.clear();
            entry.in_flight.clear();
            entry.will = will;
        } else if entry.will.is_none() {
            entry.will = will;
        }
        entry.clean_start = clean_start;
    }

    /// DISCONNECT — Spec §3.14. Will wird nur bei "abnormaler"
    /// Disconnect (Caller liefert `with_will=true`) emittiert.
    pub fn disconnect(&mut self, client_id: &str, with_will: bool) -> Vec<DeliveryEnvelope> {
        // Step 1: extract will + clean_start without holding mut-borrow.
        let (will, clean_start) = match self.sessions.get_mut(client_id) {
            Some(s) => {
                let will = if with_will { s.will.take() } else { None };
                if !with_will {
                    s.will = None;
                }
                (will, s.clean_start)
            }
            None => return Vec::new(),
        };
        // Step 2: fanout (immutable borrow OK now).
        let envelopes = if let Some(w) = will {
            self.fanout_publish(&w.topic, &w.payload, w.qos, w.retain)
        } else {
            Vec::new()
        };
        // Step 3: drop session if clean-start.
        if clean_start {
            self.sessions.remove(client_id);
        }
        envelopes
    }

    /// SUBSCRIBE (Spec §3.8) — fuegt Subscriptions in die Session.
    /// Liefert eine Liste von Granted-QoS pro Filter.
    ///
    /// # Errors
    /// Static-String wenn Client unbekannt oder Filter invalid.
    pub fn subscribe(
        &mut self,
        client_id: &str,
        subs: Vec<Subscription>,
    ) -> Result<Vec<QoS>, &'static str> {
        let session = self.sessions.get_mut(client_id).ok_or("unknown client")?;
        let mut granted = Vec::with_capacity(subs.len());
        for s in subs {
            validate_filter(&s.filter).map_err(|_| "invalid filter")?;
            granted.push(s.max_qos);
            // Replace existing subscription on same filter (Spec §3.8.4).
            session.subscriptions.retain(|x| x.filter != s.filter);
            session.subscriptions.push(s);
        }
        Ok(granted)
    }

    /// Liefert die Retained-Messages, die ein neuer Subscriber sehen
    /// muss (Spec §3.3.1.3).
    #[must_use]
    pub fn retained_for(&self, filter: &str) -> Vec<&RetainedMessage> {
        self.retained
            .values()
            .filter(|r| matches(filter, &r.topic))
            .collect()
    }

    /// PUBLISH (Spec §3.3) — verteilt die Message an alle Subscriber.
    /// Setzt Retained-State falls retain=true.
    pub fn publish(
        &mut self,
        topic: &str,
        payload: Vec<u8>,
        qos: QoS,
        retain: bool,
    ) -> Vec<DeliveryEnvelope> {
        if retain {
            if payload.is_empty() {
                // Spec §3.3.1.3: empty retain payload removes retained.
                self.retained.remove(topic);
            } else {
                self.retained.insert(
                    topic.into(),
                    RetainedMessage {
                        topic: topic.into(),
                        payload: payload.clone(),
                        qos,
                    },
                );
            }
        }
        self.fanout_publish(topic, &payload, qos, retain)
    }

    fn fanout_publish(
        &self,
        topic: &str,
        payload: &[u8],
        qos: QoS,
        retain: bool,
    ) -> Vec<DeliveryEnvelope> {
        let mut envs = Vec::new();
        for session in self.sessions.values() {
            for sub in &session.subscriptions {
                if matches(&sub.filter, topic) {
                    let effective_qos = match (sub.max_qos, qos) {
                        (QoS::AtMostOnce, _) | (_, QoS::AtMostOnce) => QoS::AtMostOnce,
                        (QoS::AtLeastOnce, _) | (_, QoS::AtLeastOnce) => QoS::AtLeastOnce,
                        _ => QoS::ExactlyOnce,
                    };
                    envs.push(DeliveryEnvelope {
                        client_id: session.client_id.clone(),
                        topic: topic.into(),
                        payload: payload.to_vec(),
                        qos: effective_qos,
                        retain: if sub.retain_as_published {
                            retain
                        } else {
                            false
                        },
                    });
                }
            }
        }
        envs
    }

    /// UNSUBSCRIBE (Spec §3.10).
    ///
    /// # Errors
    /// Static-String wenn Client unbekannt.
    pub fn unsubscribe(
        &mut self,
        client_id: &str,
        filters: &[String],
    ) -> Result<usize, &'static str> {
        let session = self.sessions.get_mut(client_id).ok_or("unknown client")?;
        let before = session.subscriptions.len();
        session
            .subscriptions
            .retain(|s| !filters.iter().any(|f| f == &s.filter));
        Ok(before - session.subscriptions.len())
    }

    /// Anzahl aktiver Sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Anzahl Retained-Messages.
    #[must_use]
    pub fn retained_count(&self) -> usize {
        self.retained.len()
    }

    /// Lookup einer Session.
    #[must_use]
    pub fn session(&self, client_id: &str) -> Option<&Session> {
        self.sessions.get(client_id)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sub(filter: &str, qos: QoS) -> Subscription {
        Subscription {
            filter: filter.into(),
            max_qos: qos,
            no_local: false,
            retain_as_published: false,
        }
    }

    #[test]
    fn qos_round_trip() {
        for q in [QoS::AtMostOnce, QoS::AtLeastOnce, QoS::ExactlyOnce] {
            assert_eq!(QoS::from_u8(q.to_u8()).unwrap(), q);
        }
        assert!(QoS::from_u8(3).is_err());
    }

    #[test]
    fn connect_creates_session() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        assert_eq!(b.session_count(), 1);
    }

    #[test]
    fn subscribe_then_publish_delivers() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("a/+", QoS::AtLeastOnce)])
            .unwrap();
        let envs = b.publish("a/x", alloc::vec![1, 2, 3], QoS::AtLeastOnce, false);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].client_id, "c1");
        assert_eq!(envs[0].payload, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn publish_qos_is_min_of_pub_and_sub() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("t", QoS::AtLeastOnce)])
            .unwrap();
        // Publisher sends QoS 2, subscription max is 1 → effective 1.
        let envs = b.publish("t", alloc::vec![1], QoS::ExactlyOnce, false);
        assert_eq!(envs[0].qos, QoS::AtLeastOnce);
    }

    #[test]
    fn retained_message_persists() {
        let mut b = Broker::new();
        b.publish("t", alloc::vec![1, 2], QoS::AtMostOnce, true);
        assert_eq!(b.retained_count(), 1);
        let r = b.retained_for("#");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].payload, alloc::vec![1, 2]);
    }

    #[test]
    fn empty_payload_clears_retained() {
        let mut b = Broker::new();
        b.publish("t", alloc::vec![1], QoS::AtMostOnce, true);
        b.publish("t", alloc::vec![], QoS::AtMostOnce, true);
        assert_eq!(b.retained_count(), 0);
    }

    #[test]
    fn invalid_filter_rejected() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        assert!(
            b.subscribe("c1", alloc::vec![sub("a/#/c", QoS::AtMostOnce)])
                .is_err()
        );
    }

    #[test]
    fn unknown_client_subscribe_rejected() {
        let mut b = Broker::new();
        assert!(
            b.subscribe("ghost", alloc::vec![sub("a", QoS::AtMostOnce)])
                .is_err()
        );
    }

    #[test]
    fn unsubscribe_removes_filter() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe(
            "c1",
            alloc::vec![sub("a", QoS::AtMostOnce), sub("b", QoS::AtMostOnce)],
        )
        .unwrap();
        let n = b.unsubscribe("c1", &alloc::vec!["a".into()]).unwrap();
        assert_eq!(n, 1);
        assert_eq!(b.session("c1").unwrap().subscriptions.len(), 1);
    }

    #[test]
    fn disconnect_with_will_delivers_will_message() {
        let mut b = Broker::new();
        b.connect(
            "c1".into(),
            true,
            Some(Will {
                topic: "lwt".into(),
                payload: alloc::vec![9, 9],
                qos: QoS::AtLeastOnce,
                retain: false,
            }),
        );
        b.connect("c2".into(), true, None);
        b.subscribe("c2", alloc::vec![sub("lwt", QoS::AtLeastOnce)])
            .unwrap();
        let envs = b.disconnect("c1", true);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].client_id, "c2");
    }

    #[test]
    fn clean_disconnect_drops_will() {
        let mut b = Broker::new();
        b.connect(
            "c1".into(),
            true,
            Some(Will {
                topic: "lwt".into(),
                payload: alloc::vec![],
                qos: QoS::AtMostOnce,
                retain: false,
            }),
        );
        let envs = b.disconnect("c1", false);
        assert!(envs.is_empty());
    }

    #[test]
    fn subscribe_replaces_existing_filter() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("a", QoS::AtMostOnce)])
            .unwrap();
        b.subscribe("c1", alloc::vec![sub("a", QoS::ExactlyOnce)])
            .unwrap();
        assert_eq!(b.session("c1").unwrap().subscriptions.len(), 1);
        assert_eq!(
            b.session("c1").unwrap().subscriptions[0].max_qos,
            QoS::ExactlyOnce
        );
    }

    #[test]
    fn allocate_packet_id_increments() {
        let mut s = Session::new("c1".into(), true);
        let id1 = s.allocate_packet_id("t".into());
        let id2 = s.allocate_packet_id("t".into());
        assert_ne!(id1, id2);
        assert!(s.ack_packet_id(id1));
        assert!(!s.ack_packet_id(id1));
    }

    #[test]
    fn packet_id_wraps_skipping_zero() {
        let mut s = Session::new("c1".into(), true);
        s.next_packet_id = u16::MAX;
        let id1 = s.allocate_packet_id("t".into());
        assert_eq!(id1, u16::MAX);
        let id2 = s.allocate_packet_id("t".into());
        assert_eq!(id2, 1, "wraps to 1, never 0");
    }
}
