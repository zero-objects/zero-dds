# `zerodds-xcdr2-c` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-c-1.0.md` (185 lines) -- the ZeroDDS C-FFI XCDR2 encoding spec.

Implementation:

- `crates/zerodds-c-api/` — C-FFI XCDR2 TypeSupport encoding.

## §1 Motivation

### §1 No OMG DDS C-PSM spec

**Spec:** §1 -- "There is no OMG DDS C-PSM spec. The existing zerodds-c-api-1.0 covers entity lifecycle and QoS, but not typed encoding."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `zerodds_typesupport_t` struct with a function table

**Spec:** §2 -- a C struct with `type_hash[16]`, `type_name`, `is_keyed`, `extensibility`, plus the function pointers `encode/decode/key_hash/sample_free`.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs` defines `zerodds_typesupport_t` as a `#[repr(C)]` struct in `crates/zerodds-c-api/include/zerodds/typesupport.h`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_compile.rs` (12 tests) compiles the generated C code against the header.

**Status:** done

## §3 Required FFI functions

### §3 Topic + writer/reader + standalone encode/decode

**Spec:** §3 -- 6 FFI functions: `zerodds_topic_create_typed`, `zerodds_writer_write_typed`, `zerodds_reader_take_typed`, `zerodds_xcdr2_encode`, `zerodds_xcdr2_decode`. Return codes 0=OK, -7=BAD_PARAMETER, -13=BUFFER_TOO_SMALL, -3=UNSUPPORTED.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs` exports the FFI functions. Status codes via the central `zerodds-c-api` status mapping.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests) calls the FFI encoders/decoder; `crates/zerodds-c-api/tests/smoke_ffi.rs`.

**Status:** done

## §4 Codegen requirement

### §4 idl-cpp `--c-mode` codegen

**Spec:** §4 -- "Per IDL `struct`, a C codegen (idl-c, if it exists, or as a task of idl-cpp via an `extern C` wrapper) must provide: the data structure `MyType_t` + `extern const zerodds_typesupport_t MyType_typesupport`."

**Repo:** `crates/idl-cpp/src/c_mode.rs` (`emit_struct`, `emit_encode_body`, `emit_decode_body`, `emit_key_hash_body`, `emit_free_body`) -- a full C codegen path. The `MyType_typesupport` static table in emit_struct.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs` (12 tests) verifies the generated C output.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-C99 types + wire layout

**Spec:** §5, table of 16 IDL types → C99 → XCDR2 LE. Strings + sequences are heap-allocated; sample_free() MUST free them.

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_primitive_write`, `emit_sequence_write`, `emit_member_write`. Heap allocation in `emit_decode_body` with malloc.

**Tests:** V-4 (string), V-5/V-6 (sequence) in `xcdr2_wire_vectors.rs`; memory free in `xcdr2_c_compile.rs`.

**Status:** done

## §6 Memory ownership

### §6 Caller/callee contract for encode/decode/free

**Spec:** §6, table of 4 entries -- the caller provides `out_buf` (or NULL for a size probe), the callee writes `out_len`; decode allocates strings/sequences in the sample; `ts.sample_free` frees the heap pointers.

**Repo:** `crates/zerodds-c-api/src/xcdr2.rs::zerodds_xcdr2_encode` size probe with NULL `out_buf` (returns the required size in `out_len`). The decode path allocates via the codegen-emitted `MyType_decode`.

**Tests:** size-probe test + free-path test in `xcdr2_c_compile.rs`.

**Status:** done

## §7 Conformance

### §7 L1 wire (V-1..V-12 byte-exact via FFI)

**Spec:** §7 -- "L1 (wire): `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` checks V-1..V-12 byte-exact via FFI."

**Repo:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests).

**Tests:** as above.

**Status:** done

### §7 L2 codegen

**Spec:** §7 -- "L2 (codegen): the C codegen is part of idl-cpp (the `--c-mode` flag) OR a separate idl-c crate."

**Repo:** `crates/idl-cpp/src/c_mode.rs` as part of idl-cpp. Driver `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs` (12 tests).

**Tests:** as above.

**Status:** done

### §7 L3 cross-language

**Spec:** §7 -- "L3 (cross-language): C encoder vs Rust decoder, C decoder vs Rust encoder."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_3_c_ffi_binding` calls the zerodds-c-api wire-vector test suite via subprocess against the identical V-1..V-12 hex fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_3_c_ffi_binding`.

**Status:** done

### §7 L4 cross-vendor

**Spec:** §7 -- "L4 (cross-vendor): C-FFI over RTPS against Cyclone DDS."

**Repo:** `tests/interop/xcdr2_cross_vendor.sh` orchestrates the cross-vendor setup; the fixture tree `crates/discovery/tests/fixtures/cyclone-xcdr2/` holds V-1..V-12. The C-FFI encoder dispatches over the same `crates/cdr` logic; therefore `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` also covers Cyclone equivalence for the C-FFI. All 12 vectors were live-captured against Cyclone DDS 0.11 (forced XCDR2) on the Linux bench host and byte-compared: V-2..V-9 + V-11b byte-exact (V-3/V-8 confirm the XCDR2 64-bit alignment cap §7.4.1.1.1, V-6 the `sequence<string>` DHEADER §7.4.3.5); V-10/V-11a conformant LC divergence (not a bug, allowed by spec §6, decoder reads all LCs). Both discovered gaps (alignment + sequence DHEADER) are fixed across all 6 bindings; `v6.bin` corrected.

**Tests:** `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests); encoder side `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 tests) green against the corrected vectors (V-6 with DHEADER, V-3/V-8 4-byte-aligned).

**Status:** done -- all deterministic vectors (V-1..V-9, V-11b) byte-exact against Cyclone DDS 0.11; mutable V-10/V-11a conformant LC divergence (spec-allowed, roundtrip interop). Per-capture procedure reproducible on the Linux bench host.

## §8 Examples

### §8 C smoke demo

**Spec:** §8 -- "`#include zerodds.h` + the generated `MyType_typesupport`, topic-create-typed + writer."

