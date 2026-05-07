// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cryptographic-Plugin SPI (OMG DDS-Security 1.1 §8.5).
//!
//! Das SPI ist in drei Sub-Interfaces aufgeteilt (Spec-Struktur):
//! * **KeyFactory** (§8.5.1.7) — Key-Derivation aus `SharedSecret`.
//! * **KeyExchange** (§8.5.1.8) — Key-Tokens zwischen Peers austauschen.
//! * **Transform** (§8.5.1.9) — konkrete Encrypt/Decrypt/Sign/Verify.
//!
//! Wir bundeln die drei in einem Trait (`CryptographicPlugin`), damit
//! Nutzer nur einen `Box<dyn ...>` halten muessen. Backend-Impls
//! (rustls, ring, mbedtls) implementieren alle drei Sub-Interfaces.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-SPI benötigt `Box<dyn CryptographicPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::authentication::{IdentityHandle, SharedSecretHandle};
use crate::error::SecurityResult;

/// Opaker Handle fuer ein abgeleitetes Schluesselmaterial (Master-Key
/// eines Participants/Endpoints).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CryptoHandle(pub u64);

/// Receiver-Specific-MAC (Spec §7.3.6.3 `ReceiverSpecificMAC`).
///
/// Wenn ein Sender ein Ciphertext an N Receiver mit **gleicher Suite
/// aber unterschiedlichen Keys** schickt, wird pro Receiver ein
/// 16-byte Truncated-HMAC berechnet. Die Wire-Repraesentation ist
/// eine Sequenz von `(key_id, mac)`-Paaren im SEC_POSTFIX.
///
/// `key_id` ist die Spec-konforme 4-Byte-ID (typisch low-32-bits des
/// Sender-seitigen [`CryptoHandle`] fuer diesen Receiver), anhand
/// derer der Empfaenger seinen spezifischen MAC-Eintrag findet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReceiverMac {
    /// 4-byte `CryptoTransformKeyId` aus §7.3.6.3.
    pub key_id: u32,
    /// 16-byte Truncated-HMAC-SHA256 ueber den Ciphertext.
    pub mac: [u8; 16],
}

impl ReceiverMac {
    /// Wire-Size eines einzelnen `ReceiverMac`-Eintrags (Spec §7.3.6.3).
    pub const WIRE_SIZE: usize = 4 + 16;
}

/// Cryptographic-Plugin (Spec §8.5.1). In v1.3 ist das ein reines
/// **Interface** — Produktions-Impls leben in `zerodds-security-crypto`
/// (AES-GCM + HMAC), `zerodds-security-keyexchange` (DH-Keyexchange,
/// Spec §9.5.3) und `zerodds-security-rtps` (RTPS-Header-AAD-Wrapper,
/// Spec §7.3.5).
pub trait CryptographicPlugin: Send + Sync {
    // -------- KeyFactory (§8.5.1.7) --------

    /// Erzeugt Participant-Crypto-Material aus dem Handshake-
    /// SharedSecret.
    fn register_local_participant(
        &mut self,
        identity: IdentityHandle,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle>;

    /// Erzeugt Crypto-Material fuer einen Remote-Participant.
    fn register_matched_remote_participant(
        &mut self,
        local: CryptoHandle,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle>;

    /// Erzeugt Crypto-Material fuer einen lokalen DataWriter/Reader.
    fn register_local_endpoint(
        &mut self,
        participant: CryptoHandle,
        is_writer: bool,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle>;

    // -------- KeyExchange (§8.5.1.8) --------

    /// Erzeugt das `ParticipantCryptoTokens`-Blob, das an den Remote-
    /// Participant gesendet wird (enthaelt verschluesseltes
    /// Key-Material).
    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>>;

    /// Verarbeitet die Tokens vom Remote-Participant. Danach sind die
    /// Keys fuer encrypted Submessages wechselseitig bekannt.
    fn set_remote_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()>;

    // -------- Transform (§8.5.1.9) --------

    /// Encrypt + Sign einer RTPS-Submessage. Input: plain submessage
    /// bytes. Output: `SecureSubmessage`-Payload (ciphertext + tag).
    ///
    /// `aad_extension` ist die Spec-konforme AAD-Extension (Spec §10.5.2
    /// Tab.78). Submessage-Protection (§8.5.1.9.2) liefert hier
    /// `SubmessageHeader || SecureSubmessageHeader`-Bytes;
    /// RTPS-Message-Protection (§8.5.1.9.7) den RTPS-Header (20 Byte) +
    /// SecureRTPSSubmessageHeader. Leer (`&[]`) ist nur spec-konform
    /// wenn der Caller explizit Spec-§8.1 Tab.78 ohne Header-Coverage
    /// akzeptiert (z.B. Pre-Shared-Key-Pfad ohne Header-Auth).
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
    /// muss byte-identisch zur Sender-AAD sein (sonst Tag-Mismatch).
    ///
    /// Spec §8.5.1.9.4 `decode_serialized_payload`.
    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>>;

    /// Encrypt+Sign mit `Receiver-Specific-MACs` (Spec
    /// §7.3.6.3). Produziert **einen** Ciphertext (Sender-Key) plus
    /// pro Remote einen 16-byte Truncated-HMAC.
    ///
    /// Die `receivers`-Liste enthaelt pro Empfaenger `(handle,
    /// key_id)`:
    /// * `handle` — CryptoHandle auf den MAC-Key im Plugin-Slot
    ///   (typisch der aus `register_matched_remote_participant`
    ///   abgeleitete Per-Peer-Key).
    /// * `key_id` — 4-byte Wire-Identifier, den der Empfaenger in
    ///   der MAC-Liste sucht (muss zwischen Sender und Empfaenger
    ///   synchronisiert sein, typisch low-32-bits des Peer-GuidPrefix).
    ///
    /// Default-Impl: faellt auf `encrypt_submessage` zurueck und
    /// liefert leere MAC-Liste — Plugins ohne Multi-MAC-Support
    /// signalisieren dem Caller damit "bitte Multi-Cipher-Fan-Out
    /// nutzen".
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
    /// `own_key_id` ist die Wire-Id, unter der der Empfaenger in der
    /// MAC-Liste zu finden ist; `own_mac_key_handle` ist der Slot mit
    /// dem dazugehoerigen HMAC-Key.
    ///
    /// Wenn `macs.is_empty()` wird auf [`Self::decrypt_submessage`]
    /// delegiert (Backward-Compat).
    ///
    /// # Errors
    /// * `CryptoFailed` wenn kein MAC-Eintrag zur `own_key_id`
    ///   passt oder der MAC-Vergleich fehlschlaegt.
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

    /// Plugin-Class-Id (z.B. "DDS:Crypto:AES-GCM-GMAC:1.2").
    fn plugin_class_id(&self) -> &str;
}

/// Factory-Alias.
pub type CryptoPluginBox = Box<dyn CryptographicPlugin>;
