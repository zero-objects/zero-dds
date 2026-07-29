# `zerodds-xcdr2-lua` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-lua-1.0.md` — the ZeroDDS Lua XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-lua/` — IDL → Lua codegen (`marshal_<name>`, self-contained writer prelude).
- `endpoints/lua/` — pure-Lua XCDR2 wire core (writer/reader) + sync/async SDK.

## §1 Motivation

### §1 No OMG-IDL-to-Lua wire mapping

**Spec:** §1 — ZeroDDS defines the Lua XCDR2 wire mapping.

**Repo:** the motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 `marshal_<name>(v, endian)`

**Spec:** §2 — per IDL `@final struct` a `marshal_<name>` function.

**Repo:** `crates/idl-lua/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-lua/tests/golden.rs::final_struct_emits_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/reader primitives + generated marshal

**Spec:** §3 — writer (putU8..putSeqU8, bytes), reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generated `marshal_<name>`.

**Repo:** `endpoints/lua/zerodds.lua` (writer, reader incl. getU8/getU16/getU64/getF32/getString/getSeqU8 — a byte-exact inverse, f32 via `string.unpack("<f")`, u64 via `"I8"`); `crates/idl-lua/src/emitter.rs` (marshal).

**Tests:** `endpoints/lua/test.lua` (byte identity + sync + async), `endpoints/lua/example_sync|async.lua` (full field decode), `crates/idl-lua/tests/golden.rs`.

**Status:** done

### §3.a Generated `unmarshal_<name>` (decode codegen)

**Spec:** §3/§11 — bidirectional binding: generated `unmarshal_{ty}` as the exact inverse of `marshal_{ty}`.

**Repo:** `map_get` (the inverse of `map_type`) + the `Reader` wire core (string.unpack) in the prelude + `read_{ty}`/`unmarshal_{ty}` per struct+union (`crates/idl-lua/src/emitter.rs`).

**Tests:** `crates/idl-lua/tests/golden.rs` — 8 `decode_roundtrip_*` (final/nested/array/union/map/mutable/wide/longdouble), `marshal(unmarshal(golden)) == golden` LE+BE (codepit, lua5.4).

**Status:** done

**Decision record:** decode codegen covers every field type (prim/string/seq/enum/typedef/nested/array/union/map/@mutable/wchar/wstring/long double). Dynamic tables → the holder `v={}` is filled field by field; integers are read as unsigned (put re-masks). @final=inline, @appendable=DHEADER skip, @mutable=DHEADER + per-member EMHEADER+NEXTINT skip.

## §4 Codegen requirement (`idl-lua`)

### §4 marshal_<name> + embedded writer prelude

**Spec:** §4 — per struct: `marshal_<name>`, self-contained writer prelude.

**Repo:** `crates/idl-lua/src/emitter.rs`; `tools/idlc` `Backend::Lua`, `--lua`.

**Tests:** `crates/idl-lua/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → Lua → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; f32 via `string.pack("<f")`.

**Repo:** `crates/idl-lua/src/emitter.rs::map_type/map_primitive`; `endpoints/lua/zerodds.lua` (put*/get* align cap 4).

