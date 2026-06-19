// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-crypto`. Safety classification: **SAFE**
//! (a wrapper around `ring`; no own primitive code).
//!
//! AES-GCM + HMAC `CryptographicPlugin` implementation for
//! DDS-Security 1.1 §8.5 (spec `formal/2018-04-01`).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Implements the SPI from
//! `zerodds-security::crypto::CryptographicPlugin`.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`AesGcmCryptoPlugin`] — AES-GCM-128/256 + HMAC-SHA256 plugin impl.
//! - [`PskCryptoPlugin`] — pre-shared-key plugin for out-of-band setups.
//! - [`Suite`] — suite discriminator (AES-128-GCM / AES-256-GCM).
//! - [`crypto_transform`] module — `CryptoHeader`/`CryptoFooter` wire codec
//!   plus `CryptoTransformKind` + `CryptoTransformIdentifier`.
//! - [`session_key`] module — `derive_session_key` + `derive_session_hmac_key`
//!   + `compute_aad` + tag constants (spec §10.5.2 Tab.74).
//! - [`aes_gcm_hw`] module — HW capabilities detection (`Arch`, `HwCapabilities`).
//! - `metrics` (feature `metrics`) — hook points for `zerodds-monitor` §2.5.
//!
//! ## Suite coverage
//!
//! | Suite | Wire kind | Use case |
//! |-------|-----------|----------|
//! | AES-128-GCM | 0x01 | Default production |
//! | AES-256-GCM | 0x02 | High assurance |
//! | HMAC-SHA256 (auth-only) | 0x03 | Governance `metadata_protection_kind=SIGN` |
//!
//! 12-byte nonce = 4-byte session ID + 8-byte counter (spec §9.5.3.3.4.4).
//! Wire token: `[kind_id(1) | session_id(4) | master_key(16|32)]`.
//!
//! Nonce wrap-around protection: at 2^63 encrypts per session the
//! plugin rejects new encrypt calls with "key-refresh required" — the caller must
//! trigger a new `register_local_*` roundtrip.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod aes_gcm_hw;
pub mod crypto_transform;
#[cfg(feature = "metrics")]
pub mod metrics;
mod plugin;
pub mod psk_plugin;
pub mod session_key;
pub mod suite;

pub use aes_gcm_hw::{Arch, HwCapabilities};

pub use crypto_transform::{
    BUILTIN_CRYPTO_PLUGIN, CryptoFooter, CryptoHeader, CryptoTransformIdentifier,
    CryptoTransformKind, negotiate_transform,
};
pub use plugin::AesGcmCryptoPlugin;
pub use psk_plugin::{CLASS_ID_PSK_CRYPTO, HKDF_INFO_PSK_MASTER_KEY, PskCryptoPlugin};
pub use session_key::{
    AAD_HEADER_LEN, SESSION_KEY_TAG, SESSION_RECEIVER_KEY_TAG, compute_aad,
    derive_session_hmac_key, derive_session_key,
};
pub use suite::Suite;
