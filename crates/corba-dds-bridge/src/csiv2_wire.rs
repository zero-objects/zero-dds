// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CSIv2 (Common Secure Interoperability v2) Wire-Hooks fuer den
//! Bridge-Daemon.
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §7.2 (= CORBA Spec §24).
//!
//! CSIv2 ist eine zwei-stufige Authentifizierung:
//! 1. **Transport-Layer (TLS)** — bereits ueber `bridge-security`-
//!    rustls-Wireup abgedeckt.
//! 2. **SAS_ContextElement** — Username/Password-Token
//!    (`GssupCredentialToken`) im IIOP-`ServiceContext` mit
//!    `SecurityAttributeService`-ID 15 (Spec §24.7.2).
//!
//! Diese Schicht haengt das `corba-csiv2`-GSSUP-Encoding direkt an die
//! `ServiceContext`-Liste eines GIOP-`Request`/`Reply` an. Die
//! Daemon-Layer kann den Token ausserhalb der Wire-Schicht generieren
//! (z.B. aus `--sasl-user`/`--sasl-pass`-Flags).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::Endianness;
use zerodds_corba_csiv2::{GssupCredentialToken, INITIAL_CONTEXT_TOKEN_TAG};

/// `service_context_id` fuer SAS — Spec §24.7.2.
pub const SECURITY_ATTRIBUTE_SERVICE_ID: u32 = 15;

/// Baut einen `(service_context_id, context_data)`-Eintrag fuer einen
/// `GssupCredentialToken`, der direkt in den `ServiceContext`-Liste
/// eines GIOP-Frames angehaengt werden kann.
///
/// # Errors
/// `gssup_encode_failed` wenn der GSSUP-Codec fehlschlaegt.
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
    // Token-Tag ist Teil der ContextData (Spec §24.7.2.1
    // SASContextBody → Header 0xa = `EstablishContext`).
    let _ = INITIAL_CONTEXT_TOKEN_TAG;
    Ok((SECURITY_ATTRIBUTE_SERVICE_ID, body))
}

/// Extrahiere SAS-Token aus einer `ServiceContext`-Liste.
///
/// Liefert `Some(GssupCredentialToken)` wenn ein Eintrag mit
/// `service_context_id == 15` und gueltigem GSSUP-Body vorhanden ist.
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
