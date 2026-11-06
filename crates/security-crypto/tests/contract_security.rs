// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-crypto` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `crypto::CryptographicPlugin`, implemented by `AesGcmCryptoPlugin`
//!   (`plugin.rs`, nearly every operation), `PskDerivedCryptoPlugin`
//!   (`psk_plugin.rs`) and `PskCryptoPlugin` (`psk_rtps.rs`); the last two
//!   keep the default bodies of the Kx, Cyclone-format, key-id and payload
//!   operations. `crypto::CryptoHandle` for every slot, and the property
//!   names `PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION`,
//!   `ENDPOINT_METADATA_PROTECTION_KIND` and `ENDPOINT_DATA_PROTECTION_KIND`
//!   (`plugin.rs`, `plugin/spec.rs`).
//! - `cryptographic::{CryptoKeyFactory, CryptoKeyExchange, CryptoTransform,
//!   CryptographicPluginInterfaces, VendorFormCrypto}`, implemented by
//!   `AesGcmCryptoPlugin` (`plugin/spec.rs`, with `vendor_forms`) and
//!   `PskCryptoPlugin` (`psk_rtps.rs`, default `vendor_forms`); the handle
//!   types `ParticipantCryptoHandle`, `DatawriterCryptoHandle`,
//!   `DatareaderCryptoHandle`, `GuidT`, the structures
//!   `ParticipantSecurityConfig`, `EndpointSecurityConfig`,
//!   `ParticipantSecurityAlgorithmInfo` (built with `..Default::default()`),
//!   `ParticipantSecuritySymmetricCipherAlgorithmInfo`,
//!   `EndpointSecurityAlgorithmInfo`,
//!   `EndpointSecuritySymmetricCipherAlgorithmInfo`, `EncodedPayload`,
//!   `PreprocessedSubmessage`, `SecureSubmessageCategory`, and the
//!   `PLUGIN_PARTICIPANT_FLAG_IS_RTPS_{AXK,PSK}_ENCRYPTED`,
//!   `PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED` and
//!   `PLUGIN_ENDPOINT_FLAG_IS_{SUBMESSAGE,PAYLOAD}_ENCRYPTED`,
//!   `PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED` bits.
//! - `algorithm::{CBIT_AES128_GCM, CBIT_AES256_GCM,
//!   CRYPTO_ALGORITHM_SET_EMPTY, CryptoTransformKeyRevisionIntHolder,
//!   key_revision_to_int, key_revision_from_int}` (`plugin.rs` sorts the
//!   revisions by their int holder, `plugin/spec.rs` and `psk_rtps.rs`).
//! - `authentication::{IdentityHandle, SharedSecretHandle,
//!   SharedSecretProvider}`: `plugin.rs` holds an
//!   `Arc<dyn SharedSecretProvider>` and calls `get_shared_secret` and
//!   `get_shared_secret_challenges`.
//! - `access_control::PermissionsHandle`, `properties::Property` (the
//!   `name`/`value` of the participant and endpoint properties),
//!   `token::{CryptoToken, DataHolder}` (`DataHolder::new`,
//!   `with_binary_property`, `class_id`, `binary_property`).
//! - `error::{SecurityError, SecurityErrorKind, SecurityResult}` everywhere
//!   (`rsa_pss.rs`, `suite.rs` among them).
//!
//! Oracles: DDS-Security 1.2 §7.3.2 (vendor property names do not start
//! with "dds.sec." and start with a reversed domain), §7.3.4 (DataHolder),
//! §7.3.9 (`CRYPTO_ALGORITHM_SET_EMPTY`), §7.3.17 (the int holder of a key
//! revision), §8.1 (`CBIT_AES128_GCM`, `CBIT_AES256_GCM`), §9.5.1.2 to
//! §9.5.1.4 (`HandleNIL`), §9.5.1.7 Table 41, §9.5.1.8 to §9.5.1.10 Tables
//! 42 to 44 (the operations this crate implements), §10.4.2.4 Table 67 and
//! §10.4.2.6 Table 69 (plugin attribute bits), §10.5.2.1 Table 72 (the
//! CryptoToken class and `dds.cryp.keymat`), §10.5.3 Table 78 ("all other
//! members ... zero"), and the rustdoc of `CryptographicPlugin` (the
//! "Default:" of each provided operation), of `SharedSecretProvider` and of
//! `CryptographicPluginInterfaces::vendor_forms`.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::{
    CBIT_AES128_GCM, CBIT_AES256_GCM, CRYPTO_ALGORITHM_SET_EMPTY,
    CRYPTO_TRANSFORM_KEY_REVISION_NONE, CryptoTransformKeyRevisionIntHolder, key_revision_from_int,
    key_revision_to_int,
};
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security::crypto::{
    CryptoHandle, CryptographicPlugin, ENDPOINT_DATA_PROTECTION_KIND,
    ENDPOINT_METADATA_PROTECTION_KIND, PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
};
use zerodds_security::cryptographic::{
    CryptoKeyExchange, CryptoKeyFactory, CryptoTransform, CryptographicPluginInterfaces,
    DatareaderCryptoHandle, DatawriterCryptoHandle, EncodedPayload, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, EndpointSecuritySymmetricCipherAlgorithmInfo, GuidT,
    PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
    PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED, ParticipantCryptoHandle,
    ParticipantSecurityAlgorithmInfo, ParticipantSecurityConfig,
    ParticipantSecuritySymmetricCipherAlgorithmInfo, PreprocessedSubmessage,
    SecureSubmessageCategory, VendorFormCrypto,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::Property;
use zerodds_security::token::{BinaryProperty, CryptoToken, DataHolder};

#[test]
fn security_7_3_17_the_int_holder_of_a_key_revision() {
    // §7.3.17: int_holder_value = 256*256*r[0] + 256*r[1] + r[2].
    assert_eq!(
        key_revision_to_int([0x01, 0x02, 0x03]),
        65_536 + 2 * 256 + 3
    );
    assert_eq!(key_revision_to_int([0xff, 0xff, 0xff]), 0x00ff_ffff);
    assert_eq!(key_revision_to_int(CRYPTO_TRANSFORM_KEY_REVISION_NONE), 0);
    assert_eq!(CRYPTO_TRANSFORM_KEY_REVISION_NONE, [0, 0, 0]);
    let holder: CryptoTransformKeyRevisionIntHolder = 0x0001_0203;
    assert_eq!(key_revision_from_int(holder), [0x01, 0x02, 0x03]);

    // `plugin.rs` sorts revisions by the int holder: the order is that of the
    // three octets read as a big-endian number.
    let mut revisions = [[0, 1, 0], [1, 0, 0], [0, 0, 7], [0, 0, 0]];
    revisions.sort_by_key(|r| key_revision_to_int(*r));
    assert_eq!(revisions, [[0, 0, 0], [0, 0, 7], [0, 1, 0], [1, 0, 0]]);
}

#[test]
fn security_8_1_the_aes_gcm_bits_and_the_empty_set() {
    // §8.1: CBIT_AES128_GCM = 1 << 0, CBIT_AES256_GCM = 1 << 1.
    assert_eq!(CBIT_AES128_GCM, 1 << 0);
    assert_eq!(CBIT_AES256_GCM, 1 << 1);
    // §7.3.9: CRYPTO_ALGORITHM_SET_EMPTY = 0x00000000.
    assert_eq!(CRYPTO_ALGORITHM_SET_EMPTY, 0);
}

#[test]
fn security_10_4_2_4_plugin_participant_attribute_bits() {
    // Table 67.
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED, 1 << 0);
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ENCRYPTED, 1 << 1);
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ENCRYPTED, 1 << 2);
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED, 1 << 3);
    assert_eq!(
        PLUGIN_PARTICIPANT_FLAG_IS_DISCOVERY_ORIGIN_AUTHENTICATED,
        1 << 4
    );
    assert_eq!(
        PLUGIN_PARTICIPANT_FLAG_IS_LIVELINESS_ORIGIN_AUTHENTICATED,
        1 << 5
    );
    assert_eq!(PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED, 1 << 6);
}

