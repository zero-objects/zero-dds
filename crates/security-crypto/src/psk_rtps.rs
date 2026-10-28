// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DDS:Crypto:PSK` (DDS-Security 1.2 §10.9): protection of whole RTPS
//! messages with a pre-shared key.
//!
//! * [`Passphrase`] and [`parse_passphrase_list`] read the content of the
//!   properties `rtps_psk_secret_passphrase` and
//!   `rtps_psk_secret_passphrase_alt` (Table 86): `passphrase_id:passphrase`.
//! * [`derive_key_material`] derives the key material from a passphrase as
//!   §10.5.2.1.3, Tables 75 and 76, say (HMAC-SHA256 on the sender key id,
//!   protocol version, vendor id and GUID prefix of the RTPS Header).
//! * [`PskCryptoPlugin`] is the plugin of Tables 87 to 89: it makes the
//!   participant key material, protects a message in `encode_rtps_message`
//!   with the flags A and P, opens one in `decode_rtps_message` with the
//!   primary or an alternative passphrase, and does nothing in every other
//!   operation.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::{
    CRYPTO_ALGORITHM_SET_EMPTY, CryptoTransformKeyRevisionIntHolder,
};
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security::cryptographic::{
    DatareaderCryptoHandle, DatawriterCryptoHandle, EncodedPayload, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, GuidT, ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo,
    ParticipantSecurityConfig, ParticipantSecuritySymmetricCipherAlgorithmInfo,
    PreprocessedSubmessage,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::Property;
use zerodds_security::token::CryptoToken;

use crate::backend::{digest, hmac};
use crate::plugin::{PskRtpsKey, PskWire, parse_psk_wire};
use crate::suite::Suite;

/// Class of the plugin (DDS-Security 1.2 §10.9 names it `DDS:Crypto:PSK`;
/// the class string of the built-in plugin family carries the algorithm).
pub const CLASS_ID: &str = "DDS:Crypto:PSK:AES-GCM-GMAC:1.2";

/// `dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm` (Table 86).
pub const PROP_CIPHER: &str = "dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm";
/// `dds.sec.crypto.rtps_psk_secret_passphrase` (Table 86).
pub const PROP_PASSPHRASE: &str = "dds.sec.crypto.rtps_psk_secret_passphrase";
/// The name §10.5.2.1.3 Table 76 uses for the same property.
pub const PROP_PASSPHRASE_TABLE_76: &str = "dds.sec.crypto.psk.secret_passphrase";
/// `org.zerodds.sec.crypto.rtps_psk_secret_passphrase_alt` (Table 86).
pub const PROP_PASSPHRASE_ALT: &str = "org.zerodds.sec.crypto.rtps_psk_secret_passphrase_alt";
/// The name Table 71 of §10.5.1 uses for the same property.
pub const PROP_PASSPHRASE_EXTRA: &str = "org.zerodds.sec.crypto.rtps_psk_secret_passphrase_extra";

fn config_error(message: impl Into<String>) -> SecurityError {
    SecurityError::new(SecurityErrorKind::InvalidConfiguration, message.into())
}

/// A passphrase and its id (Table 86).
#[derive(Clone, PartialEq, Eq)]
pub struct Passphrase {
    /// `passphrase_id`: "a number between 0 and 2^32-1".
    pub id: u32,
    /// The passphrase: up to 512 printable ASCII characters.
    pub secret: String,
}

impl core::fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The secret stays out of logs.
        f.debug_struct("Passphrase")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Passphrase {
    /// Reads `passphrase_id:passphrase` (Table 86: "The passphrase shall
    /// follow the passphrase_id and be separated from it by the ':'
    /// character").
    ///
    /// # Errors
    /// `InvalidConfiguration` for an id that is no decimal number up to
    /// 2^32-1, an id with `id & 0xFF == 0xFF` ("reserved and shall not be
    /// used"), a passphrase of more than 512 characters or with a character
    /// outside 32 to 126, or one that starts or ends with a space; an empty
    /// passphrase.
    pub fn parse(text: &str) -> SecurityResult<Self> {
        let (id, secret) = text
            .split_once(':')
            .ok_or_else(|| config_error("psk: a passphrase is 'passphrase_id:passphrase'"))?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(config_error("psk: the passphrase_id is a decimal number"));
        }
        let id: u32 = id
            .parse()
            .map_err(|_| config_error("psk: the passphrase_id is at most 2^32-1"))?;
        if id & 0xFF == 0xFF {
            return Err(config_error(
                "psk: a passphrase_id with 0xFF as its low octet is reserved",
            ));
        }
        if secret.is_empty() || secret.len() > 512 {
            return Err(config_error("psk: the passphrase has 1 to 512 characters"));
        }
        if !secret.bytes().all(|b| (32..=126).contains(&b)) {
            return Err(config_error(
                "psk: the passphrase has printable ASCII characters only (32 to 126)",
            ));
        }
        if secret.starts_with(' ') || secret.ends_with(' ') {
            return Err(config_error(
                "psk: the passphrase does not start or end with a space",
            ));
        }
        Ok(Self {
            id,
            secret: secret.to_string(),
        })
    }

    /// `PassphraseKeyId`: the low octet of the id (Table 76).
    #[must_use]
    pub fn key_id(&self) -> u8 {
        (self.id & 0xFF) as u8
    }

    /// `PassphraseKeyRevision`: the three high octets of the id (Table 76).
    #[must_use]
    pub fn revision(&self) -> [u8; 3] {
        [
            (self.id >> 24) as u8,
            (self.id >> 16) as u8,
            (self.id >> 8) as u8,
        ]
    }
}

/// Reads a list of passphrases, "separated from the next using the LineFeed
/// (\n, character 10), the CarryReturn (\r, character 13), or both"
/// (Table 86, `rtps_psk_secret_passphrase_alt`). Empty lines are skipped.
///
/// # Errors
/// As [`Passphrase::parse`], for the first entry that does not parse.
pub fn parse_passphrase_list(text: &str) -> SecurityResult<Vec<Passphrase>> {
    text.split(['\n', '\r'])
        .filter(|line| !line.is_empty())
        .map(Passphrase::parse)
        .collect()
}

/// The key bits of `rtps_psk_symmetric_cipher_algorithm`: "AUTO" is
/// AES256-GCM; the four names of Table 22 select the width.
///
/// # Errors
/// `InvalidConfiguration` for any other value.
pub fn key_bits_from_cipher(value: Option<&str>) -> SecurityResult<u16> {
    crate::suite::symmetric_cipher_key_bits(PROP_CIPHER, value)
}

/// How the RTPS messages are protected (the RTPS protection kind of
/// §10.4.1.2.5.8, which the access control plugin reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PskProtection {
    /// "NONE ... will disable all protection": `CRYPTO_ALGORITHM_ID_NONE`.
    None,
    /// Authentication only (GMAC).
    Sign,
    /// Authenticated encryption (GCM).
    Encrypt,
}

/// The configuration of the plugin.
#[derive(Debug, Clone)]
pub struct PskConfig {
    /// DomainId of the participant.
    pub domain_id: u32,
    /// DomainTag of the participant; empty is allowed.
    pub domain_tag: String,
    /// `rtps_psk_secret_passphrase`: protects what this participant sends.
    pub passphrase: Passphrase,
    /// `rtps_psk_secret_passphrase_alt`: also accepted in decoding.
    pub alternatives: Vec<Passphrase>,
    /// 128 or 256.
    pub key_bits: u16,
    /// The RTPS protection kind.
    pub protection: PskProtection,
}

