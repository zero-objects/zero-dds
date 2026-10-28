//! C4.2 — TypeLookup-Service Integration-Tests (XTypes 1.3 §7.6.3.3.4).
//!
//! These tests cover the types layer: TypeRegistry helpers
//! (`dependencies_of`, `transitive_dependencies`) and the cross-
//! cycling from server build → client parse via the [`GetTypesReply`]
//! wire roundtrip.

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

use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_lookup::{GetTypesReply, TypeIdentifierTypeObjectPair};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier};

fn struct_with_hash_dep(name: &str, dep: EquivalenceHash) -> MinimalTypeObject {
    MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type(name)
            .member("p", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("dep", TypeIdentifier::EquivalenceHashMinimal(dep), |m| m)
            .build_minimal(),
    )
}

#[test]
fn registry_insert_and_get_roundtrip() {
    let mut reg = TypeRegistry::new();
    let m = struct_with_hash_dep("::A", EquivalenceHash([1; 14]));
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    reg.insert_minimal(h, m.clone());
    assert_eq!(reg.get_minimal(&h).unwrap(), &m);
}

#[test]
fn registry_dependencies_of_returns_member_hashes() {
    let dep = EquivalenceHash([0xFE; 14]);
    let m = struct_with_hash_dep("::A", dep);
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    let mut reg = TypeRegistry::new();
    reg.insert_minimal(h, m);

    let deps = reg.dependencies_of(&h);
    assert_eq!(deps, alloc_vec(&[dep]));
}

#[test]
fn registry_dependencies_of_for_unknown_returns_empty() {
    let reg = TypeRegistry::new();
    let deps = reg.dependencies_of(&EquivalenceHash([0; 14]));
    assert!(deps.is_empty());
}

#[test]
fn registry_transitive_dependencies_follows_chain() {
    // A → B → C, all in the registry.
    let c = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::C")
            .member("p", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal(),
    );
    let h_c = zerodds_types::compute_minimal_hash(&c).unwrap();
    let b = struct_with_hash_dep("::B", h_c);
    let h_b = zerodds_types::compute_minimal_hash(&b).unwrap();
    let a = struct_with_hash_dep("::A", h_b);
    let h_a = zerodds_types::compute_minimal_hash(&a).unwrap();

    let mut reg = TypeRegistry::new();
    reg.insert_minimal(h_a, a);
    reg.insert_minimal(h_b, b);
    reg.insert_minimal(h_c, c);

    let deps = reg.transitive_dependencies(&h_a, 1024);
    // Should contain both B and C (the order is not
    // guaranteed deterministic, but the set content is).
    assert!(deps.contains(&h_b));
    assert!(deps.contains(&h_c));
    assert_eq!(deps.len(), 2);
}

#[test]
fn registry_transitive_dependencies_caps_at_max_nodes() {
    // 5 verkettete Types, cap = 2.
    let mut reg = TypeRegistry::new();
    let mut last = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let mut hashes = alloc_vec_empty();
    for i in 0..5 {
        let s = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type(alloc_format(&format!("::S{i}")))
                .member("dep", last.clone(), |m| m)
                .build_minimal(),
        );
        let h = zerodds_types::compute_minimal_hash(&s).unwrap();
        reg.insert_minimal(h, s);
        hashes.push(h);
        last = TypeIdentifier::EquivalenceHashMinimal(h);
    }
    let last_hash = *hashes.last().unwrap();
    let deps = reg.transitive_dependencies(&last_hash, 2);
    assert!(deps.len() <= 2);
}

#[test]
fn reply_typeobject_minimal_decodes_back_into_registry() {
    // Server side: build a reply with two TypeObjects.
    let m1 = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::X")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_minimal(),
    );
    let m2 = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Y")
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int8), |m| m)
            .build_minimal(),
    );
    let h1 = zerodds_types::compute_minimal_hash(&m1).unwrap();
    let h2 = zerodds_types::compute_minimal_hash(&m2).unwrap();

    let reply = GetTypesReply {
        types: alloc::vec![
            TypeIdentifierTypeObjectPair {
                type_identifier: TypeIdentifier::EquivalenceHashMinimal(h1),
                type_object: TypeObject::Minimal(m1),
            },
            TypeIdentifierTypeObjectPair {
                type_identifier: TypeIdentifier::EquivalenceHashMinimal(h2),
                type_object: TypeObject::Minimal(m2),
            },
        ],
        complete_to_minimal: alloc::vec![],
    };

    // A wire round trip, at the alignment the production path uses.
    // `zerodds_types::framing` names it; `Writer::new` is XCDR1's eight.
    let mut w = zerodds_types::framing::xcdr2_writer();
    reply.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    let mut r = zerodds_types::framing::xcdr2_reader(&bytes);
    let decoded = GetTypesReply::decode_from(&mut r).unwrap();
    assert_eq!(decoded.types.len(), 2);

    // Client side: insert into the registry and check that the hashes
    // are consistent.
    let mut reg = TypeRegistry::new();
    for it in decoded.types {
        // The identifier the reply carried is the peer's claim; the
        // registry is keyed by the hash of the object it actually got.
        let stored = reg.insert_object(it.type_object).unwrap();
        assert_eq!(
            TypeIdentifier::EquivalenceHashMinimal(stored),
            it.type_identifier,
            "the identifier beside the object has to be the object's own"
        );
    }
    assert!(reg.get_minimal(&h1).is_some());
    assert!(reg.get_minimal(&h2).is_some());
}

#[test]
fn registry_iter_minimals_yields_inserted() {
    let mut reg = TypeRegistry::new();
    let m = struct_with_hash_dep("::A", EquivalenceHash([0; 14]));
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    reg.insert_minimal(h, m);
    let collected: alloc::vec::Vec<EquivalenceHash> =
        reg.iter_minimals().map(|(k, _)| *k).collect();
    assert_eq!(collected, alloc_vec(&[h]));
}

#[test]
fn registry_iter_completes_yields_inserted() {
    use zerodds_types::CompleteTypeObject;
    let mut reg = TypeRegistry::new();
    let c = CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("::X")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete(),
    );
    let h = zerodds_types::compute_complete_hash(&c).unwrap();
    reg.insert_complete(h, c);
    let collected: alloc::vec::Vec<EquivalenceHash> =
        reg.iter_completes().map(|(k, _)| *k).collect();
    assert_eq!(collected, alloc_vec(&[h]));
}

// --- Helpers ----------------------------------------------------------

extern crate alloc;
use alloc::format;

fn alloc_vec(items: &[EquivalenceHash]) -> alloc::vec::Vec<EquivalenceHash> {
    items.to_vec()
}

fn alloc_vec_empty<T>() -> alloc::vec::Vec<T> {
    alloc::vec::Vec::new()
}

fn alloc_format(s: &str) -> &'static str {
    // Identity wrapper to keep call signature simple.
    Box::leak(s.to_string().into_boxed_str())
}