#[test]
fn security_10_4_2_6_plugin_endpoint_attribute_bits() {
    // Table 69.
    assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED, 1 << 0);
    assert_eq!(PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, 1 << 1);
    assert_eq!(
        PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
        1 << 2
    );
}

#[test]
fn security_9_5_1_2_handle_nil_is_zero_and_the_default() {
    // §9.5.1.2 to §9.5.1.4 and the rustdoc: `0` is `HandleNIL`.
    assert_eq!(ParticipantCryptoHandle::NIL, ParticipantCryptoHandle(0));
    assert_eq!(
        ParticipantCryptoHandle::default(),
        ParticipantCryptoHandle::NIL
    );
    assert!(ParticipantCryptoHandle::NIL.is_nil());
    assert!(!ParticipantCryptoHandle(1).is_nil());
    assert_eq!(DatawriterCryptoHandle::NIL, DatawriterCryptoHandle(0));
    assert_eq!(
        DatawriterCryptoHandle::default(),
        DatawriterCryptoHandle::NIL
    );
    assert!(DatawriterCryptoHandle::NIL.is_nil());
    assert_eq!(DatareaderCryptoHandle::NIL, DatareaderCryptoHandle(0));
    assert_eq!(
        DatareaderCryptoHandle::default(),
        DatareaderCryptoHandle::NIL
    );
    assert!(DatareaderCryptoHandle::NIL.is_nil());
    // The handles of the other plugins this crate takes.
    assert_eq!(IdentityHandle::NIL, IdentityHandle(0));
    assert_eq!(SharedSecretHandle::NIL, SharedSecretHandle(0));
    assert_eq!(PermissionsHandle::NIL, PermissionsHandle(0));
    assert!(PermissionsHandle::NIL.is_nil());
}

