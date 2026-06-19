// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security builtin-endpoint slots — wire layer.
//!
//! Provides the two endpoint-slot pairs from DDS-Security 1.2 §7.4.4 +
//! §7.4.5:
//!
//! | Topic                                  | Reliability | Module               |
//! |----------------------------------------|-------------|----------------------|
//! | `DCPSParticipantStatelessMessage`      | BestEffort  | [`stateless`]        |
//! | `DCPSParticipantVolatileMessageSecure` | Reliable    | [`volatile_secure`]  |
//!
//! Plus [`stack::SecurityBuiltinStack`] as a bundle of the four endpoints
//! with automatic proxy wiring based on the BuiltinEndpointSet bits 22..25
//! announced by the peer in SPDP.
//!
//! ## Layer boundary
//!
//! This module provides the **wire endpoint slots**. The plugin
//! pipeline logic (auth handshake state machine, crypto token routing)
//! lives in the DCPS layer (`crates/dcps/src/security/`), where the hooks
//! are installed per DDS-Security 1.2 §10.3.4 + §10.5.4 — this is a
//! spec-conformant layer separation, not a deferral.

pub mod codec;
pub mod stack;
pub mod stateless;
pub mod volatile_secure;

pub use codec::{
    ENCAPSULATION_CDR_LE, ENCAPSULATION_HEADER_LEN, decode_generic_message, encode_generic_message,
};
pub use stack::SecurityBuiltinStack;
pub use stateless::{StatelessMessageReader, StatelessMessageWriter};
pub use volatile_secure::{
    VOLATILE_SECURE_DEFAULT_DEPTH, VOLATILE_SECURE_HEARTBEAT_PERIOD,
    VOLATILE_SECURE_READER_CAPACITY, VolatileSecureMessageReader, VolatileSecureMessageWriter,
};
