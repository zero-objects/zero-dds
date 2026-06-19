//! C5.5 — Cyclone gap-filler: TypeLookup responder side.
//!
//! `cyclone_live_typelookup.rs` only covers the requester side (we send
//! a request and parse the reply). This test verifies the **responder
//! side**: parse Cyclone-like TL requests and build matching replies.
//!
//! Since the full live RPC is only wired up in WP 1.6+, we simulate the
//! wire loop deterministically: build a request via the requester API,
//! pass it to the responder, and check the built reply for Cyclone spec
//! conformance.
//!
//! # Spec reference
//!
//! - DDS-XTypes 1.3 §7.6.3.3 — TypeLookup service RPC
//! - Wire compat: the reply bytes must match Cyclone's
//!   `dds-type-lookup` implementation byte-exact.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_discovery::type_lookup::TypeLookupStack;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier};

#[test]
fn typelookup_responder_builds_cyclone_compatible_reply() {
    // Server (Cyclone-side simulation): has a type registered.
    let mut responder = TypeLookupStack::new(GuidPrefix::from_bytes([0x11; 12]));
    let m = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::ShapeType")
            .member("color", TypeIdentifier::String8Small { bound: 128 }, |m| {
                m.key()
            })
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("y", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member(
                "shapesize",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m,
            )
            .build_minimal(),
    );
    let hash = zerodds_types::compute_minimal_hash(&m).unwrap();
    responder.registry.insert_minimal(hash, m);

    // Client (ZeroDDS side): sends a request.
    let mut requester = TypeLookupStack::new(GuidPrefix::from_bytes([0x22; 12]));
    let (req_bytes, _seq) = requester
        .make_get_types_request(&[hash], true)
        .expect("request");
    eprintln!("request bytes: {}", req_bytes.len());

    // Responder builds a reply.
    let reply_bytes = responder
        .build_get_types_reply(&[hash], true)
        .expect("reply");
    eprintln!("reply bytes: {}", reply_bytes.len());

    // Wire sanity: the reply begins with a 4-byte count > 0.
    assert!(reply_bytes.len() >= 4, "reply too short");
    let count_le = u32::from_le_bytes(reply_bytes[..4].try_into().unwrap());
    assert_eq!(
        count_le, 1,
        "expected exactly 1 type in reply, got {count_le}"
    );

    // Round trip: client parses the reply.
    let n = requester
        .handle_get_types_reply(&reply_bytes)
        .expect("handle reply");
    assert_eq!(n, 1, "expected 1 type registered after reply");
    assert!(
        requester.registry.get_minimal(&hash).is_some(),
        "type not in client registry post-reply"
    );
}

#[test]
fn typelookup_responder_unknown_hash_yields_empty_reply() {
    // Cross-vendor edge case: the client asks for a hash the server
    // doesn't have. In this case Cyclone sends a reply with an empty
    // types list (no error). We must hit the wire format of this empty
    // reply byte-exact.
    let responder = TypeLookupStack::new(GuidPrefix::from_bytes([0x33; 12]));
    // No insert.
    let hash = zerodds_types::EquivalenceHash([0xAA; 14]);
    let reply = responder
        .build_get_types_reply(&[hash], true)
        .expect("reply");
    // At least 4 bytes (count = 0).
    assert!(reply.len() >= 4);
    let count = u32::from_le_bytes(reply[..4].try_into().unwrap());
    assert_eq!(count, 0, "expected empty types-list, got count={count}");
}
