// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request-Identity-Propagation (Spec §7.8.2.x).
//!
//! Service- und Client-Side koennen die Identitaet eines Requests
//! abfragen via `get_request_identity()`. Wir liefern das via
//! [`RequestIdentity`]-Struct, das vom Replier/Requester-Pfad
//! gefuellt wird.
//!
//! # Spec-Mapping
//!
//! * **§7.8.2.1** Service-Side `get_request_identity()` —
//!   `current_request_identity` (siehe Replier-Modul) auf Replier-Seite.
//! * **§7.8.2.2** Client-Side `get_request_identity()` —
//!   `RequesterEndpoint`-Pfad.
//! * **§7.8.2.3** Propagation via Inline-QoS
//!   (`PID_RELATED_SAMPLE_IDENTITY`) — siehe `zerodds-rtps`-Wire-Pfad.

extern crate alloc;

use crate::common_types::SampleIdentity;

/// Spec §7.8.2.1 + §7.8.2.2: Identitaet eines aktiven Requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequestIdentity {
    /// SampleIdentity des Requests.
    pub sample_identity: SampleIdentity,
    /// `true` wenn die Identitaet via Inline-QoS propagiert wurde
    /// (Spec §7.8.2.3); `false` bei Header-basierter Propagation.
    pub via_inline_qos: bool,
}

impl RequestIdentity {
    /// Konstruktor fuer Header-basierte Propagation.
    #[must_use]
    pub fn from_header(sample_identity: SampleIdentity) -> Self {
        Self {
            sample_identity,
            via_inline_qos: false,
        }
    }

    /// Konstruktor fuer Inline-QoS-basierte Propagation.
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
