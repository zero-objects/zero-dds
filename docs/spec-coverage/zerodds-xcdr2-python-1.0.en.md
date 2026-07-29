# `zerodds-xcdr2-python` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-python-1.0.md` — ZeroDDS Python XCDR2 TypeSupport codegen spec.

Implementation (three layers):

- `crates/idl-python/` — IDL → Python codegen (`@idl_struct`/`@dataclass`, enum/union/typedef/map/array/brands).
- `crates/py/python/zerodds/` — runtime (`cdr.py` encode/decode, `idl.py` `@idl_struct`).
- `endpoints/python/` — pure-Python XCDR2 wire core (`zerodds_wire.py`) + sync/async SDK.

Open items: see `zerodds-xcdr2-python-1.0.open.md` (only §7.a keyHash codegen).

## §1 Motivation

### §1 No OMG-IDL-to-Python wire mapping

**Spec:** §1 — ZeroDDS defines the Python XCDR2 wire mapping (codegen + runtime + endpoint).

**Repo:** motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 `@idl_struct`/`@dataclass` + `cdr.encode`

**Spec:** §2 — one annotated dataclass per IDL `@final struct`; the runtime marshals reflectively.

**Repo:** `crates/idl-python/src/emitter.rs::emit_struct`; `crates/py/python/zerodds/cdr.py`.

**Tests:** `crates/idl-python/tests/smoke.rs::struct_with_primitives_emits_idl_struct_and_dataclass`; pytest via `gen_for_pytest.rs`.

**Status:** done

## §3 Required API surface

### §3 Codegen + runtime encode/decode + endpoint writer/reader

**Spec:** §3 — `@idl_struct` dataclass; runtime `encode`+`decode`; endpoint Writer/Reader (put_*/get_* incl. DHEADER/EMHEADER).

**Repo:** `crates/idl-python/src/emitter.rs`; `crates/py/python/zerodds/cdr.py` (encode+decode); `endpoints/python/zerodds_wire.py` (Writer+Reader complete, get_u8/16/32/64/f32/f64/string/seq_u8 + dheader/emheader).

**Tests:** `endpoints/python/test_byte_identity.py` (roundtrip), `endpoints/python/example_sync|async.py` (full field decode), pytest.

**Status:** done

### §3.a Generated decode/`unmarshal`

**Spec:** §3 — decode.

**Repo:** `crates/py/python/zerodds/cdr.py::decode` (reflective from the `@idl_struct` brands).

**Tests:** pytest roundtrip (`gen_for_pytest.rs` → encode+decode).

**Status:** done — the runtime decodes reflectively (not an "open" item, unlike the thin backends).

## §4 Codegen requirement (`idl-python`)

### §4 Dataclass + enum/union/typedef/bitmask + brands

**Spec:** §4 — per construct: dataclass/IntEnum/`@idl_union`/alias/IntFlag + field brands.

**Repo:** `crates/idl-python/src/emitter.rs`; `tools/idlc` `Backend::Python`, `--python`.

**Tests:** `crates/idl-python/tests/smoke.rs` (54 tests).

**Status:** done

## §5 Wire type mapping

### §5 Primitives + string/wstring + float/double

**Spec:** §5 — bool/octet/char/wchar/short..long long/float/double/string/wstring.

**Repo:** `crates/idl-python/src/emitter.rs` (map_primitive/brands); `endpoints/python/zerodds_wire.py`.

**Tests:** `smoke.rs::boolean_maps_to_python_bool`, `floating_types_map_to_brands`, `char_and_wstring_brands_are_emitted`, `struct_with_wstring_uses_wstring_brand`; `endpoints/python/test_byte_identity.py` (@final golden LE+BE).

**Status:** done

### §5.a enum / union / typedef / map / array / nested / bounded / bitmask / bitset / inheritance

**Spec:** §5 — the full IDL4 constructs.

**Repo:** `crates/idl-python/src/emitter.rs`.

**Tests:** `smoke.rs`: `enum_emits_intenum_subclass`, `union_with_integer_switch_emits_idl_union_factory` (+default/negative/hex/scoped/enum discriminator), `typedef_emits_type_alias`, `map_emits_dict_annotation` (+bounded), `fixed_array_member_uses_array_brand_not_list` (+multidim), `nested_sequence_emits_nested_list`, `bounded_string_emits_bounded_string_brand` (+wstring/sequence/map), `bitmask_emits_intflag_with_shifted_bits`, `bitset_emits_alias_plus_bits_helper`, `struct_inheritance_emits_subclass`, `optional_member_wrapped_in_optional_brand`.

**Status:** done

### §5.b long double

**Spec:** §5 — `long double`.

**Repo:** `crates/idl-python/src/emitter.rs` → `LongDouble` brand (`crates/py/python/zerodds`).

**Tests:** `smoke.rs::floating_types_map_to_brands`.

**Status:** done — the Python runtime does NOT depend on Rust `f128`; no blocker here (unlike the thin backends).

## §6 Extensibility

### §6 @final / @appendable / @mutable (with member IDs)

**Spec:** §6 — @final compact, @appendable DHEADER, @mutable EMHEADER + `@id(N)`.

**Repo:** `crates/idl-python/src/emitter.rs::mutable_member_ids` + extensibility kwarg; `endpoints/python/zerodds_wire.py` (dheader/emheader).

**Tests:** `smoke.rs::{explicit_final_struct_omits_extensibility_kwarg, appendable_struct_emits_extensibility_kwarg, mutable_struct_emits_extensibility_kwarg, extensibility_annotation_form_emits_kwarg}`; `endpoints/python/test_mutable.py`.

**Status:** done

## §7 Key extraction

### §7 Non-keyed 16-zero + keyed runtime

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), runtime.

**Repo:** `endpoints/python/zerodds_wire.py` (BE writer produces the BE serialization); `crates/py/python/zerodds` (key-hash runtime).

**Tests:** —

**Status:** done (non-keyed + runtime keyhash)

### §7.a Per-struct generated `keyHash`

**Spec:** §7 — codegen of a `keyHash` method from `@key`.

**Repo:** —

**Tests:** —

**Status:** open — see `.open.md` (roadmap; the runtime computes key hashes today).

## §8 Wire core

### §8 `endpoints/python/zerodds_wire.py` + runtime `cdr.py`

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/python/zerodds_wire.py`, `crates/py/python/zerodds/cdr.py`.

**Tests:** `test_byte_identity.py`; CI `python-tests` + endpoints-native (if the Python endpoint is there).

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte-for-byte.

**Repo:** `crates/idl-python` + `crates/py`, `endpoints/python`.

**Tests:** `endpoints/python/test_byte_identity.py`; pytest roundtrip (`gen_for_pytest.rs`); CI `python-tests`.

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — working sync/async examples.

**Repo:** `endpoints/python/example_sync.py`, `endpoints/python/example_async.py`, `endpoints/python/QUICKSTART.md`.

**Tests:** both green locally (5/5 field decode); CI addition in the Python endpoint job.

**Status:** done

## §11 Errata + open questions

### §11 Non-goals + open items

**Spec:** §11 — interface/valuetype/any (non-goal); keyHash codegen (open).

**Repo:** `crates/idl-python/src/emitter.rs` (Unsupported for interface/valuetype/any); §7.a.

**Tests:** `smoke.rs::{interface_and_valuetype_still_unsupported, any_type_still_unsupported}`.

**Status:** n/a (informative) — interface/valuetype/any are genuine non-goals (RPC/OO/dynamic, not DDS data types); keyHash codegen is `open`.
