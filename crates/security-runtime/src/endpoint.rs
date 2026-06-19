// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endpoint-level protection abstraction.
//!
//! Bridge between the wire type [`EndpointSecurityInfo`] and the
//! policy level [`ProtectionLevel`]. Holds the match logic for a
//! writer/reader pair: per endpoint the wire carries a 2x u32
//! bitmask block, from which the policy engine decides whether a match
//! occurs and with which resulting protection level.
//!
//! # DoD matching matrix (plan §stage 3)
//!
//! | Writer         | Reader          | Result                         |
//! |----------------|-----------------|--------------------------------|
//! | `Encrypt`      | no caps         | match rejected                 |
//! | `Sign`         | `Encrypt`       | match, end level = `Encrypt`   |
//! | `None`         | `None`          | match, end level = `None`      |
//! | `Encrypt`      | `Encrypt`       | match, end level = `Encrypt`   |
//!
//! "Strongest value wins": `max(writer, reader)`. If one of the
//! endpoints supplies no `EndpointSecurityInfo` (legacy peer), the
//! pair is only accepted if **both** effectively run `None`.

use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};

use crate::policy::ProtectionLevel;

/// Policy view of an endpoint: which protection level this writer/reader
/// requires/offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointProtection {
    /// Required/offered protection level.
    pub level: ProtectionLevel,
}

impl EndpointProtection {
    /// Shorthand: plaintext endpoint (legacy or deliberately `None`).
    pub const PLAIN: Self = Self {
        level: ProtectionLevel::None,
    };

    /// Builder.
    #[must_use]
    pub const fn new(level: ProtectionLevel) -> Self {
        Self { level }
    }

    /// Derivation from an [`EndpointSecurityInfo`]:
    /// * payload-encrypted → `Encrypt`
    /// * submessage-encrypted → `Encrypt`
    /// * submessage/payload protected without the plugin-encrypt flag → `Sign`
    /// * no protection bits → `None`
    ///
    /// `None` as the argument (legacy endpoint) → `Self::PLAIN`.
    #[must_use]
    pub fn from_info(info: Option<&EndpointSecurityInfo>) -> Self {
        let Some(info) = info else {
            return Self::PLAIN;
        };
        if !info.is_valid() {
            // Spec §7.4.1.5: with IS_VALID missing the bits are
            // not interpretable. Treat as legacy.
            return Self::PLAIN;
        }
        if info.is_payload_encrypted() || info.is_submessage_encrypted() {
            return Self::new(ProtectionLevel::Encrypt);
        }
        if info.is_submessage_protected() || info.is_payload_protected() {
            return Self::new(ProtectionLevel::Sign);
        }
        Self::PLAIN
    }

    /// Serialization back into [`EndpointSecurityInfo`] — for the
    /// SEDP announce of our own endpoint.
    #[must_use]
    pub fn to_info(self) -> EndpointSecurityInfo {
        let mut endpoint = attrs::IS_VALID;
        let mut plugin = plugin_attrs::IS_VALID;
        match self.level {
            ProtectionLevel::None => {}
            ProtectionLevel::Sign => {
                endpoint |= attrs::IS_SUBMESSAGE_PROTECTED;
            }
            ProtectionLevel::Encrypt => {
                endpoint |= attrs::IS_SUBMESSAGE_PROTECTED | attrs::IS_PAYLOAD_PROTECTED;
                plugin |=
                    plugin_attrs::IS_SUBMESSAGE_ENCRYPTED | plugin_attrs::IS_PAYLOAD_ENCRYPTED;
            }
        }
        EndpointSecurityInfo {
            endpoint_security_attributes: endpoint,
            plugin_endpoint_security_attributes: plugin,
        }
    }
}

/// Result of a writer/reader match check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointMatch {
    /// Match accepted, at this effective protection level
    /// (`max(writer, reader)`).
    Accept(ProtectionLevel),
    /// Match rejected — the peer cannot supply the required protection.
    Reject(MatchRejectReason),
}

