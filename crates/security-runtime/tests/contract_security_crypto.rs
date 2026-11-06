// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with `zerodds-security-crypto`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security-crypto`, from its non-test
//! sources:
//! - `Suite` (`engine.rs`, `policy.rs`, `profile.rs`): `Suite::default()`,
//!   the four variants, `with_key_bits` (SIGN gives the GMAC suite, the
//!   other kinds the GCM suite, of the configured width).
//! - `AesGcmCryptoPlugin` (`profile.rs`): `with_secret_provider`,
//!   `set_max_blocks_per_session`, `set_local_protection_suites`, handed to
//!   the gate as `Box<dyn CryptographicPluginInterfaces>`.
//! - `symmetric_cipher_key_bits` (`qos_properties.rs`,
//!   `plugin_attributes.rs`): the width of
//!   `dds.sec.crypto.symmetric_cipher_algorithm`.
//! - `PROP_ORIGIN_AUTHENTICATION_FALLBACK` (`shared.rs`): set to `accept`
//!   in the properties of the participant configuration.
//! - `psk_rtps` (`qos_properties.rs`, `psk_gate.rs`): `PROP_PASSPHRASE`,
//!   `PROP_PASSPHRASE_TABLE_76`, `ALTERNATIVE_PASSPHRASE_PROPERTIES`, `PROP_CIPHER`,
//!   `Passphrase::parse`, `parse_passphrase_list`, `key_bits_from_cipher`,
//!   `PskProtection`, `PskConfig` (built field by field) and
//!   `PskCryptoPlugin::new`, `encode_rtps_message_derived`,
//!   `decode_rtps_message`.
//!
//! What it relies on: the cipher names and the defaults of Table 71 and
//! Table 86, the passphrase rules of Table 86, the key material of the
//! pre-shared key as §10.5.2.1.3 derives it, a protected message whose
//! SRTPS_PREFIX carries the flags A and P (the gate tells PSK messages by
//! P) and the CryptoHeader of that key material, and a plugin that refuses
//! a property value it does not know.
//!
//! Oracles: DDS-Security 1.2 §7.3.2 (vendor property names), §7.3.18 and
//! §10.5.2.3 (the CryptoHeader: transformation_kind = key revision and
//! algorithm id, key id, session id, IV suffix), §7.4.7.8 (the SRTPS_PREFIX
//! flags E, A, P), §8.1 Table 22 (the `CryptoAlgorithmId`s 0x01 to 0x04),
//! §10.5.1 Table 71 and §10.9.1 Table 86 (property names, "AUTO" is
//! "AES256-GCM", the passphrase rules and the examples `5612:Open Sesame`
//! and the three-line `_alt` list), §10.5.2.1.2 Table 73 (the Kx key is
//! AES256-GCM), §10.5.2.1.3 Tables 75 and 76 with the worked example of the
//! specification (passphrase 5632, DomainId 201, master_salt `a4ebff57...`,
//! master_sender_key `4708460a...`; recomputed with Python `hmac`), §10.5.2.2
//! Table 77 (16 and 32 octets of key), and the rustdoc of
//! `PROP_ORIGIN_AUTHENTICATION_FALLBACK` (`spec` or `accept`) and
//! `PskCryptoPlugin::new` (`key_bits` 128 or 256).
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::{CBIT_AES128_GCM, CBIT_AES256_GCM};
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security::cryptographic::{
    CryptographicPluginInterfaces, PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
    ParticipantSecurityConfig,
};
use zerodds_security::error::SecurityErrorKind;
use zerodds_security::properties::Property;
use zerodds_security_crypto::psk_rtps::{
    ALTERNATIVE_PASSPHRASE_PROPERTIES, PROP_CIPHER, PROP_PASSPHRASE, PROP_PASSPHRASE_TABLE_76,
    Passphrase, PskConfig, PskCryptoPlugin, PskProtection, derive_key_material, domain_key_prefix,
    key_bits_from_cipher, parse_passphrase_list,
};
use zerodds_security_crypto::{
    AesGcmCryptoPlugin, PROP_ORIGIN_AUTHENTICATION_FALLBACK, Suite, symmetric_cipher_key_bits,
};

/// The passphrase of the example of §10.5.2.1.3.
const EXAMPLE: &str = "5632:castle super radar denial swing lunar kind swarm wet toilet output \
                       harbor basic begin margin huge year visit";

