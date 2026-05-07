// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Thread-safe Wrapper um [`SecurityGate`] fuer Multi-Thread-Nutzung.
//!
//! Die referenz-basierte `SecurityGate<'c, P>` ist fuer Einzel-Thread-
//! Nutzung gedacht — mehrere Runtime-Threads (SPDP-Loop, User-RX,
//! Event-Loop) brauchen aber synchronisierten Zugriff.
//!
//! [`SharedSecurityGate`] kapselt:
//! * `Governance` (immutable pro Teilnehmer — Clone-bar).
//! * `Box<dyn CryptographicPlugin>` (mutable beim Key-Registrieren).
//! * Cache des lokalen CryptoHandles.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin wird via `Box<dyn CryptographicPlugin>` gehalten, damit
//! der Nutzer das Backend frei waehlen kann.)

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex, PoisonError};

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security_permissions::{Governance, ProtectionKind};
use zerodds_security_rtps::{
    RTPS_HEADER_LEN, SRTPS_PREFIX, decode_secured_rtps_message, decode_secured_submessage_multi,
    encode_secured_rtps_message, encode_secured_submessage_multi,
};

use crate::gate::SecurityGateError;
use crate::policy::{NetInterface, ProtectionLevel};

// ============================================================================
// Inbound-Verdict
// ============================================================================

/// Ergebnis einer `classify_inbound`-Entscheidung.
///
/// Die Enum-Varianten trennen die moeglichen Gruende sauber, damit der
/// Caller (dcps-Runtime) pro Grund einen passenden `LogLevel` an das
/// [`zerodds_security::logging::LoggingPlugin`] weiterreichen kann.
///
/// Der Interface-Kontext (`NetInterface`) wird vom Caller mit
/// uebergeben und findet sich in [`InboundVerdict::iface`] wieder —
/// damit Log-Events pro Interface attributierbar sind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundVerdict {
    /// Paket ist zulaessig — `bytes` ist das dekodierte RTPS-Datagram,
    /// das an den SPDP/SEDP/User-Dispatch weitergegeben wird.
    Accept(Vec<u8>),
    /// Paket ist zu kurz fuer einen RTPS-Header (< 20 Byte). Das ist
    /// eigentlich ein Transport-Bug oder ein Fuzz-Probe. Severity
    /// sollte `Error` sein.
    Malformed,
    /// Paket kam von einem **unauth-Peer auf einer Domain, die
    /// Authentication verlangt** (`allow_unauthenticated_participants =
    /// false`). Severity sollte `Error` sein.
    LegacyBlocked,
    /// Policy-Violation: die Domain verlangt Protection, das Paket
    /// ist aber plain (oder umgekehrt). Severity sollte `Warning`
    /// sein — ggf. Tampering-Versuch.
    PolicyViolation(String),
    /// Cryptographischer Fehler beim Unwrap (Tag-Mismatch, falscher
    /// Key, replay-Attack etc.). Severity `Warning`.
    CryptoError(String),
}

impl InboundVerdict {
    /// Kurzform: `true` wenn Paket weitergegeben wird.
    #[must_use]
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::Accept(_))
    }

    /// Log-Kategorie (OMG §8.6.3) — freier String, der den Drop-Grund
    /// identifiziert. Hilfreich wenn ein LoggingPlugin nach Kategorien
    /// filtert.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::Accept(_) => "inbound.accept",
            Self::Malformed => "inbound.malformed",
            Self::LegacyBlocked => "inbound.legacy_blocked",
            Self::PolicyViolation(_) => "inbound.policy_violation",
            Self::CryptoError(_) => "inbound.crypto_error",
        }
    }
}

/// Opaker Peer-Identifier. In RTPS-Umgebungen mappt der Caller typisch
/// `GuidPrefix` (12 byte) darauf — `[u8; 12]` passt genau.
pub type PeerKey = [u8; 12];

/// Thread-sicherer Security-Gate. Clone gibt eine zweite Referenz auf
/// die gleiche Plugin-Instance — alle Clones operieren auf gleichem
/// Key-Store.
#[derive(Clone)]
pub struct SharedSecurityGate {
    inner: Arc<Mutex<GateInner>>,
}

impl core::fmt::Debug for SharedSecurityGate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Plugin + Keys niemals in Debug-Output — nur Metadaten.
        match self.inner.lock() {
            Ok(g) => write!(
                f,
                "SharedSecurityGate {{ domain_id: {}, peers: {}, local_registered: {} }}",
                g.domain_id,
                g.peers.len(),
                g.local.is_some()
            ),
            Err(_) => write!(f, "SharedSecurityGate {{ <poisoned> }}"),
        }
    }
}

struct GateInner {
    domain_id: u32,
    governance: Governance,
    crypto: Box<dyn CryptographicPlugin>,
    local: Option<CryptoHandle>,
    /// Peer-Key → CryptoHandle-Mapping. Wird von
    /// `register_remote_by_guid` gefuellt; `transform_inbound_from`
    /// schaut hier nach.
    peers: BTreeMap<PeerKey, CryptoHandle>,
}

/// Leitet eine deterministische 4-Byte `key_id` aus einem
/// 12-Byte [`PeerKey`] (GuidPrefix) ab. Beide Kommunikationspartner
/// berechnen sie identisch ohne Handshake, d.h. die `key_id` im
/// Wire-Format-SEC_POSTFIX ist plattformuebergreifend
/// synchronisierbar.
///
/// Verwendung: Low-32-bits der ersten 4 Bytes des GuidPrefix.
#[must_use]
pub fn peer_key_to_id(pk: &PeerKey) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&pk[..4]);
    u32::from_le_bytes(buf)
}

