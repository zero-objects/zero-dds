# Changelog — `zerodds-c-api`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer.

## [Unreleased]

### Added

- **`xcdr2` module** — implements the `zerodds-xcdr2-c-1.0`
  vendor spec.
  - `ZeroDdsTypeSupport` (`zerodds_typesupport_t` C-side) as a
    function-table-based TypeSupport struct.
  - FFI functions `zerodds_topic_create_typed`,
    `zerodds_topic_destroy_typed`, `zerodds_writer_write_typed`,
    `zerodds_reader_take_typed`, `zerodds_xcdr2_encode`,
    `zerodds_xcdr2_decode`.
  - `ZeroDdsTopic` opaque handle for typed topics.
  - Helper functions `copy_to_out_buf`, `input_slice`, `write_out_len`
    for codegen encoders/decoders.
- **`include/zerodds_xcdr2.h`** — hand-maintained C99 header with the
  TypeSupport struct + FFI forward declarations + inline helpers for the
  codegen output (`zerodds_xcdr2_c_write_uN/iN/fN/string`,
  `zerodds_xcdr2_c_read_*`, `zerodds_xcdr2_c_kh_write_*`,
  `zerodds_xcdr2_c_compute_key_hash`, built-in MD5).
- **L1 wire-conformance tests** in `tests/xcdr2_wire_vectors.rs` —
  check V-1..V-12 byte-exactly over the FFI pattern.
- **L2 codegen-conformance tests** in `tests/xcdr2_c_codegen.rs` —
  verify the `idl-cpp::generate_c_header` output for each
  V-1..V-12 IDL snippet.
- **L2 C-compile tests** in `tests/xcdr2_c_compile.rs` — compile
  each generated header with `cc -std=c99 -Wall -Werror`, to guarantee C99
  validity (skipped if no C compiler is in PATH).

### Changed

- `Cargo.toml`: add `zerodds-cdr` as a runtime dependency (with
  `alloc` + `std` features), `zerodds-idl` + `zerodds-idl-cpp` as
  dev-dependencies for codegen/wire tests.
- `src/lib.rs`: registers `pub mod xcdr2`.

### Notes

- **Codegen path choice** (vendor spec §4): **Option A** implemented — a new
  module `crates/idl-cpp/src/c_mode.rs` with `generate_c_header` /
  `CGenOptions`, integrated into the existing `zerodds-idl-cpp`
  crate. Limited to the V-1..V-12 corpus + extensible form (final/
  appendable/mutable structs, primitive types, `string`, `sequence<T>`,
  nested modules, `@key`, `@id`).
- **Wire-vector errata** vs. `zerodds-xcdr2-bindings-conformance-1.0`
  §6:
  - **V-3** the spec doc shows a 40-byte wire with three numeric errors
    (`l = -1234567`, `ul = 2345678`, `ll = -987654321`); XCDR2-spec-
    conformant LE bytes yield 48 bytes. Tests check the corrected
    form, which interoperates with `zerodds-cdr` and Cyclone DDS.
  - **V-10**/V-11/V-12 EMHEADER is shown visually BE-grouped in the spec doc;
    the wire bytes are LE-serialized (XCDR2 stream
    endianness applies to all fields, incl. EMHEADER). Tests assert
    the LE form, consistent with `zerodds-cdr::struct_enc` and Cyclone DDS.

### Added — ABI compat test

- `tests/abi_compat.rs` + `abi.snapshot.json` as a 185-symbol baseline
  for drift detection: every new or renamed FFI symbol entry
  makes the test fail, until `abi.snapshot.json` is updated with a
  justification. `serde` + `serde_json` as dev-deps.
