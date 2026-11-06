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

    /// zerodds-ros2-bridge-1.0 §4.2 and §5.1: the request and reply
    /// topics of a service are `rq/<service>Request` and
    /// `rr/<service>Reply`, as rmw_cyclonedds_cpp and rmw_fastrtps_cpp
    /// name them.
    #[test]
    fn ros2bridge_4_2_a_service_is_the_topics_rq_request_and_rr_reply() {
        for name in ["/foo/bar", "foo/bar"] {
            let s = ServiceTopics::from_service(name);
            assert_eq!(s.request, "rq/foo/barRequest", "{name}");
            assert_eq!(s.reply, "rr/foo/barReply", "{name}");
        }
        let s = ServiceTopics::from_service("/add_two_ints");
        assert_eq!(s.request, "rq/add_two_intsRequest");
        assert_eq!(s.reply, "rr/add_two_intsReply");
    }

    /// §4.2: a reply belongs to the request of the same client GUID and
    /// sequence number, once; a second request under the same identity
    /// is refused, a reply nobody asked for finds nothing.
    #[test]
    fn ros2bridge_4_2_a_reply_correlates_once_by_client_guid_and_sequence_number() {
        let mut p = PendingRequests::new();
        let id = ServiceRequestId {
            client_guid: [0xab; 16],
            sequence_number: 7,
        };
        let other = ServiceRequestId {
            client_guid: [0xab; 16],
            sequence_number: 8,
        };
        assert!(p.match_reply(other).is_none());
        assert!(p.register(id, b"req".to_vec()));
        assert!(!p.register(id, b"again".to_vec()));
        assert_eq!(p.outstanding(), 1);
        assert!(p.match_reply(other).is_none());
        assert_eq!(p.match_reply(id).expect("match"), b"req");
        assert_eq!(p.outstanding(), 0);
        assert!(p.match_reply(id).is_none());
    }
}
