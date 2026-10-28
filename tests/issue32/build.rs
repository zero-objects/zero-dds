// SPDX-License-Identifier: Apache-2.0
// `Robot.rs` used to be a checked-in fixture of the retired
// `zerodds-idlc` generation; the rebuild removed that generator and
// `zerodds-cdr` with it. The type is now generated at build time from
// the same IDL, through the path every consumer takes.
fn main() -> Result<(), zerodds_build::Error> {
    zerodds_build::Config::new()
        .include_dir("idl")
        .type_identifier_constants(true)
        .compile(&["idl/Robot.idl"])
}
