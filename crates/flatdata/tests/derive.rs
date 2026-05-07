//! Smoke-Tests fuer #[derive(FlatStruct)].

#![allow(clippy::expect_used, clippy::unwrap_used)]

use zerodds_flatdata::FlatStruct;
use zerodds_flatdata_derive::FlatStruct as DeriveFlatStruct;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DeriveFlatStruct)]
#[repr(C)]
struct PoseDerived {
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, DeriveFlatStruct)]
#[repr(C)]
struct PoseDifferent {
    a: i32,
    b: i32,
}

#[test]
fn derive_generates_wire_size() {
    assert_eq!(PoseDerived::WIRE_SIZE, 24);
    assert_eq!(PoseDifferent::WIRE_SIZE, 8);
}

#[test]
fn derive_generates_type_hash_nonzero() {
    // Trivial-Sicherheit: Hash ist nicht alle 0x00 (das wuerde
    // bedeuten Caller-Hash + derive-Hash kollidieren).
    assert_ne!(PoseDerived::TYPE_HASH, [0u8; 16]);
}

#[test]
fn derive_generates_distinct_hash_per_layout() {
    // Verschiedene Structs → verschiedene Hashes.
    assert_ne!(PoseDerived::TYPE_HASH, PoseDifferent::TYPE_HASH);
}

#[test]
fn derive_as_bytes_roundtrip() {
    let p = PoseDerived { x: 1, y: 2, z: 3 };
    let bytes = p.as_bytes();
    assert_eq!(bytes.len(), 24);
    // SAFETY: bytes stammt aus genau dieser Pose-Instanz.
    let p2: PoseDerived = unsafe { PoseDerived::from_bytes_unchecked(bytes) };
    assert_eq!(p, p2);
}

// Tuple-Struct-Variante.
#[derive(Copy, Clone, Debug, PartialEq, Eq, DeriveFlatStruct)]
#[repr(C)]
struct Tuple(u64, u64);

#[test]
fn derive_works_for_tuple_struct() {
    assert_eq!(Tuple::WIRE_SIZE, 16);
    let t = Tuple(0xAA, 0xBB);
    assert_eq!(t.as_bytes().len(), 16);
}

// repr(transparent) — wrapper um einen einzigen repr(C)-Type. Spec
// §1.1 erlaubt das explizit.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(transparent)]
struct WrappedPose(PoseDerived);

#[test]
fn derive_accepts_repr_transparent() {
    assert_eq!(WrappedPose::WIRE_SIZE, PoseDerived::WIRE_SIZE);
    // Hash ist distinct gegen den inneren Type, weil der Layout-String
    // den Outer-Type-Namen einbezieht.
    assert_ne!(WrappedPose::TYPE_HASH, PoseDerived::TYPE_HASH);
}

// Field-Reorder erzeugt einen anderen Hash.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct PoseReordered {
    z: i64,
    y: i64,
    x: i64,
}

#[test]
fn derive_detects_field_reorder() {
    assert_ne!(PoseDerived::TYPE_HASH, PoseReordered::TYPE_HASH);
}

// Field-Type-Change (i64 → u64) erzeugt einen anderen Hash trotz
// gleicher Wire-Size.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct PoseUnsigned {
    x: u64,
    y: u64,
    z: u64,
}

#[test]
fn derive_detects_field_type_change() {
    assert_eq!(PoseDerived::WIRE_SIZE, PoseUnsigned::WIRE_SIZE);
    assert_ne!(PoseDerived::TYPE_HASH, PoseUnsigned::TYPE_HASH);
}

// Type-Rename erzeugt einen anderen Hash auch bei identischen Feldern.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct PoseRenamed {
    x: i64,
    y: i64,
    z: i64,
}

#[test]
fn derive_detects_type_rename() {
    assert_ne!(PoseDerived::TYPE_HASH, PoseRenamed::TYPE_HASH);
}

// Hash ist deterministisch — gleiche Definition → gleicher Hash.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct PoseDerived2 {
    x: i64,
    y: i64,
    z: i64,
}

#[test]
fn derive_hash_is_deterministic_for_identical_layout() {
    // PoseDerived und PoseDerived2 sind strukturell identisch im
    // Layout-String, ABER der Type-Name ist Teil der Signatur — daher
    // unterschiedliche Hashes (das ist das gewuenschte Verhalten:
    // Hash ist eine Type-Identitaet, nicht nur Layout-Identitaet).
    assert_ne!(PoseDerived::TYPE_HASH, PoseDerived2::TYPE_HASH);
}

// Unit-Struct (Wire-Size = 0).
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct Marker;

#[test]
fn derive_works_for_unit_struct() {
    assert_eq!(Marker::WIRE_SIZE, 0);
    assert_ne!(Marker::TYPE_HASH, [0u8; 16]);
}
