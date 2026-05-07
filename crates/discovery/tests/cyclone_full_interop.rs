//! WP 1.5 T19 — Full-Live-Interop-Skelett gegen Cyclone DDS.
//!
//! **Opt-in only** — `#[ignore]`. Ziel: ein User-Data-Sample zwischen
//! ZeroDDS und Cyclone fliesst mit XCDR2-Encoding. Das braucht:
//!
//! 1. DCPS-artige API (DataWriter/DataReader) — Phase 2 (WP 2.*)
//! 2. Representation-Negotiation aus T18 — OK, da
//! 3. Assignability-Check aus T16 — OK
//! 4. TypeLookup-Endpoints am Transport — WP 1.6+
//!
//! Dieser Test dokumentiert die Test-Architektur; die eigentliche
//! Implementierung kommt mit Phase 2.

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

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::qos::{DataRepresentationId, negotiate_representation};
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeInformation};

#[test]
#[ignore = "full user-data interop requires DCPS API (WP 2.*)"]
fn cyclone_full_interop_chatter_struct_placeholder() {
    // 1. Schema: Chatter struct
    let b = TypeObjectBuilder::struct_type("::chat::Chatter")
        .extensibility(Extensibility::Appendable)
        .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
            m.key()
        })
        .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m);
    let minimal = MinimalTypeObject::Struct(b.build_minimal());
    let complete = zerodds_types::type_object::CompleteTypeObject::Struct(b.build_complete());

    // 2. TypeInformation bauen
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    eprintln!(
        "type_information: minimal_hash={:?}",
        ti.minimal.typeid_with_size.type_id
    );

    // 3. Representation-Negotiation (Mac offeriert [XCDR2, XCDR1],
    //    Cyclone akzeptiert [XCDR2, XCDR1] — Match: XCDR2).
    let neg = negotiate_representation(&[2, 0], &[2, 0]);
    assert_eq!(
        neg,
        zerodds_types::qos::RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
    );

    // 4. TODO WP 2.*: Create DataWriter<Chatter>, publish Sample.
    // 5. TODO WP 2.*: Cyclone-Subscriber-Subprocess connects + receives.
    // 6. TODO WP 2.*: Assert: sample.id + sample.text round-trippen
    //    byte-genau zwischen ZeroDDS-Writer und Cyclone-Reader.
}

#[test]
fn negotiation_matches_xcdr2_when_both_support() {
    // Happy path — Voraussetzung fuer Full-Interop ist dass beide
    // Seiten XCDR2 offerieren. Test ohne Cyclone moeglich.
    let neg = negotiate_representation(&[2], &[2]);
    assert_eq!(
        neg,
        zerodds_types::qos::RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
    );
}

#[test]
fn typeinfo_for_cyclone_chatter_has_nonzero_sizes() {
    // Sanity: TI enthaelt tatsaechliche Groessen.
    let b = TypeObjectBuilder::struct_type("::chat::Chatter").member(
        "id",
        TypeIdentifier::Primitive(PrimitiveKind::Int64),
        |m| m,
    );
    let minimal = MinimalTypeObject::Struct(b.build_minimal());
    let complete = zerodds_types::type_object::CompleteTypeObject::Struct(b.build_complete());
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    assert!(ti.minimal.typeid_with_size.typeobject_serialized_size > 0);
    assert!(ti.complete.typeid_with_size.typeobject_serialized_size > 0);
    // Complete ist typischerweise groesser (enthaelt Namen).
    assert!(
        ti.complete.typeid_with_size.typeobject_serialized_size
            > ti.minimal.typeid_with_size.typeobject_serialized_size
    );
}
