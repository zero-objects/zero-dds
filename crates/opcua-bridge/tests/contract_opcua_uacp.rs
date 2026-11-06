// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-opcua-uacp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-uacp`:
//! `securechannel::DATETIME_TICKS_AT_UNIX_EPOCH` to turn an RTPS `INFO_TS`
//! and the wall clock into an OPC UA `DateTime` (`src/daemon/server.rs`);
//! with the feature `crypto`, `crypto::{SecurityPolicy, SecuredMode,
//! RsaPrivateKey, RsaPublicKey}` for the secured `opc.tcp` endpoint, the
//! policy chosen by its configuration name (`basic256sha256`,
//! `aes128_sha256_rsaoaep`, `aes256_sha256_rsapss`).
//!
//! So the crate relies on: the DateTime epoch offset being the one of the
//! encoding; each configured policy being the AES variant its name says.
//!
//! Oracles: OPC 10000-6 v1.05.07 §5.2.2.5 (DateTime: 100 ns intervals since
//! 1601-01-01 UTC; 1970-01-01 is 11 644 473 600 s later); the policy names
//! of OPC 10000-7 (`Aes128_…` uses AES-128, a 16-octet key; `Basic256Sha256`
//! and `Aes256_…` use AES-256, a 32-octet key).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_opcua_uacp::securechannel::DATETIME_TICKS_AT_UNIX_EPOCH;

#[test]
fn uacs_5_2_2_5_the_unix_epoch_is_11_644_473_600_seconds_after_1601() {
    assert_eq!(DATETIME_TICKS_AT_UNIX_EPOCH, 11_644_473_600 * 10_000_000);
}

#[cfg(feature = "crypto")]
#[test]
fn each_configured_policy_is_the_aes_variant_its_name_says() {
    use zerodds_opcua_uacp::crypto::SecurityPolicy;
    for (policy, name, key_len) in [
        (SecurityPolicy::Basic256Sha256, "Basic256Sha256", 32),
        (
            SecurityPolicy::Aes128Sha256RsaOaep,
            "Aes128_Sha256_RsaOaep",
            16,
        ),
        (
            SecurityPolicy::Aes256Sha256RsaPss,
            "Aes256_Sha256_RsaPss",
            32,
        ),
    ] {
        assert_eq!(policy.sym_enc_key_len(), key_len, "{name}");
        assert_eq!(
            policy.uri(),
            format!("http://opcfoundation.org/UA/SecurityPolicy#{name}")
        );
        assert_eq!(SecurityPolicy::from_uri(policy.uri()), Some(policy));
    }
}
