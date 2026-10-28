// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The three interfaces of DDS-Security 1.2 §9.5.1 (Tables 42 to 44) for
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`vendor_forms` returns a trait object.)
//! `DDS:Crypto:AES-GCM-GMAC` (§10.5.3, Tables 78 to 80), implemented by
//! [`AesGcmCryptoPlugin`] on top of its key store.
//!
//! * The handles are the three handle types of the interfaces. A handle of a
//!   local entity stands for the key material the plugin made for it; a handle
//!   of a matched remote entity stands for the key material that entity sent
//!   (set by the `set_remote_*_crypto_tokens` operations) and, for the
//!   receivers of a key with origin authentication, for the receiver-specific
//!   key the plugin made for it.
//! * The key material of a matched remote entity is found by the handle of
//!   that entity, the `transformation_key_id` and the key revision of the
//!   CryptoHeader: "looks whether the transformation_key_id attribute ... is
//!   associated with those CryptoHandles. If the association is not found,
//!   the operation shall fail with an exception" (Table 80).
//! * `BuiltinParticipantVolatileMessageSecureWriter` and `...Reader` are the
//!   local endpoints whose `endpoint_guid` carries the EntityId `0xff0202c3`
//!   and `0xff0202c4`. Their key material is the Kx key derived from the
//!   SharedSecret (§10.5.2.1.2); it is made by
//!   `register_matched_remote_datareader` / `_datawriter` for the one remote
//!   endpoint, and nothing is exchanged for them.
//!
//! The properties this plugin reads beyond Table 71 are named
//! [`PROP_DOMAIN_ID`], [`PROP_DOMAIN_TAG`] (the DomainId and DomainTag of
//! §10.5.2.1.3 Table 76, which no operation of the interfaces carries) and
//! [`PROP_RTPS_AAD`] (whether `encode_rtps_message` authenticates the RTPS
//! Header as "Additional Authenticated Data", Table 80, 2; the specification
//! describes both transformations and leaves the choice open).

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::CryptoHandle;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use super::{
    AesGcmCryptoPlugin, HEADER_EXTENSION_ID, KeyMaterial, ReceiverId, ReceiverKey, SEC_PREFIX_ID,
    SRTPS_FLAG_P, SYMMETRIC_CIPHER_ALGORITHM, assemble_secure_submessages,
    assemble_secure_submessages_with, check_no_receiver_macs, encode_payload_from_mat,
    parse_secure_submessages_footer, parse_srtps, poisoned,
};
use crate::backend::digest;
use crate::suite::Suite;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::{
    CBIT_AES128_GCM, CBIT_AES256_GCM, CryptoTransformKeyRevisionIntHolder, key_revision_from_int,
};
use zerodds_security::cryptographic::{
    CryptoKeyExchange, CryptoKeyFactory, CryptoTransform, DatareaderCryptoHandle,
    DatawriterCryptoHandle, EncodedPayload, EndpointSecurityAlgorithmInfo, EndpointSecurityConfig,
    GuidT, PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED, PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
    PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED, ParticipantCryptoHandle,
    ParticipantSecurityAlgorithmInfo, ParticipantSecurityConfig, PreprocessedSubmessage,
    SecureSubmessageCategory,
};
use zerodds_security::properties::Property;
use zerodds_security::token::{CryptoToken, DataHolder};

use crate::psk_rtps::{PskCryptoPlugin, PskProtection};

/// `class_id` of the CryptoToken of the plugin (Table 72).
const TOKEN_CLASS_ID: &str = "DDS:Crypto:AES_GCM_GMAC";
/// The binary property of the CryptoToken that holds the
/// `KeyMaterial_AES_GCM_GMAC` (Table 72).
const TOKEN_KEYMAT: &str = "dds.cryp.keymat";

/// The number of receivers whose receiver-specific MACs one call of
/// `encode_datawriter_submessage` or `encode_rtps_message` puts into the
/// encoded message; the next call (the `..._list_index` of §9.5.1.10.2)
/// replaces them with the MACs of the next receivers.
pub const RECEIVER_MACS_PER_MESSAGE: usize = 1024;

/// `dds.sec.crypto` property with the DomainId of the participant, which the
/// derivation of the pre-shared key material needs (Table 76, `DomainId`).
pub const PROP_DOMAIN_ID: &str = "org.zerodds.crypto.domain_id";
/// The DomainTag of the participant (Table 76, `DomainTag`); empty if absent.
pub const PROP_DOMAIN_TAG: &str = "org.zerodds.crypto.domain_tag";
/// `true`: `encode_rtps_message` sets the flag A and authenticates the RTPS
/// Header and HeaderExtension (Table 80, 2); `false` (the default): the RTPS
/// Header is replaced by an INFO_SRC inside the protected content (Table 80,
/// 1), the form the other vendors read.
pub const PROP_RTPS_AAD: &str = "org.zerodds.crypto.rtps_additional_authenticated_data";
/// What a participant whose RTPS or Metadata Protection Kind asks for origin
/// authentication does with an RTPS message or submessage under a key without
/// a receiver-specific key (receiver_specific_key_id zero): `spec` (the
/// default) refuses it, as Table 80 `decode_rtps_message` asks ("If the kind
/// is not the expected one, the operation shall fail"); `accept` verifies it
/// with the common MAC alone, as Table 80 does for such a key. The form of a
/// sender that makes no receiver-specific keys although its governance asks
/// for them (OpenDDS 3.34: "origin auth is not implemented",
/// `CryptoBuiltInImpl.cpp`). A key that carries a receiver-specific key is
/// held to it under either value.
pub const PROP_ORIGIN_AUTHENTICATION_FALLBACK: &str =
    "org.zerodds.crypto.origin_authentication_fallback";

/// EntityId of `BuiltinParticipantVolatileMessageSecureWriter`.
const VOLATILE_WRITER_ENTITY: [u8; 4] = [0xff, 0x02, 0x02, 0xc3];
/// EntityId of `BuiltinParticipantVolatileMessageSecureReader`.
const VOLATILE_READER_ENTITY: [u8; 4] = [0xff, 0x02, 0x02, 0xc4];

/// Which key material of an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum KeyRole {
    /// The key of the submessages (or of the messages) of the entity.
    Main,
    /// The key of the serialized payload of a DataWriter.
    Payload,
}

/// What a key material that a remote entity sent belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstalledRole {
    /// Of a remote participant (`set_remote_participant_crypto_tokens`).
    Participant,
    /// Of a remote DataWriter (`set_remote_datawriter_crypto_tokens`).
    Writer,
    /// Of a remote DataReader (`set_remote_datareader_crypto_tokens`).
    Reader,
}

/// A key a remote entity sent: for which local entity.
#[derive(Debug, Clone, Copy)]
struct InstalledKey {
    local: u64,
    role: InstalledRole,
}

struct LocalParticipant {
    /// The slot of the ParticipantKeyMaterial.
    slot: CryptoHandle,
    /// The symmetric cipher algorithms the plugin accepts
    /// (`adjusted_algorithm_info.symmetric_cipher.supported_mask`).
    accepted_mask: u32,
    /// Key width of the participant, 128 or 256.
    bits: u16,
    /// `is_rtps_axk_protected`.
    axk_protected: bool,
    /// What the participant expects of the RTPS messages it receives.
    expect_encrypt: bool,
    expect_origin: bool,
    /// `PROP_ORIGIN_AUTHENTICATION_FALLBACK` is `accept`.
    origin_fallback: bool,
    /// `PROP_RTPS_AAD`.
    aad: bool,
    /// The revision `encode` uses (0 until a revision is activated) and the
    /// last one `revise_local_entity_keys` made.
    active_revision: i32,
    last_revision: i32,
    /// The pre-shared-key engine, with `is_rtps_psk_protected`.
    psk: Option<Arc<PskCryptoPlugin>>,
}

struct RemoteParticipant {
    local: u64,
}

struct LocalEndpoint {
    participant: u64,
    slot: CryptoHandle,
    /// A volatile endpoint: its key is the Kx key of the SharedSecret.
    kx: bool,
    submessage_protected: bool,
    payload_protected: bool,
    expect_submessage_encrypt: bool,
    expect_payload_encrypt: bool,
    expect_origin: bool,
}

struct RemoteEndpoint {
    /// The local endpoint it is matched with.
    local: u64,
    /// The remote participant it belongs to.
    participant: u64,
    relay_only: bool,
}

#[derive(Default)]
pub(super) struct SpecState {
    participants: BTreeMap<u64, LocalParticipant>,
    remote_participants: BTreeMap<u64, RemoteParticipant>,
    writers: BTreeMap<u64, LocalEndpoint>,
    readers: BTreeMap<u64, LocalEndpoint>,
    remote_readers: BTreeMap<u64, RemoteEndpoint>,
    remote_writers: BTreeMap<u64, RemoteEndpoint>,
    /// (handle of the remote entity, `transformation_key_id`, key revision)
    /// of the keys the plugin installed for it.
    keys: BTreeMap<(u64, [u8; 4], [u8; 3]), InstalledKey>,
    /// Kx keys of the volatile endpoints, by the handle of the remote
    /// endpoint.
    kx: BTreeMap<u64, KeyMaterial>,
    /// Revised key material not yet active: (revision, local entity, role).
    pending: BTreeMap<(i32, u64, KeyRole), KeyMaterial>,
    /// SHA-256 of the key material of the tokens issued and not returned.
    issued: BTreeMap<[u8; 32], u32>,
}

fn bad(detail: &'static str) -> SecurityError {
    SecurityError::new(SecurityErrorKind::BadArgument, detail)
}

fn unknown(what: &'static str) -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::BadArgument,
        alloc::format!("crypto: unknown {what} handle"),
    )
}

fn prop<'a>(props: &'a [Property], name: &str) -> Option<&'a str> {
    props
        .iter()
        .rev()
        .find(|p| p.name == name)
        .map(|p| p.value.as_ref())
}

/// The `CryptoAlgorithmBit` of the key width of `suite` (Table 22).
fn bit_of(suite: Suite) -> u32 {
    if suite.key_len() == 16 {
        CBIT_AES128_GCM
    } else {
        CBIT_AES256_GCM
    }
}

fn cipher_name(bits: u16) -> &'static str {
    if bits == 128 {
        "AES128-GCM"
    } else {
        "AES256-GCM"
    }
}

/// The name of a Protection Kind (§10.4.1.2.2) for the properties of
/// `register_local_endpoint`.
fn kind_name(protected: bool, encrypted: bool, origin: bool) -> &'static str {
    match (protected, encrypted, origin) {
        (false, _, _) => "NONE",
        (true, true, true) => "ENCRYPT_WITH_ORIGIN_AUTHENTICATION",
        (true, true, false) => "ENCRYPT",
        (true, false, true) => "SIGN_WITH_ORIGIN_AUTHENTICATION",
        (true, false, false) => "SIGN",
    }
}

fn token_of(keymat: Vec<u8>) -> CryptoToken {
    DataHolder::new(TOKEN_CLASS_ID).with_binary_property(TOKEN_KEYMAT, keymat)
}

/// The key material of a token of this plugin.
fn keymat_of(token: &CryptoToken) -> SecurityResult<KeyMaterial> {
    if token.class_id != TOKEN_CLASS_ID {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            alloc::format!(
                "crypto: a CryptoToken of the class '{}', not '{TOKEN_CLASS_ID}'",
                token.class_id
            ),
        ));
    }
    let bytes = token
        .binary_property(TOKEN_KEYMAT)
        .ok_or_else(|| bad("crypto: the CryptoToken has no dds.cryp.keymat"))?;
    KeyMaterial::from_keymat_cyclone(bytes)
}

