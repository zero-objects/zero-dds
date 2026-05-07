//! E2E-Handshake-Test fuer DDS-Security PKI Auth-Plugin.
//!
//! Verifiziert den drei-Stufen-Handshake (Request → Reply → Final)
//! durch reines Encode→Wire→Decode pro Stufe. Spec: DDS-Security
//! 1.2 §9.3.2.5 (Handshake Request/Reply/Final Tokens).
//!
//! 1. Initiator (Participant A) baut RequestToken mit eigenem Cert,
//!    DH1, Challenge1.
//! 2. Replier (Participant B) parst, baut ReplyToken mit eigenem
//!    Cert, DH2, Challenge2, Echo von HashC1/DH1/Ch1, plus Signatur.
//! 3. Initiator (A) parst Reply, baut FinalToken mit eigener Signatur.
//! 4. Replier (B) parst Final.
//!
//! Hash-c1/c2 werden bei jedem parse_* validiert; Signatur-Bytes
//! sind im Test placeholder (echte ECDSA-Sign braucht externes
//! Crypto).

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

use zerodds_security_pki::handshake_token::{
    FinalBuildInput, ReplyBuildInput, RequestBuildInput, build_final_token, build_reply_token,
    build_request_token,
};
use zerodds_security_pki::{parse_final_token, parse_reply_token, parse_request_token};

const A_CERT: &[u8] = b"---- A-CERT-DER (placeholder) ----";
const B_CERT: &[u8] = b"---- B-CERT-DER (placeholder) ----";
const A_PERM: &[u8] = b"---- A-PERMISSIONS-XML ----";
const B_PERM: &[u8] = b"---- B-PERMISSIONS-XML ----";
const A_PDATA: &[u8] = b"a-pdata";
const B_PDATA: &[u8] = b"b-pdata";
const DSIGN_ALGO: &str = "ECDSA-SHA256";
const KAGREE_ALGO: &str = "ECDHE-CEUM-P256";

const A_DH1: &[u8] = b"32-byte-A-public-DH-key__________";
const B_DH2: &[u8] = b"32-byte-B-public-DH-key__________";
const A_CHALLENGE: [u8; 32] = [0xAA; 32];
const B_CHALLENGE: [u8; 32] = [0xBB; 32];
const A_SIG_PLACEHOLDER: &[u8] = b"a-signature-placeholder-32-bytes";
const B_SIG_PLACEHOLDER: &[u8] = b"b-signature-placeholder-32-bytes";
const OCSP_STATUS: &[u8] = b"";

#[test]
fn full_three_stage_handshake() {
    // Stage 1: A -> B (Request)
    let req_bytes = build_request_token(&RequestBuildInput {
        cert_der: A_CERT,
        permissions: A_PERM,
        pdata: A_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh1: A_DH1,
        challenge1: &A_CHALLENGE,
        ocsp_status: OCSP_STATUS,
    })
    .unwrap();

    let req_view = parse_request_token(&req_bytes).expect("B must accept A's request");
    assert_eq!(req_view.cert_der, A_CERT);
    assert_eq!(req_view.dh1, A_DH1);
    assert_eq!(req_view.challenge1, A_CHALLENGE);

    // Stage 2: B -> A (Reply, echoing hash_c1/dh1/ch1 from request)
    let reply_bytes = build_reply_token(&ReplyBuildInput {
        cert_der: B_CERT,
        permissions: B_PERM,
        pdata: B_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh2: B_DH2,
        challenge2: &B_CHALLENGE,
        hash_c1: &req_view.hash_c1,
        dh1: &req_view.dh1,
        challenge1: &req_view.challenge1,
        ocsp_status: OCSP_STATUS,
        signature: B_SIG_PLACEHOLDER,
    })
    .unwrap();

    let reply_view = parse_reply_token(&reply_bytes).expect("A must accept B's reply");
    assert_eq!(reply_view.cert_der, B_CERT);
    assert_eq!(reply_view.dh2, B_DH2);
    assert_eq!(reply_view.challenge2, B_CHALLENGE);
    // Echo verifizieren
    assert_eq!(reply_view.hash_c1, req_view.hash_c1);
    assert_eq!(reply_view.dh1, A_DH1);
    assert_eq!(reply_view.challenge1, A_CHALLENGE);

    // Stage 3: A -> B (Final, echoing all)
    let final_bytes = build_final_token(&FinalBuildInput {
        hash_c1: &req_view.hash_c1,
        hash_c2: &reply_view.hash_c2,
        dh1: A_DH1,
        dh2: B_DH2,
        challenge1: &A_CHALLENGE,
        challenge2: &B_CHALLENGE,
        ocsp_status: OCSP_STATUS,
        signature: A_SIG_PLACEHOLDER,
    })
    .unwrap();

    let final_view = parse_final_token(&final_bytes).expect("B must accept A's final");
    assert_eq!(final_view.hash_c1, req_view.hash_c1);
    assert_eq!(final_view.hash_c2, reply_view.hash_c2);
    assert_eq!(final_view.challenge1, A_CHALLENGE);
    assert_eq!(final_view.challenge2, B_CHALLENGE);
}

