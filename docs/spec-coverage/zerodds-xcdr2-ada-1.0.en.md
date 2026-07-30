# `zerodds-xcdr2-ada` 1.0 — Spec-Coverage

**Source:** `docs/specs/zerodds-xcdr2-ada-1.0.md` — ZeroDDS Ada XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-ada/` — IDL → Ada codegen (package `Zdgen`, `Marshal`, self-contained bounded wire).
- `endpoints/ada/` — Ada Stage-1 endpoint: `Interfaces.C` bindings over the C89 wire core + sync/async deep examples.

## §1 Motivation

### §1 No OMG IDL-to-Ada wire mapping

**Spec:** §1 — ZeroDDS defines the Ada XCDR2 wire mapping (codegen + FFI endpoint).

**Repo:** Motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 record + `Marshal (V; Endian) return Byte_Array`

**Spec:** §2 — one Ada `record` + `Marshal` function in package `Zdgen` per IDL `@final struct`.

**Repo:** `crates/idl-ada/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-ada/tests/golden.rs::final_struct_emits_record_and_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/reader primitives + generated Marshal

**Spec:** §3 — writer (Put_U8..Put_Seq_U8), reader (Get_U8/Get_U16/Get_U32/Get_U64/Get_F32/Get_String/Get_Seq_U8), generated `Marshal`.

**Repo:** `endpoints/ada/src/zdw.ads` (writer + reader incl. Get_U8/Get_U16/Get_U64/Get_F32/Get_String/Get_Seq_U8 — byte-exact inverse via the C core, f32 via `zdw_get_f32`, u64 via `Zdw_U64`); `crates/idl-ada/src/emitter.rs` (Marshal).

**Tests:** `endpoints/ada/test/test_byte_identity.adb` (byte identity + decode roundtrip), `endpoints/ada/test/example_sync|async.adb` (full field decode), `crates/idl-ada/tests/golden.rs`.

**Status:** done

### §3.a Generated `Unmarshal` (decode codegen)

**Spec:** §3/§11 — bidirectional binding: generated `Unmarshal` as the exact inverse of `Marshal`.

**Repo:** `map_get` (inverse of `map_type`) + `Reader` (Get_* functions with `in out Pos`, Ada 2012) in the package body + `Read_<T>`/`Unmarshal` per struct+union + enum `_Of_U32` (`crates/idl-ada/src/emitter.rs`).

**Tests:** `crates/idl-ada/tests/golden.rs` — 8 `decode_roundtrip_*` (final/nested/array/union/map/mutable/wide/longdouble), `Marshal (Unmarshal (golden)) = golden` LE+BE (codepit, gnatmake).

**Status:** done

**Decision record:** Decode codegen covers every field type (prim/string/seq/enum/typedef/nested/array/union/map/@mutable/wchar/wstring/long double). Mutable records are filled member-wise; Get_* over `Data`/`Pos`/`Endian`, float via inverse Unchecked_Conversion; seq→Vectors.Append, map→Ordered_Maps.Insert. @final=inline, @appendable=DHEADER skip, @mutable=DHEADER + per-member EMHEADER+NEXTINT skip.

## §4 Codegen requirement (`idl-ada`)

### §4 Package Zdgen + Marshal + embedded wire helpers

**Spec:** §4 — per struct: Ada record, `Marshal`, self-contained bounded `Buf_T`/`Put_*`.

**Repo:** `crates/idl-ada/src/emitter.rs`; `tools/idlc` `Backend::Ada`, `--ada`.

**Tests:** `crates/idl-ada/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → Ada → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; f32 via IEEE_Float_32.

**Repo:** `crates/idl-ada/src/emitter.rs::map_type/map_primitive`; `endpoints/ada/src/zdw.ads` (Put_*/Get_* over C core align cap 4).

**Tests:** `crates/idl-ada/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a enum

**Spec:** §5 — IDL `enum` (32-bit signed integer on the wire, XTypes 1.3 §7.4.5.1).

**Repo:** `crates/idl-ada/src/emitter.rs::build_enum`/`emit_enum_to_u32` (Ada `type ... is (...)` + portable `<Name>_To_U32` case function) + `map_type` Scoped → `Put_U32 ($w, <Name>_To_U32 (...))`.

