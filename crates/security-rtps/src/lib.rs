// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-rtps`. Safety classification: **SAFE** (a pure wire-format adapter; the actual crypto delegates to a [`CryptographicPlugin`]).
//!
//! Secure submessage wrapper (OMG DDS-Security 1.1 §7.3.6) +
//! RTPS header AAD codec (§9.5).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services. Consumes `zerodds-security` (SPI) +
//! `zerodds-rtps` (RTPS submessage layout). Used by the DCPS runtime via
//! `Box<dyn CryptographicPlugin>` and the inbound/outbound datapath.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! Takes one or more plain RTPS submessages (as opaque bytes)
//! and wraps them into:
//!
//! ```text
//! SEC_PREFIX  | SEC_BODY (ciphertext)  | SEC_POSTFIX
//! ```
//!
//! On the receiver side `decode_secured_submessage` does the step
//! in reverse: extract SEC_BODY, send it through the crypto plugin,
//! return the plaintext.
//!
//! - Submessage IDs + flags per spec §7.3.6.
//! - `encode_secured_submessage` + `decode_secured_submessage` with a
//!   `&mut dyn CryptographicPlugin` callback — so AES-GCM, HMAC,
//!   or future backends are interchangeable.
//! - SRTPS wrap (§9.5 RTPS message protection): `SRTPS_PREFIX` + `SRTPS_POSTFIX` codec.
//! - Receiver-specific MAC list in the POSTFIX (`MAX_RECEIVER_MACS`): one
//!   16-byte MAC per remote reader; single-receiver paths leave the
//!   list empty (spec §7.3.6.3 allows that).
//! - Little-endian submessage header (`0x01` flag).
//!
//! ## Non-goals
//!
//! - Big-endian submessage header — the spec allows both; all vendors
//!   use LE by default. Re-add additively in major-2.0.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod codec;
pub mod header_aad;
mod srtps;

pub use codec::{
    MAX_RECEIVER_MACS, SEC_BODY, SEC_POSTFIX, SEC_PREFIX, SRTPS_POSTFIX, SRTPS_PREFIX,
    SecurityRtpsError, decode_secured_submessage, decode_secured_submessage_multi,
    encode_secured_submessage, encode_secured_submessage_multi,
};
pub use header_aad::{build_rtps_header_aad, build_submessage_aad};
pub use srtps::{
    PRE_SHARED_KEY_FLAG, RTPS_HEADER_LEN, decode_secured_rtps_message, encode_secured_rtps_message,
    encode_secured_rtps_message_psk, srtps_psk_flag,
};
