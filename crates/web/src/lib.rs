// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS-WEB 1.0 — Web-Enabled DDS Object Model + REST PSM.
//!
//! Crate `zerodds-web`. Safety classification: **STANDARD**.
//! Spec `formal/2014-12-01` (`docs/standards/cache/omg/zerodds-web-1.0.pdf`).
//!
//! # Scope
//!
//! Wir implementieren das **WebDDS-Object-Model** (Spec §7) und das
//! **REST-PSM** (Spec §8.3) als pure-Rust no_std+alloc Library:
//!
//! * `WebDDS::Root` Singleton + `Application` + `Client` +
//!   `AccessController` + `DomainParticipant` (§7.3 Object-Model).
//! * `ReturnStatus` mit allen Spec-Codes (§7.3.1 + §8.3.2 Mapping zu
//!   HTTP-Status).
//! * `RestRoute` Parser fuer alle URI-Patterns aus Spec §8.3.1
//!   Tab 4 + §8.3.3 Tab 5 (alle 30+ Routes mit Parameter-Extraktion).
//! * `Representation` XML-Element-Name-Registry (Spec §8.3.4 Tab 6).
//! * `Headers` HTTP-Request/Response-Header-Set (Spec §8.3.5 Tab 7+8).
//! * `SessionId` Authenticated-Session-Tracking (§7.3.1.1).
//!
//! # Was nicht abgedeckt ist
//!
//! * **HTTP-Server-Implementation** — Caller-Layer (typisch
//!   `axum`/`hyper`); diese Crate liefert Routing-Tabellen + Status-
//!   Mapping, der Caller bindet an HTTP-Stack.
//! * **XML-Body-Serialization** — Caller-Layer mit `crates/xml/`-
//!   Loader + DDS-XTYPES-Type-Definition-Reader; das XML-Element-
//!   Name-Registry hier liefert die Wire-Tags.
//! * **WebSocket-Push fuer DataReader-Notifications** — siehe
//!   `crates/websocket-bridge/`; Caller wired das via Listener-
//!   Callback aus `crates/dcps/`.
//! * **SOAP/WSDL-Platform** (Spec §8.4) — `n/a` (REST ist mandatory,
//!   SOAP ist optional + selten verwendet).

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
