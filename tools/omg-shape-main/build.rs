// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ShapeType`, generated from the suite's own IDL.
//!
//! The client used to take the type from `zerodds_dcps::interop`, which
//! is written by hand. That is the wrong side of the measurement: a run
//! of the OMG `dds-rtps` suite would then report on the hand-written
//! type and not on what this project generates — a defect in the
//! generator cannot fail the suite, and a defect in the hand-written
//! type is charged to us. Both directions were real: the hand-written
//! `ShapeType` did not override `encode_be`/`decode_be` (the `DdsType`
//! defaults delegate to little-endian, so a big-endian writer stamped
//! `D_CDR2_BE` onto an LE body), and it left `type_object()` at its
//! default `None`, which announces a topic type name and no type at all
//! — the very shape `interop/cyclone-xtypes-27/reader/build.rs` records
//! as what a matrix is there to measure.
//!
//! `dds_type_impls` writes the `DdsType` impl from the same model the
//! codec came from, so the announced `EXTENSIBILITY` and the codec
//! cannot disagree, and `create_datawriter`/`create_datareader` have a
//! `type_object()` to register with the TypeLookup server (XTypes 1.3
//! §7.6.3.2).
//!
//! The hand-written type stays where it is: it is the second path of
//! `crates/dcps/tests/shapes_type_idl_agreement.rs`, which holds the
//! octets of both against each other in all four modes.

fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("../../interop/shapes")
        .default_extensibility(zerodds_build::DefaultExt::Appendable)
        .dds_type_impls(true)
        .compile(&["../../interop/shapes/omg-dds-rtps.idl"])
}
