// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The suite's 25 shape types, generated from the suite's own IDL.
//!
//! `tools/omg-shape-main` took its `ShapeType` from a hand-written Rust
//! struct before commit `98b4800bdc3`, and that was the wrong side of
//! the measurement: a defect in the generator could then not fail the
//! suite, and a defect in the hand form was charged to us. Both
//! directions were real there — the hand-written type left
//! `DdsType::type_object()` at its default `None` (a topic type name
//! announced with no type behind it) and did not override `encode_be`.
//!
//! This client starts on the generated side, and for this suite that is
//! not a preference but the whole point: `dds-xtypes` measures type
//! assignability. Its 625 publisher/subscriber pairs ask what a
//! `TypeObject` says about a type and whether two of them are
//! assignable (XTypes 1.3 §7.2.4.4.4.4). A hand-written type would put
//! a hand-written answer on both sides of that question.
//!
//! `default_extensibility(Appendable)` states what the suite's
//! unannotated `Shape…Default` types mean. XTypes 1.3 §7.3.1.2.1.9
//! makes APPENDABLE the default extensibility of an aggregated type,
//! and the suite's `compatibility_matrix.xlsx` agrees: every
//! Default/Extensible cell in it carries the same value as the
//! corresponding Extensible/Extensible cell. Leaving it to the front
//! end's own default would make the five `…Default` types mean whatever
//! that default happens to be.

fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("../../interop/shapes")
        .default_extensibility(zerodds_build::DefaultExt::Appendable)
        .dds_type_impls(true)
        .compile(&["../../interop/shapes/omg-dds-xtypes.idl"])
}