**Tests:** `crates/idl-lua/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a enum

**Spec:** §5 — IDL `enum` (a 32-bit signed integer on the wire, XTypes 1.3 §7.4.5.1).

**Repo:** `crates/idl-lua/src/emitter.rs::emit_enum` + `map_type` (Scoped→enum→`putU32(... & 0xffffffff)`).

**Tests:** `crates/idl-lua/tests/golden.rs::enum_emits_table_and_member_marshals` + `enum_member_is_byte_identical_i32` (`lua5.4`, LE `02000000efbeadde`).

**Status:** done

### §5.b nested struct member + sequence<struct>

**Spec:** §5 — nested struct member + `sequence<struct>` (collection DHEADER, XTypes 1.3 §7.4.3.5.3).

**Repo:** `crates/idl-lua/src/emitter.rs`: `marshalInto_<T>(w, v)` per struct + `map_type` for scoped struct + `map_sequence` struct element (`do` block: collection DHEADER + count + per-element marshalInto).

**Tests:** `crates/idl-lua/tests/golden.rs::nested_struct_emits_marshal_into` + `nested_is_byte_identical_vs_rust_golden` (`lua5.4`, byte-identical vs. `golden_nested_le/be.bin`).

**Status:** done

### §5.c typedef

**Spec:** §5 — `typedef` (a wire-transparent alias; a member of its alias type marshals byte-identically to the underlying type).

**Repo:** `crates/idl-lua/src/emitter.rs::collect_typedefs`/`resolve_typedef` — the alias chain (incl. `sequence` elements) is resolved to the underlying type before `map_type`.

**Tests:** `crates/idl-lua/tests/golden.rs::typedef_resolves_to_underlying_type` + `typedef_is_byte_identical_vs_rust_golden` (byte-identical vs. `golden_typedef_le/be.bin`).

**Status:** done

### §5.d array

**Spec:** §5 — fixed arrays (XCDR2 §7.4.3.5.3: elements inline, row-major, multi-dim; no length prefix for primitive elements).

**Repo:** `crates/idl-lua/src/emitter.rs`: the `Declarator::Array` branch — `array_size` evaluates the bound, `build_array_put` nests the element put in row-major loops; the field type becomes the language-native array.

**Tests:** `crates/idl-lua/tests/golden.rs::array_*` (byte-identical vs. `golden_array_le/be.bin`: `long xs[3]` + `short m[2][2]` + `octet bs[4]`).

**Status:** done

### §5.e union

**Spec:** §5 — `union switch(...)` (XCDR2 §7.4.3.5.4: discriminator inline, then the selected member; no DHEADER for @final).

**Repo:** `crates/idl-lua/src/emitter.rs::emit_union` — a holder with a discriminator + one field per case member; `marshalInto` puts the discriminator, then a `switch`/`case`/`match` dispatches to the selected member. Integer-family discriminator + integer labels.

**Tests:** `crates/idl-lua/tests/golden.rs::union_*` (byte-identical vs. `golden_union_le/be.bin`: disc=2 selects `unsigned short b` — checks non-first-case dispatch).

**Status:** done

### §5.f map

**Spec:** §5 — `map<K, V>` (XCDR2 §7.4.3.5: entries sorted ascending by key, `u32 count` + key/value pairs; no DHEADER for a primitive key/value pair, otherwise DHEADER-framed).

**Repo:** `crates/idl-lua/src/emitter.rs` — a map member in the native associative idiom + key sorting before marshalling; the primitive-pair rule for the collection DHEADER.

**Tests:** `crates/idl-lua/tests/golden.rs::map_*` (byte-identical vs. `golden_map_le/be.bin`: `map<long, unsigned long>` {1,2}).

**Status:** done

### §5.g wchar / wstring

**Spec:** §5 — `wchar` (wchar32, a UTF-32 code point) + `wstring` (u32 octet length + UTF-16 code units, no BOM).

**Repo:** `crates/idl-lua/src/emitter.rs` — `putWString`/`put_wstring` (manual UTF-16 with surrogate pairs) + `wchar`→`putU32`. Wire core in the prelude.

**Tests:** `crates/idl-lua/tests/golden.rs::wide_is_byte_identical_vs_rust_golden` (byte-identical vs. `golden_wide_le/be.bin`: c=U+03A9, s="wπ").

**Status:** done

### §5.h long double

**Spec:** §5 — `long double` (IEEE binary128, 16 bytes).

**Repo:** `crates/idl-lua/src/emitter.rs` — `putLongDouble`/`put_long_double`: binary128 by exactly widening the `float64` value (sign + 15-bit exponent + 112-bit mantissa), endian-correct.

**Tests:** `crates/idl-lua/tests/golden.rs::longdouble_is_byte_identical_vs_rust_golden` (byte-identical vs. `golden_longdouble_le/be.bin`: d=1.1).

**Status:** done

**Note (honest):** input precision = `float64` (or the language's native float), exactly widened to binary128 — covers every float64-representable value. The Rust reference (`idl-rust` + `zerodds-cdr`) remains blocked for native `f128` (no stable Rust f128, ~2027); the goldens are hardcoded from the float64 bits, without f128.


## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with a uint32 body length + body.

**Repo:** `crates/idl-lua/src/emitter.rs::emit_struct` (final/appendable).

**Tests:** `crates/idl-lua/tests/golden.rs::final_struct_emits_marshal` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** `crates/idl-lua/src/emitter.rs` — the @mutable `marshalInto`: a DHEADER-framed member list, per member an EMHEADER (LC4 = `0x40000000 | member-id`) + NEXTINT (body length) + body (serialized into a sub-writer). Member IDs from `@id(n)` or sequential.

**Tests:** `crates/idl-lua/tests/golden.rs::mutable_*` (byte-identical vs. `golden_mutable_le/be.bin`).

**Status:** done
## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), at runtime.

**Repo:** `endpoints/lua/zerodds.lua` (the writer with a BE prefix produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 / XTypes §7.6.8 — codegen of a `keyHash` method from `@key` members.

**Repo:** `crates/idl-lua/src/emitter.rs` — structs with `@key` members get a `keyHash`/`Key_Hash` method: the `@key` members are serialized PLAIN_CDR2-BE (member-id order), a ≤16-byte key holder is zero-padded to 16 bytes, larger (or dynamically sized) ones go via MD5(bytes)[0..16]. The static max-key-size analysis lives in the shared `zerodds_idl::keyhash`.

**Tests:** `crates/idl-lua/tests/golden.rs::keyhash_is_byte_identical_vs_rust_golden` (byte-identical vs. `golden_keyhash.bin` via `zerodds_cdr::compute_key_hash`).

**Status:** done

**Note:** both branches implemented (XTypes §7.6.8.4 step 5): the static max-key-size analysis (`zerodds_idl::keyhash`) decides ≤16 bytes → zero-pad, otherwise MD5(bytes)[0..16]. Byte-verified against `golden_keyhash_md5.bin` (5×@key long = 20 bytes).


## §8 Wire core

### §8 `endpoints/lua` as the reference writer/reader

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/lua/zerodds.lua`.

**Tests:** test.lua, CI job `endpoints-lua`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte for byte.

**Repo:** `crates/idl-lua`, `endpoints/lua`.

**Tests:** `crates/idl-lua/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-lua`); `endpoints/lua` test.lua (CI `endpoints-lua`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — runnable sync/async examples.

**Repo:** `endpoints/lua/example_sync.lua`, `endpoints/lua/example_async.lua`, `endpoints/lua/QUICKSTART.md`.

**Tests:** CI job `endpoints-lua` runs both (`lua5.4 example_sync.lua` + `example_async.lua`).

**Status:** done

## §11 Errata + open questions

### §11 Honest non-goals

**Spec:** §11 — former non-goals have been built: decode codegen, keyHash codegen, @mutable, wchar/wstring/map/array/nested/union/long double. Full IDL4 byte-verified.

**Repo:** see §3.a/§5.a/§6.a/§7.a (each `done` with a decision record).

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

20 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `GOLDEN_DIR=<gold> cargo test -p zerodds-idl-lua` — 29 tests green, 0 failed (codepit, toolchain `lua5.4`).

Open items: none. Decision records: none.