fn digest_of(keymat: &[u8]) -> [u8; 32] {
    let d = digest::digest(&digest::SHA256, keymat);
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

/// What a received message has to be, to be taken (Table 80,
/// `decode_rtps_message`: "the transformation_kind matches the one the
/// receiving DomainParticipant is expecting both in terms of the type of
/// algorithm as well as the protection").
#[derive(Debug, Clone, Copy)]
pub(super) struct Expected {
    /// Authenticated encryption (GCM) or authentication only (GMAC).
    encrypt: bool,
    /// The key carries a receiver-specific key (origin authentication);
    /// `None` where the transformation does not use it (the payload, Table 80
    /// `encode_serialized_payload`: no receiver-specific MACs).
    origin: Option<bool>,
    /// A key without a receiver-specific key is taken where one is expected
    /// (`PROP_ORIGIN_AUTHENTICATION_FALLBACK`).
    origin_fallback: bool,
    /// The algorithms the plugin accepts.
    accepted: u32,
}

impl Expected {
    pub(super) fn check(&self, mat: &KeyMaterial) -> SecurityResult<()> {
        if mat.suite.is_aead() != self.encrypt {
            return Err(bad(
                "crypto: the transformation_kind is not the one expected (encryption or authentication only)",
            ));
        }
        let receiver_key = mat.origin.receiver_key.is_some();
        let fallback = self.origin_fallback && self.origin == Some(true) && !receiver_key;
        if self.origin.is_some_and(|origin| receiver_key != origin) && !fallback {
            return Err(bad(
                "crypto: the transformation_kind is not the one expected (origin authentication)",
            ));
        }
        if self.accepted & bit_of(mat.suite) == 0 {
            return Err(bad(
                "crypto: the algorithm of the key is not in the accepted supported_mask",
            ));
        }
        Ok(())
    }
}

impl AesGcmCryptoPlugin {
    fn spec_read(&self) -> SecurityResult<std::sync::RwLockReadGuard<'_, SpecState>> {
        self.spec.read().map_err(|_| poisoned())
    }

    fn spec_write(&self) -> SecurityResult<std::sync::RwLockWriteGuard<'_, SpecState>> {
        self.spec.write().map_err(|_| poisoned())
    }

    /// The Kx key of a SharedSecret (§10.5.2.1.2).
    fn kx_material(&self, shared_secret: SharedSecretHandle) -> SecurityResult<KeyMaterial> {
        let provider = self.secret_provider.as_ref().ok_or_else(|| {
            bad("crypto: the key of VolatileMessageSecure is derived from the SharedSecret, and no SharedSecretProvider is set")
        })?;
        let secret = provider
            .get_shared_secret(shared_secret)
            .ok_or_else(|| bad("crypto: the SharedSecret handle is unknown"))?;
        match provider.get_shared_secret_challenges(shared_secret) {
            Some((c1, c2)) => KeyMaterial::from_kx_shared_secret(&secret, &c1, &c2),
            None => KeyMaterial::from_shared_secret_kx(self.local_suite, &secret),
        }
    }

    /// Runs `f` on the key material of `entity` for `revision`: the active
    /// one, or the one `revise_local_entity_keys` made and `activate_key_revision`
    /// has not made active yet.
    #[allow(clippy::too_many_arguments)]
    fn with_local_material<R>(
        &self,
        spec: &SpecState,
        entity: u64,
        role: KeyRole,
        slot: CryptoHandle,
        active_revision: i32,
        revision: i32,
        f: impl FnOnce(&KeyMaterial) -> SecurityResult<R>,
    ) -> SecurityResult<R> {
        if revision == active_revision {
            let table = match role {
                KeyRole::Main => &self.slots,
                KeyRole::Payload => &self.payload_slots,
            };
            let table = table.read().map_err(|_| poisoned())?;
            let mat = table
                .get(&slot)
                .ok_or_else(|| bad("crypto: the entity has no key material of this kind"))?;
            return f(mat);
        }
        let mat = spec.pending.get(&(revision, entity, role)).ok_or_else(|| {
            bad("crypto: key_revision is no revision revise_local_entity_keys returned")
        })?;
        f(mat)
    }

    fn record_issued(&self, spec: &mut SpecState, keymat: &[u8]) {
        *spec.issued.entry(digest_of(keymat)).or_insert(0) += 1;
    }

    /// The receiver-specific key `mat` has for `remote`, made on the first
    /// call (the registration of a matched remote entity creates it).
    fn receiver_key_of(
        &self,
        mat: &KeyMaterial,
        remote: u64,
    ) -> SecurityResult<Option<ReceiverKey>> {
        if !mat.origin.enabled {
            return Ok(None);
        }
        let mut taken = self.local_key_ids.lock().map_err(|_| poisoned())?;
        mat.receiver_key_for(ReceiverId::Handle(remote), &self.rng, &mut taken)
            .map(Some)
    }

    /// `register_local_datawriter` and `register_local_datareader`.
    fn register_local_entity(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        properties: &[Property],
        config: &EndpointSecurityConfig,
        guid: GuidT,
        is_writer: bool,
    ) -> SecurityResult<(u64, EndpointSecurityAlgorithmInfo)> {
        let (participant_slot, bits, accepted) = {
            let spec = self.spec_read()?;
            let p = spec
                .participants
                .get(&participant_crypto.0)
                .ok_or_else(|| unknown("participant"))?;
            (p.slot, p.bits, p.accepted_mask)
        };
        if !config.is_submessage_protected && !config.is_payload_protected {
            return Err(bad(
                "crypto: neither the submessages nor the payload of the endpoint are protected: no key material to make (Table 37)",
            ));
        }
        let attrs = config.plugin_endpoint_attributes;
        let sub_encrypted = attrs & PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED != 0;
        let payload_encrypted = attrs & PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED != 0;
        let origin = attrs & PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED != 0;
        // §10.5.1.1: the property of the endpoint overrides the default of
        // the participant; "AUTO" or none leaves the default.
        let cipher = match prop(properties, SYMMETRIC_CIPHER_ALGORITHM) {
            None | Some("AUTO") => cipher_name(bits),
            Some(value) => {
                let named = crate::suite::symmetric_cipher_key_bits(
                    SYMMETRIC_CIPHER_ALGORITHM,
                    Some(value),
                )?;
                cipher_name(named)
            }
        };
        let kinds = [
            (
                zerodds_security::crypto::ENDPOINT_METADATA_PROTECTION_KIND,
                kind_name(config.is_submessage_protected, sub_encrypted, origin),
            ),
            (
                zerodds_security::crypto::ENDPOINT_DATA_PROTECTION_KIND,
                kind_name(config.is_payload_protected, payload_encrypted, false),
            ),
            (SYMMETRIC_CIPHER_ALGORITHM, cipher),
        ];
        let slot = zerodds_security::crypto::CryptographicPlugin::register_local_endpoint(
            self,
            participant_slot,
            is_writer,
            &kinds,
        )?;
        let (required, ok) = {
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let payload = self.payload_slots.read().map_err(|_| poisoned())?;
            let mut required = 0u32;
            for mat in slots.get(&slot).into_iter().chain(payload.get(&slot)) {
                required |= bit_of(mat.suite);
            }
            (required, required & accepted == required)
        };
        if !ok {
            zerodds_security::crypto::CryptographicPlugin::unregister_local_endpoint(self, slot)?;
            return Err(bad(
                "crypto: the algorithm of the endpoint is not in the accepted supported_mask of the participant",
            ));
        }
        let handle = self.next_id();
        let wanted_entity = if is_writer {
            VOLATILE_WRITER_ENTITY
        } else {
            VOLATILE_READER_ENTITY
        };
        let entry = LocalEndpoint {
            participant: participant_crypto.0,
            slot,
            kx: guid[12..16] == wanted_entity,
            submessage_protected: config.is_submessage_protected,
            payload_protected: config.is_payload_protected,
            expect_submessage_encrypt: sub_encrypted,
            expect_payload_encrypt: payload_encrypted,
            expect_origin: origin,
        };
        let mut spec = self.spec_write()?;
        if is_writer {
            spec.writers.insert(handle, entry);
        } else {
            spec.readers.insert(handle, entry);
        }
        Ok((
            handle,
            EndpointSecurityAlgorithmInfo {
                symmetric_cipher:
                    zerodds_security::cryptographic::EndpointSecuritySymmetricCipherAlgorithmInfo {
                        required_mask: required,
                        supported_mask: accepted,
                    },
            },
        ))
    }

    /// The tokens of a local endpoint for one remote endpoint: its
    /// submessage key (with the receiver-specific key of that endpoint under
    /// origin authentication) and, for a DataWriter, its payload key.
    fn endpoint_tokens(
        &self,
        spec: &mut SpecState,
        local: u64,
        remote: u64,
        is_writer: bool,
        relay_only: bool,
        key_revision: i32,
    ) -> SecurityResult<Vec<CryptoToken>> {
        let (entry_slot, participant, kx) = {
            let e = if is_writer {
                spec.writers.get(&local)
            } else {
                spec.readers.get(&local)
            }
            .ok_or_else(|| unknown("endpoint"))?;
            (e.slot, e.participant, e.kx)
        };
        if kx {
            // §10.5.2.1.2: the key is derived from the SharedSecret, nothing
            // is exchanged.
            return Ok(Vec::new());
        }
        let active = spec
            .participants
            .get(&participant)
            .ok_or_else(|| unknown("participant"))?
            .active_revision;
        let mut out = Vec::new();
        let relay_has_key = self
            .submessage_keys
            .read()
            .map_err(|_| poisoned())?
            .contains(&entry_slot);
        if !relay_only || relay_has_key {
            let bytes = self.with_local_material(
                spec,
                local,
                KeyRole::Main,
                entry_slot,
                active,
                key_revision,
                |mat| {
                    // A relay gets the key without a receiver-specific one.
                    let receiver = if relay_only {
                        None
                    } else {
                        self.receiver_key_of(mat, remote)?
                    };
                    Ok(mat.serialize_keymat_for(receiver.as_ref()))
                },
            )?;
            out.push(bytes);
        }
        if is_writer && !relay_only {
            let has_payload = self
                .payload_slots
                .read()
                .map_err(|_| poisoned())?
                .contains_key(&entry_slot);
            if has_payload {
                let bytes = self.with_local_material(
                    spec,
                    local,
                    KeyRole::Payload,
                    entry_slot,
                    active,
                    key_revision,
                    |mat| Ok(mat.serialize_keymat_cyclone()),
                )?;
                out.push(bytes);
            }
        }
        Ok(out
            .into_iter()
            .map(|bytes| {
                self.record_issued(spec, &bytes);
                token_of(bytes)
            })
            .collect())
    }

    /// Installs the keys of the tokens a remote entity sent, under its
    /// handle, for the local entity `local`.
    fn install_remote_tokens(
        &self,
        local_participant: u64,
        owner: u64,
        local: u64,
        role: InstalledRole,
        tokens: &[CryptoToken],
        max: usize,
    ) -> SecurityResult<()> {
        if tokens.is_empty() || tokens.len() > max {
            return Err(bad(
                "crypto: the number of CryptoTokens is not the one the operation takes",
            ));
        }
        let accepted = {
            let spec = self.spec_read()?;
            spec.participants
                .get(&local_participant)
                .ok_or_else(|| unknown("participant"))?
                .accepted_mask
        };
        let mut mats = Vec::new();
        for token in tokens {
            let mat = keymat_of(token)?;
            if accepted & bit_of(mat.suite) == 0 {
                return Err(bad(
                    "crypto: the algorithm of the key is not in the accepted supported_mask",
                ));
            }
            mats.push(mat);
        }
        let mut spec = self.spec_write()?;
        for (token, mat) in tokens.iter().zip(mats) {
            let id = (owner, mat.transformation_key_id, mat.key_revision);
            self.index_remote_key(CryptoHandle(owner), mat)?;
            spec.keys.insert(id, InstalledKey { local, role });
            if role == InstalledRole::Participant {
                // The key of the participant itself, for the vendor forms that
                // protect submessages or payloads with it.
                self.slots
                    .write()
                    .map_err(|_| poisoned())?
                    .insert(CryptoHandle(owner), keymat_of(token)?);
            }
        }
        Ok(())
    }

    /// The material of the remote entity `owner` for the key in `header`
    /// (the CryptoHeader: kind with the revision, key id), and for which
    /// local entity it is (Table 80: the key has to be associated with the
    /// handles).
    fn installed_key(
        &self,
        spec: &SpecState,
        owner: u64,
        header: &[u8],
        role: InstalledRole,
        local: u64,
    ) -> SecurityResult<CryptoHandle> {
        if header.len() < 8 {
            return Err(bad("crypto: the CryptoHeader is too short"));
        }
        let key_id = [header[4], header[5], header[6], header[7]];
        let revision = [header[0], header[1], header[2]];
        spec.keys
            .get(&(owner, key_id, revision))
            .filter(|k| k.role == role && k.local == local)
            .ok_or_else(|| {
                bad("crypto: no key of the sending entity for this transformation_key_id and revision is associated with the receiving entity")
            })?;
        self.key_id_slot(Some(CryptoHandle(owner)), key_id, revision)?
            .ok_or_else(|| bad("crypto: the key material of the sending entity is gone"))
    }
}

/// The header of the first secure submessage of `wire`: the CryptoHeader of
/// a SEC_PREFIX.
fn sec_prefix_header(wire: &[u8]) -> SecurityResult<&[u8]> {
    if wire.len() < 4 + 20 || wire[0] != SEC_PREFIX_ID {
        return Err(bad("crypto: not a SEC_PREFIX with a CryptoHeader"));
    }
    Ok(&wire[4..24])
}

fn psk_plugin_from(
    properties: &[Property],
    protection: PskProtection,
) -> SecurityResult<PskCryptoPlugin> {
    let domain_id: u32 = prop(properties, PROP_DOMAIN_ID)
        .ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!(
                    "crypto: {PROP_DOMAIN_ID} is needed for the pre-shared key (Table 76, DomainId)"
                ),
            )
        })?
        .parse()
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("crypto: {PROP_DOMAIN_ID} is a decimal DomainId"),
            )
        })?;
    let tag = prop(properties, PROP_DOMAIN_TAG).unwrap_or("");
    let pairs: Vec<(&str, &str)> = properties
        .iter()
        .map(|p| (p.name.as_ref(), p.value.as_ref()))
        .collect();
    let config =
        crate::psk_rtps::PskConfig::from_uri_properties(&pairs, domain_id, tag, protection)?;
    PskCryptoPlugin::new(config)
}

