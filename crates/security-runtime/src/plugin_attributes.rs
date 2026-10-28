// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The security attributes this participant announces, and the reading of
//! those a peer announces.
//!
//! DDS-Security 1.2 §7.3.23 and §7.3.24: the `plugin_*_security_attributes`
//! masks "shall be defined by the security plugin implementation and are
//! opaque to the DDS middleware (other than the is_valid bit)". The meaning of
//! their bits is that of this plugin (§10.4.2.3); the DDS middleware
//! (`zerodds-dcps`) asks this module for the masks to announce and for what
//! a peer's mask means, and does not look at the bits.

use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
use zerodds_rtps::participant_security_info::{
    ParticipantSecurityAttributesMaskExt, ParticipantSecurityInfo, attrs as participant_attrs,
    plugin_attrs as participant_plugin_attrs,
};

use crate::{EndpointProtection, ProtectionLevel, SharedSecurityGate};

/// The protection level the plugin reads from the `EndpointSecurityInfo` a
/// peer announced for one of its endpoints: the middleware does not interpret
/// the plugin bits (§7.3.24), this plugin does. `None`: the endpoint is plain.
#[must_use]
pub fn protection_level_of(info: &EndpointSecurityInfo) -> ProtectionLevel {
    EndpointProtection::from_info(Some(info)).level
}

/// The `required_mask` of an endpoint's `EndpointSecuritySymmetricCipherAlgorithmInfo`
/// (DDS-Security 1.2 §7.3.15): the algorithm the endpoint's own
/// `dds.sec.crypto.symmetric_cipher_algorithm` names (§10.5.1.1), as the bit of
/// §8.1 (`CBIT_AES128_*` or `CBIT_AES256_*`); with "AUTO", or none, the
/// width of the DomainParticipant (`participant_bits`, 128 or 256), which is
/// its `user_endpoints_default_required_mask` (§7.3.13.2).
///
/// # Errors
/// `InvalidConfiguration` for a value outside Table 22.
pub fn endpoint_cipher_required_mask(
    value: Option<&str>,
    participant_bits: u16,
) -> Result<u32, zerodds_security::error::SecurityError> {
    use zerodds_rtps::security_algo_info::symmetric_bit;
    let bits = match value.map(str::trim) {
        None | Some("AUTO") => participant_bits,
        other => zerodds_security_crypto::symmetric_cipher_key_bits(
            crate::qos_properties::PROP_SYMMETRIC_CIPHER,
            other,
        )?,
    };
    Ok(if bits == 128 {
        symmetric_bit::AES128
    } else {
        symmetric_bit::AES256
    })
}

/// The `ParticipantSecurityInfo` this participant announces
/// (`PID_PARTICIPANT_SECURITY_INFO`, §7.3.23): the protection kinds of the
/// domain's governance as `is_*_protected` bits in the participant mask and as
/// `is_*_encrypted` and `*_origin_authenticated` bits in the plugin mask
/// (§9.4.2.4, §10.4.2.3). Without a gate both masks hold only the valid bit.
#[must_use]
pub fn participant_security_info(gate: Option<&SharedSecurityGate>) -> ParticipantSecurityInfo {
    use participant_attrs as attrs;
    use participant_plugin_attrs as plugin_attrs;
    let (mut a, mut p) = (attrs::IS_VALID, plugin_attrs::IS_VALID);
    let mut optional = ParticipantSecurityAttributesMaskExt::default();
    if let Some(gate) = gate {
        // §9.4.2.9.22 `get_participant_security_config`: the AccessControl
        // plugin says what the participant protects and how (Tables 33, 34,
        // 35, 67).
        if let Ok(config) = gate.participant_security_config() {
            a = config.attributes_mask();
            p |= config.plugin_participant_attributes;
            if gate.sends_optional_attributes().unwrap_or(true) {
                let (is_set, value) = config.optional_attributes_mask();
                optional = ParticipantSecurityAttributesMaskExt { is_set, value };
            }
        } else {
            // No config (a governance without a rule for the domain): what
            // the governance alone says.
            let rtps = gate.rtps_protection().unwrap_or(ProtectionLevel::None);
            let disc = gate.discovery_protection().unwrap_or(ProtectionLevel::None);
            let live = gate
                .liveliness_protection()
                .unwrap_or(ProtectionLevel::None);
            if rtps != ProtectionLevel::None {
                a |= attrs::IS_RTPS_AXK_PROTECTED;
            }
            if disc != ProtectionLevel::None {
                a |= attrs::IS_DISCOVERY_PROTECTED;
            }
            if live != ProtectionLevel::None {
                a |= attrs::IS_LIVELINESS_PROTECTED;
            }
            if rtps == ProtectionLevel::Encrypt {
                p |= plugin_attrs::IS_RTPS_AXK_ENCRYPTED;
            }
            if disc == ProtectionLevel::Encrypt {
                p |= plugin_attrs::IS_DISCOVERY_ENCRYPTED;
            }
            if live == ProtectionLevel::Encrypt {
                p |= plugin_attrs::IS_LIVELINESS_ENCRYPTED;
            }
            if gate.rtps_origin_authenticated().unwrap_or(false) {
                p |= plugin_attrs::IS_RTPS_ORIGIN_AUTHENTICATED;
            }
            if gate.discovery_origin_authenticated().unwrap_or(false) {
                p |= plugin_attrs::IS_DISCOVERY_ORIGIN_AUTHENTICATED;
            }
            if gate.liveliness_origin_authenticated().unwrap_or(false) {
                p |= plugin_attrs::IS_LIVELINESS_ORIGIN_AUTHENTICATED;
            }
        }
    }
    ParticipantSecurityInfo {
        participant_security_attributes: a,
        plugin_participant_security_attributes: p,
        optional_attributes: optional,
    }
}

