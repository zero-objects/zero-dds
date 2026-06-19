// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Security builtin-topic QoS profile — DDS-Security 1.2 §7.5.3 + §7.5.4.
//!
//! Spec §7.5.3 (`DCPSParticipantStatelessMessage`):
//! * Reliability: `BEST_EFFORT`
//! * Durability: `VOLATILE`
//! * History: `KEEP_LAST(1)`
//! * Lifespan: `INFINITE`
//!
//! Spec §7.5.4 (`DCPSParticipantVolatileMessageSecure`):
//! * Reliability: `RELIABLE`
//! * Durability: `VOLATILE`
//! * History: `KEEP_ALL`
//! * Lifespan: `INFINITE`
//! * Crypto: receiver-specific MACs MUST be enabled
//!   (`participant_crypto_handle` encryption with per-receiver
//!   distinct MACs).

use core::fmt;

/// `BuiltinSecurityTopicProfile` — describes the spec-conform
/// QoS profile for a security builtin topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinSecurityTopicProfile {
    /// Reliability kind (best-effort vs reliable).
    pub reliability: ReliabilityKind,
    /// Durability kind (always Volatile for security builtins).
    pub durability: DurabilityKind,
    /// History kind.
    pub history: HistoryKind,
    /// `true` if receiver-specific MACs are mandatory.
    pub require_receiver_specific_macs: bool,
}

/// Reliability kind (spec DDS 1.4 §2.2.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReliabilityKind {
    /// `BEST_EFFORT`.
    BestEffort,
    /// `RELIABLE`.
    Reliable,
}

/// Durability kind (spec DDS 1.4 §2.2.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityKind {
    /// `VOLATILE` (default for security builtins).
    Volatile,
    /// `TRANSIENT_LOCAL`.
    TransientLocal,
}

/// History kind (spec DDS 1.4 §2.2.3.18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    /// `KEEP_LAST(depth)`.
    KeepLast(u32),
    /// `KEEP_ALL`.
    KeepAll,
}

impl fmt::Display for ReliabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BestEffort => "BEST_EFFORT",
            Self::Reliable => "RELIABLE",
        })
    }
}

/// Spec §7.5.3 `DCPSParticipantStatelessMessage` topic profile.
///
/// Used by the authentication plugin for the pre-handshake token exchange
/// (`AuthRequestMessageToken`) and the handshake sequence (request/reply/
/// final). BEST_EFFORT because the tokens are self-contained
/// (each token carries its own challenge — replays are prevented via the
/// future_challenge echo).
#[must_use]
pub fn stateless_message_profile() -> BuiltinSecurityTopicProfile {
    BuiltinSecurityTopicProfile {
        reliability: ReliabilityKind::BestEffort,
        durability: DurabilityKind::Volatile,
        history: HistoryKind::KeepLast(1),
        require_receiver_specific_macs: false,
    }
}

/// Spec §7.5.4 `DCPSParticipantVolatileMessageSecure` topic profile.
///
/// Used by the crypto plugin for the crypto-token exchange
/// (participant/DataWriter/DataReader crypto tokens). RELIABLE because
/// missing tokens lead to permanent decryption errors — we
/// must be able to retransmit tokens until acknowledged.
/// `require_receiver_specific_macs = true` is mandatory per spec §7.5.4:
/// each receiver MUST see a distinct MAC, otherwise a
/// receiver could relay a token sample it receives to
/// another receiver.
#[must_use]
pub fn volatile_message_secure_profile() -> BuiltinSecurityTopicProfile {
    BuiltinSecurityTopicProfile {
        reliability: ReliabilityKind::Reliable,
        durability: DurabilityKind::Volatile,
        history: HistoryKind::KeepAll,
        require_receiver_specific_macs: true,
    }
}

/// Validates that a proposed profile matches the spec-conform
/// profile for a security builtin topic.
///
/// # Errors
/// `&'static str` with the error cause.
pub fn validate_security_topic_profile(
    topic_name: &str,
    actual: BuiltinSecurityTopicProfile,
) -> Result<(), &'static str> {
    let expected = match topic_name {
        crate::generic_message::TOPIC_STATELESS_MESSAGE => stateless_message_profile(),
        crate::generic_message::TOPIC_VOLATILE_MESSAGE_SECURE => volatile_message_secure_profile(),
        _ => return Err("not a security builtin topic"),
    };
    if actual.reliability != expected.reliability {
        return Err("reliability mismatch");
    }
    if actual.durability != expected.durability {
        return Err("durability mismatch");
    }
    if actual.history != expected.history {
        return Err("history mismatch");
    }
    if actual.require_receiver_specific_macs != expected.require_receiver_specific_macs {
        return Err("receiver-specific-MAC requirement mismatch");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn stateless_profile_is_best_effort() {
        let p = stateless_message_profile();
        assert_eq!(p.reliability, ReliabilityKind::BestEffort);
        assert_eq!(p.durability, DurabilityKind::Volatile);
        assert_eq!(p.history, HistoryKind::KeepLast(1));
        assert!(!p.require_receiver_specific_macs);
    }

    #[test]
    fn volatile_secure_is_reliable_with_receiver_macs() {
        let p = volatile_message_secure_profile();
        assert_eq!(p.reliability, ReliabilityKind::Reliable);
        assert_eq!(p.history, HistoryKind::KeepAll);
        assert!(p.require_receiver_specific_macs);
    }

    #[test]
    fn validate_known_topic_with_matching_profile() {
        validate_security_topic_profile(
            crate::generic_message::TOPIC_STATELESS_MESSAGE,
            stateless_message_profile(),
        )
        .unwrap();
        validate_security_topic_profile(
            crate::generic_message::TOPIC_VOLATILE_MESSAGE_SECURE,
            volatile_message_secure_profile(),
        )
        .unwrap();
    }

    #[test]
    fn validate_rejects_wrong_reliability_on_volatile_topic() {
        let mut wrong = volatile_message_secure_profile();
        wrong.reliability = ReliabilityKind::BestEffort;
        let err = validate_security_topic_profile(
            crate::generic_message::TOPIC_VOLATILE_MESSAGE_SECURE,
            wrong,
        )
        .unwrap_err();
        assert_eq!(err, "reliability mismatch");
    }

    #[test]
    fn validate_rejects_missing_receiver_macs() {
        let mut wrong = volatile_message_secure_profile();
        wrong.require_receiver_specific_macs = false;
        let err = validate_security_topic_profile(
            crate::generic_message::TOPIC_VOLATILE_MESSAGE_SECURE,
            wrong,
        )
        .unwrap_err();
        assert_eq!(err, "receiver-specific-MAC requirement mismatch");
    }

    #[test]
    fn validate_rejects_unknown_topic() {
        let err =
            validate_security_topic_profile("Other", stateless_message_profile()).unwrap_err();
        assert_eq!(err, "not a security builtin topic");
    }

    #[test]
    fn reliability_display_matches_spec() {
        assert_eq!(
            alloc::format!("{}", ReliabilityKind::BestEffort),
            "BEST_EFFORT"
        );
        assert_eq!(alloc::format!("{}", ReliabilityKind::Reliable), "RELIABLE");
    }
}
