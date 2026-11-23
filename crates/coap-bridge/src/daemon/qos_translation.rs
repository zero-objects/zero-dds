// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → CoAP behavior.
//!
//! The table of spec `zerodds-coap-bridge-1.0.md` §6, from a topic's
//! `qos:`:
//!
//! * Reliability `RELIABLE` → notifications are Confirmable and
//!   retransmitted; `BEST_EFFORT` → Non-confirmable.
//! * Durability `VOLATILE` → a new observer gets no sample in the first
//!   response; `TRANSIENT_LOCAL` and above → the last sample, with
//!   History `KEEP_LAST(N)` the last N (the first in the response, the
//!   others as notifications), with `KEEP_ALL` the last
//!   `KEEP_ALL_WINDOW` (1024) samples of the observe module.
//! * Lifespan → the Max-Age of a representation, what is left of it.
//! * Deadline → `5.03 Service Unavailable` with Max-Age 0 once the topic
//!   has gone longer without a sample.
//! * Liveliness → the daemon's CoAP ping every `coap.keep_alive_secs`
//!   (`super::observe`); the lease goes to the topic's DDS endpoints.
//! * Partition → the partitions of the topic's DDS endpoints: a sample of
//!   another partition does not reach the daemon, so no observer.

use std::time::Duration;

use super::config::TopicConfig;

/// CoAP message type of a notification (RFC 7252 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoapMessageType {
    /// Confirmable (CON): acknowledged, retransmitted.
    Confirmable,
    /// Non-confirmable (NON).
    NonConfirmable,
}

/// The CoAP behavior of a topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoapBehavior {
    /// Whether notifications are Confirmable.
    pub confirmable: bool,
    /// How many of the last samples a new observer is given; 0 = none.
    pub replay_depth: usize,
    /// The lifespan of a sample: the Max-Age of its representation.
    pub lifespan: Option<Duration>,
    /// The longest the topic goes without a sample.
    pub deadline: Option<Duration>,
}

impl CoapBehavior {
    /// The message type of a notification.
    #[must_use]
    pub const fn message_type(&self) -> CoapMessageType {
        if self.confirmable {
            CoapMessageType::Confirmable
        } else {
            CoapMessageType::NonConfirmable
        }
    }
}

/// The behavior of `topic` (spec §6).
#[must_use]
pub fn topic_behavior(topic: &TopicConfig) -> CoapBehavior {
    let confirmable = topic.reliability != "best_effort";
    let replay_depth = match topic.durability.as_str() {
        "transient_local" | "transient" | "persistent" => {
            if topic.history_kind == "keep_all" {
                super::observe::KEEP_ALL_WINDOW
            } else {
                usize::try_from(topic.history_depth.max(1)).unwrap_or(1)
            }
        }
        _ => 0,
    };
    CoapBehavior {
        confirmable,
        replay_depth,
        lifespan: topic.lifespan_ms.map(Duration::from_millis),
        deadline: topic.deadline_ms.map(Duration::from_millis),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topic(reliability: &str, durability: &str) -> TopicConfig {
        TopicConfig {
            dds_name: "T".into(),
            reliability: reliability.into(),
            durability: durability.into(),
            history_depth: 10,
            history_kind: "keep_last".into(),
            ..TopicConfig::default()
        }
    }

    /// Spec §6: RELIABLE → Confirmable notifications, BEST_EFFORT →
    /// Non-confirmable; RELIABLE is the default.
    #[test]
    fn coapbridge_6_reliability_picks_the_message_type() {
        assert_eq!(
            topic_behavior(&topic("reliable", "volatile")).message_type(),
            CoapMessageType::Confirmable
        );
        assert_eq!(
            topic_behavior(&topic("best_effort", "volatile")).message_type(),
            CoapMessageType::NonConfirmable
        );
        assert!(topic_behavior(&topic("", "")).confirmable);
    }

    /// Spec §6: VOLATILE gives a new observer nothing, TRANSIENT_LOCAL with
    /// KEEP_LAST(N) the last N, KEEP_ALL the window.
    #[test]
    fn coapbridge_6_durability_and_history_set_the_replay() {
        assert_eq!(
            topic_behavior(&topic("reliable", "volatile")).replay_depth,
            0
        );
        assert_eq!(topic_behavior(&topic("reliable", "")).replay_depth, 0);
        assert_eq!(
            topic_behavior(&topic("reliable", "transient_local")).replay_depth,
            10
        );
        let mut all = topic("reliable", "transient_local");
        all.history_kind = "keep_all".into();
        assert_eq!(
            topic_behavior(&all).replay_depth,
            super::super::observe::KEEP_ALL_WINDOW
        );
    }

    /// Spec §6: the lifespan and the deadline of the topic.
    #[test]
    fn coapbridge_6_lifespan_and_deadline() {
        let mut t = topic("reliable", "volatile");
        assert_eq!(topic_behavior(&t).lifespan, None);
        assert_eq!(topic_behavior(&t).deadline, None);
        t.lifespan_ms = Some(2500);
        t.deadline_ms = Some(1000);
        assert_eq!(
            topic_behavior(&t).lifespan,
            Some(Duration::from_millis(2500))
        );
        assert_eq!(topic_behavior(&t).deadline, Some(Duration::from_secs(1)));
    }
}
