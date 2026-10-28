// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RoundtripBench::Roundtrip`, generated from the IDL the vendor apps
//! are generated from.
//!
//! `tests/perf/dds-roundtrip-bench/roundtrip.idl` is the one file the
//! Cyclone, RTI and Fast DDS apps of this bench run their own compilers
//! over. Our side used to write the type and its codec by hand in
//! `src/bin/roundtrip_typed.rs`, which is the wrong side of the
//! measurement twice over: an apples-to-apples number is only that if
//! both apples come out of the same IDL, and a codec whose writer and
//! reader are ten lines apart in one file round-trips through its own
//! mistakes. The file's own module docs record two faults that did
//! exactly that.
//!
//! The crate compiles its own copy, `idl/roundtrip.idl`: a published
//! crate carries nothing outside its directory, and `cargo publish`
//! builds it from the packaged files alone. In the repository both
//! files exist, and a copy that differs from the vendor apps' file
//! fails the build rather than measure another type.
//!
//! `dds_type_impls(true)` writes the `DdsType` surface from the same
//! model the codec came from, so `encode_be` is a big-endian encoder
//! rather than the trait default that hands out little-endian octets,
//! and `type_object()` answers with the `CompleteTypeObject` the peer
//! asks the TypeLookup service for (XTypes 1.3 §7.6.3.2).

use std::path::Path;

const IDL: &str = "idl/roundtrip.idl";
const VENDOR_IDL: &str = "../../tests/perf/dds-roundtrip-bench/roundtrip.idl";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={VENDOR_IDL}");
    if Path::new(VENDOR_IDL).exists() && std::fs::read(VENDOR_IDL)? != std::fs::read(IDL)? {
        return Err(
            format!("{IDL} differs from {VENDOR_IDL}, the IDL the vendor apps compile").into(),
        );
    }
    zerodds_build::Config::new()
        .include_dir("idl")
        .default_extensibility(zerodds_build::DefaultExt::Final)
        .dds_type_impls(true)
        .compile(&[IDL])?;
    Ok(())
}