/// The GuidPrefix of the example of §10.5.2.1.3.
const EXAMPLE_PREFIX: [u8; 12] = [
    0xDF, 0xCD, 0x91, 0xE1, 0x68, 0x68, 0x04, 0x51, 0x6C, 0xB1, 0xB6, 0x0E,
];

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn security_10_5_1_the_cipher_names_and_their_key_widths() {
    // Table 71 and Table 86: no value or "AUTO" is "AES256-GCM"; the names
    // of Table 22 choose the width.
    for value in [None, Some("AUTO"), Some("AES256+GCM"), Some("AES256-GCM")] {
        assert_eq!(
            symmetric_cipher_key_bits("p", value).unwrap(),
            256,
            "{value:?}"
        );
        assert_eq!(key_bits_from_cipher(value).unwrap(), 256, "{value:?}");
    }
    for value in [Some("AES128+GCM"), Some("AES128-GCM")] {
        assert_eq!(
            symmetric_cipher_key_bits("p", value).unwrap(),
            128,
            "{value:?}"
        );
        assert_eq!(key_bits_from_cipher(value).unwrap(), 128, "{value:?}");
    }
    for value in [Some("AES192+GCM"), Some("DES"), Some("")] {
        assert_eq!(
            symmetric_cipher_key_bits("p", value).unwrap_err().kind,
            SecurityErrorKind::InvalidConfiguration,
            "{value:?}"
        );
    }
}

#[test]
fn security_8_1_the_suites_by_protection_kind_and_width() {
    // `profile.rs`: SIGN gives the GMAC suite, the other kinds the GCM
    // suite, at the width of the property.
    assert_eq!(Suite::Aes256Gmac.with_key_bits(128), Suite::Aes128Gmac);
    assert_eq!(Suite::Aes256Gmac.with_key_bits(256), Suite::Aes256Gmac);
    assert_eq!(Suite::Aes256Gcm.with_key_bits(128), Suite::Aes128Gcm);
    assert_eq!(Suite::Aes256Gcm.with_key_bits(256), Suite::Aes256Gcm);
    // "AUTO" is AES256-GCM.
    assert_eq!(Suite::default(), Suite::Aes256Gcm);
    // §8.1: CID_AES128_GMAC 0x01, CID_AES128_GCM 0x02, CID_AES256_GMAC 0x03,
    // CID_AES256_GCM 0x04; Table 77: 16 or 32 octets of key; GMAC does not
    // encrypt.
    for (suite, id, key, aead) in [
        (Suite::Aes128Gmac, 0x01, 16, false),
        (Suite::Aes128Gcm, 0x02, 16, true),
        (Suite::Aes256Gmac, 0x03, 32, false),
        (Suite::Aes256Gcm, 0x04, 32, true),
    ] {
        assert_eq!(suite.transform_kind_id(), id, "{suite:?}");
        assert_eq!(suite.transform_kind(), [0, 0, 0, id], "{suite:?}");
        assert_eq!(suite.key_len(), key, "{suite:?}");
        assert_eq!(suite.is_aead(), aead, "{suite:?}");
        assert_eq!(Suite::from_transform_kind_id(id), Some(suite));
    }
}

#[test]
fn security_10_5_2_1_3_the_key_material_of_the_example_of_the_specification() {
    let passphrase = Passphrase::parse(EXAMPLE).unwrap();
    // "PassphraseId = 5632 = 0x1600", "PassphraseKeyId = 0x00",
    // "PassphraseKeyRevision = {0x00, 0x00, 0x16}".
    assert_eq!(passphrase.id, 5632);
    assert_eq!(passphrase.key_id(), 0x00);
    assert_eq!(passphrase.revision(), [0x00, 0x00, 0x16]);
    // sha256("DomainId=201;DomainTag=") = 61c853ac...
    assert_eq!(domain_key_prefix(201, ""), [0x61, 0xC8, 0x53]);

    let m = derive_key_material(
        201,
        "",
        &passphrase,
        256,
        [0x02, 0x05],
        [0x01, 0x01],
        EXAMPLE_PREFIX,
    );
    assert_eq!(m.sender_key_id, [0x61, 0xC8, 0x53, 0x00]);
    assert_eq!(m.revision, [0x00, 0x00, 0x16]);
    assert_eq!(
        m.master_salt,
        unhex("a4ebff5738dc6826c8d3f5e55a24bb96d9e80147b51c4e49a0927c4fa2cfec8c")
    );
    assert_eq!(
        m.master_sender_key,
        unhex("4708460adc6bb886521fbdc4b3a9d34e27eed36c162ccdf4fb7427a7f347738b")
    );
    // Table 75: the AES128 variants take the first 128 bits.
    let short = derive_key_material(
        201,
        "",
        &passphrase,
        128,
        [0x02, 0x05],
        [0x01, 0x01],
        EXAMPLE_PREFIX,
    );
    assert_eq!(short.master_salt, m.master_salt[..16]);
    assert_eq!(short.master_sender_key, m.master_sender_key[..16]);
}

