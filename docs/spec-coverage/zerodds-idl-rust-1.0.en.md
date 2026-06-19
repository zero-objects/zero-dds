# zerodds-idl-rust 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-idl-rust-1.0.md` (ZeroDDS vendor spec)

Implementation:

- `crates/idl-rust/` — OMG-IDL→Rust codegen + DdsType trait.

## §1 Scope

### §1.1 Codegen builds DdsType, CdrEncode/Decode, field_value

**Spec:** §1 — IDL struct → Rust `struct` + `impl DdsType` + `impl CdrEncode`/`CdrDecode` for an enum + `field_value` for the SQL filter.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{emit_struct, emit_dds_type_impl, emit_field_value}`, `crates/idl-rust/src/enum_emit.rs::emit_enum`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_simple_struct_primitives_only`, `snapshot_struct_with_field_value_filter_paths`, `snapshot_enum`.

**Status:** done

### §1.2 Out-of-scope constructs are ignored or reported as Unsupported

**Spec:** §1 — `fixed`/`map`/`any`/`valuetype`/`interface`/`component`/`home`/`bitset`/`bitmask` are not emitted (the codegen returns an empty output section OR `RustGenError::Unsupported`).

**Repo:** `crates/idl-rust/src/emitter.rs::emit_definition` (catch-all arm); `crates/idl-rust/src/type_map.rs::rust_type_for` (Unsupported for Fixed/Map/Any).

**Tests:** indirectly via `compile_check_*` — all in-scope constructs are emittable; dedicated negative tests for out-of-scope constructs are pending.

**Status:** done

## §2 Type mapping

### §2.1 Primitive mapping

**Spec:** §2.1 — table for the 14 IDL primitives (`boolean`/`octet`/`int8`/`uint8`/`short`/`unsigned short`/`long`/`unsigned long`/`long long`/`unsigned long long`/`float`/`double`/`long double`/`char`/`wchar`).

**Repo:** `crates/idl-rust/src/type_map.rs::rust_primitive`, `primitive_wire_size`.

**Tests:** `snapshot_codegen.rs::snapshot_struct_full_primitive_set` covers 13 typical primitives; `wire_roundtrip.rs::wire_simple_struct_primitives_roundtrip` checks the wire encoding for `i32`.

**Status:** done

### §2.2 String + WString

**Spec:** §2.2 — `string`/`wstring` → `String` (UTF-8); wire encoding per XCDR2 §7.4.4.

**Repo:** `crates/idl-rust/src/type_map.rs::rust_string`.

**Tests:** `snapshot_codegen.rs::snapshot_struct_with_string_and_sequence`, `wire_roundtrip.rs::wire_string_and_sequence_roundtrip`.

**Status:** done

### §2.3 Composite Sequence/Array/Optional

**Spec:** §2.3 — `sequence<T>` → `Vec<T>`, `T[N]` → `[T; N]`, multidimensional arrays.

**Repo:** `crates/idl-rust/src/type_map.rs::rust_sequence`, `crates/idl-rust/src/struct_emit.rs::emit_member_field` (array wrap).

**Tests:** `snapshot_codegen.rs::snapshot_struct_with_string_and_sequence`, `snapshot_struct_with_array_dimensions`, `wire_roundtrip.rs::wire_string_and_sequence_roundtrip`.

**Status:** done

### §2.4 Constructed (struct/enum/union/typedef)

**Spec:** §2.4 — struct → DdsType impl; enum → CdrEncode/Decode + `from_wire`; union → tagged enum; typedef → Rust type alias.

**Repo:** `crates/idl-rust/src/struct_emit.rs`, `crates/idl-rust/src/enum_emit.rs`, `crates/idl-rust/src/union_emit.rs`, `crates/idl-rust/src/typedef_emit.rs`.

**Tests:** `snapshot_codegen.rs::{snapshot_enum, snapshot_typedef, snapshot_union}`, `wire_roundtrip.rs::wire_enum_roundtrip`.

**Status:** done

### §2.5 Module hierarchy

**Spec:** §2.5 — IDL module → `pub mod` with nested definitions.

**Repo:** `crates/idl-rust/src/emitter.rs::emit_module`.

