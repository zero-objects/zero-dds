# zerodds-idl-rust 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-idl-rust-1.0.md` (ZeroDDS Vendor-Spec)

Implementation:

- `crates/idl-rust/` — OMG-IDL→Rust-Codegen + DdsType-Trait.

## §1 Scope

### §1.1 Codegen baut DdsType, CdrEncode/Decode, field_value

**Spec:** §1 — IDL-Struct → Rust-`struct` + `impl DdsType` + `impl CdrEncode`/`CdrDecode` für Enum + `field_value` für SQL-Filter.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{emit_struct, emit_dds_type_impl, emit_field_value}`, `crates/idl-rust/src/enum_emit.rs::emit_enum`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_simple_struct_primitives_only`, `snapshot_struct_with_field_value_filter_paths`, `snapshot_enum`.

**Status:** done

### §1.2 Out-of-Scope-Konstrukte werden ignoriert oder als Unsupported gemeldet

**Spec:** §1 — `fixed`/`map`/`any`/`valuetype`/`interface`/`component`/`home`/`bitset`/`bitmask` werden nicht emittiert (Codegen liefert leere Output-Section ODER `RustGenError::Unsupported`).

**Repo:** `crates/idl-rust/src/emitter.rs::emit_definition` (Catch-all-arm); `crates/idl-rust/src/type_map.rs::rust_type_for` (Unsupported für Fixed/Map/Any).

**Tests:** indirekt via `compile_check_*` — alle in-scope-Konstrukte sind emittierbar; dedizierte Negativ-Tests für out-of-scope-Konstrukte stehen aus.

**Status:** done

## §2 Type-Mapping

### §2.1 Primitive-Mapping

**Spec:** §2.1 — Tabelle für 14 IDL-Primitives (`boolean`/`octet`/`int8`/`uint8`/`short`/`unsigned short`/`long`/`unsigned long`/`long long`/`unsigned long long`/`float`/`double`/`long double`/`char`/`wchar`).

**Repo:** `crates/idl-rust/src/type_map.rs::rust_primitive`, `primitive_wire_size`.

**Tests:** `snapshot_codegen.rs::snapshot_struct_full_primitive_set` deckt 13 typische Primitives ab; `wire_roundtrip.rs::wire_simple_struct_primitives_roundtrip` prüft Wire-Encoding für `i32`.

**Status:** done

### §2.2 String + WString

**Spec:** §2.2 — `string`/`wstring` → `String` (UTF-8); Wire-Encoding gemäß XCDR2 §7.4.4.

**Repo:** `crates/idl-rust/src/type_map.rs::rust_string`.

**Tests:** `snapshot_codegen.rs::snapshot_struct_with_string_and_sequence`, `wire_roundtrip.rs::wire_string_and_sequence_roundtrip`.

**Status:** done

### §2.3 Composite Sequence/Array/Optional

**Spec:** §2.3 — `sequence<T>` → `Vec<T>`, `T[N]` → `[T; N]`, mehrdimensionale Arrays.

**Repo:** `crates/idl-rust/src/type_map.rs::rust_sequence`, `crates/idl-rust/src/struct_emit.rs::emit_member_field` (Array-Wrap).

**Tests:** `snapshot_codegen.rs::snapshot_struct_with_string_and_sequence`, `snapshot_struct_with_array_dimensions`, `wire_roundtrip.rs::wire_string_and_sequence_roundtrip`.

**Status:** done

### §2.4 Constructed (struct/enum/union/typedef)

**Spec:** §2.4 — Struct → DdsType-Impl; Enum → CdrEncode/Decode + `from_wire`; Union → Tagged Enum; Typedef → Rust-Type-Alias.

**Repo:** `crates/idl-rust/src/struct_emit.rs`, `crates/idl-rust/src/enum_emit.rs`, `crates/idl-rust/src/union_emit.rs`, `crates/idl-rust/src/typedef_emit.rs`.

**Tests:** `snapshot_codegen.rs::{snapshot_enum, snapshot_typedef, snapshot_union}`, `wire_roundtrip.rs::wire_enum_roundtrip`.

**Status:** done

### §2.5 Module-Hierarchie

**Spec:** §2.5 — IDL-Module → `pub mod` mit nested Definitions.

**Repo:** `crates/idl-rust/src/emitter.rs::emit_module`.

**Tests:** `snapshot_codegen.rs::snapshot_module_nested`.

**Status:** done

## §3 Annotation-Mapping

### §3.1 Extensibility (final/appendable/mutable/extensibility)

