//! End-to-end: PKI handshake → SharedSecret → AES-GCM encrypt.
//!
//! Updated for C3.1: spec-conformant 3-round handshake with
//! cert-bind and signatures. Identities must be registered before the handshake
//! via `validate_with_config` + `validate_remote_identity`.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The integration test must use `Arc<dyn SharedSecretProvider>` —
//! that is exactly the purpose of the trait: plugin substitution.)

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

/// Test helper: creates a shared CA + two EE certs (Alice,
/// Bob) including private keys.
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
        // The SubjectName gives the first 47 bits of the bound GUID.
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, name);
        p.distinguished_name = dn;
        p.is_ca = rcgen::IsCa::NoCa;
        let kp = KeyPair::generate().unwrap();
        let cert = p.signed_by(&kp, &ca_cert, &ca_key).unwrap();
        CertBundle {
            ca_pem: ca_pem.clone(),
            cert_pem: cert.pem().into_bytes(),
            key_pem: kp.serialize_pem().into_bytes(),
        }
    };
    // The bound GUID depends on the candidate too, so the pair is made
    // again until the first one (candidate 0xAA..) is the smaller.
    let (alice, bob) = (0u32..)
        .map(|attempt| {
            (
                make_ee(&format!("alice-{attempt}")),
                make_ee(&format!("bob-{attempt}")),
            )
        })
        .find(|(alice, bob)| bound(&alice.cert_pem, [0xAA; 16]) < bound(&bob.cert_pem, [0xBB; 16]))
        .unwrap();
    // Table 59: the participant with the smaller GUID starts the handshake.
    // The GUIDs are bound to the certificates (the tests use the candidates
    // 0xAA.. for Alice and 0xBB.. for Bob): Alice is the one that is smaller.
    (ca_pem, alice, bob)
}

/// The IdentityToken a peer announces: a DataHolder of the class
/// `DDS:Auth:PKI-DH:1.0` (DDS-Security 1.2 §10.3.2.1).
fn identity_token_of(_cert_pem: &[u8]) -> Vec<u8> {
    zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.0").to_cdr_le()
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

/// The participant GUID bound to the identity in `cert_pem` for the
/// candidate `seed` (DDS-Security 1.2 §10.3.3 Table 59).
fn bound(cert_pem: &[u8], seed: [u8; 16]) -> [u8; 16] {
    let prefix =
        zerodds_security_pki::adjust_participant_guid_prefix(&seed, &cert_der(cert_pem)).unwrap();
    let mut guid = seed;
    guid[..12].copy_from_slice(&prefix);
    guid
}

/// A `c.pdata`: PL_CDR_BE with PID_PARTICIPANT_GUID and PID_SENTINEL.
fn pdata_of(guid: [u8; 16]) -> Vec<u8> {
    let mut pdata = vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x50, 0x00, 0x10];
    pdata.extend_from_slice(&guid);
    pdata.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    pdata
}

/// Registers `me` and the peer under GUIDs bound to their certificates;
/// `me_guid` and `peer_guid` are the candidates.
fn register_pair(
    me: &mut PkiAuthenticationPlugin,
    me_b: &CertBundle,
    peer_cert_pem: &[u8],
    me_guid: [u8; 16],
    peer_guid: [u8; 16],
) -> (IdentityHandle, IdentityHandle, Vec<u8>) {
    let me_guid = bound(&me_b.cert_pem, me_guid);
    let peer_guid = bound(peer_cert_pem, peer_guid);
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
        .validate_remote_identity(me_h, peer_guid, &identity_token_of(peer_cert_pem), None)
        .unwrap()
        .identity;
    (me_h, peer_h, pdata_of(me_guid))
}

/// Shared-ownership wrapper — lets the same
/// `PkiAuthenticationPlugin` be used both as the `AuthenticationPlugin` for
/// the handshake and as the `SharedSecretProvider` for the
/// AesGcmCryptoPlugin.
#[derive(Clone)]
struct SharedPki(Arc<Mutex<PkiAuthenticationPlugin>>);

