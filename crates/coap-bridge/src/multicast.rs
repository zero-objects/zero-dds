// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP multicast operation per RFC 7252 §8.
//!
//! Multicast addresses group communication: a NON request is
//! sent to a group address and multiple servers respond. The
//! spec requires:
//! - Multicast requests SHOULD be Non-confirmable (§8.1).
//! - Servers wait a leisure time before responding (§8.2).
//! - DEFAULT_LEISURE = 5 seconds.
//! - The token must be unique per multicast group (§8 + §5.3).

use core::time::Duration;

/// Spec §8.1 — `coap.me`-IPv4-CoAP-All-Nodes-Address (RFC 7252).
pub const ALL_NODES_LINK_LOCAL_V4: &str = "224.0.1.187";

/// Spec §8.1 — IPv6 link-local All-CoAP-Nodes (`FF02::FD`).
pub const ALL_NODES_LINK_LOCAL_V6: &str = "FF02::FD";

/// Spec §8.1 — IPv6 site-local All-CoAP-Nodes (`FF05::FD`).
pub const ALL_NODES_SITE_LOCAL_V6: &str = "FF05::FD";

/// Spec §8.2 — DEFAULT_LEISURE (5 seconds).
pub const DEFAULT_LEISURE: Duration = Duration::from_secs(5);

/// Multicast validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MulticastError {
    /// Spec §8.1: a multicast request MUST be NON, not CON.
    ConfirmableNotAllowed,
    /// Spec §8.1: a multicast request MUST have a token.
    EmptyToken,
    /// Reserved IPv4 multicast range (224.0.0.0/4).
    InvalidMulticastAddress,
}

impl core::fmt::Display for MulticastError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ConfirmableNotAllowed => write!(f, "ConfirmableNotAllowed"),
            Self::EmptyToken => write!(f, "EmptyToken"),
            Self::InvalidMulticastAddress => write!(f, "InvalidMulticastAddress"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MulticastError {}

/// Validates an IPv4 address as being in the multicast range (224.0.0.0/4).
#[must_use]
pub fn is_ipv4_multicast(addr: [u8; 4]) -> bool {
    addr[0] >= 224 && addr[0] <= 239
}

/// Validates an IPv6 address as multicast (`FFxx::*`).
#[must_use]
pub fn is_ipv6_multicast(addr: [u16; 8]) -> bool {
    (addr[0] & 0xFF00) == 0xFF00
}

/// Spec §8.1 — validates a multicast request.
///
/// Returns `Err` if:
/// - Message type is Confirmable (CON) — multicast requires NON.
/// - Token is empty — multicast requires a unique token for reply
///   matching.
///
/// # Errors
/// See [`MulticastError`].
pub fn validate_multicast_request(
    is_confirmable: bool,
    token: &[u8],
) -> Result<(), MulticastError> {
    if is_confirmable {
        return Err(MulticastError::ConfirmableNotAllowed);
    }
    if token.is_empty() {
        return Err(MulticastError::EmptyToken);
    }
    Ok(())
}

/// Spec §8.2 — server leisure time. The caller adds a random choice
/// between `0..leisure` to each multicast reply to avoid reply
/// floods.
///
/// `leisure_seed` is a per-server pseudo-random value (e.g. from
/// the token hash); here we only provide the deterministic
/// interval helper.
#[must_use]
pub fn leisure_delay(leisure: Duration, leisure_seed: u32) -> Duration {
    let max_ns = leisure.as_nanos() as u64;
    if max_ns == 0 {
        return Duration::ZERO;
    }
    let delay_ns = u64::from(leisure_seed) % max_ns;
    Duration::from_nanos(delay_ns)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_multicast_recognizes_class_d_range() {
        assert!(is_ipv4_multicast([224, 0, 1, 187]));
        assert!(is_ipv4_multicast([239, 255, 255, 255]));
        assert!(!is_ipv4_multicast([223, 255, 255, 255]));
        assert!(!is_ipv4_multicast([240, 0, 0, 0]));
    }

    #[test]
    fn ipv6_multicast_recognizes_ff_prefix() {
        assert!(is_ipv6_multicast([0xFF02, 0, 0, 0, 0, 0, 0, 0xFD]));
        assert!(is_ipv6_multicast([0xFF05, 0, 0, 0, 0, 0, 0, 0xFD]));
        assert!(!is_ipv6_multicast([0xFE80, 0, 0, 0, 0, 0, 0, 1]));
    }

    #[test]
    fn multicast_request_must_not_be_confirmable() {
        assert_eq!(
            validate_multicast_request(true, b"abc"),
            Err(MulticastError::ConfirmableNotAllowed)
        );
    }

    #[test]
    fn multicast_request_must_have_token() {
        assert_eq!(
            validate_multicast_request(false, b""),
            Err(MulticastError::EmptyToken)
        );
    }

    #[test]
    fn valid_multicast_request_passes() {
        assert!(validate_multicast_request(false, b"abc").is_ok());
    }

    #[test]
    fn leisure_delay_within_max() {
        let leisure = Duration::from_secs(5);
        for seed in [0u32, 100, 1_000_000, u32::MAX] {
            let d = leisure_delay(leisure, seed);
            assert!(d <= leisure);
        }
    }

    #[test]
    fn leisure_delay_zero_when_leisure_is_zero() {
        assert_eq!(leisure_delay(Duration::ZERO, 42), Duration::ZERO);
    }

    #[test]
    fn well_known_addresses_match_spec() {
        assert_eq!(ALL_NODES_LINK_LOCAL_V4, "224.0.1.187");
        assert_eq!(ALL_NODES_LINK_LOCAL_V6, "FF02::FD");
        assert_eq!(ALL_NODES_SITE_LOCAL_V6, "FF05::FD");
    }

    #[test]
    fn default_leisure_is_5_seconds() {
        assert_eq!(DEFAULT_LEISURE, Duration::from_secs(5));
    }
}
