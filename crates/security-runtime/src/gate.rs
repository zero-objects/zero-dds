// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SecurityGate` — governance-aware Submessage-Wrap.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security::error::SecurityError;
use zerodds_security_permissions::{Governance, ProtectionKind};
use zerodds_security_rtps::{
    RTPS_HEADER_LEN, SEC_PREFIX, SRTPS_PREFIX, SecurityRtpsError, decode_secured_rtps_message,
    decode_secured_submessage, encode_secured_rtps_message, encode_secured_submessage,
};

/// Fehler-Klasse fuer das Gate.
#[derive(Debug)]
pub enum SecurityGateError {
    /// Crypto-Plugin konnte keinen lokalen Handle registrieren.
    CryptoSetup(SecurityError),
    /// Encode/Decode des Secured-Wrappers fehlgeschlagen.
    Wrapper(SecurityRtpsError),
    /// Crypto-Operation selbst fehlgeschlagen.
    Crypto(SecurityError),
    /// Inbound erwartete SEC_PREFIX-Stream, bekam aber ein anderes
    /// Submessage-Format (z.B. plaintext wo Governance `SIGN` verlangt).
    PolicyViolation(String),
}

impl core::fmt::Display for SecurityGateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CryptoSetup(e) => write!(f, "security-gate setup: {e}"),
            Self::Wrapper(e) => write!(f, "security-gate wrapper: {e}"),
            Self::Crypto(e) => write!(f, "security-gate crypto: {e}"),
            Self::PolicyViolation(m) => write!(f, "security-gate policy: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SecurityGateError {}

impl From<SecurityRtpsError> for SecurityGateError {
    fn from(e: SecurityRtpsError) -> Self {
        Self::Wrapper(e)
    }
}

/// Entscheidet pro Topic ob/wie ausgehende Submessages verschluesselt
/// oder signiert werden muessen.
pub struct SecurityGate<'c, P: CryptographicPlugin> {
    domain_id: u32,
    governance: Governance,
    crypto: &'c mut P,
    /// Lokal registrierter CryptoHandle. Lazy erzeugt beim ersten
    /// encode, damit ein Gate auch ohne Handshake konstruierbar ist.
    local: Option<CryptoHandle>,
}

impl<'c, P: CryptographicPlugin> SecurityGate<'c, P> {
    /// Konstruktor.
    pub fn new(domain_id: u32, governance: Governance, crypto: &'c mut P) -> Self {
        Self {
            domain_id,
            governance,
            crypto,
            local: None,
        }
    }

    /// Liefert den CryptoHandle des lokalen Participants; registriert
    /// ihn beim Crypto-Plugin, wenn noch nicht geschehen.
    fn ensure_local(&mut self) -> Result<CryptoHandle, SecurityGateError> {
        if let Some(h) = self.local {
            return Ok(h);
        }
        let h = self
            .crypto
            .register_local_participant(IdentityHandle(1), &[])
            .map_err(SecurityGateError::CryptoSetup)?;
        self.local = Some(h);
        Ok(h)
    }

    /// Entscheidet, ob ausgehende Submessage fuer `topic_name` wrapped
    /// werden muss.
    #[must_use]
    pub fn outbound_protection(&self, topic_name: &str) -> ProtectionKind {
        self.governance
            .find_topic_rule(self.domain_id, topic_name)
            .map(|r| r.data_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Wrap ausgehende Submessage wenn Governance es verlangt.
    /// Protection-Kind `None` liefert das Original-Byte-Slice
    /// unveraendert zurueck (Passthrough).
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn encode_outbound(
        &mut self,
        topic_name: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let kind = self.outbound_protection(topic_name);
        match kind {
            ProtectionKind::None => Ok(plaintext.to_vec()),
            _ => {
                let local = self.ensure_local()?;
                let wrapped = encode_secured_submessage(self.crypto, local, &[], plaintext)?;
                Ok(wrapped)
            }
        }
    }

    /// Unwrap eingehende Submessage. Wenn das Format KEIN SEC_PREFIX
    /// zeigt, aber Governance `SIGN`/`ENCRYPT` verlangt → Policy-
    /// Violation. Wenn Governance `None` und die Bytes kein SEC_PREFIX,
    /// einfach passthrough.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    ///
    /// **Loopback-only:** dieses Convenience-Entry nutzt den lokalen
    /// Slot fuer Key-Lookup; echte Cross-Participant-Decoding laeuft
    /// ueber [`Self::decode_inbound_message`] mit `remote_slot`.
    pub fn decode_inbound(
        &mut self,
        topic_name: &str,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let kind = self.outbound_protection(topic_name);
        let looks_secured = !wire.is_empty() && wire[0] == SEC_PREFIX;
        match (kind, looks_secured) {
            (ProtectionKind::None, false) => Ok(wire.to_vec()),
            (_, true) => {
                let local = self.ensure_local()?;
                decode_secured_submessage(self.crypto, local, local, wire)
                    .map_err(SecurityGateError::from)
            }
            (_, false) => Err(SecurityGateError::PolicyViolation(alloc::format!(
                "topic '{topic_name}' verlangt {kind:?}, bekam plain-submessage"
            ))),
        }
    }

    /// Registriert einen Remote-Peer. Der zurueckgegebene Handle ist
    /// der CryptoHandle im **lokalen** Plugin, an dem der Remote-Key
    /// danach via [`Self::set_remote_token`] angelegt wird.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn register_remote(
        &mut self,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> Result<CryptoHandle, SecurityGateError> {
        let local = self.ensure_local()?;
        self.crypto
            .register_matched_remote_participant(local, remote_identity, shared_secret)
            .map_err(SecurityGateError::CryptoSetup)
    }

    /// Liefert den Crypto-Token des lokalen Participants (zu senden an
    /// Remote via SEDP-Participant-CryptoToken-Submessage).
    ///
    /// # Errors
    /// `CryptoSetup`/`Crypto` wenn der lokale Handle nicht existiert.
    pub fn local_token(&mut self) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        self.crypto
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .map_err(SecurityGateError::Crypto)
    }

    /// Akzeptiert einen Remote-Crypto-Token und installiert ihn unter
    /// dem uebergebenen Remote-Handle.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn set_remote_token(
        &mut self,
        remote: CryptoHandle,
        token: &[u8],
    ) -> Result<(), SecurityGateError> {
        let local = self.ensure_local()?;
        self.crypto
            .set_remote_participant_crypto_tokens(local, remote, token)
            .map_err(SecurityGateError::Crypto)
    }