impl SharedSecretProvider for SharedPki {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.secret_bytes(handle).map(<[u8]>::to_vec)
    }
}

/// Runs the full 3-round handshake. Returns the
/// SharedSecret handles of both sides.
fn run_handshake(
    alice: &mut PkiAuthenticationPlugin,
    bob: &mut PkiAuthenticationPlugin,
    alice_h: IdentityHandle,
    alice_remote_bob: IdentityHandle,
    bob_h: IdentityHandle,
    bob_remote_alice: IdentityHandle,
    (alice_pdata, bob_pdata): (&[u8], &[u8]),
) -> (SharedSecretHandle, SharedSecretHandle) {
    let (alice_hs, out1) = alice
        .begin_handshake_request(alice_h, alice_remote_bob, alice_pdata)
        .expect("alice request");
    let req = match out1 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected SendMessage from request"),
    };

    let (bob_hs, out2) = bob
        .begin_handshake_reply(bob_h, bob_remote_alice, &req, bob_pdata)
        .expect("bob reply");
    let reply_token = match out2 {
        HandshakeStepOutcome::SendMessage { token } => token,
        _ => panic!("expected SendMessage from reply"),
    };

    let out3 = alice.process_handshake(alice_hs, &reply_token).unwrap();
    let final_token = match out3 {
        HandshakeStepOutcome::SendFinalMessage { token } => token,
        _ => panic!("expected SendFinalMessage from initiator"),
    };
    // Alice is ready after SendMessage(final) — her SharedSecret is
    // already stored (handshake_to_secret).
    let alice_secret = alice.shared_secret(alice_hs).unwrap();

    // Bob processes the final → Complete.
    let out4 = bob.process_handshake(bob_hs, &final_token).unwrap();
    let bob_secret = match out4 {
        HandshakeStepOutcome::Complete { secret } => secret,
        _ => panic!("expected Complete for bob"),
    };
    (alice_secret, bob_secret)
}

#[test]
fn x25519_handshake_shared_secret_drives_aes_gcm_roundtrip() {
    // --- Setup: Alice + Bob with a shared CA + identity keys ---
    let (_ca, alice_b, bob_b) = make_pair();
    let alice_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let bob_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let (alice_h, alice_remote_bob, bob_h, bob_remote_alice, alice_pdata, bob_pdata) = {
        let mut a = alice_pki.lock().unwrap();
        let mut b = bob_pki.lock().unwrap();
        let (ah, abob, apd) =
            register_pair(&mut a, &alice_b, &bob_b.cert_pem, [0xAA; 16], [0xBB; 16]);
        let (bh, balice, bpd) =
            register_pair(&mut b, &bob_b, &alice_b.cert_pem, [0xBB; 16], [0xAA; 16]);
        (ah, abob, bh, balice, apd, bpd)
    };

    // --- 3-round handshake ---
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
            (&alice_pdata, &bob_pdata),
        )
    };

    // Sanity: both have the same 32-byte SharedSecret.
    assert_eq!(
        alice_pki
            .lock()
            .unwrap()
            .secret_bytes(alice_secret)
            .unwrap(),
        bob_pki.lock().unwrap().secret_bytes(bob_secret).unwrap(),
        "DH shared secret must be identical on both sides"
    );

    // --- Crypto plugin with provider ---
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

    // FU2 dual key (spec §9.5.3.5): the data key is local+random and is transmitted
    // via a crypto token, NOT derived from the DH secret. Alice
    // exports her send key, Bob installs it for the decode.
    let alice_data_token = alice_crypto
        .create_local_participant_crypto_tokens(alice_for_bob, alice_for_bob)
        .unwrap();
    bob_crypto
        .set_remote_participant_crypto_tokens(bob_for_alice, bob_for_alice, &alice_data_token)
        .unwrap();

    let plain = b"per-peer-key roundtrip";
    let wire = alice_crypto
        .encrypt_submessage(alice_for_bob, &[], plain, &[])
        .unwrap();
    let back = bob_crypto
        .decrypt_submessage(bob_for_alice, bob_for_alice, &wire, &[])
        .expect("bob decodes alice's token-exchanged data key");
    assert_eq!(back, plain);
}

