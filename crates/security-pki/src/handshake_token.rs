// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security 1.2 §10.3.2.6-8 — `HandshakeMessageToken` Codec.
//!
//! Spec-conformant wire layout for the three handshake tokens
//! (`HandshakeRequestMessageToken` / `HandshakeReplyMessageToken` /
//! `HandshakeFinalMessageToken`). Both sides transport cert DER,
//! ephemerals, challenges and the signature over
//! (`c.kagree_algo` || challengeI || dhI || challengeR || dhR).
//!
//! # Wire layout
//!
//! We use the shared `DataHolder` codec from `zerodds_security::token`
//! (spec §7.2.7 Tab.7) — XCDR1-LE with length-prefix strings (NUL-
//! terminated) and 4-byte alignment. The same codec transports
//! the SPDP `PID_IDENTITY_TOKEN`/`PID_PERMISSIONS_TOKEN` (C3.5), so
//! that there is only **one** wire-layout path.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
// Re-export so callers can keep using `ht::DataHolder`.
use crate::backend::digest;
pub use zerodds_security::token::DataHolder;

// ----------------------------------------------------------------------
// Spec constants — §10.3.2.6
// ----------------------------------------------------------------------

/// Spec class_id strings for the three handshake tokens.
///
/// OMG DDS-Security §9.3.2.5: the built-in PKI-DH handshake tokens use the
/// class_ids `DDS:Auth:PKI-DH:1.0+Req/Reply/Final`. EXACTLY these are expected/
/// recognized by FastDDS/Cyclone/RTI cross-vendor — the earlier ZeroDDS variant
/// (`1.2+AuthReq/AuthReply/AuthFinal`) was not recognized by the foreign vendor, the
/// handshake reply never came (cross-vendor NO_MATCH).
pub mod class_id {
    /// `HandshakeRequestMessageToken` — spec §9.3.2.5.1.
    pub const REQUEST: &str = "DDS:Auth:PKI-DH:1.0+Req";
    /// `HandshakeReplyMessageToken` — spec §9.3.2.5.2.
    pub const REPLY: &str = "DDS:Auth:PKI-DH:1.0+Reply";
    /// `HandshakeFinalMessageToken` — spec §9.3.2.5.3.
    pub const FINAL: &str = "DDS:Auth:PKI-DH:1.0+Final";
}

/// Spec algorithm strings (`c.dsign_algo`, `c.kagree_algo`).
pub mod algo {
    /// ECDSA over P-256 with SHA-256 (default for rcgen certs).
    pub const ECDSA_SHA256: &str = "ECDSA-SHA256";
    /// RSASSA-PSS with SHA-256 (alternative for RSA certs).
    pub const RSASSA_PSS_SHA256: &str = "RSASSA-PSS-SHA256";
    /// ECDH over P-256 (prime256v1), cofactor-unified model — the **actually
    /// FastDDS/Cyclone-used** `c.kagree_algo` string (wire-capture-
    /// proven: cyclone sends exactly `ECDH+prime256v1-CEUM`). The previously
    /// guessed OMG text string "ECDHE+P-256+SHA-256" was NOT the real
    /// vendor string → cross-vendor mismatch. **Cross-vendor default.**
    pub const ECDHE_P256_SHA256: &str = "ECDH+prime256v1-CEUM";
    /// DH with MODP-2048 group — spec alternative (§9.3.2.5.1).
    pub const DH_MODP_2048: &str = "DH+MODP-2048-256";
    /// Obsolete ZeroDDS-internal spelling (not cross-vendor-readable) —
    /// only tolerated for backward compat when parsing, NO longer emitted.
    pub const ECDHE_CEUM_P256: &str = "ECDHE-CEUM-P256";
    /// X25519 — ZeroDDS extension, NOT spec. Only as an explicitly
    /// configurable vendor extension, NEVER default (no other vendor
    /// knows it → breaks the cross-vendor handshake).
    pub const X25519: &str = "X25519";
}