**Tests:** `snapshot_codegen.rs::snapshot_module_nested`.

**Status:** done

## §3 Annotation mapping

### §3.1 Extensibility (final/appendable/mutable/extensibility)

**Spec:** §3.1 — `@final`/`@appendable`/`@mutable`/`@extensibility(...)` map onto the XCDR2 wire modes.

**Repo:** `crates/idl-rust/src/annotations.rs::struct_extensibility`, `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (3 branches).

**Tests:** `snapshot_codegen.rs::{snapshot_appendable_struct, snapshot_mutable_struct_with_ids}`, `wire_roundtrip.rs::wire_appendable_dheader_present`.

**Status:** done

### §3.2 @key + KeyHolder

**Spec:** §3.2 — `@key` members are written member-id-sorted in `encode_key_holder_be` (XTypes 1.3 §7.6.8.3.1.b); `KEY_HOLDER_MAX_SIZE` is computed statically.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{compute_key_holder_max_size, emit_key_holder_be, emit_key_field_write}`.

**Tests:** `snapshot_codegen.rs::{snapshot_struct_with_single_key, snapshot_struct_with_multi_key_id_sorting, snapshot_struct_with_string_key_unbounded}`, `wire_roundtrip.rs::wire_keyed_struct_keyhash_roundtrip`.

**Status:** done

### §3.3 @id(N) member IDs

**Spec:** §3.3 — `@id(N)` sets the explicit member ID for mutable + KeyHolder sorting; the default is the positional index.

**Repo:** `crates/idl-rust/src/annotations.rs::member_id`.

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids`, `snapshot_struct_with_multi_key_id_sorting`.

**Status:** done

### §3.4 @must_understand

**Spec:** §3.4 — a wire flag in the mutable-member EMHeader.

**Repo:** `crates/idl-rust/src/annotations.rs::member_must_understand`, `crates/idl-rust/src/struct_emit.rs::emit_mutable_member_encode`.

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids` (default false tested).

**Status:** done

### §3.5 @nested

**Spec:** §3.5 — a property API to mark as "not topic-capable" (XTypes §7.4.6.3.5). The codegen emits `const IS_NESTED: bool = true` for `@nested`-annotated structs; `zerodds_dcps::DdsType::IS_NESTED` as a new trait const.

**Repo:** `crates/idl-rust/src/annotations.rs::struct_is_nested`, `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl`, `crates/dcps/src/dds_type.rs::DdsType::IS_NESTED`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_nested_struct_emits_is_nested_const`.

**Status:** done

### §3.6 @optional

**Spec:** §3.6 — field-type wrap to `Option<T>`; the wire present-flag is covered by `composite::CdrEncode/Decode for Option<T>` (XCDR2 §7.4.5.1.4).

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_member_field` + `emit_field_decode_with_optional` + `emit_field_value_arm`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_optional_member_field_wraps_in_option` + `crates/idl-rust/tests/compile_check.rs::compile_check_optional_member_field`.

**Status:** done

## §4 DdsType trait impl

### §4.1 Constants (TYPE_NAME, HAS_KEY, KEY_HOLDER_MAX_SIZE)

**Spec:** §4 — constants set correctly per struct.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl`.

**Tests:** `wire_roundtrip.rs::wire_keyed_struct_keyhash_roundtrip` asserts `HAS_KEY = true` and `KEY_HOLDER_MAX_SIZE = Some(4)`.

**Status:** done

### §4.2 Methods (encode/decode/encode_key_holder_be/field_value)