/// Creates a bundle for N+1 identities (1 Alice + N peers) with a
/// shared CA. Returns Alice + Vec<peer-bundle>.
fn make_group(n_peers: usize) -> (CertBundle, Vec<CertBundle>) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_params = CertificateParams::new(vec!["GroupCA".into()]).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let ca_pem = ca_cert.pem().into_bytes();
    let make_ee = |name: &str| {
        let mut p = CertificateParams::new(vec![name.into()]).unwrap();
        // The SubjectName gives the first 47 bits of the bound GUID.
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, name);
        p.distinguished_name = dn;
        p.is_ca = rcgen::IsCa::NoCa;
        let kp = KeyPair::generate().unwrap();
        let cert = p.signed_by(&kp, &ca_cert, &ca_key).unwrap();
        CertBundle {
            ca_pem: ca_pem.clone(),
            cert_pem: cert.pem().into_bytes(),
            key_pem: kp.serialize_pem().into_bytes(),
        }
    };
    // Table 59: the participant with the smaller GUID starts the handshake;
    // Alice starts with every peer, so each peer's bound GUID (the
    // candidates 0xB1.., 0xC1.., 0xD1..) has to be greater than hers. Alice
    // is made again until her GUID is in the lower part of the range, so
    // that peers are found quickly.
    let (alice, alice_guid) = (0u32..)
        .map(|attempt| make_ee(&format!("alice-{attempt}")))
        .map(|alice| {
            let guid = bound(&alice.cert_pem, [0xAA; 16]);
            (alice, guid)
        })
        .find(|(_, guid)| guid[0] < 0xC0)
        .unwrap();
    let seeds = [0xB1u8, 0xC1, 0xD1];
    let peers = (0..n_peers)
        .map(|i| {
            // The SubjectName decides the order, so each attempt has its own.
            (0u32..)
                .map(|attempt| make_ee(&format!("peer-{i}-{attempt}")))
                .find(|peer| bound(&peer.cert_pem, [seeds[i]; 16]) > alice_guid)
                .unwrap()
        })
        .collect();
    (alice, peers)
}

#[test]
fn three_peers_each_own_handshake_yield_distinct_per_peer_keys() {
    let (alice_b, peers) = make_group(3);
    let alice_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let mut bob_pki = PkiAuthenticationPlugin::new();
    let mut charlie_pki = PkiAuthenticationPlugin::new();
    let mut dave_pki = PkiAuthenticationPlugin::new();

    let (alice_h, alice_remote_bob, alice_pdata) = {
        let mut a = alice_pki.lock().unwrap();
        register_pair(&mut a, &alice_b, &peers[0].cert_pem, [0xAA; 16], [0xB1; 16])
    };
    let alice_remote_charlie = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(
            alice_h,
            bound(&peers[1].cert_pem, [0xC1; 16]),
            &identity_token_of(&peers[1].cert_pem),
            None,
        )
        .unwrap()
        .identity
    };
    let alice_remote_dave = {
        let mut a = alice_pki.lock().unwrap();
        a.validate_remote_identity(
            alice_h,
            bound(&peers[2].cert_pem, [0xD1; 16]),
            &identity_token_of(&peers[2].cert_pem),
            None,
        )
        .unwrap()
        .identity
    };

    let (bob_h, bob_remote_alice, bob_pdata) = register_pair(
        &mut bob_pki,
        &peers[0],
        &alice_b.cert_pem,
        [0xB1; 16],
        [0xAA; 16],
    );
    let (charlie_h, charlie_remote_alice, charlie_pdata) = register_pair(
        &mut charlie_pki,
        &peers[1],
        &alice_b.cert_pem,
        [0xC1; 16],
        [0xAA; 16],
    );
    let (dave_h, dave_remote_alice, dave_pdata) = register_pair(
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
            (&alice_pdata, &bob_pdata),
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
            (&alice_pdata, &charlie_pdata),
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
            (&alice_pdata, &dave_pdata),
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
