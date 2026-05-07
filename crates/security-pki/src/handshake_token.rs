// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §10.3.2.6-8 — `HandshakeMessageToken` Codec.
//!
//! Spec-konformer Wire-Layout fuer die drei Handshake-Tokens
//! (`HandshakeRequestMessageToken` / `HandshakeReplyMessageToken` /
//! `HandshakeFinalMessageToken`). Beide Seiten transportieren cert-DER,
//! ephemerals, challenges und die Signature ueber
//! (`c.kagree_algo` || challengeI || dhI || challengeR || dhR).
//!
//! # Wire-Layout
//!
//! Wir nutzen den shared `DataHolder`-Codec aus `zerodds_security::token`
//! (Spec §7.2.7 Tab.7) — XCDR1-LE mit length-prefix-Strings (NUL-
//! terminiert) und 4-byte-Alignment. Der gleiche Codec transportiert
//! die SPDP-`PID_IDENTITY_TOKEN`/`PID_PERMISSIONS_TOKEN` (C3.5), so
//! dass es nur **einen** Wire-Layout-Pfad gibt.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
// Re-Export, damit Caller `ht::DataHolder` weiter benutzen koennen.
use ring::digest;
pub use zerodds_security::token::DataHolder;

// ----------------------------------------------------------------------
// Spec-Konstanten — §10.3.2.6
// ----------------------------------------------------------------------

/// Spec class_id-Strings fuer die drei Handshake-Tokens.
pub mod class_id {
    /// `HandshakeRequestMessageToken` — Spec §10.3.2.6 Tab.56.
    pub const REQUEST: &str = "DDS:Auth:PKI-DH:1.2+AuthReq";
    /// `HandshakeReplyMessageToken` — Spec §10.3.2.7 Tab.57.
    pub const REPLY: &str = "DDS:Auth:PKI-DH:1.2+AuthReply";
    /// `HandshakeFinalMessageToken` — Spec §10.3.2.8 Tab.58.
    pub const FINAL: &str = "DDS:Auth:PKI-DH:1.2+AuthFinal";
}

/// Spec-Algorithmus-Strings (`c.dsign_algo`, `c.kagree_algo`).
pub mod algo {
    /// ECDSA über P-256 mit SHA-256 (Default fuer rcgen-Certs).
    pub const ECDSA_SHA256: &str = "ECDSA-SHA256";
    /// RSASSA-PSS mit SHA-256 (alternative fuer RSA-Certs).
    pub const RSASSA_PSS_SHA256: &str = "RSASSA-PSS-SHA256";
    /// ECDHE über P-256 mit Cofactor-EUM-Modus — Spec-Default.
    pub const ECDHE_CEUM_P256: &str = "ECDHE-CEUM-P256";
    /// DH mit MODP-2048-Group (Spec-Fallback).
    pub const DH_MODP_2048: &str = "DH+MODP-2048-256";
    /// X25519 — ZeroDDS-Erweiterung; nicht Spec, aber moderner Curve.
    pub const X25519: &str = "X25519";
}

/// Property-Keys laut Spec §10.3.2.6 Tab.56.
pub mod prop {
    /// Identity-Cert (DER).
    pub const C_ID: &str = "c.id";
    /// Permissions-Document (kann leer sein, wenn kein
    /// AccessControl-Plugin am Participant haengt).
    pub const C_PERM: &str = "c.perm";
    /// ParticipantBuiltinTopicData (XCDR), opaque hier.
    pub const C_PDATA: &str = "c.pdata";
    /// Digital-Signature-Algorithmus.
    pub const C_DSIGN_ALGO: &str = "c.dsign_algo";
    /// Key-Agreement-Algorithmus.
    pub const C_KAGREE_ALGO: &str = "c.kagree_algo";
    /// SHA-256 ueber das Tupel (c.id, c.perm, c.pdata, c.dsign_algo, c.kagree_algo).
    pub const HASH_C1: &str = "hash_c1";
    /// SHA-256 ueber das Replier-Tupel.
    pub const HASH_C2: &str = "hash_c2";
    /// Initiator-DH-Public-Key.
    pub const DH1: &str = "dh1";
    /// Replier-DH-Public-Key.
    pub const DH2: &str = "dh2";
    /// Initiator-Random-Challenge (32 byte).
    pub const CHALLENGE1: &str = "challenge1";
    /// Replier-Random-Challenge (32 byte).
    pub const CHALLENGE2: &str = "challenge2";
    /// OCSP-Response-Bytes (optional, leer = none).
    pub const OCSP_STATUS: &str = "ocsp_status";
    /// Replier/Initiator-Signature.
    pub const SIGNATURE: &str = "signature";
}

