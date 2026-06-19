// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 service pattern (request-reply) as a 2-topic pair.
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §4.2 (= REP-2003 §3.4).
//!
//! A ROS-2 service `/foo/bar` with `Foo.srv` is mapped to two DDS topics:
//! * **Request**:  `rq/foo/barRequest`  (Reliable+KeepLast)
//! * **Reply**:    `rr/foo/barReply`    (Reliable+KeepLast)
//!
//! Plus: the sample properties carry `client_guid` (16 bytes) +
//! `sequence_number` (i64), so that multiple clients can
//! request in parallel. Here we provide the topic-name generation and
//! an in-memory correlation map.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Topic-name pair for a ROS-2 service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTopics {
    /// `rq/<base>Request` topic.
    pub request: String,
    /// `rr/<base>Reply` topic.
    pub reply: String,
}

impl ServiceTopics {
    /// Generates the topic names from the ROS-2 service path.
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

/// Correlation token: 16-byte client_guid + i64 sequence_number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceRequestId {
    /// Client GUID (DDS side often GUID_PREFIX || EntityId).
    pub client_guid: [u8; 16],
    /// Ascending per client.
    pub sequence_number: i64,
}

/// Pending-request tracker. The caller thread creates a token, the
/// reply-listener thread resolves it.
#[derive(Debug, Clone, Default)]
pub struct PendingRequests {
    pending: BTreeMap<ServiceRequestId, Vec<u8>>,
}

impl PendingRequests {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an outstanding request with the serialized
    /// request body (for re-send cases). Returns `false` if the
    /// token was already registered.
    pub fn register(&mut self, id: ServiceRequestId, body: Vec<u8>) -> bool {
        if self.pending.contains_key(&id) {
            return false;
        }
        self.pending.insert(id, body);
        true
    }

    /// Matches an incoming reply against the `request_id`. Returns
    /// the previously registered request body if a match exists.
    pub fn match_reply(&mut self, id: ServiceRequestId) -> Option<Vec<u8>> {
        self.pending.remove(&id)
    }

    /// How many requests are still outstanding?
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