impl CryptoKeyFactory for AesGcmCryptoPlugin {
    fn register_local_participant(
        &mut self,
        participant_identity: IdentityHandle,
        participant_permissions: PermissionsHandle,
        participant_properties: &[Property],
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)> {
        if participant_identity.0 == 0 || participant_permissions.0 == 0 {
            return Err(bad(
                "crypto: register_local_participant: a nil identity or permissions handle",
            ));
        }
        let config = participant_security_config;
        // "participant_properties" plus the properties the AccessControl
        // plugin adds (Table 33, `ac_participant_properties`).
        let mut properties: Vec<Property> = participant_properties.to_vec();
        properties.extend(config.ac_endpoint_properties.iter().cloned());
        let attrs = config.plugin_participant_attributes;
        let bits = crate::suite::symmetric_cipher_key_bits(
            SYMMETRIC_CIPHER_ALGORITHM,
            prop(&properties, SYMMETRIC_CIPHER_ALGORITHM),
        )?;
        // Table 78: GCM or GMAC "according to the setting of the RTPS
        // Protection Kind" (`is_rtps_axk_encrypted`, Table 66).
        let encrypt = attrs & PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED != 0;
        let origin = attrs & PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED != 0;
        let suite = if encrypt {
            Suite::Aes256Gcm
        } else {
            Suite::Aes256Gmac
        }
        .with_key_bits(bits);
        // "The operation shall configure the Crypto plugins to only accept the
        // resulting set of supported algorithms".
        let capable = CBIT_AES128_GCM | CBIT_AES256_GCM;
        let requested = config.algorithm_info.symmetric_cipher.supported_mask;
        let accepted = if requested == 0 {
            capable
        } else {
            capable & requested
        };
        if accepted & bit_of(suite) == 0 {
            return Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "crypto: the symmetric_cipher_algorithm of the participant is not in supported_mask",
            ));
        }
        let psk = if config.is_rtps_psk_protected {
            let protection = if attrs & PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED != 0 {
                PskProtection::Encrypt
            } else {
                PskProtection::Sign
            };
            Some(Arc::new(psk_plugin_from(&properties, protection)?))
        } else {
            None
        };
        let aad = match prop(&properties, PROP_RTPS_AAD) {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => {
                return Err(SecurityError::new(
                    SecurityErrorKind::InvalidConfiguration,
                    alloc::format!("crypto: {PROP_RTPS_AAD} is true or false"),
                ));
            }
        };
        let origin_fallback = match prop(&properties, PROP_ORIGIN_AUTHENTICATION_FALLBACK) {
            None | Some("spec") => false,
            Some("accept") => true,
            Some(_) => {
                return Err(SecurityError::new(
                    SecurityErrorKind::InvalidConfiguration,
                    alloc::format!(
                        "crypto: {PROP_ORIGIN_AUTHENTICATION_FALLBACK} is spec or accept"
                    ),
                ));
            }
        };
        let mut mat = self.new_local_key(suite)?;
        mat.origin.enabled = origin;
        let slot = self.insert(mat)?;
        let handle = self.next_id();
        self.spec_write()?.participants.insert(
            handle,
            LocalParticipant {
                slot,
                accepted_mask: accepted,
                bits,
                axk_protected: config.is_rtps_axk_protected,
                expect_encrypt: encrypt,
                expect_origin: origin,
                origin_fallback,
                aad,
                active_revision: 0,
                last_revision: 0,
                psk,
            },
        );
        // Table 78: supported_mask with the algorithms the plugin can use;
        // builtin_kx_endpoints_required_mask for the protection of
        // DCPSParticipantVolatileMessageSecure (AES256-GCM, §10.5.2.1.2);
        // builtin_endpoints_required_mask for the other builtin topics;
        // "all other members ... zero".
        let builtin = if config.is_discovery_protected || config.is_liveliness_protected {
            bit_of(suite)
        } else {
            0
        };
        let info = ParticipantSecurityAlgorithmInfo {
            symmetric_cipher:
                zerodds_security::cryptographic::ParticipantSecuritySymmetricCipherAlgorithmInfo {
                    supported_mask: accepted,
                    builtin_endpoints_required_mask: builtin,
                    builtin_kx_endpoints_required_mask: CBIT_AES256_GCM,
                    user_endpoints_default_required_mask: 0,
                },
            ..Default::default()
        };
        Ok((ParticipantCryptoHandle(handle), info))
    }

    fn register_matched_remote_participant(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        remote_participant_identity: IdentityHandle,
        remote_participant_permissions: PermissionsHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        if remote_participant_identity.0 == 0 || remote_participant_permissions.0 == 0 {
            return Err(bad(
                "crypto: register_matched_remote_participant: a nil identity or permissions handle",
            ));
        }
        let slot = {
            let spec = self.spec_read()?;
            spec.participants
                .get(&local_participant_crypto_handle.0)
                .ok_or_else(|| unknown("participant"))?
                .slot
        };
        let handle = self.next_id();
        // Participant2ParticipantKeyMaterial: with origin authentication a new
        // receiver-specific key for this one participant.
        {
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let mat = slots.get(&slot).ok_or_else(|| unknown("participant"))?;
            self.receiver_key_of(mat, handle)?;
        }
        self.spec_write()?.remote_participants.insert(
            handle,
            RemoteParticipant {
                local: local_participant_crypto_handle.0,
            },
        );
        Ok(ParticipantCryptoHandle(handle))
    }

    fn register_local_datawriter(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datawriter_properties: &[Property],
        datawriter_security_config: &EndpointSecurityConfig,
        endpoint_guid: GuidT,
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        let (h, info) = self.register_local_entity(
            participant_crypto,
            datawriter_properties,
            datawriter_security_config,
            endpoint_guid,
            true,
        )?;
        Ok((DatawriterCryptoHandle(h), info))
    }

    fn register_matched_remote_datareader(
        &mut self,
        local_datawriter_crypto_handle: DatawriterCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
        relay_only: bool,
    ) -> SecurityResult<DatareaderCryptoHandle> {
        let (slot, kx) = {
            let spec = self.spec_read()?;
            if !spec
                .remote_participants
                .contains_key(&remote_participant_crypto.0)
            {
                return Err(unknown("remote participant"));
            }
            let w = spec
                .writers
                .get(&local_datawriter_crypto_handle.0)
                .ok_or_else(|| unknown("datawriter"))?;
            (w.slot, w.kx)
        };
        let handle = self.next_id();
        let kx_mat = if kx {
            Some(self.kx_material(shared_secret)?)
        } else {
            // Writer2ReaderKeyMaterialSeq: with origin authentication in the
            // Metadata Protection Kind a receiver-specific key for this one
            // reader.
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let mat = slots.get(&slot).ok_or_else(|| unknown("datawriter"))?;
            self.receiver_key_of(mat, handle)?;
            None
        };
        let mut spec = self.spec_write()?;
        if let Some(mat) = kx_mat {
            spec.kx.insert(handle, mat);
        }
        spec.remote_readers.insert(
            handle,
            RemoteEndpoint {
                local: local_datawriter_crypto_handle.0,
                participant: remote_participant_crypto.0,
                relay_only,
            },
        );
        Ok(DatareaderCryptoHandle(handle))
    }

    fn register_local_datareader(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datareader_properties: &[Property],
        datareader_security_config: &EndpointSecurityConfig,
        endpoint_guid: GuidT,
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        let (h, info) = self.register_local_entity(
            participant_crypto,
            datareader_properties,
            datareader_security_config,
            endpoint_guid,
            false,
        )?;
        Ok((DatareaderCryptoHandle(h), info))
    }

    fn register_matched_remote_datawriter(
        &mut self,
        local_datareader_crypto_handle: DatareaderCryptoHandle,
        remote_participant_crypt: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle> {
        let (slot, kx) = {
            let spec = self.spec_read()?;
            if !spec
                .remote_participants
                .contains_key(&remote_participant_crypt.0)
            {
                return Err(unknown("remote participant"));
            }
            let r = spec
                .readers
                .get(&local_datareader_crypto_handle.0)
                .ok_or_else(|| unknown("datareader"))?;
            (r.slot, r.kx)
        };
        let handle = self.next_id();
        let kx_mat = if kx {
            Some(self.kx_material(shared_secret)?)
        } else {
            // Reader2WriterKeyMaterial.
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let mat = slots.get(&slot).ok_or_else(|| unknown("datareader"))?;
            self.receiver_key_of(mat, handle)?;
            None
        };
        let mut spec = self.spec_write()?;
        if let Some(mat) = kx_mat {
            spec.kx.insert(handle, mat);
        }
        spec.remote_writers.insert(
            handle,
            RemoteEndpoint {
                local: local_datareader_crypto_handle.0,
                participant: remote_participant_crypt.0,
                relay_only: false,
            },
        );
        Ok(DatawriterCryptoHandle(handle))
    }

    fn revise_local_entity_keys(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder> {
        let mut spec = self.spec_write()?;
        let p = spec
            .participants
            .get(&participant_crypto_handle.0)
            .ok_or_else(|| unknown("participant"))?;
        let revision = p.last_revision + 1;
        if revision > 0x00ff_ffff {
            return Err(bad(
                "crypto: the key revisions of the participant are used up",
            ));
        }
        let wire = key_revision_from_int(revision);
        // The key material of the participant and of its entities, without the
        // volatile endpoints ("This operation shall not create new Key Material
        // for the BuiltinParticipantVolatileMessageSecureWriter and ...Reader").
        let mut entities: Vec<(u64, CryptoHandle)> = vec![(participant_crypto_handle.0, p.slot)];
        for (h, e) in spec.writers.iter().chain(spec.readers.iter()) {
            if e.participant == participant_crypto_handle.0 && !e.kx {
                entities.push((*h, e.slot));
            }
        }
        let mut made = Vec::new();
        {
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let payload = self.payload_slots.read().map_err(|_| poisoned())?;
            for (entity, slot) in &entities {
                let mat = slots.get(slot).ok_or_else(|| unknown("entity"))?;
                made.push((
                    (revision, *entity, KeyRole::Main),
                    mat.revised(wire, self.max_blocks_per_session, &self.rng)?,
                ));
                if let Some(mat) = payload.get(slot) {
                    made.push((
                        (revision, *entity, KeyRole::Payload),
                        mat.revised(wire, self.max_blocks_per_session, &self.rng)?,
                    ));
                }
            }
        }
        for (key, mat) in made {
            spec.pending.insert(key, mat);
        }
        if let Some(p) = spec.participants.get_mut(&participant_crypto_handle.0) {
            p.last_revision = revision;
        }
        Ok(revision)
    }

    fn activate_key_revision(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool> {
        let mut spec = self.spec_write()?;
        let p = spec
            .participants
            .get(&local_participant_crypto_handle.0)
            .ok_or_else(|| unknown("participant"))?;
        if key_revision == p.active_revision {
            return Ok(true);
        }
        let entities: BTreeMap<u64, CryptoHandle> =
            core::iter::once((local_participant_crypto_handle.0, p.slot))
                .chain(
                    spec.writers
                        .iter()
                        .chain(spec.readers.iter())
                        .filter(|(_, e)| e.participant == local_participant_crypto_handle.0)
                        .map(|(h, e)| (*h, e.slot)),
                )
                .collect();
        if !spec
            .pending
            .keys()
            .any(|(rev, entity, _)| *rev == key_revision && entities.contains_key(entity))
        {
            return Err(bad(
                "crypto: key_revision is no revision revise_local_entity_keys returned",
            ));
        }
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        let mut payload = self.payload_slots.write().map_err(|_| poisoned())?;
        let activate: Vec<(i32, u64, KeyRole)> = spec
            .pending
            .keys()
            .filter(|(rev, entity, _)| *rev == key_revision && entities.contains_key(entity))
            .copied()
            .collect();
        for key in activate {
            let (_, entity, role) = key;
            if let (Some(mat), Some(slot)) = (spec.pending.remove(&key), entities.get(&entity)) {
                match role {
                    KeyRole::Main => slots.insert(*slot, mat),
                    KeyRole::Payload => payload.insert(*slot, mat),
                };
            }
        }
        // Revisions up to this one are no use any more.
        spec.pending
            .retain(|(rev, entity, _), _| *rev > key_revision || !entities.contains_key(entity));
        if let Some(p) = spec
            .participants
            .get_mut(&local_participant_crypto_handle.0)
        {
            p.active_revision = key_revision;
        }
        Ok(true)
    }

    fn unregister_participant(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<bool> {
        let handle = participant_crypto_handle.0;
        let mut release_local: Vec<CryptoHandle> = Vec::new();
        let mut release_remote: Vec<CryptoHandle> = Vec::new();
        {
            let mut spec = self.spec_write()?;
            if let Some(p) = spec.participants.remove(&handle) {
                release_local.push(p.slot);
                let writers: Vec<u64> = spec
                    .writers
                    .iter()
                    .filter(|(_, e)| e.participant == handle)
                    .map(|(h, _)| *h)
                    .collect();
                let readers: Vec<u64> = spec
                    .readers
                    .iter()
                    .filter(|(_, e)| e.participant == handle)
                    .map(|(h, _)| *h)
                    .collect();
                for h in &writers {
                    if let Some(e) = spec.writers.remove(h) {
                        release_local.push(e.slot);
                    }
                }
                for h in &readers {
                    if let Some(e) = spec.readers.remove(h) {
                        release_local.push(e.slot);
                    }
                }
                let remotes: Vec<u64> = spec
                    .remote_participants
                    .iter()
                    .filter(|(_, r)| r.local == handle)
                    .map(|(h, _)| *h)
                    .collect();
                for r in remotes {
                    release_remote_participant(&mut spec, r, &mut release_remote);
                }
                spec.pending.retain(|(_, e, _), _| {
                    *e != handle && !writers.contains(e) && !readers.contains(e)
                });
            } else if spec.remote_participants.contains_key(&handle) {
                release_remote_participant(&mut spec, handle, &mut release_remote);
            } else {
                return Err(unknown("participant"));
            }
        }
        for slot in release_local {
            zerodds_security::crypto::CryptographicPlugin::unregister_local_endpoint(self, slot)?;
        }
        for owner in release_remote {
            zerodds_security::crypto::CryptographicPlugin::unregister_matched_remote_participant(
                self, owner,
            )?;
        }
        Ok(true)
    }

    fn unregister_datawriter(
        &mut self,
        datawriter_crypto_handle: DatawriterCryptoHandle,
    ) -> SecurityResult<bool> {
        let handle = datawriter_crypto_handle.0;
        let mut release_local: Vec<CryptoHandle> = Vec::new();
        let mut release_remote: Vec<CryptoHandle> = Vec::new();
        {
            let mut spec = self.spec_write()?;
            if let Some(e) = spec.writers.remove(&handle) {
                release_local.push(e.slot);
                let matched: Vec<u64> = spec
                    .remote_readers
                    .iter()
                    .filter(|(_, r)| r.local == handle)
                    .map(|(h, _)| *h)
                    .collect();
                for h in matched {
                    spec.remote_readers.remove(&h);
                    spec.kx.remove(&h);
                    spec.keys.retain(|(owner, _, _), _| *owner != h);
                    release_remote.push(CryptoHandle(h));
                }
                spec.pending.retain(|(_, entity, _), _| *entity != handle);
            } else if spec.remote_writers.remove(&handle).is_some() {
                spec.kx.remove(&handle);
                spec.keys.retain(|(owner, _, _), _| *owner != handle);
                release_remote.push(CryptoHandle(handle));
            } else {
                return Err(unknown("datawriter"));
            }
        }
        for slot in release_local {
            zerodds_security::crypto::CryptographicPlugin::unregister_local_endpoint(self, slot)?;
        }
        for owner in release_remote {
            zerodds_security::crypto::CryptographicPlugin::unregister_matched_remote_participant(
                self, owner,
            )?;
        }
        Ok(true)
    }

    fn unregister_datareader(
        &mut self,
        datareader_crypto_handle: DatareaderCryptoHandle,
    ) -> SecurityResult<bool> {
        let handle = datareader_crypto_handle.0;
        let mut release_local: Vec<CryptoHandle> = Vec::new();
        let mut release_remote: Vec<CryptoHandle> = Vec::new();
        {
            let mut spec = self.spec_write()?;
            if let Some(e) = spec.readers.remove(&handle) {
                release_local.push(e.slot);
                let matched: Vec<u64> = spec
                    .remote_writers
                    .iter()
                    .filter(|(_, r)| r.local == handle)
                    .map(|(h, _)| *h)
                    .collect();
                for h in matched {
                    spec.remote_writers.remove(&h);
                    spec.kx.remove(&h);
                    spec.keys.retain(|(owner, _, _), _| *owner != h);
                    release_remote.push(CryptoHandle(h));
                }
                spec.pending.retain(|(_, entity, _), _| *entity != handle);
            } else if spec.remote_readers.remove(&handle).is_some() {
                spec.kx.remove(&handle);
                spec.keys.retain(|(owner, _, _), _| *owner != handle);
                release_remote.push(CryptoHandle(handle));
            } else {
                return Err(unknown("datareader"));
            }
        }
        for slot in release_local {
            zerodds_security::crypto::CryptographicPlugin::unregister_local_endpoint(self, slot)?;
        }
        for owner in release_remote {
            zerodds_security::crypto::CryptographicPlugin::unregister_matched_remote_participant(
                self, owner,
            )?;
        }
        Ok(true)
    }
}

/// Drops a matched remote participant and everything that belongs to it;
/// the owners whose key material the plugin releases go into `owners`.
fn release_remote_participant(spec: &mut SpecState, handle: u64, owners: &mut Vec<CryptoHandle>) {
    spec.remote_participants.remove(&handle);
    owners.push(CryptoHandle(handle));
    let readers: Vec<u64> = spec
        .remote_readers
        .iter()
        .filter(|(_, r)| r.participant == handle)
        .map(|(h, _)| *h)
        .collect();
    let writers: Vec<u64> = spec
        .remote_writers
        .iter()
        .filter(|(_, r)| r.participant == handle)
        .map(|(h, _)| *h)
        .collect();
    for h in readers {
        spec.remote_readers.remove(&h);
        spec.kx.remove(&h);
        owners.push(CryptoHandle(h));
    }
    for h in writers {
        spec.remote_writers.remove(&h);
        spec.kx.remove(&h);
        owners.push(CryptoHandle(h));
    }
    let gone: Vec<u64> = owners.iter().map(|o| o.0).collect();
    spec.keys.retain(|(owner, _, _), _| !gone.contains(owner));
}

impl CryptoKeyExchange for AesGcmCryptoPlugin {
    fn create_local_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        let mut spec = self.spec_write()?;
        let (slot, active) = {
            let p = spec
                .participants
                .get(&local_participant_crypto.0)
                .ok_or_else(|| unknown("participant"))?;
            (p.slot, p.active_revision)
        };
        if spec
            .remote_participants
            .get(&remote_participant_crypto.0)
            .is_none_or(|r| r.local != local_participant_crypto.0)
        {
            return Err(unknown("remote participant"));
        }
        let bytes = self.with_local_material(
            &spec,
            local_participant_crypto.0,
            KeyRole::Main,
            slot,
            active,
            key_revision,
            |mat| {
                let receiver = self.receiver_key_of(mat, remote_participant_crypto.0)?;
                Ok(mat.serialize_keymat_for(receiver.as_ref()))
            },
        )?;
        self.record_issued(&mut spec, &bytes);
        Ok(Some(vec![token_of(bytes)]))
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        remote_participant_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        {
            let spec = self.spec_read()?;
            if spec
                .remote_participants
                .get(&remote_participant_crypto.0)
                .is_none_or(|r| r.local != local_participant_crypto.0)
            {
                return Err(unknown("remote participant"));
            }
        }
        self.install_remote_tokens(
            local_participant_crypto.0,
            remote_participant_crypto.0,
            local_participant_crypto.0,
            InstalledRole::Participant,
            remote_participant_tokens,
            1,
        )?;
        Ok(true)
    }

    fn create_local_datawriter_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        let mut spec = self.spec_write()?;
        let relay_only = {
            let r = spec
                .remote_readers
                .get(&remote_datareader_crypto.0)
                .filter(|r| r.local == local_datawriter_crypto.0)
                .ok_or_else(|| unknown("remote datareader"))?;
            r.relay_only
        };
        self.endpoint_tokens(
            &mut spec,
            local_datawriter_crypto.0,
            remote_datareader_crypto.0,
            true,
            relay_only,
            key_revision,
        )
        .map(Some)
    }

    fn set_remote_datawriter_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        remote_datawriter_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        let participant = {
            let spec = self.spec_read()?;
            let w = spec
                .remote_writers
                .get(&remote_datawriter_crypto.0)
                .filter(|w| w.local == local_datareader_crypto.0)
                .ok_or_else(|| unknown("remote datawriter"))?;
            let r = spec
                .readers
                .get(&local_datareader_crypto.0)
                .ok_or_else(|| unknown("datareader"))?;
            if r.kx {
                return Err(bad(
                    "crypto: the key of VolatileMessageSecure is derived from the SharedSecret, no tokens are set",
                ));
            }
            let _ = w;
            r.participant
        };
        self.install_remote_tokens(
            participant,
            remote_datawriter_crypto.0,
            local_datareader_crypto.0,
            InstalledRole::Writer,
            remote_datawriter_tokens,
            2,
        )?;
        Ok(true)
    }

    fn create_local_datareader_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        let mut spec = self.spec_write()?;
        spec.remote_writers
            .get(&remote_datawriter_crypto.0)
            .filter(|r| r.local == local_datareader_crypto.0)
            .ok_or_else(|| unknown("remote datawriter"))?;
        self.endpoint_tokens(
            &mut spec,
            local_datareader_crypto.0,
            remote_datawriter_crypto.0,
            false,
            false,
            key_revision,
        )
        .map(Some)
    }

    fn set_remote_datareader_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        remote_datareader_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        let participant = {
            let spec = self.spec_read()?;
            spec.remote_readers
                .get(&remote_datareader_crypto.0)
                .filter(|r| r.local == local_datawriter_crypto.0)
                .ok_or_else(|| unknown("remote datareader"))?;
            let w = spec
                .writers
                .get(&local_datawriter_crypto.0)
                .ok_or_else(|| unknown("datawriter"))?;
            if w.kx {
                return Err(bad(
                    "crypto: the key of VolatileMessageSecure is derived from the SharedSecret, no tokens are set",
                ));
            }
            w.participant
        };
        self.install_remote_tokens(
            participant,
            remote_datareader_crypto.0,
            local_datawriter_crypto.0,
            InstalledRole::Reader,
            remote_datareader_tokens,
            1,
        )?;
        Ok(true)
    }

    fn return_crypto_tokens(&mut self, crypto_tokens: Vec<CryptoToken>) -> SecurityResult<bool> {
        let mut spec = self.spec_write()?;
        // Every token has to be one this plugin issued and has not got back.
        let mut counts = spec.issued.clone();
        let mut digests = Vec::new();
        for token in &crypto_tokens {
            let bytes = token
                .binary_property(TOKEN_KEYMAT)
                .filter(|_| token.class_id == TOKEN_CLASS_ID)
                .ok_or_else(|| bad("crypto: not a CryptoToken of this plugin"))?;
            let d = digest_of(bytes);
            match counts.get_mut(&d) {
                Some(n) if *n > 0 => *n -= 1,
                _ => {
                    return Err(bad(
                        "crypto: the CryptoToken was not issued by this plugin, or is returned twice",
                    ));
                }
            }
            digests.push(d);
        }
        for d in digests {
            if let Some(n) = spec.issued.get_mut(&d) {
                *n -= 1;
                if *n == 0 {
                    spec.issued.remove(&d);
                }
            }
        }
        Ok(true)
    }
}

impl zerodds_security::cryptographic::CryptographicPluginInterfaces for AesGcmCryptoPlugin {
    fn vendor_forms(&self) -> Option<&dyn zerodds_security::cryptographic::VendorFormCrypto> {
        Some(self)
    }
}

impl zerodds_security::cryptographic::VendorFormCrypto for AesGcmCryptoPlugin {
    fn encode_submessage_with_participant_key(
        &self,
        local: ParticipantCryptoHandle,
        plain_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let slot = self
            .spec_read()?
            .participants
            .get(&local.0)
            .ok_or_else(|| unknown("participant"))?
            .slot;
        zerodds_security::crypto::CryptographicPlugin::encode_data_datawriter_submessage(
            self,
            slot,
            plain_rtps_submessage,
        )
    }

    fn decode_submessage_with_participant_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_rtps_submessage: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        zerodds_security::crypto::CryptographicPlugin::decode_data_datawriter_submessage(
            self,
            CryptoHandle(remote.0),
            encoded_rtps_submessage,
        )
    }

    fn decode_serialized_payload_with_participant_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        zerodds_security::crypto::CryptographicPlugin::decode_serialized_payload_with(
            self,
            CryptoHandle(remote.0),
            encoded_buffer,
        )
    }

    fn decode_serialized_payload_with_kx_key(
        &self,
        remote: ParticipantCryptoHandle,
        encoded_buffer: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if encoded_buffer.len() < 20 + 4 + 16 + 4 {
            return Err(bad("crypto: encoded serialized_payload too short"));
        }
        let ct_len = u32::from_be_bytes([
            encoded_buffer[20],
            encoded_buffer[21],
            encoded_buffer[22],
            encoded_buffer[23],
        ]) as usize;
        if encoded_buffer.len() < 24 + ct_len + 16 + 4 {
            return Err(bad("crypto: serialized_payload content.length > buffer"));
        }
        check_no_receiver_macs(&encoded_buffer[24 + ct_len + 16..24 + ct_len + 20])?;
        let spec = self.spec_read()?;
        let kx = spec
            .remote_writers
            .iter()
            .filter(|(_, w)| w.participant == remote.0)
            .find_map(|(h, _)| spec.kx.get(h))
            .ok_or_else(|| bad("crypto: no Kx key of this participant"))?;
        kx.open_cyclone(
            &encoded_buffer[..20],
            &encoded_buffer[24..24 + ct_len],
            &encoded_buffer[24 + ct_len..24 + ct_len + 16],
        )
    }
}

impl AesGcmCryptoPlugin {
    /// The number of CryptoTokens the plugin issued that
    /// `return_crypto_tokens` has not got back.
    ///
    /// # Errors
    /// `Internal` if a lock is poisoned.
    pub fn outstanding_crypto_tokens(&self) -> SecurityResult<usize> {
        Ok(self.spec_read()?.issued.values().map(|n| *n as usize).sum())
    }

