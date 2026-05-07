//! WP 1.5 T15 — Live-Interop-Skelett fuer TypeLookup gegen Cyclone DDS.
//!
//! **Opt-in only** — `#[ignore]`. Verifiziert dass unser TypeLookupStack
//! Requests serialisiert, die Cyclone byte-technisch akzeptieren wuerde.
//! Voller Live-RPC-Roundtrip (Request via Reliable-Writer → Reply via
//! Reliable-Reader) ist noch nicht verdrahtet — der Stack ist Transport-
//! agnostisch, die TypeLookup-Endpoints muessten analog SEDP (WP 1.4 T4)
//! als weitere Reliable-Writer/Reader-Paare registriert werden.
//!
//! Phase-1-Scope: Request-Payload byte-genau gemaess XTypes §7.6.3.3,
//! Sample-Identity-Tracking deterministisch, Responder-Logik aus
//! Registry. Full-Wire-Flow: WP 1.6+ oder WP 2.

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
    // Setup: Requester + Responder mit identischer Registry.
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

    // Simulated transport: Responder-build → Requester-parse.
    let reply_bytes = responder.build_get_types_reply(&[hash], true).unwrap();
    eprintln!("reply serialized: {} bytes", reply_bytes.len());
    let n = requester.handle_get_types_reply(&reply_bytes).unwrap();
    assert_eq!(n, 1);
    assert!(requester.registry.get_minimal(&hash).is_some());

    // TODO WP 1.6: request_bytes → ReliableWriter(TL_REQ_WRITER) → UDP
    // TODO WP 1.6: Cyclone-TL-Responder antwortet via TL_REPLY_WRITER
    // TODO WP 1.6: reply_bytes an handle_get_types_reply()
}

#[test]
fn get_types_request_is_cdr_wire_compatible() {
    // Sanity: serialisierter Request beginnt mit u32-Laenge und enthaelt
    // dann die TypeIdentifier. Cyclone akzeptiert diese als CDR-sequence.
    let mut stack = TypeLookupStack::new(GuidPrefix::from_bytes([1; 12]));
    let hashes = [EquivalenceHash([0xA1; 14]), EquivalenceHash([0xB2; 14])];
    let (bytes, _) = stack.make_get_types_request(&hashes, true).unwrap();
    // Ersten 4 Bytes = u32 count = 2 (LE)
    assert_eq!(&bytes[..4], &[0x02, 0x00, 0x00, 0x00]);
    // Danach 2 * (1 disc + 14 hash) = 30 bytes TypeIdentifier.
    assert_eq!(bytes.len(), 4 + 2 * 15);
    assert_eq!(bytes[4], 0xF1); // EK_MINIMAL
}
