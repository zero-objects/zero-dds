// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DDS:Crypto:PSK` in the gate: the protection of whole RTPS messages with a
//! pre-shared key (DDS-Security 1.2 §9.4.2.4 `is_rtps_psk_protected`,
//! §10.9).
//!
//! "If is_rtps_psk_protected is TRUE all RTPS messages will be
//! cryptographically protected by either a pre-shared key or a
//! 'Autenticated Partcipant Exchanged Key.'": the messages the exchanged key
//! does not protect, which are the RTPS Bootstrapping Messages (§7.5.7) when
//! `rtps_protection_kind` is not NONE and every message otherwise, go through
//! `encode_rtps_message` with `transform_with_psk` TRUE, and the messages
//! with the flag P in their SRTPS_PREFIX through `decode_rtps_message`.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::PoisonError;

use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security_crypto::psk_rtps::{PskConfig, PskCryptoPlugin};

use crate::gate::SecurityGateError;
use crate::shared::SharedSecurityGate;

/// The pre-shared-key state of a gate.
#[derive(Default)]
pub(crate) struct PskState {
    /// The configuration without the DomainId and DomainTag, which the gate
    /// has.
    template: Option<PskConfig>,
    /// The plugin for a DomainId and DomainTag.
    built: Option<(u32, String, Arc<PskCryptoPlugin>)>,
}

fn poisoned<T>(_: PoisonError<T>) -> SecurityGateError {
    SecurityGateError::Crypto(SecurityError::new(
        SecurityErrorKind::Internal,
        "security-runtime: psk mutex poisoned",
    ))
}

/// The flags octet of the SRTPS_PREFIX of `bytes`, behind the RTPS Header and
/// the HeaderExtension, if there is one; `None` for a message without one.
fn srtps_prefix_flags(bytes: &[u8]) -> Option<u8> {
    let mut at = 20;
    if bytes.get(at) == Some(&0x00) {
        // HeaderExtension: id, flags (E), octetsToNextHeader.
        let flags = *bytes.get(at + 1)?;
        let len = [*bytes.get(at + 2)?, *bytes.get(at + 3)?];
        let len = if flags & 0x01 != 0 {
            u16::from_le_bytes(len)
        } else {
            u16::from_be_bytes(len)
        };
        at += 4 + usize::from(len);
    }
    (bytes.get(at) == Some(&0x33)).then(|| bytes.get(at + 1).copied())?
}

impl SharedSecurityGate {
    /// Enables the protection with a pre-shared key: `config` is the
    /// configuration of `DDS:Crypto:PSK` (its DomainId and DomainTag are
    /// replaced by the gate's).
    ///
    /// # Errors
    /// Mutex poison.
    pub fn set_rtps_psk(&self, config: PskConfig) -> Result<(), SecurityGateError> {
        let mut state = self.psk.lock().map_err(poisoned)?;
        state.template = Some(config);
        state.built = None;
        Ok(())
    }

    /// Whether a pre-shared key protects the RTPS messages.
    #[must_use]
    pub fn has_rtps_psk(&self) -> bool {
        self.psk.lock().is_ok_and(|state| state.template.is_some())
    }

    /// Whether `bytes` is an RTPS message protected with a pre-shared key: its
    /// SRTPS_PREFIX sets the PreSharedKeyFlag (Table 7).
    #[must_use]
    pub fn is_psk_protected(bytes: &[u8]) -> bool {
        srtps_prefix_flags(bytes).is_some_and(|flags| flags & 0x04 != 0)
    }

    fn psk_plugin(&self) -> Result<Arc<PskCryptoPlugin>, SecurityGateError> {
        let domain = self.domain_id()?;
        let tag = self.domain_tag()?;
        let mut state = self.psk.lock().map_err(poisoned)?;
        if let Some((id, t, plugin)) = &state.built {
            if *id == domain && *t == tag {
                return Ok(Arc::clone(plugin));
            }
        }
        let mut config = state.template.clone().ok_or_else(|| {
            SecurityGateError::PolicyViolation(String::from(
                "no pre-shared key is configured (rtps_psk_secret_passphrase)",
            ))
        })?;
        config.domain_id = domain;
        config.domain_tag.clone_from(&tag);
        let plugin = Arc::new(PskCryptoPlugin::new(config).map_err(SecurityGateError::Crypto)?);
        state.built = Some((domain, tag, Arc::clone(&plugin)));
        Ok(plugin)
    }