// ----------------------------------------------------------------------
// DoS-Caps — Spec §8.2.2 + ZeroDDS-Hardening
// ----------------------------------------------------------------------

/// Max Token-Bytes (DoS-Cap). Spec MTU-Heuristic: 64 KiB.
pub const MAX_TOKEN_BYTES: usize = 65_536;
/// Max Cert-DER-Bytes (16 KiB).
pub const MAX_CERT_DER: usize = 16_384;
/// Max Challenge-Bytes (Spec ist 32, aber wir cappen <= 64).
pub const MAX_CHALLENGE: usize = 64;
/// Max DH-Public-Key-Bytes (uncompressed P-256 = 65 byte, X25519 = 32).
pub const MAX_DH_PUB: usize = 256;
/// Max Property-Wert-String.
pub const MAX_PROP_VALUE: usize = 4096;
/// Max Property-Anzahl pro DataHolder.
pub const MAX_PROPS: usize = 64;
/// Max binary-Property-Anzahl pro DataHolder.
pub const MAX_BIN_PROPS: usize = 64;
/// Max Signature-Bytes (RSA-PSS-2048 = 256, ECDSA-P256-ASN.1 ≤ 72).
pub const MAX_SIGNATURE: usize = 1024;

// ----------------------------------------------------------------------
// Hash-Helper
// ----------------------------------------------------------------------