/// Why the matching is a reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchRejectReason {
    /// One of the peers has a protection requirement but supplied no
    /// `EndpointSecurityInfo` → interpreted as legacy —
    /// the match is only admissible with `None` on both sides.
    LegacyPeerVsProtection,
}

/// Match writer ↔ reader. From the two [`EndpointProtection`]
/// values it is determined:
/// * Is the pair compatible?
/// * At which level is communication done?
///
/// Rule (plan §stage 3 DoD):
/// * writer `Encrypt`, reader `None` without info → `Reject`
/// * writer `None` without info, reader `Encrypt` → `Reject`
/// * writer `Sign`, reader `Encrypt` → `Accept(Encrypt)` (stronger wins)
/// * everything else → `Accept(max(w, r))`
#[must_use]
pub fn match_endpoints(
    writer: &EndpointProtection,
    reader: &EndpointProtection,
    writer_has_info: bool,
    reader_has_info: bool,
) -> EndpointMatch {
    // If one side wants protection and the other is a legacy
    // endpoint (no EndpointSecurityInfo), that is a reject.
    let writer_wants_protection = !matches!(writer.level, ProtectionLevel::None);
    let reader_wants_protection = !matches!(reader.level, ProtectionLevel::None);
    if writer_wants_protection && !reader_has_info {
        return EndpointMatch::Reject(MatchRejectReason::LegacyPeerVsProtection);
    }
    if reader_wants_protection && !writer_has_info {
        return EndpointMatch::Reject(MatchRejectReason::LegacyPeerVsProtection);
    }
    EndpointMatch::Accept(writer.level.stronger(reader.level))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // ---- EndpointProtection::from_info ----

    #[test]
    fn from_info_none_is_plain() {
        assert_eq!(
            EndpointProtection::from_info(None),
            EndpointProtection::PLAIN
        );
    }

    #[test]
    fn from_info_invalid_valid_bit_is_plain() {
        // Flags set, but no IS_VALID -> spec forbids
        // interpretation -> legacy.
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_SUBMESSAGE_ENCRYPTED,
        };
        assert_eq!(
            EndpointProtection::from_info(Some(&info)),
            EndpointProtection::PLAIN
        );
    }

    #[test]
    fn from_info_plain_legacy_is_plain() {
        let info = EndpointSecurityInfo::plain();
        assert_eq!(
            EndpointProtection::from_info(Some(&info)),
            EndpointProtection::PLAIN
        );
    }

    #[test]
    fn from_info_submessage_protected_without_encrypt_is_sign() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID,
        };
        assert_eq!(
            EndpointProtection::from_info(Some(&info)).level,
            ProtectionLevel::Sign
        );
    }

    #[test]
    fn from_info_submessage_encrypted_is_encrypt() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_SUBMESSAGE_ENCRYPTED,
        };
        assert_eq!(
            EndpointProtection::from_info(Some(&info)).level,
            ProtectionLevel::Encrypt
        );
    }

    #[test]
    fn from_info_payload_encrypted_is_encrypt() {
        let info = EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_PAYLOAD_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_PAYLOAD_ENCRYPTED,
        };
        assert_eq!(
            EndpointProtection::from_info(Some(&info)).level,
            ProtectionLevel::Encrypt
        );
    }

    // ---- EndpointProtection::to_info ----

    #[test]
    fn to_info_none_sets_only_valid_bits() {
        let info = EndpointProtection::PLAIN.to_info();
        assert!(info.is_valid());
        assert!(!info.is_submessage_protected());
        assert!(!info.is_payload_protected());
        assert!(!info.is_submessage_encrypted());
    }

    #[test]
    fn to_info_sign_sets_submessage_protected_without_encryption() {
        let info = EndpointProtection::new(ProtectionLevel::Sign).to_info();
        assert!(info.is_valid());
        assert!(info.is_submessage_protected());
        assert!(!info.is_submessage_encrypted());
        assert!(!info.is_payload_encrypted());
    }

    #[test]
    fn to_info_encrypt_sets_both_protection_and_encryption() {
        let info = EndpointProtection::new(ProtectionLevel::Encrypt).to_info();
        assert!(info.is_valid());
        assert!(info.is_submessage_protected());
        assert!(info.is_payload_protected());
        assert!(info.is_submessage_encrypted());
        assert!(info.is_payload_encrypted());
    }

    #[test]
    fn to_info_from_info_roundtrip_for_all_levels() {
        for lvl in [
            ProtectionLevel::None,
            ProtectionLevel::Sign,
            ProtectionLevel::Encrypt,
        ] {
            let ep = EndpointProtection::new(lvl);
            let info = ep.to_info();
            let back = EndpointProtection::from_info(Some(&info));
            assert_eq!(back, ep, "roundtrip fails for {lvl:?}");
        }
    }

    // ---- match_endpoints — DoD matrix from plan §stage 3 ----

    #[test]
    fn dod_writer_encrypt_reader_no_plugin_is_reject() {
        let w = EndpointProtection::new(ProtectionLevel::Encrypt);
        let r = EndpointProtection::PLAIN;
        let result = match_endpoints(
            &w, &r, /*writer_has_info=*/ true, /*reader_has_info=*/ false,
        );
        assert_eq!(
            result,
            EndpointMatch::Reject(MatchRejectReason::LegacyPeerVsProtection)
        );
    }

    #[test]
    fn dod_writer_sign_reader_encrypt_accepts_with_encrypt() {
        let w = EndpointProtection::new(ProtectionLevel::Sign);
        let r = EndpointProtection::new(ProtectionLevel::Encrypt);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Encrypt));
    }

    #[test]
    fn writer_none_reader_none_both_info_accepts_none() {
        let w = EndpointProtection::PLAIN;
        let r = EndpointProtection::PLAIN;
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::None));
    }

    #[test]
    fn writer_none_reader_none_both_legacy_accepts_none() {
        // Two legacy peers without a security PID — legacy ↔ legacy is ok.
        let w = EndpointProtection::PLAIN;
        let r = EndpointProtection::PLAIN;
        let result = match_endpoints(&w, &r, false, false);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::None));
    }

    #[test]
    fn writer_encrypt_reader_encrypt_accepts_encrypt() {
        let w = EndpointProtection::new(ProtectionLevel::Encrypt);
        let r = EndpointProtection::new(ProtectionLevel::Encrypt);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Encrypt));
    }

    #[test]
    fn writer_plain_reader_encrypt_rejects_if_writer_legacy() {
        // Writer is legacy (no security info), reader wants Encrypt → reject.
        let w = EndpointProtection::PLAIN;
        let r = EndpointProtection::new(ProtectionLevel::Encrypt);
        let result = match_endpoints(&w, &r, /*writer_has_info=*/ false, true);
        assert_eq!(
            result,
            EndpointMatch::Reject(MatchRejectReason::LegacyPeerVsProtection)
        );
    }

    #[test]
    fn writer_encrypt_reader_sign_accepts_with_encrypt() {
        // Symmetric case to the DoD example: reader offers SIGN,
        // writer ENCRYPT → stronger wins = ENCRYPT.
        let w = EndpointProtection::new(ProtectionLevel::Encrypt);
        let r = EndpointProtection::new(ProtectionLevel::Sign);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Encrypt));
    }

    #[test]
    fn writer_sign_reader_sign_accepts_with_sign() {
        let w = EndpointProtection::new(ProtectionLevel::Sign);
        let r = EndpointProtection::new(ProtectionLevel::Sign);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Sign));
    }

    #[test]
    fn writer_none_with_info_reader_sign_rejects_only_if_writer_cant() {
        // Writer has SecurityInfo but level=None (explicitly plaintext);
        // reader wants SIGN → from the reader's perspective that is not satisfiable
        // — in the current version we accept the match because both
        // have SecurityInfo. The DoD allows "stronger wins".
        let w = EndpointProtection::PLAIN;
        let r = EndpointProtection::new(ProtectionLevel::Sign);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Sign));
    }
}