    /// Seals a submessage of a local endpoint under its key for the
    /// receivers in `list` of the remote endpoints `ids`.
    fn seal_endpoint_submessage(
        &self,
        mat: &KeyMaterial,
        plain: &[u8],
        ids: &[ReceiverId],
    ) -> SecurityResult<Vec<u8>> {
        let (header, ct, tag) = mat.seal_cyclone(plain)?;
        let tail = mat.footer_tail_for(&header, &tag, Some(ids))?;
        Ok(assemble_secure_submessages_with(&header, &ct, &tag, &tail))
    }
}

/// The receivers `list[index..]` of one call: at most
/// [`RECEIVER_MACS_PER_MESSAGE`] when the key has origin authentication, all
/// of them otherwise (nothing to MAC).
fn batch(origin: bool, len: usize, index: usize) -> core::ops::Range<usize> {
    let end = if origin {
        (index + RECEIVER_MACS_PER_MESSAGE).min(len)
    } else {
        len
    };
    index..end
}

impl CryptoTransform for AesGcmCryptoPlugin {
    fn encode_serialized_payload(
        &self,
        plain_buffer: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>> {
        let spec = self.spec_read()?;
        let w = spec
            .writers
            .get(&sending_datawriter_crypto.0)
            .ok_or_else(|| unknown("datawriter"))?;
        if !w.payload_protected {
            // The data of the writer is not protected: nothing to transform.
            return Ok(None);
        }
        let active = spec
            .participants
            .get(&w.participant)
            .ok_or_else(|| unknown("participant"))?
            .active_revision;
        // The payload key of a writer with two keys, else its one key.
        let has_payload_key = self
            .payload_slots
            .read()
            .map_err(|_| poisoned())?
            .contains_key(&w.slot);
        let role = if has_payload_key {
            KeyRole::Payload
        } else {
            KeyRole::Main
        };
        let encoded = self.with_local_material(
            &spec,
            sending_datawriter_crypto.0,
            role,
            w.slot,
            active,
            active,
            |mat| encode_payload_from_mat(mat, plain_buffer),
        )?;
        // The CryptoTransformIdentifier is in the CryptoHeader of the
        // CryptoContent; there is nothing to add to the inlineQos.
        Ok(Some(EncodedPayload {
            encoded_buffer: encoded,
            extra_inline_qos: Vec::new(),
        }))
    }

    fn encode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &mut Vec<u8>,
        plain_rtps_submessage: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
        receiving_datareader_crypto_list: &[DatareaderCryptoHandle],
        receiving_datareader_crypto_list_index: &mut usize,
    ) -> SecurityResult<bool> {
        let spec = self.spec_read()?;
        let w = spec
            .writers
            .get(&sending_datawriter_crypto.0)
            .ok_or_else(|| unknown("datawriter"))?;
        let list = receiving_datareader_crypto_list;
        for r in list {
            spec.remote_readers
                .get(&r.0)
                .filter(|e| e.local == sending_datawriter_crypto.0)
                .ok_or_else(|| unknown("remote datareader"))?;
        }
        let index = *receiving_datareader_crypto_list_index;
        if index > list.len() {
            return Err(bad(
                "crypto: receiving_datareader_crypto_list_index is beyond the list",
            ));
        }
        if w.kx {
            // §9.5.1.10.2: the list has ONE element with the Kx key material.
            if list.len() != 1 {
                return Err(bad(
                    "crypto: the list of BuiltinParticipantVolatileMessageSecureWriter has one element",
                ));
            }
            if index != 0 {
                *receiving_datareader_crypto_list_index = list.len();
                return Ok(true);
            }
            let kx = spec
                .kx
                .get(&list[0].0)
                .ok_or_else(|| unknown("remote datareader"))?;
            let (header, ct, tag) = kx.seal_cyclone(plain_rtps_submessage)?;
            *encoded_rtps_submessage = assemble_secure_submessages(&header, &ct, &tag);
            *receiving_datareader_crypto_list_index = list.len();
            return Ok(true);
        }
        if !w.submessage_protected {
            return Err(bad(
                "crypto: the submessages of the DataWriter are not protected",
            ));
        }
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots
            .get(&w.slot)
            .ok_or_else(|| bad("crypto: the DataWriter has no key material"))?;
        let range = batch(mat.origin.enabled, list.len(), index);
        let ids: Vec<ReceiverId> = list[range.clone()]
            .iter()
            .map(|r| ReceiverId::Handle(r.0))
            .collect();
        if index == 0 {
            if plain_rtps_submessage.is_empty() {
                return Err(bad(
                    "crypto: a call with index 0 needs the plain submessage",
                ));
            }
            *encoded_rtps_submessage =
                self.seal_endpoint_submessage(mat, plain_rtps_submessage, &ids)?;
        } else {
            if !plain_rtps_submessage.is_empty() {
                return Err(bad(
                    "crypto: with a non-zero index the plain submessage is empty (§9.5.1.10.2)",
                ));
            }
            let (header, ct, footer) = parse_secure_submessages_footer(encoded_rtps_submessage)?;
            let tag = footer
                .get(..16)
                .ok_or_else(|| bad("crypto: the encoded submessage has no common_mac"))?;
            let tail = mat.footer_tail_for(&header, tag, Some(&ids))?;
            *encoded_rtps_submessage = assemble_secure_submessages_with(&header, &ct, tag, &tail);
        }
        *receiving_datareader_crypto_list_index = range.end;
        Ok(true)
    }

    fn encode_datareader_submessage(
        &self,
        plain_rtps_submessage: &[u8],
        sending_datareader_crypto: DatareaderCryptoHandle,
        receiving_datawriter_crypto_list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>> {
        let spec = self.spec_read()?;
        let r = spec
            .readers
            .get(&sending_datareader_crypto.0)
            .ok_or_else(|| unknown("datareader"))?;
        let list = receiving_datawriter_crypto_list;
        for w in list {
            spec.remote_writers
                .get(&w.0)
                .filter(|e| e.local == sending_datareader_crypto.0)
                .ok_or_else(|| unknown("remote datawriter"))?;
        }
        if r.kx {
            if list.len() != 1 {
                return Err(bad(
                    "crypto: the list of BuiltinParticipantVolatileMessageSecureReader has one element",
                ));
            }
            let kx = spec
                .kx
                .get(&list[0].0)
                .ok_or_else(|| unknown("remote datawriter"))?;
            let (header, ct, tag) = kx.seal_cyclone(plain_rtps_submessage)?;
            return Ok(Some(assemble_secure_submessages(&header, &ct, &tag)));
        }
        if !r.submessage_protected {
            return Err(bad(
                "crypto: the submessages of the DataReader are not protected",
            ));
        }
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots
            .get(&r.slot)
            .ok_or_else(|| bad("crypto: the DataReader has no key material"))?;
        let ids: Vec<ReceiverId> = list.iter().map(|w| ReceiverId::Handle(w.0)).collect();
        self.seal_endpoint_submessage(mat, plain_rtps_submessage, &ids)
            .map(Some)
    }

    fn encode_rtps_message(
        &self,
        encoded_rtps_message: &mut Vec<u8>,
        plain_rtps_message: &[u8],
        sending_participant_crypto: ParticipantCryptoHandle,
        receiving_participant_crypto_list: &[ParticipantCryptoHandle],
        receiving_participant_crypto_list_index: &mut usize,
        transform_with_psk: bool,
    ) -> SecurityResult<bool> {
        let spec = self.spec_read()?;
        let p = spec
            .participants
            .get(&sending_participant_crypto.0)
            .ok_or_else(|| unknown("participant"))?;
        let list = receiving_participant_crypto_list;
        for r in list {
            spec.remote_participants
                .get(&r.0)
                .filter(|e| e.local == sending_participant_crypto.0)
                .ok_or_else(|| unknown("remote participant"))?;
        }
        let index = *receiving_participant_crypto_list_index;
        if index > list.len() {
            return Err(bad(
                "crypto: receiving_participant_crypto_list_index is beyond the list",
            ));
        }
        if transform_with_psk {
            // "If true, indicates that the RTPS message shall be protected
            // using the sending Participant's pre-shared key"; the pre-shared
            // key has no receiver-specific part.
            let psk = p.psk.as_ref().ok_or_else(|| {
                bad("crypto: no pre-shared key is configured (rtps_psk_secret_passphrase)")
            })?;
            if index == 0 {
                if plain_rtps_message.is_empty() {
                    return Err(bad("crypto: a call with index 0 needs the plain message"));
                }
                *encoded_rtps_message = psk.encode_rtps_message_derived(plain_rtps_message)?;
            }
            *receiving_participant_crypto_list_index = list.len();
            return Ok(true);
        }
        if !p.axk_protected {
            // "This operation may optionally not perform any transformation":
            // FALSE, no exception; the middleware sends the original message.
            return Ok(false);
        }
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots
            .get(&p.slot)
            .ok_or_else(|| bad("crypto: the participant has no key material"))?;
        let range = batch(mat.origin.enabled, list.len(), index);
        let ids: Vec<ReceiverId> = list[range.clone()]
            .iter()
            .map(|r| ReceiverId::Handle(r.0))
            .collect();
        if index == 0 {
            if plain_rtps_message.is_empty() {
                return Err(bad("crypto: a call with index 0 needs the plain message"));
            }
            // Table 80, 1): "If Additional Authenticated Data (AAD)' is not
            // enabled the input RTPS Message cannot contain an RTPS Header
            // Extension."
            if !p.aad && plain_rtps_message.get(20) == Some(&HEADER_EXTENSION_ID) {
                return Err(bad(
                    "crypto: with the AAD form disabled the RTPS message cannot contain a HeaderExtension (Table 80)",
                ));
            }
            *encoded_rtps_message = if p.aad {
                mat.encode_rtps_aad(plain_rtps_message, false, Some(&ids))?
            } else {
                mat.encode_rtps_info_src(plain_rtps_message, Some(&ids))?
            };
        } else {
            if !plain_rtps_message.is_empty() {
                return Err(bad(
                    "crypto: with a non-zero index the plain message is empty (§9.5.1.10.4)",
                ));
            }
            *encoded_rtps_message = mat.retail_srtps(encoded_rtps_message, &ids)?;
        }
        *receiving_participant_crypto_list_index = range.end;
        Ok(true)
    }

    fn decode_rtps_message(
        &self,
        encoded_buffer: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let spec = self.spec_read()?;
        let p = spec
            .participants
            .get(&receiving_participant_crypto.0)
            .ok_or_else(|| unknown("participant"))?;
        if spec
            .remote_participants
            .get(&sending_participant_crypto.0)
            .is_none_or(|r| r.local != receiving_participant_crypto.0)
        {
            return Err(unknown("remote participant"));
        }
        let message = parse_srtps(encoded_buffer)?;
        if message.prefix_flags & SRTPS_FLAG_P != 0 {
            // 2) The PreSharedKeyFlag is set: the pre-shared key material.
            let psk = p.psk.as_ref().ok_or_else(|| {
                bad("crypto: the message is protected with a pre-shared key, and none is configured")
            })?;
            return psk.decode_rtps_message(encoded_buffer).map(Some);
        }
        // 1) Not set: the key of the sending participant, found by its
        // transformation_key_id among the keys associated with the two
        // handles.
        self.installed_key(
            &spec,
            sending_participant_crypto.0,
            &message.cheader,
            InstalledRole::Participant,
            receiving_participant_crypto.0,
        )?;
        let expected = Expected {
            encrypt: p.expect_encrypt,
            origin: Some(p.expect_origin),
            origin_fallback: p.origin_fallback,
            accepted: p.accepted_mask,
        };
        self.decode_rtps_message_of(
            Some(CryptoHandle(sending_participant_crypto.0)),
            encoded_buffer,
            Some(&expected),
        )
        .map(Some)
    }

    fn preprocess_secure_submsg(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<PreprocessedSubmessage>> {
        let spec = self.spec_read()?;
        if !spec
            .participants
            .contains_key(&receiving_participant_crypto.0)
        {
            return Err(unknown("participant"));
        }
        if spec
            .remote_participants
            .get(&sending_participant_crypto.0)
            .is_none_or(|r| r.local != receiving_participant_crypto.0)
        {
            return Err(unknown("remote participant"));
        }
        let header = sec_prefix_header(encoded_rtps_submessage)?;
        // 1. "Determine whether the CryptoTransformIdentifier the
        // transformation_kind matches one of the recognized kinds."
        if Suite::from_transform_kind_id(header[3]).is_none() {
            return Err(bad(
                "crypto: the transformation_kind of the CryptoTransformIdentifier is not one the plugin recognizes",
            ));
        }
        let key_id = [header[4], header[5], header[6], header[7]];
        let revision = [header[0], header[1], header[2]];
        // 3. The handles associated with the transformation_key_id, among
        // those of the sending participant.
        if key_id == [0; 4] {
            // The Kx key of the volatile endpoints: one key for both
            // directions. A submessage of the remote volatile writer is
            // reported first; the key opens both.
            let pair = |remote: &BTreeMap<u64, RemoteEndpoint>| {
                remote
                    .iter()
                    .find(|&(h, r)| {
                        r.participant == sending_participant_crypto.0 && spec.kx.contains_key(h)
                    })
                    .map(|(h, r)| (*h, r.local))
            };
            if let Some((remote_writer, local_reader)) = pair(&spec.remote_writers) {
                return Ok(Some(PreprocessedSubmessage {
                    datawriter_crypto: DatawriterCryptoHandle(remote_writer),
                    datareader_crypto: DatareaderCryptoHandle(local_reader),
                    secure_submessage_category: SecureSubmessageCategory::DatawriterSubmessage,
                }));
            }
            if let Some((remote_reader, local_writer)) = pair(&spec.remote_readers) {
                return Ok(Some(PreprocessedSubmessage {
                    datawriter_crypto: DatawriterCryptoHandle(local_writer),
                    datareader_crypto: DatareaderCryptoHandle(remote_reader),
                    secure_submessage_category: SecureSubmessageCategory::DatareaderSubmessage,
                }));
            }
            return Err(bad(
                "crypto: no volatile endpoint of the sending participant has a Kx key",
            ));
        }
        for ((owner, id, rev), installed) in &spec.keys {
            if *id != key_id || *rev != revision {
                continue;
            }
            match installed.role {
                InstalledRole::Writer => {
                    let w = spec.remote_writers.get(owner);
                    if w.is_some_and(|w| w.participant == sending_participant_crypto.0) {
                        return Ok(Some(PreprocessedSubmessage {
                            datawriter_crypto: DatawriterCryptoHandle(*owner),
                            datareader_crypto: DatareaderCryptoHandle(installed.local),
                            secure_submessage_category:
                                SecureSubmessageCategory::DatawriterSubmessage,
                        }));
                    }
                }
                InstalledRole::Reader => {
                    let r = spec.remote_readers.get(owner);
                    if r.is_some_and(|r| r.participant == sending_participant_crypto.0) {
                        return Ok(Some(PreprocessedSubmessage {
                            datawriter_crypto: DatawriterCryptoHandle(installed.local),
                            datareader_crypto: DatareaderCryptoHandle(*owner),
                            secure_submessage_category:
                                SecureSubmessageCategory::DatareaderSubmessage,
                        }));
                    }
                }
                InstalledRole::Participant => {}
            }
        }
        Err(bad(
            "crypto: no key of the sending participant is associated with this transformation_key_id",
        ))
    }

    fn decode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let spec = self.spec_read()?;
        let r = spec
            .readers
            .get(&receiving_datareader_crypto.0)
            .ok_or_else(|| unknown("datareader"))?;
        spec.remote_writers
            .get(&sending_datawriter_crypto.0)
            .filter(|w| w.local == receiving_datareader_crypto.0)
            .ok_or_else(|| unknown("remote datawriter"))?;
        let (header, ct, footer) = parse_secure_submessages_footer(encoded_rtps_submessage)?;
        if r.kx {
            let kx = spec
                .kx
                .get(&sending_datawriter_crypto.0)
                .ok_or_else(|| unknown("remote datawriter"))?;
            return kx.open_submessage(&header, &ct, &footer).map(Some);
        }
        let slot = self.installed_key(
            &spec,
            sending_datawriter_crypto.0,
            &header,
            InstalledRole::Writer,
            receiving_datareader_crypto.0,
        )?;
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots
            .get(&slot)
            .ok_or_else(|| bad("crypto: the key material of the DataWriter is gone"))?;
        let (accepted, origin_fallback) = spec
            .participants
            .get(&r.participant)
            .map_or((0, false), |p| (p.accepted_mask, p.origin_fallback));
        Expected {
            encrypt: r.expect_submessage_encrypt,
            origin: Some(r.expect_origin),
            origin_fallback,
            accepted,
        }
        .check(mat)?;
        mat.open_submessage(&header, &ct, &footer).map(Some)
    }

    fn decode_datareader_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datawriter_crypto: DatawriterCryptoHandle,
        sending_datareader_crypto: DatareaderCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let spec = self.spec_read()?;
        let w = spec
            .writers
            .get(&receiving_datawriter_crypto.0)
            .ok_or_else(|| unknown("datawriter"))?;
        spec.remote_readers
            .get(&sending_datareader_crypto.0)
            .filter(|r| r.local == receiving_datawriter_crypto.0)
            .ok_or_else(|| unknown("remote datareader"))?;
        let (header, ct, footer) = parse_secure_submessages_footer(encoded_rtps_submessage)?;
        if w.kx {
            let kx = spec
                .kx
                .get(&sending_datareader_crypto.0)
                .ok_or_else(|| unknown("remote datareader"))?;
            return kx.open_submessage(&header, &ct, &footer).map(Some);
        }
        let slot = self.installed_key(
            &spec,
            sending_datareader_crypto.0,
            &header,
            InstalledRole::Reader,
            receiving_datawriter_crypto.0,
        )?;
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots
            .get(&slot)
            .ok_or_else(|| bad("crypto: the key material of the DataReader is gone"))?;
        let (accepted, origin_fallback) = spec
            .participants
            .get(&w.participant)
            .map_or((0, false), |p| (p.accepted_mask, p.origin_fallback));
        // The DataReader's submessages are protected as the matched
        // DataWriter's: its Metadata Protection Kind is the same (the
        // endpoints match only with equal EndpointSecurityInfo).
        Expected {
            encrypt: w.expect_submessage_encrypt,
            origin: Some(w.expect_origin),
            origin_fallback,
            accepted,
        }
        .check(mat)?;
        mat.open_submessage(&header, &ct, &footer).map(Some)
    }

