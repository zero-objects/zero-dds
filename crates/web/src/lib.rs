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
//!   Tab 4 + §8.3.3 Tab 5 — every one of Table 5's 48 PIM operations,
//!   with parameter extraction, both segment spellings the table uses,
//!   and HEAD routed like GET.
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
//! * **SOAP/WSDL platform** (Spec §8.4) — not in this crate; the optional
//!   SIMPLE-WSDL-SOAP profile lives in `crates/zerodds-soap/` (see
//!   `crates/zerodds-soap/src/wsdl.rs`).

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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod scope_tests {
    //! Spec §1 Scope. §1.1/§1.2/§1.6 are purpose, background and
    //! explicitly non-normative example scenarios — there is nothing there
    //! to check against code. §1.3.1, §1.4 and §1.5 do make structural
    //! claims about what a WebDDS implementation must carry, and those are
    //! what this module checks.

    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// §1.3.1 reason 2 + §7.3.1.1: "Web client connectivity is inherently
    /// intermittent... the Web Enabled DDS Object must overcome this by
    /// introducing a 'session,' whose life can span multiple physical
    /// connections."
    ///
    /// The session is therefore a value the client carries between
    /// requests, not per-connection state: the same token must compare
    /// equal across independently constructed instances, which is what
    /// lets a second physical connection resume the first one's session.
    #[test]
    fn scope_session_identity_survives_reconstruction_across_connections() {
        let issued = SessionId::new("tok-6f21");
        // A later, physically separate request rebuilds the id from the
        // token it carried in its headers.
        let resumed = SessionId::new(String::from("tok-6f21"));
        assert_eq!(issued, resumed, "same token must be the same session");
        assert_eq!(resumed.as_str(), "tok-6f21");
        assert!(resumed.is_valid());
        // A different token is a different session.
        assert_ne!(issued, SessionId::new("tok-0000"));
        // No token at all is not a session.
        assert!(!SessionId::new("").is_valid());
    }

    /// §1.3.1 reason 3 + §1.4: the model "extends the Standard DDS Object
    /// Model in order to include an access control model", one that
    /// "controls which operations each principal can perform (e.g., which
    /// DDS Topics it can read and write)".
    ///
    /// So the decision must depend on the operation *and* the topic, and
    /// an unlisted topic must not be readable — a permit-by-default model
    /// would not be an access control model at all.
    #[test]
    fn scope_access_control_gates_per_operation_and_per_topic() {
        let perms = Permissions {
            subject_name: String::from("plant7-operator"),
            default: Decision::Deny,
            rules: Vec::from([
                Rule::allow("Telemetry/*", Vec::from([Operation::Subscribe])),
                Rule::allow("Command/*", Vec::from([Operation::Publish])),
            ]),
        };
        // Granted operation on a granted topic.
        assert_eq!(
            perms.evaluate(Operation::Subscribe, "Telemetry/gps"),
            Decision::Permit
        );
        assert_eq!(
            perms.evaluate(Operation::Publish, "Command/arm"),
            Decision::Permit
        );
        // Right topic, wrong operation.
        assert_eq!(
            perms.evaluate(Operation::Publish, "Telemetry/gps"),
            Decision::Deny
        );
        assert_eq!(
            perms.evaluate(Operation::Subscribe, "Command/arm"),
            Decision::Deny
        );
        // Topic nobody granted: default deny, not default permit.
        assert_eq!(
            perms.evaluate(Operation::Subscribe, "Secret/keys"),
            Decision::Deny
        );
        assert_eq!(
            Permissions::default().evaluate(Operation::Subscribe, "Telemetry/gps"),
            Decision::Deny,
            "an empty permission set must deny, or §1.4 access control is vacuous"
        );
    }

    /// §1.5: "The **RESTful Platform** mapping defines how to access the
    /// Object Model using a RESTful interface." That mapping must be
    /// reachable from the object model this crate exposes — the WebDDS
    /// root, an application, a participant — rather than being a parallel
    /// vocabulary. The SOAP platform of §1.5 is the optional second
    /// mapping and lives in `crates/zerodds-soap/`.
    #[test]
    fn scope_rest_mapping_addresses_the_webdds_object_model() {
        // The object model names an application and a participant...
        let root = WebDdsRoot::default();
        let _ = &root;
        let app_name = "plant7";
        let part_name = "line3";
        // ...and the REST mapping addresses exactly those, under the
        // §8.3.1 prefix.
        let uri =
            alloc::format!("{REST_PREFIX}/applications/{app_name}/domain_participants/{part_name}");
        let route = parse_route(RestMethod::Put, &uri).expect("prefixed URI");
        assert_eq!(
            route,
            RestRoute::UpdateParticipant {
                app: String::from(app_name),
                participant: String::from(part_name),
            }
        );
        // A URI outside the mapping's prefix is not part of the platform.
        assert!(parse_route(RestMethod::Get, "/applications").is_err());
    }
}