/// Property keys per spec §10.3.2.6 Tab.56.
pub mod prop {
    /// Identity cert (DER).
    pub const C_ID: &str = "c.id";
    /// Permissions document (may be empty if no
    /// AccessControl plugin is attached to the participant).
    pub const C_PERM: &str = "c.perm";
    /// ParticipantBuiltinTopicData (XCDR), opaque here.
    pub const C_PDATA: &str = "c.pdata";
    /// Digital signature algorithm.
    pub const C_DSIGN_ALGO: &str = "c.dsign_algo";
    /// Key agreement algorithm.
    pub const C_KAGREE_ALGO: &str = "c.kagree_algo";
    /// SHA-256 over the tuple (c.id, c.perm, c.pdata, c.dsign_algo, c.kagree_algo).
    pub const HASH_C1: &str = "hash_c1";
    /// SHA-256 over the replier tuple.
    pub const HASH_C2: &str = "hash_c2";
    /// Initiator DH public key.
    pub const DH1: &str = "dh1";
    /// Replier DH public key.
    pub const DH2: &str = "dh2";
    /// Initiator random challenge (32 bytes).
    pub const CHALLENGE1: &str = "challenge1";
    /// Replier random challenge (32 bytes).
    pub const CHALLENGE2: &str = "challenge2";
    /// OCSP response bytes (optional, empty = none).
    pub const OCSP_STATUS: &str = "ocsp_status";
    /// Replier/initiator signature.
    pub const SIGNATURE: &str = "signature";
}

// ----------------------------------------------------------------------
// DoS caps — spec §8.2.2 + ZeroDDS hardening
// ----------------------------------------------------------------------

/// Max token bytes (DoS cap). Spec MTU heuristic: 64 KiB.
pub const MAX_TOKEN_BYTES: usize = 65_536;
/// Max cert DER bytes (16 KiB).
pub const MAX_CERT_DER: usize = 16_384;
/// Max challenge bytes (spec is 32, but we cap at <= 64).
pub const MAX_CHALLENGE: usize = 64;
/// Max DH public key bytes (uncompressed P-256 = 65 bytes, X25519 = 32).
pub const MAX_DH_PUB: usize = 256;
/// Max property value string.
pub const MAX_PROP_VALUE: usize = 4096;
/// Max property count per DataHolder.
pub const MAX_PROPS: usize = 64;
/// Max binary-property count per DataHolder.
pub const MAX_BIN_PROPS: usize = 64;
/// Max signature bytes (RSA-PSS-2048 = 256, ECDSA-P256-ASN.1 ≤ 72).
pub const MAX_SIGNATURE: usize = 1024;

// ----------------------------------------------------------------------
// Hash-Helper
// ----------------------------------------------------------------------

/// `hash_c1`/`hash_c2` = SHA-256 of the **XCDR1-big-endian-serialized**
/// `BinaryPropertySeq{c.id, c.perm, c.pdata, c.dsign_algo, c.kagree_algo}`
/// (DDS-Security §9.3.2.5.2.1). `get_common_encoding()` is XCDR1/`ENDIAN_BIG`.
///
/// **Algo strings WITHOUT a NUL terminator** — reverse engineering of the FastDDS-fast<->
/// fast reference (openssl-verified): FastDDS hashes `c.dsign_algo`=`ECDSA-SHA256`
/// / `c.kagree_algo`=`ECDH+prime256v1-CEUM` without `\0`. ZeroDDS used to append a
/// `\0` (like cyclone/OpenDDS) -> hash_c2(with-NUL) != FastDDS recomputation(without)
/// -> reply reject (FastDDS sends 10x +Req, never +Final). cyclone stays
/// compatible: its validate path hashes the RAW received wire bytes
/// (no-NUL) and reproduces the same hash; ZeroDDS' own validate path
/// ([`compute_hash_c_raw`]) does too (vendor-agnostic). No config guard needed.
#[must_use]
pub fn compute_hash_c(
    c_id: &[u8],
    c_perm: &[u8],
    c_pdata: &[u8],
    c_dsign_algo: &str,
    c_kagree_algo: &str,
) -> [u8; 32] {
    compute_hash_c_raw(
        c_id,
        c_perm,
        c_pdata,
        c_dsign_algo.as_bytes(),
        c_kagree_algo.as_bytes(),
    )
}