    /// `encode_rtps_message` with `transform_with_psk` TRUE: `message`
    /// protected with the pre-shared key (flags A and P).
    ///
    /// # Errors
    /// `PolicyViolation` without a configured pre-shared key; `Crypto` from
    /// the plugin.
    pub fn psk_encode(&self, message: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        self.psk_plugin()?
            .encode_rtps_message_derived(message)
            .map_err(SecurityGateError::Crypto)
    }

    /// `decode_rtps_message` of a message with the flag P.
    ///
    /// # Errors
    /// `PolicyViolation` without a configured pre-shared key; `Crypto` for a
    /// message the key does not open or whose kind is not the expected one.
    pub fn psk_decode(&self, message: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        self.psk_plugin()?
            .decode_rtps_message(message)
            .map_err(SecurityGateError::Crypto)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_crypto::psk_rtps::{Passphrase, PskProtection};
    use zerodds_security_permissions::parse_governance_xml;

    const GOV: &str = r#"<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"#;

    fn gate(passphrase: &str, protection: PskProtection) -> SharedSecurityGate {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        gate.set_rtps_psk(PskConfig {
            domain_id: 99,
            domain_tag: String::from("ignored"),
            passphrase: Passphrase::parse(passphrase).unwrap(),
            alternatives: Vec::new(),
            key_bits: 256,
            protection,
        })
        .unwrap();
        gate
    }

    fn message() -> Vec<u8> {
        let mut m = b"RTPS".to_vec();
        m.extend_from_slice(&[2, 5, 1, 1]);
        m.extend_from_slice(&[0x21; 12]);
        m.extend_from_slice(&[0x15, 0x01, 0x08, 0x00, 9, 9, 9, 9, 9, 9, 9, 9]);
        m
    }

    /// §10.9, Table 89 in the gate: a message goes out with the flags A and
    /// P and is read by a gate with the same passphrase, the DomainId and
    /// DomainTag being the gate's; another passphrase does not read it.
    #[test]
    fn the_gate_protects_a_message_with_the_pre_shared_key() {
        let a = gate("5612:Open Sesame", PskProtection::Encrypt);
        let b = gate("5612:Open Sesame", PskProtection::Encrypt);
        assert!(a.has_rtps_psk());
        let plain = message();
        let wire = a.psk_encode(&plain).unwrap();
        assert!(SharedSecurityGate::is_psk_protected(&wire));
        assert!(!SharedSecurityGate::is_psk_protected(&plain));
        assert_eq!(b.psk_decode(&wire).unwrap(), plain);
        let other = gate("5612:Another passphrase", PskProtection::Encrypt);
        assert!(other.psk_decode(&wire).is_err());
        // A tag set later changes the key: the DomainTag is part of SenderKeyId.
        b.set_domain_tag("other-tag").unwrap();
        assert!(b.psk_decode(&wire).is_err());
        a.set_domain_tag("other-tag").unwrap();
        let wire = a.psk_encode(&plain).unwrap();
        assert_eq!(b.psk_decode(&wire).unwrap(), plain);
    }

    /// Without a configured passphrase there is no pre-shared key.
    #[test]
    fn a_gate_without_a_passphrase_has_no_pre_shared_key() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!gate.has_rtps_psk());
        assert!(gate.psk_encode(&message()).is_err());
        assert!(gate.psk_decode(&message()).is_err());
    }

    /// The flag P is found behind a HeaderExtension too.
    #[test]
    fn the_flag_p_is_found_behind_a_header_extension() {
        let a = gate("5612:Open Sesame", PskProtection::Sign);
        let mut plain = message()[..20].to_vec();
        plain.extend_from_slice(&[0x00, 0x03, 0x04, 0x00, 0, 0, 0, 0]);
        plain.extend_from_slice(&message()[20..]);
        let wire = a.psk_encode(&plain).unwrap();
        assert_eq!(wire[20], 0x00, "the HeaderExtension stays in front");
        assert!(SharedSecurityGate::is_psk_protected(&wire));
        let b = gate("5612:Open Sesame", PskProtection::Sign);
        assert_eq!(b.psk_decode(&wire).unwrap().len(), plain.len());
    }
}
