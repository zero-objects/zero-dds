//! C4.2 — TypeLookup-Service Integration-Tests (XTypes 1.3 §7.6.3.3.4).
//!
//! Diese Tests decken das types-Layer ab: TypeRegistry-Helfer
//! (`dependencies_of`, `transitive_dependencies`) und das Cross-
//! Cycling von Server-Build → Client-Parse via [`GetTypesReply`]-
//! Wire-Roundtrip.

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
use zerodds_types::type_lookup::{GetTypesReply, ReplyTypeObject};
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
    // A → B → C, alle in Registry.
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
    // Sollte sowohl B als auch C enthalten (Reihenfolge ist nicht
    // garantiert deterministisch, aber Set-Inhalt schon).
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
    // Server-Side: build a Reply mit zwei TypeObjects.
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
        types: alloc::vec![ReplyTypeObject::Minimal(m1), ReplyTypeObject::Minimal(m2)],
    };

    // Wire-Roundtrip.
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let mut w = BufferWriter::new(Endianness::Little);
    reply.encode_into(&mut w).unwrap();
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = GetTypesReply::decode_from(&mut r).unwrap();
    assert_eq!(decoded.types.len(), 2);

    // Client-Side: insert into registry und checke dass die Hashes
    // konsistent sind.
    let mut reg = TypeRegistry::new();
    for it in decoded.types {
        match it {
            ReplyTypeObject::Minimal(m) => {
                let h = zerodds_types::compute_hash(&TypeObject::Minimal(m.clone())).unwrap();
                reg.insert_minimal(h, m);
            }
            ReplyTypeObject::Complete(c) => {
                let h = zerodds_types::compute_hash(&TypeObject::Complete(c.clone())).unwrap();
                reg.insert_complete(h, c);
            }
        }
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
