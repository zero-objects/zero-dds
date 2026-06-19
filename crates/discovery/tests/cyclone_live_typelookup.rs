//! WP 1.5 T15 — live-interop skeleton for TypeLookup against Cyclone DDS.
//!
//! **Opt-in only** — `#[ignore]`. Verifies that our TypeLookupStack
//! serializes requests that Cyclone would accept byte-wise. The full
//! live RPC round trip (request via reliable writer → reply via
//! reliable reader) is not yet wired up — the stack is transport-
//! agnostic, and the TypeLookup endpoints would need to be registered
//! as additional reliable writer/reader pairs, analogous to SEDP
//! (WP 1.4 T4).
//!
//! Phase 1 scope: request payload byte-exact per XTypes §7.6.3.3,
//! deterministic sample-identity tracking, responder logic from the
//! registry. Full wire flow: WP 1.6+ or WP 2.

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
#[ignore = "placeholder — full Cyclone-RPC-roundtrip requires TypeLookup reliable endpoints wired into transport (WP 1.6+)"]
fn cyclone_typelookup_live_roundtrip_placeholder() {
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
    eprintln!("request serialized: {} bytes", req_bytes.len());

    // Simulated transport: responder build → requester parse.
    let reply_bytes = responder.build_get_types_reply(&[hash], true).unwrap();
    eprintln!("reply serialized: {} bytes", reply_bytes.len());
    let n = requester.handle_get_types_reply(&reply_bytes).unwrap();
    assert_eq!(n, 1);
    assert!(requester.registry.get_minimal(&hash).is_some());

    // TODO WP 1.6: request_bytes → ReliableWriter(TL_REQ_WRITER) → UDP
    // TODO WP 1.6: Cyclone TL responder replies via TL_REPLY_WRITER
    // TODO WP 1.6: reply_bytes to handle_get_types_reply()
}

#[test]
fn get_types_request_is_cdr_wire_compatible() {
    // Sanity: the serialized request begins with a u32 length and then
    // contains the TypeIdentifiers. Cyclone accepts these as a CDR
    // sequence.
    let mut stack = TypeLookupStack::new(GuidPrefix::from_bytes([1; 12]));
    let hashes = [EquivalenceHash([0xA1; 14]), EquivalenceHash([0xB2; 14])];
    let (bytes, _) = stack.make_get_types_request(&hashes, true).unwrap();
    // First 4 bytes = u32 count = 2 (LE)
    assert_eq!(&bytes[..4], &[0x02, 0x00, 0x00, 0x00]);
    // Then 2 * (1 disc + 14 hash) = 30 bytes of TypeIdentifier.
    assert_eq!(bytes.len(), 4 + 2 * 15);
    assert_eq!(bytes[4], 0xF1); // EK_MINIMAL
}