fn poisoned<T>(_: PoisonError<T>) -> SecurityGateError {
    SecurityGateError::Crypto(zerodds_security::error::SecurityError::new(
        zerodds_security::error::SecurityErrorKind::Internal,
        "security-runtime: mutex poisoned",
    ))
}

impl SharedSecurityGate {
    /// Konstruktor. Der Plugin wird vom Gate **geownt** (Box).
    #[must_use]
    pub fn new(
        domain_id: u32,
        governance: Governance,
        crypto: Box<dyn CryptographicPlugin>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GateInner {
                domain_id,
                governance,
                crypto,
                local: None,
                peers: BTreeMap::new(),
            })),
        }
    }

    fn with_inner<R>(
        &self,
        f: impl FnOnce(&mut GateInner) -> Result<R, SecurityGateError>,
    ) -> Result<R, SecurityGateError> {
        let mut g = self.inner.lock().map_err(poisoned)?;
        f(&mut g)
    }

    /// Gibt die Domain-Id zurueck, fuer die der Gate laeuft.
    pub fn domain_id(&self) -> Result<u32, SecurityGateError> {
        self.with_inner(|g| Ok(g.domain_id))
    }

    /// `ProtectionKind` fuer Message-Level — abgeleitet aus dem ersten
    /// matchenden `<domain_rule>`.
    pub fn message_protection(&self) -> Result<ProtectionKind, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.rtps_protection_kind)
                .unwrap_or(ProtectionKind::None))
        })
    }

    /// Registriert den lokalen Participant beim Crypto-Plugin (idempotent).
    ///
    /// # Errors
    /// `CryptoSetup` wenn das Plugin den Identity-Handle nicht akzeptiert.
    pub fn ensure_local(&self) -> Result<CryptoHandle, SecurityGateError> {
        self.with_inner(|g| {
            if let Some(h) = g.local {
                return Ok(h);
            }
            let h = g
                .crypto
                .register_local_participant(IdentityHandle(1), &[])
                .map_err(SecurityGateError::CryptoSetup)?;
            g.local = Some(h);
            Ok(h)
        })
    }

    /// Token des lokalen Participants (zu senden an Remote via SEDP).
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn local_token(&self) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            g.crypto
                .create_local_participant_crypto_tokens(local, CryptoHandle(0))
                .map_err(SecurityGateError::Crypto)
        })
    }

    /// Registriert einen Remote-Peer und installiert seinen Token.
    /// Der zurueckgegebene `CryptoHandle` identifiziert den Slot, in
    /// dem der Remote-Key abgelegt ist — wird bei `decode_inbound_message`
    /// wieder gebraucht.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn register_remote_with_token(
        &self,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
        token: &[u8],
    ) -> Result<CryptoHandle, SecurityGateError> {
        let local = self.ensure_local()?;
        self.with_inner(|g| {
            let slot = g
                .crypto
                .register_matched_remote_participant(local, remote_identity, shared_secret)
                .map_err(SecurityGateError::CryptoSetup)?;
            g.crypto
                .set_remote_participant_crypto_tokens(local, slot, token)
                .map_err(SecurityGateError::Crypto)?;
            Ok(slot)
        })
    }

    /// Registriert einen Remote-Peer **mit Peer-Key-Mapping**. Nach
    /// diesem Call kann [`Self::transform_inbound_from`] den Peer
    /// anhand seines [`PeerKey`] (GuidPrefix) wiederfinden.
    ///
    /// Idempotent: wiederholter Call mit gleichem Key ueberschreibt den
    /// alten Slot nicht — der Caller muss explizit
    /// [`Self::forget_remote`] aufrufen, um rotieren zu koennen.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn register_remote_by_guid(
        &self,
        peer_key: PeerKey,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
        token: &[u8],
    ) -> Result<CryptoHandle, SecurityGateError> {
        // Idempotenz-Check: wenn schon registriert, die existierende
        // Handle zurueckgeben.
        {
            let g = self.inner.lock().map_err(poisoned)?;
            if let Some(h) = g.peers.get(&peer_key) {
                return Ok(*h);
            }
        }
        let slot = self.register_remote_with_token(remote_identity, shared_secret, token)?;
        self.with_inner(|g| {
            g.peers.insert(peer_key, slot);
            Ok(())
        })?;
        Ok(slot)
    }

    /// Entfernt die Peer-Key → Slot-Zuordnung. Der Slot selbst bleibt
    /// im Plugin (Key-Cleanup ist Aufgabe des Plugins bei Re-Register).
    pub fn forget_remote(&self, peer_key: &PeerKey) -> Result<(), SecurityGateError> {
        self.with_inner(|g| {
            g.peers.remove(peer_key);
            Ok(())
        })
    }

    /// Liefert den Slot fuer einen Peer-Key, falls registriert.
    pub fn slot_for(&self, peer_key: &PeerKey) -> Result<Option<CryptoHandle>, SecurityGateError> {
        self.with_inner(|g| Ok(g.peers.get(peer_key).copied()))
    }

    /// Inbound-Transform mit **Peer-Key-Lookup**. Der RTPS-Header
    /// enthaelt den GuidPrefix des Senders auf den Bytes 8..20 — der
    /// Caller muss diesen hier als `peer_key` uebergeben.
    ///
    /// Wenn der Peer nicht registriert ist **und** die Message
    /// verschluesselt aussieht: `PolicyViolation` (unbekannter Sender
    /// schickt SRTPS).
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn transform_inbound_from(
        &self,
        peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let looks_secured = wire.len() > RTPS_HEADER_LEN && wire[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = self.message_protection()?;
        if !looks_secured {
            // Passthrough oder Policy-Violation — gleiche Logik wie im
            // slot-basierten Pfad.
            return if matches!(kind, ProtectionKind::None) {
                Ok(wire.to_vec())
            } else {
                Err(SecurityGateError::PolicyViolation(alloc::format!(
                    "domain verlangt {kind:?}, bekam plain-rtps-message"
                )))
            };
        }
        let slot = self.slot_for(peer_key)?.ok_or_else(|| {
            SecurityGateError::PolicyViolation(alloc::format!(
                "unbekannter peer {peer_key:?} sendet SRTPS_PREFIX"
            ))
        })?;
        self.transform_inbound(slot, wire)
    }

    /// Outbound-Message: wrap wenn Governance Message-Schutz verlangt.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn transform_outbound(&self, message: &[u8]) -> Result<Vec<u8>, SecurityGateError> {
        match self.message_protection()? {
            ProtectionKind::None => Ok(message.to_vec()),
            _ => {
                let local = self.ensure_local()?;
                self.with_inner(|g| {
                    encode_secured_rtps_message(&*g.crypto, local, &[], message)
                        .map_err(SecurityGateError::from)
                })
            }
        }
    }

    /// Group-Outbound mit Receiver-Specific-MACs.
    ///
    /// Nutzt [`encode_secured_submessage_multi`], wenn alle Empfaenger
    /// bereits ueber Peer-Keys im Gate registriert sind. Liefert einen
    /// **einzigen** Wire-Datagram mit Multi-MAC-SEC_POSTFIX; der
    /// Caller kann dasselbe Datagram an alle matched Readers unicasten.
    ///
    /// Der resultierende Wire ist KEIN RTPS-Message-Level-Wrapper —
    /// es ist eine Submessage-Sequenz (SEC_PREFIX/BODY/POSTFIX). Der
    /// Caller muss den RTPS-Header selber davor setzen oder die
    /// `transform_outbound_multi_wrapped`-Variante nutzen (folgt bei
    /// Bedarf — fuer Stufe 7 reicht die Submessage-Sequenz).
    ///
    /// # Errors
    /// * `Crypto` durchgereicht.
    /// * `PolicyViolation` wenn ein Peer-Key nicht registriert ist.
    pub fn transform_outbound_group(
        &self,
        peer_keys: &[PeerKey],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        // Resolve alle PeerKeys zu (CryptoHandle, key_id)-Paaren. Die
        // key_id leiten wir deterministisch aus dem PeerKey-Prefix ab
        // (low-32-bits des GuidPrefix) — beide Seiten (Sender +
        // Empfaenger) koennen sie ohne zusaetzlichen Handshake
        // berechnen. Caller muss vorher via register_remote_by_guid
        // pro PeerKey einen Slot angelegt haben.
        let bindings: Vec<(CryptoHandle, u32)> = self.with_inner(|g| {
            let mut out = Vec::with_capacity(peer_keys.len());
            for pk in peer_keys {
                let h = g.peers.get(pk).copied().ok_or_else(|| {
                    SecurityGateError::PolicyViolation(alloc::format!(
                        "transform_outbound_group: peer {pk:?} not registered"
                    ))
                })?;
                out.push((h, peer_key_to_id(pk)));
            }
            Ok(out)
        })?;
        self.with_inner(|g| {
            encode_secured_submessage_multi(&*g.crypto, local, &bindings, plaintext)
                .map_err(SecurityGateError::from)
        })
    }

    /// Group-Inbound: dekodiert eine Multi-MAC-Submessage-Sequenz und
    /// validiert **den eigenen** MAC.
    ///
    /// `sender_peer_key` ist der GuidPrefix des Senders (wie in
    /// [`Self::register_remote_by_guid`] registriert). Der Empfaenger-
    /// MAC-Key kommt aus `ensure_local()` — der Sender hat beim
    /// Encoding genau diesen Slot-Handle als `remote_list`-Eintrag
    /// gesetzt (via `register_remote_by_guid(our_prefix, our_local_token)`).
    ///
    /// # Errors
    /// * `PolicyViolation` wenn `sender_peer_key` nicht registriert ist.
    /// * `Crypto` / `Wrapper` bei Tag-/MAC-Mismatch.
    pub fn transform_inbound_group(
        &self,
        sender_peer_key: &PeerKey,
        own_peer_key: &PeerKey,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let sender_slot = self.slot_for(sender_peer_key)?.ok_or_else(|| {
            SecurityGateError::PolicyViolation(alloc::format!(
                "transform_inbound_group: unknown sender {sender_peer_key:?}"
            ))
        })?;
        // Die MAC-Key-Slot-Quelle ist unser eigener local-Slot (der
        // Sender hat bei der Registrierung unseres PeerKeys genau
        // unseren local_token abgespeichert → gleicher Master-Key).
        let own_local = self.ensure_local()?;
        let own_id = peer_key_to_id(own_peer_key);
        self.with_inner(|g| {
            decode_secured_submessage_multi(
                &*g.crypto,
                sender_slot,
                sender_slot,
                own_id,
                own_local,
                wire,
            )
            .map_err(SecurityGateError::from)
        })
    }

    /// Peer-spezifische Outbound-Transform.
    ///
    /// Im Gegensatz zu [`Self::transform_outbound`] ignoriert diese
    /// Methode die Domain-Rule und wendet **stattdessen** das
    /// Caller-gegebene [`ProtectionLevel`] an. Das ist die API, die
    /// der heterogeneous-Security-Writer-Tick (Per-Reader-Serializer)
    /// pro matched Reader aufruft.
    ///
    /// * `ProtectionLevel::None`    → plaintext passthrough
    /// * `ProtectionLevel::Sign`    → RTPS-Message-wrap (HMAC/GCM-Tag)
    /// * `ProtectionLevel::Encrypt` → RTPS-Message-wrap (AEAD-Ciphertext)
    ///
    /// Die Sign/Encrypt-Unterscheidung nutzt heute denselben Encoder
    /// wie [`Self::transform_outbound`] — das aktuelle
    /// `AesGcmCryptoPlugin` differenziert nicht. Receiver-Specific-MACs
    /// und weitere Erweiterungen rüsten die Unterscheidung nach. Der
    /// `peer_key` wird mitgefuehrt (fuer zukuenftige pro-Peer-Crypto-
    /// Handles), aber noch nicht an den Plugin weitergereicht.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`]. Im `None`-Pfad nie ein Fehler.
    pub fn transform_outbound_for(
        &self,
        _peer_key: &PeerKey,
        message: &[u8],
        level: ProtectionLevel,
    ) -> Result<Vec<u8>, SecurityGateError> {
        match level {
            ProtectionLevel::None => Ok(message.to_vec()),
            ProtectionLevel::Sign | ProtectionLevel::Encrypt => {
                let local = self.ensure_local()?;
                self.with_inner(|g| {
                    encode_secured_rtps_message(&*g.crypto, local, &[], message)
                        .map_err(SecurityGateError::from)
                })
            }
        }
    }

    /// Liefert das `allow_unauthenticated_participants`-Flag aus der
    /// Domain-Rule. Default `false` wenn keine Rule fuer die Domain
    /// existiert — konservativ-sichere Haltung.
    pub fn allow_unauthenticated(&self) -> Result<bool, SecurityGateError> {
        self.with_inner(|g| {
            Ok(g.governance
                .find_domain_rule(g.domain_id)
                .map(|r| r.allow_unauthenticated_participants)
                .unwrap_or(false))
        })
    }

    /// Klassifiziert ein eingehendes RTPS-Datagram gegen Domain-Rule,
    /// Peer-Registrierung, Wire-Format und Netzwerk-Interface
    ///.
    ///
    /// Entscheidungs-Matrix:
    ///
    /// 1. `bytes.len() < 20` → `Malformed`.
    /// 2. Extrahiere `peer_key` aus `bytes[8..20]`.
    /// 3. Wenn Paket SRTPS-gewrappt ist → Standard-Unwrap-Pfad
    ///    (`transform_inbound_from`). Bei Crypto-Fehler `CryptoError`,
    ///    bei unbekannter Peer `PolicyViolation`.
    /// 4. Wenn Paket plain ist UND Domain ProtectionKind::None verlangt
    ///    → `Accept`.
    /// 5. Wenn Paket plain ist UND Domain Protection verlangt:
    ///    * Interface ist `Loopback` oder `LocalHost` → `Accept`
    ///      (Bytes verlassen den Host-Kernel nicht — spec-konform
    ///      plaintext auf Host-local Transport)
    ///    * `allow_unauthenticated_participants = true` → `Accept`
    ///    * sonst → `LegacyBlocked`
    ///
    /// Der `iface`-Kontext wird derzeit in den Regeln nur fuer den
    /// Loopback-Fast-Path konsultiert; die feinere Peer-/Topic-
    /// Klassifizierung pro Interface uebernimmt die `PolicyEngine`
    /// ab Stufe 8 (Governance-XML `<interface_bindings>`).
    #[must_use]
    pub fn classify_inbound(&self, bytes: &[u8], iface: &NetInterface) -> InboundVerdict {
        if bytes.len() < RTPS_HEADER_LEN + 8 {
            return InboundVerdict::Malformed;
        }
        let mut peer_key = [0u8; 12];
        peer_key.copy_from_slice(&bytes[8..20]);

        let looks_secured = bytes.len() > RTPS_HEADER_LEN && bytes[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = match self.message_protection() {
            Ok(k) => k,
            Err(e) => {
                return InboundVerdict::CryptoError(alloc::format!("gate lookup failed: {e:?}"));
            }
        };

        if looks_secured {
            return match self.transform_inbound_from(&peer_key, bytes) {
                Ok(clear) => InboundVerdict::Accept(clear),
                Err(SecurityGateError::PolicyViolation(msg)) => {
                    InboundVerdict::PolicyViolation(msg)
                }
                Err(e) => InboundVerdict::CryptoError(alloc::format!("{e:?}")),
            };
        }

        // Plain-Paket kam rein.
        if matches!(kind, ProtectionKind::None) {
            return InboundVerdict::Accept(bytes.to_vec());
        }
        // Loopback / LocalHost: Bytes verlassen den Host nicht, also
        // ist plain fachlich OK — passt zum Arch-Doc §2.1 "Intra-
        // Host-Loopback: Plain (kein Netz verlassen)".
        if matches!(iface, NetInterface::Loopback | NetInterface::LocalHost) {
            return InboundVerdict::Accept(bytes.to_vec());
        }
        // Domain verlangt Schutz, Peer hat plain auf einem Remote-
        // Interface geschickt.
        match self.allow_unauthenticated() {
            Ok(true) => InboundVerdict::Accept(bytes.to_vec()),
            Ok(false) => InboundVerdict::LegacyBlocked,
            Err(e) => InboundVerdict::CryptoError(alloc::format!("gate lookup failed: {e:?}")),
        }
    }

    /// Inbound-Message: unwrap wenn SRTPS_PREFIX erkannt, sonst
    /// passthrough oder PolicyViolation.
    ///
    /// `remote_slot` zeigt auf den Slot, in dem der Sender-Key
    /// abgelegt ist (aus [`Self::register_remote_with_token`]).
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn transform_inbound(
        &self,
        remote_slot: CryptoHandle,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let looks_secured = wire.len() > RTPS_HEADER_LEN && wire[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = self.message_protection()?;
        match (kind, looks_secured) {
            (ProtectionKind::None, false) => Ok(wire.to_vec()),
            (_, true) => self.with_inner(|g| {
                decode_secured_rtps_message(&*g.crypto, remote_slot, remote_slot, wire)
                    .map_err(SecurityGateError::from)
            }),
            (_, false) => Err(SecurityGateError::PolicyViolation(alloc::format!(
                "domain verlangt {kind:?}, bekam plain-rtps-message"
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::thread;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    const GOV_RTPS: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    fn fake_msg(body: &[u8]) -> Vec<u8> {
        let mut m = Vec::with_capacity(20 + body.len());
        m.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        m.extend_from_slice(&[0u8; 12]);
        m.extend_from_slice(body);
        m
    }

    #[test]
    fn outbound_none_is_passthrough() {
        let gov = parse_governance_xml(
            r#"<domain_access_rules><domain_rule><domains><id>0</id></domains><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules>"#,
        )
        .unwrap();
        let gate = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));
        let msg = fake_msg(b"x");
        assert_eq!(gate.transform_outbound(&msg).unwrap(), msg);
    }

    #[test]
    fn e2e_alice_bob_with_shared_gate() {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let bob = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let alice_token = alice.local_token().unwrap();
        let bob_view_of_alice = bob
            .register_remote_with_token(IdentityHandle(1), SharedSecretHandle(1), &alice_token)
            .unwrap();

        let plain = fake_msg(b"[DATA:shared]");
        let wire = alice.transform_outbound(&plain).unwrap();
        let back = bob.transform_inbound(bob_view_of_alice, &wire).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn clone_shares_same_plugin_instance() {
        // Clone erzeugt einen zweiten Gate-Handle auf DAS SELBE Plugin.
        // `ensure_local` durch clone1 legt den local-Slot an; clone2
        // sieht dieselbe Session-ID.
        let gate1 = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let gate2 = gate1.clone();
        let t1 = gate1.local_token().unwrap();
        let t2 = gate2.local_token().unwrap();
        assert_eq!(t1, t2, "beide Clones sehen den gleichen lokalen Slot");
    }

    #[test]
    fn concurrent_transform_is_thread_safe() {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let mut handles = Vec::new();
        for i in 0..8 {
            let g = alice.clone();
            handles.push(thread::spawn(move || {
                let m = fake_msg(alloc::format!("[DATA:{i}]").as_bytes());
                // Muss OHNE Panic serialisieren — Nonce-Counter bleibt
                // thread-safe (AtomicU64 im KeyMaterial).
                let _ = g.transform_outbound(&m).unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn plain_inbound_on_protected_domain_is_policy_violation() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let plain = fake_msg(b"nope");
        let err = gate
            .transform_inbound(CryptoHandle(99), &plain)
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn domain_id_reflects_constructor() {
        let gate = SharedSecurityGate::new(
            7,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert_eq!(gate.domain_id().unwrap(), 7);
    }

    // -------------------------------------------------------------
    // RC1.4 Vorbereitung — Peer-Key-Mapping
    // -------------------------------------------------------------

    fn build_pair() -> (SharedSecurityGate, SharedSecurityGate) {
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let bob = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        (alice, bob)
    }

    #[test]
    fn register_remote_by_guid_is_idempotent() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let atoken = alice.local_token().unwrap();
        let slot1 = bob
            .register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &atoken,
            )
            .unwrap();
        let slot2 = bob
            .register_remote_by_guid(
                alice_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &atoken,
            )
            .unwrap();
        assert_eq!(
            slot1, slot2,
            "idempotent: gleicher guid-prefix → gleicher slot"
        );
    }

    #[test]
    fn transform_inbound_from_looks_up_slot_by_guid() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        let atoken = alice.local_token().unwrap();
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &atoken,
        )
        .unwrap();

        let msg = fake_msg(b"[DATA:guid-lookup]");
        let wire = alice.transform_outbound(&msg).unwrap();
        let back = bob.transform_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn transform_inbound_from_unknown_peer_is_policy_violation() {
        let (alice, bob) = build_pair();
        // Alice registriert sich NICHT bei Bob.
        let msg = fake_msg(b"x");
        let wire = alice.transform_outbound(&msg).unwrap();
        let err = bob.transform_inbound_from(&[0xCC; 12], &wire).unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn multi_peer_mapping_routes_correctly() {
        // Alice + Charlie senden an Bob. Bob muss die beiden per
        // GuidPrefix unterscheiden.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let charlie = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [1u8; 12];
        let charlie_prefix: PeerKey = [3u8; 12];

        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        bob.register_remote_by_guid(
            charlie_prefix,
            IdentityHandle(3),
            SharedSecretHandle(3),
            &charlie.local_token().unwrap(),
        )
        .unwrap();

        let m_alice = fake_msg(b"from-alice");
        let m_charlie = fake_msg(b"from-charlie");
        let w_alice = alice.transform_outbound(&m_alice).unwrap();
        let w_charlie = charlie.transform_outbound(&m_charlie).unwrap();

        assert_eq!(
            bob.transform_inbound_from(&alice_prefix, &w_alice).unwrap(),
            m_alice
        );
        assert_eq!(
            bob.transform_inbound_from(&charlie_prefix, &w_charlie)
                .unwrap(),
            m_charlie
        );
    }

    #[test]
    fn wrong_prefix_fails_tag_verify() {
        // Alice schickt, Bob dekodiert mit Charlie's Slot → Tag-
        // Mismatch.
        let (alice, bob) = build_pair();
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let charlie = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [1u8; 12];
        let charlie_prefix: PeerKey = [3u8; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        bob.register_remote_by_guid(
            charlie_prefix,
            IdentityHandle(3),
            SharedSecretHandle(3),
            &charlie.local_token().unwrap(),
        )
        .unwrap();

        let msg = fake_msg(b"from-alice");
        let wire = alice.transform_outbound(&msg).unwrap();
        // Bob dekodiert mit Charlie's Prefix → Crypto-Error.
        let err = bob
            .transform_inbound_from(&charlie_prefix, &wire)
            .unwrap_err();
        assert!(matches!(
            err,
            SecurityGateError::Wrapper(_) | SecurityGateError::Crypto(_)
        ));
    }

    // -------------------------------------------------------------
    // RC1 Stufe 7 — Receiver-Specific-MACs im Gate-E2E
    // -------------------------------------------------------------

    #[test]
    fn group_transform_one_ciphertext_three_macs_each_reader_decodes() {
        // DoD §Stufe 7 wortwoertlich: 1 Writer, 3 Reader gleiche Suite,
        // unterschiedliche Tokens → ein Ciphertext + 3 MACs; jeder
        // Reader validiert seinen eigenen MAC.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let charlie = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let dave = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let bob_prefix: PeerKey = [0xB1; 12];
        let charlie_prefix: PeerKey = [0xC1; 12];
        let dave_prefix: PeerKey = [0xD1; 12];
        let alice_prefix: PeerKey = [0xA1; 12];

        // Alice registriert alle drei Receiver mit ihren **eigenen**
        // Session-Keys (das ist das pro-Reader-SharedSecret).
        alice
            .register_remote_by_guid(
                bob_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &bob.local_token().unwrap(),
            )
            .unwrap();
        alice
            .register_remote_by_guid(
                charlie_prefix,
                IdentityHandle(2),
                SharedSecretHandle(2),
                &charlie.local_token().unwrap(),
            )
            .unwrap();
        alice
            .register_remote_by_guid(
                dave_prefix,
                IdentityHandle(3),
                SharedSecretHandle(3),
                &dave.local_token().unwrap(),
            )
            .unwrap();

        // Jeder Receiver registriert Alice unter ihrem Alice-Prefix —
        // damit `transform_inbound_group` die Sender-Seite findet
        // fuer den AES-GCM-Unwrap.
        for recv in [&bob, &charlie, &dave] {
            recv.register_remote_by_guid(
                alice_prefix,
                IdentityHandle(10),
                SharedSecretHandle(10),
                &alice.local_token().unwrap(),
            )
            .unwrap();
        }

        // Encode: 1 Ciphertext + 3 MACs.
        let plain = b"hetero-broadcast-e2e";
        let wire = alice
            .transform_outbound_group(&[bob_prefix, charlie_prefix, dave_prefix], plain)
            .unwrap();

        // Jeder Receiver decodiert seine Variante identisch — mit
        // seinem eigenen PeerKey als Match-ID.
        let out_bob = bob
            .transform_inbound_group(&alice_prefix, &bob_prefix, &wire)
            .unwrap();
        let out_charlie = charlie
            .transform_inbound_group(&alice_prefix, &charlie_prefix, &wire)
            .unwrap();
        let out_dave = dave
            .transform_inbound_group(&alice_prefix, &dave_prefix, &wire)
            .unwrap();
        assert_eq!(out_bob, plain);
        assert_eq!(out_charlie, plain);
        assert_eq!(out_dave, plain);
    }

    #[test]
    fn group_transform_rogue_receiver_without_mac_rejects() {
        // Ein 4. Receiver (Eve) ist bei Alice NICHT in der MAC-Liste.
        // Eve hat Alice's Token via Seitenkanal bekommen (oder
        // mitgelauscht) und versucht zu decodieren. Der eigene HMAC-
        // Key in Eve's `ensure_local()`-Slot matcht keinen MAC-
        // Eintrag → Crypto-Fail.
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let alice = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let bob = SharedSecurityGate::new(0, gov.clone(), Box::new(AesGcmCryptoPlugin::new()));
        let eve = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [0xA2; 12];
        let bob_prefix: PeerKey = [0xB2; 12];
        alice
            .register_remote_by_guid(
                bob_prefix,
                IdentityHandle(1),
                SharedSecretHandle(1),
                &bob.local_token().unwrap(),
            )
            .unwrap();
        // Eve kennt Alice's Token (Angreifer-Szenario), registriert sie.
        eve.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(10),
            SharedSecretHandle(10),
            &alice.local_token().unwrap(),
        )
        .unwrap();

        let wire = alice
            .transform_outbound_group(&[bob_prefix], b"confidential")
            .unwrap();

        let eve_prefix: PeerKey = [0xEE; 12];
        let err = eve
            .transform_inbound_group(&alice_prefix, &eve_prefix, &wire)
            .unwrap_err();
        assert!(
            matches!(
                err,
                SecurityGateError::Crypto(_) | SecurityGateError::Wrapper(_)
            ),
            "Eve ohne MAC-Entry muss droppen, got: {err:?}"
        );
    }

    #[test]
    fn group_transform_unknown_peer_is_policy_violation() {
        // Alice versucht fuer einen nicht-registrierten Peer zu
        // encoden → PolicyViolation.
        let alice = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let unregistered: PeerKey = [0x99; 12];
        let err = alice
            .transform_outbound_group(&[unregistered], b"x")
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn forget_remote_removes_mapping() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        assert!(bob.slot_for(&alice_prefix).unwrap().is_some());
        bob.forget_remote(&alice_prefix).unwrap();
        assert!(bob.slot_for(&alice_prefix).unwrap().is_none());
    }

    // -------------------------------------------------------------
    // RC1 Stufe 4a — transform_outbound_for
    // -------------------------------------------------------------

    #[test]
    fn transform_outbound_for_none_is_passthrough_even_on_protected_domain() {
        // Domain ist ENCRYPT per Governance, aber Caller verlangt
        // per-Reader None — muss plaintext liefern (das ist der
        // Heterogeneous-Fall).
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xBB; 12];
        let msg = fake_msg(b"[plain-for-legacy]");
        let out = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::None)
            .unwrap();
        assert_eq!(out, msg, "None-Level muss byte-identisch passthrough sein");
    }

    #[test]
    fn transform_outbound_for_encrypt_produces_srtps_wire() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xCC; 12];
        let msg = fake_msg(b"[enc-for-secure]");
        let wire = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::Encrypt)
            .unwrap();
        // Output muss laenger als plain sein (SRTPS-Overhead) und beim
        // SRTPS_PREFIX-Byte nach dem RTPS-Header starten.
        assert!(wire.len() > msg.len());
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    }

    #[test]
    fn transform_outbound_for_sign_also_uses_srtps_encoder() {
        // Sign-Level nutzt heute denselben Encoder wie Encrypt (v1.4-
        // Plugin-Status). Wichtig ist nur: Output ist KEIN plaintext.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let peer_key: PeerKey = [0xDD; 12];
        let msg = fake_msg(b"[sig-for-fast]");
        let wire = gate
            .transform_outbound_for(&peer_key, &msg, ProtectionLevel::Sign)
            .unwrap();
        assert_ne!(wire, msg, "Sign darf nicht byte-identisch zu plain sein");
        assert_eq!(wire[RTPS_HEADER_LEN], SRTPS_PREFIX);
    }

    #[test]
    fn transform_outbound_for_heterogeneous_three_readers() {
        // 1 Writer → 3 Reader (legacy/sign/encrypt). Jedes Output ist
        // individuell verschieden — das ist der Kern von RC1.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[broadcast]");
        let legacy = gate
            .transform_outbound_for(&[1; 12], &msg, ProtectionLevel::None)
            .unwrap();
        let fast = gate
            .transform_outbound_for(&[2; 12], &msg, ProtectionLevel::Sign)
            .unwrap();
        let secure = gate
            .transform_outbound_for(&[3; 12], &msg, ProtectionLevel::Encrypt)
            .unwrap();
        assert_eq!(legacy, msg, "Legacy-Reader bekommt plain");
        assert_ne!(fast, msg, "Fast-Reader bekommt SRTPS-wrapped");
        assert_ne!(secure, msg, "Secure-Reader bekommt SRTPS-wrapped");
        // Sign- und Encrypt-Pakete sind nicht byte-identisch — auch
        // wenn derselbe Encoder genutzt wird, nutzt jedes encode einen
        // frischen Nonce-Counter.
        assert_ne!(fast, secure, "Per-Reader-Encoding muss je verschieden sein");
    }

    // -------------------------------------------------------------
    // RC1 Stufe 5 — classify_inbound + allow_unauthenticated
    // -------------------------------------------------------------

    const GOV_NONE: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>NONE</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
    const GOV_ENCRYPT_ALLOW_UNAUTH: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <allow_unauthenticated_participants>TRUE</allow_unauthenticated_participants>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    #[test]
    fn allow_unauthenticated_default_false_without_element() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn allow_unauthenticated_reads_true_when_set() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_ENCRYPT_ALLOW_UNAUTH).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn allow_unauthenticated_defaults_false_for_unknown_domain() {
        let gate = SharedSecurityGate::new(
            99,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        assert!(!gate.allow_unauthenticated().unwrap());
    }

    #[test]
    fn classify_inbound_rejects_truncated_datagram() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_NONE).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let verdict = gate.classify_inbound(&[0u8; 10], &NetInterface::Wan);
        assert_eq!(verdict, InboundVerdict::Malformed);
        assert_eq!(verdict.category(), "inbound.malformed");
    }

    #[test]
    fn classify_inbound_plain_on_none_domain_accepts() {
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_NONE).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[plain-hello]");
        match gate.classify_inbound(&msg, &NetInterface::Wan) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_plain_on_protected_domain_is_legacy_blocked() {
        // Domain verlangt ENCRYPT, allow_unauth = false (default).
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[legacy-on-encrypted]");
        let verdict = gate.classify_inbound(&msg, &NetInterface::Wan);
        assert_eq!(verdict, InboundVerdict::LegacyBlocked);
        assert_eq!(verdict.category(), "inbound.legacy_blocked");
        assert!(!verdict.is_accept());
    }

    #[test]
    fn classify_inbound_plain_on_protected_domain_with_allow_unauth_accepts() {
        // DoD-Test: Legacy-Peer wird akzeptiert wenn Governance das
        // explizit zulaesst.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_ENCRYPT_ALLOW_UNAUTH).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[legacy-allowed]");
        match gate.classify_inbound(&msg, &NetInterface::Wan) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Accept (allow_unauthenticated=true), got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_plain_on_loopback_accepts_even_on_protected_domain() {
        // Arch-Doc §2.1: "Intra-Host-Loopback: Plain (kein Netz
        // verlassen)". Protected Domain, aber Interface=Loopback →
        // plaintext ist spec-konform akzeptiert.
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV_RTPS).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let msg = fake_msg(b"[loopback-plain]");
        match gate.classify_inbound(&msg, &NetInterface::Loopback) {
            InboundVerdict::Accept(out) => assert_eq!(out, msg),
            other => panic!("expected Loopback-Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_srtps_from_unknown_peer_is_policy_violation() {
        // Kein register_remote_by_guid — Peer ist unbekannt.
        let (alice, bob) = build_pair();
        // alice sendet uns einen SRTPS-Wrapper, aber bob hat alice
        // nicht registriert → classify muss PolicyViolation melden.
        let msg = fake_msg(b"[from-unknown]");
        let wire = alice.transform_outbound(&msg).unwrap();
        let verdict = bob.classify_inbound(&wire, &NetInterface::Wan);
        assert!(
            matches!(verdict, InboundVerdict::PolicyViolation(_)),
            "expected PolicyViolation, got {verdict:?}"
        );
        assert_eq!(verdict.category(), "inbound.policy_violation");
    }

    #[test]
    fn classify_inbound_srtps_from_known_peer_accepts() {
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        let msg = fake_msg(b"[authed-peer]");
        // Sender muss den gleichen GuidPrefix im Header tragen, damit
        // classify_inbound den Peer-Key findet.
        let mut hdr_msg = Vec::with_capacity(msg.len());
        hdr_msg.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        hdr_msg.extend_from_slice(&alice_prefix);
        hdr_msg.extend_from_slice(b"payload-body");
        let wire = alice.transform_outbound(&hdr_msg).unwrap();
        match bob.classify_inbound(&wire, &NetInterface::Wan) {
            InboundVerdict::Accept(_) => {}
            other => panic!("expected Accept, got {other:?}"),
        }
    }

    #[test]
    fn classify_inbound_srtps_with_wrong_key_is_crypto_error() {
        // Alice + Charlie encoden mit verschiedenen Keys; Bob hat
        // Alice registriert aber kriegt Charlie's Bytes unter Alice's
        // peer_key → Crypto-Tag-Mismatch.
        let (alice, bob) = build_pair();
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let charlie = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();

        // Charlie encodet mit Alice's prefix im Header (MITM-Simulation).
        let mut body = Vec::new();
        body.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        body.extend_from_slice(&alice_prefix);
        body.extend_from_slice(b"mitm-try");
        let spoofed = charlie.transform_outbound(&body).unwrap();

        let verdict = bob.classify_inbound(&spoofed, &NetInterface::Wan);
        assert!(
            matches!(verdict, InboundVerdict::CryptoError(_)),
            "expected CryptoError, got {verdict:?}"
        );
        assert_eq!(verdict.category(), "inbound.crypto_error");
    }

    #[test]
    fn transform_outbound_for_is_decodable_with_registered_token() {
        // E2E: Alice serialisiert per `transform_outbound_for`, Bob
        // registriert Alice's Token und dekodiert erfolgreich.
        let (alice, bob) = build_pair();
        let alice_prefix: PeerKey = [0xAA; 12];
        bob.register_remote_by_guid(
            alice_prefix,
            IdentityHandle(1),
            SharedSecretHandle(1),
            &alice.local_token().unwrap(),
        )
        .unwrap();
        let msg = fake_msg(b"[hetero-e2e]");
        let wire = alice
            .transform_outbound_for(&[9; 12], &msg, ProtectionLevel::Encrypt)
            .unwrap();
        let back = bob.transform_inbound_from(&alice_prefix, &wire).unwrap();
        assert_eq!(back, msg);
    }
}