/// Reads the text a property of Table 71 or Table 86 names by URI: `data:,`
/// followed by the text (the schema delimiter is `:,`), or `file:` followed
/// by a path ("Supported URI schemas are: 'file' and 'data'").
///
/// # Errors
/// `InvalidConfiguration` for another scheme, a `data:` without the comma,
/// or a file that cannot be read.
pub fn read_uri(property: &str, value: &str) -> SecurityResult<String> {
    let value = value.trim_start_matches([' ', '\t']);
    if let Some(rest) = value.strip_prefix("data:") {
        let text = rest.strip_prefix(',').ok_or_else(|| {
            config_error(alloc::format!(
                "{property}: a data: URI is 'data:,' and the text"
            ))
        })?;
        return Ok(text.to_string());
    }
    if let Some(rest) = value.strip_prefix("file:") {
        // file:///path and file://localhost/path name the path after the
        // authority.
        let path = match rest.strip_prefix("//") {
            Some(authority_and_path) => {
                let at = authority_and_path.find('/').ok_or_else(|| {
                    config_error(alloc::format!("{property}: a file: URI without a path"))
                })?;
                let (authority, path) = authority_and_path.split_at(at);
                if !authority.is_empty() && authority != "localhost" {
                    return Err(config_error(alloc::format!(
                        "{property}: a file: URI names this host or none"
                    )));
                }
                path
            }
            None => rest,
        };
        return std::fs::read_to_string(path)
            .map_err(|e| config_error(alloc::format!("{property}: {path}: {e}")));
    }
    Err(config_error(alloc::format!(
        "{property}: the supported URI schemas are file and data"
    )))
}

impl PskConfig {
    /// The configuration of the pre-shared key out of the properties of the
    /// participant (§10.5.1 Table 71, §10.9.1 Table 86): the passphrase
    /// (`dds.sec.crypto.rtps_psk_secret_passphrase`, or the spelling of
    /// Table 76 `dds.sec.crypto.psk.secret_passphrase`), the alternatives
    /// (`..._alt`, Table 71 spells it `..._extra`) and the algorithm
    /// (`rtps_psk_symmetric_cipher_algorithm`), each a URI.
    ///
    /// # Errors
    /// `InvalidConfiguration` without a passphrase, for a URI or a passphrase
    /// that Table 86 does not allow, or an algorithm name outside Table 22.
    pub fn from_uri_properties(
        properties: &[(&str, &str)],
        domain_id: u32,
        domain_tag: &str,
        protection: PskProtection,
    ) -> SecurityResult<Self> {
        let get = |name: &str| {
            properties
                .iter()
                .rev()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let (name, value) = [PROP_PASSPHRASE, PROP_PASSPHRASE_TABLE_76]
            .into_iter()
            .find_map(|name| get(name).map(|value| (name, value)))
            .ok_or_else(|| {
                config_error(alloc::format!(
                    "psk: {PROP_PASSPHRASE} is needed to enable the pre-shared key"
                ))
            })?;
        let text = read_uri(name, value)?;
        let passphrase = Passphrase::parse(text.trim_end_matches(['\r', '\n']))?;
        let alternatives = match [PROP_PASSPHRASE_ALT, PROP_PASSPHRASE_EXTRA]
            .into_iter()
            .find_map(|name| get(name).map(|value| (name, value)))
        {
            Some((name, value)) => parse_passphrase_list(&read_uri(name, value)?)?,
            None => Vec::new(),
        };
        Ok(Self {
            domain_id,
            domain_tag: domain_tag.to_string(),
            passphrase,
            alternatives,
            key_bits: key_bits_from_cipher(get(PROP_CIPHER))?,
            protection,
        })
    }
}

/// The key material of Table 75, derived by [`derive_key_material`].
#[derive(Clone, PartialEq, Eq)]
pub struct DerivedKeyMaterial {
    /// `sender_key_id`: three octets of the SHA-256 of the domain and the
    /// `PassphraseKeyId`.
    pub sender_key_id: [u8; 4],
    /// `transformation_key_revision`: `PassphraseKeyRevision`.
    pub revision: [u8; 3],
    /// `master_salt`: 32 octets, the first 16 for AES-128.
    pub master_salt: Vec<u8>,
    /// `master_sender_key`: 32 octets, the first 16 for AES-128.
    pub master_sender_key: Vec<u8>,
}

impl core::fmt::Debug for DerivedKeyMaterial {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DerivedKeyMaterial")
            .field("sender_key_id", &self.sender_key_id)
            .finish_non_exhaustive()
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

/// The three octets of the SenderKeyId that the domain fixes: the first
/// three octets of `Sha256("DomainId=" | DomainId | ";DomainTag=" |
/// DomainTag)` (Table 76).
#[must_use]
pub fn domain_key_prefix(domain_id: u32, domain_tag: &str) -> [u8; 3] {
    let text = alloc::format!("DomainId={domain_id};DomainTag={domain_tag}");
    let hash = digest::digest(&digest::SHA256, text.as_bytes());
    let h = hash.as_ref();
    [h[0], h[1], h[2]]
}

/// Derives the key material of a pre-shared key for the messages whose RTPS
/// Header carries `protocol_version`, `vendor_id` and `guid_prefix`
/// (DDS-Security 1.2 §10.5.2.1.3, Tables 75 and 76):
///
/// ```text
/// SenderKeyId       = Sha256("DomainId=" | DomainId | ";DomainTag=" | DomainTag)[0..3] | PassphraseKeyId
/// master_salt       = HMACsha256(HMACsha256("PSK-SALT" | SenderKeyId | "RTPS" | ProtocolVersion | VendorId | GuidPrefix, PassphraseSecret),
///                                "master salt derivation" | 0x01)
/// master_sender_key = HMACsha256(HMACsha256("PSK-SKEY" | SenderKeyId | "RTPS" | ProtocolVersion | VendorId | GuidPrefix, PassphraseSecret),
///                                "master sender key derivation" | 0x01)
/// ```
///
/// Both are cut to the first 16 octets for the AES-128 variants.
#[must_use]
pub fn derive_key_material(
    domain_id: u32,
    domain_tag: &str,
    passphrase: &Passphrase,
    key_bits: u16,
    protocol_version: [u8; 2],
    vendor_id: [u8; 2],
    guid_prefix: [u8; 12],
) -> DerivedKeyMaterial {
    let prefix = domain_key_prefix(domain_id, domain_tag);
    let sender_key_id = [prefix[0], prefix[1], prefix[2], passphrase.key_id()];
    let tail = |label: &[u8]| {
        let mut data = label.to_vec();
        data.extend_from_slice(&sender_key_id);
        data.extend_from_slice(b"RTPS");
        data.extend_from_slice(&protocol_version);
        data.extend_from_slice(&vendor_id);
        data.extend_from_slice(&guid_prefix);
        data
    };
    let derive = |label: &[u8], what: &[u8]| {
        // HMACsha256(key, data): the key is the tagged identifiers, the data the
        // passphrase (Table 76, and the expanded example).
        let inner = hmac_sha256(&tail(label), passphrase.secret.as_bytes());
        let mut text = what.to_vec();
        text.push(0x01);
        let mut out = hmac_sha256(&inner, &text);
        if key_bits == 128 {
            out.truncate(16);
        }
        out
    };
    DerivedKeyMaterial {
        sender_key_id,
        revision: passphrase.revision(),
        master_salt: derive(b"PSK-SALT", b"master salt derivation"),
        master_sender_key: derive(b"PSK-SKEY", b"master sender key derivation"),
    }
}

/// The handle the operations return that "do nothing and return dummy Handle" (Table 87).
const DUMMY_HANDLE: CryptoHandle = CryptoHandle(0);

/// `DDS:Crypto:PSK` (DDS-Security 1.2 §10.9).
///
/// The plugin keeps the key material of what this participant sends and
/// derives, per sender, the material to open what it receives
/// ("compute (or locate a previously computed) PSK Key Material").
pub struct PskCryptoPlugin {
    config: PskConfig,
    local: Option<LocalKey>,
    next_handle: u64,
    /// Material of senders, by (passphrase index, version, vendor, prefix).
    received: Mutex<BTreeMap<SenderKey, Arc<PskRtpsKey>>>,
    /// The material of what this participant sends, by the RTPS Header of
    /// the message (version, vendor, prefix): `encode_rtps_message` derives
    /// it from the header of the message it protects (Table 75).
    sent: Mutex<BTreeMap<SenderKeyOf, Arc<PskRtpsKey>>>,
    /// The participant `CryptoKeyFactory::register_local_participant` made
    /// and the `supported_mask` of the config it was given.
    spec_local: Option<(u64, u32)>,
}

/// Protocol version, vendor id, GUID prefix of the header of a sent message.
type SenderKeyOf = ([u8; 2], [u8; 2], [u8; 12]);

/// Passphrase index, protocol version, vendor id, GUID prefix of a sender.
type SenderKey = (usize, [u8; 2], [u8; 2], [u8; 12]);

struct LocalKey {
    handle: u64,
    key: PskRtpsKey,
}

impl PskCryptoPlugin {
    /// A plugin with `config`.
    ///
    /// # Errors
    /// `InvalidConfiguration` for `key_bits` other than 128 or 256.
    pub fn new(config: PskConfig) -> SecurityResult<Self> {
        if config.key_bits != 128 && config.key_bits != 256 {
            return Err(config_error("psk: key_bits is 128 or 256"));
        }
        Ok(Self {
            config,
            local: None,
            next_handle: 1,
            received: Mutex::new(BTreeMap::new()),
            sent: Mutex::new(BTreeMap::new()),
            spec_local: None,
        })
    }

