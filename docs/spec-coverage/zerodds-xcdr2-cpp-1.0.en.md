# `zerodds-xcdr2-cpp` 1.0 -- Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-cpp-1.0.md` (213 lines) -- the ZeroDDS C++17 TypeSupport codegen spec.

Implementation:

- `crates/cpp/` — C++17 XCDR2 TypeSupport codegen.

## §1 Motivation

### §1 OMG DDS-PSM-Cxx gap for TypeSupport

**Spec:** §1 -- "OMG DDS-PSM-Cxx 1.0 mandates TypeSupport registration in `Participant::create_topic`, but specifies no concrete `topic_type_support<T>` trait."

**Repo:** the motivation text of the vendor spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport pattern

### §2 `topic_type_support<T>` forward in `dds::topic`

**Spec:** §2 -- "template <typename T> struct topic_type_support; // forward (specialization per T)" in namespace `dds::topic`.

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp` declares the forward template; specializations per IDL `struct` are emitted by the idl-cpp codegen (`crates/idl-cpp/src/emitter.rs::emit_topic_type_support_specs`).

**Tests:** `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests) verifies the emitted form.

**Status:** done

## §3 Required methods

### §3 type_name() / encode() / encode_be() / decode() / key_hash() / is_keyed() / extensibility()

**Spec:** §3 -- C++ method signatures: `static const char* type_name()`, `static std::vector<uint8_t> encode(const T&)`, `static std::vector<uint8_t> encode_be(const T&)`, `static T decode(const uint8_t*, size_t)`, `static std::array<uint8_t,16> key_hash(const T&)`, `static constexpr bool is_keyed()`, `static constexpr ::dds::core::policy::DataRepresentationKind extensibility()`.

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_topic_type_support_for` (line 1429ff) emits exactly these 7 methods including the `constexpr` for is_keyed/extensibility.

**Tests:** `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (15 V-tests), `crates/idl-cpp/tests/snapshot_codegen.rs`, `crates/idl-cpp/tests/spec_conformance.rs` (30 tests).

**Status:** done

## §4 Codegen requirement (idl-cpp)

### §4 Specialization per IDL `struct` with FQN

**Spec:** §4 -- "Per IDL `struct` (top-level or module-nested), idl-cpp MUST emit a `topic_type_support<FQN>` specialization. The FQN is `::Module::Sub::Struct`."

**Repo:** `crates/idl-cpp/src/emitter.rs::emit_topic_type_support_specs` iterates over all structs of the TU; FQN emission via the `cpp_fqn` variable in `emit_topic_type_support_for`.

