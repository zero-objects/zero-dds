// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS-2 Service-Pattern (Request-Reply) als 2-Topic-Pair.
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §4.2 (= REP-2003 §3.4).
//!
//! Ein ROS-2-Service `/foo/bar` mit `Foo.srv` wird auf zwei DDS-Topics
//! abgebildet:
//! * **Request**:  `rq/foo/barRequest`  (Reliable+KeepLast)
//! * **Reply**:    `rr/foo/barReply`    (Reliable+KeepLast)
//!
//! Plus: in den Sample-Properties stehen `client_guid` (16-Byte) +
//! `sequence_number` (i64), so dass mehrere Clients parallel
//! requesten koennen. Wir liefern hier die Topic-Name-Generation und
//! eine in-memory Correlation-Map.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Topic-Name-Paar fuer einen ROS-2-Service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTopics {
    /// `rq/<base>Request`-Topic.
    pub request: String,
    /// `rr/<base>Reply`-Topic.
    pub reply: String,
}

impl ServiceTopics {
    /// Generiere die Topic-Namen aus dem ROS-2-Service-Path.
    /// Spec §4.2: prefix `rq/`/`rr/`, suffix `Request`/`Reply`.
    #[must_use]
    pub fn from_service(ros_service_name: &str) -> Self {
        let base = ros_service_name.trim_start_matches('/');
        Self {
            request: format!("rq/{base}Request"),
            reply: format!("rr/{base}Reply"),
        }
    }
}

/// Korrelation-Token: 16-Byte client_guid + i64 sequence_number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceRequestId {
    /// Client-GUID (DDS-Side oft GUID_PREFIX || EntityId).
    pub client_guid: [u8; 16],
    /// Aufsteigend pro Client.
    pub sequence_number: i64,
}

/// Pending-Request-Tracker. Caller-Thread legt ein Token an, der
/// Reply-Listener-Thread loest es auf.
#[derive(Debug, Clone, Default)]
pub struct PendingRequests {
    pending: BTreeMap<ServiceRequestId, Vec<u8>>,
}

impl PendingRequests {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriere einen ausstehenden Request mit dem serialisierten
    /// Request-Body (fuer Re-Send-Cases). Liefert `false` wenn das
    /// Token schon registriert war.
    pub fn register(&mut self, id: ServiceRequestId, body: Vec<u8>) -> bool {
        if self.pending.contains_key(&id) {
            return false;
        }
        self.pending.insert(id, body);
        true
    }

    /// Match einen eingehenden Reply mit der `request_id` ab. Liefert
    /// das vorher registrierte Request-Body wenn ein Match existiert.
    pub fn match_reply(&mut self, id: ServiceRequestId) -> Option<Vec<u8>> {
        self.pending.remove(&id)
    }

    /// Wieviele Requests sind noch offen?
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn topic_names_for_simple_service() {
        let s = ServiceTopics::from_service("/foo/bar");
        assert_eq!(s.request, "rq/foo/barRequest");
        assert_eq!(s.reply, "rr/foo/barReply");
    }

    #[test]
    fn topic_names_handle_no_leading_slash() {
        let s = ServiceTopics::from_service("foo/bar");
        assert_eq!(s.request, "rq/foo/barRequest");
        assert_eq!(s.reply, "rr/foo/barReply");
    }

    #[test]
    fn pending_register_and_match_round_trips() {
        let mut p = PendingRequests::new();
        let id = ServiceRequestId {
            client_guid: [0xab; 16],
            sequence_number: 7,
        };
        assert!(p.register(id, b"req".to_vec()));
        assert_eq!(p.outstanding(), 1);
        let body = p.match_reply(id).expect("match");
        assert_eq!(body, b"req");
        assert_eq!(p.outstanding(), 0);
    }

    #[test]
    fn duplicate_register_rejected() {
        let mut p = PendingRequests::new();
        let id = ServiceRequestId {
            client_guid: [0; 16],
            sequence_number: 1,
        };
        assert!(p.register(id, b"x".to_vec()));
        assert!(!p.register(id, b"y".to_vec()));
    }

    #[test]
    fn unmatched_reply_returns_none() {
        let mut p = PendingRequests::new();
        let id = ServiceRequestId {
            client_guid: [0; 16],
            sequence_number: 1,
        };
        assert!(p.match_reply(id).is_none());
    }
}