    /// The suite of the protection kind, `None` for NONE.
    fn suite(&self) -> Option<Suite> {
        match (self.config.protection, self.config.key_bits) {
            (PskProtection::None, _) => None,
            (PskProtection::Encrypt, 128) => Some(Suite::Aes128Gcm),
            (PskProtection::Encrypt, _) => Some(Suite::Aes256Gcm),
            (PskProtection::Sign, 128) => Some(Suite::Aes128Gmac),
            (PskProtection::Sign, _) => Some(Suite::Aes256Gmac),
        }
    }

    /// `register_local_participant` (Table 87): makes the
    /// ParticipantKeyMaterial for the messages whose RTPS Header carries
    /// `protocol_version`, `vendor_id` and `guid_prefix`. The
    /// `transformation_algorithm_id` follows the RTPS protection kind (GCM
    /// for ENCRYPT, GMAC for SIGN), the width `rtps_psk_symmetric_cipher_algorithm`.
    ///
    /// # Errors
    /// `InvalidConfiguration` if the protection kind is NONE: there is no
    /// key material to make; `CryptoFailed` from the random generator.
    pub fn register_local_participant_for(
        &mut self,
        protocol_version: [u8; 2],
        vendor_id: [u8; 2],
        guid_prefix: [u8; 12],
    ) -> SecurityResult<u64> {
        let suite = self.suite().ok_or_else(|| {
            config_error("psk: the RTPS protection kind is NONE, so there is no key material")
        })?;
        let material = derive_key_material(
            self.config.domain_id,
            &self.config.domain_tag,
            &self.config.passphrase,
            self.config.key_bits,
            protocol_version,
            vendor_id,
            guid_prefix,
        );
        let key = PskRtpsKey::new(
            suite,
            material.sender_key_id,
            material.revision,
            material.master_salt,
            material.master_sender_key,
        )?;
        let handle = self.next_handle;
        self.next_handle += 1;
        self.local = Some(LocalKey { handle, key });
        Ok(handle)
    }

    /// `unregister_participant` (Table 87): releases the key material and
    /// what was derived for senders.
    pub fn unregister_participant(&mut self, handle: u64) {
        if self.local.as_ref().is_some_and(|l| l.handle == handle) {
            self.local = None;
        }
        if let Ok(mut received) = self.received.lock() {
            received.clear();
        }
    }

    /// `encode_rtps_message` (Table 89), with `transform_with_psk` TRUE and
    /// no receiver-specific MACs: the message of the participant `handle`,
    /// protected with its key.
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle or a message that cannot be
    /// protected (see `PskRtpsKey`); the key material must have been made
    /// for the protocol version, vendor and GUID prefix of this message.
    pub fn encode_rtps_message(&self, handle: u64, message: &[u8]) -> SecurityResult<Vec<u8>> {
        let local = self
            .local
            .as_ref()
            .filter(|l| l.handle == handle)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: unknown participant crypto handle",
                )
            })?;
        local.key.encode_rtps_message(message)
    }

    /// `encode_rtps_message` (Table 89) with the key material derived from
    /// the RTPS Header of the message itself: "Uses content of the RTPS
    /// Header, the pre-shared secret and SenderKeyId to compute (or locate a
    /// previously computed) PSK Key Material" (§10.9.3.3.1), so that no
    /// participant registration has to know the GUID prefix beforehand.
    ///
    /// # Errors
    /// `BadArgument` for a message under 20 octets;
    /// `InvalidConfiguration` if the RTPS protection kind is NONE;
    /// `CryptoFailed` from the cipher.
    pub fn encode_rtps_message_derived(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        let suite = self.suite().ok_or_else(|| {
            config_error("psk: the RTPS protection kind is NONE, so there is no key material")
        })?;
        let header = message.get(..20).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: rtps message too short",
            )
        })?;
        let mut version = [0u8; 2];
        version.copy_from_slice(&header[4..6]);
        let mut vendor = [0u8; 2];
        vendor.copy_from_slice(&header[6..8]);
        let mut prefix = [0u8; 12];
        prefix.copy_from_slice(&header[8..20]);
        let key = {
            let mut sent = self.sent.lock().map_err(|_| {
                SecurityError::new(SecurityErrorKind::Internal, "psk: lock poisoned")
            })?;
            if let Some(key) = sent.get(&(version, vendor, prefix)) {
                Arc::clone(key)
            } else {
                let m = derive_key_material(
                    self.config.domain_id,
                    &self.config.domain_tag,
                    &self.config.passphrase,
                    self.config.key_bits,
                    version,
                    vendor,
                    prefix,
                );
                let key = Arc::new(PskRtpsKey::new(
                    suite,
                    m.sender_key_id,
                    m.revision,
                    m.master_salt,
                    m.master_sender_key,
                )?);
                sent.insert((version, vendor, prefix), Arc::clone(&key));
                key
            }
        };
        key.encode_rtps_message(message)
    }

    /// `decode_rtps_message` (Table 89), with `transform_with_psk` TRUE: the
    /// key material comes from the passphrase the CryptoHeader names (the
    /// primary or an alternative) and the RTPS Header; the kind has to be
    /// the one this participant expects.
    ///
    /// # Errors
    /// `BadArgument` for a message without the flags P and A, with a
    /// SenderKeyId of another domain, a passphrase this participant does not
    /// hold, or another kind than the expected; `CryptoFailed` if the tag
    /// does not verify.
    pub fn decode_rtps_message(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        let suite = self.suite().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: RTPS protection is NONE, nothing to decode",
            )
        })?;
        let wire: PskWire = parse_psk_wire(message)?;
        let ch = &wire.cheader;
        let hdr = &wire.header;
        let prefix = domain_key_prefix(self.config.domain_id, &self.config.domain_tag);
        if ch[4..7] != prefix {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: the SenderKeyId is not of this domain",
            ));
        }
        let all: Vec<&Passphrase> = core::iter::once(&self.config.passphrase)
            .chain(self.config.alternatives.iter())
            .collect();
        let index = all
            .iter()
            .position(|p| p.key_id() == ch[7] && p.revision() == [ch[0], ch[1], ch[2]])
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: the message names a passphrase this participant does not hold",
                )
            })?;
        let mut version = [0u8; 2];
        version.copy_from_slice(&hdr[4..6]);
        let mut vendor = [0u8; 2];
        vendor.copy_from_slice(&hdr[6..8]);
        let mut guid_prefix = [0u8; 12];
        guid_prefix.copy_from_slice(&hdr[8..20]);
        let key = {
            let mut received = self.received.lock().map_err(|_| {
                SecurityError::new(SecurityErrorKind::Internal, "psk: lock poisoned")
            })?;
            if let Some(key) = received.get(&(index, version, vendor, guid_prefix)) {
                Arc::clone(key)
            } else {
                let m = derive_key_material(
                    self.config.domain_id,
                    &self.config.domain_tag,
                    all[index],
                    self.config.key_bits,
                    version,
                    vendor,
                    guid_prefix,
                );
                let key = Arc::new(PskRtpsKey::new(
                    suite,
                    m.sender_key_id,
                    m.revision,
                    m.master_salt,
                    m.master_sender_key,
                )?);
                received.insert((index, version, vendor, guid_prefix), Arc::clone(&key));
                key
            }
        };
        key.decode_rtps_message(&wire)
    }

    /// The CryptoHeader `transformation_kind` this participant writes, `None`
    /// before [`Self::register_local_participant_for`] and for NONE.
    #[must_use]
    pub fn local_transformation_kind(&self) -> Option<[u8; 4]> {
        self.local.as_ref().map(|l| l.key.kind())
    }
}

