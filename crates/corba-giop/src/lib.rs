// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 2 §15 — General Inter-ORB Protocol (GIOP).
//!
//! Crate `zerodds-corba-giop`. Safety classification: **STANDARD**.
//! Spec OMG CORBA 3.3 Part 2 §15 (`docs/standards/cache/omg/corba-3.3-part2.pdf`,
//! eingepflegt in Sprint-1-Setup-Phase).
//!
//! # Scope
//!
//! Voller GIOP-Wire-Codec — alle 8 Message-Types fuer GIOP 1.0, 1.1
//! und 1.2 (inkl. Bidirectional-GIOP). Reuse CDR-1 aus `crates/cdr/`.
//!
//! Implementierte Message-Types (Spec §15.4.1-15.4.9):
//!
//! * `Request` (Spec §15.4.2) — Client→Server Method-Invocation.
//! * `Reply` (Spec §15.4.3) — Server→Client Response inkl. aller 6
//!   Reply-Statuses (NO_EXCEPTION/USER_EXCEPTION/SYSTEM_EXCEPTION/
//!   LOCATION_FORWARD/LOCATION_FORWARD_PERM/NEEDS_ADDRESSING_MODE).
//! * `CancelRequest` (Spec §15.4.4) — Client cancelt pending Request.
//! * `LocateRequest` (Spec §15.4.5) — Client probt Object-Location.
//! * `LocateReply` (Spec §15.4.6) — Server antwortet auf LocateRequest.
//! * `CloseConnection` (Spec §15.4.7) — Server schliesst Connection.
//! * `MessageError` (Spec §15.4.8) — Wire-Format-Fehler-Indication.
//! * `Fragment` (Spec §15.4.9) — Multi-Frame-Message-Body.
//!
//! GIOP-Versionen:
//!
//! * `1.0` — Initial-Format. Request/Reply-Header inkl. `requesting_principal`.
//!   Kein Fragment-Support, kein Bidirectional-GIOP.
//! * `1.1` — `byte_order` wird zu `flags`-Octet (Bit 0 = byte_order,
//!   Bit 1 = fragment). Fragment fuer Request+Reply. Request-Header
//!   bekommt 3-Byte-`reserved`-Feld.
//! * `1.2` — Request/Reply-Header neu organisiert: `request_id`
//!   zuerst, `service_context` zuletzt. `TargetAddress`-Union ersetzt
//!   den `object_key`. Body 8-Byte-aligned. Fragment fuer alle
//!   Message-Types. Bidirectional-GIOP via `BiDirIIOPServiceContext`.
//!
//! # Was nicht hier ist
//!
//! * **Transport** — TCP/UDS-Lieferung in `crates/corba-iiop/`.
//! * **IOR-Format** — Object-Reference-Marshalling in `crates/corba-ior/`.
//! * **POA** — Servant-Dispatch in `crates/corba-poa/`.
//! * **Codeset-Konversion** — `crates/cdr/` macht UTF-8/UTF-16 nativ;
//!   ISO-Latin-Konversion ist Caller-Layer (selten in modernen
//!   CORBA-Deployments).
//!
//! # Beispiel
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
pub mod codec;
pub mod error;
pub mod flags;
pub mod fragment;
pub mod header;
pub mod locate_reply;
pub mod locate_request;
pub mod message_error;
pub mod message_type;
pub mod reply;
pub mod request;
pub mod service_context;
pub mod target_address;
pub mod version;

pub use cancel_request::CancelRequest;
pub use close_connection::CloseConnection;
pub use codec::{Message, decode_message, encode_message};
pub use error::{GiopError, GiopResult};
pub use flags::Flags;
pub use fragment::{Fragment, FragmentHeader};
pub use header::{MAGIC, MAGIC_BYTES, MessageHeader};
pub use locate_reply::{LocateReply, LocateStatusType};
pub use locate_request::LocateRequest;
pub use message_error::MessageError;
pub use message_type::MessageType;
pub use reply::{Reply, ReplyStatusType};
pub use request::{Request, ResponseFlags};
pub use service_context::{ServiceContext, ServiceContextList, ServiceContextTag};
pub use target_address::{ObjectKey, TargetAddress};
pub use version::Version;
