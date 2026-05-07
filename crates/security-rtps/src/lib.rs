// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-rtps`. Safety classification: **SAFE** (reiner Wire-Format-Adapter; die eigentliche Crypto delegiert an ein [`CryptographicPlugin`]).
//!
//! Secure-Submessage-Wrapper (OMG DDS-Security 1.1 §7.3.6) +
//! RTPS-Header-AAD-Codec (§9.5).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert `zerodds-security` (SPI) +
//! `zerodds-rtps` (RTPS-Submessage-Layout). Wird vom DCPS-Runtime via
//! `Box<dyn CryptographicPlugin>` und dem Inbound/Outbound-Datapath
//! genutzt.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! Nimmt eine oder mehrere plain-RTPS-Submessages (als opaque Bytes)
//! und wrapped sie in:
//!
//! ```text
//! SEC_PREFIX  | SEC_BODY (ciphertext)  | SEC_POSTFIX
//! ```
//!
//! Auf Empfaenger-Seite macht `decode_secured_submessage` den Schritt
//! rueckwaerts: SEC_BODY extrahieren, durch den Crypto-Plugin schicken,
//! plaintext zurueckgeben.
//!
//! - Submessage-IDs + -Flags gemaess Spec §7.3.6.
//! - `encode_secured_submessage` + `decode_secured_submessage` mit
//!   `&mut dyn CryptographicPlugin`-Callback — damit AES-GCM, HMAC,
//!   oder zukuenftige Backends austauschbar sind.
//! - SRTPS-Wrap (§9.5 RTPS-Message-Protection): `SRTPS_PREFIX` + `SRTPS_POSTFIX`-Codec.
//! - Receiver-Specific-MAC-Liste im POSTFIX (`MAX_RECEIVER_MACS`): pro
//!   Remote-Reader ein 16-byte MAC; Single-Receiver-Pfade lassen die
//!   Liste leer (Spec §7.3.6.3 erlaubt das).
//! - Little-Endian-Submessage-Header (`0x01` flag).
//!
//! ## Nicht-Ziele
//!
//! - Big-Endian-Submessage-Header — Spec erlaubt beide; alle Vendoren
//!   nutzen LE per Default. Re-Add additiv bei Major-2.0.

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