/// `("org.zerodds.psk.protocol_version", "2.5")`: the ProtocolVersion of the RTPS
/// Header of the messages the key material is for, for the trait operation
/// `register_local_participant`, whose parameters have no place for it.
pub const PROP_PROTOCOL_VERSION: &str = "org.zerodds.psk.protocol_version";
/// The VendorId of the RTPS Header, four hexadecimal digits.
pub const PROP_VENDOR_ID: &str = "org.zerodds.psk.vendor_id";
/// The GuidPrefix of the RTPS Header, 24 hexadecimal digits.
pub const PROP_GUID_PREFIX: &str = "org.zerodds.psk.guid_prefix";

fn hex_bytes<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

fn not_done(what: &'static str) -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::NotImplemented,
        alloc::format!("DDS:Crypto:PSK: {what} does nothing and returns FALSE (Tables 88, 89)"),
    )
}

/// The operations of Tables 87 to 89. The ones that "do nothing and return
/// dummy Handle" return the handle 0; the ones that "do nothing and return
/// FALSE" return `NotImplemented`, which is what this trait calls that;
/// the crypto token operations among them: the pre-shared keys are not
/// sent.
impl CryptographicPlugin for PskCryptoPlugin {
    fn register_local_participant(
        &mut self,
        _identity: IdentityHandle,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        let get = |name: &str| {
            properties
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let version = get(PROP_PROTOCOL_VERSION)
            .and_then(|v| v.split_once('.'))
            .and_then(|(major, minor)| Some([major.parse().ok()?, minor.parse().ok()?]))
            .ok_or_else(|| {
                config_error(alloc::format!(
                    "psk: {PROP_PROTOCOL_VERSION} is 'major.minor'"
                ))
            })?;
        let vendor = get(PROP_VENDOR_ID)
            .and_then(hex_bytes::<2>)
            .ok_or_else(|| {
                config_error(alloc::format!("psk: {PROP_VENDOR_ID} is four hex digits"))
            })?;
        let prefix = get(PROP_GUID_PREFIX)
            .and_then(hex_bytes::<12>)
            .ok_or_else(|| {
                config_error(alloc::format!("psk: {PROP_GUID_PREFIX} is 24 hex digits"))
            })?;
        self.register_local_participant_for(version, vendor, prefix)
            .map(CryptoHandle)
    }

    fn register_matched_remote_participant(
        &mut self,
        _local: CryptoHandle,
        _remote_identity: IdentityHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        Ok(DUMMY_HANDLE)
    }

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        _is_writer: bool,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        Ok(DUMMY_HANDLE)
    }

    fn unregister_matched_remote_participant(
        &mut self,
        _handle: CryptoHandle,
    ) -> SecurityResult<()> {
        // "Releases any resources allocated on the corresponding call to
        // register_local_participant, or register_matched_remote_participant":
        // the second made none.
        Ok(())
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        Err(not_done("create_local_*_crypto_tokens"))
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
        _tokens: &[u8],
    ) -> SecurityResult<()> {
        Err(not_done("set_remote_*_crypto_tokens"))
    }

    fn encrypt_submessage(
        &self,
        _local: CryptoHandle,
        _remote_list: &[CryptoHandle],
        _plaintext: &[u8],
        _aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Err(not_done("encode_*_submessage"))
    }

    fn decrypt_submessage(
        &self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
        _ciphertext: &[u8],
        _aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        Err(not_done("decode_*_submessage"))
    }

    fn encode_rtps_message_psk(
        &self,
        local: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.encode_rtps_message(local.0, message)
    }

    fn decode_rtps_message_psk(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        self.decode_rtps_message(message)
    }

    fn plugin_class_id(&self) -> &str {
        CLASS_ID
    }
}

/// The handle the operations of Tables 87 to 89 return that "do nothing and
/// return dummy Handle": not `HandleNIL`, which stands for a failure.
const DUMMY_SPEC_HANDLE: u64 = u64::MAX;

/// The `CryptoAlgorithmBit` of the key width of the pre-shared key (Table 22).
fn psk_bit(key_bits: u16) -> u32 {
    if key_bits == 128 {
        zerodds_security::algorithm::CBIT_AES128_GCM
    } else {
        zerodds_security::algorithm::CBIT_AES256_GCM
    }
}

fn psk_bad(detail: &'static str) -> SecurityError {
    SecurityError::new(SecurityErrorKind::BadArgument, detail)
}

/// `CryptoKeyFactory` of `DDS:Crypto:PSK` (DDS-Security 1.2 §10.9.3.1,
/// Table 87).
impl zerodds_security::cryptographic::CryptoKeyFactory for PskCryptoPlugin {
    fn register_local_participant(
        &mut self,
        participant_identity: IdentityHandle,
        participant_permissions: PermissionsHandle,
        _participant_properties: &[Property],
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)> {
        if participant_identity.0 == 0 || participant_permissions.0 == 0 {
            return Err(psk_bad(
                "psk: register_local_participant: a nil identity or permissions handle",
            ));
        }
        // The ParticipantKeyMaterial: derived from the passphrase for the
        // RTPS Header of each message it protects (Table 75), so no key is
        // made here; the kind (GCM or GMAC) follows the RTPS protection kind
        // and the width the algorithm property.
        self.suite().ok_or_else(|| {
            config_error("psk: the RTPS protection kind is NONE, so there is no key material")
        })?;
        let handle = self.next_handle;
        self.next_handle += 1;
        let supported = psk_bit(self.config.key_bits);
        // "The operation shall store in the internal state of the plugin the
        // value for particiant_security_config.algorithm_info.symmetric_cipher.supported_mask."
        self.spec_local = Some((
            handle,
            participant_security_config
                .algorithm_info
                .symmetric_cipher
                .supported_mask,
        ));
        // The adjusted_algorithm_info: supported_mask the algorithm of
        // rtps_psk_symmetric_cipher_algorithm; builtin_kx_endpoints_required_mask
        // and builtin_endpoints_required_mask CRYPTO_ALGORITHM_SET_EMPTY; all
        // other members zero. The text also gives symmetric_cipher a
        // `required_mask`, which that type does not have; the member of the
        // type that carries a default for required algorithms,
        // user_endpoints_default_required_mask, is one of "all other members".
        let info = ParticipantSecurityAlgorithmInfo {
            symmetric_cipher: ParticipantSecuritySymmetricCipherAlgorithmInfo {
                supported_mask: supported,
                builtin_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                builtin_kx_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                user_endpoints_default_required_mask: 0,
            },
            ..Default::default()
        };
        Ok((ParticipantCryptoHandle(handle), info))
    }

    fn register_matched_remote_participant(
        &mut self,
        _local_participant_crypto_handle: ParticipantCryptoHandle,
        _remote_participant_identity: IdentityHandle,
        _remote_participant_permissions: PermissionsHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        Ok(ParticipantCryptoHandle(DUMMY_SPEC_HANDLE))
    }

