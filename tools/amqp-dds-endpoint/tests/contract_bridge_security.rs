// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `amqp-dds-endpoint` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `tls_provider` (`src/tls.rs`): the rustls provider of the TLS bracket.
//!
//! Oracles: RFC 8446 §9.1 (TLS_AES_128_GCM_SHA256 must be implemented).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_bridge_security::{rustls, tls_provider};

#[test]
fn ddsamqp_7_1_the_tls_provider_implements_tls_aes_128_gcm_sha256() {
    let provider = tls_provider();
    assert!(
        provider
            .cipher_suites
            .iter()
            .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)
    );
}
