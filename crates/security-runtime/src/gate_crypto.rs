// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The gate's use of the Cryptographic plugin: nothing but the operations of
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The gate holds the plugin as a trait object.)
//! DDS-Security 1.2 §9.5.1 Tables 42 to 44 (`CryptoKeyFactory`,
//! `CryptoKeyExchange`, `CryptoTransform`), with the handles they return.
//!
//! The gate speaks of peers (a GuidPrefix) and of endpoints it holds a
//! [`CryptoHandle`] for; the plugin speaks of `ParticipantCryptoHandle`,
//! `DatawriterCryptoHandle` and `DatareaderCryptoHandle` that belong together
//! in pairs. This layer is the table between the two:
//!
//! * the local participant, and the local endpoints of
//!   `BuiltinParticipantVolatileMessageSecure` (their key is derived from the
//!   SharedSecret, §10.5.2.1.2), are registered once;
//! * a peer is a matched remote participant and, for the volatile channel, a
//!   matched remote DataReader and DataWriter;
//! * a local user or discovery endpoint is a registered DataWriter or
//!   DataReader; a remote endpoint it sends tokens to is a matched remote
//!   DataReader or DataWriter registered for the first token;
//! * the keys a peer's endpoints send arrive without the gate knowing which
//!   local endpoint they are for, so they are installed for a local endpoint
//!   of the kind of the key (encrypting or signing, with or without origin
//!   authentication) that stands for all of them, and found by
//!   `preprocess_secure_submsg`.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::CryptoHandle;
use zerodds_security::cryptographic::{
    CryptographicPluginInterfaces, DatareaderCryptoHandle, DatawriterCryptoHandle,
    EndpointSecurityConfig, PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED,
    PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
    PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED,
    PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED, ParticipantCryptoHandle,
    ParticipantSecurityConfig, SecureSubmessageCategory,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind};
use zerodds_security::properties::Property;
use zerodds_security::token::{CryptoToken, DataHolder};
use zerodds_security_permissions::ProtectionKind;

use crate::gate::SecurityGateError;
use crate::shared::PeerKey;

/// `class_id` of the CryptoToken of the built-in plugin (Table 72).
const TOKEN_CLASS_ID: &str = "DDS:Crypto:AES_GCM_GMAC";
/// The binary property of the CryptoToken with the key material (Table 72).
const TOKEN_KEYMAT: &str = "dds.cryp.keymat";
/// `dds.sec.crypto.symmetric_cipher_algorithm` (Table 71).
const CIPHER_PROPERTY: &str = "dds.sec.crypto.symmetric_cipher_algorithm";
/// EntityId of `BuiltinParticipantVolatileMessageSecureWriter` / `...Reader`.
const VOLATILE_WRITER: [u8; 4] = [0xff, 0x02, 0x02, 0xc3];
const VOLATILE_READER: [u8; 4] = [0xff, 0x02, 0x02, 0xc4];

/// What the gate gives the plugin for the local participant.
#[derive(Debug, Clone)]
pub(crate) struct LocalParams {
    /// Authenticated encryption (GCM), else authentication only (GMAC).
    pub encrypt: bool,
    /// Origin authentication of the RTPS messages.
    pub origin: bool,
    /// Key width, 128 or 256.
    pub bits: u16,
    /// The `PermissionsHandle` of the local participant.
    pub permissions: PermissionsHandle,
    /// `ac_participant_properties` of the AccessControl plugin
    /// (DDS-Security 1.2 §9.4.2.4).
    pub properties: Vec<(String, String)>,
    /// The `ParticipantSecurityConfig` the AccessControl plugin gave
    /// (`get_participant_security_config`), passed to
    /// `register_local_participant` (DDS-Security 1.2 §9.8.1 step 11);
    /// `None` without a plugin.
    pub config: Option<zerodds_security::access_control::ParticipantSecurityConfig>,
}

/// (encrypting, with origin authentication): what a key is.
type Kind = (bool, bool);

struct Local {
    participant: ParticipantCryptoHandle,
    volatile_writer: DatawriterCryptoHandle,
    volatile_reader: DatareaderCryptoHandle,
}

struct Peer {
    remote: ParticipantCryptoHandle,
    secret: SharedSecretHandle,
    /// The remote DataReader of the local volatile writer and the remote
    /// DataWriter of the local volatile reader, with the Kx key between them;
    /// the error when the plugin could not make it (no SharedSecret known).
    kx: Result<(DatareaderCryptoHandle, DatawriterCryptoHandle), SecurityError>,
    /// The remote DataWriters whose keys this peer sent, by kind of key.
    writers: BTreeMap<Kind, DatawriterCryptoHandle>,
    /// The remote DataReaders whose keys this peer sent, by kind of key.
    readers: BTreeMap<Kind, DatareaderCryptoHandle>,
}