#[test]
fn security_10_9_1_the_passphrases_of_table_86() {
    // The names of Table 86 ("dds.sec.crypto." prefix) and of Table 76.
    assert_eq!(PROP_PASSPHRASE, "dds.sec.crypto.rtps_psk_secret_passphrase");
    assert_eq!(
        PROP_CIPHER,
        "dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm"
    );
    assert_eq!(
        PROP_PASSPHRASE_TABLE_76,
        "dds.sec.crypto.psk.secret_passphrase"
    );
    // The alternatives: Table 86 and Table 71 first, then the names read
    // before.
    assert_eq!(
        ALTERNATIVE_PASSPHRASE_PROPERTIES[..2],
        [
            "dds.sec.crypto.rtps_psk_secret_passphrase_alt",
            "dds.sec.crypto.rtps_psk_secret_passphrase_extra"
        ]
    );

    // "Here the passphrase_id is 5612 and the passphrase is 'Open Sesame'".
    let p = Passphrase::parse("5612:Open Sesame").unwrap();
    assert_eq!((p.id, p.secret.as_str()), (5612, "Open Sesame"));
    // 5612 = 0x15EC: PassphraseKeyId 0xEC, revision {0x00, 0x00, 0x15}.
    assert_eq!((p.key_id(), p.revision()), (0xEC, [0x00, 0x00, 0x15]));
    assert_eq!(
        Passphrase::parse("4294967040:x").unwrap().id,
        u32::MAX - 0xFF
    );

    let at_most_512 = format!("1:{}", "a".repeat(512));
    assert!(Passphrase::parse(&at_most_512).is_ok());
    let too_long = format!("1:{}", "a".repeat(513));
    for bad in [
        "255:reserved", // passphrase_id & 0xFF == 0xFF is reserved
        "511:reserved", // the same low octet
        "4294967296:x", // above 2^32-1
        "-1:x",         // not a decimal number
        "12 :x",        // not a decimal number
        "1: leading",   // a space first
        "1:trailing ",  // a space last
        "1:tab\there",  // character 9
        "1:del\u{7f}",  // character 127
        "1:",           // no passphrase
        "no colon",     // no passphrase_id
        too_long.as_str(),
    ] {
        assert_eq!(
            Passphrase::parse(bad).unwrap_err().kind,
            SecurityErrorKind::InvalidConfiguration,
            "{bad:?}"
        );
    }

    // The `_alt` example of Table 86, with LF, CR LF and CR between them.
    let list =
        parse_passphrase_list("5613:ExtraSecretPassphase\n5614:AnotherSecretPassphase\r\n5615:YetAnotherSecretPassphase\r")
            .unwrap();
    let ids: Vec<u32> = list.iter().map(|p| p.id).collect();
    assert_eq!(ids, [5613, 5614, 5615]);
}

/// The RTPS Header of the example of §10.5.2.1.3: version 2.5, vendor
/// 01 01, its GuidPrefix.
fn example_message() -> Vec<u8> {
    let mut m = vec![b'R', b'T', b'P', b'S', 0x02, 0x05, 0x01, 0x01];
    m.extend_from_slice(&EXAMPLE_PREFIX);
    // INFO_TS (0x09, E, 8 octets) as the body.
    m.extend_from_slice(&[0x09, 0x01, 0x08, 0x00, 1, 0, 0, 0, 2, 0, 0, 0]);
    m
}

fn config(passphrase: &str, alternatives: &[&str], protection: PskProtection) -> PskConfig {
    PskConfig {
        domain_id: 201,
        domain_tag: String::new(),
        passphrase: Passphrase::parse(passphrase).unwrap(),
        alternatives: alternatives
            .iter()
            .map(|p| Passphrase::parse(p).unwrap())
            .collect(),
        key_bits: 256,
        protection,
    }
}

