// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cryptographic plugin SPI (OMG DDS-Security 1.2 §9.5).
//!
//! The SPI is split into three sub-interfaces (spec structure):
//! * **KeyFactory** (§9.5.1.8) — key derivation from `SharedSecret`.
//! * **KeyExchange** (§9.5.1.9) — exchange key tokens between peers.
//! * **Transform** (§9.5.1.10) — the concrete encrypt/decrypt/sign/verify.
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

/// The property of [`CryptographicPlugin::register_local_participant`]
/// that the RTPS Protection Kind of the domain specifies origin
/// authentication (`"true"`; DDS-Security 1.2 §10.4.1.2.5.7,
/// `register_matched_remote_participant`): the key material to a remote
/// participant then carries a receiver-specific key and the protected
/// RTPS messages a MAC for it.
pub const PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION: &str =
    "org.zerodds.crypto.rtps_origin_authentication";

/// The property of [`CryptographicPlugin::register_local_endpoint`] with
/// the Metadata Protection Kind of the endpoint (DDS-Security 1.2
/// §10.4.1.2.6.6; for a secure builtin endpoint the discovery or liveliness
/// protection kind of the domain).
pub const ENDPOINT_METADATA_PROTECTION_KIND: &str = "org.zerodds.crypto.metadata_protection_kind";

/// The property of [`CryptographicPlugin::register_local_endpoint`] with
/// the Data Protection Kind of the endpoint (DDS-Security 1.2 §10.4.1.2.6.7).
pub const ENDPOINT_DATA_PROTECTION_KIND: &str = "org.zerodds.crypto.data_protection_kind";

/// Cryptographic plugin (spec §9.5.1). In v1.3 this is a pure
/// **interface** — production impls live in `zerodds-security-crypto`
/// (AES-GCM + HMAC), `zerodds-security-keyexchange` (DH key exchange,
/// spec §9.5.3) and `zerodds-security-rtps` (RTPS header AAD wrapper,
/// spec §7.3.5).
pub trait CryptographicPlugin: Send + Sync {
    // -------- KeyFactory (§9.5.1.8) --------

    /// Optional second (payload) token of a local datawriter endpoint
    /// whose submessages and payload have keys of their own (Cyclone DDS
    /// `writer_key_material_message` + `writer_key_material_payload`).
    /// `None` = one key.
    fn endpoint_payload_token(&self, _handle: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        None
    }

    /// The datawriter token for a relay-only reader (DDS-Security 1.2
    /// §9.5.1.9.5): "only the cryptographic material needed to verify and
    /// decode the RTPS SubMessage but not the CryptoContent". `None` when the
    /// writer has no key that protects its submessages alone; the relay then
    /// gets no key material, never the payload key. Default `None`.
    fn endpoint_relay_token(&self, _handle: CryptoHandle) -> Option<alloc::vec::Vec<u8>> {
        None
    }

    /// Releases the key material of a local endpoint (DDS-Security 1.2
    /// §9.5.1.8 `unregister_local_endpoint`): its handle is invalid after
    /// this. Default: nothing to release.
    ///
    /// # Errors
    /// Plugin-specific.
    fn unregister_local_endpoint(&mut self, _handle: CryptoHandle) -> SecurityResult<()> {
        Ok(())
    }