/// Like [`compute_hash_c`], but the algo values are hashed **raw** (exactly as on
/// the wire) — NO normalization / null appending. The validate path
/// MUST use this variant with the raw received `c.dsign_algo`/`c.kagree_algo`
/// value bytes: cyclone/OpenDDS send them null-terminated, **FastDDS
/// WITHOUT a NUL** — and each vendor hashes its own raw bytes. If
/// ZeroDDS normalizes on validation (strips + appends a NUL), FastDDS' `algo`
/// vs ZeroDDS' `algo\0` produces a hash_c1/hash_c2 mismatch (§9.3.2.3.2).
#[must_use]
pub fn compute_hash_c_raw(
    c_id: &[u8],
    c_perm: &[u8],
    c_pdata: &[u8],
    c_dsign_algo: &[u8],
    c_kagree_algo: &[u8],
) -> [u8; 32] {
    let props: [(&str, &[u8]); 5] = [
        ("c.id", c_id),
        ("c.perm", c_perm),
        ("c.pdata", c_pdata),
        ("c.dsign_algo", c_dsign_algo),
        ("c.kagree_algo", c_kagree_algo),
    ];
    let d = digest::digest(&digest::SHA256, &serialize_bin_prop_seq(&props));
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

/// XCDR1-big-endian serialization of a `BinaryPropertySeq` (DDS-Security
/// §9.3.2.5.2.1): length as a BE u32, then each `BinaryProperty_t = { string
/// name; sequence<octet> value; }`; `propagate` is **not** serialized.
/// Each length field is 4-byte-aligned (relative to the stream start, no
/// encapsulation header). Byte-identical to OpenDDS' common encoding
/// (XCDR1/`ENDIAN_BIG`) — the same serializer for `hash_c1`/`hash_c2` AND
/// the signature content (§9.3.2.5.2.2). A simple concatenation produced
/// cross-vendor "hash_c1 invalid" or signature verify errors.
#[must_use]
pub fn serialize_bin_prop_seq(props: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    buf.extend_from_slice(&(props.len() as u32).to_be_bytes());
    for (name, value) in props {
        align4(&mut buf);
        buf.extend_from_slice(&(name.len() as u32 + 1).to_be_bytes()); // incl. NUL
        buf.extend_from_slice(name.as_bytes());
        buf.push(0);
        align4(&mut buf);
        buf.extend_from_slice(&(value.len() as u32).to_be_bytes());
        buf.extend_from_slice(value);
    }
    buf
}

/// Padding to 4-byte alignment (XCDR1).
fn align4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

/// Signature content of the **HandshakeReplyMessageToken** (DDS-Security
/// §9.3.2.5.2.2): the replier signs (ECDSA/RSA) over the XCDR1-BE-
/// serialized `BinaryPropertySeq{ hash_c2, challenge2, dh2, challenge1,
/// dh1, hash_c1 }`. `hash_c1`/`hash_c2` are **in** the signed content, even
/// if cyclone omits them from the transmitted token (both sides
/// recompute them). Order + property names are byte-identical to
/// OpenDDS' `make_handshake_reply`.
#[must_use]
pub fn reply_signing_bytes(
    hash_c2: &[u8],
    challenge2: &[u8],
    dh2: &[u8],
    challenge1: &[u8],
    dh1: &[u8],
    hash_c1: &[u8],
) -> Vec<u8> {
    serialize_bin_prop_seq(&[
        (prop::HASH_C2, hash_c2),
        (prop::CHALLENGE2, challenge2),
        (prop::DH2, dh2),
        (prop::CHALLENGE1, challenge1),
        (prop::DH1, dh1),
        (prop::HASH_C1, hash_c1),
    ])
}

/// Signature content of the **HandshakeFinalMessageToken** (DDS-Security
/// §9.3.2.5.2.3): the initiator signs over the XCDR1-BE-serialized
/// `BinaryPropertySeq{ hash_c1, challenge1, dh1, challenge2, dh2, hash_c2 }`
/// (mirrored order compared to the reply). Byte-identical to OpenDDS'
/// `make_handshake_final`.
#[must_use]
pub fn final_signing_bytes(
    hash_c1: &[u8],
    challenge1: &[u8],
    dh1: &[u8],
    challenge2: &[u8],
    dh2: &[u8],
    hash_c2: &[u8],
) -> Vec<u8> {
    serialize_bin_prop_seq(&[
        (prop::HASH_C1, hash_c1),
        (prop::CHALLENGE1, challenge1),
        (prop::DH1, dh1),
        (prop::CHALLENGE2, challenge2),
        (prop::DH2, dh2),
        (prop::HASH_C2, hash_c2),
    ])
}

// ----------------------------------------------------------------------
// Token builders
// ----------------------------------------------------------------------

/// Parsed view of a request token (initiator → replier).
#[derive(Debug, Clone)]
pub struct RequestTokenView {
    /// Initiator identity cert (DER).
    pub cert_der: Vec<u8>,
    /// Permissions document (may be empty).
    pub permissions: Vec<u8>,
    /// ParticipantBuiltinTopicData (opaque).
    pub pdata: Vec<u8>,
    /// Digital signature algorithm (e.g. `"ECDSA-SHA256"`).
    pub dsign_algo: String,
    /// Key agreement algorithm (e.g. `"X25519"`).
    pub kagree_algo: String,
    /// SHA-256 over the 5 properties above.
    pub hash_c1: [u8; 32],
    /// Initiator DH public key.
    pub dh1: Vec<u8>,
    /// Initiator random challenge (32 bytes).
    pub challenge1: [u8; 32],
    /// OCSP response bytes (may be empty).
    pub ocsp_status: Vec<u8>,
}

/// Parsed view of a reply token (replier → initiator).
#[derive(Debug, Clone)]
pub struct ReplyTokenView {
    /// Replier identity cert (DER).
    pub cert_der: Vec<u8>,
    /// Replier permissions.
    pub permissions: Vec<u8>,
    /// Replier pdata.
    pub pdata: Vec<u8>,
    /// Replier digital signature algo.
    pub dsign_algo: String,
    /// Replier key agreement algo.
    pub kagree_algo: String,
    /// SHA-256 over the replier properties (recomputed from the reply; cyclone/
    /// FastDDS do not send `hash_c2` in the reply, the initiator computes it).
    pub hash_c2: [u8; 32],
    /// Echo of the initiator `hash_c1` — OPTIONAL: cyclone/FastDDS omit it
    /// (the initiator knows its own value), `None` = not sent.
    pub hash_c1: Option<[u8; 32]>,
    /// Replier DH public key.
    pub dh2: Vec<u8>,
    /// Echo of the initiator DH public — OPTIONAL (see `hash_c1`).
    pub dh1: Option<Vec<u8>>,
    /// Replier challenge.
    pub challenge2: [u8; 32],
    /// Echo of the initiator challenge.
    pub challenge1: [u8; 32],
    /// Replier OCSP status.
    pub ocsp_status: Vec<u8>,
    /// Replier signature over `reply_signing_bytes` (§9.3.2.5.2.2).
    pub signature: Vec<u8>,
}

/// Parsed view of a final token (initiator → replier).
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
    /// Initiator OCSP status.
    pub ocsp_status: Vec<u8>,
    /// Initiator signature over `final_signing_bytes` (§9.3.2.5.2.3).
    pub signature: Vec<u8>,
}