struct Endpoint {
    is_writer: bool,
    /// The remote endpoints it sent tokens to, by GUID: handles of matched
    /// remote DataReaders (of a writer) or DataWriters (of a reader).
    matched: BTreeMap<[u8; 16], u64>,
    /// `adjusted_algorithm_info.symmetric_cipher.required_mask` the plugin
    /// returned for the endpoint (§9.5.1.8 Table 42).
    required_mask: u32,
}

/// The plugin and what the gate keeps of its handles.
pub(crate) struct CryptoLayer {
    plugin: Box<dyn CryptographicPluginInterfaces>,
    local: Option<Local>,
    peers: BTreeMap<PeerKey, Peer>,
    endpoints: BTreeMap<u64, Endpoint>,
    /// Local DataReaders / DataWriters that stand for the keys of the remote
    /// endpoints of one kind.
    catch_readers: BTreeMap<Kind, DatareaderCryptoHandle>,
    catch_writers: BTreeMap<Kind, DatawriterCryptoHandle>,
    /// A remote participant for the tokens that are made for no peer.
    anonymous: Option<ParticipantCryptoHandle>,
    /// The matched remote endpoints of the tokens made for no peer.
    anonymous_matched: BTreeMap<u64, u64>,
    /// Matched remote readers made for a relay-only reader, by writer.
    relay_matched: BTreeMap<u64, u64>,
    /// The key revision the tokens are made for: the revision
    /// `revise_local_entity_keys` returned last (0 before any), which
    /// `activate_key_revision` then makes the active one.
    revision: i32,
}

fn setup(e: SecurityError) -> SecurityGateError {
    SecurityGateError::CryptoSetup(e)
}

fn crypto(e: SecurityError) -> SecurityGateError {
    SecurityGateError::Crypto(e)
}

fn refused(detail: &'static str) -> SecurityGateError {
    SecurityGateError::Crypto(SecurityError::new(SecurityErrorKind::CryptoFailed, detail))
}

/// (protected, encrypted, origin-authenticated) of a Protection Kind.
fn flags(kind: ProtectionKind) -> (bool, bool, bool) {
    match kind {
        ProtectionKind::None => (false, false, false),
        ProtectionKind::Sign => (true, false, false),
        ProtectionKind::Encrypt => (true, true, false),
        ProtectionKind::SignWithOriginAuthentication => (true, false, true),
        ProtectionKind::EncryptWithOriginAuthentication => (true, true, true),
    }
}

fn token_of(keymat: Vec<u8>) -> CryptoToken {
    DataHolder::new(TOKEN_CLASS_ID).with_binary_property(TOKEN_KEYMAT, keymat)
}

fn keymat_of(token: &CryptoToken) -> Option<Vec<u8>> {
    token.binary_property(TOKEN_KEYMAT).map(<[u8]>::to_vec)
}

