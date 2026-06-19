// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SecurityGate` — governance-aware submessage wrap.

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

/// Error class for the gate.
#[derive(Debug)]
pub enum SecurityGateError {
    /// The crypto plugin could not register a local handle.
    CryptoSetup(SecurityError),
    /// Encode/decode of the secured wrapper failed.
    Wrapper(SecurityRtpsError),
    /// The crypto operation itself failed.
    Crypto(SecurityError),
    /// Inbound expected a SEC_PREFIX stream but got a different
    /// submessage format (e.g. plaintext where governance requires `SIGN`).
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

/// Decides per topic whether/how outgoing submessages must be
/// encrypted or signed.
pub struct SecurityGate<'c, P: CryptographicPlugin> {
    domain_id: u32,
    governance: Governance,
    crypto: &'c mut P,
    /// Locally registered CryptoHandle. Created lazily on the first
    /// encode, so a gate is constructible even without a handshake.
    local: Option<CryptoHandle>,
}

impl<'c, P: CryptographicPlugin> SecurityGate<'c, P> {
    /// Constructor.
    pub fn new(domain_id: u32, governance: Governance, crypto: &'c mut P) -> Self {
        Self {
            domain_id,
            governance,
            crypto,
            local: None,
        }
    }

    /// Returns the CryptoHandle of the local participant; registers
    /// it with the crypto plugin if not already done.
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

    /// Decides whether the outgoing submessage for `topic_name` must be
    /// wrapped.
    #[must_use]
    pub fn outbound_protection(&self, topic_name: &str) -> ProtectionKind {
        self.governance
            .find_topic_rule(self.domain_id, topic_name)
            .map(|r| r.data_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Wraps the outgoing submessage if governance requires it.
    /// Protection kind `None` returns the original byte slice
    /// unchanged (passthrough).
    ///
    /// # Errors
    /// See [`SecurityGateError`].
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

    /// Unwraps an incoming submessage. If the format shows NO SEC_PREFIX
    /// but governance requires `SIGN`/`ENCRYPT` → policy
    /// violation. If governance is `None` and the bytes are not a SEC_PREFIX,
    /// simply passthrough.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
    ///
    /// **Loopback-only:** this convenience entry uses the local
    /// slot for key lookup; real cross-participant decoding goes
    /// via [`Self::decode_inbound_message`] with `remote_slot`.
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
                "topic '{topic_name}' requires {kind:?}, got a plain submessage"
            ))),
        }
    }

    /// Registers a remote peer. The returned handle is
    /// the CryptoHandle in the **local** plugin at which the remote key
    /// is then created via [`Self::set_remote_token`].
    ///
    /// # Errors
    /// See [`SecurityGateError`].
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

    /// Returns the crypto token of the local participant (to be sent to
    /// the remote via the SEDP ParticipantCryptoToken submessage).
    ///
    /// # Errors
    /// `CryptoSetup`/`Crypto` if the local handle does not exist.
    pub fn local_token(&mut self) -> Result<Vec<u8>, SecurityGateError> {
        let local = self.ensure_local()?;
        self.crypto
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .map_err(SecurityGateError::Crypto)
    }

    /// Accepts a remote crypto token and installs it under
    /// the supplied remote handle.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
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

    /// Is an RTPS message-level protection configured for this domain?
    /// Looks into the first matching `<domain_rule>` and returns the
    /// `rtps_protection_kind`.
    #[must_use]
    pub fn message_protection(&self) -> ProtectionKind {
        self.governance
            .find_domain_rule(self.domain_id)
            .map(|r| r.rtps_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Wraps a complete RTPS message (incl. 20-byte header) if
    /// `rtps_protection_kind` != None. Otherwise passthrough.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
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

    /// Unwraps an incoming RTPS message. `remote_slot` is the
    /// `CryptoHandle` under which the **sender key** is registered
    /// (returned by `register_remote` + `set_remote_token`).
    ///
    /// Implementation detail: the plugin trait uses `local` as the
    /// key-slot identifier (see OMG §8.5.1.9.4 mapping), so we
    /// pass `remote_slot` as the `local` arg through to the codec —
    /// that is the slot where Alice's master key lives.
    ///
    /// # Errors
    /// See [`SecurityGateError`].
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
                // Key lookup via remote_slot (the sender key is there).
                decode_secured_rtps_message(self.crypto, remote_slot, remote_slot, wire)
                    .map_err(SecurityGateError::from)
            }
            (_, false) => Err(SecurityGateError::PolicyViolation(alloc::format!(
                "domain {} requires {kind:?}, got a plain rtps message",
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
            "plaintext should not be in the wire"
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
        // The peer sends plain on `SecretOrder` — the policy requires ENCRYPT.
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
        // Governance defines domain 0, we run in domain 99.
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

    /// Governance with `rtps_protection_kind=ENCRYPT` for domain 0.
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
        // Default governance (without RTPS protection) returns None.
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
        // Plain RTPS message incoming — but governance wants ENCRYPT.
        let plain = fake_rtps_message(b"nope");
        let err = gate
            .decode_inbound_message(CryptoHandle(1), &plain)
            .unwrap_err();
        assert!(matches!(err, SecurityGateError::PolicyViolation(_)));
    }

    /// E2E test: Alice + Bob — each side its own crypto plugin,
    /// token exchange, then a message roundtrip.
    #[test]
    fn e2e_cross_participant_message_roundtrip() {
        let gov1 = parse_governance_xml(GOV_RTPS).unwrap();
        let gov2 = parse_governance_xml(GOV_RTPS).unwrap();
        let mut alice_crypto = AesGcmCryptoPlugin::new();
        let mut bob_crypto = AesGcmCryptoPlugin::new();

        let mut alice = SecurityGate::new(0, gov1, &mut alice_crypto);
        let mut bob = SecurityGate::new(0, gov2, &mut bob_crypto);

        // 1) Each pulls its token and exchanges it. In production
        //    this goes via the SEDP ParticipantCryptoToken submessage.
        let alice_token = alice.local_token().unwrap();
        let bob_token = bob.local_token().unwrap();

        // 2) Each registers the counterpart as a remote handle and
        //    installs the received token there.
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

        // 3) Alice sends an encrypted message.
        let msg = fake_rtps_message(b"[DATA:cross-participant]");
        let wire = alice.encode_outbound_message(&msg).unwrap();

        // 4) Bob decrypts (with the slot for Alice).
        let back = bob
            .decode_inbound_message(bob_view_of_alice, &wire)
            .unwrap();
        assert_eq!(back, msg);
    }
}