/// Inputs for building the request token.
pub struct RequestBuildInput<'a> {
    /// Cert DER.
    pub cert_der: &'a [u8],
    /// Permissions (may be empty).
    pub permissions: &'a [u8],
    /// Pdata (opaque).
    pub pdata: &'a [u8],
    /// `c.dsign_algo` value.
    pub dsign_algo: &'a str,
    /// `c.kagree_algo` value.
    pub kagree_algo: &'a str,
    /// Initiator DH public.
    pub dh1: &'a [u8],
    /// Initiator challenge (32 bytes).
    pub challenge1: &'a [u8; 32],
    /// OCSP response bytes (may be empty).
    pub ocsp_status: &'a [u8],
}

/// Builds the wire bytes of a request token via DataHolder.
///
/// # Errors
/// If DoS caps are violated (cert DER > 16 KiB, DH > 256 bytes, ...).
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
    // IMPORTANT (cross-vendor): the `c.*` properties MUST go on the wire in the
    // same order as in the hash (c.id, c.perm, c.pdata,
    // c.dsign_algo, c.kagree_algo) — FastDDS recomputes hash_c1 over the
    // `c.*` properties in **wire order** (PKIDH.cpp:
    // addBinaryPropertySeq(..., "c.", false)). A different wire order
    // (e.g. algos first) produces a different hash_c1 for FastDDS → echo/
    // signature mismatch in the reply. c.dsign_algo/c.kagree_algo are BINARY
    // properties (spec §9.3.2.5.2.1; value = null-terminated UTF-8 algo bytes).
    h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
    h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
    h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
    h.set_binary_property(prop::C_DSIGN_ALGO, input.dsign_algo.as_bytes().to_vec());
    h.set_binary_property(prop::C_KAGREE_ALGO, input.kagree_algo.as_bytes().to_vec());
    h.set_binary_property(prop::HASH_C1, hash_c1.to_vec());
    h.set_binary_property(prop::DH1, input.dh1.to_vec());
    h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
    // Do NOT emit ocsp_status (spec-optional): FastDDS does not send it and
    // validates more strictly; cyclone tolerates its absence. Spec §9.3.2.x optional.
    Ok(h.to_cdr_le())
}