    fn register_local_datawriter(
        &mut self,
        _participant_crypto: ParticipantCryptoHandle,
        _datawriter_properties: &[Property],
        _datawriter_security_config: &EndpointSecurityConfig,
        _endpoint_guid: GuidT,
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        Ok((
            DatawriterCryptoHandle(DUMMY_SPEC_HANDLE),
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
        Ok(DatareaderCryptoHandle(DUMMY_SPEC_HANDLE))
    }

    fn register_local_datareader(
        &mut self,
        _participant_crypto: ParticipantCryptoHandle,
        _datareader_properties: &[Property],
        _datareader_security_config: &EndpointSecurityConfig,
        _endpoint_guid: GuidT,
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        Ok((
            DatareaderCryptoHandle(DUMMY_SPEC_HANDLE),
            EndpointSecurityAlgorithmInfo::default(),
        ))
    }

    fn register_matched_remote_datawriter(
        &mut self,
        _local_datareader_crypto_handle: DatareaderCryptoHandle,
        _remote_participant_crypt: ParticipantCryptoHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle> {
        Ok(DatawriterCryptoHandle(DUMMY_SPEC_HANDLE))
    }

    /// "This operation shall do nothing and return dummy Handle": a key
    /// revision of `0`, which §9.5.1.8.7 reads as "the Plugin does not
    /// support Key Revisions" (the pre-shared key is not sent, so there is
    /// nothing to revise).
    fn revise_local_entity_keys(
        &mut self,
        _participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder> {
        Ok(0)
    }

    /// "This operation shall do nothing and return FALSE."
    fn activate_key_revision(
        &mut self,
        _local_participant_crypto_handle: ParticipantCryptoHandle,
        _key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn unregister_participant(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<bool> {
        if participant_crypto_handle.0 == DUMMY_SPEC_HANDLE {
            // The handle register_matched_remote_participant returned: it
            // allocated nothing.
            return Ok(true);
        }
        if self
            .spec_local
            .is_some_and(|(handle, _)| handle == participant_crypto_handle.0)
        {
            self.spec_local = None;
            if let Ok(mut received) = self.received.lock() {
                received.clear();
            }
            if let Ok(mut sent) = self.sent.lock() {
                sent.clear();
            }
            return Ok(true);
        }
        Err(psk_bad("psk: unknown participant handle"))
    }

    fn unregister_datawriter(
        &mut self,
        datawriter_crypto_handle: DatawriterCryptoHandle,
    ) -> SecurityResult<bool> {
        if datawriter_crypto_handle.0 == DUMMY_SPEC_HANDLE {
            return Ok(true);
        }
        Err(psk_bad("psk: unknown datawriter handle"))
    }

    fn unregister_datareader(
        &mut self,
        datareader_crypto_handle: DatareaderCryptoHandle,
    ) -> SecurityResult<bool> {
        if datareader_crypto_handle.0 == DUMMY_SPEC_HANDLE {
            return Ok(true);
        }
        Err(psk_bad("psk: unknown datareader handle"))
    }
}

impl zerodds_security::cryptographic::CryptographicPluginInterfaces for PskCryptoPlugin {}

/// `CryptoKeyExchange` of `DDS:Crypto:PSK` (§10.9.3.2, Table 88): the tokens
/// do nothing and return FALSE; `return_crypto_tokens` releases the tokens
/// (this plugin issues none).
impl zerodds_security::cryptographic::CryptoKeyExchange for PskCryptoPlugin {
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
            return Ok(true);
        }
        Err(psk_bad(
            "psk: this plugin issues no CryptoToken, so none can be returned",
        ))
    }
}

/// `CryptoTransform` of `DDS:Crypto:PSK` (§10.9.3.3, Table 89): only
/// `encode_rtps_message` and `decode_rtps_message` transform; the others do
/// nothing and return FALSE.
impl zerodds_security::cryptographic::CryptoTransform for PskCryptoPlugin {
    fn encode_serialized_payload(
        &self,
        _plain_buffer: &[u8],
        _sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>> {
        Ok(None)
    }

    fn encode_datawriter_submessage(
        &self,
        _encoded_rtps_submessage: &mut Vec<u8>,
        _plain_rtps_submessage: &[u8],
        _sending_datawriter_crypto: DatawriterCryptoHandle,
        _receiving_datareader_crypto_list: &[DatareaderCryptoHandle],
        _receiving_datareader_crypto_list_index: &mut usize,
    ) -> SecurityResult<bool> {
        Ok(false)
    }

    fn encode_datareader_submessage(
        &self,
        _plain_rtps_submessage: &[u8],
        _sending_datareader_crypto: DatareaderCryptoHandle,
        _receiving_datawriter_crypto_list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Table 89: "The operation checks that the parameter
    /// transform_with_psk=TRUE. If this is not the case the operation shall
    /// fail and return FALSE. The operation checks that the parameter
    /// receiver_specific_macs contains an empty list": the list of receivers
    /// is the one parameter of the operation that stands for them, and has to
    /// be empty. "The transformation uses the ParticipantKeyMaterial
    /// associated with the sending_participant_crypto."
    fn encode_rtps_message(
        &self,
        encoded_rtps_message: &mut Vec<u8>,
        plain_rtps_message: &[u8],
        sending_participant_crypto: ParticipantCryptoHandle,
        receiving_participant_crypto_list: &[ParticipantCryptoHandle],
        receiving_participant_crypto_list_index: &mut usize,
        transform_with_psk: bool,
    ) -> SecurityResult<bool> {
        if !transform_with_psk {
            return Err(psk_bad(
                "psk: encode_rtps_message needs transform_with_psk = TRUE (Table 89)",
            ));
        }
        if !receiving_participant_crypto_list.is_empty() {
            return Err(psk_bad(
                "psk: encode_rtps_message takes no receiver-specific MACs: the list of receivers is empty (Table 89)",
            ));
        }
        if self
            .spec_local
            .is_none_or(|(handle, _)| handle != sending_participant_crypto.0)
        {
            return Err(psk_bad("psk: unknown sending participant handle"));
        }
        *encoded_rtps_message = self.encode_rtps_message_derived(plain_rtps_message)?;
        *receiving_participant_crypto_list_index = 0;
        Ok(true)
    }

    fn decode_rtps_message(
        &self,
        encoded_buffer: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        _sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        if self
            .spec_local
            .is_none_or(|(handle, _)| handle != receiving_participant_crypto.0)
        {
            return Err(psk_bad("psk: unknown receiving participant handle"));
        }
        PskCryptoPlugin::decode_rtps_message(self, encoded_buffer).map(Some)
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// §10.5.2.1.3, the worked example: `data:,5632:castle super radar ...`,
    /// DomainId 201, empty DomainTag, ProtocolVersion 2.5, VendorId 1.1,
    /// GuidPrefix DF CD 91 E1 68 68 04 51 6C B1 B6 0E. The values below are
    /// the ones the specification prints.
    const SECRET: &str = "castle super radar denial swing lunar kind swarm wet toilet output \
                          harbor basic begin margin huge year visit";
    const PREFIX: [u8; 12] = [
        0xDF, 0xCD, 0x91, 0xE1, 0x68, 0x68, 0x04, 0x51, 0x6C, 0xB1, 0xB6, 0x0E,
    ];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| alloc::format!("{b:02x}")).collect()
    }

    #[test]
    fn the_key_material_is_the_one_the_spec_prints() {
        let passphrase = Passphrase::parse(&alloc::format!("5632:{SECRET}")).unwrap();
        assert_eq!(passphrase.id, 0x1600);
        assert_eq!(passphrase.key_id(), 0x00);
        assert_eq!(passphrase.revision(), [0, 0, 22]);
        assert_eq!(passphrase.secret, SECRET);
        assert_eq!(domain_key_prefix(201, ""), [0x61, 0xC8, 0x53]);
        let m = derive_key_material(201, "", &passphrase, 256, [2, 5], [1, 1], PREFIX);
        assert_eq!(m.sender_key_id, [0x61, 0xC8, 0x53, 0x00]);
        assert_eq!(m.revision, [0, 0, 22]);
        assert_eq!(
            hex(&m.master_salt),
            "a4ebff5738dc6826c8d3f5e55a24bb96d9e80147b51c4e49a0927c4fa2cfec8c"
        );
        assert_eq!(
            hex(&m.master_sender_key),
            "4708460adc6bb886521fbdc4b3a9d34e27eed36c162ccdf4fb7427a7f347738b"
        );
        // AES-128 variants: "truncated to the first 128 bits".
        let m128 = derive_key_material(201, "", &passphrase, 128, [2, 5], [1, 1], PREFIX);
        assert_eq!(m128.master_salt, m.master_salt[..16]);
        assert_eq!(m128.master_sender_key, m.master_sender_key[..16]);
        // Another GUID prefix gives another key.
        let other = derive_key_material(201, "", &passphrase, 256, [2, 5], [1, 1], [7; 12]);
        assert_ne!(other.master_sender_key, m.master_sender_key);
    }

    /// Table 86: id and passphrase, the id up to 2^32-1 and not reserved,
    /// the passphrase of printable ASCII up to 512 characters without a
    /// space at either end; a list separated by LF, CR or both.
    #[test]
    fn the_passphrase_rules_of_table_86() {
        let p = Passphrase::parse("5612:Open Sesame").unwrap();
        assert_eq!((p.id, p.secret.as_str()), (5612, "Open Sesame"));
        assert_eq!(
            Passphrase::parse("4294967295:x").unwrap_err().kind,
            SecurityErrorKind::InvalidConfiguration,
            "0xFFFFFFFF has 0xFF as its low octet"
        );
        assert!(Passphrase::parse("4294967294:x").is_ok());
        for bad in [
            "4294967296:x",
            "255:x",
            "511:x",
            "12:",
            "12: lead",
            "12:trail ",
            "12:tab\there",
            "12:ü",
            "x:y",
            ":y",
            "no colon",
            &alloc::format!("1:{}", "a".repeat(513)),
        ] {
            assert!(Passphrase::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Passphrase::parse(&alloc::format!("1:{}", "a".repeat(512))).is_ok());
        assert_eq!(Passphrase::parse("1:a:b").unwrap().secret, "a:b");
        let list =
            parse_passphrase_list("5613:Extra\n5614:Another\r\n5615:Yet\r5616:Last\n").unwrap();
        assert_eq!(
            list.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![5613, 5614, 5615, 5616]
        );
        assert!(parse_passphrase_list("1:ok\n255:reserved").is_err());
        // "AUTO" is AES256-GCM.
        assert_eq!(key_bits_from_cipher(None).unwrap(), 256);
        assert_eq!(key_bits_from_cipher(Some("AUTO")).unwrap(), 256);
        assert_eq!(key_bits_from_cipher(Some("AES128-GCM")).unwrap(), 128);
        assert!(key_bits_from_cipher(Some("ChaCha")).is_err());
    }

    fn config(protection: PskProtection, bits: u16) -> PskConfig {
        PskConfig {
            domain_id: 201,
            domain_tag: String::new(),
            passphrase: Passphrase::parse(&alloc::format!("5632:{SECRET}")).unwrap(),
            alternatives: Vec::new(),
            key_bits: bits,
            protection,
        }
    }

    fn message(prefix: [u8; 12]) -> Vec<u8> {
        let mut m = b"RTPS".to_vec();
        m.extend_from_slice(&[2, 5, 1, 1]);
        m.extend_from_slice(&prefix);
        // INFO_TS and a DATA-like submessage of 8 octets.
        m.extend_from_slice(&[0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]);
        m.extend_from_slice(&[0x15, 0x01, 0x08, 0x00, 9, 9, 9, 9, 9, 9, 9, 9]);
        m
    }

    fn sender(protection: PskProtection, bits: u16) -> (PskCryptoPlugin, u64) {
        let mut p = PskCryptoPlugin::new(config(protection, bits)).unwrap();
        let h = p
            .register_local_participant_for([2, 5], [1, 1], PREFIX)
            .unwrap();
        (p, h)
    }

    /// Table 89, `encode_rtps_message` / `decode_rtps_message`: the RTPS
    /// Header stays, the SRTPS_PREFIX has the flags A and P, its
    /// CryptoHeader names the kind (the key revision from the passphrase id,
    /// the algorithm id) and the SenderKeyId; GCM encrypts the body, GMAC
    /// leaves it; the receiver derives the sender's key from the header.
    #[test]
    fn a_message_goes_through_encode_and_decode() {
        for (protection, bits, id, encrypted) in [
            (PskProtection::Encrypt, 256, 0x04u8, true),
            (PskProtection::Encrypt, 128, 0x02, true),
            (PskProtection::Sign, 256, 0x03, false),
            (PskProtection::Sign, 128, 0x01, false),
        ] {
            let (tx, h) = sender(protection, bits);
            let plain = message(PREFIX);
            let wire = tx.encode_rtps_message(h, &plain).unwrap();
            assert_eq!(&wire[..20], &plain[..20], "the RTPS Header stays");
            assert_eq!(wire[20], 0x33, "SRTPS_PREFIX");
            assert_eq!(wire[21] & 0x06, 0x06, "flags A and P");
            assert_eq!(
                &wire[24..28],
                &[0, 0, 22, id],
                "kind: revision, algorithm id"
            );
            assert_eq!(&wire[28..32], &[0x61, 0xC8, 0x53, 0x00], "SenderKeyId");
            assert_eq!(
                tx.local_transformation_kind(),
                Some([0, 0, 22, id]),
                "{protection:?} {bits}"
            );
            let body_visible = wire.windows(8).any(|w| w == [9u8; 8]);
            assert_eq!(body_visible, !encrypted, "{protection:?} {bits}");
            // another participant of the domain, another process
            let rx = PskCryptoPlugin::new(config(protection, bits)).unwrap();
            assert_eq!(rx.decode_rtps_message(&wire).unwrap(), plain);
        }
    }

    #[test]
    fn what_does_not_hold_is_refused() {
        let (tx, h) = sender(PskProtection::Encrypt, 256);
        let plain = message(PREFIX);
        let wire = tx.encode_rtps_message(h, &plain).unwrap();
        let rx = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        assert!(rx.decode_rtps_message(&wire).is_ok());
        // The RTPS Header is under the tag (AAD): another prefix, other
        // version or vendor.
        for at in [4usize, 6, 8, 19] {
            let mut forged = wire.clone();
            forged[at] ^= 0x01;
            assert!(
                rx.decode_rtps_message(&forged).is_err(),
                "header octet {at}"
            );
        }
        // The body.
        let mut forged = wire.clone();
        let n = forged.len();
        forged[n / 2 + 8] ^= 0x01;
        assert!(rx.decode_rtps_message(&forged).is_err());
        // Another passphrase (same id), another domain, another expected
        // kind, a message without the flag A, a message without the flag P.
        let mut other = config(PskProtection::Encrypt, 256);
        other.passphrase.secret.push('!');
        assert!(
            PskCryptoPlugin::new(other)
                .unwrap()
                .decode_rtps_message(&wire)
                .is_err()
        );
        let mut other = config(PskProtection::Encrypt, 256);
        other.domain_id = 202;
        assert!(
            PskCryptoPlugin::new(other)
                .unwrap()
                .decode_rtps_message(&wire)
                .is_err()
        );
        for expected in [(PskProtection::Sign, 256), (PskProtection::Encrypt, 128)] {
            assert!(
                PskCryptoPlugin::new(config(expected.0, expected.1))
                    .unwrap()
                    .decode_rtps_message(&wire)
                    .is_err(),
                "{expected:?}: the kind is not the expected one"
            );
        }
        for flag in [0x02u8, 0x04] {
            let mut forged = wire.clone();
            forged[21] &= !flag;
            assert!(rx.decode_rtps_message(&forged).is_err(), "flag {flag:#04x}");
        }
        // NONE protects nothing: no key material, nothing to decode with.
        let mut none = PskCryptoPlugin::new(config(PskProtection::None, 256)).unwrap();
        assert!(
            none.register_local_participant_for([2, 5], [1, 1], PREFIX)
                .is_err()
        );
        assert!(none.decode_rtps_message(&wire).is_err());
        assert!(tx.encode_rtps_message(h + 1, &plain).is_err());
    }

    /// Table 86, `rtps_psk_secret_passphrase_alt`: a message protected with
    /// an alternative passphrase of the receiver is accepted.
    #[test]
    fn an_alternative_passphrase_decodes() {
        let (tx, h) = sender(PskProtection::Encrypt, 256);
        let plain = message(PREFIX);
        let wire = tx.encode_rtps_message(h, &plain).unwrap();
        let mut cfg = config(PskProtection::Encrypt, 256);
        let old = cfg.passphrase.clone();
        cfg.passphrase = Passphrase::parse("7000:the new passphrase").unwrap();
        cfg.alternatives = vec![Passphrase::parse("9:unrelated").unwrap(), old];
        let rx = PskCryptoPlugin::new(cfg).unwrap();
        assert_eq!(rx.decode_rtps_message(&wire).unwrap(), plain);
        // twice: the derived key is found again
        assert_eq!(rx.decode_rtps_message(&wire).unwrap(), plain);
    }

    /// Tables 87 to 89: what does nothing returns the dummy handle or fails,
    /// as `FALSE`.
    #[test]
    fn the_other_operations_do_nothing() {
        use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
        let mut p = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        let local = p
            .register_local_participant_for([2, 5], [1, 1], PREFIX)
            .map(CryptoHandle)
            .unwrap();
        assert_eq!(
            p.register_matched_remote_participant(local, IdentityHandle(1), SharedSecretHandle(1))
                .unwrap(),
            CryptoHandle(0)
        );
        assert_eq!(
            p.register_local_endpoint(local, true, &[]).unwrap(),
            CryptoHandle(0)
        );
        assert!(
            p.create_local_participant_crypto_tokens(local, CryptoHandle(0))
                .is_err()
        );
        assert!(
            p.set_remote_participant_crypto_tokens(local, CryptoHandle(0), &[])
                .is_err()
        );
        assert!(p.encrypt_submessage(local, &[], b"x", &[]).is_err());
        assert!(
            p.decrypt_submessage(local, CryptoHandle(0), b"x", &[])
                .is_err()
        );
        assert!(p.encode_serialized_payload(local, b"x").is_err());
        assert!(p.decode_serialized_payload(b"x").is_err());
        assert!(
            p.unregister_matched_remote_participant(CryptoHandle(0))
                .is_ok()
        );
        // The trait operation reads the header values from its properties.
        let mut q = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        let handle = q
            .register_local_participant(
                IdentityHandle(1),
                &[
                    (PROP_PROTOCOL_VERSION, "2.5"),
                    (PROP_VENDOR_ID, "0101"),
                    (PROP_GUID_PREFIX, "dfcd91e1686804516cb1b60e"),
                ],
            )
            .unwrap();
        let wire = q.encode_rtps_message_psk(handle, &message(PREFIX)).unwrap();
        assert_eq!(p.decode_rtps_message_psk(&wire).unwrap(), message(PREFIX));
        assert!(
            q.register_local_participant(IdentityHandle(1), &[])
                .is_err()
        );
    }

    /// Table 89, 3): a HeaderExtension stays behind the RTPS Header with its
    /// `messageLength` set to the length of the encoded message, and is under
    /// the tag with `messageLength` zero.
    #[test]
    fn a_header_extension_is_kept_and_its_length_set() {
        let (tx, h) = sender(PskProtection::Encrypt, 256);
        let mut plain = message(PREFIX)[..20].to_vec();
        // HeaderExtension (id 0, flags: little endian + L), messageLength = 1234.
        plain.extend_from_slice(&[0x00, 0x03, 0x04, 0x00]);
        plain.extend_from_slice(&1234u32.to_le_bytes());
        plain.extend_from_slice(&message(PREFIX)[20..]);
        let wire = tx.encode_rtps_message(h, &plain).unwrap();
        assert_eq!(&wire[20..22], &[0x00, 0x03]);
        let length = u32::from_le_bytes([wire[24], wire[25], wire[26], wire[27]]) as usize;
        assert_eq!(length, wire.len());
        let rx = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        let decoded = rx.decode_rtps_message(&wire).unwrap();
        assert_eq!(&decoded[..20], &plain[..20]);
        assert_eq!(&decoded[decoded.len() - 24..], &plain[plain.len() - 24..]);
        // Table 89, 3.3 and 4: the decoded message carries messageLength and
        // messageChecksum of the decoded message.
        let decoded_length =
            u32::from_le_bytes([decoded[24], decoded[25], decoded[26], decoded[27]]) as usize;
        assert_eq!(decoded_length, decoded.len());
    }

    /// Table 89, 3.2 and 3.3: a messageChecksum (CRC-32C) is zero under the
    /// tag and set to the checksum of the encoded message afterwards; the
    /// decoded message has the checksum of the decoded message.
    #[test]
    fn a_header_extension_with_a_checksum_is_adjusted_both_ways() {
        let (tx, h) = sender(PskProtection::Encrypt, 256);
        let mut plain = message(PREFIX)[..20].to_vec();
        // HeaderExtension: E | L | C=01; messageLength, checksum.
        plain.extend_from_slice(&[0x00, 0x23, 0x08, 0x00]);
        plain.extend_from_slice(&0u32.to_le_bytes());
        plain.extend_from_slice(&0u32.to_be_bytes());
        plain.extend_from_slice(&message(PREFIX)[20..]);
        let wire = tx.encode_rtps_message(h, &plain).unwrap();
        let mut zeroed = wire.clone();
        zeroed[28..32].fill(0);
        assert_eq!(
            &wire[28..32],
            &zerodds_foundation::crc32c(&zeroed).to_be_bytes(),
            "the checksum of the encoded message"
        );
        let rx = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        let decoded = rx.decode_rtps_message(&wire).unwrap();
        let mut zeroed = decoded.clone();
        zeroed[28..32].fill(0);
        assert_eq!(
            &decoded[28..32],
            &zerodds_foundation::crc32c(&zeroed).to_be_bytes(),
            "the checksum of the decoded message"
        );
        assert_eq!(
            u32::from_le_bytes([decoded[24], decoded[25], decoded[26], decoded[27]]) as usize,
            decoded.len()
        );
    }

    fn nonnil_pconfig() -> ParticipantSecurityConfig {
        ParticipantSecurityConfig {
            is_rtps_psk_protected: true,
            ..Default::default()
        }
    }

    /// §10.9.3.1 Table 87: `register_local_participant` stores the key
    /// material's algorithm, fills `adjusted_algorithm_info` with the
    /// algorithm of `rtps_psk_symmetric_cipher_algorithm` as supported mask,
    /// empty builtin masks and zero elsewhere; the other registrations "do
    /// nothing and return dummy Handle"; `revise_local_entity_keys` has no
    /// revisions; `activate_key_revision` returns FALSE; the unregister
    /// operations release.
    #[test]
    fn the_key_factory_of_table_87() {
        use zerodds_security::algorithm::{CBIT_AES128_GCM, CBIT_AES256_GCM};
        use zerodds_security::cryptographic::CryptoKeyFactory as F;
        for (bits, bit) in [(256u16, CBIT_AES256_GCM), (128u16, CBIT_AES128_GCM)] {
            let mut p = PskCryptoPlugin::new(config(PskProtection::Encrypt, bits)).unwrap();
            let (h, info) = F::register_local_participant(
                &mut p,
                IdentityHandle(1),
                PermissionsHandle(1),
                &[],
                &nonnil_pconfig(),
            )
            .unwrap();
            assert!(!h.is_nil());
            let sc = info.symmetric_cipher;
            assert_eq!(sc.supported_mask, bit);
            assert_eq!(
                sc.builtin_endpoints_required_mask,
                CRYPTO_ALGORITHM_SET_EMPTY
            );
            assert_eq!(
                sc.builtin_kx_endpoints_required_mask,
                CRYPTO_ALGORITHM_SET_EMPTY
            );
            assert_eq!(sc.user_endpoints_default_required_mask, 0);
            assert_eq!(info.digital_signature, Default::default());
            assert_eq!(info.key_establishment, Default::default());
            // The supported_mask of the config is kept in the plugin.
            assert_eq!(p.spec_local, Some((h.0, 0)));
            let remote = F::register_matched_remote_participant(
                &mut p,
                h,
                IdentityHandle(2),
                PermissionsHandle(2),
                SharedSecretHandle(1),
            )
            .unwrap();
            assert!(!remote.is_nil(), "a dummy handle, not HandleNIL");
            let cfg = EndpointSecurityConfig::default();
            let (w, _) = F::register_local_datawriter(&mut p, h, &[], &cfg, [0; 16]).unwrap();
            let (r, _) = F::register_local_datareader(&mut p, h, &[], &cfg, [0; 16]).unwrap();
            assert!(!w.is_nil() && !r.is_nil());
            let mr = F::register_matched_remote_datareader(
                &mut p,
                w,
                remote,
                SharedSecretHandle(1),
                false,
            )
            .unwrap();
            let mw =
                F::register_matched_remote_datawriter(&mut p, r, remote, SharedSecretHandle(1))
                    .unwrap();
            assert!(!mr.is_nil() && !mw.is_nil());
            assert_eq!(F::revise_local_entity_keys(&mut p, h).unwrap(), 0);
            assert!(!F::activate_key_revision(&mut p, h, 1).unwrap());
            assert!(F::unregister_datawriter(&mut p, w).unwrap());
            assert!(F::unregister_datareader(&mut p, r).unwrap());
            assert!(F::unregister_participant(&mut p, remote).unwrap());
            assert!(F::unregister_participant(&mut p, h).unwrap());
            assert!(
                F::unregister_participant(&mut p, h).is_err(),
                "released once"
            );
            assert_eq!(p.spec_local, None);
        }
        // The supported_mask of the config is stored.
        let mut p = PskCryptoPlugin::new(config(PskProtection::Sign, 256)).unwrap();
        let mut c = nonnil_pconfig();
        c.algorithm_info.symmetric_cipher.supported_mask = 0x0f;
        let (h, _) =
            F::register_local_participant(&mut p, IdentityHandle(1), PermissionsHandle(1), &[], &c)
                .unwrap();
        assert_eq!(p.spec_local, Some((h.0, 0x0f)));
        // A nil identity or permissions handle, and an RTPS protection kind
        // NONE (no key material to make).
        assert!(
            F::register_local_participant(&mut p, IdentityHandle(0), PermissionsHandle(1), &[], &c)
                .is_err()
        );
        let mut none = PskCryptoPlugin::new(config(PskProtection::None, 256)).unwrap();
        assert!(
            F::register_local_participant(
                &mut none,
                IdentityHandle(1),
                PermissionsHandle(1),
                &[],
                &c
            )
            .is_err()
        );
        assert!(F::unregister_datawriter(&mut p, DatawriterCryptoHandle(5)).is_err());
        assert!(F::unregister_datareader(&mut p, DatareaderCryptoHandle(5)).is_err());
    }

    /// §10.9.3.2 Table 88: the token operations "do nothing and return
    /// FALSE"; `return_crypto_tokens` releases tokens, and this plugin has
    /// issued none.
    #[test]
    fn the_key_exchange_of_table_88() {
        use zerodds_security::cryptographic::CryptoKeyExchange as E;
        let mut p = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
        let (pc, wc, rc) = (
            ParticipantCryptoHandle(2),
            DatawriterCryptoHandle(3),
            DatareaderCryptoHandle(4),
        );
        assert!(
            E::create_local_participant_crypto_tokens(&mut p, pc, pc, 0)
                .unwrap()
                .is_none()
        );
        assert!(
            E::create_local_datawriter_crypto_tokens(&mut p, wc, rc, 0)
                .unwrap()
                .is_none()
        );
        assert!(
            E::create_local_datareader_crypto_tokens(&mut p, rc, wc, 0)
                .unwrap()
                .is_none()
        );
        assert!(!E::set_remote_participant_crypto_tokens(&mut p, pc, pc, &[]).unwrap());
        assert!(!E::set_remote_datawriter_crypto_tokens(&mut p, rc, wc, &[]).unwrap());
        assert!(!E::set_remote_datareader_crypto_tokens(&mut p, wc, rc, &[]).unwrap());
        assert!(E::return_crypto_tokens(&mut p, Vec::new()).unwrap());
        let token = zerodds_security::token::DataHolder::new("DDS:Crypto:AES_GCM_GMAC");
        assert!(E::return_crypto_tokens(&mut p, alloc::vec![token]).is_err());
    }

    /// §10.9.3.3 Table 89: only `encode_rtps_message` and
    /// `decode_rtps_message` transform; the others "do nothing and return
    /// FALSE". `encode_rtps_message` fails without `transform_with_psk` and
    /// with receiver-specific parts, and sets the flags A and P.
    #[test]
    fn the_transform_of_table_89() {
        use zerodds_security::cryptographic::CryptoKeyFactory as F;
        use zerodds_security::cryptographic::CryptoTransform as T;
        let make = || {
            let mut p = PskCryptoPlugin::new(config(PskProtection::Encrypt, 256)).unwrap();
            let (h, _) = F::register_local_participant(
                &mut p,
                IdentityHandle(1),
                PermissionsHandle(1),
                &[],
                &nonnil_pconfig(),
            )
            .unwrap();
            (p, h)
        };
        let (tx, th) = make();
        let (rx, rh) = make();
        let plain = message(PREFIX);
        let mut out = Vec::new();
        let mut index = 5;
        assert!(T::encode_rtps_message(&tx, &mut out, &plain, th, &[], &mut index, true).unwrap());
        assert_eq!(index, 0);
        assert_eq!(out[21] & 0x06, 0x06, "the flags A and P");
        assert_eq!(
            T::decode_rtps_message(&rx, &out, rh, ParticipantCryptoHandle(77))
                .unwrap()
                .unwrap(),
            plain
        );
        // transform_with_psk FALSE: fails.
        assert!(
            T::encode_rtps_message(&tx, &mut Vec::new(), &plain, th, &[], &mut 0, false).is_err()
        );
        // A list of receivers: receiver-specific MACs are not made.
        assert!(
            T::encode_rtps_message(
                &tx,
                &mut Vec::new(),
                &plain,
                th,
                &[ParticipantCryptoHandle(9)],
                &mut 0,
                true
            )
            .is_err()
        );
        // A handle that is not the registered participant.
        assert!(
            T::encode_rtps_message(
                &tx,
                &mut Vec::new(),
                &plain,
                ParticipantCryptoHandle(99),
                &[],
                &mut 0,
                true
            )
            .is_err()
        );
        assert!(T::decode_rtps_message(&rx, &out, ParticipantCryptoHandle(99), th).is_err());
        // The rest does nothing.
        let (wc, rc) = (DatawriterCryptoHandle(3), DatareaderCryptoHandle(4));
        assert!(
            T::encode_serialized_payload(&tx, b"x", wc)
                .unwrap()
                .is_none()
        );
        assert!(
            !T::encode_datawriter_submessage(&tx, &mut Vec::new(), b"x", wc, &[rc], &mut 0)
                .unwrap()
        );
        assert!(
            T::encode_datareader_submessage(&tx, b"x", rc, &[wc])
                .unwrap()
                .is_none()
        );
        assert!(
            T::preprocess_secure_submsg(&tx, b"x", th, th)
                .unwrap()
                .is_none()
        );
        assert!(
            T::decode_datawriter_submessage(&tx, b"x", rc, wc)
                .unwrap()
                .is_none()
        );
        assert!(
            T::decode_datareader_submessage(&tx, b"x", wc, rc)
                .unwrap()
                .is_none()
        );
        assert!(
            T::decode_serialized_payload(&tx, b"x", &[], rc, wc)
                .unwrap()
                .is_none()
        );
    }

    /// Table 71, Table 86: the properties name URIs: `data:,` and `file:`;
    /// both spellings of the list of alternatives; the algorithm.
    #[test]
    fn the_uri_properties_of_the_pre_shared_key_are_read() {
        let props = [
            (
                "dds.sec.crypto.rtps_psk_secret_passphrase",
                "data:,5612:Open Sesame",
            ),
            (
                "org.zerodds.sec.crypto.rtps_psk_secret_passphrase_extra",
                "data:,5613:Second\n5614:Third",
            ),
            (
                "dds.sec.crypto.rtps_psk_symmetric_cipher_algorithm",
                "AES128-GMAC",
            ),
        ];
        let c = PskConfig::from_uri_properties(&props, 7, "tag", PskProtection::Sign).unwrap();
        assert_eq!((c.domain_id, c.domain_tag.as_str()), (7, "tag"));
        assert_eq!(
            (c.passphrase.id, c.passphrase.secret.as_str()),
            (5612, "Open Sesame")
        );
        assert_eq!(c.alternatives.len(), 2);
        assert_eq!(c.alternatives[1].id, 5614);
        assert_eq!(c.key_bits, 128);
        // The file: scheme, in the three spellings.
        let dir = std::env::temp_dir().join(alloc::format!("zerodds-psk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("psk.txt");
        std::fs::write(&file, "5612:Open Sesame\n").unwrap();
        for uri in [
            alloc::format!("file:{}", file.display()),
            alloc::format!("file://{}", file.display()),
            alloc::format!("file://localhost{}", file.display()),
        ] {
            let p = [("dds.sec.crypto.rtps_psk_secret_passphrase", uri.as_str())];
            let c = PskConfig::from_uri_properties(&p, 7, "", PskProtection::Encrypt).unwrap();
            assert_eq!(c.passphrase.secret, "Open Sesame", "{uri}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
        // Refused: another scheme, no comma, no passphrase, a missing file, a
        // host that is not this one.
        for bad in [
            "http://example.org/psk",
            "data:5612:Open Sesame",
            "file:/nonexistent/zerodds/psk.txt",
            "file://elsewhere/psk.txt",
        ] {
            let p = [("dds.sec.crypto.rtps_psk_secret_passphrase", bad)];
            assert!(
                PskConfig::from_uri_properties(&p, 7, "", PskProtection::Sign).is_err(),
                "{bad}"
            );
        }
        assert!(PskConfig::from_uri_properties(&[], 7, "", PskProtection::Sign).is_err());
    }
}
