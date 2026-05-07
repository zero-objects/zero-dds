// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-crypto`. Safety classification: **SAFE**
//! (Wrapper um `ring`; kein eigener Primitive-Code).
//!
//! AES-GCM + HMAC `CryptographicPlugin`-Implementation fuer
//! DDS-Security 1.1 §8.5 (Spec `formal/2018-04-01`).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Implementiert die SPI aus
//! `zerodds-security::crypto::CryptographicPlugin`.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`AesGcmCryptoPlugin`] — AES-GCM-128/256 + HMAC-SHA256 Plugin-Impl.
//! - [`PskCryptoPlugin`] — Pre-Shared-Key-Plugin fuer Out-of-Band-Setups.
//! - [`Suite`] — Suite-Diskriminator (AES-128-GCM / AES-256-GCM).
//! - [`crypto_transform`]-Modul — `CryptoHeader`/`CryptoFooter` Wire-Codec
//!   plus `CryptoTransformKind` + `CryptoTransformIdentifier`.
//! - [`session_key`]-Modul — `derive_session_key` + `derive_session_hmac_key`
//!   + `compute_aad` + Tag-Konstanten (Spec §10.5.2 Tab.74).
//! - [`aes_gcm_hw`]-Modul — HW-Capabilities-Detection (`Arch`, `HwCapabilities`).
//! - `metrics` (Feature `metrics`) — Hook-Points fuer `zerodds-monitor` §2.5.
//!
//! ## Suite-Coverage
//!
//! | Suite | Wire-Kind | Use-Case |
//! |-------|-----------|----------|
//! | AES-128-GCM | 0x01 | Default-Production |
//! | AES-256-GCM | 0x02 | High-Assurance |
//! | HMAC-SHA256 (Auth-only) | 0x03 | Governance `metadata_protection_kind=SIGN` |
//!
//! 12-byte-Nonce = 4 byte Session-ID + 8 byte Counter (Spec §9.5.3.3.4.4).
//! Wire-Token: `[kind_id(1) | session_id(4) | master_key(16|32)]`.
//!
//! Nonce-Wrap-around-Protection: bei 2^63 Encrypts pro Session lehnt der
//! Plugin neue Encrypt-Calls mit "key-refresh required" ab — Caller muss
//! ein neues `register_local_*`-Roundtrip ausloesen.

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