/// SHA-256 ueber `(c.id || c.perm || c.pdata || c.dsign_algo || c.kagree_algo)`.
///
/// Eingaben werden length-prefixed (u32-LE) verkettet, damit das Hash
/// kollisionsfest gegen unterschiedliche Splittings der gleichen Bytes
/// ist.
#[must_use]
pub fn compute_hash_c(
    c_id: &[u8],
    c_perm: &[u8],
    c_pdata: &[u8],
    c_dsign_algo: &str,
    c_kagree_algo: &str,
) -> [u8; 32] {
    let mut ctx = digest::Context::new(&digest::SHA256);
    fn put(ctx: &mut digest::Context, bytes: &[u8]) {
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        ctx.update(&len.to_le_bytes());
        ctx.update(bytes);
    }
    put(&mut ctx, c_id);
    put(&mut ctx, c_perm);
    put(&mut ctx, c_pdata);
    put(&mut ctx, c_dsign_algo.as_bytes());
    put(&mut ctx, c_kagree_algo.as_bytes());
    let d = ctx.finish();
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

/// Deterministische Bytes ueber die der Replier signiert (und der
/// Initiator beim Final).
///
/// Spec-Layout: `(c.kagree_algo || challengeI || dhI || challengeR || dhR)`.
/// Wir length-prefixen jedes Element (u32-LE), damit kein
/// Concatenation-Ambiguity-Angriff moeglich ist.
#[must_use]
pub fn signing_bytes(
    c_kagree_algo: &str,
    challenge1: &[u8],
    dh1: &[u8],
    challenge2: &[u8],
    dh2: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(c_kagree_algo.len() + 32 + 64 + 32 + 64 + 4 * 5);
    fn put(out: &mut Vec<u8>, bytes: &[u8]) {
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(bytes);
    }
    put(&mut out, c_kagree_algo.as_bytes());
    put(&mut out, challenge1);
    put(&mut out, dh1);
    put(&mut out, challenge2);
    put(&mut out, dh2);
    out
}

// ----------------------------------------------------------------------
// Token-Builder
// ----------------------------------------------------------------------

/// Parsed-View eines Request-Tokens (Initiator → Replier).
#[derive(Debug, Clone)]
pub struct RequestTokenView {
    /// Initiator-Identity-Cert (DER).
    pub cert_der: Vec<u8>,
    /// Permissions-Document (kann leer sein).
    pub permissions: Vec<u8>,
    /// ParticipantBuiltinTopicData (opaque).
    pub pdata: Vec<u8>,
    /// Digital-Signature-Algorithmus (z.B. `"ECDSA-SHA256"`).
    pub dsign_algo: String,
    /// Key-Agreement-Algorithmus (z.B. `"X25519"`).
    pub kagree_algo: String,
    /// SHA-256 ueber die obigen 5 Properties.
    pub hash_c1: [u8; 32],
    /// Initiator-DH-Public-Key.
    pub dh1: Vec<u8>,
    /// Initiator-Random-Challenge (32 byte).
    pub challenge1: [u8; 32],
    /// OCSP-Response-Bytes (kann leer sein).
    pub ocsp_status: Vec<u8>,
}

/// Parsed-View eines Reply-Tokens (Replier → Initiator).
#[derive(Debug, Clone)]
pub struct ReplyTokenView {
    /// Replier-Identity-Cert (DER).
    pub cert_der: Vec<u8>,
    /// Replier-Permissions.
    pub permissions: Vec<u8>,
    /// Replier-pdata.
    pub pdata: Vec<u8>,
    /// Replier-Digital-Signature-Algo.
    pub dsign_algo: String,
    /// Replier-Key-Agreement-Algo.
    pub kagree_algo: String,
    /// SHA-256 ueber Replier-Properties.
    pub hash_c2: [u8; 32],
    /// Echo des Initiator-`hash_c1`.
    pub hash_c1: [u8; 32],
    /// Replier-DH-Public-Key.
    pub dh2: Vec<u8>,
    /// Echo des Initiator-DH-Public.
    pub dh1: Vec<u8>,
    /// Replier-Challenge.
    pub challenge2: [u8; 32],
    /// Echo des Initiator-Challenge.
    pub challenge1: [u8; 32],
    /// Replier-OCSP-Status.
    pub ocsp_status: Vec<u8>,
    /// Replier-Signatur über `signing_bytes(kagree, ch1, dh1, ch2, dh2)`.
    pub signature: Vec<u8>,
}

/// Parsed-View eines Final-Tokens (Initiator → Replier).
#[derive(Debug, Clone)]
pub struct FinalTokenView {
    /// Echo `hash_c1`.
    pub hash_c1: [u8; 32],
    /// Echo `hash_c2`.
    pub hash_c2: [u8; 32],
    /// Echo `dh1`.
    pub dh1: Vec<u8>,
    /// Echo `dh2`.
    pub dh2: Vec<u8>,
    /// Echo `challenge1`.
    pub challenge1: [u8; 32],
    /// Echo `challenge2`.
    pub challenge2: [u8; 32],
    /// Initiator-OCSP-Status.
    pub ocsp_status: Vec<u8>,
    /// Initiator-Signatur über `signing_bytes(kagree, ch2, dh2, ch1, dh1)`.
    pub signature: Vec<u8>,
}

/// Eingaben zum Bauen des Request-Tokens.
pub struct RequestBuildInput<'a> {
    /// Cert-DER.
    pub cert_der: &'a [u8],
    /// Permissions (kann leer sein).
    pub permissions: &'a [u8],
    /// Pdata (opaque).
    pub pdata: &'a [u8],
    /// `c.dsign_algo` Wert.
    pub dsign_algo: &'a str,
    /// `c.kagree_algo` Wert.
    pub kagree_algo: &'a str,
    /// Initiator-DH-Public.
    pub dh1: &'a [u8],
    /// Initiator-Challenge (32 byte).
    pub challenge1: &'a [u8; 32],
    /// OCSP-Response-Bytes (kann leer sein).
    pub ocsp_status: &'a [u8],
}

/// Baut den Wire-Bytes eines Request-Tokens via DataHolder.
///
/// # Errors
/// Wenn DoS-Caps verletzt sind (Cert-DER > 16 KiB, DH > 256 byte, ...).
pub fn build_request_token(input: &RequestBuildInput<'_>) -> SecurityResult<Vec<u8>> {
    cap_check_request(input)?;
    let hash_c1 = compute_hash_c(
        input.cert_der,
        input.permissions,
        input.pdata,
        input.dsign_algo,
        input.kagree_algo,
    );
    let mut h = DataHolder::new(class_id::REQUEST);
    h.set_property(prop::C_DSIGN_ALGO, input.dsign_algo.to_owned());
    h.set_property(prop::C_KAGREE_ALGO, input.kagree_algo.to_owned());
    h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
    h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
    h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
    h.set_binary_property(prop::HASH_C1, hash_c1.to_vec());
    h.set_binary_property(prop::DH1, input.dh1.to_vec());
    h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
    h.set_binary_property(prop::OCSP_STATUS, input.ocsp_status.to_vec());
    Ok(h.to_cdr_le())
}

