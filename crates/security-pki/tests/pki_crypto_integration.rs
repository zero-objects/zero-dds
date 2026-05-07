//! End-to-End: PKI-Handshake → SharedSecret → AES-GCM-Encrypt.
//!
//! Aktualisiert fuer C3.1: Spec-konformer 3-Round-Handshake mit
//! cert-bind und signaturen. Identitaeten muessen vor dem Handshake
//! ueber `validate_with_config` + `validate_remote_identity` registriert
//! sein.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Integration-Test muss `Arc<dyn SharedSecretProvider>` nutzen —
//! das ist genau der Zweck des Traits: Plugin-Substitution.)

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

use std::sync::{Arc, Mutex};

use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
    SharedSecretProvider,
};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

/// Test-Helper: erzeugt eine gemeinsame CA + zwei EE-Certs (Alice,
/// Bob) inklusive Private-Keys.
struct CertBundle {
    ca_pem: Vec<u8>,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
}

fn make_pair() -> (Vec<u8>, CertBundle, CertBundle) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_params = CertificateParams::new(vec!["IT-CA".into()]).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let ca_pem = ca_cert.pem().into_bytes();

    let make_ee = |name: &str| {
        let mut p = CertificateParams::new(vec![name.into()]).unwrap();
        p.is_ca = rcgen::IsCa::NoCa;
        let kp = KeyPair::generate().unwrap();
        let cert = p.signed_by(&kp, &ca_cert, &ca_key).unwrap();
        CertBundle {
            ca_pem: ca_pem.clone(),
            cert_pem: cert.pem().into_bytes(),
            key_pem: kp.serialize_pem().into_bytes(),
        }
    };
    let alice = make_ee("alice");
    let bob = make_ee("bob");
    (ca_pem, alice, bob)
}

fn cert_der(pem: &[u8]) -> Vec<u8> {
    use rustls_pki_types::CertificateDer;
    use rustls_pki_types::pem::PemObject;
    CertificateDer::pem_slice_iter(pem)
        .next()
        .unwrap()
        .unwrap()
        .as_ref()
        .to_vec()
}

fn register_pair(
    me: &mut PkiAuthenticationPlugin,
    me_b: &CertBundle,
    peer_cert_pem: &[u8],
    me_guid: [u8; 16],
    peer_guid: [u8; 16],
) -> (IdentityHandle, IdentityHandle) {
    let me_h = me
        .validate_with_config(
            IdentityConfig {
                identity_cert_pem: me_b.cert_pem.clone(),
                identity_ca_pem: me_b.ca_pem.clone(),
                identity_key_pem: Some(me_b.key_pem.clone()),
            },
            me_guid,
        )
        .unwrap();
    let peer_h = me
        .validate_remote_identity(me_h, peer_guid, &cert_der(peer_cert_pem))
        .unwrap();
    (me_h, peer_h)
}

/// Shared-Ownership-Wrapper — ermoeglicht dass derselbe
/// `PkiAuthenticationPlugin` sowohl als `AuthenticationPlugin` fuer
/// den Handshake als auch als `SharedSecretProvider` fuer den
/// AesGcmCryptoPlugin genutzt wird.
#[derive(Clone)]
struct SharedPki(Arc<Mutex<PkiAuthenticationPlugin>>);

impl SharedSecretProvider for SharedPki {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.secret_bytes(handle).map(<[u8]>::to_vec)
    }
}

/// Fuehrt den vollen 3-Round-Handshake durch. Liefert die
/// SharedSecret-Handles beider Seiten.
fn run_handshake(
    alice: &mut PkiAuthenticationPlugin,
    bob: &mut PkiAuthenticationPlugin,
    alice_h: IdentityHandle,
    alice_remote_bob: IdentityHandle,
    bob_h: IdentityHandle,
    bob_remote_alice: IdentityHandle,
) -> (SharedSecretHandle, SharedSecretHandle) {
    let (alice_hs, out1) = alice
        .begin_handshake_request(alice_h, alice_remote_bob)
        .expect("alice request");
    let req = match out1 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected SendMessage from request"),
    };

    let (bob_hs, out2) = bob
        .begin_handshake_reply(bob_h, bob_remote_alice, &req)
        .expect("bob reply");
    let reply_token = match out2 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected SendMessage from reply"),
    };

    let out3 = alice.process_handshake(alice_hs, &reply_token).unwrap();
    let final_token = match out3 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected SendMessage (final) from initiator"),
    };
    // Alice ist nach SendMessage(final) bereit — ihr SharedSecret ist
    // schon gespeichert (handshake_to_secret).
    let alice_secret = alice.shared_secret(alice_hs).unwrap();

    // Bob verarbeitet das Final → Complete.
    let out4 = bob.process_handshake(bob_hs, &final_token).unwrap();
    let bob_secret = match out4 {
        HandshakeStepOutcome::Complete { secret } => secret,
        _ => panic!("expected Complete for bob"),
    };
    (alice_secret, bob_secret)
}