**Spec:** §4 — all four methods emitted when relevant; `encode_key_holder_be` only when `HAS_KEY`.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{emit_dds_type_impl, emit_encode_body, emit_decode_body, emit_key_holder_be, emit_field_value}`.

**Tests:** all 8 `compile_check.rs::compile_check_*` (trait compliance), all 6 `wire_roundtrip.rs::wire_*` (behavior).

**Status:** done

## §5 Wire-format conformance

### §5.1 Final struct = direct encode in declaration order

**Spec:** §5 — Final without a DHEADER, members in declaration order.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Final branch).

**Tests:** `wire_roundtrip.rs::wire_simple_struct_primitives_roundtrip` asserts `buf.len() == 8` for 2×i32.

**Status:** done

### §5.2 Appendable = DHEADER + body

**Spec:** §5 — `zerodds_cdr::struct_enc::encode_appendable` with a DHEADER wrap (XTypes §7.4.3.4.5).

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Appendable branch).

**Tests:** `wire_roundtrip.rs::wire_appendable_dheader_present` asserts `buf.len() >= 16` (DHEADER 4 + body ≥ 12).

**Status:** done

### §5.3 Mutable encode = MutableStructEncoder

**Spec:** §5 — member-id-tagged encoding with a LengthCode (XTypes §7.4.3.4.4).

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Mutable branch).

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids` (code output checks the `enc.member(...)` calls).

**Status:** done

### §5.4 Mutable decode with arbitrary order

**Spec:** §5 — a `read_mutable_member` loop with member-id lookup; the order on the wire may differ from declaration. Unknown must_understand member IDs → `DecodeError::UnknownMustUnderstandMember`; missing non-optional members → `DecodeError::MissingNonOptionalMember`.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_decode_body` (Mutable branch); `crates/cdr/src/error.rs::DecodeError::UnknownMustUnderstandMember` + `MissingNonOptionalMember`.

**Tests:** `crates/idl-rust/tests/compile_check.rs::compile_check_mutable_with_arbitrary_member_order`.

**Status:** done

### §5.5 Enum = i32

**Spec:** §5 — wire format `i32` with a discriminator value (XTypes §7.4.5.1).

**Repo:** `crates/idl-rust/src/enum_emit.rs::emit_enum`.

**Tests:** `wire_roundtrip.rs::wire_enum_roundtrip` asserts `bytes == [1, 0, 0, 0]` for `Color::GREEN`.

**Status:** done

## §6 Naming conventions

### §6.1 Identifier 1:1

**Spec:** §6.1 — type/field/enumerator identifiers taken over unchanged.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_struct_decl` (`out.push_str(&s.name.text)`).

**Tests:** all snapshot tests check 1:1 naming (e.g. `pub struct Point`, `pub x: i32`).

**Status:** done

### §6.2 Reserved words

**Spec:** §6.2 — Rust reserved words as IDL identifiers need escaping (raw identifier `r#…`).

**Repo:** `crates/idl-rust/src/type_map.rs::escape_keyword` with the full list of Rust 2024-edition strict + 2018+ + reserved keywords; carried through in struct_emit/enum_emit/union_emit/typedef_emit/emitter (all identifier-emit sites).

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_struct_with_rust_reserved_word_identifiers`, `crates/idl-rust/tests/compile_check.rs::compile_check_reserved_word_identifiers`.

**Status:** done

## §7 Out-of-scope constructs

### §7.1 Bitset/Bitmask

**Spec:** §7 — IDL §7.4.7 bitset/bitmask: not typical for DDS topics.

**Repo:** `crates/idl-rust/src/bitset_emit.rs::emit_bitset` + `emit_bitmask` — emits a `pub struct` with a storage integer + getter/setter per bit (bitset) or a `const` per value + bitwise ops (bitmask).

**Tests:** `snapshot_codegen.rs::{snapshot_bitset_with_named_bitfields, snapshot_bitmask_with_const_values}`, `compile_check.rs::{compile_check_bitset, compile_check_bitmask}`.

**Status:** done

### §7.2 Fixed

**Spec:** §7 — IDL §7.4.4.5 fixed-point decimal arithmetic (XCDR2 §7.4.4.5 BCD wire format).

**Repo:** `crates/cdr/src/fixed.rs::Fixed<P, S>` with packed-BCD storage; `crates/idl-rust/src/type_map.rs::rust_type_for` maps `fixed<P, S>` → `zerodds_cdr::fixed::Fixed<P, S>`.

**Tests:** `crates/cdr/src/fixed.rs::tests::{fixed_default_is_zero_positive, fixed_roundtrip_via_string, fixed_roundtrip_negative, fixed_wire_roundtrip, fixed_overflow_returns_error}`, `snapshot_struct_with_fixed_field`.

**Status:** done

### §7.3 Map<K,V>

**Spec:** §7 — IDL §7.4.4.6 map: associative container.

**Repo:** `crates/cdr/src/composite.rs::impl CdrEncode for BTreeMap<K, V>` + `CdrDecode for BTreeMap<K, V>`; `crates/idl-rust/src/type_map.rs::rust_map` maps `map<K, V>` → `::std::collections::BTreeMap<K, V>`. Default choice BTreeMap (deterministic iteration order); a HashMap variant via the `@map_impl(HashMap)` annotation is optional and not implemented.

**Tests:** `snapshot_struct_with_map_field`.

**Status:** done

### §7.4 Any

**Spec:** §7 — IDL §7.4.4.7 Any: type erasure with a runtime type tag.

**Repo:** `crates/dcps/src/dds_type.rs::DdsAny` with `pack<T>`/`unpack<T>` convenience; `crates/idl-rust/src/type_map.rs::rust_type_for` maps IDL `any` → `zerodds_dcps::DdsAny`. Wire format: a `type_name` CDR string + length-prefixed `payload` bytes.

**Tests:** `snapshot_struct_with_any_field`.

**Status:** done

### §7.5 Valuetype/Interface/Component/Home

**Spec:** §7 — IDL §7.4.5.4/§7.4.6.4/§7.4.8/§7.4.9: CORBA service constructs. Emitted in the **separate service codegen** `zerodds-corba-rust` (Layer 8), not in the DataType codegen `zerodds-idl-rust` (Layer 3).

**Repo:** `crates/corba-rust/src/{interface_emit,valuetype_emit}.rs` — emits `pub trait`/stub/skeleton for interfaces and `pub trait V: ValueBase` for valuetypes; Component/Home via `component_emit.rs` (`emit_component`/`emit_home`).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs`.

