//! Two preconditions of user-data interop that need no peer: the
//! representation two endpoints agree on, and a `TypeInformation` built
//! from the type objects.
//!
//! The exchange itself is measured against the vendors, not here:
//! `tests/interop/ci/run_vendor.sh cyclone` and `… fastdds` (CI jobs
//! `interop-cyclone` and `interop-fastdds`) carry samples in both
//! directions, refuse the incompatible pairs and fetch a writer's type
//! with TypeLookup. This file held a `#[ignore]`d placeholder for that
//! exchange, "requires DCPS API (WP 2.*)", which no job ran; its local
//! half is the first test below.

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
fn negotiation_takes_xcdr2_when_both_offer_both_versions() {
    // Cyclone offers [XCDR2, XCDR1]; so does ZeroDDS. The first common
    // entry of the offer wins.
    let neg = negotiate_representation(&[2, 0], &[2, 0]);
    assert_eq!(
        neg,
        zerodds_types::qos::RepresentationNegotiation::Accepted(DataRepresentationId::Xcdr2)
    );
}

#[test]
fn type_information_is_built_from_a_keyed_appendable_struct() {
    let b = TypeObjectBuilder::struct_type("::chat::Chatter")
        .extensibility(Extensibility::Appendable)
        .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
            m.key()
        })
        .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m);
    let minimal = MinimalTypeObject::Struct(b.build_minimal());
    let complete = zerodds_types::type_object::CompleteTypeObject::Struct(b.build_complete());
    let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    assert_ne!(
        ti.minimal.typeid_with_size.type_id,
        ti.complete.typeid_with_size.type_id
    );
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
