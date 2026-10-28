// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 2 §15 — General Inter-ORB Protocol (GIOP).
//!
//! Crate `zerodds-corba-giop`. Safety classification: **STANDARD**.
//! Spec OMG CORBA 3.3 Part 2 §15 (`internal/standards/cache/omg/corba-3.3-part2.pdf`,
//! imported during the Sprint-1 setup phase).
//!
//! # Scope
//!
//! Full GIOP wire codec — all 8 message types for GIOP 1.0, 1.1
//! and 1.2 (including bidirectional GIOP). CDR-1 marshalling comes from
//! `crates/wire/`.
//!
//! Implemented message types (spec §15.4.1-15.4.9):
//!
//! * `Request` (spec §15.4.2) — client→server method invocation.
//! * `Reply` (spec §15.4.3) — server→client response including all 6
//!   reply statuses (NO_EXCEPTION/USER_EXCEPTION/SYSTEM_EXCEPTION/
//!   LOCATION_FORWARD/LOCATION_FORWARD_PERM/NEEDS_ADDRESSING_MODE).
//! * `CancelRequest` (spec §15.4.4) — client cancels a pending request.
//! * `LocateRequest` (spec §15.4.5) — client probes an object's location.
//! * `LocateReply` (spec §15.4.6) — server answers a LocateRequest.
//! * `CloseConnection` (spec §15.4.7) — server closes the connection.
//! * `MessageError` (spec §15.4.8) — wire-format error indication.
//! * `Fragment` (spec §15.4.9) — multi-frame message body.
//!
//! GIOP versions:
//!
//! * `1.0` — initial format. Request/Reply header includes `requesting_principal`.
//!   No fragment support, no bidirectional GIOP.
//! * `1.1` — `byte_order` becomes a `flags` octet (bit 0 = byte_order,
//!   bit 1 = fragment). Fragment for Request+Reply. The request header
//!   gains a 3-byte `reserved` field.
//! * `1.2` — Request/Reply header reorganized: `request_id`
//!   first, `service_context` last. The `TargetAddress` union replaces
//!   the `object_key`. Body 8-byte aligned. Fragment for all
//!   message types. Bidirectional GIOP via `BiDirIIOPServiceContext`.
//!
//! # What is not here
//!
//! * **Transport** — TCP/UDS delivery in `crates/corba-iiop/`.
//! * **IOR format** — object-reference marshalling in `crates/corba-ior/`.
//! * **POA** — servant *routing* (which servant answers an object key)
//!   in `crates/corba-poa/`. The six `CORBA::Object` pseudo-operations
//!   §15.4.2 names ([`object_ops`]) are answered here instead, ahead of
//!   that routing — see the module's own doc for why.
//! * **Codeset conversion** — `crates/wire/` handles UTF-8/UTF-16 natively;
//!   ISO-Latin conversion is a caller-layer concern (rare in modern
//!   CORBA deployments).
//!
//! # Example
//!
//! ```
//! use zerodds_corba_giop::{MAGIC_BYTES, Version};
//! assert_eq!(MAGIC_BYTES, *b"GIOP");
//! let v = Version { major: 1, minor: 2 };
//! assert_eq!(v.major, 1);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod cancel_request;
pub mod close_connection;
pub mod code_set;
pub mod codec;
pub mod error;
pub mod flags;
pub mod fragment;
pub mod header;
pub mod locate_reply;
pub mod locate_request;
pub mod message_error;
pub mod message_type;
pub mod object_ops;
pub mod reply;
pub mod request;
pub mod service_context;
pub mod target_address;
pub mod version;

pub use cancel_request::CancelRequest;
pub use close_connection::CloseConnection;
pub use code_set::{CodeSetContext, well_known as code_set_ids};
pub use codec::{
    Message, MessageWithPadding, decode_message, decode_message_ctx, encode_message,
    encode_message_with_padding,
};
pub use error::{GiopError, GiopResult};
pub use flags::Flags;
pub use fragment::{Fragment, FragmentHeader};
pub use header::{HEADER_SIZE, MAGIC, MAGIC_BYTES, MessageHeader};
pub use locate_reply::{LocateReply, LocateStatusType};
pub use locate_request::LocateRequest;
pub use message_error::MessageError;
pub use message_type::MessageType;
pub use reply::{Reply, ReplyStatusType};
pub use request::{Request, ResponseFlags};
pub use service_context::{ServiceContext, ServiceContextList, ServiceContextTag};
pub use target_address::{ObjectKey, TargetAddress};
pub use version::Version;
