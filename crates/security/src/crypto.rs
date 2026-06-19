// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cryptographic plugin SPI (OMG DDS-Security 1.1 §8.5).
//!
//! The SPI is split into three sub-interfaces (spec structure):
//! * **KeyFactory** (§8.5.1.7) — key derivation from `SharedSecret`.
//! * **KeyExchange** (§8.5.1.8) — exchange key tokens between peers.
//! * **Transform** (§8.5.1.9) — the concrete encrypt/decrypt/sign/verify.
//!
//! We bundle the three into one trait (`CryptographicPlugin`), so that
//! users only have to hold a single `Box<dyn ...>`. Backend impls
//! (rustls, ring, mbedtls) implement all three sub-interfaces.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn CryptographicPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::authentication::{IdentityHandle, SharedSecretHandle};
use crate::error::SecurityResult;

/// Opaque handle for derived key material (master key
/// of a participant/endpoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CryptoHandle(pub u64);

/// Receiver-specific MAC (spec §7.3.6.3 `ReceiverSpecificMAC`).
///
/// When a sender sends a ciphertext to N receivers with the **same
/// suite but different keys**, a 16-byte truncated HMAC is computed
/// per receiver. The wire representation is
/// a sequence of `(key_id, mac)` pairs in the SEC_POSTFIX.
///
/// `key_id` is the spec-conformant 4-byte ID (typically the low 32 bits
/// of the sender-side [`CryptoHandle`] for this receiver), by which
/// the receiver finds its specific MAC entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReceiverMac {
    /// 4-byte `CryptoTransformKeyId` from §7.3.6.3.
    pub key_id: u32,
    /// 16-byte truncated HMAC-SHA256 over the ciphertext.
    pub mac: [u8; 16],
}

impl ReceiverMac {
    /// Wire size of a single `ReceiverMac` entry (spec §7.3.6.3).
    pub const WIRE_SIZE: usize = 4 + 16;
}

/// Cryptographic plugin (spec §8.5.1). In v1.3 this is a pure
/// **interface** — production impls live in `zerodds-security-crypto`
/// (AES-GCM + HMAC), `zerodds-security-keyexchange` (DH key exchange,
/// spec §9.5.3) and `zerodds-security-rtps` (RTPS header AAD wrapper,
/// spec §7.3.5).
pub trait CryptographicPlugin: Send + Sync {
    // -------- KeyFactory (§8.5.1.7) --------

    /// Optional second (payload) token of a local datawriter endpoint,
    /// when metadata and data protection have different suites (cyclone
    /// dual-key model: `writer_key_material_message` + `writer_key_material_payload`).
    /// `None` = single key (all profiles with metadata == data).
    fn endpoint_payload_token(&self, _handle: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        None
    }

