// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request-Identity-Propagation (Spec §7.8.2.x).
//!
//! The service side and client side can query the identity of a request
//! via `get_request_identity()`. We provide that via the
//! [`RequestIdentity`] struct, which is filled by the replier/requester
//! path.
//!
//! # Spec mapping
//!
//! * **§7.8.2.1** service-side `get_request_identity()` —
//!   `current_request_identity` (see the replier module) on the replier side.
//! * **§7.8.2.2** client-side `get_request_identity()` —
//!   `RequesterEndpoint` path.
//! * **§7.8.2.3** propagation via inline QoS
//!   (`PID_RELATED_SAMPLE_IDENTITY`) — see the `zerodds-rtps` wire path.

extern crate alloc;

use crate::common_types::SampleIdentity;

/// Spec §7.8.2.1 + §7.8.2.2: identity of an active request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequestIdentity {
    /// SampleIdentity of the request.
    pub sample_identity: SampleIdentity,
    /// `true` if the identity was propagated via inline QoS
    /// (Spec §7.8.2.3); `false` for header-based propagation.
    pub via_inline_qos: bool,
}

impl RequestIdentity {
    /// Constructor for header-based propagation.
    #[must_use]
    pub fn from_header(sample_identity: SampleIdentity) -> Self {
        Self {
            sample_identity,
            via_inline_qos: false,
        }
    }

    /// Constructor for inline-QoS-based propagation.
    #[must_use]
    pub fn from_inline_qos(sample_identity: SampleIdentity) -> Self {
        Self {
            sample_identity,
            via_inline_qos: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_header_marks_explicit_propagation() {
        let id = SampleIdentity::default();
        let r = RequestIdentity::from_header(id);
        assert!(!r.via_inline_qos);
        assert_eq!(r.sample_identity, id);
    }

    #[test]
    fn from_inline_qos_marks_implicit_propagation() {
        let id = SampleIdentity::default();
        let r = RequestIdentity::from_inline_qos(id);
        assert!(r.via_inline_qos);
        assert_eq!(r.sample_identity, id);
    }

    #[test]
    fn default_is_header_based_with_zero_identity() {
        let r = RequestIdentity::default();
        assert!(!r.via_inline_qos);
    }
}