/// Parsed Wire-Bytes eines Request-Tokens und re-validiert `hash_c1`.
///
/// # Errors
/// `AuthenticationFailed` wenn `hash_c1` nicht zu den uebrigen
/// Properties passt; `BadArgument` bei Decode-/Cap-Fehlern.
pub fn parse_request_token(bytes: &[u8]) -> SecurityResult<RequestTokenView> {
    let h = DataHolder::from_cdr_le(bytes)?;
    if h.class_id != class_id::REQUEST {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "request: class_id mismatch",
        ));
    }
    let cert_der = take_bin(&h, prop::C_ID, MAX_CERT_DER)?;
    let permissions = take_bin(&h, prop::C_PERM, MAX_TOKEN_BYTES)?;
    let pdata = take_bin(&h, prop::C_PDATA, MAX_TOKEN_BYTES)?;
    let dsign_algo = take_prop(&h, prop::C_DSIGN_ALGO)?.to_owned();
    let kagree_algo = take_prop(&h, prop::C_KAGREE_ALGO)?.to_owned();
    let hash_c1 = take_fixed::<32>(&h, prop::HASH_C1)?;
    let dh1 = take_bin(&h, prop::DH1, MAX_DH_PUB)?;
    let challenge1 = take_fixed::<32>(&h, prop::CHALLENGE1)?;
    let ocsp_status = h.binary_property(prop::OCSP_STATUS).unwrap_or(&[]).to_vec();

    // Recompute hash_c1 — Schutz gegen MitM, der die Properties ohne
    // Cert-Tausch umschreibt.
    let recomputed = compute_hash_c(&cert_der, &permissions, &pdata, &dsign_algo, &kagree_algo);
    if !ct_eq(&recomputed, &hash_c1) {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "request: hash_c1 mismatch (token tampered)",
        ));
    }

    Ok(RequestTokenView {
        cert_der,
        permissions,
        pdata,
        dsign_algo,
        kagree_algo,
        hash_c1,
        dh1,
        challenge1,
        ocsp_status,
    })
}

/// Eingaben zum Bauen des Reply-Tokens.
pub struct ReplyBuildInput<'a> {
    /// Replier-Cert-DER.
    pub cert_der: &'a [u8],
    /// Replier-Permissions.
    pub permissions: &'a [u8],
    /// Replier-pdata.
    pub pdata: &'a [u8],
    /// Replier-`c.dsign_algo`.
    pub dsign_algo: &'a str,
    /// Replier-`c.kagree_algo` (Echo des Initiator-Werts).
    pub kagree_algo: &'a str,
    /// Replier-DH-Public.
    pub dh2: &'a [u8],
    /// Replier-Challenge.
    pub challenge2: &'a [u8; 32],
    /// Echo `hash_c1`.
    pub hash_c1: &'a [u8; 32],
    /// Echo `dh1`.
    pub dh1: &'a [u8],
    /// Echo `challenge1`.
    pub challenge1: &'a [u8; 32],
    /// OCSP-Status.
    pub ocsp_status: &'a [u8],
    /// Replier-Signatur ueber `signing_bytes`.
    pub signature: &'a [u8],
}

/// Baut Wire-Bytes des Reply-Tokens.
///
/// # Errors
/// DoS-Cap-Verletzung.
pub fn build_reply_token(input: &ReplyBuildInput<'_>) -> SecurityResult<Vec<u8>> {
    cap_check_reply(input)?;
    let hash_c2 = compute_hash_c(
        input.cert_der,
        input.permissions,
        input.pdata,
        input.dsign_algo,
        input.kagree_algo,
    );
    let mut h = DataHolder::new(class_id::REPLY);
    h.set_property(prop::C_DSIGN_ALGO, input.dsign_algo.to_owned());
    h.set_property(prop::C_KAGREE_ALGO, input.kagree_algo.to_owned());
    h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
    h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
    h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
    h.set_binary_property(prop::HASH_C2, hash_c2.to_vec());
    h.set_binary_property(prop::HASH_C1, input.hash_c1.to_vec());
    h.set_binary_property(prop::DH2, input.dh2.to_vec());
    h.set_binary_property(prop::DH1, input.dh1.to_vec());
    h.set_binary_property(prop::CHALLENGE2, input.challenge2.to_vec());
    h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
    h.set_binary_property(prop::OCSP_STATUS, input.ocsp_status.to_vec());
    h.set_binary_property(prop::SIGNATURE, input.signature.to_vec());
    Ok(h.to_cdr_le())
}