/// Parses the wire bytes of a request token and re-validates `hash_c1`.
///
/// # Errors
/// `AuthenticationFailed` if `hash_c1` does not match the other
/// properties; `BadArgument` on decode/cap errors.
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
    let dsign_algo = take_bin_string(&h, prop::C_DSIGN_ALGO)?;
    let kagree_algo = take_bin_string(&h, prop::C_KAGREE_ALGO)?;
    let dh1 = take_bin(&h, prop::DH1, MAX_DH_PUB)?;
    let challenge1 = take_fixed::<32>(&h, prop::CHALLENGE1)?;
    let ocsp_status = h.binary_property(prop::OCSP_STATUS).unwrap_or(&[]).to_vec();

    // Recompute hash_c1 over the RAW wire value bytes of the algo properties
    // (the sender hashes its own raw bytes; FastDDS without NUL, cyclone
    // with) — otherwise a cross-vendor hash_c1 mismatch.
    let dsign_raw = h.binary_property(prop::C_DSIGN_ALGO).unwrap_or(&[]);
    let kagree_raw = h.binary_property(prop::C_KAGREE_ALGO).unwrap_or(&[]);
    let recomputed = compute_hash_c_raw(&cert_der, &permissions, &pdata, dsign_raw, kagree_raw);
    // OMG DDS-Security §9.3.2.3.1: `hash_c1` is OPTIONAL in the HandshakeRequest.
    // cyclone/FastDDS do NOT send it (optimization) → the responder computes
    // it itself (was F-RESPONDER: every foreign initiator → zerodds failed with
    // "missing binary property: hash_c1"). If it is PRESENT, check it against the
    // recompute (protection against a MitM that rewrites the properties without a cert swap).
    let hash_c1 = match take_fixed::<32>(&h, prop::HASH_C1) {
        Ok(received) => {
            if !ct_eq(&recomputed, &received) {
                return Err(SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "request: hash_c1 mismatch (token tampered)",
                ));
            }
            received
        }
        Err(_) => recomputed,
    };

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

/// Inputs for building the reply token.
pub struct ReplyBuildInput<'a> {
    /// Replier cert DER.
    pub cert_der: &'a [u8],
    /// Replier permissions.
    pub permissions: &'a [u8],
    /// Replier pdata.
    pub pdata: &'a [u8],
    /// Replier `c.dsign_algo`.
    pub dsign_algo: &'a str,
    /// Replier `c.kagree_algo` (echo of the initiator value).
    pub kagree_algo: &'a str,
    /// Replier DH public.
    pub dh2: &'a [u8],
    /// Replier challenge.
    pub challenge2: &'a [u8; 32],
    /// Echo `hash_c1`.
    pub hash_c1: &'a [u8; 32],
    /// Echo `dh1`.
    pub dh1: &'a [u8],
    /// Echo `challenge1`.
    pub challenge1: &'a [u8; 32],
    /// OCSP status.
    pub ocsp_status: &'a [u8],
    /// Replier signature over `reply_signing_bytes`.
    pub signature: &'a [u8],
}

