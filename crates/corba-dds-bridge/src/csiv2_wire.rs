// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CSIv2 (Common Secure Interoperability v2) wire hooks for the
//! bridge daemon.
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §7.2 (= CORBA Spec §24).
//!
//! CSIv2 is a two-stage authentication:
//! 1. **Transport layer (TLS)** — already covered by the
//!    `bridge-security` rustls wire-up.
//! 2. **SAS_ContextElement** — a username/password token
//!    (`GssupCredentialToken`) in the IIOP `ServiceContext` with
//!    `SecurityAttributeService` ID 15 (Spec §24.7.2).
//!
//! This layer appends the `corba-csiv2` GSSUP encoding directly to the
//! `ServiceContext` list of a GIOP `Request`/`Reply`. The daemon layer
//! can generate the token outside the wire layer (e.g. from
//! `--sasl-user`/`--sasl-pass` flags).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::Endianness;
use zerodds_corba_csiv2::{GssupCredentialToken, INITIAL_CONTEXT_TOKEN_TAG};

/// `service_context_id` for SAS — Spec §24.7.2.
pub const SECURITY_ATTRIBUTE_SERVICE_ID: u32 = 15;

/// Builds a `(service_context_id, context_data)` entry for a
/// `GssupCredentialToken` that can be appended directly to the
/// `ServiceContext` list of a GIOP frame.
///
/// # Errors
/// `gssup_encode_failed` if the GSSUP codec fails.
pub fn build_sas_context(
    username: String,
    password: String,
    target_name: Vec<u8>,
    endianness: Endianness,
) -> Result<(u32, Vec<u8>), &'static str> {
    let token = GssupCredentialToken::new(username, password, target_name);
    let body = token
        .encode_encapsulation(endianness)
        .map_err(|_| "gssup_encode_failed")?;
    // The token tag is part of the ContextData (Spec §24.7.2.1
    // SASContextBody → header 0xa = `EstablishContext`).
    let _ = INITIAL_CONTEXT_TOKEN_TAG;
    Ok((SECURITY_ATTRIBUTE_SERVICE_ID, body))
}

/// Extracts the SAS token from a `ServiceContext` list.
///
/// Returns `Some(GssupCredentialToken)` if an entry with
/// `service_context_id == 15` and a valid GSSUP body is present.
#[must_use]
pub fn extract_sas_context(contexts: &[(u32, Vec<u8>)]) -> Option<GssupCredentialToken> {
    for (id, data) in contexts {
        if *id == SECURITY_ATTRIBUTE_SERVICE_ID {
            if let Ok(t) = GssupCredentialToken::decode_encapsulation(data) {
                return Some(t);
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_then_extract_round_trips_username() {
        let (id, body) = build_sas_context(
            "alice".into(),
            "secret".into(),
            b"target.example.com".to_vec(),
            Endianness::Little,
        )
        .expect("build");
        assert_eq!(id, SECURITY_ATTRIBUTE_SERVICE_ID);
        let contexts = vec![(id, body)];
        let token = extract_sas_context(&contexts).expect("token");
        assert_eq!(token.username, "alice");
        assert_eq!(token.password, "secret");
    }

    #[test]
    fn extract_returns_none_when_no_sas_context() {
        let contexts: Vec<(u32, Vec<u8>)> = vec![(1, vec![0; 4])];
        assert!(extract_sas_context(&contexts).is_none());
    }

    #[test]
    fn extract_skips_corrupted_body() {
        let contexts = vec![(SECURITY_ATTRIBUTE_SERVICE_ID, vec![0xff; 3])];
        assert!(extract_sas_context(&contexts).is_none());
    }

    #[test]
    fn id_constant_is_fifteen_per_spec() {
        // CORBA Spec §24.7.2 — SecurityAttributeService = 15.
        assert_eq!(SECURITY_ATTRIBUTE_SERVICE_ID, 15);
    }
}