#[test]
fn x25519_handshake_shared_secret_drives_aes_gcm_roundtrip() {
    // --- Setup: Alice + Bob mit gemeinsamer CA + Identity-Keys ---
    let (_ca, alice_b, bob_b) = make_pair();
    let alice_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let bob_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let (alice_h, alice_remote_bob, bob_h, bob_remote_alice) = {
        let mut a = alice_pki.lock().unwrap();
        let mut b = bob_pki.lock().unwrap();
        let (ah, abob) = register_pair(&mut a, &alice_b, &bob_b.cert_pem, [0xAA; 16], [0xBB; 16]);
        let (bh, balice) = register_pair(&mut b, &bob_b, &alice_b.cert_pem, [0xBB; 16], [0xAA; 16]);
        (ah, abob, bh, balice)
    };

    // --- 3-Round-Handshake ---
    let (alice_secret, bob_secret) = {
        let mut a = alice_pki.lock().unwrap();
        let mut b = bob_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut b,
            alice_h,
            alice_remote_bob,
            bob_h,
            bob_remote_alice,
        )
    };

    // Sanity: beide haben denselben 32-byte-SharedSecret.
    assert_eq!(
        alice_pki
            .lock()
            .unwrap()
            .secret_bytes(alice_secret)
            .unwrap(),
        bob_pki.lock().unwrap().secret_bytes(bob_secret).unwrap(),
        "DH-shared-secret muss auf beiden Seiten identisch sein"
    );

    // --- Crypto-Plugin mit Provider ---
    let alice_provider: Arc<dyn SharedSecretProvider> = Arc::new(SharedPki(Arc::clone(&alice_pki)));
    let bob_provider: Arc<dyn SharedSecretProvider> = Arc::new(SharedPki(Arc::clone(&bob_pki)));
    let mut alice_crypto =
        AesGcmCryptoPlugin::with_secret_provider(Suite::Aes128Gcm, alice_provider);
    let mut bob_crypto = AesGcmCryptoPlugin::with_secret_provider(Suite::Aes128Gcm, bob_provider);

    let alice_local = alice_crypto
        .register_local_participant(alice_h, &[])
        .unwrap();
    let bob_local = bob_crypto.register_local_participant(bob_h, &[]).unwrap();

    let alice_for_bob = alice_crypto
        .register_matched_remote_participant(alice_local, alice_remote_bob, alice_secret)
        .unwrap();
    let bob_for_alice = bob_crypto
        .register_matched_remote_participant(bob_local, bob_remote_alice, bob_secret)
        .unwrap();

    let plain = b"per-peer-key roundtrip";
    let wire = alice_crypto
        .encrypt_submessage(alice_for_bob, &[], plain, &[])
        .unwrap();
    let back = bob_crypto
        .decrypt_submessage(bob_for_alice, bob_for_alice, &wire, &[])
        .expect("bob decodes with hkdf-derived key");
    assert_eq!(back, plain);
}

/// Erzeugt Bundle fuer N+1 Identitaeten (1 Alice + N peers) mit
/// gemeinsamer CA. Liefert Alice + Vec<peer-bundle>.
fn make_group(n_peers: usize) -> (CertBundle, Vec<CertBundle>) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_params = CertificateParams::new(vec!["GroupCA".into()]).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let ca_pem = ca_cert.pem().into_bytes();
    let make_ee = |name: &str| {
        let mut p = CertificateParams::new(vec![name.into()]).unwrap();
        p.is_ca = rcgen::IsCa::NoCa;
        let kp = KeyPair::generate().unwrap();
        let cert = p.signed_by(&kp, &ca_cert, &ca_key).unwrap();
        CertBundle {
            ca_pem: ca_pem.clone(),
            cert_pem: cert.pem().into_bytes(),
            key_pem: kp.serialize_pem().into_bytes(),
        }
    };
    let alice = make_ee("alice");
    let peers = (0..n_peers)
        .map(|i| make_ee(&format!("peer-{i}")))
        .collect();
    (alice, peers)
}

