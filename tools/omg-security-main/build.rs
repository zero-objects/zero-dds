// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ShapeType`, generated from the suite's own IDL.
//!
//! `tools/omg-shape-main` learned this the expensive way and it is
//! written down there: its `ShapeType` first came from
//! `zerodds_dcps::interop`, written by hand, which is the wrong side of
//! the measurement. A suite run has to measure what this project
//! generates — with a hand-written type a defect in the generator
//! cannot fail the suite, and a defect in the hand form is charged to
//! us. Both were real over there: the hand form did not override
//! `encode_be`/`decode_be` (so a big-endian writer stamped `D_CDR2_BE`
//! onto a little-endian body) and left `type_object()` at its default
//! `None`, announcing a topic type name with no type behind it.
//!
//! This client generates from `interop/shapes/omg-dds-security.idl`
//! from its first commit. There is no hand-written `dds-security`
//! `ShapeType` anywhere in the tree and there is not going to be one.
//!
//! `dds_type_impls` writes the `DdsType` impl from the same model the
//! codec came from, so the announced `EXTENSIBILITY` and the codec
//! cannot disagree, and `create_datawriter`/`create_datareader` have a
//! `type_object()` to register with the TypeLookup server (XTypes 1.3
//! §7.6.3.2).
//!
//! `DefaultExt::Appendable` is not a preference. The suite's IDL
//! carries no extensibility annotation at all, and XTypes 1.3
//! §7.3.1.2.1 makes the absent annotation APPENDABLE. What each
//! vendor's own IDL compiler does with the same file is a separate
//! question the suite does not answer; `README.md` records it as
//! unmeasured.

fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("../../interop/shapes")
        .default_extensibility(zerodds_build::DefaultExt::Appendable)
        .dds_type_impls(true)
        .compile(&["../../interop/shapes/omg-dds-security.idl"])
}