/// The kind of the key a `KeyMaterial_AES_GCM_GMAC` holds (§10.5.2.1.1):
/// GCM (algorithm id 2 or 4) or GMAC, and whether it has a receiver-specific
/// key.
fn keymat_kind(bytes: &[u8]) -> Option<Kind> {
    let u32_at = |o: usize| -> Option<usize> {
        let b = bytes.get(o..o + 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let encrypt = matches!(*bytes.get(3)?, 2 | 4);
    let mut o = 4;
    o += 4 + u32_at(o)?; // master_salt
    o += 4; // sender_key_id
    o += 4 + u32_at(o)?; // master_sender_key
    let origin = bytes.get(o..o + 4).is_some_and(|id| id != [0; 4]);
    Some((encrypt, origin))
}

/// The properties the middleware passes to the registration of the
/// BuiltinParticipantVolatileMessageSecureWriter (`writer`) or -Reader:
/// "a property with name 'dds.sec.builtin_endpoint_name' and value
/// 'BuiltinParticipantVolatileMessageSecureWriter'" or `...Reader`
/// (DDS-Security 1.2 §9.8.9.1).
/// The algorithm info of the AccessControl plugin as the Cryptographic
/// plugin's type (same fields, §7.3.10 to §7.3.14).
fn algorithm_info(
    a: &zerodds_security::access_control::ParticipantSecurityAlgorithmInfo,
) -> zerodds_security::cryptographic::ParticipantSecurityAlgorithmInfo {
    use zerodds_security::cryptographic as c;
    let req = |r: zerodds_security::access_control::CryptoAlgorithmRequirements| {
        c::CryptoAlgorithmRequirements {
            supported_mask: r.supported_mask,
            required_mask: r.required_mask,
        }
    };
    c::ParticipantSecurityAlgorithmInfo {
        digital_signature: c::ParticipantSecurityDigitalSignatureAlgorithmInfo {
            trust_chain: req(a.digital_signature.trust_chain),
            message_auth: req(a.digital_signature.message_auth),
        },
        key_establishment: c::ParticipantSecurityKeyEstablishmentAlgorithmInfo {
            shared_secret: req(a.key_establishment.shared_secret),
        },
        symmetric_cipher: c::ParticipantSecuritySymmetricCipherAlgorithmInfo {
            supported_mask: a.symmetric_cipher.supported_mask,
            builtin_endpoints_required_mask: a.symmetric_cipher.builtin_endpoints_required_mask,
            builtin_kx_endpoints_required_mask: a
                .symmetric_cipher
                .builtin_kx_endpoints_required_mask,
            user_endpoints_default_required_mask: a
                .symmetric_cipher
                .user_endpoints_default_required_mask,
        },
    }
}

pub(crate) fn volatile_properties(writer: bool) -> Vec<Property> {
    vec![Property::local(
        crate::shared::PROP_BUILTIN_ENDPOINT_NAME,
        if writer {
            crate::shared::BUILTIN_VOLATILE_MESSAGE_WRITER
        } else {
            crate::shared::BUILTIN_VOLATILE_MESSAGE_READER
        },
    )]
}

/// The properties passed to `register_local_datawriter` and
/// `register_local_datareader`: the properties of the AccessControl plugin
/// (for a builtin volatile message endpoint among them
/// `dds.sec.builtin_endpoint_name`, DDS-Security 1.2 §9.8.9.1) and the cipher
/// of the endpoint.
pub(crate) fn registration_properties(
    ac_properties: &[(&str, &str)],
    cipher: Option<&str>,
) -> Vec<Property> {
    let mut properties: Vec<Property> = ac_properties
        .iter()
        .map(|(name, value)| Property::local(name.to_string(), value.to_string()))
        .collect();
    properties.extend(cipher.map(|c| Property::local(CIPHER_PROPERTY, c.to_string())));
    properties
}

impl CryptoLayer {
    pub(crate) fn new(plugin: Box<dyn CryptographicPluginInterfaces>) -> Self {
        Self {
            plugin,
            local: None,
            peers: BTreeMap::new(),
            endpoints: BTreeMap::new(),
            catch_readers: BTreeMap::new(),
            catch_writers: BTreeMap::new(),
            anonymous: None,
            anonymous_matched: BTreeMap::new(),
            relay_matched: BTreeMap::new(),
            revision: 0,
        }
    }

    fn local(&self) -> Result<&Local, SecurityGateError> {
        self.local.as_ref().ok_or_else(|| {
            SecurityGateError::PolicyViolation("no local participant is registered".into())
        })
    }

    fn peer(&self, key: &PeerKey) -> Result<&Peer, SecurityGateError> {
        self.peers.get(key).ok_or_else(|| {
            SecurityGateError::PolicyViolation(alloc::format!("peer {key:?} not registered"))
        })
    }

    /// `register_local_participant`, and the local endpoints of the volatile
    /// channel.
    pub(crate) fn ensure_local(
        &mut self,
        params: LocalParams,
    ) -> Result<CryptoHandle, SecurityGateError> {
        if let Some(local) = &self.local {
            return Ok(CryptoHandle(local.participant.0));
        }
        let mut attrs = 0;
        if params.encrypt {
            attrs |= PLUGIN_PARTICIPANT_FLAG_IS_RTPS_AXK_ENCRYPTED;
        }
        if params.origin {
            attrs |= PLUGIN_PARTICIPANT_FLAG_IS_RTPS_ORIGIN_AUTHENTICATED;
        }
        let config = params.config.as_ref().map_or_else(
            || ParticipantSecurityConfig {
                is_rtps_axk_protected: true,
                plugin_participant_attributes: attrs,
                ..Default::default()
            },
            |c| ParticipantSecurityConfig {
                allow_unauthenticated_participants: c.allow_unauthenticated_participants,
                is_access_protected: c.is_access_protected,
                is_rtps_axk_protected: c.is_rtps_axk_protected,
                is_rtps_psk_protected: c.is_rtps_psk_protected,
                is_discovery_protected: c.is_discovery_protected,
                is_liveliness_protected: c.is_liveliness_protected,
                is_key_revision_enabled: c.is_key_revision_enabled,
                plugin_participant_attributes: c.plugin_participant_attributes,
                ac_endpoint_properties: Vec::new(),
                algorithm_info: algorithm_info(&c.algorithm_info),
            },
        );
        let cipher = Property::local(
            CIPHER_PROPERTY,
            if params.bits == 128 {
                "AES128-GCM"
            } else {
                "AES256-GCM"
            },
        );
        let mut properties = vec![cipher];
        properties.extend(
            params
                .properties
                .iter()
                .map(|(name, value)| Property::local(name.clone(), value.clone())),
        );
        let (participant, _) = self
            .plugin
            .register_local_participant(IdentityHandle(1), params.permissions, &properties, &config)
            .map_err(setup)?;
        let volatile = EndpointSecurityConfig {
            is_submessage_protected: true,
            plugin_endpoint_attributes: PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED,
            ..Default::default()
        };
        let guid = |entity: [u8; 4]| {
            let mut g = [0u8; 16];
            g[12..].copy_from_slice(&entity);
            g
        };
        let (volatile_writer, _) = self
            .plugin
            .register_local_datawriter(
                participant,
                &volatile_properties(true),
                &volatile,
                guid(VOLATILE_WRITER),
            )
            .map_err(setup)?;
        let (volatile_reader, _) = self
            .plugin
            .register_local_datareader(
                participant,
                &volatile_properties(false),
                &volatile,
                guid(VOLATILE_READER),
            )
            .map_err(setup)?;
        self.local = Some(Local {
            participant,
            volatile_writer,
            volatile_reader,
        });
        Ok(CryptoHandle(participant.0))
    }

    /// `register_matched_remote_participant` for a peer, with the Kx key of the
    /// volatile channel.
    pub(crate) fn register_peer(
        &mut self,
        key: PeerKey,
        identity: IdentityHandle,
        secret: SharedSecretHandle,
    ) -> Result<CryptoHandle, SecurityGateError> {
        let (participant, vw, vr) = {
            let local = self.local()?;
            (
                local.participant,
                local.volatile_writer,
                local.volatile_reader,
            )
        };
        // HandleNIL is a failure; an identity the caller left out is not.
        let identity = if identity.0 == 0 {
            IdentityHandle(u64::MAX)
        } else {
            identity
        };
        let remote = self
            .plugin
            .register_matched_remote_participant(
                participant,
                identity,
                PermissionsHandle(1),
                secret,
            )
            .map_err(setup)?;
        let kx = self
            .plugin
            .register_matched_remote_datareader(vw, remote, secret, false)
            .and_then(|reader| {
                self.plugin
                    .register_matched_remote_datawriter(vr, remote, secret)
                    .map(|writer| (reader, writer))
            });
        self.peers.insert(
            key,
            Peer {
                remote,
                secret,
                kx,
                writers: BTreeMap::new(),
                readers: BTreeMap::new(),
            },
        );
        Ok(CryptoHandle(remote.0))
    }

    pub(crate) fn peer_handle(&self, key: &PeerKey) -> Option<CryptoHandle> {
        self.peers.get(key).map(|p| CryptoHandle(p.remote.0))
    }

    pub(crate) fn peer_keys(&self) -> Vec<PeerKey> {
        self.peers.keys().copied().collect()
    }

    fn token_bytes(tokens: Option<Vec<CryptoToken>>) -> Result<Vec<Vec<u8>>, SecurityGateError> {
        let tokens = tokens.ok_or_else(|| refused("the plugin made no CryptoToken (FALSE)"))?;
        tokens
            .iter()
            .map(|t| keymat_of(t).ok_or_else(|| refused("a CryptoToken without dds.cryp.keymat")))
            .collect()
    }

    fn anonymous(&mut self) -> Result<ParticipantCryptoHandle, SecurityGateError> {
        if let Some(a) = self.anonymous {
            return Ok(a);
        }
        let local = self.local()?.participant;
        let a = self
            .plugin
            .register_matched_remote_participant(
                local,
                IdentityHandle(u64::MAX),
                PermissionsHandle(1),
                SharedSecretHandle(0),
            )
            .map_err(setup)?;
        self.anonymous = Some(a);
        Ok(a)
    }

    /// `create_local_participant_crypto_tokens` for the peer, or for no peer.
    pub(crate) fn participant_token(
        &mut self,
        peer: Option<&PeerKey>,
    ) -> Result<Vec<u8>, SecurityGateError> {
        let remote = match peer {
            Some(key) => self.peer(key)?.remote,
            None => self.anonymous()?,
        };
        let local = self.local()?.participant;
        let tokens = self
            .plugin
            .create_local_participant_crypto_tokens(local, remote, self.revision)
            .map_err(crypto)?;
        Self::token_bytes(tokens)?
            .into_iter()
            .next()
            .ok_or_else(|| refused("no participant token"))
    }

    /// `set_remote_participant_crypto_tokens`.
    pub(crate) fn set_participant_token(
        &mut self,
        peer: &PeerKey,
        keymat: &[u8],
    ) -> Result<(), SecurityGateError> {
        let remote = self.peer(peer)?.remote;
        let local = self.local()?.participant;
        let ok = self
            .plugin
            .set_remote_participant_crypto_tokens(local, remote, &[token_of(keymat.to_vec())])
            .map_err(crypto)?;
        ok.then_some(())
            .ok_or_else(|| refused("the plugin did not take the participant token (FALSE)"))
    }

    /// `register_local_datawriter` / `register_local_datareader` for the
    /// Metadata and the Data Protection Kind of the endpoint.
    pub(crate) fn register_endpoint(
        &mut self,
        is_writer: bool,
        metadata: ProtectionKind,
        data: ProtectionKind,
        cipher: Option<&str>,
        ac_properties: &[(&str, &str)],
    ) -> Result<CryptoHandle, SecurityGateError> {
        let participant = self.local()?.participant;
        let (mut sub, mut sub_enc, origin) = flags(metadata);
        let (payload, payload_enc, _) = flags(data);
        if !sub && !payload {
            // Neither protected: the endpoint has no use for a key, and the
            // plugin makes none for it. The builtin endpoints are registered
            // all the same; they get a key that is not used.
            sub = true;
            sub_enc = true;
        }
        let mut attrs = 0;
        if sub_enc {
            attrs |= PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED;
        }
        if payload_enc {
            attrs |= PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED;
        }
        if origin {
            attrs |= PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED;
        }
        let config = EndpointSecurityConfig {
            is_submessage_protected: sub,
            is_payload_protected: payload,
            plugin_endpoint_attributes: attrs,
            ..Default::default()
        };
        // `ac_endpoint_properties` of the AccessControl plugin
        // (DDS-Security 1.2 §9.4.2.7 Table 37) and the cipher of the endpoint.
        let properties = registration_properties(ac_properties, cipher);
        let (handle, info) = if is_writer {
            let (h, info) = self
                .plugin
                .register_local_datawriter(participant, &properties, &config, [0; 16])
                .map_err(setup)?;
            (h.0, info)
        } else {
            let (h, info) = self
                .plugin
                .register_local_datareader(participant, &properties, &config, [0; 16])
                .map_err(setup)?;
            (h.0, info)
        };
        self.endpoints.insert(
            handle,
            Endpoint {
                is_writer,
                matched: BTreeMap::new(),
                required_mask: info.symmetric_cipher.required_mask,
            },
        );
        Ok(CryptoHandle(handle))
    }

    /// The tokens of a local endpoint for one remote endpoint: the handle of
    /// the matched remote endpoint is made on the first call.
    fn endpoint_tokens(
        &mut self,
        endpoint: CryptoHandle,
        remote: Option<[u8; 16]>,
        relay_only: bool,
    ) -> Result<Vec<Vec<u8>>, SecurityGateError> {
        let is_writer = self
            .endpoints
            .get(&endpoint.0)
            .ok_or_else(|| refused("unknown endpoint handle"))?
            .is_writer;
        // The remote participant and the SharedSecret of the remote endpoint.
        let (participant, secret) = match remote {
            Some(guid) => {
                let mut prefix = [0u8; 12];
                prefix.copy_from_slice(&guid[..12]);
                match self.peers.get(&prefix) {
                    Some(peer) => (peer.remote, peer.secret),
                    None => (self.anonymous()?, SharedSecretHandle(0)),
                }
            }
            None => (self.anonymous()?, SharedSecretHandle(0)),
        };
        let existing = match (remote, relay_only) {
            (Some(guid), false) => self
                .endpoints
                .get(&endpoint.0)
                .and_then(|e| e.matched.get(&guid).copied()),
            (None, false) => self.anonymous_matched.get(&endpoint.0).copied(),
            (_, true) => self.relay_matched.get(&endpoint.0).copied(),
        };
        let remote_handle = match existing {
            Some(h) => h,
            None => {
                let h = if is_writer {
                    self.plugin
                        .register_matched_remote_datareader(
                            DatawriterCryptoHandle(endpoint.0),
                            participant,
                            secret,
                            relay_only,
                        )
                        .map_err(setup)?
                        .0
                } else {
                    self.plugin
                        .register_matched_remote_datawriter(
                            DatareaderCryptoHandle(endpoint.0),
                            participant,
                            secret,
                        )
                        .map_err(setup)?
                        .0
                };
                match (remote, relay_only) {
                    (Some(guid), false) => {
                        if let Some(e) = self.endpoints.get_mut(&endpoint.0) {
                            e.matched.insert(guid, h);
                        }
                    }
                    (None, false) => {
                        self.anonymous_matched.insert(endpoint.0, h);
                    }
                    (_, true) => {
                        self.relay_matched.insert(endpoint.0, h);
                    }
                }
                h
            }
        };
        let tokens = if is_writer {
            self.plugin
                .create_local_datawriter_crypto_tokens(
                    DatawriterCryptoHandle(endpoint.0),
                    DatareaderCryptoHandle(remote_handle),
                    self.revision,
                )
                .map_err(crypto)?
        } else {
            self.plugin
                .create_local_datareader_crypto_tokens(
                    DatareaderCryptoHandle(endpoint.0),
                    DatawriterCryptoHandle(remote_handle),
                    self.revision,
                )
                .map_err(crypto)?
        };
        Self::token_bytes(tokens)
    }

    /// The first token of the endpoint for the remote endpoint `remote`.
    pub(crate) fn endpoint_token(
        &mut self,
        endpoint: CryptoHandle,
        remote: Option<[u8; 16]>,
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.endpoint_tokens(endpoint, remote, false)?
            .into_iter()
            .next()
            .ok_or_else(|| refused("the endpoint has no token"))
    }

    /// The second (payload) token of a DataWriter that has two keys.
    pub(crate) fn payload_token(&mut self, endpoint: CryptoHandle) -> Option<Vec<u8>> {
        self.endpoint_tokens(endpoint, None, false)
            .ok()?
            .into_iter()
            .nth(1)
    }

    /// The token for a relay-only reader (§9.5.1.9.5).
    pub(crate) fn relay_token(&mut self, endpoint: CryptoHandle) -> Option<Vec<u8>> {
        self.endpoint_tokens(endpoint, None, true)
            .ok()?
            .into_iter()
            .next()
    }

    /// `set_remote_datawriter_crypto_tokens` / `set_remote_datareader_crypto_tokens`
    /// for a key an endpoint of the peer sent.
    pub(crate) fn set_endpoint_token(
        &mut self,
        peer: &PeerKey,
        keymat: &[u8],
        from_reader: bool,
    ) -> Result<(), SecurityGateError> {
        let kind = keymat_kind(keymat).ok_or_else(|| refused("a malformed key material"))?;
        let participant = self.local()?.participant;
        let (remote, secret) = {
            let p = self.peer(peer)?;
            (p.remote, p.secret)
        };
        let token = [token_of(keymat.to_vec())];
        let mut attrs = 0;
        if kind.0 {
            attrs |= PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ENCRYPTED
                | PLUGIN_ENDPOINT_FLAG_IS_PAYLOAD_ENCRYPTED;
        }
        if kind.1 {
            attrs |= PLUGIN_ENDPOINT_FLAG_IS_SUBMESSAGE_ORIGIN_AUTHENTICATED;
        }
        let config = EndpointSecurityConfig {
            is_submessage_protected: true,
            is_payload_protected: !from_reader,
            plugin_endpoint_attributes: attrs,
            ..Default::default()
        };
        let ok = if from_reader {
            let local_writer = match self.catch_writers.get(&kind) {
                Some(w) => *w,
                None => {
                    let (w, _) = self
                        .plugin
                        .register_local_datawriter(participant, &[], &config, [0; 16])
                        .map_err(setup)?;
                    self.catch_writers.insert(kind, w);
                    w
                }
            };
            let existing = self.peer(peer)?.readers.get(&kind).copied();
            let remote_reader = match existing {
                Some(r) => r,
                None => {
                    let r = self
                        .plugin
                        .register_matched_remote_datareader(local_writer, remote, secret, false)
                        .map_err(setup)?;
                    if let Some(p) = self.peers.get_mut(peer) {
                        p.readers.insert(kind, r);
                    }
                    r
                }
            };
            self.plugin
                .set_remote_datareader_crypto_tokens(local_writer, remote_reader, &token)
                .map_err(crypto)?
        } else {
            let local_reader = match self.catch_readers.get(&kind) {
                Some(r) => *r,
                None => {
                    let (r, _) = self
                        .plugin
                        .register_local_datareader(participant, &[], &config, [0; 16])
                        .map_err(setup)?;
                    self.catch_readers.insert(kind, r);
                    r
                }
            };
            let existing = self.peer(peer)?.writers.get(&kind).copied();
            let remote_writer = match existing {
                Some(w) => w,
                None => {
                    let w = self
                        .plugin
                        .register_matched_remote_datawriter(local_reader, remote, secret)
                        .map_err(setup)?;
                    if let Some(p) = self.peers.get_mut(peer) {
                        p.writers.insert(kind, w);
                    }
                    w
                }
            };
            self.plugin
                .set_remote_datawriter_crypto_tokens(local_reader, remote_writer, &token)
                .map_err(crypto)?
        };
        ok.then_some(())
            .ok_or_else(|| refused("the plugin did not take the endpoint token (FALSE)"))
    }

    /// `encode_datawriter_submessage` (a DataWriter's handle) or
    /// `encode_datareader_submessage` (a DataReader's).
    pub(crate) fn encode_endpoint_submessage(
        &mut self,
        endpoint: CryptoHandle,
        submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let ep = self
            .endpoints
            .get(&endpoint.0)
            .ok_or_else(|| refused("unknown endpoint handle"))?;
        if ep.is_writer {
            let list: Vec<DatareaderCryptoHandle> = ep
                .matched
                .values()
                .map(|h| DatareaderCryptoHandle(*h))
                .collect();
            let mut out = Vec::new();
            let mut index = 0;
            {
                let plain: &[u8] = submessage;
                let ok = self
                    .plugin
                    .encode_datawriter_submessage(
                        &mut out,
                        plain,
                        DatawriterCryptoHandle(endpoint.0),
                        &list,
                        &mut index,
                    )
                    .map_err(crypto)?;
                if !ok {
                    return Err(refused("encode_datawriter_submessage returned FALSE"));
                }
                // One encoded message has to carry the MACs of every receiver;
                // the later calls would replace those of the first.
                if index < list.len() {
                    return Err(SecurityGateError::PolicyViolation(
                        "more receivers with origin authentication than one message carries".into(),
                    ));
                }
            }
            Ok(out)
        } else {
            let list: Vec<DatawriterCryptoHandle> = ep
                .matched
                .values()
                .map(|h| DatawriterCryptoHandle(*h))
                .collect();
            self.plugin
                .encode_datareader_submessage(submessage, DatareaderCryptoHandle(endpoint.0), &list)
                .map_err(crypto)?
                .ok_or_else(|| refused("encode_datareader_submessage returned FALSE"))
        }
    }

    /// `encode_serialized_payload`.
    pub(crate) fn encode_payload(
        &self,
        endpoint: CryptoHandle,
        payload: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        self.plugin
            .encode_serialized_payload(payload, DatawriterCryptoHandle(endpoint.0))
            .map_err(crypto)?
            .map(|e| e.encoded_buffer)
            .ok_or_else(|| refused("the payload of the endpoint is not protected"))
    }

    /// Protects a submessage with the key of the local participant (the
    /// vendor form for an endpoint that has no key of its own).
    pub(crate) fn encode_with_participant_key(
        &self,
        submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.local()?.participant;
        self.plugin
            .vendor_forms()
            .ok_or_else(|| refused("the plugin has no vendor forms"))?
            .encode_submessage_with_participant_key(local, submessage)
            .map_err(crypto)
    }

    /// `encode_rtps_message`, for the receivers of `peers` (all peers with
    /// `None`).
    pub(crate) fn encode_message(
        &self,
        message: &[u8],
        peer: Option<&PeerKey>,
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.local()?.participant;
        let list: Vec<ParticipantCryptoHandle> = match peer.and_then(|k| self.peers.get(k)) {
            Some(p) => vec![p.remote],
            None => self.peers.values().map(|p| p.remote).collect(),
        };
        let mut out = Vec::new();
        let mut index = 0;
        {
            let plain: &[u8] = message;
            let ok = self
                .plugin
                .encode_rtps_message(&mut out, plain, local, &list, &mut index, false)
                .map_err(crypto)?;
            if !ok {
                return Err(refused("encode_rtps_message did not transform the message"));
            }
            if index < list.len() {
                return Err(SecurityGateError::PolicyViolation(
                    "more receivers with origin authentication than one message carries".into(),
                ));
            }
        }
        Ok(out)
    }

    /// `decode_rtps_message`.
    pub(crate) fn decode_message(
        &self,
        remote: CryptoHandle,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.local()?.participant;
        self.plugin
            .decode_rtps_message(wire, local, ParticipantCryptoHandle(remote.0))
            .map_err(crypto)?
            .ok_or_else(|| refused("decode_rtps_message returned FALSE"))
    }

    /// `preprocess_secure_submsg`, then `decode_datawriter_submessage` or
    /// `decode_datareader_submessage` with the handles it returned.
    pub(crate) fn decode_submessage(
        &self,
        peer: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let remote = self.peer(peer)?.remote;
        let local = self.local()?.participant;
        let pre = self
            .plugin
            .preprocess_secure_submsg(wire, local, remote)
            .map_err(crypto)?
            .ok_or_else(|| refused("preprocess_secure_submsg returned FALSE"))?;
        let plain = match pre.secure_submessage_category {
            SecureSubmessageCategory::DatawriterSubmessage => self
                .plugin
                .decode_datawriter_submessage(wire, pre.datareader_crypto, pre.datawriter_crypto),
            SecureSubmessageCategory::DatareaderSubmessage => self
                .plugin
                .decode_datareader_submessage(wire, pre.datawriter_crypto, pre.datareader_crypto),
            SecureSubmessageCategory::InfoSubmessage => return Ok(wire.to_vec()),
        };
        plain
            .map_err(crypto)?
            .ok_or_else(|| refused("the decode operation returned FALSE"))
    }

    /// `decode_serialized_payload`, with the keys of the remote DataWriters of
    /// the peer.
    pub(crate) fn decode_payload(
        &self,
        peer: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let p = self.peer(peer)?;
        let mut last = refused("no key of the peer decodes the serialized payload");
        for (kind, remote_writer) in &p.writers {
            let Some(reader) = self.catch_readers.get(kind) else {
                continue;
            };
            match self
                .plugin
                .decode_serialized_payload(encoded, &[], *reader, *remote_writer)
            {
                Ok(Some(plain)) => return Ok(plain),
                Ok(None) => {}
                Err(e) => last = crypto(e),
            }
        }
        Err(last)
    }

    /// The Kx key of the volatile channel: protects a DATA submessage for the
    /// peer.
    pub(crate) fn encode_kx(
        &self,
        peer: &PeerKey,
        submessage: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let p = self.peer(peer)?;
        let (remote_reader, _) = p.kx.clone().map_err(crypto)?;
        let writer = self.local()?.volatile_writer;
        let mut out = Vec::new();
        let mut index = 0;
        let ok = self
            .plugin
            .encode_datawriter_submessage(
                &mut out,
                submessage,
                writer,
                &[remote_reader],
                &mut index,
            )
            .map_err(crypto)?;
        ok.then_some(out)
            .ok_or_else(|| refused("encode_datawriter_submessage returned FALSE"))
    }

    /// The Kx key: opens a submessage of the peer's volatile writer.
    pub(crate) fn decode_kx(
        &self,
        peer: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let p = self.peer(peer)?;
        let (_, remote_writer) = p.kx.clone().map_err(crypto)?;
        let reader = self.local()?.volatile_reader;
        self.plugin
            .decode_datawriter_submessage(wire, reader, remote_writer)
            .map_err(crypto)?
            .ok_or_else(|| refused("decode_datawriter_submessage returned FALSE"))
    }

    /// A vendor form: opens a submessage with the key of the participant.
    pub(crate) fn decode_with_participant_key(
        &self,
        peer: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let remote = self.peer(peer)?.remote;
        self.plugin
            .vendor_forms()
            .ok_or_else(|| refused("the plugin has no vendor forms"))?
            .decode_submessage_with_participant_key(remote, wire)
            .map_err(crypto)
    }

    /// A vendor form: opens a payload with the key of the participant.
    pub(crate) fn decode_payload_with_participant_key(
        &self,
        peer: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let remote = self.peer(peer)?.remote;
        self.plugin
            .vendor_forms()
            .ok_or_else(|| refused("the plugin has no vendor forms"))?
            .decode_serialized_payload_with_participant_key(remote, encoded)
            .map_err(crypto)
    }

    /// A vendor form: opens a payload with the Kx key of the peer.
    pub(crate) fn decode_payload_with_kx_key(
        &self,
        peer: &PeerKey,
        encoded: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let remote = self.peer(peer)?.remote;
        self.plugin
            .vendor_forms()
            .ok_or_else(|| refused("the plugin has no vendor forms"))?
            .decode_serialized_payload_with_kx_key(remote, encoded)
            .map_err(crypto)
    }

    /// `unregister_participant` of a remote peer that is gone.
    pub(crate) fn forget_peer(&mut self, peer: &PeerKey) -> Result<(), SecurityGateError> {
        if let Some(p) = self.peers.remove(peer) {
            // The matched remote endpoints of this peer go with it.
            for e in self.endpoints.values_mut() {
                e.matched.retain(|guid, _| guid[..12] != peer[..]);
            }
            self.plugin
                .unregister_participant(p.remote)
                .map_err(crypto)?;
        }
        Ok(())
    }

    /// `unregister_datawriter` / `unregister_datareader` of a deleted local
    /// endpoint.
    pub(crate) fn unregister_endpoint(
        &mut self,
        handle: CryptoHandle,
    ) -> Result<(), SecurityGateError> {
        if let Some(ep) = self.endpoints.remove(&handle.0) {
            self.anonymous_matched.remove(&handle.0);
            self.relay_matched.remove(&handle.0);
            if ep.is_writer {
                self.plugin
                    .unregister_datawriter(DatawriterCryptoHandle(handle.0))
                    .map_err(crypto)?;
            } else {
                self.plugin
                    .unregister_datareader(DatareaderCryptoHandle(handle.0))
                    .map_err(crypto)?;
            }
        }
        Ok(())
    }

    /// The `required_mask` of the `adjusted_algorithm_info` the plugin returned
    /// when the endpoint was registered.
    pub(crate) fn endpoint_required_mask(&self, endpoint: CryptoHandle) -> Option<u32> {
        self.endpoints.get(&endpoint.0).map(|e| e.required_mask)
    }

    /// `revise_local_entity_keys`.
    pub(crate) fn revise(&mut self) -> Result<i32, SecurityGateError> {
        let local = self.local()?.participant;
        let revision = self
            .plugin
            .revise_local_entity_keys(local)
            .map_err(crypto)?;
        // The tokens made from here on carry the revised key material.
        self.revision = revision;
        Ok(revision)
    }

    /// `activate_key_revision`.
    pub(crate) fn activate(&mut self, revision: i32) -> Result<bool, SecurityGateError> {
        let local = self.local()?.participant;
        self.plugin
            .activate_key_revision(local, revision)
            .map_err(crypto)
    }
}