#[test]
fn security_10_5_3_algorithm_infos_default_to_zero_masks() {
    // `plugin/spec.rs` fills the symmetric cipher part and leaves the rest
    // at `Default` (Table 78: "all other members ... zero").
    let info = ParticipantSecurityAlgorithmInfo {
        symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo {
            supported_mask: CBIT_AES128_GCM | CBIT_AES256_GCM,
            builtin_endpoints_required_mask: 0,
            builtin_kx_endpoints_required_mask: CBIT_AES256_GCM,
            user_endpoints_default_required_mask: 0,
        },
        ..Default::default()
    };
    let ds = info.digital_signature;
    assert_eq!(
        [
            ds.trust_chain.supported_mask,
            ds.trust_chain.required_mask,
            ds.message_auth.supported_mask,
            ds.message_auth.required_mask,
            info.key_establishment.shared_secret.supported_mask,
            info.key_establishment.shared_secret.required_mask,
        ],
        [0; 6]
    );
    assert_eq!(info.symmetric_cipher.builtin_kx_endpoints_required_mask, 2);

    let endpoint = EndpointSecurityAlgorithmInfo {
        symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: CBIT_AES256_GCM,
            supported_mask: CBIT_AES128_GCM | CBIT_AES256_GCM,
        },
    };
    assert_eq!(
        EndpointSecurityAlgorithmInfo::default()
            .symmetric_cipher
            .required_mask,
        0
    );
    assert_ne!(endpoint, EndpointSecurityAlgorithmInfo::default());

    // The configurations the factory takes start out with nothing set.
    let p = ParticipantSecurityConfig::default();
    assert!(!p.is_rtps_axk_protected);
    assert!(!p.is_rtps_psk_protected);
    assert!(!p.is_discovery_protected);
    assert_eq!(p.plugin_participant_attributes, 0);
    let e = EndpointSecurityConfig::default();
    assert!(!e.is_submessage_protected);
    assert!(!e.is_payload_protected);
    assert_eq!(e.plugin_endpoint_attributes, 0);
}

