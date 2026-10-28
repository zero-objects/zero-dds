//! C5.5 — Cyclone gap-filler: TypeLookup responder side.
//!
//! `type_lookup_requests.rs` covers the requester side (we send a
//! request and parse the reply). This test verifies the **responder
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

    // Wire sanity. XTypes 1.3 §7.6.3.3.3 declares
    // `TypeLookup_getTypes_Out` `@extensibility(MUTABLE)`, so the first
    // word is a DHEADER over the member list and not a sequence count —
    // this assertion used to read it as the count, which is the shape
    // the tree wrote before the clause's framing was in place.
    assert!(reply_bytes.len() >= 4, "reply too short");
    let dheader = u32::from_le_bytes(reply_bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(
        dheader,
        reply_bytes.len() - 4,
        "the DHEADER counts everything after it"
    );
    let mut r = zerodds_types::framing::xcdr2_reader(&reply_bytes);
    let decoded = zerodds_types::type_lookup::GetTypesReply::decode_from(&mut r).expect("decode");
    assert_eq!(decoded.types.len(), 1, "expected exactly 1 type in reply");
    // §7.6.3.3.3: the identifier travels with the object.
    assert_eq!(
        decoded.types[0].type_identifier,
        TypeIdentifier::EquivalenceHashMinimal(hash)
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
    // The client asks for a hash the server does not have.
    // §7.6.3.3.4.2 says the reply's `types` "shall contain TypeObjects
    // that correspond to the TypeIdentifiers in the request" and names
    // no error path for one that does not, so an unknown hash is
    // answered with an empty list.
    //
    // Not measured against Cyclone. The comment this replaced claimed
    // the byte form was Cyclone's; nothing in this file has ever spoken
    // to Cyclone, and the shape it asserted — a bare sequence count in
    // the first word — was the tree's own, not the clause's.
    let responder = TypeLookupStack::new(GuidPrefix::from_bytes([0x33; 12]));
    // No insert.
    let hash = zerodds_types::EquivalenceHash([0xAA; 14]);
    let reply = responder
        .build_get_types_reply(&[hash], true)
        .expect("reply");
    let mut r = zerodds_types::framing::xcdr2_reader(&reply);
    let decoded = zerodds_types::type_lookup::GetTypesReply::decode_from(&mut r).expect("decode");
    assert!(decoded.types.is_empty(), "expected an empty types list");
    assert!(decoded.complete_to_minimal.is_empty());
}
