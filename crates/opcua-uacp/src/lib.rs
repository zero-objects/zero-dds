// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC-UA Connection Protocol (**UACP**, OPC Foundation Part 6 §7.1) and
//! Secure Conversation (§6.7) — the `opc.tcp://` binary message framing and
//! SecureChannel chunks that carry OPC-UA Client/Server traffic.
//!
//! Crate `zerodds-opcua-uacp`. Safety classification: **STANDARD**.
//! Spec: OPC-UA **Part 6 — Mappings** §7.1 (Connection Protocol).
//!
//! # Scope and relationship to the rest of the OPC-UA stack
//!
//! This is the **wire foundation** for a native OPC-UA Client/Server stack
//! (the request/response counterpart to the native UADP PubSub stack in
//! [`zerodds-opcua-pubsub`](https://docs.rs/zerodds-opcua-pubsub)). The layers
//! build up:
//!
//! 1. **UACP** (`connection`) — the TCP message framing: every message starts
//!    with an 8-byte header (`MessageType` + `ChunkType` + `MessageSize`),
//!    then a body. The connection-setup messages — Hello / Acknowledge /
//!    Error / ReverseHello.
//! 2. SecureChannel (`securechannel`, Part 6 §6.7) — the `OPN`/`MSG`/`CLO`
//!    chunks and the OpenSecureChannel service, in this crate; the secured
//!    SecurityPolicies are `crypto` (feature `crypto`).
//! 3. Session + Service dispatch (Part 4) over the SecureChannel, and
//! 4. the Server and Client: `zerodds-opcua-server`.
//!
//! The OPC-UA Part 6 §5 binary encoding is reused from
//! [`zerodds_opcua_pubsub::binary`] (`UaReader`/`UaWriter`).

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod connection;
#[cfg(feature = "crypto")]
pub mod crypto;
pub mod securechannel;

pub use connection::{
    AcknowledgeMessage, ChunkType, ErrorMessage, HelloMessage, MessageHeader, MessageType,
    ReverseHelloMessage,
};
#[cfg(feature = "crypto")]
pub use securechannel::SecuritySession;
pub use securechannel::{
    AsymmetricAlgorithmSecurityHeader, ChannelSecurityToken, OpenSecureChannelRequest,
    OpenSecureChannelResponse, ParsedChunk, RequestHeader, ResponseHeader, SecureChannel,
    SequenceHeader, parse_chunk,
};
pub use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaReader, UaWriter};