    fn decode_serialized_payload(
        &self,
        encoded_buffer: &[u8],
        _inline_qos: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let spec = self.spec_read()?;
        let r = spec
            .readers
            .get(&receiving_datareader_crypto.0)
            .ok_or_else(|| unknown("datareader"))?;
        spec.remote_writers
            .get(&sending_datawriter_crypto.0)
            .filter(|w| w.local == receiving_datareader_crypto.0)
            .ok_or_else(|| unknown("remote datawriter"))?;
        if encoded_buffer.len() < 24 {
            return Err(bad("crypto: the encoded serialized payload is too short"));
        }
        // The CryptoTransformIdentifier is in the CryptoHeader of the
        // CryptoContent; the key is found by it, among those of the sending
        // DataWriter.
        let slot = self.installed_key(
            &spec,
            sending_datawriter_crypto.0,
            &encoded_buffer[..8],
            InstalledRole::Writer,
            receiving_datareader_crypto.0,
        )?;
        let accepted = spec
            .participants
            .get(&r.participant)
            .map_or(0, |p| p.accepted_mask);
        {
            let slots = self.slots.read().map_err(|_| poisoned())?;
            let mat = slots
                .get(&slot)
                .ok_or_else(|| bad("crypto: the key material of the DataWriter is gone"))?;
            // Origin authentication is not checked for the payload. Table 80
            // `encode_serialized_payload` sets its receiver_specific_macs
            // "always" to the empty sequence, and Table 78
            // `register_local_datawriter` allows one key material for
            // metadata and data, which then carries the receiver-specific key
            // of `register_matched_remote_datareader`. The row of
            // `decode_serialized_payload` asks for receiver-specific MACs
            // when that key is present, which no encoder writes for a
            // payload: a contradiction of the specification, read here as
            // the encoder's row. An empty receiver_specific_macs is checked
            // in `decode_serialized_payload_with`.
            Expected {
                encrypt: r.expect_payload_encrypt,
                origin: None,
                origin_fallback: false,
                accepted,
            }
            .check(mat)?;
        }
        zerodds_security::crypto::CryptographicPlugin::decode_serialized_payload_with(
            self,
            slot,
            encoded_buffer,
        )
        .map(Some)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::backend::rand::SystemRandom;
    use crate::plugin::{SRTPS_FLAG_A, SRTPS_PREFIX_ID, adjust_header_extension};
    use zerodds_security::authentication::SharedSecretProvider;
    use zerodds_security::cryptographic::{
        PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED as PAYLOAD_ENC,
        PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED as SUB_ENC,
        PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED as SUB_ORIGIN,
        PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED as AXK_ENC,
        PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED as AXK_ORIGIN,
        PLUGIN_PARTICIPANT_FLAG_IS_RTPS_PSK_ENCRYPTED as PSK_ENC,
    };

    /// A SharedSecret both sides know, with the challenges of the handshake.
    struct Secret;

    impl SharedSecretProvider for Secret {
        fn get_shared_secret(&self, _: SharedSecretHandle) -> Option<Vec<u8>> {
            Some(alloc::vec![0x5a; 32])
        }
        fn get_shared_secret_challenges(
            &self,
            _: SharedSecretHandle,
        ) -> Option<([u8; 32], [u8; 32])> {
            Some(([0x11; 32], [0x22; 32]))
        }
    }

    const SECRET: SharedSecretHandle = SharedSecretHandle(1);

    fn plugin() -> AesGcmCryptoPlugin {
        AesGcmCryptoPlugin::with_secret_provider(Suite::Aes256Gcm, Arc::new(Secret))
    }

    fn pconfig(attrs: u32) -> ParticipantSecurityConfig {
        ParticipantSecurityConfig {
            is_rtps_axk_protected: true,
            plugin_participant_attributes: attrs,
            ..Default::default()
        }
    }

    fn econfig(submessage: bool, payload: bool, attrs: u32) -> EndpointSecurityConfig {
        EndpointSecurityConfig {
            is_submessage_protected: submessage,
            is_payload_protected: payload,
            plugin_endpoint_attributes: attrs,
            ..Default::default()
        }
    }

    fn guid(entity: [u8; 4]) -> GuidT {
        let mut g = [7u8; 16];
        g[12..].copy_from_slice(&entity);
        g
    }

    fn user_entity(n: u8) -> [u8; 4] {
        [0, 0, n, 0x02]
    }

    /// One participant with the remote participant it is matched with.
    struct Side {
        p: AesGcmCryptoPlugin,
        local: ParticipantCryptoHandle,
        remote: ParticipantCryptoHandle,
    }

    fn side(attrs: u32, props: &[Property]) -> Side {
        let mut p = plugin();
        let (local, _) = p
            .register_local_participant(
                IdentityHandle(1),
                PermissionsHandle(1),
                props,
                &pconfig(attrs),
            )
            .unwrap();
        let remote = p
            .register_matched_remote_participant(
                local,
                IdentityHandle(2),
                PermissionsHandle(2),
                SECRET,
            )
            .unwrap();
        Side { p, local, remote }
    }

    /// `a` sends its participant key to `b`.
    fn send_participant_key(a: &mut Side, b: &mut Side) {
        let t =
            a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .unwrap()
                .unwrap();
        assert!(
            b.p.set_remote_participant_crypto_tokens(b.local, b.remote, &t)
                .unwrap()
        );
    }

    /// A DataWriter of `a` matched with a DataReader of `b`, tokens exchanged.
    struct Link {
        w: DatawriterCryptoHandle,
        w_remote_reader: DatareaderCryptoHandle,
        r: DatareaderCryptoHandle,
        r_remote_writer: DatawriterCryptoHandle,
    }

    fn link(
        a: &mut Side,
        b: &mut Side,
        wcfg: &EndpointSecurityConfig,
        rcfg: &EndpointSecurityConfig,
    ) -> Link {
        let (w, _) =
            a.p.register_local_datawriter(a.local, &[], wcfg, guid(user_entity(1)))
                .unwrap();
        let (r, _) =
            b.p.register_local_datareader(b.local, &[], rcfg, guid(user_entity(2)))
                .unwrap();
        let w_remote_reader =
            a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                .unwrap();
        let r_remote_writer =
            b.p.register_matched_remote_datawriter(r, b.remote, SECRET)
                .unwrap();
        let wt =
            a.p.create_local_datawriter_crypto_tokens(w, w_remote_reader, 0)
                .unwrap()
                .unwrap();
        b.p.set_remote_datawriter_crypto_tokens(r, r_remote_writer, &wt)
            .unwrap();
        let rt =
            b.p.create_local_datareader_crypto_tokens(r, r_remote_writer, 0)
                .unwrap()
                .unwrap();
        a.p.set_remote_datareader_crypto_tokens(w, w_remote_reader, &rt)
            .unwrap();
        Link {
            w,
            w_remote_reader,
            r,
            r_remote_writer,
        }
    }

    /// A DATA submessage of eight octets of content.
    fn data_submessage() -> Vec<u8> {
        let mut m = alloc::vec![0x15, 0x01, 0x08, 0x00];
        m.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        m
    }

    fn rtps_message(prefix: [u8; 12]) -> Vec<u8> {
        let mut m = b"RTPS".to_vec();
        m.extend_from_slice(&[2, 5, 1, 1]);
        m.extend_from_slice(&prefix);
        m.extend_from_slice(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        m.extend_from_slice(&[0x15, 0x01, 0x08, 0x00, 9, 9, 9, 9, 9, 9, 9, 9]);
        m
    }

    fn tokens_of(
        p: &mut AesGcmCryptoPlugin,
        w: DatawriterCryptoHandle,
        r: DatareaderCryptoHandle,
    ) -> Vec<KeyMaterial> {
        p.create_local_datawriter_crypto_tokens(w, r, 0)
            .unwrap()
            .unwrap()
            .iter()
            .map(|t| keymat_of(t).unwrap())
            .collect()
    }

    /// §9.5.1.8.1, Table 78: `adjusted_algorithm_info` has the
    /// supported_mask of the algorithms the plugin can use,
    /// `builtin_kx_endpoints_required_mask` for the protection of
    /// DCPSParticipantVolatileMessageSecure (AES256-GCM), the
    /// `builtin_endpoints_required_mask` for the other builtin topics, and
    /// zero everywhere else; a nil identity or permissions handle is a
    /// failure.
    #[test]
    fn register_local_participant_fills_the_adjusted_algorithm_info() {
        let mut p = plugin();
        let mut config = pconfig(AXK_ENC);
        config.is_discovery_protected = true;
        let (h, info) = p
            .register_local_participant(IdentityHandle(1), PermissionsHandle(1), &[], &config)
            .unwrap();
        assert!(!h.is_nil());
        let sc = info.symmetric_cipher;
        assert_eq!(sc.supported_mask, CBIT_AES128_GCM | CBIT_AES256_GCM);
        assert_eq!(sc.builtin_kx_endpoints_required_mask, CBIT_AES256_GCM);
        assert_eq!(sc.builtin_endpoints_required_mask, CBIT_AES256_GCM);
        assert_eq!(sc.user_endpoints_default_required_mask, 0);
        assert_eq!(info.digital_signature, Default::default());
        assert_eq!(info.key_establishment, Default::default());
        // AES128-GCM as the cipher of the participant: the builtin endpoints
        // use that, the key exchange stays AES256-GCM.
        let props = [Property::local(SYMMETRIC_CIPHER_ALGORITHM, "AES128-GCM")];
        let (_, info) = p
            .register_local_participant(IdentityHandle(1), PermissionsHandle(1), &props, &config)
            .unwrap();
        assert_eq!(
            info.symmetric_cipher.builtin_endpoints_required_mask,
            CBIT_AES128_GCM
        );
        assert_eq!(
            info.symmetric_cipher.builtin_kx_endpoints_required_mask,
            CBIT_AES256_GCM
        );
        // Nothing of the builtin topics protected: nothing required.
        let (_, info) = p
            .register_local_participant(
                IdentityHandle(1),
                PermissionsHandle(1),
                &[],
                &pconfig(AXK_ENC),
            )
            .unwrap();
        assert_eq!(info.symmetric_cipher.builtin_endpoints_required_mask, 0);
        // A nil identity or permissions handle: HandleNIL with an exception.
        assert!(
            p.register_local_participant(IdentityHandle(0), PermissionsHandle(1), &[], &config)
                .is_err()
        );
        assert!(
            p.register_local_participant(IdentityHandle(1), PermissionsHandle(0), &[], &config)
                .is_err()
        );
        // "Properties not understood ... shall be silently ignored".
        let other = [Property::local("dds.sec.crypto.unknown_property", "x")];
        assert!(
            p.register_local_participant(IdentityHandle(1), PermissionsHandle(1), &other, &config)
                .is_ok()
        );
    }

    /// Table 78 (and the plugin's configuration): the plugin accepts only the
    /// algorithms of the resulting supported_mask.
    #[test]
    fn the_plugin_accepts_only_the_supported_algorithms() {
        let mut p = plugin();
        let mut config = pconfig(AXK_ENC);
        config.algorithm_info.symmetric_cipher.supported_mask = CBIT_AES256_GCM;
        let props = [Property::local(SYMMETRIC_CIPHER_ALGORITHM, "AES128-GCM")];
        assert!(
            p.register_local_participant(IdentityHandle(1), PermissionsHandle(1), &props, &config)
                .is_err(),
            "a cipher outside supported_mask"
        );
        let (local, info) = p
            .register_local_participant(IdentityHandle(1), PermissionsHandle(1), &[], &config)
            .unwrap();
        assert_eq!(info.symmetric_cipher.supported_mask, CBIT_AES256_GCM);
        // A key of AES128 sent by the remote side is refused.
        let remote = p
            .register_matched_remote_participant(
                local,
                IdentityHandle(2),
                PermissionsHandle(2),
                SECRET,
            )
            .unwrap();
        let mut sender = side(AXK_ENC, &props);
        let token = sender
            .p
            .create_local_participant_crypto_tokens(sender.local, sender.remote, 0)
            .unwrap()
            .unwrap();
        assert!(
            p.set_remote_participant_crypto_tokens(local, remote, &token)
                .is_err()
        );
    }

    /// Table 78, `register_local_participant`: GCM or GMAC "according to the
    /// setting of the RTPS Protection Kind", 128 or 256 bits by the
    /// property.
    #[test]
    fn the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width() {
        for (attrs, cipher, expected) in [
            (AXK_ENC, None, Suite::Aes256Gcm),
            (0, None, Suite::Aes256Gmac),
            (AXK_ENC, Some("AES128-GCM"), Suite::Aes128Gcm),
            (0, Some("AES128-GCM"), Suite::Aes128Gmac),
            (AXK_ENC, Some("AUTO"), Suite::Aes256Gcm),
        ] {
            let props: Vec<Property> = cipher
                .map(|c| Property::local(SYMMETRIC_CIPHER_ALGORITHM, c))
                .into_iter()
                .collect();
            let mut s = side(attrs, &props);
            let t =
                s.p.create_local_participant_crypto_tokens(s.local, s.remote, 0)
                    .unwrap()
                    .unwrap();
            assert_eq!(t.len(), 1);
            let mat = keymat_of(&t[0]).unwrap();
            assert_eq!(mat.suite, expected, "attrs {attrs:#x} cipher {cipher:?}");
            assert_eq!(t[0].class_id, "DDS:Crypto:AES_GCM_GMAC");
        }
    }

    /// Table 78, `register_matched_remote_participant`: the
    /// Participant2ParticipantKeyMaterial has the kind, salt, key and key id
    /// of the ParticipantKeyMaterial; under origin authentication a new
    /// receiver-specific key for each remote participant, without it none.
    #[test]
    fn the_participant2participant_material_matches_the_participant_key() {
        let mut a = side(AXK_ENC | AXK_ORIGIN, &[]);
        let second =
            a.p.register_matched_remote_participant(
                a.local,
                IdentityHandle(3),
                PermissionsHandle(3),
                SECRET,
            )
            .unwrap();
        let first = keymat_of(
            &a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        let other = keymat_of(
            &a.p.create_local_participant_crypto_tokens(a.local, second, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        assert_eq!(first.suite, other.suite);
        assert_eq!(first.transformation_key_id, other.transformation_key_id);
        assert_eq!(first.master_salt, other.master_salt);
        assert_eq!(first.master_key, other.master_key);
        let k1 = first.origin.receiver_key.expect("origin authentication");
        let k2 = other.origin.receiver_key.expect("origin authentication");
        assert_ne!(k1.id, [0; 4]);
        assert_ne!(k1.id, k2.id, "a key of its own for each remote participant");
        assert_ne!(k1.master, k2.master);
        // The same remote participant gets the same receiver-specific key again.
        let again = keymat_of(
            &a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        assert_eq!(again.origin.receiver_key.unwrap().id, k1.id);
        // Without origin authentication: receiver_specific_key_id 0 and an
        // empty master_receiver_specific_key.
        let mut plain = side(AXK_ENC, &[]);
        let m = keymat_of(
            &plain
                .p
                .create_local_participant_crypto_tokens(plain.local, plain.remote, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        assert!(m.origin.receiver_key.is_none());
        // A remote participant of another local participant is refused.
        let bogus = ParticipantCryptoHandle(u64::MAX - 5);
        assert!(
            a.p.create_local_participant_crypto_tokens(a.local, bogus, 0)
                .is_err()
        );
        // The remote side takes the sequence of one token, and no other.
        let mut b = side(AXK_ENC | AXK_ORIGIN, &[]);
        let t =
            a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .unwrap()
                .unwrap();
        assert!(
            b.p.set_remote_participant_crypto_tokens(b.local, b.remote, &[])
                .is_err()
        );
        let mut two = t.clone();
        two.extend(t.clone());
        assert!(
            b.p.set_remote_participant_crypto_tokens(b.local, b.remote, &two)
                .is_err()
        );
        let wrong_class = [DataHolder::new("DDS:Crypto:Other")];
        assert!(
            b.p.set_remote_participant_crypto_tokens(b.local, b.remote, &wrong_class)
                .is_err()
        );
    }

    /// Table 78, `register_local_datawriter`: the WriterKeyMaterialSeq has one
    /// or two elements by the Metadata and the Data Protection Kind, the key
    /// of the submessages first; `adjusted_algorithm_info.required_mask` has
    /// the algorithms used.
    #[test]
    fn a_datawriter_has_one_or_two_key_materials() {
        let mut a = side(AXK_ENC, &[]);
        let mut reg = |cfg: EndpointSecurityConfig, props: &[Property]| {
            let (w, info) =
                a.p.register_local_datawriter(a.local, props, &cfg, guid(user_entity(1)))
                    .unwrap();
            let r =
                a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                    .unwrap();
            (tokens_of(&mut a.p, w, r), info)
        };
        let (both, info) = reg(econfig(true, true, SUB_ENC | PAYLOAD_ENC), &[]);
        assert_eq!(both.len(), 2, "metadata and data protected");
        assert_ne!(both[0].transformation_key_id, both[1].transformation_key_id);
        assert_eq!(both[0].suite, Suite::Aes256Gcm);
        assert_eq!(both[1].suite, Suite::Aes256Gcm);
        assert_eq!(info.symmetric_cipher.required_mask, CBIT_AES256_GCM);
        assert_eq!(
            info.symmetric_cipher.supported_mask,
            CBIT_AES128_GCM | CBIT_AES256_GCM
        );
        // Submessage SIGN, payload ENCRYPT: the first is the key of the
        // submessages (GMAC), the second the key of the payload (GCM).
        let (mixed, _) = reg(econfig(true, true, PAYLOAD_ENC), &[]);
        assert_eq!(mixed.len(), 2);
        assert_eq!(mixed[0].suite, Suite::Aes256Gmac);
        assert_eq!(mixed[1].suite, Suite::Aes256Gcm);
        let (sub_only, _) = reg(econfig(true, false, SUB_ENC), &[]);
        assert_eq!(sub_only.len(), 1);
        assert_eq!(sub_only[0].suite, Suite::Aes256Gcm);
        let (pay_only, _) = reg(econfig(false, true, PAYLOAD_ENC), &[]);
        assert_eq!(pay_only.len(), 1);
        // The cipher of the endpoint overrides the participant's (Table 71).
        let (small, info) = reg(
            econfig(true, true, SUB_ENC | PAYLOAD_ENC),
            &[Property::local(SYMMETRIC_CIPHER_ALGORITHM, "AES128-GCM")],
        );
        assert_eq!(small[0].suite, Suite::Aes128Gcm);
        assert_eq!(small[1].suite, Suite::Aes128Gcm);
        assert_eq!(info.symmetric_cipher.required_mask, CBIT_AES128_GCM);
        // Neither submessages nor payload protected: no key material.
        assert!(
            a.p.register_local_datawriter(
                a.local,
                &[],
                &econfig(false, false, 0),
                guid(user_entity(1))
            )
            .is_err()
        );
        // An unknown participant handle.
        assert!(
            a.p.register_local_datawriter(
                ParticipantCryptoHandle(u64::MAX - 1),
                &[],
                &econfig(true, false, 0),
                guid(user_entity(1))
            )
            .is_err()
        );
    }

    /// §9.5.1.9.3: for a relay-only reader "the Tokens returned contain only
    /// the cryptographic material needed to verify and decode the RTPS
    /// SubMessage but not the CryptoContent".
    #[test]
    fn a_relay_only_reader_gets_the_submessage_key_alone() {
        let mut a = side(AXK_ENC, &[]);
        let (w, _) =
            a.p.register_local_datawriter(
                a.local,
                &[],
                &econfig(true, true, SUB_ENC | PAYLOAD_ENC),
                guid(user_entity(1)),
            )
            .unwrap();
        let full =
            a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                .unwrap();
        let relay =
            a.p.register_matched_remote_datareader(w, a.remote, SECRET, true)
                .unwrap();
        let all = tokens_of(&mut a.p, w, full);
        let some = tokens_of(&mut a.p, w, relay);
        assert_eq!(all.len(), 2);
        assert_eq!(some.len(), 1);
        assert_eq!(some[0].transformation_key_id, all[0].transformation_key_id);
        // A writer with one key for both: the relay gets none.
        let (single, _) =
            a.p.register_local_datawriter(
                a.local,
                &[],
                &econfig(false, true, PAYLOAD_ENC),
                guid(user_entity(2)),
            )
            .unwrap();
        let relay2 =
            a.p.register_matched_remote_datareader(single, a.remote, SECRET, true)
                .unwrap();
        assert!(tokens_of(&mut a.p, single, relay2).is_empty());
    }

    /// Table 78, `register_matched_remote_datareader` and
    /// `register_matched_remote_datawriter`: with origin authentication in
    /// the Metadata Protection Kind each remote endpoint gets a receiver-
    /// specific key of its own; the Reader2WriterKeyMaterial has the kind,
    /// salt, key and key id of the ReaderKeyMaterial.
    #[test]
    fn the_endpoints_get_receiver_specific_keys_under_origin_authentication() {
        let mut a = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC | SUB_ORIGIN);
        let (w, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(user_entity(1)))
                .unwrap();
        let r1 =
            a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                .unwrap();
        let r2 =
            a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                .unwrap();
        let t1 = tokens_of(&mut a.p, w, r1);
        let t2 = tokens_of(&mut a.p, w, r2);
        assert_eq!(t1[0].transformation_key_id, t2[0].transformation_key_id);
        assert_eq!(t1[0].master_key, t2[0].master_key);
        let k1 = t1[0].origin.receiver_key.clone().expect("receiver key");
        let k2 = t2[0].origin.receiver_key.clone().expect("receiver key");
        assert_ne!(k1.id, k2.id);
        // Reader side.
        let (r, _) =
            a.p.register_local_datareader(a.local, &[], &cfg, guid(user_entity(3)))
                .unwrap();
        let w1 =
            a.p.register_matched_remote_datawriter(r, a.remote, SECRET)
                .unwrap();
        let w2 =
            a.p.register_matched_remote_datawriter(r, a.remote, SECRET)
                .unwrap();
        let rt1 = keymat_of(
            &a.p.create_local_datareader_crypto_tokens(r, w1, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        let rt2 = keymat_of(
            &a.p.create_local_datareader_crypto_tokens(r, w2, 0)
                .unwrap()
                .unwrap()[0],
        )
        .unwrap();
        assert_eq!(rt1.transformation_key_id, rt2.transformation_key_id);
        assert_eq!(rt1.master_key, rt2.master_key);
        assert_ne!(
            rt1.origin.receiver_key.unwrap().id,
            rt2.origin.receiver_key.unwrap().id
        );
        // Without origin authentication none.
        let (w_plain, _) =
            a.p.register_local_datawriter(
                a.local,
                &[],
                &econfig(true, false, SUB_ENC),
                guid(user_entity(4)),
            )
            .unwrap();
        let rr =
            a.p.register_matched_remote_datareader(w_plain, a.remote, SECRET, false)
                .unwrap();
        assert!(
            tokens_of(&mut a.p, w_plain, rr)[0]
                .origin
                .receiver_key
                .is_none()
        );
    }

    /// §9.5.1.10.2, .3, .6, .7, .8: a DataWriter's submessage goes through
    /// `encode_datawriter_submessage`, `preprocess_secure_submsg` (category
    /// DATAWRITER_SUBMESSAGE, the handles of the pair) and
    /// `decode_datawriter_submessage`; a DataReader's through the reader
    /// operations with the category DATAREADER_SUBMESSAGE.
    #[test]
    fn submessages_go_through_preprocess_and_decode() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC | SUB_ORIGIN);
        let l = link(&mut a, &mut b, &cfg, &cfg);
        let plain = data_submessage();
        // Writer to reader.
        let mut encoded = Vec::new();
        let mut index = 0;
        assert!(
            a.p.encode_datawriter_submessage(
                &mut encoded,
                &plain,
                l.w,
                &[l.w_remote_reader],
                &mut index
            )
            .unwrap()
        );
        assert_eq!(index, 1, "the iteration over the list is complete");
        assert_eq!(encoded[0], SEC_PREFIX_ID);
        let pre =
            b.p.preprocess_secure_submsg(&encoded, b.local, b.remote)
                .unwrap()
                .unwrap();
        assert_eq!(
            pre.secure_submessage_category,
            SecureSubmessageCategory::DatawriterSubmessage
        );
        assert_eq!(pre.datawriter_crypto, l.r_remote_writer);
        assert_eq!(pre.datareader_crypto, l.r);
        let decoded = b
            .p
            .decode_datawriter_submessage(&encoded, pre.datareader_crypto, pre.datawriter_crypto)
            .unwrap()
            .unwrap();
        assert_eq!(decoded, plain);
        // Reader to writer.
        let ack = {
            let mut m = alloc::vec![0x06, 0x01, 0x08, 0x00];
            m.extend_from_slice(&[9, 8, 7, 6, 5, 4, 3, 2]);
            m
        };
        let encoded =
            b.p.encode_datareader_submessage(&ack, l.r, &[l.r_remote_writer])
                .unwrap()
                .unwrap();
        let pre =
            a.p.preprocess_secure_submsg(&encoded, a.local, a.remote)
                .unwrap()
                .unwrap();
        assert_eq!(
            pre.secure_submessage_category,
            SecureSubmessageCategory::DatareaderSubmessage
        );
        assert_eq!(pre.datawriter_crypto, l.w);
        assert_eq!(pre.datareader_crypto, l.w_remote_reader);
        let decoded = a
            .p
            .decode_datareader_submessage(&encoded, pre.datawriter_crypto, pre.datareader_crypto)
            .unwrap()
            .unwrap();
        assert_eq!(decoded, ack);
        // A tampered submessage fails.
        let mut bad_wire = encoded.clone();
        bad_wire[30] ^= 0x55;
        assert!(
            a.p.decode_datareader_submessage(&bad_wire, l.w, l.w_remote_reader)
                .is_err()
        );
    }

    /// Table 80: `preprocess_secure_submsg` "determine[s] whether the
    /// CryptoTransformIdentifier the transformation_kind matches one of the
    /// recognized kinds", and fails for a key that no handle of the sender
    /// is associated with.
    #[test]
    fn preprocess_refuses_unknown_kinds_and_unknown_keys() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC);
        let l = link(&mut a, &mut b, &cfg, &cfg);
        let mut encoded = Vec::new();
        let mut index = 0;
        a.p.encode_datawriter_submessage(
            &mut encoded,
            &data_submessage(),
            l.w,
            &[l.w_remote_reader],
            &mut index,
        )
        .unwrap();
        // An algorithm id of Table 22 that does not exist.
        let mut unknown_kind = encoded.clone();
        unknown_kind[4 + 3] = 0x77;
        assert!(
            b.p.preprocess_secure_submsg(&unknown_kind, b.local, b.remote)
                .is_err()
        );
        // A key id nobody sent.
        let mut unknown_key = encoded.clone();
        unknown_key[4 + 4] ^= 0xff;
        assert!(
            b.p.preprocess_secure_submsg(&unknown_key, b.local, b.remote)
                .is_err()
        );
        // The right key, asked for as sent by a participant that is not the
        // sender (a second remote participant).
        let other =
            b.p.register_matched_remote_participant(
                b.local,
                IdentityHandle(9),
                PermissionsHandle(9),
                SECRET,
            )
            .unwrap();
        assert!(
            b.p.preprocess_secure_submsg(&encoded, b.local, other)
                .is_err()
        );
        // Not a SEC_PREFIX.
        assert!(
            b.p.preprocess_secure_submsg(&data_submessage(), b.local, b.remote)
                .is_err()
        );
        // The category of an INFO submessage is never reported: this plugin
        // wraps none.
        assert_ne!(
            b.p.preprocess_secure_submsg(&encoded, b.local, b.remote)
                .unwrap()
                .unwrap()
                .secure_submessage_category,
            SecureSubmessageCategory::InfoSubmessage
        );
    }

    /// §9.5.1.10.1 and §9.5.1.10.9: the payload goes through
    /// `encode_serialized_payload` (an empty `extra_inline_qos`) and
    /// `decode_serialized_payload`, with the payload key of a writer that has
    /// two; a writer whose payload is not protected transforms nothing.
    #[test]
    fn the_serialized_payload_goes_through_the_interface() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let both = econfig(true, true, SUB_ENC | PAYLOAD_ENC);
        let l = link(&mut a, &mut b, &both, &both);
        let payload = b"serialized payload of the sample".to_vec();
        let encoded =
            a.p.encode_serialized_payload(&payload, l.w)
                .unwrap()
                .unwrap();
        assert!(encoded.extra_inline_qos.is_empty());
        assert_ne!(
            &encoded.encoded_buffer[24..24 + payload.len()],
            payload.as_slice(),
            "encrypted"
        );
        let plain =
            b.p.decode_serialized_payload(&encoded.encoded_buffer, &[], l.r, l.r_remote_writer)
                .unwrap()
                .unwrap();
        assert_eq!(plain, payload);
        // Authentication only for the payload.
        let sign = econfig(false, true, 0);
        let l2 = {
            let (w, _) =
                a.p.register_local_datawriter(a.local, &[], &sign, guid(user_entity(5)))
                    .unwrap();
            let (r, _) =
                b.p.register_local_datareader(b.local, &[], &sign, guid(user_entity(6)))
                    .unwrap();
            let wr =
                a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                    .unwrap();
            let rw =
                b.p.register_matched_remote_datawriter(r, b.remote, SECRET)
                    .unwrap();
            let t =
                a.p.create_local_datawriter_crypto_tokens(w, wr, 0)
                    .unwrap()
                    .unwrap();
            b.p.set_remote_datawriter_crypto_tokens(r, rw, &t).unwrap();
            (w, r, rw)
        };
        let signed =
            a.p.encode_serialized_payload(&payload, l2.0)
                .unwrap()
                .unwrap();
        assert_eq!(
            &signed.encoded_buffer[20..20 + payload.len()],
            payload.as_slice(),
            "GMAC leaves the payload as it is"
        );
        assert_eq!(
            b.p.decode_serialized_payload(&signed.encoded_buffer, &[], l2.1, l2.2)
                .unwrap()
                .unwrap(),
            payload
        );
        // A writer that does not protect its payload: nothing to transform.
        let (sub_only, _) =
            a.p.register_local_datawriter(
                a.local,
                &[],
                &econfig(true, false, SUB_ENC),
                guid(user_entity(7)),
            )
            .unwrap();
        assert!(
            a.p.encode_serialized_payload(&payload, sub_only)
                .unwrap()
                .is_none()
        );
    }

    /// §10.5.3 Table 78, `register_local_datawriter`: where metadata and data
    /// protection are the same, a writer may reuse one key material for both
    /// (Fast DDS 3.6 does when both encrypt: AESGCMGMAC_KeyFactory.cpp,
    /// `is_payload_encrypted != is_sub_encrypted`; source read). Under origin
    /// authentication `register_matched_remote_datareader` puts the
    /// receiver-specific key into that first element. Table 80,
    /// `encode_serialized_payload`, sets the `receiver_specific_macs` of a
    /// payload "always" to the empty sequence, so the receiver-specific part
    /// of the key material has no role for the payload. A payload sealed with
    /// such a reused key opens; one with receiver-specific MACs does not.
    #[test]
    fn a_payload_under_a_key_reused_with_origin_authentication_opens() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, true, SUB_ENC | SUB_ORIGIN | PAYLOAD_ENC);
        let l = link(&mut a, &mut b, &cfg, &cfg);
        let tokens =
            a.p.create_local_datawriter_crypto_tokens(l.w, l.w_remote_reader, 0)
                .unwrap()
                .unwrap();
        let reused = keymat_of(&tokens[0]).unwrap();
        assert!(
            reused.origin.receiver_key.is_some(),
            "the first element carries it"
        );
        // The reader of a writer that sends only that one element.
        let (r, _) =
            b.p.register_local_datareader(b.local, &[], &cfg, guid(user_entity(8)))
                .unwrap();
        let rw =
            b.p.register_matched_remote_datawriter(r, b.remote, SECRET)
                .unwrap();
        b.p.set_remote_datawriter_crypto_tokens(r, rw, &tokens[..1])
            .unwrap();
        let payload = b"serialized payload of the sample".to_vec();
        let sealed = super::super::encode_payload_from_mat(&reused, &payload).unwrap();
        assert_eq!(
            b.p.decode_serialized_payload(&sealed, &[], r, rw)
                .unwrap()
                .unwrap(),
            payload
        );
        // A footer that names receiver-specific MACs is refused.
        let mut with_macs = sealed.clone();
        let n = with_macs.len();
        with_macs[n - 4..].copy_from_slice(&1u32.to_be_bytes());
        assert!(
            b.p.decode_serialized_payload(&with_macs, &[], r, rw)
                .is_err()
        );
    }

    /// Table 80: the key has to be "associated with those CryptoHandles":
    /// the key of one remote writer does not decode what another writer, or
    /// a reader that is not the one matched, sends or receives.
    #[test]
    fn a_key_is_taken_only_from_the_entities_it_is_associated_with() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, true, SUB_ENC | PAYLOAD_ENC);
        let l1 = link(&mut a, &mut b, &cfg, &cfg);
        let l2 = link(&mut a, &mut b, &cfg, &cfg);
        let mut encoded = Vec::new();
        let mut index = 0;
        a.p.encode_datawriter_submessage(
            &mut encoded,
            &data_submessage(),
            l1.w,
            &[l1.w_remote_reader],
            &mut index,
        )
        .unwrap();
        // Right writer, right reader.
        assert!(
            b.p.decode_datawriter_submessage(&encoded, l1.r, l1.r_remote_writer)
                .is_ok()
        );
        // The other writer's handle: the key id is not associated with it.
        assert!(
            b.p.decode_datawriter_submessage(&encoded, l2.r, l2.r_remote_writer)
                .is_err()
        );
        // The writer matched with the other reader.
        assert!(
            b.p.decode_datawriter_submessage(&encoded, l1.r, l2.r_remote_writer)
                .is_err()
        );
        // The same for the payload.
        let p1 = a.p.encode_serialized_payload(b"x", l1.w).unwrap().unwrap();
        assert!(
            b.p.decode_serialized_payload(&p1.encoded_buffer, &[], l1.r, l1.r_remote_writer)
                .is_ok()
        );
        assert!(
            b.p.decode_serialized_payload(&p1.encoded_buffer, &[], l2.r, l2.r_remote_writer)
                .is_err()
        );
        // And a key of the reader is no key of a writer.
        assert!(
            b.p.decode_datareader_submessage(&encoded, l1.r_remote_writer, l1.r)
                .is_err()
        );
    }