    /// Creates participant crypto material from the handshake
    /// SharedSecret.
    fn register_local_participant(
        &mut self,
        identity: IdentityHandle,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle>;

    /// Creates crypto material for a remote participant.
    fn register_matched_remote_participant(
        &mut self,
        local: CryptoHandle,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle>;

    /// Creates crypto material for a local DataWriter/Reader.
    fn register_local_endpoint(
        &mut self,
        participant: CryptoHandle,
        is_writer: bool,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle>;

    // -------- KeyExchange (§8.5.1.8) --------

    /// Creates the `ParticipantCryptoTokens` blob that is sent to the
    /// remote participant (contains encrypted
    /// key material).
    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>>;

    /// Processes the tokens from the remote participant. Afterwards the
    /// keys for encrypted submessages are mutually known.
    fn set_remote_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()>;

    // -------- Transform (§8.5.1.9) --------

    /// Encrypt + sign an RTPS submessage. Input: plain submessage
    /// bytes. Output: `SecureSubmessage` payload (ciphertext + tag).
    ///
    /// `aad_extension` is the spec-conformant AAD extension (spec §10.5.2
    /// Tab.78). Submessage protection (§8.5.1.9.2) provides
    /// `SubmessageHeader || SecureSubmessageHeader` bytes here;
    /// RTPS message protection (§8.5.1.9.7) the RTPS header (20 bytes) +
    /// SecureRTPSSubmessageHeader. Empty (`&[]`) is only spec-conformant
    /// if the caller explicitly accepts spec §8.1 Tab.78 without header
    /// coverage (e.g. pre-shared-key path without header auth).
    ///
    /// Spec §8.5.1.9.1 `encode_serialized_payload`.
    fn encrypt_submessage(
        &self,
        local: CryptoHandle,
        remote_list: &[CryptoHandle],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Decrypt + Verify. Output: plain submessage bytes. `aad_extension`
    /// must be byte-identical to the sender AAD (otherwise tag mismatch).
    ///
    /// Spec §8.5.1.9.4 `decode_serialized_payload`.
    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Encrypt+Sign with `Receiver-Specific-MACs` (spec
    /// §7.3.6.3). Produces **one** ciphertext (sender key) plus
    /// one 16-byte truncated HMAC per remote.
    ///
    /// The `receivers` list contains `(handle, key_id)` per receiver:
    /// * `handle` — CryptoHandle to the MAC key in the plugin slot
    ///   (typically the per-peer key derived from
    ///   `register_matched_remote_participant`).
    /// * `key_id` — 4-byte wire identifier the receiver looks up in
    ///   the MAC list (must be synchronized between sender and receiver,
    ///   typically the low 32 bits of the peer GuidPrefix).
    ///
    /// Default impl: falls back to `encrypt_submessage` and
    /// returns an empty MAC list — plugins without multi-MAC support
    /// thereby signal the caller "please use the multi-cipher
    /// fan-out".
    fn encrypt_submessage_multi(
        &self,
        local: CryptoHandle,
        receivers: &[(CryptoHandle, u32)],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<ReceiverMac>)> {
        let handles: Vec<CryptoHandle> = receivers.iter().map(|(h, _)| *h).collect();
        let ciphertext = self.encrypt_submessage(local, &handles, plaintext, aad_extension)?;
        Ok((ciphertext, Vec::new()))
    }

    /// Verify Receiver-Specific-MAC + Decrypt.
    ///
    /// `own_key_id` is the wire ID under which the receiver is found in
    /// the MAC list; `own_mac_key_handle` is the slot with the
    /// associated HMAC key.
    ///
    /// When `macs.is_empty()` this delegates to [`Self::decrypt_submessage`]
    /// (backward compat).
    ///
    /// # Errors
    /// * `CryptoFailed` if no MAC entry matches the `own_key_id`
    ///   or the MAC comparison fails.
    #[allow(clippy::too_many_arguments)]
    fn decrypt_submessage_with_receiver_mac(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        own_key_id: u32,
        own_mac_key_handle: CryptoHandle,
        ciphertext: &[u8],
        macs: &[ReceiverMac],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (own_key_id, own_mac_key_handle);
        if macs.is_empty() {
            return self.decrypt_submessage(local, remote, ciphertext, aad_extension);
        }
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement receiver-specific mac verification",
        ))
    }

    // -------- KeyExchange channel protection (§9.5.3.5) --------
    //
    // The `DCPSParticipantVolatileMessageSecure` channel (over which the
    // ParticipantCryptoTokens are exchanged) is protected with a **Kx key**
    // derived from the handshake SharedSecret —
    // separate from the data key (which only arrives via token). This
    // lets both sides exchange tokens confidentially before the
    // data key is established.

    /// Encrypt+Sign a VolatileSecure payload with the **Kx key** of the
    /// peer slot (`handle` from `register_matched_remote_participant`).
    ///
    /// Default: `NotImplemented` — plugins without Kx channel support.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn encode_kx_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, plaintext, aad_extension);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement key-exchange-channel protection",
        ))
    }