/// Parses Reply-Token. Rekomputiert hash_c2.
///
/// # Errors
/// Decode-/Cap-/Hash-Mismatch.
pub fn parse_reply_token(bytes: &[u8]) -> SecurityResult<ReplyTokenView> {
    let h = DataHolder::from_cdr_le(bytes)?;
    if h.class_id != class_id::REPLY {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "reply: class_id mismatch",
        ));
    }
    let cert_der = take_bin(&h, prop::C_ID, MAX_CERT_DER)?;
    let permissions = take_bin(&h, prop::C_PERM, MAX_TOKEN_BYTES)?;
    let pdata = take_bin(&h, prop::C_PDATA, MAX_TOKEN_BYTES)?;
    let dsign_algo = take_prop(&h, prop::C_DSIGN_ALGO)?.to_owned();
    let kagree_algo = take_prop(&h, prop::C_KAGREE_ALGO)?.to_owned();
    let hash_c2 = take_fixed::<32>(&h, prop::HASH_C2)?;
    let hash_c1 = take_fixed::<32>(&h, prop::HASH_C1)?;
    let dh2 = take_bin(&h, prop::DH2, MAX_DH_PUB)?;
    let dh1 = take_bin(&h, prop::DH1, MAX_DH_PUB)?;
    let challenge2 = take_fixed::<32>(&h, prop::CHALLENGE2)?;
    let challenge1 = take_fixed::<32>(&h, prop::CHALLENGE1)?;
    let ocsp_status = h.binary_property(prop::OCSP_STATUS).unwrap_or(&[]).to_vec();
    let signature = take_bin(&h, prop::SIGNATURE, MAX_SIGNATURE)?;

    let recomputed = compute_hash_c(&cert_der, &permissions, &pdata, &dsign_algo, &kagree_algo);
    if !ct_eq(&recomputed, &hash_c2) {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "reply: hash_c2 mismatch",
        ));
    }

    Ok(ReplyTokenView {
        cert_der,
        permissions,
        pdata,
        dsign_algo,
        kagree_algo,
        hash_c2,
        hash_c1,
        dh2,
        dh1,
        challenge2,
        challenge1,
        ocsp_status,
        signature,
    })
}

/// Eingaben zum Bauen des Final-Tokens.
pub struct FinalBuildInput<'a> {
    /// Echo `hash_c1`.
    pub hash_c1: &'a [u8; 32],
    /// Echo `hash_c2`.
    pub hash_c2: &'a [u8; 32],
    /// Echo `dh1`.
    pub dh1: &'a [u8],
    /// Echo `dh2`.
    pub dh2: &'a [u8],
    /// Echo `challenge1`.
    pub challenge1: &'a [u8; 32],
    /// Echo `challenge2`.
    pub challenge2: &'a [u8; 32],
    /// Initiator-OCSP-Status.
    pub ocsp_status: &'a [u8],
    /// Initiator-Signatur ueber `signing_bytes(kagree, ch2, dh2, ch1, dh1)`.
    pub signature: &'a [u8],
}

/// Baut Wire-Bytes des Final-Tokens.
///
/// # Errors
/// DoS-Cap.
pub fn build_final_token(input: &FinalBuildInput<'_>) -> SecurityResult<Vec<u8>> {
    if input.signature.len() > MAX_SIGNATURE {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "final: signature > cap",
        ));
    }
    if input.dh1.len() > MAX_DH_PUB || input.dh2.len() > MAX_DH_PUB {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "final: dh > cap",
        ));
    }
    let mut h = DataHolder::new(class_id::FINAL);
    h.set_binary_property(prop::HASH_C1, input.hash_c1.to_vec());
    h.set_binary_property(prop::HASH_C2, input.hash_c2.to_vec());
    h.set_binary_property(prop::DH1, input.dh1.to_vec());
    h.set_binary_property(prop::DH2, input.dh2.to_vec());
    h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
    h.set_binary_property(prop::CHALLENGE2, input.challenge2.to_vec());
    h.set_binary_property(prop::OCSP_STATUS, input.ocsp_status.to_vec());
    h.set_binary_property(prop::SIGNATURE, input.signature.to_vec());
    Ok(h.to_cdr_le())
}