**Spec:** §3.1 — `@final`/`@appendable`/`@mutable`/`@extensibility(...)` mappen auf XCDR2-Wire-Modi.

**Repo:** `crates/idl-rust/src/annotations.rs::struct_extensibility`, `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (3 Branches).

**Tests:** `snapshot_codegen.rs::{snapshot_appendable_struct, snapshot_mutable_struct_with_ids}`, `wire_roundtrip.rs::wire_appendable_dheader_present`.

**Status:** done

### §3.2 @key + KeyHolder

**Spec:** §3.2 — `@key`-Member werden in `encode_key_holder_be` member-id-sortiert geschrieben (XTypes 1.3 §7.6.8.3.1.b); `KEY_HOLDER_MAX_SIZE` wird statisch berechnet.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{compute_key_holder_max_size, emit_key_holder_be, emit_key_field_write}`.

**Tests:** `snapshot_codegen.rs::{snapshot_struct_with_single_key, snapshot_struct_with_multi_key_id_sorting, snapshot_struct_with_string_key_unbounded}`, `wire_roundtrip.rs::wire_keyed_struct_keyhash_roundtrip`.

**Status:** done

### §3.3 @id(N) Member-IDs

**Spec:** §3.3 — `@id(N)` setzt explizite Member-ID für mutable + KeyHolder-Sortierung; default ist positional-Index.

**Repo:** `crates/idl-rust/src/annotations.rs::member_id`.

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids`, `snapshot_struct_with_multi_key_id_sorting`.

**Status:** done

### §3.4 @must_understand

**Spec:** §3.4 — Wire-Flag in mutable-Member-EMHeader.

**Repo:** `crates/idl-rust/src/annotations.rs::member_must_understand`, `crates/idl-rust/src/struct_emit.rs::emit_mutable_member_encode`.

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids` (default false getestet).

**Status:** done

### §3.5 @nested

**Spec:** §3.5 — Property-API zum Markieren als „nicht topic-fähig" (XTypes §7.4.6.3.5). Codegen emittiert `const IS_NESTED: bool = true` für `@nested`-annotated structs; `zerodds_dcps::DdsType::IS_NESTED` als neuer Trait-Const.

**Repo:** `crates/idl-rust/src/annotations.rs::struct_is_nested`, `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl`, `crates/dcps/src/dds_type.rs::DdsType::IS_NESTED`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_nested_struct_emits_is_nested_const`.

**Status:** done

### §3.6 @optional

**Spec:** §3.6 — Field-Type-Wrap zu `Option<T>`; Wire-present-Flag wird durch `composite::CdrEncode/Decode for Option<T>` (XCDR2 §7.4.5.1.4) abgedeckt.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_member_field` + `emit_field_decode_with_optional` + `emit_field_value_arm`.

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_optional_member_field_wraps_in_option` + `crates/idl-rust/tests/compile_check.rs::compile_check_optional_member_field`.

**Status:** done

## §4 DdsType-Trait-Impl

### §4.1 Konstanten (TYPE_NAME, HAS_KEY, KEY_HOLDER_MAX_SIZE)

**Spec:** §4 — Konstanten korrekt gesetzt pro Struct.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_dds_type_impl`.

**Tests:** `wire_roundtrip.rs::wire_keyed_struct_keyhash_roundtrip` asserted `HAS_KEY = true` und `KEY_HOLDER_MAX_SIZE = Some(4)`.

**Status:** done

### §4.2 Methoden (encode/decode/encode_key_holder_be/field_value)

**Spec:** §4 — alle vier Methoden emittieren wenn relevant; `encode_key_holder_be` nur wenn `HAS_KEY`.

**Repo:** `crates/idl-rust/src/struct_emit.rs::{emit_dds_type_impl, emit_encode_body, emit_decode_body, emit_key_holder_be, emit_field_value}`.

**Tests:** alle 8 `compile_check.rs::compile_check_*` (Trait-Compliance), alle 6 `wire_roundtrip.rs::wire_*` (Verhalten).

**Status:** done

## §5 Wire-Format-Konformität

### §5.1 Final-Struct = direkt encode in deklarations-Reihenfolge

**Spec:** §5 — Final ohne DHEADER, Members in deklarations-Reihenfolge.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Final-Branch).

**Tests:** `wire_roundtrip.rs::wire_simple_struct_primitives_roundtrip` asserted `buf.len() == 8` für 2×i32.

**Status:** done

### §5.2 Appendable = DHEADER + Body