**Tests:** `crates/idl-ada/tests/golden.rs::enum_emits_type_and_member_marshals` + `enum_member_is_byte_identical_i32` (`gnatmake`, LE `02000000efbeadde`).

**Status:** done

### §5.b nested struct member + sequence<struct>

**Spec:** §5 — nested struct member + `sequence<struct>` (collection DHEADER, XTypes 1.3 §7.4.3.5.3).

**Repo:** `crates/idl-ada/src/emitter.rs`: body-local `Marshal_Into (V; W : in out Buf_T)` per struct (overloaded per record type; stream-relative alignment) + `map_type` scoped struct → `Marshal_Into (V.<field>, $w)` + `map_sequence` struct element → `<Name>_Vectors` instance (`Ada.Containers.Vectors`) in the spec + collection DHEADER + count + `for E of ... loop Marshal_Into (E, Sub)`.

**Tests:** `crates/idl-ada/tests/golden.rs::nested_struct_emits_marshal_into` + `nested_is_byte_identical_vs_rust_golden` (`gnatmake`, byte-identical against `golden_nested_le/be.bin`).

**Status:** done

### §5.c typedef

**Spec:** §5 — `typedef` (wire-transparent alias; a member of its alias type marshals byte-identically to the underlying type).

**Repo:** `crates/idl-ada/src/emitter.rs::collect_typedefs`/`resolve_typedef` — the alias chain (including `sequence` elements) is resolved to the underlying type before `map_type`.

**Tests:** `crates/idl-ada/tests/golden.rs::typedef_resolves_to_underlying_type` + `typedef_is_byte_identical_vs_rust_golden` (byte-identical against `golden_typedef_le/be.bin`).

**Status:** done

### §5.d array

**Spec:** §5 — fixed arrays (XCDR2 §7.4.3.5.3: elements inline, row-major, multi-dim; no length prefix for primitive elements).

**Repo:** `crates/idl-ada/src/emitter.rs`: `Declarator::Array` branch — `array_size` evaluates the bound, `build_array_put` nests the element put in row-major loops; the field type becomes the language-native array.

**Tests:** `crates/idl-ada/tests/golden.rs::array_*` (byte-identical against `golden_array_le/be.bin`: `long xs[3]` + `short m[2][2]` + `octet bs[4]`).

**Status:** done

### §5.e union

**Spec:** §5 — `union switch(...)` (XCDR2 §7.4.3.5.4: discriminator inline, then the selected member; no DHEADER for @final).

**Repo:** `crates/idl-ada/src/emitter.rs::emit_union` — a holder with a discriminator + one field per case member; `marshalInto` puts the discriminator, then a `switch`/`case`/`match` dispatches on the selected member. Integer-family discriminator + integer labels.

**Tests:** `crates/idl-ada/tests/golden.rs::union_*` (byte-identical against `golden_union_le/be.bin`: disc=2 selects `unsigned short b` — checks non-first-case dispatch).

**Status:** done

### §5.f map

**Spec:** §5 — `map<K, V>` (XCDR2 §7.4.3.5: entries sorted ascending by key, `u32 count` + key/value pairs; no DHEADER for a primitive key/value pair, otherwise DHEADER-framed).

**Repo:** `crates/idl-ada/src/emitter.rs` — map member in the native associative idiom + key sorting before marshalling; primitive-pair rule for the collection DHEADER.

**Tests:** `crates/idl-ada/tests/golden.rs::map_*` (byte-identical against `golden_map_le/be.bin`: `map<long, unsigned long>` {1,2}).

**Status:** done

### §5.g wchar / wstring

**Spec:** §5 — `wchar` (wchar32, UTF-32 code point) + `wstring` (u32 octet length + UTF-16 code units, no BOM).

**Repo:** `crates/idl-ada/src/emitter.rs` — `putWString`/`put_wstring` (manual UTF-16 with surrogate pairs) + `wchar`→`putU32`. Wire core in the prelude.

**Tests:** `crates/idl-ada/tests/golden.rs::wide_is_byte_identical_vs_rust_golden` (byte-identical against `golden_wide_le/be.bin`: c=U+03A9, s="wπ").