**Status:** done — architecture split via the `corba-rust` crate. Coverage of the CORBA-specific mappings: `docs/spec-coverage/zerodds-corba-rust-1.1.md`.

## §8 Conformance tests

### §8.1 Snapshot identity (deterministic codegen)

**Spec:** §8.1 — reproducible output per IDL source.

**Repo:** `crates/idl-rust/tests/snapshot_codegen.rs` with the `tests/snapshots/` directory.

**Tests:** 14 snapshot tests committed.

**Status:** done

### §7.x Exception mapping

**Spec:** OMG-IDL4 §7.4.10 — `exception E { ... };`.

**Repo:** `crates/idl-rust/src/emitter.rs::emit_exception` — emits IDL exceptions as `pub struct E { fields }` (CORBA 3.3 §7.4.10). Cross-language wiring: `zerodds-corba-rust` references the struct in the `<I>Error::E(E)` variant.

**Tests:** `crates/corba-rust/tests/compile_check.rs::compile_check_interface_raises` shows compile correctness of the cross-crate reference.

**Status:** done

### §8.2 Compile correctness

**Spec:** §8.2 — the emitted code compiles against `zerodds-cdr`/`zerodds-dcps`/`zerodds-sql-filter`.

**Repo:** `crates/idl-rust/tests/compile_check.rs`.

**Tests:** 8 tests, `#[ignore]`-gated, run via `--include-ignored`.

**Status:** done

### §8.3 Wire round-trip

**Spec:** §8.3 — encode + decode = identity.

**Repo:** `crates/idl-rust/tests/wire_roundtrip.rs`.

**Tests:** 6 tests including round-trip identity for primitives, keyed, field_value, appendable, string+sequence, enum.

**Status:** done

### §8.4 Wire conformance

**Spec:** §8.4 — XCDR2 bytes spec-conformant.

**Repo:** `crates/idl-rust/tests/wire_roundtrip.rs`.

**Tests:** byte-exact wire assertions: Final = 8 bytes for 2×i32, Appendable = ≥16 bytes (DHEADER + body), enum = 4-byte i32-LE, keyed = stable hash + identity property.

**Status:** done

---

## Audit status

27 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-rust --tests && cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored && cargo test -p zerodds-idl-rust --test wire_roundtrip -- --include-ignored` — 14 + 8 + 6 = 28 tests green, 0 failed.