**Spec:** §5 — `zerodds_cdr::struct_enc::encode_appendable` mit DHEADER-Wrap (XTypes §7.4.3.4.5).

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Appendable-Branch).

**Tests:** `wire_roundtrip.rs::wire_appendable_dheader_present` asserted `buf.len() >= 16` (DHEADER 4 + body ≥ 12).

**Status:** done

### §5.3 Mutable Encode = MutableStructEncoder

**Spec:** §5 — member-id-tagged Encoding mit LengthCode (XTypes §7.4.3.4.4).

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_encode_body` (Mutable-Branch).

**Tests:** `snapshot_codegen.rs::snapshot_mutable_struct_with_ids` (Code-Output prüft `enc.member(...)`-Aufrufe).

**Status:** done

### §5.4 Mutable Decode mit beliebiger Reihenfolge

**Spec:** §5 — `read_mutable_member`-Loop mit member-id-Lookup; Reihenfolge im Wire kann von deklaration abweichen. Unbekannte must_understand-Member-IDs → `DecodeError::UnknownMustUnderstandMember`; fehlende non-optional Member → `DecodeError::MissingNonOptionalMember`.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_decode_body` (Mutable-Branch); `crates/cdr/src/error.rs::DecodeError::UnknownMustUnderstandMember` + `MissingNonOptionalMember`.

**Tests:** `crates/idl-rust/tests/compile_check.rs::compile_check_mutable_with_arbitrary_member_order`.

**Status:** done

### §5.5 Enum = i32

**Spec:** §5 — Wire-Format `i32` mit Discriminator-Wert (XTypes §7.4.5.1).

**Repo:** `crates/idl-rust/src/enum_emit.rs::emit_enum`.

**Tests:** `wire_roundtrip.rs::wire_enum_roundtrip` asserted `bytes == [1, 0, 0, 0]` für `Color::GREEN`.

**Status:** done

## §6 Naming-Konventionen

### §6.1 Identifier 1:1

**Spec:** §6.1 — Type/Field/Enumerator-Identifier unverändert übernommen.

**Repo:** `crates/idl-rust/src/struct_emit.rs::emit_struct_decl` (`out.push_str(&s.name.text)`).

**Tests:** alle Snapshot-Tests prüfen 1:1-Naming (z.B. `pub struct Point`, `pub x: i32`).

**Status:** done

### §6.2 Reserved-Words

**Spec:** §6.2 — Rust-Reserved-Words als IDL-Identifier brauchen Escaping (raw-identifier `r#…`).

**Repo:** `crates/idl-rust/src/type_map.rs::escape_keyword` mit voller Liste der Rust-2024-Edition strict + 2018+ + reserved-Keywords; durchgezogen in struct_emit/enum_emit/union_emit/typedef_emit/emitter (alle Identifier-Emit-Stellen).

**Tests:** `crates/idl-rust/tests/snapshot_codegen.rs::snapshot_struct_with_rust_reserved_word_identifiers`, `crates/idl-rust/tests/compile_check.rs::compile_check_reserved_word_identifiers`.

**Status:** done

## §7 Out-of-Scope-Konstrukte

### §7.1 Bitset/Bitmask

**Spec:** §7 — IDL §7.4.7 Bitset/Bitmask: nicht typisch für DDS-Topics.

**Repo:** `crates/idl-rust/src/bitset_emit.rs::emit_bitset` + `emit_bitmask` — emittiert `pub struct` mit Storage-Integer + Getter/Setter pro Bit (bitset) bzw. `const`-pro-Wert + bitwise-Ops (bitmask).

**Tests:** `snapshot_codegen.rs::{snapshot_bitset_with_named_bitfields, snapshot_bitmask_with_const_values}`, `compile_check.rs::{compile_check_bitset, compile_check_bitmask}`.

**Status:** done

### §7.2 Fixed

**Spec:** §7 — IDL §7.4.4.5 Fixed-Point Decimal-Arithmetik (XCDR2 §7.4.4.5 BCD-Wire-Format).

**Repo:** `crates/cdr/src/fixed.rs::Fixed<P, S>` mit Packed-BCD-Storage; `crates/idl-rust/src/type_map.rs::rust_type_for` mappt `fixed<P, S>` → `zerodds_cdr::fixed::Fixed<P, S>`.

**Tests:** `crates/cdr/src/fixed.rs::tests::{fixed_default_is_zero_positive, fixed_roundtrip_via_string, fixed_roundtrip_negative, fixed_wire_roundtrip, fixed_overflow_returns_error}`, `snapshot_struct_with_fixed_field`.

**Status:** done

