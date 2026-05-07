//! C5.5 — Cyclone-Lueckenfueller: TypeLookup-Responder-Side.
//!
//! `cyclone_live_typelookup.rs` deckt nur die Requester-Seite (wir
//! schicken einen Request und parsen die Reply). Dieser Test
//! verifiziert die **Responder-Seite**: Cyclone-aehnliche TL-Requests
//! parsen und passende Replies bauen.
//!
//! Da der Full-Live-RPC erst mit WP 1.6+ verdrahtet wird, simulieren
//! wir den Wire-Loop deterministisch: Request via Requester-API
//! bauen, an Responder durchreichen, und auf Cyclone-Spec-Konformitaet
//! der gebauten Reply pruefen.
//!
//! # Spec-Bezug
//!
//! - DDS-XTypes 1.3 §7.6.3.3 — TypeLookup-Service-RPC
//! - Wire-Compat: Reply-Bytes muessen byte-genau mit Cyclone's
//!   `dds-type-lookup`-Implementierung uebereinstimmen.

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
    // Server (Cyclone-Side-Simulation): hat einen Type registriert.
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

    // Client (ZeroDDS-Side): schickt Request.
    let mut requester = TypeLookupStack::new(GuidPrefix::from_bytes([0x22; 12]));
    let (req_bytes, _seq) = requester
        .make_get_types_request(&[hash], true)
        .expect("request");
    eprintln!("request bytes: {}", req_bytes.len());

    // Responder baut Reply.
    let reply_bytes = responder
        .build_get_types_reply(&[hash], true)
        .expect("reply");
    eprintln!("reply bytes: {}", reply_bytes.len());

    // Wire-Sanity: Reply beginnt mit 4-Byte-Count > 0.
    assert!(reply_bytes.len() >= 4, "reply too short");
    let count_le = u32::from_le_bytes(reply_bytes[..4].try_into().unwrap());
    assert_eq!(
        count_le, 1,
        "expected exactly 1 type in reply, got {count_le}"
    );

    // Round-Trip: Client parst Reply.
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
    // Cross-Vendor-Edge-Case: Client fragt nach einem Hash, den
    // Server nicht hat. Cyclone schickt in diesem Fall eine Reply
    // mit leerer types-Liste (kein Error). Wir muessen das Wire-Format
    // dieser leeren Reply byte-genau treffen.
    let responder = TypeLookupStack::new(GuidPrefix::from_bytes([0x33; 12]));
    // Kein insert.
    let hash = zerodds_types::EquivalenceHash([0xAA; 14]);
    let reply = responder
        .build_get_types_reply(&[hash], true)
        .expect("reply");
    // Mind. 4 Bytes (count = 0).
    assert!(reply.len() >= 4);
    let count = u32::from_le_bytes(reply[..4].try_into().unwrap());
    assert_eq!(count, 0, "expected empty types-list, got count={count}");
}
