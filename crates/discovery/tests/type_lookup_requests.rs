//! The TypeLookup request and reply this crate builds, between two
//! stacks and against the clause's framing.
//!
//! The round trip against a vendor is measured in
//! `tests/interop/ci/run_vendor.sh cyclone` (CI job `interop-cyclone`):
//! the two `(TypeLookup)` cells make ZeroDDS fetch a Cyclone writer's
//! type with `getTypes` and decide on the object it gets back. This file
//! held a `#[ignore]`d placeholder for that, waiting on "TypeLookup
//! reliable endpoints wired into transport (WP 1.6+)"; the endpoints are
//! wired, the vendor cells run, and the local half of the placeholder is
//! the first test below.

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
use zerodds_types::{EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier};

#[test]
fn a_get_types_request_and_its_reply_round_trip_between_two_stacks() {
    // Setup: requester + responder with identical registry.
    let mut responder = TypeLookupStack::new(GuidPrefix::from_bytes([0xAA; 12]));
    let m = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::X")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_minimal(),
    );
    let hash = zerodds_types::compute_minimal_hash(&m).unwrap();
    responder.registry.insert_minimal(hash, m);

    let mut requester = TypeLookupStack::new(GuidPrefix::from_bytes([0xBB; 12]));
    let (req_bytes, _seq) = requester.make_get_types_request(&[hash], true).unwrap();
    assert!(!req_bytes.is_empty());

    // Simulated transport: responder build → requester parse.
    let reply_bytes = responder.build_get_types_reply(&[hash], true).unwrap();
    let n = requester.handle_get_types_reply(&reply_bytes).unwrap();
    assert_eq!(n, 1);
    assert!(requester.registry.get_minimal(&hash).is_some());
}

#[test]
fn get_types_request_carries_the_mutable_framing_of_the_clause() {
    // XTypes 1.3 §7.6.3.3.3 declares `TypeLookup_getTypes_In`
    // `@extensibility(MUTABLE)` with `@hashid sequence<TypeIdentifier>
    // type_ids`, so the request is a DHEADER over a member list and one
    // EMHEADER in front of the sequence — not a bare CDR sequence.
    //
    // This test used to assert the bare sequence ("First 4 bytes = u32
    // count = 2"), which is what the tree wrote and not what the clause
    // says. A peer reading the clause's form takes those first four
    // octets as a byte length.
    let mut stack = TypeLookupStack::new(GuidPrefix::from_bytes([1; 12]));
    let hashes = [EquivalenceHash([0xA1; 14]), EquivalenceHash([0xB2; 14])];
    let (bytes, _) = stack.make_get_types_request(&hashes, true).unwrap();

    // Mutable DHEADER + EMHEADER + sequence DHEADER + count/identifiers.
    // TypeIdentifier is nonprimitive: its sequence owns a DHEADER. LC5 reuses
    // that prefix, as measured by the Cyclone 11.0.1 descriptor byte oracle in
    // crates/types/tests/type_lookup_vendor_envelope.rs. LC4 would instead need
    // an additional NEXTINT ahead of the sequence's own DHEADER.
    let member = 4 + 2 * 15;
    assert_eq!(bytes.len(), 4 + 4 + 4 + member);
    assert_eq!(
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize,
        bytes.len() - 4,
        "the DHEADER counts everything after it"
    );

    // The EMHEADER names the member by the `@hashid` of `type_ids` and
    // carries length code 5, reusing the nonprimitive sequence's DHEADER.
    let emheader = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        emheader & 0x0FFF_FFFF,
        zerodds_types::type_lookup::member_ids::TYPE_IDS
    );
    assert_eq!((emheader >> 28) & 0x7, 5);
    assert_eq!(
        u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize,
        member
    );

    // Then the sequence itself: the count, then the identifiers.
    assert_eq!(&bytes[12..16], &[0x02, 0x00, 0x00, 0x00]);
    assert_eq!(bytes[16], 0xF1); // EK_MINIMAL
}