    /// Table 80, `decode_rtps_message`, `decode_datawriter_submessage`:
    /// "the transformation_kind matches the one the receiving
    /// DomainParticipant is expecting both in terms of the type of
    /// algorithm as well as the protection"; "If the kind is not the
    /// expected one, the operation shall fail".
    #[test]
    fn a_kind_other_than_the_expected_one_is_refused() {
        let mut a = side(0, &[]);
        let mut b = side(AXK_ENC, &[]);
        // The sender signs, the receiver expects encryption.
        send_participant_key(&mut a, &mut b);
        let plain = rtps_message([3; 12]);
        let mut encoded = Vec::new();
        let mut index = 0;
        a.p.encode_rtps_message(
            &mut encoded,
            &plain,
            a.local,
            &[a.remote],
            &mut index,
            false,
        )
        .unwrap();
        let err =
            b.p.decode_rtps_message(&encoded, b.local, b.remote)
                .unwrap_err();
        assert!(err.detail.contains("not the one expected"), "{err}");
        // The sender encrypts and authenticates the origin, the receiver does
        // not expect origin authentication.
        let mut a = side(AXK_ENC | AXK_ORIGIN, &[]);
        let mut b = side(AXK_ENC, &[]);
        send_participant_key(&mut a, &mut b);
        index = 0;
        a.p.encode_rtps_message(
            &mut encoded,
            &plain,
            a.local,
            &[a.remote],
            &mut index,
            false,
        )
        .unwrap();
        assert!(
            b.p.decode_rtps_message(&encoded, b.local, b.remote)
                .is_err()
        );
        // Submessages: the reader expects encryption, the writer only signs.
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let l = link(
            &mut a,
            &mut b,
            &econfig(true, false, 0),
            &econfig(true, false, SUB_ENC),
        );
        let mut wire = Vec::new();
        let mut i = 0;
        a.p.encode_datawriter_submessage(
            &mut wire,
            &data_submessage(),
            l.w,
            &[l.w_remote_reader],
            &mut i,
        )
        .unwrap();
        let err =
            b.p.decode_datawriter_submessage(&wire, l.r, l.r_remote_writer)
                .unwrap_err();
        assert!(err.detail.contains("not the one expected"), "{err}");
        // A reader of the same kind decodes it.
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let l = link(
            &mut a,
            &mut b,
            &econfig(true, false, 0),
            &econfig(true, false, 0),
        );
        i = 0;
        a.p.encode_datawriter_submessage(
            &mut wire,
            &data_submessage(),
            l.w,
            &[l.w_remote_reader],
            &mut i,
        )
        .unwrap();
        assert_eq!(
            b.p.decode_datawriter_submessage(&wire, l.r, l.r_remote_writer)
                .unwrap()
                .unwrap(),
            data_submessage()
        );
    }