    /// Ist fuer diese Domain ein RTPS-Message-Level-Schutz konfiguriert?
    /// Schaut in das erste matchende `<domain_rule>` und liefert den
    /// `rtps_protection_kind`.
    #[must_use]
    pub fn message_protection(&self) -> ProtectionKind {
        self.governance
            .find_domain_rule(self.domain_id)
            .map(|r| r.rtps_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Wrap eine komplette RTPS-Message (inkl. 20-byte Header) wenn
    /// `rtps_protection_kind` != None. Sonst passthrough.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn encode_outbound_message(
        &mut self,
        message: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        match self.message_protection() {
            ProtectionKind::None => Ok(message.to_vec()),
            _ => {
                let local = self.ensure_local()?;
                encode_secured_rtps_message(self.crypto, local, &[], message)
                    .map_err(SecurityGateError::from)
            }
        }
    }

    /// Unwrap eine eingehende RTPS-Message. `remote_slot` ist der
    /// `CryptoHandle` unter dem der **Sender-Key** registriert ist
    /// (liefert `register_remote` + `set_remote_token` zurueck).
    ///
    /// Implementation-Detail: der Plugin-Trait nutzt `local` als
    /// Key-Slot-Identifier (siehe OMG §8.5.1.9.4 Mapping), daher
    /// reichen wir `remote_slot` als `local`-Arg an den Codec durch —
    /// das ist der Slot in dem Alice's Master-Key liegt.
    ///
    /// # Errors
    /// Siehe [`SecurityGateError`].
    pub fn decode_inbound_message(
        &mut self,
        remote_slot: CryptoHandle,
        wire: &[u8],
    ) -> Result<Vec<u8>, SecurityGateError> {
        let looks_secured = wire.len() > RTPS_HEADER_LEN && wire[RTPS_HEADER_LEN] == SRTPS_PREFIX;
        let kind = self.message_protection();
        match (kind, looks_secured) {
            (ProtectionKind::None, false) => Ok(wire.to_vec()),
            (_, true) => {
                // Key-Lookup ueber remote_slot (dort ist der Sender-Key).
                decode_secured_rtps_message(self.crypto, remote_slot, remote_slot, wire)
                    .map_err(SecurityGateError::from)
            }
            (_, false) => Err(SecurityGateError::PolicyViolation(alloc::format!(
                "domain {} verlangt {kind:?}, bekam plain-rtps-message",
                self.domain_id
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <topic_access_rules>
      <topic_rule>
        <topic_expression>Secret*</topic_expression>
        <data_protection_kind>ENCRYPT</data_protection_kind>
      </topic_rule>
      <topic_rule>
        <topic_expression>*</topic_expression>
        <data_protection_kind>NONE</data_protection_kind>
      </topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    #[test]
    fn outbound_protection_reads_governance_topic_rule() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let gate = SecurityGate::new(0, gov, &mut crypto);
        assert_eq!(
            gate.outbound_protection("SecretRecipe"),
            ProtectionKind::Encrypt
        );
        assert_eq!(gate.outbound_protection("Chatter"), ProtectionKind::None);
    }

    #[test]
    fn encode_none_is_passthrough_byte_identical() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let plain = b"plaintext submessage";
        let wire = gate.encode_outbound("Chatter", plain).unwrap();
        assert_eq!(wire, plain);
    }

    #[test]
    fn encode_encrypt_wraps_in_sec_prefix() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let wire = gate.encode_outbound("SecretOrder", b"top-secret").unwrap();
        assert_eq!(wire[0], SEC_PREFIX, "must begin with SEC_PREFIX");
        assert!(
            !wire.windows(10).any(|w| w == b"top-secret"),
            "plaintext sollte nicht im wire sein"
        );
    }

    #[test]
    fn encode_decode_roundtrip_via_gate() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let wire = gate.encode_outbound("SecretOrder", b"hello").unwrap();
        let back = gate.decode_inbound("SecretOrder", &wire).unwrap();
        assert_eq!(back, b"hello");
    }

    #[test]
    fn inbound_plain_on_protected_topic_is_policy_violation() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        // Peer schickt plain auf `SecretOrder` — Policy verlangt ENCRYPT.
        let err = gate
            .decode_inbound("SecretOrder", b"plaintext-leak")
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    #[test]
    fn inbound_plain_on_unprotected_topic_passes_through() {
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let back = gate.decode_inbound("Chatter", b"plain-ok").unwrap();
        assert_eq!(back, b"plain-ok");
    }

    #[test]
    fn missing_domain_rule_defaults_to_none() {
        // Governance definiert Domain 0, wir laufen in Domain 99.
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let gate = SecurityGate::new(99, gov, &mut crypto);
        assert_eq!(
            gate.outbound_protection("SecretOrder"),
            ProtectionKind::None
        );
    }

    // -------------------------------------------------------------
    // RC1.2 — Message-Level + Cross-Participant E2E
    // -------------------------------------------------------------

    /// Governance mit `rtps_protection_kind=ENCRYPT` fuer Domain 0.
    const GOV_RTPS: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules>
      <topic_rule><topic_expression>*</topic_expression></topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;

    fn fake_rtps_message(body: &[u8]) -> Vec<u8> {
        let mut m = Vec::with_capacity(20 + body.len());
        m.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        m.extend_from_slice(&[0u8; 12]);
        m.extend_from_slice(body);
        m
    }

    #[test]
    fn message_protection_reads_domain_rule() {
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let gate = SecurityGate::new(0, gov, &mut crypto);
        assert_eq!(gate.message_protection(), ProtectionKind::Encrypt);
    }

    #[test]
    fn message_encode_none_is_passthrough() {
        // Default-Governance (ohne RTPS-Schutz) liefert None.
        let gov = parse_governance_xml(GOV).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let msg = fake_rtps_message(b"plain");
        let wire = gate.encode_outbound_message(&msg).unwrap();
        assert_eq!(wire, msg);
    }

    #[test]
    fn message_encode_encrypt_wraps_after_header() {
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        let msg = fake_rtps_message(b"[DATA][HEARTBEAT]");
        let wire = gate.encode_outbound_message(&msg).unwrap();
        assert_eq!(&wire[..4], b"RTPS");
        assert_eq!(wire[20], SRTPS_PREFIX);
    }

    #[test]
    fn message_policy_violation_on_plain_inbound() {
        let gov = parse_governance_xml(GOV_RTPS).unwrap();
        let mut crypto = AesGcmCryptoPlugin::new();
        let mut gate = SecurityGate::new(0, gov, &mut crypto);
        // Plain-RTPS-Message eingehend — aber Governance will ENCRYPT.
        let plain = fake_rtps_message(b"nope");
        let err = gate
            .decode_inbound_message(CryptoHandle(1), &plain)
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    /// E2E-Test: Alice + Bob — jede Seite ihr eigenes Crypto-Plugin,
    /// Token-Austausch, dann Message-Roundtrip.
    #[test]
    fn e2e_cross_participant_message_roundtrip() {
        let gov1 = parse_governance_xml(GOV_RTPS).unwrap();
        let gov2 = parse_governance_xml(GOV_RTPS).unwrap();
        let mut alice_crypto = AesGcmCryptoPlugin::new();
        let mut bob_crypto = AesGcmCryptoPlugin::new();

        let mut alice = SecurityGate::new(0, gov1, &mut alice_crypto);
        let mut bob = SecurityGate::new(0, gov2, &mut bob_crypto);

        // 1) Jeder zieht seinen Token und tauscht aus. In Produktion
        //    laeuft das ueber SEDP-ParticipantCryptoToken-Submessage.
        let alice_token = alice.local_token().unwrap();
        let bob_token = bob.local_token().unwrap();

        // 2) Jeder registriert den Gegenueber als Remote-Handle und
        //    installiert den empfangenen Token dort.
        let alice_view_of_bob = alice
            .register_remote(IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        alice
            .set_remote_token(alice_view_of_bob, &bob_token)
            .unwrap();

        let bob_view_of_alice = bob
            .register_remote(IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        bob.set_remote_token(bob_view_of_alice, &alice_token)
            .unwrap();

        // 3) Alice sendet eine verschluesselte Message.
        let msg = fake_rtps_message(b"[DATA:cross-participant]");
        let wire = alice.encode_outbound_message(&msg).unwrap();

        // 4) Bob entschluesselt (mit dem slot fuer Alice).
        let back = bob
            .decode_inbound_message(bob_view_of_alice, &wire)
            .unwrap();
        assert_eq!(back, msg);
    }
}
