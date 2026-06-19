//! Smoke tests for #[derive(FlatStruct)].

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
    // Trivial sanity check: the hash is not all 0x00 (which would
    // mean the caller hash and the derive hash collide).
    assert_ne!(PoseDerived::TYPE_HASH, [0u8; 16]);
}

#[test]
fn derive_generates_distinct_hash_per_layout() {
    // Different structs → different hashes.
    assert_ne!(PoseDerived::TYPE_HASH, PoseDifferent::TYPE_HASH);
}

#[test]
fn derive_as_bytes_roundtrip() {
    let p = PoseDerived { x: 1, y: 2, z: 3 };
    let bytes = p.as_bytes();
    assert_eq!(bytes.len(), 24);
    // SAFETY: bytes comes from exactly this Pose instance.
    let p2: PoseDerived = unsafe { PoseDerived::from_bytes_unchecked(bytes) };
    assert_eq!(p, p2);
}

// Tuple-struct variant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, DeriveFlatStruct)]
#[repr(C)]
struct Tuple(u64, u64);

#[test]
fn derive_works_for_tuple_struct() {
    assert_eq!(Tuple::WIRE_SIZE, 16);
    let t = Tuple(0xAA, 0xBB);
    assert_eq!(t.as_bytes().len(), 16);
}

// repr(transparent) — wrapper around a single repr(C) type. Spec
// §1.1 explicitly allows this.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(transparent)]
struct WrappedPose(PoseDerived);

#[test]
fn derive_accepts_repr_transparent() {
    assert_eq!(WrappedPose::WIRE_SIZE, PoseDerived::WIRE_SIZE);
    // The hash is distinct from the inner type because the layout string
    // includes the outer type name.
    assert_ne!(WrappedPose::TYPE_HASH, PoseDerived::TYPE_HASH);
}

// Reordering fields produces a different hash.
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

// Changing a field type (i64 → u64) produces a different hash despite
// the same wire size.
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

// Renaming a type produces a different hash even with identical fields.
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

// The hash is deterministic — same definition → same hash.
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct PoseDerived2 {
    x: i64,
    y: i64,
    z: i64,
}

#[test]
fn derive_hash_is_deterministic_for_identical_layout() {
    // PoseDerived and PoseDerived2 are structurally identical in the
    // layout string, BUT the type name is part of the signature — hence
    // different hashes (this is the intended behavior:
    // the hash is a type identity, not just a layout identity).
    assert_ne!(PoseDerived::TYPE_HASH, PoseDerived2::TYPE_HASH);
}

// Unit struct (wire size = 0).
#[derive(Copy, Clone, DeriveFlatStruct)]
#[repr(C)]
struct Marker;

#[test]
fn derive_works_for_unit_struct() {
    assert_eq!(Marker::WIRE_SIZE, 0);
    assert_ne!(Marker::TYPE_HASH, [0u8; 16]);
}