    /// Table 80 `decode_rtps_message`: a receiver whose RTPS Protection Kind
    /// asks for origin authentication refuses an RTPS message under a key
    /// without a receiver-specific key ("If the kind is not the expected one,
    /// the operation shall fail"); the same holds for a submessage under a
    /// Metadata Protection Kind with origin authentication. Under
    /// `PROP_ORIGIN_AUTHENTICATION_FALLBACK = accept` both are verified with
    /// the common MAC alone, as Table 80 does for a key whose
    /// receiver_specific_key_id is zero (the form of a sender that makes no
    /// receiver-specific keys).
    #[test]
    fn a_key_without_a_receiver_specific_key_is_refused_under_origin_authentication_by_default() {
        let fallback = [Property::local(
            PROP_ORIGIN_AUTHENTICATION_FALLBACK,
            "accept",
        )];
        for (props, opens) in [(&[][..], false), (&fallback[..], true)] {
            // `a` makes no receiver-specific keys; `b` expects them.
            let mut a = side(AXK_ENC, &[]);
            let mut b = side(AXK_ENC | AXK_ORIGIN, props);
            send_participant_key(&mut a, &mut b);
            let plain = rtps_message([3; 12]);
            let mut encoded = Vec::new();
            let mut index = 0;
            assert!(
                a.p.encode_rtps_message(
                    &mut encoded,
                    &plain,
                    a.local,
                    &[a.remote],
                    &mut index,
                    false
                )
                .unwrap()
            );
            let decoded = b.p.decode_rtps_message(&encoded, b.local, b.remote);
            if opens {
                assert_eq!(decoded.unwrap().unwrap(), plain);
            } else {
                assert!(decoded.is_err(), "refused without the setting");
            }
            let l = link(
                &mut a,
                &mut b,
                &econfig(true, false, SUB_ENC),
                &econfig(true, false, SUB_ENC | SUB_ORIGIN),
            );
            let mut encoded = Vec::new();
            let mut index = 0;
            a.p.encode_datawriter_submessage(
                &mut encoded,
                &data_submessage(),
                l.w,
                &[l.w_remote_reader],
                &mut index,
            )
            .unwrap();
            let decoded =
                b.p.decode_datawriter_submessage(&encoded, l.r, l.r_remote_writer);
            if opens {
                assert_eq!(decoded.unwrap().unwrap(), data_submessage());
            } else {
                assert!(decoded.is_err(), "refused without the setting");
            }
        }
        // A key that carries a receiver-specific key is held to it under the
        // setting too: the tag of another receiver does not verify.
        let mut a = side(AXK_ENC | AXK_ORIGIN, &[]);
        let mut b = side(AXK_ENC | AXK_ORIGIN, &fallback);
        send_participant_key(&mut a, &mut b);
        let other =
            a.p.register_matched_remote_participant(
                a.local,
                IdentityHandle(3),
                PermissionsHandle(3),
                SECRET,
            )
            .unwrap();
        let mut encoded = Vec::new();
        let mut index = 0;
        a.p.encode_rtps_message(
            &mut encoded,
            &rtps_message([3; 12]),
            a.local,
            &[other],
            &mut index,
            false,
        )
        .unwrap();
        assert!(
            b.p.decode_rtps_message(&encoded, b.local, b.remote)
                .is_err()
        );
        // Any other value is refused when the participant is registered.
        let mut p = plugin();
        assert!(
            p.register_local_participant(
                IdentityHandle(1),
                PermissionsHandle(1),
                &[Property::local(PROP_ORIGIN_AUTHENTICATION_FALLBACK, "yes")],
                &pconfig(AXK_ENC | AXK_ORIGIN),
            )
            .is_err()
        );
    }

    /// §9.5.1.10.4, Table 80, 1): `encode_rtps_message` and
    /// `decode_rtps_message` with the INFO_SRC form, GCM and GMAC.
    #[test]
    fn an_rtps_message_goes_through_the_interface_without_aad() {
        for attrs in [AXK_ENC, 0, AXK_ENC | AXK_ORIGIN, AXK_ORIGIN] {
            let mut a = side(attrs, &[]);
            let mut b = side(attrs, &[]);
            send_participant_key(&mut a, &mut b);
            let plain = rtps_message([3; 12]);
            let mut encoded = Vec::new();
            let mut index = 0;
            assert!(
                a.p.encode_rtps_message(
                    &mut encoded,
                    &plain,
                    a.local,
                    &[a.remote],
                    &mut index,
                    false
                )
                .unwrap()
            );
            assert_eq!(index, 1);
            assert_eq!(encoded[21] & SRTPS_FLAG_A, 0, "the flag A is unset");
            assert_eq!(encoded[21] & SRTPS_FLAG_P, 0, "the flag P is unset");
            let decoded =
                b.p.decode_rtps_message(&encoded, b.local, b.remote)
                    .unwrap()
                    .unwrap();
            assert_eq!(decoded, plain, "attrs {attrs:#x}");
        }
        // With the AAD form disabled the message cannot contain a HeaderExtension.
        let a = side(AXK_ENC, &[]);
        let mut with_extension = rtps_message([3; 12])[..20].to_vec();
        with_extension.extend_from_slice(&[0x00, 0x03, 0x04, 0x00, 0, 0, 0, 0]);
        with_extension.extend_from_slice(&rtps_message([3; 12])[20..]);
        let mut out = Vec::new();
        let mut i = 0;
        assert!(
            a.p.encode_rtps_message(
                &mut out,
                &with_extension,
                a.local,
                &[a.remote],
                &mut i,
                false
            )
            .is_err()
        );
    }

    /// Table 80, 2) and 3): with "Additional Authenticated Data" the RTPS
    /// Header and the HeaderExtension stay, the flag A is set, no INFO_SRC
    /// goes in, a tampered header fails, and `messageLength` and
    /// `messageChecksum` of the extension describe the encoded message.
    #[test]
    fn an_rtps_message_goes_through_the_interface_with_aad_and_a_header_extension() {
        let aad = [Property::local(PROP_RTPS_AAD, "true")];
        for attrs in [AXK_ENC, 0, AXK_ENC | AXK_ORIGIN] {
            let mut a = side(attrs, &aad);
            let mut b = side(attrs, &aad);
            send_participant_key(&mut a, &mut b);
            // E | L | C=01: messageLength, messageChecksum (CRC-32C).
            let mut plain = rtps_message([3; 12])[..20].to_vec();
            plain.extend_from_slice(&[0x00, 0x23, 0x08, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]);
            plain.extend_from_slice(&rtps_message([3; 12])[20..]);
            adjust_header_extension(&mut plain).unwrap();
            let mut encoded = Vec::new();
            let mut index = 0;
            assert!(
                a.p.encode_rtps_message(
                    &mut encoded,
                    &plain,
                    a.local,
                    &[a.remote],
                    &mut index,
                    false
                )
                .unwrap()
            );
            assert_eq!(&encoded[..20], &plain[..20], "the RTPS Header stays");
            assert_eq!(
                &encoded[20..22],
                &[0x00, 0x23],
                "the HeaderExtension follows"
            );
            // The SRTPS_PREFIX follows the extension; flag A set, P unset.
            let prefix_at = 20 + 4 + 8;
            assert_eq!(encoded[prefix_at], SRTPS_PREFIX_ID);
            assert_eq!(encoded[prefix_at + 1] & SRTPS_FLAG_A, SRTPS_FLAG_A);
            assert_eq!(encoded[prefix_at + 1] & SRTPS_FLAG_P, 0);
            // messageLength = length of the encoded message; the checksum is
            // the CRC-32C of it with the field zero.
            let length = u32::from_le_bytes([encoded[24], encoded[25], encoded[26], encoded[27]]);
            assert_eq!(length as usize, encoded.len());
            let mut zeroed = encoded.clone();
            zeroed[28..32].fill(0);
            assert_eq!(
                &encoded[28..32],
                &zerodds_foundation::crc32c(&zeroed).to_be_bytes()
            );
            // No INFO_SRC: in GMAC the submessages follow the prefix as they are.
            if attrs & AXK_ENC == 0 {
                assert_eq!(encoded[prefix_at + 4 + 20], 0x09, "INFO_TS, not INFO_SRC");
            }
            let decoded =
                b.p.decode_rtps_message(&encoded, b.local, b.remote)
                    .unwrap()
                    .unwrap();
            assert_eq!(
                decoded, plain,
                "attrs {attrs:#x}: the input of the encoding"
            );
            // The header is authenticated: another GUID prefix fails.
            let mut forged = encoded.clone();
            forged[10] ^= 0x01;
            assert!(
                b.p.decode_rtps_message(&forged, b.local, b.remote).is_err(),
                "attrs {attrs:#x}"
            );
        }
    }

