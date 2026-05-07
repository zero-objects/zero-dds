//! PSK-Profile Integration-Tests (Spec §10.7-§10.9, C3.9).
//!
//! End-to-End: PSK-Authentication-Handshake → SharedSecret → AES-GCM-
//! Crypto-Plugin (via SharedSecretProvider). Demonstriert dass das
//! PSK-Profile interoperabel mit dem bestehenden Symmetric-Cipher-
//! Pfad ist (gleiche Wire-Bytes, andere Key-Source).
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Integration-Test nutzt `Arc<dyn SharedSecretProvider>` — der Trait-
//! Object-Typ ist genau das Plugin-Substitution-Interface.)

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;

use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeStepOutcome, IdentityHandle, SharedSecretProvider,
};
use zerodds_security::crypto::CryptographicPlugin;
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_pki::PskAuthenticationPlugin;

fn run_handshake(
    alice: &mut PskAuthenticationPlugin,
    bob: &mut PskAuthenticationPlugin,
    alice_h: IdentityHandle,
    bob_h: IdentityHandle,
) -> (
    zerodds_security::authentication::HandshakeHandle,
    zerodds_security::authentication::HandshakeHandle,
) {
    let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
    let req = match out1 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected request token"),
    };
    let (bob_hs, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
    let reply = match out2 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected reply token"),
    };
    let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
    let final_tok = match out3 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected final token"),
    };
    let out4 = bob.process_handshake(bob_hs, &final_tok).unwrap();
    match out4 {
        HandshakeStepOutcome::Complete { .. } => {}
        _ => panic!("expected complete"),
    }
    (alice_hs, bob_hs)
}

/// Sammelt die Bytes eines abgeschlossenen Handshakes ueber einen
/// `Arc<dyn SharedSecretProvider>`-Wrapper. Erlaubt dass der Crypto-
/// Plugin via PKI-Crypto-Hook den Master-Key aus dem PSK-Secret zieht.
struct ArcWrapper(Arc<dyn SharedSecretProvider>);

impl SharedSecretProvider for ArcWrapper {
    fn get_shared_secret(
        &self,
        handle: zerodds_security::authentication::SharedSecretHandle,
    ) -> Option<Vec<u8>> {
        self.0.get_shared_secret(handle)
    }
}

#[test]
fn psk_handshake_then_aes_gcm_roundtrip_uses_shared_secret() {
    // 1) Beide Seiten haben den gleichen PSK.
    let psk = vec![0x55u8; 32];
    let mut alice_auth = PskAuthenticationPlugin::new();
    let mut bob_auth = PskAuthenticationPlugin::new();
    alice_auth.register_psk("ab".into(), psk.clone()).unwrap();
    bob_auth.register_psk("ab".into(), psk).unwrap();
    let alice_id = alice_auth.validate_local_psk_identity("ab").unwrap();
    let bob_id = bob_auth.validate_local_psk_identity("ab").unwrap();

    // 2) Handshake durchlaufen.
    let (alice_hs, bob_hs) = run_handshake(&mut alice_auth, &mut bob_auth, alice_id, bob_id);
    let alice_secret = alice_auth.shared_secret(alice_hs).unwrap();
    let bob_secret = bob_auth.shared_secret(bob_hs).unwrap();

    // 3) Verify: beide Seiten haben gleichen Secret.
    let alice_bytes = alice_auth.secret_bytes(alice_secret).unwrap().to_vec();
    let bob_bytes = bob_auth.secret_bytes(bob_secret).unwrap().to_vec();
    assert_eq!(alice_bytes, bob_bytes);

    // 4) AES-GCM-Plugin auf beiden Seiten via SharedSecretProvider.
    //    SharedSecretProvider ist nicht Object-safe-clone-bar; wir
    //    bauen eine Mem-Provider-Bridge mit den gleichen 32 byte.
    use std::collections::BTreeMap;
    use std::sync::RwLock as StdRwLock;
    struct MemProvider {
        inner: StdRwLock<BTreeMap<zerodds_security::authentication::SharedSecretHandle, Vec<u8>>>,
    }
    impl SharedSecretProvider for MemProvider {
        fn get_shared_secret(
            &self,
            handle: zerodds_security::authentication::SharedSecretHandle,
        ) -> Option<Vec<u8>> {
            self.inner.read().ok()?.get(&handle).cloned()
        }
    }
    let pa = Arc::new(MemProvider {
        inner: StdRwLock::new(BTreeMap::new()),
    });
    let pb = Arc::new(MemProvider {
        inner: StdRwLock::new(BTreeMap::new()),
    });
    pa.inner
        .write()
        .unwrap()
        .insert(alice_secret, alice_bytes.clone());
    pb.inner
        .write()
        .unwrap()
        .insert(bob_secret, bob_bytes.clone());

    let mut alice_crypto = AesGcmCryptoPlugin::with_secret_provider(
        Suite::Aes128Gcm,
        Arc::clone(&pa) as Arc<dyn SharedSecretProvider>,
    );
    let mut bob_crypto = AesGcmCryptoPlugin::with_secret_provider(
        Suite::Aes128Gcm,
        Arc::clone(&pb) as Arc<dyn SharedSecretProvider>,
    );

    let alice_local = alice_crypto
        .register_local_participant(IdentityHandle(1), &[])
        .unwrap();
    let bob_local = bob_crypto
        .register_local_participant(IdentityHandle(1), &[])
        .unwrap();
    let alice_to_bob = alice_crypto
        .register_matched_remote_participant(alice_local, IdentityHandle(2), alice_secret)
        .unwrap();
    let bob_to_alice = bob_crypto
        .register_matched_remote_participant(bob_local, IdentityHandle(1), bob_secret)
        .unwrap();

    let plain = b"psk-driven aes-gcm sample";
    let wire = alice_crypto
        .encrypt_submessage(alice_to_bob, &[], plain, &[])
        .unwrap();
    let back = bob_crypto
        .decrypt_submessage(bob_to_alice, bob_to_alice, &wire, &[])
        .unwrap();
    assert_eq!(back, plain);

    let _ = ArcWrapper(Arc::clone(&pa) as Arc<dyn SharedSecretProvider>);
}