/// Computes the user-endpoint `EndpointSecurityInfo` mask from the governance
/// protection kinds (DDS-Security 1.2 §10.4.1.2.6 / §9.4.2.4). The wire mask
/// MUST match cyclone/FastDDS/OpenDDS byte-exactly, otherwise the peer rejects
/// the endpoint match with "security_attributes mismatch".
///
/// - metadata=SIGN/ENCRYPT → IS_SUBMESSAGE_PROTECTED (+ plugin SUBMESSAGE_ENCRYPTED on ENCRYPT)
/// - data=SIGN    → IS_PAYLOAD_PROTECTED
/// - data=ENCRYPT → IS_PAYLOAD_PROTECTED | **IS_KEY_PROTECTED** (+ plugin PAYLOAD_ENCRYPTED)
/// - topic enable_liveliness_protection → **IS_LIVELINESS_PROTECTED** (per endpoint)
/// - topic enable_discovery_protection → IS_DISCOVERY_PROTECTED
///
/// is_key_protected follows §10.4.1.2.6 exclusively from the **DATA** protection
/// and only on ENCRYPT — NOT from the metadata protection. is_liveliness_protected
/// in contrast MUST be on every user endpoint as soon as liveliness_protection is active;
/// cyclone compares the mask at endpoint match and otherwise rejects with
/// "security_attributes mismatch" (0x..30 vs 0x..70).
#[must_use]
#[allow(clippy::fn_params_excessive_bools)]
pub fn endpoint_security_info(
    meta: ProtectionLevel,
    data: ProtectionLevel,
    discovery_protected: bool,
    liveliness_protected: bool,
    read_protected: bool,
    write_protected: bool,
    submessage_origin_authenticated: bool,
) -> EndpointSecurityInfo {
    let mut a = attrs::IS_VALID;
    let mut p = plugin_attrs::IS_VALID;
    if read_protected {
        a |= attrs::IS_READ_PROTECTED;
    }
    if write_protected {
        a |= attrs::IS_WRITE_PROTECTED;
    }
    if meta != ProtectionLevel::None {
        a |= attrs::IS_SUBMESSAGE_PROTECTED;
    }
    if meta == ProtectionLevel::Encrypt {
        p |= plugin_attrs::IS_SUBMESSAGE_ENCRYPTED;
    }
    // DDS-Security 1.2 §10.4.1.2.6.6, Table 69: the Metadata Protection Kind
    // of the rule is a `*_WITH_ORIGIN_AUTHENTICATION` kind.
    if submessage_origin_authenticated {
        p |= plugin_attrs::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED;
    }
    if data != ProtectionLevel::None {
        a |= attrs::IS_PAYLOAD_PROTECTED;
    }
    if data == ProtectionLevel::Encrypt {
        a |= attrs::IS_KEY_PROTECTED;
        p |= plugin_attrs::IS_PAYLOAD_ENCRYPTED;
    }
    if discovery_protected {
        a |= attrs::IS_DISCOVERY_PROTECTED;
    }
    if liveliness_protected {
        a |= attrs::IS_LIVELINESS_PROTECTED;
    }
    EndpointSecurityInfo {
        endpoint_security_attributes: a,
        plugin_endpoint_security_attributes: p,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn mask(meta: ProtectionLevel, data: ProtectionLevel) -> u32 {
        endpoint_security_info(meta, data, false, false, false, false, false)
            .endpoint_security_attributes
    }

    fn mask_liv(meta: ProtectionLevel, data: ProtectionLevel) -> u32 {
        endpoint_security_info(meta, data, false, true, false, false, false)
            .endpoint_security_attributes
    }

    #[test]
    fn liveliness_protected_sets_0x40_per_spec_9_4_1_3() {
        use ProtectionLevel::{Encrypt, None};
        let v = attrs::IS_VALID;
        let pay = attrs::IS_PAYLOAD_PROTECTED;
        let key = attrs::IS_KEY_PROTECTED;
        let liv = attrs::IS_LIVELINESS_PROTECTED;
        // liveliness=ENCRYPT + data=ENCRYPT → 0x..70 (cyclone's value at match).
        assert_eq!(mask_liv(None, Encrypt), v | pay | key | liv);
        // without liveliness → 0x..30, NO 0x40.
        assert_eq!(mask(None, Encrypt), v | pay | key);
        assert_eq!(mask_liv(None, None), v | liv);
    }

    #[test]
    fn key_protected_follows_data_encrypt_per_spec_10_4_1_2_6() {
        use ProtectionLevel::{Encrypt, None, Sign};
        let v = attrs::IS_VALID;
        let sub = attrs::IS_SUBMESSAGE_PROTECTED;
        let pay = attrs::IS_PAYLOAD_PROTECTED;
        let key = attrs::IS_KEY_PROTECTED;
        // §10.4.1.2.6: is_key_protected follows ONLY data=ENCRYPT.
        // data=ENCRYPT → PAYLOAD|KEY (= cyclone's 0x30 in the common subset).
        assert_eq!(mask(None, Encrypt), v | pay | key);
        // data=SIGN → PAYLOAD, NO KEY.
        assert_eq!(mask(None, Sign), v | pay);
        // data=NONE → no payload/key bits.
        assert_eq!(mask(None, None), v);
        // KEY does NOT depend on metadata: meta=ENCRYPT/data=NONE → only SUBMESSAGE.
        assert_eq!(mask(Encrypt, None), v | sub);
        // meta=ENCRYPT + data=ENCRYPT → SUBMESSAGE|PAYLOAD|KEY (0x38).
        assert_eq!(mask(Encrypt, Encrypt), v | sub | pay | key);
    }

    /// §7.3.24: what the plugin announces for an endpoint, it reads back as the
    /// same protection level; the middleware passes the masks on and looks at
    /// the valid bit only.
    #[test]
    fn the_plugin_reads_back_what_it_announces() {
        for level in [
            ProtectionLevel::None,
            ProtectionLevel::Sign,
            ProtectionLevel::Encrypt,
        ] {
            let info = endpoint_security_info(level, level, false, false, false, false, false);
            assert!(info.is_valid());
            assert_eq!(protection_level_of(&info), level, "{level:?}");
        }
    }

    /// §7.3.23: without a gate both masks hold the valid bit only; with the
    /// governance of the domain they hold its protection kinds.
    #[test]
    fn the_participant_masks_follow_the_governance() {
        use participant_attrs as attrs;
        use participant_plugin_attrs as plugin_attrs;
        let plain = participant_security_info(None);
        assert_eq!(plain.participant_security_attributes, attrs::IS_VALID);
        assert_eq!(
            plain.plugin_participant_security_attributes,
            plugin_attrs::IS_VALID
        );
        let gate = SharedSecurityGate::new(
            0,
            zerodds_security_permissions::parse_governance_xml(
                "<dds><domain_access_rules><domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><discovery_protection_kind>ENCRYPT</discovery_protection_kind><rtps_protection_kind>SIGN</rtps_protection_kind><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>",
            )
            .unwrap(),
            Box::new(zerodds_security_crypto::AesGcmCryptoPlugin::new()),
        );
        let info = participant_security_info(Some(&gate));
        assert_eq!(
            info.participant_security_attributes,
            attrs::IS_VALID | attrs::IS_RTPS_AXK_PROTECTED | attrs::IS_DISCOVERY_PROTECTED
        );
        assert_eq!(
            info.plugin_participant_security_attributes,
            plugin_attrs::IS_VALID | plugin_attrs::IS_DISCOVERY_ENCRYPTED
        );
    }

    /// §7.3.15: the required mask of an endpoint is the algorithm it names, or
    /// the participant's width for AUTO and none; a name outside Table 22 is
    /// refused.
    #[test]
    fn an_endpoint_requires_the_algorithm_it_names() {
        use zerodds_rtps::security_algo_info::symmetric_bit;
        let mask = |value, bits| endpoint_cipher_required_mask(value, bits).unwrap();
        assert_eq!(mask(None, 256), symmetric_bit::AES256);
        assert_eq!(mask(Some("AUTO"), 128), symmetric_bit::AES128);
        assert_eq!(mask(Some("AES128+GCM"), 256), symmetric_bit::AES128);
        assert_eq!(mask(Some("AES128-GMAC"), 256), symmetric_bit::AES128);
        assert_eq!(mask(Some("AES256-GMAC"), 128), symmetric_bit::AES256);
        assert!(endpoint_cipher_required_mask(Some("ROT13"), 256).is_err());
    }
}