    /// §9.5.1.10.2: `receiving_datareader_crypto_list_index` iterates over
    /// the list; every call puts the receiver-specific MACs of the next
    /// readers into the encoded submessage and replaces those of the call
    /// before, the common MAC and the content staying.
    #[test]
    fn the_receiver_specific_macs_follow_the_list_index() {
        let n = RECEIVER_MACS_PER_MESSAGE + 4;
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC | SUB_ORIGIN);
        let (w, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(user_entity(1)))
                .unwrap();
        let mut readers = Vec::new();
        let mut local_readers = Vec::new();
        for i in 0..n {
            let (r, _) =
                b.p.register_local_datareader(
                    b.local,
                    &[],
                    &cfg,
                    guid([1, (i >> 8) as u8, i as u8, 0x04]),
                )
                .unwrap();
            let rr =
                a.p.register_matched_remote_datareader(w, a.remote, SECRET, false)
                    .unwrap();
            let rw =
                b.p.register_matched_remote_datawriter(r, b.remote, SECRET)
                    .unwrap();
            let t =
                a.p.create_local_datawriter_crypto_tokens(w, rr, 0)
                    .unwrap()
                    .unwrap();
            b.p.set_remote_datawriter_crypto_tokens(r, rw, &t).unwrap();
            readers.push(rr);
            local_readers.push((r, rw));
        }
        let plain = data_submessage();
        let mut encoded = Vec::new();
        let mut index = 0;
        a.p.encode_datawriter_submessage(&mut encoded, &plain, w, &readers, &mut index)
            .unwrap();
        assert_eq!(index, RECEIVER_MACS_PER_MESSAGE, "the first batch");
        let count = |wire: &[u8]| {
            let (_, _, footer) = parse_secure_submessages_footer(wire).unwrap();
            u32::from_be_bytes([footer[16], footer[17], footer[18], footer[19]]) as usize
        };
        assert_eq!(count(&encoded), RECEIVER_MACS_PER_MESSAGE);
        let first = encoded.clone();
        // Reader 0 is in the first batch, the last reader is not.
        assert!(
            b.p.decode_datawriter_submessage(&encoded, local_readers[0].0, local_readers[0].1)
                .is_ok()
        );
        let last = n - 1;
        assert!(
            b.p.decode_datawriter_submessage(
                &encoded,
                local_readers[last].0,
                local_readers[last].1
            )
            .is_err(),
            "no MAC for it yet"
        );
        // The second call: plain empty, the earlier encoded message in.
        assert!(
            a.p.encode_datawriter_submessage(&mut Vec::new(), &[], w, &readers, &mut 1)
                .is_err(),
            "a non-zero index needs the earlier message"
        );
        assert!(
            a.p.encode_datawriter_submessage(&mut encoded, &plain, w, &readers, &mut index)
                .is_err(),
            "a non-zero index takes the empty plain submessage"
        );
        assert!(
            a.p.encode_datawriter_submessage(&mut encoded, &[], w, &readers, &mut index)
                .unwrap()
        );
        assert_eq!(index, n, "the list is done");
        assert_eq!(count(&encoded), 4);
        assert!(
            b.p.decode_datawriter_submessage(
                &encoded,
                local_readers[last].0,
                local_readers[last].1
            )
            .is_ok()
        );
        assert!(
            b.p.decode_datawriter_submessage(&encoded, local_readers[0].0, local_readers[0].1)
                .is_err(),
            "its MAC was replaced"
        );
        // Content and common MAC stayed: everything before the SEC_POSTFIX
        // agrees, and so does its common_mac.
        let head = |wire: &[u8], macs: usize| wire.len() - (4 + 16 + 4 + 20 * macs);
        assert_eq!(
            first[..head(&first, RECEIVER_MACS_PER_MESSAGE)],
            encoded[..head(&encoded, 4)]
        );
        assert_eq!(
            first[head(&first, RECEIVER_MACS_PER_MESSAGE) + 4..][..16],
            encoded[head(&encoded, 4) + 4..][..16]
        );
        // A list entry that is no reader of this writer is refused.
        assert!(
            a.p.encode_datawriter_submessage(
                &mut Vec::new(),
                &plain,
                w,
                &[DatareaderCryptoHandle(u64::MAX - 3)],
                &mut 0
            )
            .is_err()
        );
    }

    /// Table 44, `encode_rtps_message` with `transform_with_psk`: the
    /// pre-shared key material of §10.5.2.1.3; the flags A and P; refused
    /// without a configured passphrase.
    #[test]
    fn transform_with_psk_uses_the_pre_shared_key() {
        let props = [
            Property::local(PROP_DOMAIN_ID, "201"),
            Property::local(
                "dds.sec.crypto.rtps_psk_secret_passphrase",
                "data:,5632:castle super radar denial swing",
            ),
            Property::local(
                "dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm",
                "AES128-GCM",
            ),
        ];
        let mut config = pconfig(AXK_ENC | PSK_ENC);
        config.is_rtps_psk_protected = true;
        let make = |config: &ParticipantSecurityConfig, props: &[Property]| {
            let mut p = plugin();
            let (local, _) = p
                .register_local_participant(IdentityHandle(1), PermissionsHandle(1), props, config)
                .unwrap();
            let remote = p
                .register_matched_remote_participant(
                    local,
                    IdentityHandle(2),
                    PermissionsHandle(2),
                    SECRET,
                )
                .unwrap();
            (p, local, remote)
        };
        let (a, al, ar) = make(&config, &props);
        let (b, bl, br) = make(&config, &props);
        let plain = rtps_message([5; 12]);
        let mut encoded = Vec::new();
        let mut index = 0;
        assert!(
            a.encode_rtps_message(&mut encoded, &plain, al, &[ar], &mut index, true)
                .unwrap()
        );
        assert_eq!(
            encoded[21] & (SRTPS_FLAG_A | SRTPS_FLAG_P),
            SRTPS_FLAG_A | SRTPS_FLAG_P
        );
        assert_eq!(
            &encoded[20 + 4..20 + 4 + 4],
            &[0, 0, 22, 2],
            "revision of 5632, AES128-GCM"
        );
        assert_eq!(
            b.decode_rtps_message(&encoded, bl, br).unwrap().unwrap(),
            plain
        );
        // A participant with another passphrase does not decode it.
        let other_props = [
            Property::local(PROP_DOMAIN_ID, "201"),
            Property::local(
                "dds.sec.crypto.rtps_psk_secret_passphrase",
                "data:,5632:another secret passphrase",
            ),
            Property::local(
                "dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm",
                "AES128-GCM",
            ),
        ];
        let (c, cl, cr) = make(&config, &other_props);
        assert!(c.decode_rtps_message(&encoded, cl, cr).is_err());
        // Without the passphrase configured transform_with_psk fails.
        let (d, dl, dr) = make(&pconfig(AXK_ENC), &[]);
        assert!(
            d.encode_rtps_message(&mut Vec::new(), &plain, dl, &[dr], &mut 0, true)
                .is_err()
        );
        // The passphrase property without the DomainId is a configuration error.
        let mut p = plugin();
        assert!(
            p.register_local_participant(
                IdentityHandle(1),
                PermissionsHandle(1),
                &props[1..],
                &config
            )
            .is_err()
        );
        // axk off and no psk: "may optionally not perform any transformation".
        let mut no_axk = pconfig(0);
        no_axk.is_rtps_axk_protected = false;
        let (e, el, er) = make(&no_axk, &[]);
        let mut out = alloc::vec![1u8, 2, 3];
        assert!(
            !e.encode_rtps_message(&mut out, &plain, el, &[er], &mut 0, false)
                .unwrap(),
            "FALSE without an exception"
        );
        assert_eq!(out, [1, 2, 3], "the output stays as it was");
    }

    /// §9.5.1.6, §9.5.1.8.7, §9.5.1.8.8, §10.5.3.1: a revision keeps the
    /// `transformation_key_id` and the algorithm, increments the revision,
    /// has a new master salt and master sender key, and keeps the
    /// receiver-specific key. It is used after `activate_key_revision`; the
    /// receiver decodes both revisions.
    #[test]
    fn a_key_revision_keeps_the_key_id_and_changes_the_keys() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, true, SUB_ENC | PAYLOAD_ENC | SUB_ORIGIN);
        let l = link(&mut a, &mut b, &cfg, &cfg);
        let before = tokens_of(&mut a.p, l.w, l.w_remote_reader);
        let rev = a.p.revise_local_entity_keys(a.local).unwrap();
        assert_eq!(rev, 1);
        let after = {
            a.p.create_local_datawriter_crypto_tokens(l.w, l.w_remote_reader, rev)
                .unwrap()
                .unwrap()
                .iter()
                .map(|t| keymat_of(t).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(after.len(), 2);
        for (old, new) in before.iter().zip(after.iter()) {
            assert_eq!(new.transformation_key_id, old.transformation_key_id);
            assert_eq!(new.suite, old.suite, "the algorithm id");
            assert_eq!(new.key_revision, [0, 0, 1]);
            assert_eq!(old.key_revision, [0, 0, 0]);
            assert_ne!(new.master_salt, old.master_salt);
            assert_ne!(new.master_key, old.master_key);
        }
        assert_eq!(
            after[0].origin.receiver_key.as_ref().unwrap().id,
            before[0].origin.receiver_key.as_ref().unwrap().id,
            "the receiver-specific key stays"
        );
        assert_eq!(
            after[0].origin.receiver_key.as_ref().unwrap().master,
            before[0].origin.receiver_key.as_ref().unwrap().master
        );
        // Until it is activated the old revision encodes.
        let plain = data_submessage();
        let mut old_wire = Vec::new();
        let mut i = 0;
        a.p.encode_datawriter_submessage(&mut old_wire, &plain, l.w, &[l.w_remote_reader], &mut i)
            .unwrap();
        assert_eq!(
            &old_wire[4..7],
            &[0, 0, 0],
            "revision 0 in the CryptoHeader"
        );
        // The receiver gets the revised tokens and decodes revision 0 and 1.
        let new_tokens =
            a.p.create_local_datawriter_crypto_tokens(l.w, l.w_remote_reader, rev)
                .unwrap()
                .unwrap();
        b.p.set_remote_datawriter_crypto_tokens(l.r, l.r_remote_writer, &new_tokens)
            .unwrap();
        assert_eq!(
            b.p.decode_datawriter_submessage(&old_wire, l.r, l.r_remote_writer)
                .unwrap()
                .unwrap(),
            plain
        );
        assert!(a.p.activate_key_revision(a.local, rev).unwrap());
        let mut new_wire = Vec::new();
        let mut i = 0;
        a.p.encode_datawriter_submessage(&mut new_wire, &plain, l.w, &[l.w_remote_reader], &mut i)
            .unwrap();
        assert_eq!(
            &new_wire[4..7],
            &[0, 0, 1],
            "revision 1 in the CryptoHeader"
        );
        assert_eq!(
            b.p.decode_datawriter_submessage(&new_wire, l.r, l.r_remote_writer)
                .unwrap()
                .unwrap(),
            plain
        );
        // The payload key is revised too.
        let payload =
            a.p.encode_serialized_payload(b"payload", l.w)
                .unwrap()
                .unwrap();
        assert_eq!(&payload.encoded_buffer[..3], &[0, 0, 1]);
        assert_eq!(
            b.p.decode_serialized_payload(&payload.encoded_buffer, &[], l.r, l.r_remote_writer)
                .unwrap()
                .unwrap(),
            b"payload"
        );
        // A revision nobody made is refused for the tokens and for activation.
        assert!(
            a.p.create_local_datawriter_crypto_tokens(l.w, l.w_remote_reader, 7)
                .is_err()
        );
        assert!(a.p.activate_key_revision(a.local, 7).is_err());
        // A second revision, a third: the receiver keeps the revision it
        // replaces and drops the one before it.
        let rev2 = a.p.revise_local_entity_keys(a.local).unwrap();
        assert_eq!(rev2, 2);
        let t2 =
            a.p.create_local_datawriter_crypto_tokens(l.w, l.w_remote_reader, rev2)
                .unwrap()
                .unwrap();
        b.p.set_remote_datawriter_crypto_tokens(l.r, l.r_remote_writer, &t2)
            .unwrap();
        assert!(
            b.p.decode_datawriter_submessage(&new_wire, l.r, l.r_remote_writer)
                .is_ok(),
            "revision 1 is still known"
        );
        assert!(
            b.p.decode_datawriter_submessage(&old_wire, l.r, l.r_remote_writer)
                .is_err(),
            "revision 0 is dropped"
        );
    }

    /// §9.5.1.8.7 and Table 78: the key material of the participant is
    /// revised with the entities' and the revised participant token carries
    /// the new revision.
    #[test]
    fn the_participant_key_is_revised_with_the_entities() {
        let mut a = side(AXK_ENC | AXK_ORIGIN, &[]);
        let mut b = side(AXK_ENC | AXK_ORIGIN, &[]);
        send_participant_key(&mut a, &mut b);
        let plain = rtps_message([3; 12]);
        let rev = a.p.revise_local_entity_keys(a.local).unwrap();
        let t =
            a.p.create_local_participant_crypto_tokens(a.local, a.remote, rev)
                .unwrap()
                .unwrap();
        let mat = keymat_of(&t[0]).unwrap();
        assert_eq!(mat.key_revision, [0, 0, 1]);
        b.p.set_remote_participant_crypto_tokens(b.local, b.remote, &t)
            .unwrap();
        assert!(a.p.activate_key_revision(a.local, rev).unwrap());
        let mut encoded = Vec::new();
        let mut i = 0;
        a.p.encode_rtps_message(&mut encoded, &plain, a.local, &[a.remote], &mut i, false)
            .unwrap();
        // The CryptoHeader of the SRTPS_PREFIX names the revision.
        assert_eq!(&encoded[24..27], &[0, 0, 1]);
        assert_eq!(
            b.p.decode_rtps_message(&encoded, b.local, b.remote)
                .unwrap()
                .unwrap(),
            plain
        );
        // Activating the revision already in use is TRUE; revising an unknown
        // participant fails.
        assert!(a.p.activate_key_revision(a.local, rev).unwrap());
        assert!(
            a.p.revise_local_entity_keys(ParticipantCryptoHandle(u64::MAX - 7))
                .is_err()
        );
    }

    /// §9.5.1.8.7: the key material of the volatile endpoints is not revised.
    #[test]
    fn the_volatile_endpoints_are_not_revised() {
        let mut a = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC);
        let (vw, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(VOLATILE_WRITER_ENTITY))
                .unwrap();
        let (uw, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(user_entity(1)))
                .unwrap();
        a.p.revise_local_entity_keys(a.local).unwrap();
        let spec = a.p.spec.read().unwrap();
        assert!(spec.pending.keys().any(|(_, e, _)| *e == uw.0));
        assert!(!spec.pending.keys().any(|(_, e, _)| *e == vw.0));
        assert!(spec.pending.keys().any(|(_, e, _)| *e == a.local.0));
    }

    /// §9.5.1.8.9 to §9.5.1.8.11: the unregister operations release the key
    /// material; an unknown handle is a failure.
    #[test]
    fn unregistering_releases_the_key_material_and_the_matches() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, true, SUB_ENC | PAYLOAD_ENC);
        let l = link(&mut a, &mut b, &cfg, &cfg);
        let slots_before = a.p.slots.read().unwrap().len();
        let mut wire = Vec::new();
        let mut i = 0;
        a.p.encode_datawriter_submessage(
            &mut wire,
            &data_submessage(),
            l.w,
            &[l.w_remote_reader],
            &mut i,
        )
        .unwrap();
        // Remote reader of the writer.
        assert!(a.p.unregister_datareader(l.w_remote_reader).unwrap());
        assert!(
            a.p.encode_datawriter_submessage(
                &mut Vec::new(),
                &data_submessage(),
                l.w,
                &[l.w_remote_reader],
                &mut 0
            )
            .is_err()
        );
        // The receiver: the remote writer's keys go with its handle.
        assert!(
            b.p.decode_datawriter_submessage(&wire, l.r, l.r_remote_writer)
                .is_ok()
        );
        assert!(b.p.unregister_datawriter(l.r_remote_writer).unwrap());
        assert!(
            b.p.decode_datawriter_submessage(&wire, l.r, l.r_remote_writer)
                .is_err()
        );
        // The local writer: its slots are freed.
        assert!(a.p.unregister_datawriter(l.w).unwrap());
        assert!(a.p.slots.read().unwrap().len() < slots_before);
        assert!(a.p.encode_serialized_payload(b"x", l.w).is_err());
        assert!(a.p.unregister_datawriter(l.w).is_err(), "already gone");
        // The local reader.
        assert!(b.p.unregister_datareader(l.r).unwrap());
        assert!(b.p.unregister_datareader(l.r).is_err());
        // The participants: the remote one first, then the local one with
        // everything left under it.
        assert!(a.p.unregister_participant(a.remote).unwrap());
        assert!(
            a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .is_err()
        );
        let (w2, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(user_entity(9)))
                .unwrap();
        assert!(a.p.unregister_participant(a.local).unwrap());
        assert!(a.p.encode_serialized_payload(b"x", w2).is_err());
        assert!(a.p.slots.read().unwrap().is_empty(), "all slots are freed");
        assert!(a.p.unregister_participant(a.local).is_err());
    }

    /// §9.5.1.9.7: `return_crypto_tokens` takes back tokens this plugin
    /// issued, once.
    #[test]
    fn crypto_tokens_are_returned_once() {
        let mut a = side(AXK_ENC, &[]);
        let t =
            a.p.create_local_participant_crypto_tokens(a.local, a.remote, 0)
                .unwrap()
                .unwrap();
        assert_eq!(a.p.outstanding_crypto_tokens().unwrap(), 1);
        assert!(a.p.return_crypto_tokens(t.clone()).unwrap());
        assert_eq!(a.p.outstanding_crypto_tokens().unwrap(), 0);
        assert!(a.p.return_crypto_tokens(t).is_err(), "returned twice");
        // A token this plugin did not issue.
        let mut b = side(AXK_ENC, &[]);
        let foreign =
            b.p.create_local_participant_crypto_tokens(b.local, b.remote, 0)
                .unwrap()
                .unwrap();
        assert!(a.p.return_crypto_tokens(foreign).is_err());
        assert!(a.p.return_crypto_tokens(Vec::new()).unwrap());
    }

    /// §9.5.1.10.2, .3 and §10.5.2.1.2: the key material of the
    /// BuiltinParticipantVolatileMessageSecure endpoints is the Kx key of the
    /// SharedSecret, made by the registration of the remote endpoint; the
    /// list has one element; nothing is exchanged.
    #[test]
    fn the_volatile_endpoints_use_the_key_of_the_shared_secret() {
        let mut a = side(AXK_ENC, &[]);
        let mut b = side(AXK_ENC, &[]);
        let cfg = econfig(true, false, SUB_ENC);
        let (aw, _) =
            a.p.register_local_datawriter(a.local, &[], &cfg, guid(VOLATILE_WRITER_ENTITY))
                .unwrap();
        let (ar, _) =
            a.p.register_local_datareader(a.local, &[], &cfg, guid(VOLATILE_READER_ENTITY))
                .unwrap();
        let (bw, _) =
            b.p.register_local_datawriter(b.local, &[], &cfg, guid(VOLATILE_WRITER_ENTITY))
                .unwrap();
        let (br, _) =
            b.p.register_local_datareader(b.local, &[], &cfg, guid(VOLATILE_READER_ENTITY))
                .unwrap();
        let a_to_b_reader =
            a.p.register_matched_remote_datareader(aw, a.remote, SECRET, false)
                .unwrap();
        let a_from_b_writer =
            a.p.register_matched_remote_datawriter(ar, a.remote, SECRET)
                .unwrap();
        let b_from_a_writer =
            b.p.register_matched_remote_datawriter(br, b.remote, SECRET)
                .unwrap();
        let b_to_a_reader =
            b.p.register_matched_remote_datareader(bw, b.remote, SECRET, false)
                .unwrap();
        // Nothing is exchanged.
        assert!(
            a.p.create_local_datawriter_crypto_tokens(aw, a_to_b_reader, 0)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        assert!(
            b.p.set_remote_datawriter_crypto_tokens(br, b_from_a_writer, &[])
                .is_err()
        );
        // The writer's submessage, with the Kx key: key id 0, AES256-GCM.
        let plain = data_submessage();
        let mut wire = Vec::new();
        let mut i = 0;
        a.p.encode_datawriter_submessage(&mut wire, &plain, aw, &[a_to_b_reader], &mut i)
            .unwrap();
        assert_eq!(&wire[4..12], &[0, 0, 0, 4, 0, 0, 0, 0]);
        assert_eq!(
            b.p.decode_datawriter_submessage(&wire, br, b_from_a_writer)
                .unwrap()
                .unwrap(),
            plain
        );
        let pre =
            b.p.preprocess_secure_submsg(&wire, b.local, b.remote)
                .unwrap()
                .unwrap();
        assert_eq!(pre.datawriter_crypto, b_from_a_writer);
        assert_eq!(pre.datareader_crypto, br);
        // The reader's submessage.
        let ack = {
            let mut m = alloc::vec![0x06, 0x01, 0x08, 0x00];
            m.extend_from_slice(&[1, 1, 1, 1, 2, 2, 2, 2]);
            m
        };
        let wire =
            b.p.encode_datareader_submessage(&ack, br, &[b_from_a_writer])
                .unwrap()
                .unwrap();
        assert_eq!(
            a.p.decode_datareader_submessage(&wire, aw, a_to_b_reader)
                .unwrap()
                .unwrap(),
            ack
        );
        let _ = (a_from_b_writer, b_to_a_reader, ar, bw);
        // A list of two readers for the volatile writer is refused.
        assert!(
            a.p.encode_datawriter_submessage(
                &mut Vec::new(),
                &plain,
                aw,
                &[a_to_b_reader, a_to_b_reader],
                &mut 0
            )
            .is_err()
        );
    }

    /// Table 81: the InitializationVectorSuffix is an initially random value
    /// that changes with each operation under the same key.
    #[test]
    fn the_initialization_vector_suffix_starts_random_and_counts() {
        let suffix = |h: &[u8]| u64::from_be_bytes(h[12..20].try_into().unwrap());
        let mut starts = Vec::new();
        for _ in 0..4 {
            let mat = KeyMaterial::new_random(Suite::Aes256Gcm, &SystemRandom::new()).unwrap();
            let (h1, _, _) = mat.seal_cyclone(b"a").unwrap();
            let (h2, _, _) = mat.seal_cyclone(b"b").unwrap();
            assert_eq!(suffix(&h2), suffix(&h1).wrapping_add(1));
            starts.push(suffix(&h1));
        }
        starts.sort_unstable();
        starts.dedup();
        assert_eq!(starts.len(), 4, "four keys, four different starts");
        assert!(starts.iter().any(|s| *s > 1 << 32), "not a small counter");
    }
}