/// Parses Final-Token.
///
/// # Errors
/// Decode-/Cap.
pub fn parse_final_token(bytes: &[u8]) -> SecurityResult<FinalTokenView> {
    let h = DataHolder::from_cdr_le(bytes)?;
    if h.class_id != class_id::FINAL {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "final: class_id mismatch",
        ));
    }
    Ok(FinalTokenView {
        hash_c1: take_fixed::<32>(&h, prop::HASH_C1)?,
        hash_c2: take_fixed::<32>(&h, prop::HASH_C2)?,
        dh1: take_bin(&h, prop::DH1, MAX_DH_PUB)?,
        dh2: take_bin(&h, prop::DH2, MAX_DH_PUB)?,
        challenge1: take_fixed::<32>(&h, prop::CHALLENGE1)?,
        challenge2: take_fixed::<32>(&h, prop::CHALLENGE2)?,
        ocsp_status: h.binary_property(prop::OCSP_STATUS).unwrap_or(&[]).to_vec(),
        signature: take_bin(&h, prop::SIGNATURE, MAX_SIGNATURE)?,
    })
}

// ----------------------------------------------------------------------
// Lookup-Helper
// ----------------------------------------------------------------------

fn cap_check_request(input: &RequestBuildInput<'_>) -> SecurityResult<()> {
    if input.cert_der.len() > MAX_CERT_DER {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "request: cert > 16 KiB",
        ));
    }
    if input.dh1.len() > MAX_DH_PUB {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "request: dh1 > 256 byte",
        ));
    }
    // Hinweis: input.challenge1 ist `&[u8; 32]` (statisch fix). Der
    // MAX_CHALLENGE=64-Cap kann nicht erreicht werden — historischer
    // Defensive-Check, vor Aenderung zu fixed-array. Entfernt, weil
    // er als always-false-Branch zu equivalent-Mutationen fuehrte.
    Ok(())
}

fn cap_check_reply(input: &ReplyBuildInput<'_>) -> SecurityResult<()> {
    if input.cert_der.len() > MAX_CERT_DER {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "reply: cert > 16 KiB",
        ));
    }
    if input.dh1.len() > MAX_DH_PUB || input.dh2.len() > MAX_DH_PUB {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "reply: dh > 256 byte",
        ));
    }
    if input.signature.len() > MAX_SIGNATURE {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "reply: signature > cap",
        ));
    }
    Ok(())
}

fn take_bin(h: &DataHolder, key: &str, max: usize) -> SecurityResult<Vec<u8>> {
    let v = h.binary_property(key).ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("missing binary property: {key}"),
        )
    })?;
    if v.len() > max {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            alloc::format!("property {key} > cap"),
        ));
    }
    Ok(v.to_vec())
}

fn take_fixed<const N: usize>(h: &DataHolder, key: &str) -> SecurityResult<[u8; N]> {
    let v = h.binary_property(key).ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("missing binary property: {key}"),
        )
    })?;
    if v.len() != N {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            alloc::format!("property {key} expected {N} byte"),
        ));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(v);
    Ok(out)
}

fn take_prop<'a>(h: &'a DataHolder, key: &str) -> SecurityResult<&'a str> {
    h.property(key).ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("missing property: {key}"),
        )
    })
}