#[test]
fn multi_mac_group_broadcast_with_real_dh_derived_mac_keys() {
    use zerodds_security::crypto::ReceiverMac;
    let (alice_b, peers) = make_group(3);
    let bob_b = &peers[0];
    let charlie_b = &peers[1];
    let dave_b = &peers[2];

    let alice_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let mut bob_pki = PkiAuthenticationPlugin::new();
    let mut charlie_pki = PkiAuthenticationPlugin::new();
    let mut dave_pki = PkiAuthenticationPlugin::new();

    // Identitaeten registrieren.
    let (alice_h, alice_remote_bob) = {
        let mut a = alice_pki.lock().unwrap();
        register_pair(&mut a, &alice_b, &bob_b.cert_pem, [0xAA; 16], [0xB1; 16])
    };
    let alice_remote_charlie = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(alice_h, [0xC1; 16], &cert_der(&charlie_b.cert_pem))
            .unwrap()
    };
    let alice_remote_dave = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(alice_h, [0xD1; 16], &cert_der(&dave_b.cert_pem))
            .unwrap()
    };
    let (bob_h, bob_remote_alice) = register_pair(
        &mut bob_pki,
        bob_b,
        &alice_b.cert_pem,
        [0xB1; 16],
        [0xAA; 16],
    );
    let (charlie_h, charlie_remote_alice) = register_pair(
        &mut charlie_pki,
        charlie_b,
        &alice_b.cert_pem,
        [0xC1; 16],
        [0xAA; 16],
    );
    let (dave_h, dave_remote_alice) = register_pair(
        &mut dave_pki,
        dave_b,
        &alice_b.cert_pem,
        [0xD1; 16],
        [0xAA; 16],
    );

    // Drei separate Handshakes.
    let (alice_bob, bob_secret) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut bob_pki,
            alice_h,
            alice_remote_bob,
            bob_h,
            bob_remote_alice,
        )
    };
    let (alice_charlie, charlie_secret) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut charlie_pki,
            alice_h,
            alice_remote_charlie,
            charlie_h,
            charlie_remote_alice,
        )
    };
    let (alice_dave, dave_secret) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut dave_pki,
            alice_h,
            alice_remote_dave,
            dave_h,
            dave_remote_alice,
        )
    };

    // Alice's Crypto-Plugin.
    let alice_provider: Arc<dyn SharedSecretProvider> = Arc::new(SharedPki(Arc::clone(&alice_pki)));
    let mut alice_crypto =
        AesGcmCryptoPlugin::with_secret_provider(Suite::Aes128Gcm, alice_provider);
    let alice_local = alice_crypto
        .register_local_participant(alice_h, &[])
        .unwrap();
    let h_bob = alice_crypto
        .register_matched_remote_participant(alice_local, alice_remote_bob, alice_bob)
        .unwrap();
    let h_charlie = alice_crypto
        .register_matched_remote_participant(alice_local, alice_remote_charlie, alice_charlie)
        .unwrap();
    let h_dave = alice_crypto
        .register_matched_remote_participant(alice_local, alice_remote_dave, alice_dave)
        .unwrap();

    let alice_broadcast_token = alice_crypto
        .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
        .unwrap();

    let key_id_bob = h_bob.0 as u32;
    let key_id_charlie = h_charlie.0 as u32;
    let key_id_dave = h_dave.0 as u32;

    let plain = b"group-broadcast-with-real-dh-mac-keys";
    let (ciphertext, macs) = alice_crypto
        .encrypt_submessage_multi(
            alice_local,
            &[
                (h_bob, key_id_bob),
                (h_charlie, key_id_charlie),
                (h_dave, key_id_dave),
            ],
            plain,
            &[],
        )
        .unwrap();
    assert_eq!(macs.len(), 3, "drei receiver → drei MACs");

    let expected = [
        (bob_pki, bob_secret, key_id_bob, bob_h, bob_remote_alice),
        (
            charlie_pki,
            charlie_secret,
            key_id_charlie,
            charlie_h,
            charlie_remote_alice,
        ),
        (
            dave_pki,
            dave_secret,
            key_id_dave,
            dave_h,
            dave_remote_alice,
        ),
    ];
    for (pki, dh_secret, expected_key_id, recv_h, remote_alice_h) in expected {
        let pki_arc = Arc::new(Mutex::new(pki));
        let provider: Arc<dyn SharedSecretProvider> = Arc::new(SharedPki(Arc::clone(&pki_arc)));
        let mut crypto = AesGcmCryptoPlugin::with_secret_provider(Suite::Aes128Gcm, provider);
        let recv_local = crypto.register_local_participant(recv_h, &[]).unwrap();

        let cipher_slot = crypto
            .register_matched_remote_participant(recv_local, remote_alice_h, SharedSecretHandle(0))
            .unwrap();
        crypto
            .set_remote_participant_crypto_tokens(recv_local, cipher_slot, &alice_broadcast_token)
            .expect("receiver installs alice's broadcast key");

        let mac_slot = crypto
            .register_matched_remote_participant(recv_local, remote_alice_h, dh_secret)
            .unwrap();

        let our_mac = macs
            .iter()
            .find(|m: &&ReceiverMac| m.key_id == expected_key_id)
            .expect("own mac exists in list");

        let back = crypto
            .decrypt_submessage_with_receiver_mac(
                cipher_slot,
                cipher_slot,
                our_mac.key_id,
                mac_slot,
                &ciphertext,
                core::slice::from_ref(our_mac),
                &[],
            )
            .expect("receiver decodes own mac");
        assert_eq!(back, plain);
    }
}

