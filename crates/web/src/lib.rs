// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS-WEB 1.0 — Web-Enabled DDS Object Model + REST PSM.
//!
//! Crate `zerodds-web`. Safety classification: **STANDARD**.
//! Spec `formal/2014-12-01` (`internal/standards/cache/omg/zerodds-web-1.0.pdf`).
//!
//! # Scope
//!
//! We implement the **WebDDS object model** (Spec §7) and the
//! **REST PSM** (Spec §8.3) as a pure-Rust no_std+alloc library:
//!
//! * `WebDDS::Root` singleton + `Application` + `Client` +
//!   `AccessController` + `DomainParticipant` (§7.3 object model).
//! * `ReturnStatus` with all spec codes (§7.3.1 + §8.3.2 mapping to
//!   HTTP status).
//! * `RestRoute` parser for all URI patterns from Spec §8.3.1
//!   Tab 4 + §8.3.3 Tab 5 (all 30+ routes with parameter extraction).
//! * `Representation` XML element-name registry (Spec §8.3.4 Tab 6).
//! * `Headers` HTTP request/response header set (Spec §8.3.5 Tab 7+8).
//! * `SessionId` authenticated-session tracking (§7.3.1.1).
//!
//! # What is not covered
//!
//! * **HTTP server implementation** — caller layer (typically
//!   `axum`/`hyper`); this crate provides routing tables + status
//!   mapping, the caller binds to the HTTP stack.
//! * **XML body serialization** — caller layer with the `crates/xml/`
//!   loader + DDS-XTYPES type-definition reader; the XML element-name
//!   registry here provides the wire tags.
//! * **WebSocket push for DataReader notifications** — see
//!   `crates/websocket-bridge/`; the caller wires it via a listener
//!   callback from `crates/dcps/`.
//! * **SOAP/WSDL-Platform** (Spec §8.4) — `n/a` (REST ist mandatory,
//!   SOAP is optional + rarely used).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod access_control;
pub mod bridge;
pub mod headers;
pub mod model;
pub mod representation;
pub mod rest;
pub mod sample_selector;
pub mod session;
pub mod status;

pub use access_control::{Decision, Operation, Permissions, Rule};
pub use bridge::{BackendError, BackendResult, DdsBackend, enforce};
pub use headers::{REQUEST_API_KEY, RequestHeaders, ResponseHeaders};
pub use model::{Application, Client, DomainParticipant, WebDdsRoot};
pub use representation::{ContentType, RepresentationKind, element_name};
pub use rest::{REST_PREFIX, RestMethod, RestRoute, parse_route};
pub use sample_selector::{
    BoolOp, CompareOp, FilterExpression, InstanceStateMatch, Literal, MetadataExpression,
    ParseError as SampleSelectorParseError, SampleSelector, SampleStateMatch, ViewStateMatch,
    parse_sample_selector,
};
pub use session::SessionId;
pub use status::{ReturnStatus, http_status_for};