/// Konstantes-Zeit-Vergleich (gegen Timing-Side-Channels).
#[must_use]
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn data_holder_roundtrip() {
        let mut h = DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq");
        h.set_property("c.dsign_algo", "ECDSA-SHA256");
        h.set_binary_property("c.id", alloc::vec![0xAA; 200]);
        let bytes = h.to_cdr_le();
        let parsed = DataHolder::from_cdr_le(&bytes).unwrap();
        assert_eq!(parsed.class_id, h.class_id);
        assert_eq!(parsed.property("c.dsign_algo"), Some("ECDSA-SHA256"));
        assert_eq!(parsed.binary_property("c.id").unwrap().len(), 200);
    }

    #[test]
    fn data_holder_truncation_rejected() {
        let mut h = DataHolder::new("X");
        h.set_binary_property("a", alloc::vec![1, 2, 3]);
        let bytes = h.to_cdr_le();
        let truncated = &bytes[..bytes.len() - 1];
        assert!(DataHolder::from_cdr_le(truncated).is_err());
    }

    #[test]
    fn data_holder_replace_dup() {
        let mut h = DataHolder::new("X");
        h.set_property("k", "v1");
        h.set_property("k", "v2");
        assert_eq!(h.properties.len(), 1);
        assert_eq!(h.property("k"), Some("v2"));
    }

    #[test]
    fn hash_c_is_deterministic_and_input_separated() {
        let h1 = compute_hash_c(b"a", b"b", b"c", "d", "e");
        let h2 = compute_hash_c(b"a", b"b", b"c", "d", "e");
        assert_eq!(h1, h2);
        // Unterschiedliches Splitting derselben Bytes -> anderer Hash.
        let h3 = compute_hash_c(b"ab", b"", b"c", "d", "e");
        assert_ne!(h1, h3);
    }

    #[test]
    fn signing_bytes_is_length_prefixed() {
        let s1 = signing_bytes("X25519", &[0xAA; 32], &[0xBB; 32], &[0xCC; 32], &[0xDD; 32]);
        let s2 = signing_bytes("X25519", &[0xAA; 32], &[0xBB; 32], &[0xCC; 32], &[0xDD; 32]);
        assert_eq!(s1, s2);
        let s3 = signing_bytes("X25519", &[0xAA; 32], &[0xBB; 32], &[0xCC; 32], &[0xDE; 32]);
        assert_ne!(s1, s3);
    }

    // -------------------------------------------------------------
    // Mutation-Killer (2026-05-01) — Cap-Boundary-Tests
    // -------------------------------------------------------------

    fn make_request_input<'a>(
        cert_der: &'a [u8],
        dh1: &'a [u8],
        challenge1: &'a [u8; 32],
    ) -> RequestBuildInput<'a> {
        RequestBuildInput {
            cert_der,
            permissions: &[],
            pdata: &[],
            dsign_algo: "ECDSA-SHA256",
            kagree_algo: "DH+MODP-2048-256",
            dh1,
            challenge1,
            ocsp_status: &[],
        }
    }

    /// Faengt `>` -> `==`/`>=` auf cap_check_request.cert_der-Cap.
    /// cert_der EXAKT MAX_CERT_DER muss durchgehen, MAX_CERT_DER+1 erroren.
    #[test]
    fn cap_check_request_cert_der_at_cap_accepted() {
        let cert = vec![0u8; MAX_CERT_DER];
        let dh = vec![0u8; 32];
        let ch = [0u8; 32];
        let res = cap_check_request(&make_request_input(&cert, &dh, &ch));
        assert!(res.is_ok());
    }
    #[test]
    fn cap_check_request_cert_der_over_cap_rejected() {
        let cert = vec![0u8; MAX_CERT_DER + 1];
        let dh = vec![0u8; 32];
        let ch = [0u8; 32];
        let err = cap_check_request(&make_request_input(&cert, &dh, &ch)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// dh1-Cap auf cap_check_request.
    #[test]
    fn cap_check_request_dh1_at_cap_accepted() {
        let cert = vec![0u8; 100];
        let dh = vec![0u8; MAX_DH_PUB];
        let ch = [0u8; 32];
        assert!(cap_check_request(&make_request_input(&cert, &dh, &ch)).is_ok());
    }
    #[test]
    fn cap_check_request_dh1_over_cap_rejected() {
        let cert = vec![0u8; 100];
        let dh = vec![0u8; MAX_DH_PUB + 1];
        let ch = [0u8; 32];
        let err = cap_check_request(&make_request_input(&cert, &dh, &ch)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// build_final_token: dh1 + dh2 EXAKT bei MAX_DH_PUB muss durchgehen.
    /// Faengt `>` -> `>=` Mutation auf der dh-Pruefung (Zeile 477).
    #[test]
    fn build_final_dh_both_exactly_at_cap_accepted() {
        let dh = vec![0u8; MAX_DH_PUB];
        let sig = vec![0u8; 64];
        assert!(build_final_token(&make_final_input(&dh, &dh, &sig)).is_ok());
    }

    fn make_reply_input<'a>(
        cert_der: &'a [u8],
        dh1: &'a [u8],
        dh2: &'a [u8],
        signature: &'a [u8],
    ) -> ReplyBuildInput<'a> {
        ReplyBuildInput {
            cert_der,
            permissions: &[],
            pdata: &[],
            dsign_algo: "ECDSA-SHA256",
            kagree_algo: "DH+MODP-2048-256",
            dh1,
            dh2,
            challenge1: &[0u8; 32],
            challenge2: &[0u8; 32],
            ocsp_status: &[],
            signature,
            hash_c1: &[0u8; 32],
        }
    }

    /// cap_check_reply.cert_der Cap-Boundary.
    #[test]
    fn cap_check_reply_cert_der_at_and_over_cap() {
        let dh = vec![0u8; 32];
        let sig = vec![0u8; 64];

        let cert_at = vec![0u8; MAX_CERT_DER];
        assert!(cap_check_reply(&make_reply_input(&cert_at, &dh, &dh, &sig)).is_ok());

        let cert_over = vec![0u8; MAX_CERT_DER + 1];
        let err = cap_check_reply(&make_reply_input(&cert_over, &dh, &dh, &sig)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// cap_check_reply: `||` -> `&&` auf `dh1>cap || dh2>cap`.
    /// Pro Seite (dh1 over, dh2 ok) und (dh1 ok, dh2 over) muss Reject kommen.
    /// Mit `&&` waere nur (BEIDE over) Reject.
    #[test]
    fn cap_check_reply_dh1_only_over_cap_rejected() {
        let cert = vec![0u8; 100];
        let dh1 = vec![0u8; MAX_DH_PUB + 1];
        let dh2 = vec![0u8; MAX_DH_PUB];
        let sig = vec![0u8; 64];
        let err = cap_check_reply(&make_reply_input(&cert, &dh1, &dh2, &sig)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
    #[test]
    fn cap_check_reply_dh2_only_over_cap_rejected() {
        let cert = vec![0u8; 100];
        let dh1 = vec![0u8; MAX_DH_PUB];
        let dh2 = vec![0u8; MAX_DH_PUB + 1];
        let sig = vec![0u8; 64];
        let err = cap_check_reply(&make_reply_input(&cert, &dh1, &dh2, &sig)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
    #[test]
    fn cap_check_reply_dh_both_at_cap_accepted() {
        let cert = vec![0u8; 100];
        let dh = vec![0u8; MAX_DH_PUB];
        let sig = vec![0u8; 64];
        assert!(cap_check_reply(&make_reply_input(&cert, &dh, &dh, &sig)).is_ok());
    }

    /// signature-Cap.
    #[test]
    fn cap_check_reply_signature_at_and_over_cap() {
        let cert = vec![0u8; 100];
        let dh = vec![0u8; 32];

        let sig_at = vec![0u8; MAX_SIGNATURE];
        assert!(cap_check_reply(&make_reply_input(&cert, &dh, &dh, &sig_at)).is_ok());

        let sig_over = vec![0u8; MAX_SIGNATURE + 1];
        let err = cap_check_reply(&make_reply_input(&cert, &dh, &dh, &sig_over)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// build_final_token: signature-Cap-Boundary.
    fn make_final_input<'a>(dh1: &'a [u8], dh2: &'a [u8], sig: &'a [u8]) -> FinalBuildInput<'a> {
        FinalBuildInput {
            hash_c1: &[0u8; 32],
            hash_c2: &[0u8; 32],
            dh1,
            dh2,
            challenge1: &[0u8; 32],
            challenge2: &[0u8; 32],
            ocsp_status: &[],
            signature: sig,
        }
    }

    #[test]
    fn build_final_signature_at_and_over_cap() {
        let dh = vec![0u8; 32];

        let sig_at = vec![0u8; MAX_SIGNATURE];
        assert!(build_final_token(&make_final_input(&dh, &dh, &sig_at)).is_ok());

        let sig_over = vec![0u8; MAX_SIGNATURE + 1];
        let err = build_final_token(&make_final_input(&dh, &dh, &sig_over)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// build_final_token: dh1/dh2 Cap-Pruefung pro Seite (`||` -> `&&`).
    #[test]
    fn build_final_dh1_only_over_cap_rejected() {
        let dh1 = vec![0u8; MAX_DH_PUB + 1];
        let dh2 = vec![0u8; MAX_DH_PUB];
        let sig = vec![0u8; 64];
        let err = build_final_token(&make_final_input(&dh1, &dh2, &sig)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
    #[test]
    fn build_final_dh2_only_over_cap_rejected() {
        let dh1 = vec![0u8; MAX_DH_PUB];
        let dh2 = vec![0u8; MAX_DH_PUB + 1];
        let sig = vec![0u8; 64];
        let err = build_final_token(&make_final_input(&dh1, &dh2, &sig)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// take_bin: max-Boundary. v.len() == max muss durchgehen,
    /// v.len() > max muss erroren.
    #[test]
    fn take_bin_at_and_over_max() {
        let mut h = DataHolder::new("test");
        h.set_binary_property("k", vec![0u8; 100]);
        // exact-fit:
        assert!(take_bin(&h, "k", 100).is_ok());
        // over:
        let err = take_bin(&h, "k", 99).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }
}