#[test]
fn three_peers_each_own_handshake_yield_distinct_per_peer_keys() {
    let (alice_b, peers) = make_group(3);
    let alice_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let mut bob_pki = PkiAuthenticationPlugin::new();
    let mut charlie_pki = PkiAuthenticationPlugin::new();
    let mut dave_pki = PkiAuthenticationPlugin::new();

    let (alice_h, alice_remote_bob) = {
        let mut a = alice_pki.lock().unwrap();
        register_pair(&mut a, &alice_b, &peers[0].cert_pem, [0xAA; 16], [0xB1; 16])
    };
    let alice_remote_charlie = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(alice_h, [0xC1; 16], &cert_der(&peers[1].cert_pem))
            .unwrap()
    };
    let alice_remote_dave = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(alice_h, [0xD1; 16], &cert_der(&peers[2].cert_pem))
            .unwrap()
    };

    let (bob_h, bob_remote_alice) = register_pair(
        &mut bob_pki,
        &peers[0],
        &alice_b.cert_pem,
        [0xB1; 16],
        [0xAA; 16],
    );
    let (charlie_h, charlie_remote_alice) = register_pair(
        &mut charlie_pki,
        &peers[1],
        &alice_b.cert_pem,
        [0xC1; 16],
        [0xAA; 16],
    );
    let (dave_h, dave_remote_alice) = register_pair(
        &mut dave_pki,
        &peers[2],
        &alice_b.cert_pem,
        [0xD1; 16],
        [0xAA; 16],
    );

    let (alice_bob, _) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut bob_pki,
            alice_h,
            alice_remote_bob,
            bob_h,
            bob_remote_alice,
        )
    };
    let (alice_charlie, _) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut charlie_pki,
            alice_h,
            alice_remote_charlie,
            charlie_h,
            charlie_remote_alice,
        )
    };
    let (alice_dave, _) = {
        let mut a = alice_pki.lock().unwrap();
        run_handshake(
            &mut a,
            &mut dave_pki,
            alice_h,
            alice_remote_dave,
            dave_h,
            dave_remote_alice,
        )
    };

    assert_ne!(alice_bob, alice_charlie);
    assert_ne!(alice_charlie, alice_dave);

    let secret_b = alice_pki
        .lock()
        .unwrap()
        .secret_bytes(alice_bob)
        .unwrap()
        .to_vec();
    let secret_c = alice_pki
        .lock()
        .unwrap()
        .secret_bytes(alice_charlie)
        .unwrap()
        .to_vec();
    let secret_d = alice_pki
        .lock()
        .unwrap()
        .secret_bytes(alice_dave)
        .unwrap()
        .to_vec();
    assert_ne!(secret_b, secret_c);
    assert_ne!(secret_c, secret_d);
    assert_ne!(secret_b, secret_d);

    let provider: Arc<dyn SharedSecretProvider> = Arc::new(SharedPki(Arc::clone(&alice_pki)));
    let mut crypto = AesGcmCryptoPlugin::with_secret_provider(Suite::Aes128Gcm, provider);
    let local = crypto.register_local_participant(alice_h, &[]).unwrap();
    let h_b = crypto
        .register_matched_remote_participant(local, alice_remote_bob, alice_bob)
        .unwrap();
    let h_c = crypto
        .register_matched_remote_participant(local, alice_remote_charlie, alice_charlie)
        .unwrap();
    let h_d = crypto
        .register_matched_remote_participant(local, alice_remote_dave, alice_dave)
        .unwrap();
    let tb = crypto
        .create_local_participant_crypto_tokens(h_b, CryptoHandle(0))
        .unwrap();
    let tc = crypto
        .create_local_participant_crypto_tokens(h_c, CryptoHandle(0))
        .unwrap();
    let td = crypto
        .create_local_participant_crypto_tokens(h_d, CryptoHandle(0))
        .unwrap();
    assert_ne!(tb, tc);
    assert_ne!(tc, td);
    assert_ne!(tb, td);
}