#[test]
fn security_10_5_2_1_the_crypto_token_class_and_its_key_material() {
    // Table 72: class_id "DDS:Crypto:AES_GCM_GMAC", binary property
    // "dds.cryp.keymat". `plugin/spec.rs` builds the token this way.
    let keymat = vec![0x00, 0x00, 0x00, 0x02, 0xaa, 0xbb];
    let token: CryptoToken = DataHolder::new("DDS:Crypto:AES_GCM_GMAC")
        .with_binary_property("dds.cryp.keymat", keymat.clone());
    assert_eq!(token.class_id, "DDS:Crypto:AES_GCM_GMAC");
    assert_eq!(
        token.binary_property("dds.cryp.keymat"),
        Some(keymat.as_slice())
    );
    assert_eq!(token.binary_property("dds.cryp.other"), None);
    assert!(token.properties.is_empty());
    // A binary property built for a token is local by default (§7.3.3:
    // `propagate` is `@non-serialized`).
    assert_eq!(
        token.binary_properties,
        vec![BinaryProperty::new("dds.cryp.keymat", keymat)]
    );
    assert!(!token.binary_properties[0].propagate);
}

#[test]
fn security_9_5_1_10_6_the_categories_and_the_out_values_of_the_transform() {
    // Table 41: three categories.
    let all = [
        SecureSubmessageCategory::InfoSubmessage,
        SecureSubmessageCategory::DatawriterSubmessage,
        SecureSubmessageCategory::DatareaderSubmessage,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // §9.5.1.10.6: the pair of handles and the category.
    let p = PreprocessedSubmessage {
        datawriter_crypto: DatawriterCryptoHandle(5),
        datareader_crypto: DatareaderCryptoHandle::NIL,
        secure_submessage_category: SecureSubmessageCategory::DatawriterSubmessage,
    };
    assert_eq!(p.datawriter_crypto.0, 5);
    assert!(p.datareader_crypto.is_nil());
    // §9.5.1.10.1: the encoded buffer and the extra inline QoS.
    let e = EncodedPayload {
        encoded_buffer: vec![1, 2, 3],
        extra_inline_qos: Vec::new(),
    };
    assert_eq!(e.encoded_buffer, [1, 2, 3]);
    assert!(e.extra_inline_qos.is_empty());
}

#[test]
fn security_7_3_2_the_crypto_property_names_are_vendor_names() {
    // §7.3.2: names that start with "dds.sec." are the specification's;
    // a vendor starts its names with its domain reversed.
    let names = [
        PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION,
        ENDPOINT_METADATA_PROTECTION_KIND,
        ENDPOINT_DATA_PROTECTION_KIND,
    ];
    for name in names {
        assert!(!name.starts_with("dds.sec."), "{name}");
        assert!(name.starts_with("org.zerodds."), "{name}");
    }
    assert_ne!(names[0], names[1]);
    assert_ne!(names[1], names[2]);
    assert_ne!(names[0], names[2]);

    // `plugin/spec.rs` reads the participant properties by `name` and
    // `value`.
    let props = [Property::local(ENDPOINT_DATA_PROTECTION_KIND, "ENCRYPT")];
    let found = props
        .iter()
        .find(|p| p.name == ENDPOINT_DATA_PROTECTION_KIND)
        .map(|p| p.value.as_ref());
    assert_eq!(found, Some("ENCRYPT"));
    assert!(!props[0].propagate);
}

/// A plugin with the required operations of `CryptographicPlugin` only, as
/// `PskDerivedCryptoPlugin` and `PskCryptoPlugin` leave the rest to the
/// defaults. It records the two calls the defaults delegate to.
#[derive(Default)]
struct Minimal {
    tokens_for: Vec<(CryptoHandle, CryptoHandle)>,
    tokens_set: Vec<(CryptoHandle, CryptoHandle, Vec<u8>)>,
}

impl CryptographicPlugin for Minimal {
    fn register_local_participant(
        &mut self,
        _identity: IdentityHandle,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(1))
    }

    fn register_matched_remote_participant(
        &mut self,
        _local: CryptoHandle,
        _remote_identity: IdentityHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(2))
    }

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        _is_writer: bool,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(3))
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        self.tokens_for.push((local, remote));
        Ok(vec![0xc0])
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        self.tokens_set.push((local, remote, tokens.to_vec()));
        Ok(())
    }

    fn encrypt_submessage(
        &self,
        _local: CryptoHandle,
        _remote_list: &[CryptoHandle],
        plaintext: &[u8],
        _aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(plaintext.to_vec())
    }

    fn decrypt_submessage(
        &self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
        ciphertext: &[u8],
        _aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Crypto:contract"
    }
}