**Repo:** the smoke-demo snippet in the spec body (illustrative).

**Tests:** compile path in `xcdr2_c_compile.rs`.

**Status:** done

## §9 Errata + edge cases

### §9.1 const strings via malloc/free

**Spec:** §9.1 -- "MyType_t.text is `char*` (mutable); decode allocates via malloc and sample_free calls free."

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_decode_body` malloc path; `emit_free_body` free path.

**Tests:** string-member test in `xcdr2_c_codegen.rs`.

**Status:** done

### §9.2 Sequence bound check

**Spec:** §9.2 -- "The generated decode checks the bound from the IDL `sequence<T, N>` annotation and returns -7 on a violation."

**Repo:** `crates/idl-cpp/src/c_mode.rs::emit_sequence_read` with a bound check.

**Tests:** bound edge case in `crates/idl-cpp/tests/edge_cases.rs`.

**Status:** done

### §9.3 C99 vs C++ ABI

**Spec:** §9.3 -- "The structures are `extern C`-compatible; C++ consumers link directly against the C-FFI."

**Repo:** `#[repr(C)]` structs + `extern "C"` wrappers in `xcdr2.rs`.

**Tests:** `xcdr2_c_compile.rs` compiles C99 + C++ consumers alike.

**Status:** done

### §9.4 enum width int32_t

**Spec:** §9.4 -- "The C codegen emits enum types with explicit `int32_t` storage (not `int`) for ABI stability."

**Repo:** `crates/idl-cpp/src/c_mode.rs` enum path emits an `int32_t` typedef.

**Tests:** enum tests in `xcdr2_c_codegen.rs`.

**Status:** done

### §9.5 Reserved-keyword escaping

**Spec:** not explicitly normative in `zerodds-xcdr2-c-1.0` (no OMG C-PSM
spec exists, §1); tracked here because until 2026-07-28 (github-triage
#14) the C-mode path had **no** reserved-word check or escaping at all —
an IDL identifier colliding with a C keyword (`int`, `struct`, `default`,
`register`, ...) silently produced invalid C (`int32_t int;`), a gap the
sibling full-C++ path (`idl4-cpp-1.0.en.md` §7.1.2) at least caught via a
hard reject, C-mode did not even do that.

**Repo:** `crates/idl-cpp/src/c_keywords.rs` — full ISO/IEC 9899 C89..C23
keyword table (`C_RESERVED`) + `escape_c_ident` (trailing `_` suffix,
C's idiomatic keyword-escape convention; C has no raw-identifier/
stropping syntax). Wired into `crates/idl-cpp/src/c_mode.rs` at every
site a bare (non module-prefixed) C token is emitted from an IDL name:
`c_identifier` (unscoped struct/union/enum/bitmask/bitset/typedef type
names), `union_case_field` (union payload field names), and every
struct/union member field-name derivation used in both the `typedef
struct` declaration and the encode/decode/free/key-hash bodies (so
declaration and field-access sites stay consistent). Module-prefixed
compound tokens (`{c_name}_{enumerator}`) are left as-is since they
cannot collide with a bare keyword by construction.

**Tests:** `crates/idl-cpp/src/c_mode.rs::tests::{struct_field_named_c_keyword_is_escaped,
top_level_type_named_c_keyword_is_escaped,
union_case_field_named_c_keyword_is_escaped,
enum_value_named_c_keyword_stays_compound_and_valid}` +
`crates/idl-cpp/src/c_keywords.rs::tests::*` (list coverage, escape
round-trip, all-keywords sweep) + `crates/zerodds-c-api/tests/xcdr2_c_compile.rs::v12_keyword_identifiers_compiles`
(real `-std=c99 -Wall -Werror` `gcc`/`clang` compile of a union +
struct with `int`/`register`/`static`/`for` identifiers — proves the
escaped output is actually valid C, not just structurally
keyword-free).

**Status:** done

---

## Audit status

15 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-c-api` -- unittest 68 + smoke_ffi 1 + xcdr2_c_codegen 12 + xcdr2_c_compile 12 + xcdr2_wire_vectors 13 = 106 tests green, 0 failed; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_3_c_ffi_binding` -- 1 test green; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 tests green.
