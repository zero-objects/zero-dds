//! WP 1.5 T19 — full live-interop skeleton against Cyclone DDS.
//!
//! **Opt-in only** — `#[ignore]`. Goal: a user-data sample flows
//! between ZeroDDS and Cyclone with XCDR2 encoding. This needs:
//!
//! 1. DCPS-style API (DataWriter/DataReader) — phase 2 (WP 2.*)
//! 2. Representation negotiation from T18 — OK, present
//! 3. Assignability check from T16 — OK
//! 4. TypeLookup endpoints at the transport — WP 1.6+
//!
//! This test documents the test architecture; the actual
//! implementation comes with phase 2.

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

    // 2. Build TypeInformation
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    eprintln!(
        "type_information: minimal_hash={:?}",
        ti.minimal.typeid_with_size.type_id
    );

    // 3. Representation negotiation (the macOS host offers [XCDR2, XCDR1],
    //    Cyclone accepts [XCDR2, XCDR1] — match: XCDR2).
    let neg = negotiate_representation(&[2, 0], &[2, 0]);
    assert_eq!(
        neg,
        zerodds_types::qos::RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
    );

    // 4. TODO WP 2.*: Create DataWriter<Chatter>, publish Sample.
    // 5. TODO WP 2.*: Cyclone-Subscriber-Subprocess connects + receives.
    // 6. TODO WP 2.*: Assert: sample.id + sample.text round-trip
    //    byte-exact between the ZeroDDS writer and the Cyclone reader.
}

#[test]
fn negotiation_matches_xcdr2_when_both_support() {
    // Happy path — a precondition for full interop is that both sides
    // offer XCDR2. This test works without Cyclone.
    let neg = negotiate_representation(&[2], &[2]);
    assert_eq!(
        neg,
        zerodds_types::qos::RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
    );
}

#[test]
fn typeinfo_for_cyclone_chatter_has_nonzero_sizes() {
    // Sanity: TI contains actual sizes.
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
    // Complete is typically larger (contains names).
    assert!(
        ti.complete.typeid_with_size.typeobject_serialized_size
            > ti.minimal.typeid_with_size.typeobject_serialized_size
    );
}