fn not_implemented<T>(r: SecurityResult<T>) -> bool {
    matches!(r, Err(e) if e.kind == SecurityErrorKind::NotImplemented)
}

#[test]
fn the_provided_operations_of_the_plugin_keep_their_documented_defaults() {
    let mut p = Minimal::default();
    let h = CryptoHandle(9);

    // "`None` = one key"; "Default `None`".
    assert_eq!(p.endpoint_payload_token(h), None);
    assert_eq!(p.endpoint_relay_token(h), None);
    // "Default: nothing to release."
    assert!(p.unregister_local_endpoint(h).is_ok());
    assert!(p.unregister_matched_remote_participant(h).is_ok());

    // `create_endpoint_crypto_token_for` is the participant tokens of the
    // endpoint with `CryptoHandle(0)`; `set_remote_endpoint_crypto_tokens`
    // is `set_remote_participant_crypto_tokens` with `CryptoHandle(0)`.
    assert_eq!(
        p.create_endpoint_crypto_token_for(h, [7; 16]).unwrap(),
        vec![0xc0]
    );
    assert_eq!(p.tokens_for, vec![(h, CryptoHandle(0))]);
    p.set_remote_endpoint_crypto_tokens(CryptoHandle(1), CryptoHandle(5), &[1, 2])
        .unwrap();
    assert_eq!(
        p.tokens_set,
        vec![(CryptoHandle(1), CryptoHandle(0), vec![1, 2])]
    );

    // "Default: `NotImplemented`" for the Kx, Cyclone-format, key-id, PSK
    // and payload operations; the runtime falls back on that kind.
    assert!(not_implemented(p.encode_kx_submessage(h, b"x", b"")));
    assert!(not_implemented(p.decode_kx_submessage(h, b"x", b"")));
    assert!(not_implemented(p.encode_kx_datawriter_submessage(h, b"x")));
    assert!(not_implemented(p.decode_kx_datawriter_submessage(h, b"x")));
    assert!(not_implemented(
        p.encode_data_datawriter_submessage(h, b"x")
    ));
    assert!(not_implemented(
        p.decode_data_datawriter_submessage(h, b"x")
    ));
    assert!(not_implemented(p.decode_data_by_key_id(b"x")));
    assert!(not_implemented(p.decode_data_by_key_id_from(h, b"x")));
    assert!(not_implemented(p.encode_rtps_message_cyclone(h, b"x")));
    assert!(not_implemented(p.decode_rtps_message_cyclone(b"x")));
    assert!(not_implemented(p.decode_rtps_message_cyclone_from(h, b"x")));
    assert!(not_implemented(p.encode_rtps_message_psk(h, b"x")));
    assert!(not_implemented(p.decode_rtps_message_psk(b"x")));
    assert!(not_implemented(p.encode_serialized_payload(h, b"x")));
    assert!(not_implemented(p.decode_serialized_payload(b"x")));
    assert!(not_implemented(
        p.decode_serialized_payload_by_key_id_from(h, b"x")
    ));
    assert!(not_implemented(p.decode_serialized_payload_with(h, b"x")));
    assert!(not_implemented(p.decode_serialized_payload_kx(h, b"x")));

    // The runtime holds the plugin as `Box<dyn CryptographicPlugin>`.
    let boxed: Box<dyn CryptographicPlugin> = Box::new(p);
    assert_eq!(boxed.plugin_class_id(), "DDS:Crypto:contract");
}

