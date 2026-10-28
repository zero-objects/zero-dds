// SPDX-License-Identifier: Apache-2.0
// The probe's type and its `DdsType` impl, which registers the TypeObject
// with the participant's TypeLookup server (see
// interop/cyclone-xtypes-27/reader/build.rs).
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .dds_type_impls(true)
        .compile(&["roundtrip_probe.idl"])
}