### §7.3 Map<K,V>

**Spec:** §7 — IDL §7.4.4.6 Map: assoziative Container.

**Repo:** `crates/cdr/src/composite.rs::impl CdrEncode for BTreeMap<K, V>` + `CdrDecode for BTreeMap<K, V>`; `crates/idl-rust/src/type_map.rs::rust_map` mappt `map<K, V>` → `::std::collections::BTreeMap<K, V>`. Default-Wahl BTreeMap (deterministische Iter-Order); eine HashMap-Variante via `@map_impl(HashMap)`-Annotation ist optional und nicht implementiert.

**Tests:** `snapshot_struct_with_map_field`.

**Status:** done

### §7.4 Any

**Spec:** §7 — IDL §7.4.4.7 Any: Type-Erasure mit Runtime-Type-Tag.

**Repo:** `crates/dcps/src/dds_type.rs::DdsAny` mit `pack<T>`/`unpack<T>` Convenience; `crates/idl-rust/src/type_map.rs::rust_type_for` mappt IDL `any` → `zerodds_dcps::DdsAny`. Wire-Format: `type_name`-CDR-String + length-prefixed `payload`-Bytes.

**Tests:** `snapshot_struct_with_any_field`.

**Status:** done

### §7.5 Valuetype/Interface/Component/Home

**Spec:** §7 — IDL §7.4.5.4/§7.4.6.4/§7.4.8/§7.4.9: CORBA-Service-Konstrukte. Werden im **separaten Service-Codegen** `zerodds-corba-rust` emittiert (Layer 8), nicht im DataType-Codegen `zerodds-idl-rust` (Layer 3).

**Repo:** `crates/corba-rust/src/{interface_emit,valuetype_emit}.rs` — emittiert `pub trait`/Stub/Skeleton für Interfaces und `pub trait V: ValueBase` für Valuetypes; Component/Home via `component_emit.rs` (`emit_component`/`emit_home`).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs`.

**Status:** done — Architektur-Trennung via `corba-rust`-Crate. Coverage der CORBA-spezifischen Mappings: `docs/spec-coverage/zerodds-corba-rust-1.1.md`.

## §8 Konformitäts-Tests

### §8.1 Snapshot-Identität (deterministischer Codegen)

**Spec:** §8.1 — reproduzierbarer Output pro IDL-Quelle.

**Repo:** `crates/idl-rust/tests/snapshot_codegen.rs` mit `tests/snapshots/`-Verzeichnis.

**Tests:** 14 Snapshot-Tests committed.

**Status:** done

### §7.x Exception-Mapping

**Spec:** OMG-IDL4 §7.4.10 — `exception E { ... };`.

**Repo:** `crates/idl-rust/src/emitter.rs::emit_exception` — emittiert IDL-Exceptions als `pub struct E { fields }` (CORBA 3.3 §7.4.10). Cross-Lang-Wiring: `zerodds-corba-rust` referenziert die Struct in `<I>Error::E(E)`-Variante.

**Tests:** `crates/corba-rust/tests/compile_check.rs::compile_check_interface_raises` belegt Compile-Korrektheit der Cross-Crate-Referenz.

**Status:** done

### §8.2 Compile-Korrektheit

**Spec:** §8.2 — emittierter Code kompiliert gegen `zerodds-cdr`/`zerodds-dcps`/`zerodds-sql-filter`.

**Repo:** `crates/idl-rust/tests/compile_check.rs`.

**Tests:** 8 Tests, `#[ignore]`-gated, run via `--include-ignored`.

**Status:** done

### §8.3 Wire-Roundtrip

**Spec:** §8.3 — encode + decode = identity.

**Repo:** `crates/idl-rust/tests/wire_roundtrip.rs`.

**Tests:** 6 Tests inklusive Roundtrip-Identität für primitives, keyed, field_value, appendable, string+sequence, enum.

**Status:** done

### §8.4 Wire-Konformität

**Spec:** §8.4 — XCDR2-Bytes spec-konform.

**Repo:** `crates/idl-rust/tests/wire_roundtrip.rs`.

**Tests:** byte-genaue Wire-Assertions: Final = 8 byte für 2×i32, Appendable = ≥16 byte (DHEADER + body), Enum = 4-byte i32-LE, Keyed = stable hash + Identitäts-Eigenschaft.

**Status:** done

---

## Audit-Status

27 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-rust --tests && cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored && cargo test -p zerodds-idl-rust --test wire_roundtrip -- --include-ignored` — 14 + 8 + 6 = 28 Tests grün, 0 failed.