/// A plugin with the three interfaces of §9.5.1 (Tables 42 to 44) and the
/// vendor forms, with the signatures `plugin/spec.rs` and `psk_rtps.rs`
/// implement. Each operation answers with a fixed value. `VENDOR_FORMS`
/// says whether it offers its vendor forms, as `AesGcmCryptoPlugin` does,
/// or keeps the default body, as `PskCryptoPlugin` does.
struct Interfaces<const VENDOR_FORMS: bool>;

impl<const V: bool> CryptoKeyFactory for Interfaces<V> {
    fn register_local_participant(
        &mut self,
        _participant_identity: IdentityHandle,
        _participant_permissions: PermissionsHandle,
        _participant_properties: &[Property],
        _participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)> {
        Ok((
            ParticipantCryptoHandle(1),
            ParticipantSecurityAlgorithmInfo::default(),
        ))
    }

    fn register_matched_remote_participant(
        &mut self,
        _local_participant_crypto_handle: ParticipantCryptoHandle,
        _remote_participant_identity: IdentityHandle,
        _remote_participant_permissions: PermissionsHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        Ok(ParticipantCryptoHandle(2))
    }

    fn register_local_datawriter(
        &mut self,
        _participant_crypto: ParticipantCryptoHandle,
        _datawriter_properties: &[Property],
        _datawriter_security_config: &EndpointSecurityConfig,
        _endpoint_guid: GuidT,
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        Ok((
            DatawriterCryptoHandle(3),
            EndpointSecurityAlgorithmInfo::default(),
        ))
    }

    fn register_matched_remote_datareader(
        &mut self,
        _local_datawriter_crypto_handle: DatawriterCryptoHandle,
        _remote_participant_crypto: ParticipantCryptoHandle,
        _shared_secret: SharedSecretHandle,
        _relay_only: bool,
    ) -> SecurityResult<DatareaderCryptoHandle> {
        Ok(DatareaderCryptoHandle(4))
    }

    fn register_local_datareader(
        &mut self,
        _participant_crypto: ParticipantCryptoHandle,
        _datareader_properties: &[Property],
        _datareader_security_config: &EndpointSecurityConfig,
        _endpoint_guid: GuidT,
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        Ok((
            DatareaderCryptoHandle(5),
            EndpointSecurityAlgorithmInfo::default(),
        ))
    }