/// Builds the wire bytes of the reply token.
///
/// # Errors
/// DoS cap violation.
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
    // `c.*` in hash order (see build_request_token) — cross-vendor.
    h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
    h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
    h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
    h.set_binary_property(prop::C_DSIGN_ALGO, input.dsign_algo.as_bytes().to_vec());
    h.set_binary_property(prop::C_KAGREE_ALGO, input.kagree_algo.as_bytes().to_vec());
    // Wire property pairs in (1,2) order — exactly like FastDDS' reply. The
    // SIGNATURE content stays in spec order (2,1, reply_signing_bytes) unchanged;
    // only the wire order changes (FastDDS may parse positionally).
    // cyclone parses by name (accepted (2,1)) -> stays compatible.
    h.set_binary_property(prop::HASH_C1, input.hash_c1.to_vec());
    h.set_binary_property(prop::HASH_C2, hash_c2.to_vec());
    h.set_binary_property(prop::DH1, input.dh1.to_vec());
    h.set_binary_property(prop::DH2, input.dh2.to_vec());
    h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
    h.set_binary_property(prop::CHALLENGE2, input.challenge2.to_vec());
    // Do NOT emit ocsp_status (spec-optional): FastDDS does not send it and
    // validates more strictly; cyclone tolerates its absence. Spec §9.3.2.x optional.
    h.set_binary_property(prop::SIGNATURE, input.signature.to_vec());
    Ok(h.to_cdr_le())
}

/// Parses the reply token. Recomputes hash_c2.
///
/// # Errors
/// Decode/cap/hash mismatch.
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
    let dsign_algo = take_bin_string(&h, prop::C_DSIGN_ALGO)?;
    let kagree_algo = take_bin_string(&h, prop::C_KAGREE_ALGO)?;
    let dh2 = take_bin(&h, prop::DH2, MAX_DH_PUB)?;
    let challenge2 = take_fixed::<32>(&h, prop::CHALLENGE2)?;
    let challenge1 = take_fixed::<32>(&h, prop::CHALLENGE1)?;
    let ocsp_status = h.binary_property(prop::OCSP_STATUS).unwrap_or(&[]).to_vec();
    let signature = take_bin(&h, prop::SIGNATURE, MAX_SIGNATURE)?;

    // hash_c2 is OPTIONAL (cyclone/FastDDS do not send it): recompute it from the
    // replier credentials. If present, check it against the recompute.
    // Over the RAW wire value bytes of the algo properties (FastDDS appends
    // no NUL + hashes raw; cyclone with NUL) — otherwise a hash_c2 mismatch.
    let dsign_raw = h.binary_property(prop::C_DSIGN_ALGO).unwrap_or(&[]);
    let kagree_raw = h.binary_property(prop::C_KAGREE_ALGO).unwrap_or(&[]);
    let recomputed = compute_hash_c_raw(&cert_der, &permissions, &pdata, dsign_raw, kagree_raw);
    if let Ok(sent) = take_fixed::<32>(&h, prop::HASH_C2) {
        if !ct_eq(&recomputed, &sent) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "reply: hash_c2 mismatch",
            ));
        }
    }
    let hash_c2 = recomputed;
    // hash_c1 / dh1 are echo fields — OPTIONAL (the initiator knows them itself).
    let hash_c1 = take_fixed::<32>(&h, prop::HASH_C1).ok();
    let dh1 = h
        .binary_property(prop::DH1)
        .filter(|b| b.len() <= MAX_DH_PUB)
        .map(<[u8]>::to_vec);

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

/// Inputs for building the final token.
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
    /// Initiator OCSP status.
    pub ocsp_status: &'a [u8],
    /// Initiator signature over `final_signing_bytes`.
    pub signature: &'a [u8],
}

/// Builds the wire bytes of the final token.
///
/// # Errors
/// DoS cap.
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
    // Do NOT emit ocsp_status (spec-optional): FastDDS does not send it and
    // validates more strictly; cyclone tolerates its absence. Spec §9.3.2.x optional.
    h.set_binary_property(prop::SIGNATURE, input.signature.to_vec());
    Ok(h.to_cdr_le())
}

/// Parses the final token.
///
/// # Errors
/// Decode/cap.
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
// Lookup helpers
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
    // Note: input.challenge1 is `&[u8; 32]` (statically fixed). The
    // MAX_CHALLENGE=64 cap cannot be reached — a historical
    // defensive check, before the change to a fixed array. Removed, because
    // as an always-false branch it led to equivalent mutations.
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

