# `zerodds-xcdr2-python` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-python-1.0.md` — ZeroDDS Python XCDR2 TypeSupport-Codegen-Spec.

Implementation (drei Schichten):

- `crates/idl-python/` — IDL → Python Codegen (`@idl_struct`/`@dataclass`, Enum/Union/Typedef/Map/Array/Brands).
- `crates/py/python/zerodds/` — Runtime (`cdr.py` encode/decode, `idl.py` `@idl_struct`).
- `endpoints/python/` — pure-Python XCDR2 Wire-Core (`zerodds_wire.py`) + sync/async SDK.

Offene Items: siehe `zerodds-xcdr2-python-1.0.open.md` (nur §7.a keyHash-Codegen).

## §1 Motivation

### §1 Kein OMG-IDL-to-Python-Wire-Mapping

**Spec:** §1 — ZeroDDS definiert das Python-XCDR2-Wire-Mapping (Codegen + Runtime + Endpoint).

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal-Pattern

### §2 `@idl_struct`/`@dataclass` + `cdr.encode`

**Spec:** §2 — pro IDL-`@final struct` eine annotierte Dataclass; Runtime marshalt reflektiv.

**Repo:** `crates/idl-python/src/emitter.rs::emit_struct`; `crates/py/python/zerodds/cdr.py`.

**Tests:** `crates/idl-python/tests/smoke.rs::struct_with_primitives_emits_idl_struct_and_dataclass`; pytest via `gen_for_pytest.rs`.

**Status:** done

## §3 Required API-Surface

### §3 Codegen + Runtime-encode/decode + Endpoint-Writer/Reader

**Spec:** §3 — `@idl_struct`-Dataclass; Runtime `encode`+`decode`; Endpoint Writer/Reader (put_*/get_* inkl. DHEADER/EMHEADER).

**Repo:** `crates/idl-python/src/emitter.rs`; `crates/py/python/zerodds/cdr.py` (encode+decode); `endpoints/python/zerodds_wire.py` (Writer+Reader vollständig, get_u8/16/32/64/f32/f64/string/seq_u8 + dheader/emheader).

**Tests:** `endpoints/python/test_byte_identity.py` (Roundtrip), `endpoints/python/example_sync|async.py` (voller Feld-Decode), pytest.

**Status:** done

### §3.a Generiertes Decode/`unmarshal`

**Spec:** §3 — Decode.

**Repo:** `crates/py/python/zerodds/cdr.py::decode` (reflektiv aus den `@idl_struct`-Brands).

**Tests:** pytest-Roundtrip (`gen_for_pytest.rs` → encode+decode).