    fn register_matched_remote_datawriter(
        &mut self,
        _local_datareader_crypto_handle: DatareaderCryptoHandle,
        _remote_participant_crypt: ParticipantCryptoHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle> {
        Ok(DatawriterCryptoHandle(6))
    }

    fn revise_local_entity_keys(
        &mut self,
        _participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder> {
        Ok(0)
    }

    fn activate_key_revision(
        &mut self,
        _local_participant_crypto_handle: ParticipantCryptoHandle,
        _key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool> {
        Ok(true)
    }

    fn unregister_participant(
        &mut self,
        _participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<bool> {
        Ok(true)
    }

    fn unregister_datawriter(
        &mut self,
        _datawriter_crypto_handle: DatawriterCryptoHandle,
    ) -> SecurityResult<bool> {
        Ok(true)
    }

    fn unregister_datareader(
        &mut self,
        _datareader_crypto_handle: DatareaderCryptoHandle,
    ) -> SecurityResult<bool> {
        Ok(true)
    }
}

impl<const V: bool> CryptoKeyExchange for Interfaces<V> {
    fn create_local_participant_crypto_tokens(
        &mut self,
        _local_participant_crypto: ParticipantCryptoHandle,
        _remote_participant_crypto: ParticipantCryptoHandle,
        _key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        Ok(None)
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local_participant_crypto: ParticipantCryptoHandle,
        _remote_participant_crypto: ParticipantCryptoHandle,
        _remote_participant_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn create_local_datawriter_crypto_tokens(
        &mut self,
        _local_datawriter_crypto: DatawriterCryptoHandle,
        _remote_datareader_crypto: DatareaderCryptoHandle,
        _key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        Ok(None)
    }

    fn set_remote_datawriter_crypto_tokens(
        &mut self,
        _local_datareader_crypto: DatareaderCryptoHandle,
        _remote_datawriter_crypto: DatawriterCryptoHandle,
        _remote_datawriter_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn create_local_datareader_crypto_tokens(
        &mut self,
        _local_datareader_crypto: DatareaderCryptoHandle,
        _remote_datawriter_crypto: DatawriterCryptoHandle,
        _key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        Ok(None)
    }

    fn set_remote_datareader_crypto_tokens(
        &mut self,
        _local_datawriter_crypto: DatawriterCryptoHandle,
        _remote_datareader_crypto: DatareaderCryptoHandle,
        _remote_datareader_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn return_crypto_tokens(&mut self, crypto_tokens: Vec<CryptoToken>) -> SecurityResult<bool> {
        if crypto_tokens.is_empty() {
            Ok(true)
        } else {
            Err(SecurityError::bad_argument("not a token of this plugin"))
        }
    }
}

impl<const V: bool> CryptoTransform for Interfaces<V> {
    fn encode_serialized_payload(
        &self,
        plain_buffer: &[u8],
        _sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>> {
        Ok(Some(EncodedPayload {
            encoded_buffer: plain_buffer.to_vec(),
            extra_inline_qos: Vec::new(),
        }))
    }

    fn encode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &mut Vec<u8>,
        plain_rtps_submessage: &[u8],
        _sending_datawriter_crypto: DatawriterCryptoHandle,
        receiving_datareader_crypto_list: &[DatareaderCryptoHandle],
        receiving_datareader_crypto_list_index: &mut usize,
    ) -> SecurityResult<bool> {
        encoded_rtps_submessage.extend_from_slice(plain_rtps_submessage);
        *receiving_datareader_crypto_list_index = receiving_datareader_crypto_list.len();
        Ok(true)
    }

    fn encode_datareader_submessage(
        &self,
        _plain_rtps_submessage: &[u8],
        _sending_datareader_crypto: DatareaderCryptoHandle,
        _receiving_datawriter_crypto_list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn encode_rtps_message(
        &self,
        _encoded_rtps_message: &mut Vec<u8>,
        _plain_rtps_message: &[u8],
        _sending_participant_crypto: ParticipantCryptoHandle,
        _receiving_participant_crypto_list: &[ParticipantCryptoHandle],
        _receiving_participant_crypto_list_index: &mut usize,
        _transform_with_psk: bool,
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn decode_rtps_message(
        &self,
        _encoded_buffer: &[u8],
        _receiving_participant_crypto: ParticipantCryptoHandle,
        _sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn preprocess_secure_submsg(
        &self,
        _encoded_rtps_submessage: &[u8],
        _receiving_participant_crypto: ParticipantCryptoHandle,
        _sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<PreprocessedSubmessage>> {
        Ok(None)
    }

    fn decode_datawriter_submessage(
        &self,
        _encoded_rtps_submessage: &[u8],
        _receiving_datareader_crypto: DatareaderCryptoHandle,
        _sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn decode_datareader_submessage(
        &self,
        _encoded_rtps_submessage: &[u8],
        _receiving_datawriter_crypto: DatawriterCryptoHandle,
        _sending_datareader_crypto: DatareaderCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }

    fn decode_serialized_payload(
        &self,
        _encoded_buffer: &[u8],
        _inline_qos: &[u8],
        _receiving_datareader_crypto: DatareaderCryptoHandle,
        _sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }
}

impl<const V: bool> VendorFormCrypto for Interfaces<V> {
    fn encode_submessage_with_participant_key(
        &self,
        _local: ParticipantCryptoHandle,
        plain_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(plain_rtps_submessage.to_vec())
    }

    fn decode_submessage_with_participant_key(
        &self,
        _remote: ParticipantCryptoHandle,
        encoded_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(encoded_rtps_submessage.to_vec())
    }

    fn decode_serialized_payload_with_participant_key(
        &self,
        _remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(encoded_buffer.to_vec())
    }

    fn decode_serialized_payload_with_kx_key(
        &self,
        _remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Ok(encoded_buffer.to_vec())
    }
}

impl CryptographicPluginInterfaces for Interfaces<true> {
    fn vendor_forms(&self) -> Option<&dyn VendorFormCrypto> {
        Some(self)
    }
}

impl CryptographicPluginInterfaces for Interfaces<false> {}

#[test]
fn security_9_5_1_the_three_interfaces_are_one_object_and_vendor_forms_are_optional() {
    // The runtime holds the plugin as
    // `Box<dyn CryptographicPluginInterfaces>`.
    let mut plain: Box<dyn CryptographicPluginInterfaces> = Box::new(Interfaces::<false>);
    // The default body of `vendor_forms` offers none.
    assert!(plain.vendor_forms().is_none());

    // Each interface is reached through the one object (Tables 42 to 44).
    let (participant, _) = plain
        .register_local_participant(
            IdentityHandle(1),
            PermissionsHandle(1),
            &[Property::local("dds.sec.crypto.keysize", "256")],
            &ParticipantSecurityConfig::default(),
        )
        .unwrap();
    assert!(!participant.is_nil());
    assert_eq!(
        plain
            .create_local_participant_crypto_tokens(participant, ParticipantCryptoHandle(2), 0)
            .unwrap(),
        None
    );
    assert!(plain.return_crypto_tokens(Vec::new()).unwrap());
    let mut encoded = Vec::new();
    let mut index = 0;
    let readers = [DatareaderCryptoHandle(4), DatareaderCryptoHandle(5)];
    assert!(
        plain
            .encode_datawriter_submessage(
                &mut encoded,
                b"DATA",
                DatawriterCryptoHandle(3),
                &readers,
                &mut index,
            )
            .unwrap()
    );
    assert_eq!((encoded.as_slice(), index), (&b"DATA"[..], readers.len()));

    // A plugin that offers its vendor forms returns them.
    let offering: Box<dyn CryptographicPluginInterfaces> = Box::new(Interfaces::<true>);
    let forms = offering.vendor_forms().unwrap();
    assert_eq!(
        forms
            .decode_submessage_with_participant_key(ParticipantCryptoHandle(2), b"SEC")
            .unwrap(),
        b"SEC"
    );
}

/// A provider without challenge tracking, as the PSK and mock providers.
struct Secrets;

impl SharedSecretProvider for Secrets {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        (handle == SharedSecretHandle(7)).then(|| vec![0x5a; 32])
    }
}

#[test]
fn a_shared_secret_provider_answers_by_handle_and_has_no_challenges_by_default() {
    // `plugin.rs` holds the provider as `Arc<dyn SharedSecretProvider>`.
    let provider: Arc<dyn SharedSecretProvider> = Arc::new(Secrets);
    assert_eq!(
        provider.get_shared_secret(SharedSecretHandle(7)),
        Some(vec![0x5a; 32])
    );
    // "`None` if the handle is unknown".
    assert_eq!(provider.get_shared_secret(SharedSecretHandle(8)), None);
    // "Default `None` for providers without challenge tracking".
    assert_eq!(
        provider.get_shared_secret_challenges(SharedSecretHandle(7)),
        None
    );
}

#[test]
fn security_9_2_1_the_errors_this_crate_builds() {
    // `suite.rs` and `plugin/spec.rs` report a configuration they cannot
    // use and a bad argument; the code follows the kind.
    let e = SecurityError::new(
        SecurityErrorKind::InvalidConfiguration,
        "crypto: unknown symmetric cipher",
    );
    assert_eq!(
        (e.code, e.minor_code),
        (SecurityErrorKind::InvalidConfiguration.code(), 0)
    );
    let e = SecurityError::bad_argument("crypto: not a CryptoToken of this plugin");
    assert_eq!(e.kind, SecurityErrorKind::BadArgument);
    assert_eq!(e.detail, "crypto: not a CryptoToken of this plugin");
    let e = SecurityError::not_implemented("plugin does not implement it");
    assert_eq!(e.kind, SecurityErrorKind::NotImplemented);
}