/// Reads a BINARY property as a UTF-8 string (c.dsign_algo / c.kagree_algo
/// travel as a binary_property with the algo name as UTF-8 bytes).
fn take_bin_string(h: &DataHolder, key: &str) -> SecurityResult<String> {
    let mut bytes = take_bin(h, key, 256)?;
    // The wire value is null-terminated (OpenDDS/cyclone convention) — strip it.
    if bytes.last() == Some(&0) {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("binary property {key} not valid UTF-8"),
        )
    })
}

/// Constant-time comparison (against timing side channels).
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
    fn parse_reply_accepts_fastdds_style_non_null_terminated_algos() {
        // FastDDS appends NO null terminator to c.dsign_algo/c.kagree_algo
        // and hashes the RAW wire value bytes. ZeroDDS must also recompute
        // hash_c2 over the raw bytes (compute_hash_c_raw). The old
        // normalizing variant (strip + append NUL) produced against FastDDS
        // "reply: hash_c2 mismatch" → handshake abort, no crypto tokens.
        let cert = alloc::vec![0xAB; 16];
        let perm = alloc::vec![0xCD; 8];
        let pdata = alloc::vec![0xEF; 12];
        let dsign = b"ECDSA-SHA256"; // NO NUL (FastDDS style)
        let kagree = b"ECDH+prime256v1-CEUM"; // NO NUL
        // hash_c2 like FastDDS: over the raw (non-null-terminated) values.
        let hash_c2 = compute_hash_c_raw(&cert, &perm, &pdata, dsign, kagree);

        let mut h = DataHolder::new(class_id::REPLY);
        h.set_binary_property(prop::C_DSIGN_ALGO, dsign.to_vec());
        h.set_binary_property(prop::C_KAGREE_ALGO, kagree.to_vec());
        h.set_binary_property(prop::C_ID, cert.clone());
        h.set_binary_property(prop::C_PERM, perm.clone());
        h.set_binary_property(prop::C_PDATA, pdata.clone());
        h.set_binary_property(prop::HASH_C2, hash_c2.to_vec());
        h.set_binary_property(prop::DH2, alloc::vec![1, 2, 3, 4]);
        h.set_binary_property(prop::CHALLENGE2, alloc::vec![0u8; 32]);
        h.set_binary_property(prop::CHALLENGE1, alloc::vec![1u8; 32]);
        h.set_binary_property(prop::SIGNATURE, alloc::vec![9u8; 16]);
        let bytes = h.to_cdr_le();

        let view = parse_reply_token(&bytes)
            .expect("FastDDS-style reply (algo without NUL) must validate hash_c2");
        assert_eq!(view.dsign_algo, "ECDSA-SHA256");
        assert_eq!(view.kagree_algo, "ECDH+prime256v1-CEUM");
    }

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
        // Different splitting of the same bytes -> different hash.
        let h3 = compute_hash_c(b"ab", b"", b"c", "d", "e");
        assert_ne!(h1, h3);
    }

    #[test]
    fn signing_bytes_deterministic_and_input_separated() {
        // The reply content is deterministic + reacts to a change in every field.
        let s1 = ht_reply(&[0x11; 32], &[0x22; 32], &[0x33; 32]);
        let s2 = ht_reply(&[0x11; 32], &[0x22; 32], &[0x33; 32]);
        assert_eq!(s1, s2);
        let s3 = ht_reply(&[0x11; 32], &[0x22; 32], &[0x34; 32]);
        assert_ne!(s1, s3);
    }

    #[test]
    fn reply_and_final_content_differ_by_order() {
        // Reply and final use the same values, but a mirrored
        // property order -> different signed bytes (prevents
        // reply<->final signature reuse).
        let (hc1, ch1, dh1, hc2, ch2, dh2) = (
            &[1u8; 32], &[2u8; 32], &[3u8; 32], &[4u8; 32], &[5u8; 32], &[6u8; 32],
        );
        let r = reply_signing_bytes(hc2, ch2, dh2, ch1, dh1, hc1);
        let f = final_signing_bytes(hc1, ch1, dh1, ch2, dh2, hc2);
        assert_ne!(r, f);
    }

    fn ht_reply(hash_c2: &[u8], challenge2: &[u8], dh2: &[u8]) -> Vec<u8> {
        reply_signing_bytes(
            hash_c2,
            challenge2,
            dh2,
            &[0xAA; 32],
            &[0xBB; 32],
            &[0xCC; 32],
        )
    }

    // -------------------------------------------------------------
    // Mutation killers (2026-05-01) — cap boundary tests
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

    /// Catches `>` -> `==`/`>=` on the cap_check_request.cert_der cap.
    /// cert_der EXACTLY MAX_CERT_DER must pass, MAX_CERT_DER+1 must error.
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

    /// dh1 cap on cap_check_request.
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

    /// §9.3.2.3.1: `hash_c1` is OPTIONAL in the HandshakeRequest. cyclone/FastDDS
    /// do NOT send it (optimization); the responder MUST then compute it from the
    /// other properties itself instead of erroring with "missing binary property"
    /// (was F-RESPONDER: every foreign initiator → zerodds responder failed).
    #[test]
    fn parse_request_token_computes_hash_c1_when_absent() {
        let cert = vec![7u8; 64];
        let dh = vec![3u8; 32];
        let ch = [9u8; 32];
        let input = make_request_input(&cert, &dh, &ch);
        // Build the DataHolder like build_request_token, but WITHOUT hash_c1.
        let mut h = DataHolder::new(class_id::REQUEST);
        h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
        h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
        h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
        h.set_binary_property(prop::C_DSIGN_ALGO, input.dsign_algo.as_bytes().to_vec());
        h.set_binary_property(prop::C_KAGREE_ALGO, input.kagree_algo.as_bytes().to_vec());
        h.set_binary_property(prop::DH1, input.dh1.to_vec());
        h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
        let bytes = h.to_cdr_le();

        let view = parse_request_token(&bytes).expect("request without hash_c1 must parse");
        let dsign_raw = input.dsign_algo.as_bytes().to_vec();
        let kagree_raw = input.kagree_algo.as_bytes().to_vec();
        let expected = compute_hash_c_raw(
            input.cert_der,
            input.permissions,
            input.pdata,
            &dsign_raw,
            &kagree_raw,
        );
        assert_eq!(view.hash_c1, expected, "hash_c1 must be self-computed");
    }

    /// Counter-check: if hash_c1 is PRESENT but wrong → still a tamper reject.
    #[test]
    fn parse_request_token_rejects_tampered_hash_c1() {
        let cert = vec![7u8; 64];
        let dh = vec![3u8; 32];
        let ch = [9u8; 32];
        let input = make_request_input(&cert, &dh, &ch);
        let mut h = DataHolder::new(class_id::REQUEST);
        h.set_binary_property(prop::C_ID, input.cert_der.to_vec());
        h.set_binary_property(prop::C_PERM, input.permissions.to_vec());
        h.set_binary_property(prop::C_PDATA, input.pdata.to_vec());
        h.set_binary_property(prop::C_DSIGN_ALGO, input.dsign_algo.as_bytes().to_vec());
        h.set_binary_property(prop::C_KAGREE_ALGO, input.kagree_algo.as_bytes().to_vec());
        h.set_binary_property(prop::HASH_C1, vec![0xFFu8; 32]); // wrong
        h.set_binary_property(prop::DH1, input.dh1.to_vec());
        h.set_binary_property(prop::CHALLENGE1, input.challenge1.to_vec());
        let bytes = h.to_cdr_le();
        let err = parse_request_token(&bytes).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    /// build_final_token: dh1 + dh2 EXACTLY at MAX_DH_PUB must pass.
    /// Catches the `>` -> `>=` mutation on the dh check (line 477).
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

    /// cap_check_reply.cert_der cap boundary.
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

    /// cap_check_reply: `||` -> `&&` on `dh1>cap || dh2>cap`.
    /// Per side (dh1 over, dh2 ok) and (dh1 ok, dh2 over) must reject.
    /// With `&&` only (BOTH over) would reject.
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

    /// signature cap.
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

    /// build_final_token: signature cap boundary.
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

    /// build_final_token: dh1/dh2 cap check per side (`||` -> `&&`).
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

    /// take_bin: max boundary. v.len() == max must pass,
    /// v.len() > max must error.
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