#[test]
fn psk_profile_bundle_factory_assembles_three_plugins() {
    use zerodds_security::AccessControlPlugin;
    use zerodds_security::access_control::AccessDecision;
    use zerodds_security_permissions::PskProfile;

    let permissions_xml = r#"
<permissions>
  <grant><subject_name>node-1</subject_name>
    <allow_rule>
      <publish><topic>Sensors</topic></publish>
      <subscribe><topic>Commands</topic></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions>
"#;
    let bundle =
        PskProfile::with_pre_shared_key("node-1", vec![0xCD; 32], None, permissions_xml).unwrap();

    assert_eq!(bundle.auth.plugin_class_id(), "DDS:Auth:PSK:1.2");
    assert_eq!(
        bundle.access_control.plugin_class_id(),
        "DDS:Access:PSK:Permissions:1.2"
    );
    assert_eq!(
        bundle.crypto.plugin_class_id(),
        "DDS:Crypto:PSK:AES-GCM-GMAC:1.2"
    );

    // Permission-Test direkt auf dem Bundle.
    use zerodds_security::access_control::PermissionsHandle;
    // Die Permissions wurden bereits im Konstruktor registriert (Handle 1).
    let h = PermissionsHandle(1);
    assert_eq!(
        bundle
            .access_control
            .check_create_datawriter(h, "Sensors")
            .unwrap(),
        AccessDecision::Permit
    );
    assert_eq!(
        bundle
            .access_control
            .check_create_datawriter(h, "OtherTopic")
            .unwrap(),
        AccessDecision::Deny
    );
}

#[test]
fn psk_handshake_with_completely_different_psks_fails() {
    let mut alice_auth = PskAuthenticationPlugin::new();
    let mut bob_auth = PskAuthenticationPlugin::new();
    alice_auth
        .register_psk("ab".into(), vec![0xAAu8; 32])
        .unwrap();
    bob_auth
        .register_psk("ab".into(), vec![0xBBu8; 32])
        .unwrap();
    let alice_id = alice_auth.validate_local_psk_identity("ab").unwrap();
    let bob_id = bob_auth.validate_local_psk_identity("ab").unwrap();

    let (alice_hs, out1) = alice_auth
        .begin_handshake_request(alice_id, bob_id)
        .unwrap();
    let req = match out1 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!(),
    };
    let (_, out2) = bob_auth
        .begin_handshake_reply(bob_id, alice_id, &req)
        .unwrap();
    let reply = match out2 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!(),
    };
    // Alice prueft Bobs HMAC mit Alice's Key → Mismatch.
    let err = alice_auth.process_handshake(alice_hs, &reply).unwrap_err();
    assert_eq!(
        err.kind,
        zerodds_security::error::SecurityErrorKind::AuthenticationFailed
    );
}
