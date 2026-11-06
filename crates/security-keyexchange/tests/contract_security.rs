// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-keyexchange` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! zerodds-lint: allow no_dyn_in_safe — the crate holds the plugin and
//! logger interfaces of DDS-Security as trait objects; the contract holds
//! them the same way.
//!
//! What this crate uses of `zerodds-security`, from its non-test sources
//! (`lib.rs`, `modp.rs`):
//! - `error::{SecurityError, SecurityErrorKind, SecurityResult}`:
//!   `KeyExchange::new`, `KeyExchange::with_suite` and
//!   `KeyExchange::derive_shared_secret` return `SecurityResult<_>`, and
//!   every failure is a `SecurityError::new(kind, detail)` with a
//!   `&'static str` or a formatted `String` as detail.
//! - The kinds `CryptoFailed` (RNG, key generation, rejected or degenerate
//!   agreement, the MODP checks in `modp.rs`), `BadArgument` (a peer key of
//!   the wrong length) and `Internal` (a curve key for a finite-field suite).
//!
//! What it relies on: the error carries the three attributes of the
//! `SecurityException` (message, code, minor_code), the kind it was built
//! with, and `code` follows the kind, so that `zerodds-security-pki` and the
//! runtime can tell a bad peer key from a failed agreement by `kind` alone.
//!
//! Oracles: DDS-Security 1.2 §9.2.1 Table 28 (`SecurityException` with
//! `message: String`, `code: long`, `minor_code: long`) and the rustdoc of
//! `SecurityErrorKind::code` ("1 to 7 in the order of the variants;
//! stable") and of `SecurityError::new` ("`code` is that of the category,
//! `minor_code` 0").
#![allow(clippy::unwrap_used)]

use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

#[test]
fn security_9_2_1_an_exception_holds_message_code_and_minor_code() {
    // A static detail, as most failures of this crate carry.
    let e = SecurityError::new(
        SecurityErrorKind::CryptoFailed,
        "keyexchange: system RNG unavailable",
    );
    assert_eq!(e.kind, SecurityErrorKind::CryptoFailed);
    assert_eq!(e.detail, "keyexchange: system RNG unavailable");
    assert_eq!(e.code, SecurityErrorKind::CryptoFailed.code());
    assert_eq!(e.minor_code, 0);

    // A formatted detail, as the length check of `derive_shared_secret`
    // builds it.
    let len = 65;
    let detail = format!("keyexchange: EcdhP256 public key must be {len} bytes");
    let e = SecurityError::new(SecurityErrorKind::BadArgument, detail.clone());
    assert_eq!(e.detail, detail.as_str());
    assert_eq!(e.code, SecurityErrorKind::BadArgument.code());
    assert_eq!(e.minor_code, 0);

    // Table 28 types `code` and `minor_code` as IDL `long`.
    let _: i32 = e.code;
    let _: i32 = e.minor_code;
}

#[test]
fn the_kinds_this_crate_reports_keep_their_documented_codes() {
    // Rustdoc of `SecurityErrorKind::code`: 1 to 7 in the order of the
    // variants (AuthenticationFailed, AccessDenied, CryptoFailed,
    // InvalidConfiguration, BadArgument, NotImplemented, Internal).
    assert_eq!(SecurityErrorKind::CryptoFailed.code(), 3);
    assert_eq!(SecurityErrorKind::BadArgument.code(), 5);
    assert_eq!(SecurityErrorKind::Internal.code(), 7);
    // The three are distinct, so a caller can match on any of them.
    assert_ne!(
        SecurityErrorKind::CryptoFailed,
        SecurityErrorKind::BadArgument
    );
    assert_ne!(SecurityErrorKind::BadArgument, SecurityErrorKind::Internal);
}

#[test]
fn a_security_result_passes_the_error_unchanged_through_the_question_mark() {
    fn agree(peer: &[u8]) -> SecurityResult<Vec<u8>> {
        if peer.len() != 65 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "keyexchange: wrong public key length",
            ));
        }
        Ok(peer[1..33].to_vec())
    }
    fn caller(peer: &[u8]) -> SecurityResult<usize> {
        Ok(agree(peer)?.len())
    }

    assert_eq!(caller(&[0x04; 65]).unwrap(), 32);
    let err = caller(&[0x04; 64]).unwrap_err();
    assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    assert_eq!(
        err,
        SecurityError::new(
            SecurityErrorKind::BadArgument,
            "keyexchange: wrong public key length"
        )
    );
}

#[test]
fn the_error_is_a_std_error_for_callers_that_box_it() {
    // The crate builds `zerodds-security` with its default `std` feature;
    // callers in `std` code turn the error into `Box<dyn Error>`.
    let boxed: Box<dyn std::error::Error> = Box::new(SecurityError::new(
        SecurityErrorKind::Internal,
        "keyexchange: a curve key for a finite-field suite",
    ));
    let back = boxed.downcast_ref::<SecurityError>().unwrap();
    assert_eq!(back.kind, SecurityErrorKind::Internal);
}