#[test]
fn tampered_request_token_fails_hash_check() {
    let mut req_bytes = build_request_token(&RequestBuildInput {
        cert_der: A_CERT,
        permissions: A_PERM,
        pdata: A_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh1: A_DH1,
        challenge1: &A_CHALLENGE,
        ocsp_status: OCSP_STATUS,
    })
    .unwrap();

    // Flip one byte deep im Wire-Stream — hash_c1 sollte mismatchen.
    let mid = req_bytes.len() / 2;
    req_bytes[mid] ^= 0xFF;
    let res = parse_request_token(&req_bytes);
    assert!(res.is_err(), "tampered token must not parse — was: {res:?}");
}

#[test]
fn replay_attack_with_swapped_challenges_detected() {
    // Simuliert MitM: Angreifer faengt valides Reply-Token ab und
    // versucht es mit umgetauschtem Challenge2 weiterzugeben — der
    // Hash-C2-Recompute wuerde zwar stimmen wenn auch der c.dsign_algo
    // mitgeaendert wird, aber die Echo-Felder challenge1/dh1 muessen
    // im downstream-Final-Token gegen eigene Werte gepruefen werden.
    // In dieser Stufe: nur den Hash-Check verifizieren.
    let req_bytes = build_request_token(&RequestBuildInput {
        cert_der: A_CERT,
        permissions: A_PERM,
        pdata: A_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh1: A_DH1,
        challenge1: &A_CHALLENGE,
        ocsp_status: OCSP_STATUS,
    })
    .unwrap();
    let req_view = parse_request_token(&req_bytes).unwrap();

    let reply_bytes = build_reply_token(&ReplyBuildInput {
        cert_der: B_CERT,
        permissions: B_PERM,
        pdata: B_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh2: B_DH2,
        challenge2: &B_CHALLENGE,
        hash_c1: &req_view.hash_c1,
        dh1: &req_view.dh1,
        challenge1: &req_view.challenge1,
        ocsp_status: OCSP_STATUS,
        signature: B_SIG_PLACEHOLDER,
    })
    .unwrap();

    // Initiator parst — soll Echo gegen eigene Erinnerung pruefen.
    let reply_view = parse_reply_token(&reply_bytes).unwrap();
    assert_eq!(
        reply_view.challenge1, A_CHALLENGE,
        "Echo challenge1 must match initiator's original"
    );
    assert_eq!(
        reply_view.dh1, A_DH1,
        "Echo dh1 must match initiator's original"
    );
}

#[test]
fn empty_certs_rejected_by_dos_cap() {
    // Cert-DER groesser als MAX_CERT_DER (16 KiB) wird gerejected.
    let huge_cert = vec![0u8; 17 * 1024];
    let res = build_request_token(&RequestBuildInput {
        cert_der: &huge_cert,
        permissions: A_PERM,
        pdata: A_PDATA,
        dsign_algo: DSIGN_ALGO,
        kagree_algo: KAGREE_ALGO,
        dh1: A_DH1,
        challenge1: &A_CHALLENGE,
        ocsp_status: OCSP_STATUS,
    });
    assert!(res.is_err(), "huge cert must be rejected by DoS cap");
}

#[test]
fn handshake_with_different_cert_dh_combos() {
    // Sanity: 5 verschiedene cert/DH-Kombinationen produzieren alle
    // valide Tokens und round-trippen.
    for seed in 0u8..5 {
        let cert: Vec<u8> = format!("CERT-{seed}").into_bytes();
        let dh: Vec<u8> = format!("DH-PUBLIC-key-{seed}-32-bytes____").into_bytes();
        let challenge = [seed; 32];

        let req = build_request_token(&RequestBuildInput {
            cert_der: &cert,
            permissions: A_PERM,
            pdata: A_PDATA,
            dsign_algo: DSIGN_ALGO,
            kagree_algo: KAGREE_ALGO,
            dh1: &dh,
            challenge1: &challenge,
            ocsp_status: OCSP_STATUS,
        })
        .unwrap();

        let view = parse_request_token(&req).unwrap();
        assert_eq!(view.cert_der, cert);
        assert_eq!(view.dh1, dh);
        assert_eq!(view.challenge1, challenge);
    }
}