**Tests:** `crates/idl-cpp/tests/fixtures.rs` (13 tests), `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests).

**Status:** done

### §4 Plain members + @optional + @key codegen requirement

**Spec:** §4 -- "Required members (from the AST): all plain members (primitive, string, sequence, nested struct, enum, array). `@optional` / `@shared` via the EMHEADER M-flag (Mutable) or a present-flag byte (Final/Appendable). `@key` members in key-hash generation."

**Repo:** `crates/idl-cpp/src/emitter.rs` -- `emit_encode_fn`, `emit_decode_fn`, key-hash generation. `crates/idl-cpp/src/c_mode.rs::emit_key_hash_body`.

**Tests:** V-8 (key), V-9/V-10 (extensibility), V-11 (optional) in `xcdr2_wire_vectors.rs`.

**Status:** done

### §4 Type name without a leading `::`

**Spec:** §4 -- "Type-name form: `Module::Sub::Struct` without a leading `::`."

**Repo:** `crates/idl-cpp/src/emitter.rs::short_name` strips the leading `::`.

**Tests:** V-7 `Outer::Inner::S` assert in the wire-vector test.

**Status:** done

## §5 Wire type mapping

### §5 IDL-to-C++17 types + wire layout

**Spec:** §5, table of 18 IDL types → C++ → XCDR2 LE.

**Repo:** `crates/idl-cpp/src/type_map.rs` maps IDL primitives to C++ types. `crates/cpp/include/dds/topic/xcdr2.hpp` `write_le<T>/read_le<T>` templates.

**Tests:** V-3 mixed primitives, V-4 string, V-5 Sequence<int32>, V-6 Sequence<string>, plus `compile_check.rs` (16 tests) and `edge_cases.rs` (15 tests).

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable mode encoder

**Spec:** §6 -- "Codegen default: @appendable. The helper library provides a dedicated writer class per mode: `xcdr2::FinalWriter`, `xcdr2::AppendableWriter`, `xcdr2::MutableWriter`."

**Repo:** `crates/cpp/include/dds/topic/xcdr2.hpp` holds FinalWriter/AppendableWriter/MutableWriter. `crates/idl-cpp/src/emitter.rs::struct_extensibility` -> emit_encode_fn dispatch.

**Tests:** V-1..V-8 (Final), V-9 (Appendable), V-10/V-11 (Mutable) in `xcdr2_wire_vectors.rs`.

**Status:** done

## §7 Key extraction

### §7 PlainCdr2BeKeyHolder + MD5

**Spec:** §7 -- "Per XTypes §7.6.8: `PlainCdr2BeKeyHolder` (big-endian Plain-CDR2 over only `@key` members), MD5 of it, 16 bytes. If is_keyed()==false: 16 null bytes."

**Repo:** `crates/cpp/include/dds/topic/xcdr2_md5.hpp` (RFC-1321 MD5). `crates/idl-cpp/src/emitter.rs` emits key_hash() with holder-write_be and md5().

**Tests:** V-8 in `xcdr2_wire_vectors.rs` (key hash zero-padded for a holder ≤ 16 bytes; the MD5 path for >16 bytes via helper self-tests).

**Status:** done

## §8 Helper library

### §8 TopicTraits.hpp + xcdr2.hpp + xcdr2_md5.hpp

**Spec:** §8, table header/content -- 3 entries: `TopicTraits.hpp` (forward + ByteSeq/string defaults), `xcdr2.hpp` (primitive helpers, padding, DHEADER, EMHEADER), `xcdr2_md5.hpp` (RFC 1321).

**Repo:** `crates/cpp/include/dds/topic/TopicTraits.hpp`, `crates/cpp/include/dds/topic/xcdr2.hpp`, `crates/cpp/include/dds/topic/xcdr2_md5.hpp`.

**Tests:** `crates/idl-cpp/tests/clang_roundtrip.rs` (1 test, ignored on systems without clang); the generated code is linked against the header.

**Status:** done

### §8 Pure C++17 header-only

**Spec:** §8 -- "Pure C++17, header-only. No linking against the Rust layer; cross-compile-safe."

**Repo:** three .hpp files without a .cpp sibling. No Rust-FFI dependency.

**Tests:** `compile_check.rs` (16 tests) compiles the generated output.

**Status:** done

## §9 Conformance

### §9 L1 wire (V-1..V-12 byte-exact)

**Spec:** §9 -- "L1 (wire): `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` checks all V-1..V-12 byte-exact."

**Repo:** `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (15 #[test] functions).

**Tests:** v1_empty_final_struct, v2_plain_primitives_final, v3_mixed_primitives_final, v4_string_final, v5_sequence_int_final, v6_sequence_string_final, v7_nested_modules_final, v8_keyed_struct_final, v9_appendable_struct, v10_mutable_struct + 5 more.

**Status:** done

### §9 L2 codegen snapshots

**Spec:** §9 -- "L2 (codegen): `crates/idl-cpp/tests/snapshots/` contains a snapshot per vector."

**Repo:** `crates/idl-cpp/tests/snapshots/` dir with snapshot files; driver `crates/idl-cpp/tests/snapshot_codegen.rs` (10 tests).

**Tests:** `snapshot_codegen.rs` validates the generated outputs for all V-type IDL inputs.

**Status:** done

### §9 L3 cross-language runner

**Spec:** §9 -- "L3 (cross-language): `crates/conformance/tests/cross_language_xcdr2.rs` calls `cpp_runner`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs` holds the multi-language driver including the C++ binding call via subprocess against the idl-cpp wire-vector suite.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_2_cpp_binding`.

**Status:** done

### §9 L4 cross-vendor Cyclone

**Spec:** §9 -- "L4 (cross-vendor): `crates/discovery/tests/cyclone_xcdr2_cpp.rs` round-trip vs. Cyclone."

**Repo:** all 12 vectors were live-captured against Cyclone DDS 0.11 (forced XCDR2) on the Linux bench host and byte-compared; two surfaced gaps fixed (64-bit alignment §7.4.1.1.1, sequence DHEADER §7.4.3.5). The C++ encoder (`idl-cpp`) is byte-verified: `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` compiles+runs the generated C++ and checks V-1..V-12 byte-exact (incl. V-3/V-8 4-byte alignment, V-6 `sequence<string>` DHEADER). V-10/V-11a conformant LC divergence.

**Tests:** `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` (16 tests, compile+run C++) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests).

**Status:** done -- C++ encoder byte-exact against Cyclone DDS 0.11 (V-1..V-9/V-11b), mutable V-10/V-11a conformant LC divergence; generated code compile+run-verified.

## §10 Examples

### §10 topic_typed_smoke.cpp

**Spec:** §10 -- "`crates/cpp/examples/topic_typed_smoke.cpp` is the reference smoke (a generated `topic_type_support<Point>` + a pub/sub loop)."

**Repo:** `crates/cpp/examples/topic_typed_smoke.cpp` (smoke demo).

**Tests:** `compile_check.rs` covers the compile path; a run test is sample-only.

**Status:** done

## §11 Errata + open questions

### §11.1 wchar platform-dependent

**Spec:** §11.1 -- "C++ wchar_t is platform-dependent (4 bytes on Linux, 2 bytes on Windows). The codegen MUST truncate via `static_cast<uint16_t>` and extend on decode."

**Repo:** `crates/idl-cpp/src/emitter.rs` emit_member_write uses an explicit u16 cast for wchar.

**Tests:** wchar smoke in `crates/idl-cpp/tests/edge_cases.rs`.

**Status:** done

### §11.2 long double

**Spec:** §11.2 -- "XCDR2 specifies no long double wire. The codegen rejects long double members with an error."

**Repo:** `crates/idl-cpp/src/type_map.rs` rejects long double via the validator.

**Tests:** edge-case test in `edge_cases.rs`.

**Status:** done

### §11.3 std::variant for union

**Spec:** §11.3 -- "idl-cpp emits an IDL `union` as `std::variant<...>`. The encoder writes the discriminator + selected case per XTypes §7.4.4.5."

**Repo:** `crates/idl-cpp/src/emitter.rs` union path.

**Tests:** union tests in `crates/idl-cpp/tests/spec_conformance.rs`.

**Status:** done

---

## Audit status

18 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-cpp` -- 12 test binaries green, 0 failed (139 unit + 16+15+13+11+15+0+30+10+21+7+15 in integration tests); `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_2_cpp_binding` -- 1 test green; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 tests green.
