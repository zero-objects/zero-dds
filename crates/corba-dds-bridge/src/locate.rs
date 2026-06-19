// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP LocateRequest/LocateReply handler for the bridge daemon.
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §4.7 (= CORBA spec §15.4.5/6).
//!
//! The daemon receives a LocateRequest with an `object_key` and replies
//! with `OBJECT_HERE`, `UNKNOWN_OBJECT`, or `OBJECT_FORWARD`, depending
//! on whether the bridge mapping table knows the object_key.

use alloc::vec::Vec;

use zerodds_corba_giop::{LocateReply, LocateRequest, LocateStatusType};

use crate::mapping::BridgeMapping;

/// Result of a LocateRequest lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateOutcome {
    /// Bridge serves this object_key — `OBJECT_HERE`.
    Here,
    /// Bridge does not know this object_key — `UNKNOWN_OBJECT`.
    Unknown,
    /// Bridge requests a forward to a different IOR — `OBJECT_FORWARD`.
    /// `body` is the CDR-encapsulated IOR bytes.
    Forward(Vec<u8>),
    /// Persistent forward (GIOP 1.2+).
    ForwardPerm(Vec<u8>),
}

impl LocateOutcome {
    /// The corresponding `LocateStatusType`.
    #[must_use]
    pub const fn status(&self) -> LocateStatusType {
        match self {
            Self::Here => LocateStatusType::ObjectHere,
            Self::Unknown => LocateStatusType::UnknownObject,
            Self::Forward(_) => LocateStatusType::ObjectForward,
            Self::ForwardPerm(_) => LocateStatusType::ObjectForwardPerm,
        }
    }

    /// Body bytes (empty for `Here`/`Unknown`).
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        match self {
            Self::Here | Self::Unknown => Vec::new(),
            Self::Forward(b) | Self::ForwardPerm(b) => b,
        }
    }
}

/// Default handler: mapping lookup by `object_key` only (without
/// repository_id, because LocateRequest 1.0/1.1 has no repository ID
/// — spec §15.4.5.1).
///
/// Spec §4.7: the bridge inspects `LocateRequest::object_key`, searches
/// all routes with a matching object_key, returns `OBJECT_HERE` if ≥1
/// is found, and `UNKNOWN_OBJECT` otherwise.
#[must_use]
pub fn locate_lookup(mapping: &BridgeMapping, object_key: &[u8]) -> LocateOutcome {
    let any = mapping
        .all_routes()
        .iter()
        .any(|r| r.object_key.as_slice() == object_key);
    if any {
        LocateOutcome::Here
    } else {
        LocateOutcome::Unknown
    }
}

/// Builds the LocateReply for the LocateRequest.
#[must_use]
pub fn make_reply(req: &LocateRequest, outcome: LocateOutcome) -> LocateReply {
    let status = outcome.status();
    LocateReply {
        request_id: req.request_id,
        locate_status: status,
        body: outcome.into_body(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::mapping::{BridgeMapping, BridgeRoute, Direction, OperationMapping, TopicQosRef};

    fn empty_mapping() -> BridgeMapping {
        BridgeMapping::new()
    }

    fn mapping_with_key(key: &[u8]) -> BridgeMapping {
        let mut m = BridgeMapping::new();
        m.add_route(BridgeRoute {
            repository_id: "IDL:demo/Trader:1.0".into(),
            object_key: key.to_vec(),
            direction: Direction::CorbaToDds,
            operations: alloc::vec![OperationMapping {
                operation: "place".into(),
                request_topic: "Trade/Request".into(),
                reply_topic: "Trade/Reply".into(),
                qos: TopicQosRef::default(),
            }],
        });
        m
    }

    #[test]
    fn unknown_key_yields_unknown_object() {
        let m = empty_mapping();
        let o = locate_lookup(&m, b"unknown");
        assert_eq!(o, LocateOutcome::Unknown);
        assert_eq!(o.status(), LocateStatusType::UnknownObject);
    }

    #[test]
    fn known_key_yields_object_here() {
        let m = mapping_with_key(b"trader-1");
        let o = locate_lookup(&m, b"trader-1");
        assert_eq!(o, LocateOutcome::Here);
        assert_eq!(o.status(), LocateStatusType::ObjectHere);
    }

    #[test]
    fn forward_outcome_carries_body() {
        let body = vec![0xab, 0xcd, 0xef];
        let o = LocateOutcome::Forward(body.clone());
        assert_eq!(o.status(), LocateStatusType::ObjectForward);
        assert_eq!(o.into_body(), body);
    }

    #[test]
    fn make_reply_mirrors_request_id() {
        // LocateRequest layout uses TargetAddress in 1.2; for 1.0/1.1
        // it is just an object_key — we model both via the GIOP-crate
        // surface (LocateRequest struct).
        use zerodds_corba_giop::target_address::TargetAddress;
        let req = LocateRequest {
            request_id: 0xdead_beef,
            target: TargetAddress::Key(b"x".to_vec()),
        };
        let r = make_reply(&req, LocateOutcome::Here);
        assert_eq!(r.request_id, 0xdead_beef);
        assert_eq!(r.locate_status, LocateStatusType::ObjectHere);
    }
}