#[test]
fn security_10_9_3_a_message_protected_with_the_pre_shared_key() {
    let sender = PskCryptoPlugin::new(config(EXAMPLE, &[], PskProtection::Encrypt)).unwrap();
    let message = example_message();
    let wire = sender.encode_rtps_message_derived(&message).unwrap();

    // The RTPS Header stays; the SRTPS_PREFIX follows it.
    assert_eq!(wire[..20], message[..20]);
    assert_eq!(wire[20], 0x33);
    let flags = wire[21];
    // §7.4.7.8: A (0x02) and P (0x04); `psk_gate.rs` tells a PSK message by P.
    assert_eq!(flags & 0x06, 0x06);
    let len = if flags & 0x01 != 0 {
        u16::from_le_bytes([wire[22], wire[23]])
    } else {
        u16::from_be_bytes([wire[22], wire[23]])
    };
    // §10.5.2.3: the CryptoHeader has 20 octets.
    assert_eq!(len, 20);
    // transformation_kind = {PassphraseKeyRevision, CID_AES256_GCM},
    // transformation_key_id = SenderKeyId of the example.
    assert_eq!(wire[24..28], [0x00, 0x00, 0x16, 0x04]);
    assert_eq!(wire[28..32], [0x61, 0xC8, 0x53, 0x00]);
    // The body is encrypted.
    let timestamp = &message[24..32];
    assert!(!wire[44..].windows(8).any(|w| w == timestamp));

    // A receiver with the passphrase as its own or as an alternative opens
    // it; one without it does not.
    let receiver = PskCryptoPlugin::new(config(EXAMPLE, &[], PskProtection::Encrypt)).unwrap();
    assert_eq!(receiver.decode_rtps_message(&wire).unwrap(), message);
    let rotating =
        PskCryptoPlugin::new(config("5888:next", &[EXAMPLE], PskProtection::Encrypt)).unwrap();
    assert_eq!(rotating.decode_rtps_message(&wire).unwrap(), message);
    let stranger = PskCryptoPlugin::new(config("5888:next", &[], PskProtection::Encrypt)).unwrap();
    assert!(stranger.decode_rtps_message(&wire).is_err());

    // SIGN protects with AES256-GMAC (CID 0x03).
    let signer = PskCryptoPlugin::new(config(EXAMPLE, &[], PskProtection::Sign)).unwrap();
    let signed = signer.encode_rtps_message_derived(&message).unwrap();
    assert_eq!(signed[24..28], [0x00, 0x00, 0x16, 0x03]);

    // Rustdoc of `new`: key_bits is 128 or 256.
    let mut odd = config(EXAMPLE, &[], PskProtection::Encrypt);
    odd.key_bits = 192;
    assert_eq!(
        PskCryptoPlugin::new(odd).err().map(|e| e.kind),
        Some(SecurityErrorKind::InvalidConfiguration)
    );
}

/// A provider without secrets: the plugin only needs one to exist.
struct NoSecrets;

impl SharedSecretProvider for NoSecrets {
    fn get_shared_secret(&self, _handle: SharedSecretHandle) -> Option<Vec<u8>> {
        None
    }
}

#[test]
fn security_10_5_2_1_2_the_plugin_of_the_profile_requires_aes256_gcm_for_the_kx_key() {
    let provider: Arc<dyn SharedSecretProvider> = Arc::new(NoSecrets);
    let mut plugin =
        AesGcmCryptoPlugin::with_secret_provider(Suite::Aes256Gcm.with_key_bits(128), provider);
    plugin.set_max_blocks_per_session(1 << 20);
    plugin.set_local_protection_suites(Some(Suite::Aes128Gcm), Some(Suite::Aes128Gcm), None);
    assert_eq!(plugin.local_suite(), Suite::Aes128Gcm);
    // `profile.rs` hands it to the gate as one object.
    let mut plugin: Box<dyn CryptographicPluginInterfaces> = Box::new(plugin);

    // §7.3.2: a vendor name; `shared.rs` sets it to `accept`.
    assert!(PROP_ORIGIN_AUTHENTICATION_FALLBACK.starts_with("org.zerodds."));
    let participant_config = |fallback: &str| ParticipantSecurityConfig {
        is_rtps_axk_protected: true,
        plugin_participant_attributes: PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
        ac_endpoint_properties: vec![
            Property::local("dds.sec.crypto.symmetric_cipher_algorithm", "AES128+GCM"),
            Property::local(PROP_ORIGIN_AUTHENTICATION_FALLBACK, fallback.to_string()),
        ],
        ..ParticipantSecurityConfig::default()
    };
    let (handle, info) = plugin
        .register_local_participant(
            IdentityHandle(1),
            PermissionsHandle(1),
            &[],
            &participant_config("accept"),
        )
        .unwrap();
    assert!(!handle.is_nil());
    // Table 73: the Kx key is AES256-GCM, whatever the width of the rest.
    assert_eq!(
        info.symmetric_cipher.builtin_kx_endpoints_required_mask,
        CBIT_AES256_GCM
    );
    assert_ne!(info.symmetric_cipher.supported_mask & CBIT_AES128_GCM, 0);
    // Rustdoc of the property: `spec` or `accept`, nothing else.
    assert_eq!(
        plugin
            .register_local_participant(
                IdentityHandle(1),
                PermissionsHandle(1),
                &[],
                &participant_config("sometimes"),
            )
            .unwrap_err()
            .kind,
        SecurityErrorKind::InvalidConfiguration
    );
}