**Status:** done — die Runtime dekodiert reflektiv (kein „open" wie bei den dünnen Backends).

## §4 Codegen-Pflicht (`idl-python`)

### §4 Dataclass + Enum/Union/Typedef/Bitmask + Brands

**Spec:** §4 — pro Konstrukt: Dataclass/IntEnum/`@idl_union`/Alias/IntFlag + Feld-Brands.

**Repo:** `crates/idl-python/src/emitter.rs`; `tools/idlc` `Backend::Python`, `--python`.

**Tests:** `crates/idl-python/tests/smoke.rs` (54 Tests).

**Status:** done

## §5 Wire-Type-Mapping

### §5 Primitive + string/wstring + float/double

**Spec:** §5 — bool/octet/char/wchar/short..long long/float/double/string/wstring.

**Repo:** `crates/idl-python/src/emitter.rs` (map_primitive/brands); `endpoints/python/zerodds_wire.py`.

**Tests:** `smoke.rs::boolean_maps_to_python_bool`, `floating_types_map_to_brands`, `char_and_wstring_brands_are_emitted`, `struct_with_wstring_uses_wstring_brand`; `endpoints/python/test_byte_identity.py` (@final Golden LE+BE).

**Status:** done

### §5.a enum / union / typedef / map / array / nested / bounded / bitmask / bitset / inheritance

**Spec:** §5 — die vollen IDL4-Konstrukte.

**Repo:** `crates/idl-python/src/emitter.rs`.

**Tests:** `smoke.rs`: `enum_emits_intenum_subclass`, `union_with_integer_switch_emits_idl_union_factory` (+default/negative/hex/scoped/enum-discriminator), `typedef_emits_type_alias`, `map_emits_dict_annotation` (+bounded), `fixed_array_member_uses_array_brand_not_list` (+multidim), `nested_sequence_emits_nested_list`, `bounded_string_emits_bounded_string_brand` (+wstring/sequence/map), `bitmask_emits_intflag_with_shifted_bits`, `bitset_emits_alias_plus_bits_helper`, `struct_inheritance_emits_subclass`, `optional_member_wrapped_in_optional_brand`.

**Status:** done

### §5.b long double

**Spec:** §5 — `long double`.

**Repo:** `crates/idl-python/src/emitter.rs` → `LongDouble`-Brand (`crates/py/python/zerodds`).

**Tests:** `smoke.rs::floating_types_map_to_brands`.

**Status:** done — Python-Runtime hängt NICHT an Rust `f128`; hier kein Blocker (anders als die dünnen Backends).

## §6 Extensibility

### §6 @final / @appendable / @mutable (mit Member-IDs)

**Spec:** §6 — @final compact, @appendable DHEADER, @mutable EMHEADER + `@id(N)`.

**Repo:** `crates/idl-python/src/emitter.rs::mutable_member_ids` + Extensibility-Kwarg; `endpoints/python/zerodds_wire.py` (dheader/emheader).

**Tests:** `smoke.rs::{explicit_final_struct_omits_extensibility_kwarg, appendable_struct_emits_extensibility_kwarg, mutable_struct_emits_extensibility_kwarg, extensibility_annotation_form_emits_kwarg}`; `endpoints/python/test_mutable.py`.

**Status:** done

## §7 Key-Extraction

### §7 Non-keyed 16-Zero + keyed Runtime

**Spec:** §7 — non-keyed → 16 Nullbytes; keyed → MD5 (XCDR2-BE), Runtime.

**Repo:** `endpoints/python/zerodds_wire.py` (BE-Writer liefert BE-Serialisierung); `crates/py/python/zerodds` (Key-Hash-Runtime).

**Tests:** —

**Status:** done (non-keyed + Runtime-Keyhash)

### §7.a Per-struct generierte `keyHash`

**Spec:** §7 — Codegen einer `keyHash`-Methode aus `@key`.

**Repo:** —

**Tests:** —

**Status:** open — siehe `.open.md` (Roadmap; Runtime rechnet Key-Hashes heute).

## §8 Wire-Core

### §8 `endpoints/python/zerodds_wire.py` + Runtime `cdr.py`

**Spec:** §8 — Referenz-Wire-Core, byte-identisch zu `zerodds-cdr`.

**Repo:** `endpoints/python/zerodds_wire.py`, `crates/py/python/zerodds/cdr.py`.

**Tests:** `test_byte_identity.py`; CI `python-tests` + endpoints-native (falls Python-Endpoint dort).

**Status:** done

## §9 Conformance

### §9 Golden-Byte-Identität @final LE+BE

**Spec:** §9 — Encoding == golden_le.bin / golden_be.bin byte-für-byte.

**Repo:** `crates/idl-python` + `crates/py`, `endpoints/python`.

**Tests:** `endpoints/python/test_byte_identity.py`; pytest-Roundtrip (`gen_for_pytest.rs`); CI `python-tests`.

**Status:** done

## §10 Examples

### §10 sync + async Deep-Examples + Quickstart

**Spec:** §10 — lauffähige sync/async-Beispiele.

**Repo:** `endpoints/python/example_sync.py`, `endpoints/python/example_async.py`, `endpoints/python/QUICKSTART.md`.

**Tests:** beide lokal grün (5/5 Feld-Decode); CI-Ergänzung im Python-Endpoint-Job.

**Status:** done

## §11 Errata + Open-Questions

### §11 Nicht-Ziele + offene Punkte

**Spec:** §11 — interface/valuetype/any (non-goal); keyHash-Codegen (open).

**Repo:** `crates/idl-python/src/emitter.rs` (Unsupported für interface/valuetype/any); §7.a.

**Tests:** `smoke.rs::{interface_and_valuetype_still_unsupported, any_type_still_unsupported}`.

**Status:** n/a (informative) — interface/valuetype/any sind echte Nicht-Ziele (RPC/OO/dynamic, keine DDS-DataTypes); keyHash-Codegen ist `open`.
