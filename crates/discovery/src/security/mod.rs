// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security Builtin-Endpoint-Slots — Wire-Layer.
//!
//! Liefert die zwei Endpoint-Slot-Paare aus DDS-Security 1.2 §7.4.4 +
//! §7.4.5:
//!
//! | Topic                                  | Reliability | Modul                |
//! |----------------------------------------|-------------|----------------------|
//! | `DCPSParticipantStatelessMessage`      | BestEffort  | [`stateless`]        |
//! | `DCPSParticipantVolatileMessageSecure` | Reliable    | [`volatile_secure`]  |
//!
//! Plus [`stack::SecurityBuiltinStack`] als Bundle der vier Endpoints
//! mit automatischer Proxy-Verdrahtung anhand der vom Peer im SPDP
//! annoncierten BuiltinEndpointSet-Bits 22..25.
//!
//! ## Layer-Boundary
//!
//! Dieses Modul liefert die **Wire-Endpoint-Slots**. Die Plugin-
//! Pipeline-Logik (Auth-Handshake-State-Machine, Crypto-Token-Routing)
//! lebt im DCPS-Layer (`crates/dcps/src/security/`), wo die Hooks
//! gemäss DDS-Security 1.2 §10.3.4 + §10.5.4 gesetzt werden — das ist
//! eine Spec-konforme Layer-Trennung, kein Deferral.

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