    /// Decrypt+Verify a VolatileSecure payload with the **Kx key**.
    /// Counterpart to [`Self::encode_kx_submessage`].
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_kx_submessage(
        &self,
        handle: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, ciphertext, aad_extension);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement key-exchange-channel protection",
        ))
    }

    /// **Cyclone-conformant** VolatileSecure submessage protection (DDS-Security
    /// §9.5.3): encodes `plaintext` as a `SEC_PREFIX` + `SEC_BODY` + `SEC_POSTFIX`
    /// submessage sequence with the **Kx key** of the handle (AES256-GCM, empty AAD,
    /// 20-byte CryptoHeader, common_mac in the postfix). Wire-byte-identical to
    /// cyclone `encode_datawriter_submessage` — for the cross-vendor
    /// crypto token exchange over `ParticipantVolatileMessageSecure`.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn encode_kx_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, plaintext);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone-format kx submessage protection",
        ))
    }

    /// Counterpart to [`Self::encode_kx_datawriter_submessage`]: decodes a
    /// `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX` sequence with the Kx key.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_kx_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, wire);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone-format kx submessage protection",
        ))
    }

    /// Like [`Self::encode_kx_datawriter_submessage`], but with the **data key**
    /// of the slot (regular slot, not Kx) — for user DATA submessage
    /// protection (`metadata_protection_kind=ENCRYPT`, §9.5.3.3). Wire-identical
    /// to cyclone `encode_datawriter_submessage`.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn encode_data_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, plaintext);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone-format data submessage protection",
        ))
    }

    /// Counterpart to [`Self::encode_data_datawriter_submessage`].
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_data_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, wire);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone-format data submessage protection",
        ))
    }

    /// Decodes a SEC_* submessage by the `transformation_key_id` in the
    /// CryptoHeader (DDS-Security §9.5.2.1.1) — finds the matching remote
    /// key material itself, without the caller knowing the endpoint handle.
    /// Needed because every remote endpoint (incl. the secure built-in
    /// discovery endpoints) has its own per-endpoint key and the receiver
    /// can only map the key via the key_id on the wire (multiple keys per peer).
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument` (no key for the key_id).
    fn decode_data_by_key_id(&self, wire: &[u8]) -> SecurityResult<Vec<u8>> {
        let _ = wire;
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement key-id-based data submessage decode",
        ))
    }

    /// DDS-Security §8.4.2.4 / §7.3.7 RTPS message protection (SRTPS),
    /// cyclone-conformant: wraps the whole RTPS message (`[header(20) | body]`) in
    /// SRTPS_PREFIX(CryptoHeader with transformation_key_id) / SEC_BODY /
    /// SRTPS_POSTFIX. Unlike `encode_secured_rtps_message`, the
    /// SRTPS_PREFIX carries the real CryptoTransformIdentifier -> the receiver
    /// finds the key by key_id (cross-vendor / cyclone interop).
    ///
    /// # Errors
    /// `NotImplemented` (default) or crypto/argument error.
    fn encode_rtps_message_cyclone(
        &self,
        local: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (local, message);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone SRTPS encode",
        ))
    }

    /// Counterpart to [`Self::encode_rtps_message_cyclone`]: key_id-based
    /// SRTPS decode (remote_by_key_id, fallback to local slot for self-test).
    ///
    /// # Errors
    /// `NotImplemented` (default) or crypto/argument error.
    fn decode_rtps_message_cyclone(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        let _ = message;
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement cyclone SRTPS decode",
        ))
    }

    /// §8.5.1.9.1 / §9.5.3.3.1 `encode_serialized_payload` (data_protection,
    /// INNER payload layer). Protects the SerializedPayload of an endpoint
    /// (`handle` = per-endpoint writer key) without a submessage frame.
    ///
    /// # Errors
    /// `NotImplemented` (default) or crypto/argument error.
    fn encode_serialized_payload(
        &self,
        handle: CryptoHandle,
        payload: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, payload);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement encode_serialized_payload",
        ))
    }

    /// §8.5.1.9.4 / §9.5.3.3.1 `decode_serialized_payload`; key via the
    /// `transformation_key_id` in the CryptoHeader.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_serialized_payload(&self, encoded: &[u8]) -> SecurityResult<Vec<u8>> {
        let _ = encoded;
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement decode_serialized_payload",
        ))
    }

    /// Like [`Self::decode_serialized_payload`], but with an EXPLICIT remote handle
    /// instead of a key_id lookup. Needed for peers that index their remote key
    /// material via the GuidPrefix slot table (token exchange) instead of a unique
    /// `transformation_key_id` (zero↔zero fallback, analogous to
    /// `decode_data_datawriter_submessage`).
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_serialized_payload_with(
        &self,
        handle: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, encoded);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement decode_serialized_payload_with",
        ))
    }

    /// Like [`Self::decode_serialized_payload_with`], but opens the
    /// SerializedPayload with the **Kx/participant key material** of the handle
    /// (BuiltinParticipantVolatileMessageSecure key, §10.5.2.1.2 Tab. 73) instead
    /// of the per-endpoint DataWriter key. Needed for vendors (cyclone) that
    /// encrypt the data_protection payload with the SharedSecret-derived participant
    /// key (transformation_key_id=0) instead of the datawriter key.
    ///
    /// # Errors
    /// `NotImplemented` (default) or `BadArgument`/`CryptoFailed`.
    fn decode_serialized_payload_kx(
        &self,
        handle: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = (handle, encoded);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement decode_serialized_payload_kx",
        ))
    }

    /// Plugin class id (e.g. "DDS:Crypto:AES-GCM-GMAC:1.2").
    fn plugin_class_id(&self) -> &str;
}

/// Factory alias.
pub type CryptoPluginBox = Box<dyn CryptographicPlugin>;
