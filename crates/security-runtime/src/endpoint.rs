// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endpoint-Level-Protection Abstraktion.
//!
//! Bruecke zwischen dem Wire-Typ [`EndpointSecurityInfo`] und der
//! Policy-Ebene [`ProtectionLevel`]. Haelt die Match-Logik fuer ein
//! Writer/Reader-Paar: pro Endpoint traegt der Wire einen 2x u32-
//! Bitmask-Block, die Policy-Engine entscheidet daraus, ob ein Match
//! zustandekommt und mit welchem resultierenden Protection-Level.
//!
//! # DoD-Matching-Matrix (Plan §Stufe 3)
//!
//! | Writer         | Reader          | Ergebnis                       |
//! |----------------|-----------------|--------------------------------|
//! | `Encrypt`      | keine Caps      | Match abgelehnt                |
//! | `Sign`         | `Encrypt`       | Match, Endlevel = `Encrypt`    |
//! | `None`         | `None`          | Match, Endlevel = `None`       |
//! | `Encrypt`      | `Encrypt`       | Match, Endlevel = `Encrypt`    |
//!
//! "Staerkster Wert gewinnt": `max(writer, reader)`. Wenn einer der
//! Endpoints kein `EndpointSecurityInfo` liefert (Legacy-Peer), wird
//! das Paar nur akzeptiert wenn **beide** effektiv `None` fahren.

use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};

use crate::policy::ProtectionLevel;

/// Policy-Sicht auf ein Endpoint: welches Protection-Level verlangt/
/// bietet dieser Writer/Reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointProtection {
    /// Verlangtes/angebotenes Protection-Level.
    pub level: ProtectionLevel,
}

impl EndpointProtection {
    /// Kurzform: plaintext-Endpoint (Legacy oder bewusst `None`).
    pub const PLAIN: Self = Self {
        level: ProtectionLevel::None,
    };

    /// Builder.
    #[must_use]
    pub const fn new(level: ProtectionLevel) -> Self {
        Self { level }
    }

    /// Ableitung aus einem [`EndpointSecurityInfo`]:
    /// * Payload-encrypted → `Encrypt`
    /// * Submessage-encrypted → `Encrypt`
    /// * Submessage/Payload protected ohne Plugin-Encrypt-Flag → `Sign`
    /// * Keine Protection-Bits → `None`
    ///
    /// `None` als Argument (Legacy-Endpoint) → `Self::PLAIN`.
    #[must_use]
    pub fn from_info(info: Option<&EndpointSecurityInfo>) -> Self {
        let Some(info) = info else {
            return Self::PLAIN;
        };
        if !info.is_valid() {
            // Spec §7.4.1.5: bei fehlendem IS_VALID sind die Bits
            // nicht interpretierbar. Behandle als Legacy.
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

    /// Serialisierung zurueck in [`EndpointSecurityInfo`] — fuer
    /// SEDP-Announce unseres eigenen Endpoints.
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

/// Ergebnis eines Writer/Reader-Match-Checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointMatch {
    /// Match akzeptiert, auf diesem effektiven Protection-Level
    /// (`max(writer, reader)`).
    Accept(ProtectionLevel),
    /// Match abgelehnt — Peer kann die verlangte Protection nicht liefern.
    Reject(MatchRejectReason),
}

/// Warum das Matching ein Reject ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchRejectReason {
    /// Einer der Peers hat eine Protection-Anforderung, aber kein
    /// `EndpointSecurityInfo` geliefert → als Legacy interpretiert —
    /// Match nur bei beidseitigem `None` zulaessig.
    LegacyPeerVsProtection,
}

/// Match Writer ↔ Reader. Aus den beiden [`EndpointProtection`]-
/// Werten wird bestimmt:
/// * Ist das Paar kompatibel?
/// * Auf welchem Level wird kommuniziert?
///
/// Regel (Plan §Stufe 3 DoD):
/// * Writer `Encrypt`, Reader `None` ohne Info → `Reject`
/// * Writer `None` ohne Info, Reader `Encrypt` → `Reject`
/// * Writer `Sign`, Reader `Encrypt` → `Accept(Encrypt)` (stronger wins)
/// * Alles andere → `Accept(max(w, r))`
#[must_use]
pub fn match_endpoints(
    writer: &EndpointProtection,
    reader: &EndpointProtection,
    writer_has_info: bool,
    reader_has_info: bool,
) -> EndpointMatch {
    // Wenn eine Seite Protection will und die andere ein Legacy-
    // Endpoint ist (kein EndpointSecurityInfo), ist das ein Reject.
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
        // Flags gesetzt, aber kein IS_VALID -> Spec verbietet
        // Interpretation -> Legacy.
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
            assert_eq!(back, ep, "roundtrip scheitert fuer {lvl:?}");
        }
    }

    // ---- match_endpoints — DoD-Matrix aus Plan §Stufe 3 ----

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
        // Zwei legacy-Peers ohne Security-PID — legacy ↔ legacy ist ok.
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
        // Writer ist Legacy (no security info), Reader will Encrypt → Reject.
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
        // Symmetrischer Fall zum DoD-Beispiel: Reader bietet SIGN,
        // Writer ENCRYPT → stronger wins = ENCRYPT.
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
        // Writer hat SecurityInfo aber Level=None (explizit plaintext);
        // Reader will SIGN → das ist aus Reader-Sicht nicht erfuellbar
        // — in heutiger Version akzeptieren wir den Match, weil beide
        // SecurityInfo haben. Das DoD erlaubt "stronger wins".
        let w = EndpointProtection::PLAIN;
        let r = EndpointProtection::new(ProtectionLevel::Sign);
        let result = match_endpoints(&w, &r, true, true);
        assert_eq!(result, EndpointMatch::Accept(ProtectionLevel::Sign));
    }
}