    /// Releases what the plugin holds for a matched remote participant: its
    /// slot, its Kx key, and the keys of its endpoints (§9.5.1.8
    /// `unregister_matched_remote_participant` and
    /// `unregister_matched_remote_endpoint`). Default: nothing to release.
    ///
    /// # Errors
    /// Plugin-specific.
    fn unregister_matched_remote_participant(
        &mut self,
        _handle: CryptoHandle,
    ) -> SecurityResult<()> {
        Ok(())
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
    ///
    /// `properties` may carry the protection kinds of the endpoint,
    /// [`ENDPOINT_METADATA_PROTECTION_KIND`] and
    /// [`ENDPOINT_DATA_PROTECTION_KIND`], in the spelling of the governance
    /// (`NONE`, `SIGN`, `ENCRYPT`, `…_WITH_ORIGIN_AUTHENTICATION`). They stand
    /// for the `EndpointSecurityAttributes` of `register_local_datawriter`
    /// and `register_local_datareader` (DDS-Security 1.2 §8.5.1.7.2,
    /// §8.5.1.7.3): the key material follows the Metadata and the Data
    /// Protection Kind of the endpoint (§10.5.3.1 Table 78).
    fn register_local_endpoint(
        &mut self,
        participant: CryptoHandle,
        is_writer: bool,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle>;

    // -------- KeyExchange (§9.5.1.9) --------

    /// Creates the `ParticipantCryptoTokens` blob that is sent to the
    /// remote participant (contains encrypted
    /// key material).
    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>>;

    /// The token of the local endpoint `endpoint` for the remote endpoint
    /// `remote_endpoint` (its GUID, 16 octets): `Writer2ReaderKeyMaterial` or
    /// `Reader2WriterKeyMaterial` (DDS-Security 1.2 §10.5.3.1,
    /// `register_matched_remote_datareader` / `_datawriter`). When the
    /// Metadata Protection Kind of the endpoint specifies origin
    /// authentication, the key material carries a `receiver_specific_key_id`
    /// and a `master_receiver_specific_key` of this one remote endpoint, and
    /// the endpoint's submessages carry a receiver-specific MAC for it.
    /// Without origin authentication this is the token of the endpoint.
    ///
    /// # Errors
    /// Unknown handle, or the key could not be made.
    fn create_endpoint_crypto_token_for(
        &mut self,
        endpoint: CryptoHandle,
        remote_endpoint: [u8; 16],
    ) -> SecurityResult<Vec<u8>> {
        let _ = remote_endpoint;
        self.create_local_participant_crypto_tokens(endpoint, CryptoHandle(0))
    }

    /// Processes the tokens from the remote participant. Afterwards the
    /// keys for encrypted submessages are mutually known.
    fn set_remote_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()>;

    /// Processes the DataWriter or DataReader crypto tokens of an endpoint
    /// of the remote participant `remote` (DDS-Security 1.2 §8.5.1.7.4/.5).
    /// The key is the sender's: a `transformation_key_id` is chosen by its
    /// sender (§9.5.2.1.1) and identifies a key only together with it.
    /// Default: [`Self::set_remote_participant_crypto_tokens`] with
    /// `CryptoHandle(0)`, as before.
    ///
    /// # Errors
    /// As [`Self::set_remote_participant_crypto_tokens`].
    fn set_remote_endpoint_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        let _ = remote;
        self.set_remote_participant_crypto_tokens(local, CryptoHandle(0), tokens)
    }

    // -------- Transform (§9.5.1.10) --------

    /// Encrypt + sign an RTPS submessage. Input: plain submessage
    /// bytes. Output: `SecureSubmessage` payload (ciphertext + tag).
    ///
    /// `aad_extension` is the spec-conformant AAD extension (spec §10.5.2
    /// Tab.78). Submessage protection (§9.5.1.10.2) provides
    /// `SubmessageHeader || SecureSubmessageHeader` bytes here;
    /// RTPS message protection (§8.5.1.9.7) the RTPS header (20 bytes) +
    /// SecureRTPSSubmessageHeader. Empty (`&[]`) is only spec-conformant
    /// if the caller explicitly accepts spec §8.1 Tab.78 without header
    /// coverage (e.g. pre-shared-key path without header auth).
    ///
    /// Spec §9.5.1.10.1 `encode_serialized_payload`.
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
    /// Spec §9.5.1.10.9 `decode_serialized_payload`.
    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    // -------- KeyExchange channel protection (§10.5.3.2) --------
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
    /// §10.5.3): encodes `plaintext` as a `SEC_PREFIX` + `SEC_BODY` + `SEC_POSTFIX`
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
    /// protection (`metadata_protection_kind=ENCRYPT`, §10.5.3.3). Wire-identical
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
    /// CryptoHeader (DDS-Security §10.5.2.3) — finds the matching remote
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

    /// [`Self::decode_data_by_key_id`] with the key of the remote
    /// participant `remote`, the sender (DDS-Security 1.2 §10.5.3.3.1: the
    /// receiver uses the sender's key material). A key of another
    /// participant does not decode the submessage. Default: the key-id
    /// lookup of [`Self::decode_data_by_key_id`].
    ///
    /// # Errors
    /// As [`Self::decode_data_by_key_id`].
    fn decode_data_by_key_id_from(
        &self,
        remote: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = remote;
        self.decode_data_by_key_id(wire)
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

    /// [`Self::decode_rtps_message_cyclone`] with the key of the remote
    /// participant `remote`, the sender. Default: the key-id lookup.
    ///
    /// # Errors
    /// As [`Self::decode_rtps_message_cyclone`].
    fn decode_rtps_message_cyclone_from(
        &self,
        remote: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = remote;
        self.decode_rtps_message_cyclone(message)
    }

    /// `encode_rtps_message` with `transform_with_psk` TRUE (DDS-Security 1.2
    /// §10.9.3.3.3 Table 89): the message protected with the pre-shared key
    /// of the participant `local`, the flags A and P set. Default: not
    /// implemented (the plugin has no pre-shared key).
    ///
    /// # Errors
    /// `NotImplemented` by default; implementation-specific.
    fn encode_rtps_message_psk(
        &self,
        local: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<alloc::vec::Vec<u8>> {
        let _ = (local, message);
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement pre-shared-key SRTPS encode",
        ))
    }

    /// `decode_rtps_message` with `transform_with_psk` TRUE (Table 89).
    ///
    /// # Errors
    /// `NotImplemented` by default; implementation-specific.
    fn decode_rtps_message_psk(&self, message: &[u8]) -> SecurityResult<alloc::vec::Vec<u8>> {
        let _ = message;
        Err(crate::error::SecurityError::new(
            crate::error::SecurityErrorKind::NotImplemented,
            "plugin does not implement pre-shared-key SRTPS decode",
        ))
    }

    /// §9.5.1.10.1 / §10.5.3.3.4.4 `encode_serialized_payload` (data_protection,
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

    /// §9.5.1.10.9 / §10.5.3.3.4.4 `decode_serialized_payload`; key via the
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

    /// [`Self::decode_serialized_payload`] with the key of the remote
    /// participant `remote`, the writer's. Default: the key-id lookup.
    ///
    /// # Errors
    /// As [`Self::decode_serialized_payload`].
    fn decode_serialized_payload_by_key_id_from(
        &self,
        remote: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let _ = remote;
        self.decode_serialized_payload(encoded)
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
