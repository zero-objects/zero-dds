// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-rtps` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources:
//! - `crypto::CryptographicPlugin`, taken as `&dyn CryptographicPlugin` by
//!   `encode_secured_submessage` and `decode_secured_submessage`
//!   (`codec.rs`) and by `encode_secured_rtps_message`,
//!   `encode_secured_rtps_message_psk` and `decode_secured_rtps_message`
//!   (`srtps.rs`); of it only `encrypt_submessage(local, remote_list,
//!   plaintext, aad_extension)` and `decrypt_submessage(local, remote,
//!   ciphertext, aad_extension)`. The crate doc of `lib.rs` names the root
//!   re-export `zerodds_security::CryptographicPlugin`.
//! - `crypto::CryptoHandle`, passed through unchanged.
//! - `error::SecurityError`, wrapped in `SecurityRtpsError::Crypto`, which
//!   derives `Debug`, `Clone`, `PartialEq` and `Eq` and prints the error
//!   with `Display`.
//!
//! What it relies on: the two transform operations are callable through a
//! trait object, receive the handles, the bytes and the AAD extension the
//! wrapper built, and report a failed verification as an `Err` that the
//! wrapper can keep, compare and print.
//!
//! Oracles: DDS-Security 1.2 §9.5.1.10 (the transform operations of the
//! Cryptographic plugin; the AAD of §7.4.6.6 and §9.5.1.10.2 covers the
//! header bytes the caller passes), §9.2.1 Table 28 (`SecurityException`
//! with message, code and minor code), and the rustdoc of
//! `CryptographicPlugin::decrypt_submessage` ("`aad_extension` must be
//! byte-identical to the sender AAD (otherwise tag mismatch)").
#![allow(clippy::unwrap_used)]

use std::sync::Mutex;

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

/// What the plugin saw in one transform call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    local: CryptoHandle,
    remotes: Vec<CryptoHandle>,
    input: Vec<u8>,
    aad: Vec<u8>,
}

/// A plugin with the required operations only. Its "ciphertext" is the
/// AAD extension followed by the plaintext with every octet inverted; a
/// different AAD on decode is a verification failure, as with AES-GCM.
#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<Call>>,
}

impl CryptographicPlugin for Recorder {
    fn register_local_participant(
        &mut self,
        _identity: IdentityHandle,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(1))
    }

    fn register_matched_remote_participant(
        &mut self,
        _local: CryptoHandle,
        _remote_identity: IdentityHandle,
        _shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(2))
    }

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        _is_writer: bool,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        Ok(CryptoHandle(3))
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        _remote: CryptoHandle,
        _tokens: &[u8],
    ) -> SecurityResult<()> {
        Ok(())
    }

    fn encrypt_submessage(
        &self,
        local: CryptoHandle,
        remote_list: &[CryptoHandle],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.calls.lock().unwrap().push(Call {
            local,
            remotes: remote_list.to_vec(),
            input: plaintext.to_vec(),
            aad: aad_extension.to_vec(),
        });
        let mut out = aad_extension.to_vec();
        out.extend(plaintext.iter().map(|b| !b));
        Ok(out)
    }

    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.calls.lock().unwrap().push(Call {
            local,
            remotes: vec![remote],
            input: ciphertext.to_vec(),
            aad: aad_extension.to_vec(),
        });
        match ciphertext.strip_prefix(aad_extension) {
            Some(body) => Ok(body.iter().map(|b| !b).collect()),
            None => Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "tag mismatch",
            )),
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Crypto:contract"
    }
}

/// The RTPS header of §8.3.3 (20 octets) that `srtps.rs` puts behind
/// four reserved octets as the AAD extension of a whole message.
const RTPS_HEADER: [u8; 20] = [
    b'R', b'T', b'P', b'S', 2, 5, 0x01, 0x0f, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
];

#[test]
fn security_9_5_1_10_the_transform_operations_work_through_a_trait_object() {
    let recorder = Recorder::default();
    // The wrapper functions take `&dyn CryptographicPlugin`; the crate doc
    // names it by its root re-export.
    let plugin: &dyn zerodds_security::CryptographicPlugin = &recorder;

    let mut aad = vec![0u8; 4];
    aad.extend_from_slice(&RTPS_HEADER);
    let local = CryptoHandle(7);
    let remotes = [CryptoHandle(8), CryptoHandle(9)];
    let body = [0x15, 0x05, 0x0c, 0x00, 1, 2, 3, 4];

    let sealed = plugin
        .encrypt_submessage(local, &remotes, &body, &aad)
        .unwrap();
    let opened = plugin
        .decrypt_submessage(local, remotes[0], &sealed, &aad)
        .unwrap();
    assert_eq!(opened, body);

    // The handles, the bytes and the AAD extension arrive as the caller
    // passed them.
    let calls = recorder.calls.lock().unwrap();
    assert_eq!(
        calls[0],
        Call {
            local,
            remotes: remotes.to_vec(),
            input: body.to_vec(),
            aad: aad.clone(),
        }
    );
    assert_eq!(
        calls[1],
        Call {
            local,
            remotes: vec![remotes[0]],
            input: sealed,
            aad,
        }
    );
    assert_eq!(plugin.plugin_class_id(), "DDS:Crypto:contract");
}

#[test]
fn security_9_2_1_a_failed_verification_is_an_error_the_caller_can_keep_and_compare() {
    let recorder = Recorder::default();
    let plugin: &dyn CryptographicPlugin = &recorder;
    let mut aad = vec![0u8; 4];
    aad.extend_from_slice(&RTPS_HEADER);
    let sealed = plugin
        .encrypt_submessage(CryptoHandle(1), &[CryptoHandle(2)], b"submessage", &aad)
        .unwrap();

    // A changed GUID prefix in the header changes the AAD: the rustdoc of
    // `decrypt_submessage` makes that a tag mismatch.
    let mut tampered = aad.clone();
    tampered[4 + 8] ^= 0xff;
    let err = plugin
        .decrypt_submessage(CryptoHandle(1), CryptoHandle(2), &sealed, &tampered)
        .unwrap_err();
    assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    assert_eq!(err.code, SecurityErrorKind::CryptoFailed.code());
    assert_eq!(err.minor_code, 0);
    assert_eq!(err.detail, "tag mismatch");

    // `SecurityRtpsError::Crypto` derives Clone and Eq over it and prints it.
    let kept = err.clone();
    assert_eq!(kept, err);
    assert!(!format!("{err}").is_empty());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn crypto_handles_are_copied_and_compared_by_their_number() {
    // `CryptoHandle` is a public newtype over `u64`; the wrapper copies it
    // into every call.
    let a = CryptoHandle(42);
    let b = a;
    assert_eq!(a, b);
    assert_eq!(b.0, 42);
    assert_ne!(CryptoHandle(1), CryptoHandle(2));
    assert!(CryptoHandle(1) < CryptoHandle(2));
}