**Status:** done

### §5.h long double

**Spec:** §5 — `long double` (IEEE binary128, 16 bytes).

**Repo:** `crates/idl-ada/src/emitter.rs` — `putLongDouble`/`put_long_double`: binary128 via exact widening of the `float64` value (sign + 15-bit exponent + 112-bit mantissa), endian-correct.

**Tests:** `crates/idl-ada/tests/golden.rs::longdouble_is_byte_identical_vs_rust_golden` (byte-identical against `golden_longdouble_le/be.bin`: d=1.1).

**Status:** done

**Note (honest):** Input precision = `float64` (resp. the language's native float), exactly widened to binary128 — covers every float64-representable value. The Rust reference (`idl-rust` + `zerodds-cdr`) remains blocked on native `f128` (no stable Rust f128, ~2027); the goldens are hardcoded from the float64 bits, without f128.


## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with uint32 body length + body.

**Repo:** `crates/idl-ada/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-ada/tests/golden.rs::final_struct_emits_record_and_marshal` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** `crates/idl-ada/src/emitter.rs` — @mutable `marshalInto`: DHEADER-framed member list, per member an EMHEADER (LC4 = `0x40000000 | member-id`) + NEXTINT (body length) + body (serialized into a sub-writer). Member ids from `@id(n)` or sequential.

**Tests:** `crates/idl-ada/tests/golden.rs::mutable_*` (byte-identical against `golden_mutable_le/be.bin`).

**Status:** done
## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), at runtime.

**Repo:** `endpoints/ada/src/zdw.ads` (the writer in `ZDW_BE` mode produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 / XTypes §7.6.8 — codegen of a `keyHash` method from `@key` members.

**Repo:** `crates/idl-ada/src/emitter.rs` — structs with `@key` members get a `keyHash`/`Key_Hash` method: the `@key` members are serialized PLAIN_CDR2-BE (member-id order), a ≤16-byte key holder is zero-padded to 16 bytes, larger (or dynamically sized) ones via MD5(bytes)[0..16]. The static max-key-size analysis lives in the shared `zerodds_idl::keyhash`.

**Tests:** `crates/idl-ada/tests/golden.rs::keyhash_is_byte_identical_vs_rust_golden` (byte-identical against `golden_keyhash.bin` via `zerodds_cdr::compute_key_hash`).

**Status:** done

**Note:** Both branches implemented (XTypes §7.6.8.4 step 5): the static max-key-size analysis (`zerodds_idl::keyhash`) decides ≤16 bytes → zero-pad, otherwise MD5(bytes)[0..16]. Byte-verified against `golden_keyhash_md5.bin` (5×@key long = 20 bytes).


## §8 Wire core

### §8 `endpoints/ada` (FFI over C89) as reference

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/ada/src/zdw.ads` (bindings over `endpoints/c`).

**Tests:** test_byte_identity.adb, CI job `endpoints-ada`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte-for-byte.

**Repo:** `crates/idl-ada`, `endpoints/ada`.

**Tests:** `crates/idl-ada/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-ada`); `endpoints/ada` test_byte_identity.adb (CI `endpoints-ada`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — runnable sync/async examples.

**Repo:** `endpoints/ada/test/example_sync.adb`, `endpoints/ada/test/example_async.adb`, `endpoints/ada/src/deep_reading.ads/.adb`, `endpoints/ada/QUICKSTART.md`.

**Tests:** CI job `endpoints-ada` runs both (`make examples` → `build/example_sync` + `build/example_async`).

**Status:** done

## §11 Errata + open questions

### §11 Honest non-goals

**Spec:** §11 — former non-goals are now built: decode codegen, keyHash codegen, @mutable, wchar/wstring/map/array/nested/union/long double. Full IDL4 byte-verified.

**Repo:** see §3.a/§5.a/§6.a/§7.a (each `done` with decision record).

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

20 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `GOLDEN_DIR=<gold> cargo test -p zerodds-idl-ada` — 29 tests green, 0 failed (codepit, toolchain `gnatmake`).

Open items: none. Decision records: none.
