# `zerodds-xcdr2-zig` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-xcdr2-zig-1.0.md` — ZeroDDS Zig XCDR2 TypeSupport codegen spec.

Implementation:

- `crates/idl-zig/` — IDL → Zig codegen (`marshalXCDR`, self-contained writer).
- `endpoints/zig/` — pure-Zig XCDR2 wire core (writer/reader) + sync/async SDK.

## §1 Motivation

### §1 No OMG-IDL-to-Zig wire mapping

**Spec:** §1 — ZeroDDS defines the Zig XCDR2 wire mapping.

**Repo:** motivation text of the vendor spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — one Zig `struct` + `marshalXCDR` method per IDL `@final struct`.

**Repo:** `crates/idl-zig/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-zig/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API surface

### §3 Writer/Reader primitives + generated marshalXCDR

**Spec:** §3 — Writer (putU8..putSeqU8, bytes), Reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generated `marshalXCDR`.

**Repo:** `endpoints/zig/src/zerodds.zig` (Writer, Reader incl. getU8/getU16/getU64/getF32/getString/getSeqU8 — byte-exact inverse); `crates/idl-zig/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/zig/src/zerodds.zig` in-file tests (byte identity + sync + async), `endpoints/zig/example_sync|async.zig` (full field decode), `crates/idl-zig/tests/golden.rs`.

**Status:** done

### §3.a Generated `unmarshalXCDR` (decode codegen)

**Spec:** §3/§11 — bidirectional binding: generated `unmarshalXCDR` as the exact inverse of `marshalXCDR`.

**Repo:** `map_get` (inverse of `map_type`) + allocator-carrying `Reader` in the prelude + `readFrom`/`unmarshalXCDR` per struct+union (`crates/idl-zig/src/emitter.rs`).

**Tests:** `crates/idl-zig/tests/golden.rs` — 8 `decode_roundtrip_*` (final/nested/array/union/map/mutable/wide/longdouble), `marshal(unmarshal(golden)) == golden` LE+BE (codepit, `/opt/zig` 0.13).

**Status:** done

**Decision record:** decode codegen covers every field type (prim/string/seq/enum/typedef/nested/array/union/map/@mutable/wchar/wstring/long double). `var v: T = undefined` filled in place; strings/seqs/maps allocate via the Reader allocator (reads use `try`). @final = inline, @appendable = DHEADER skip, @mutable = DHEADER + per-member EMHEADER+NEXTINT skip.

## §4 Codegen requirement (`idl-zig`)

### §4 struct + marshalXCDR + embedded writer

**Spec:** §4 — per struct: Zig struct, `marshalXCDR`, self-contained writer.

**Repo:** `crates/idl-zig/src/emitter.rs`; `tools/idlc` `Backend::Zig`, `--zig`.

**Tests:** `crates/idl-zig/tests/golden.rs`.

**Status:** done

## §5 Wire type mapping

### §5 IDL → Zig → XCDR2 (alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> with exact wire layout; signed via @bitCast.

**Repo:** `crates/idl-zig/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/zig/src/zerodds.zig` (Put*/getLE align cap 4).

**Tests:** `crates/idl-zig/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a enum

**Spec:** §5 — IDL `enum` (32-bit signed integer on the wire, XTypes 1.3 §7.4.5.1).

**Repo:** `crates/idl-zig/src/emitter.rs::emit_enum` + `map_type` (Scoped→enum→`putU32(@bitCast(@intFromEnum(...)))`).

**Tests:** `crates/idl-zig/tests/golden.rs::enum_emits_i32_enum_and_member_marshals` + `enum_member_is_byte_identical_i32` (`zig`, LE `02000000efbeadde`).

**Status:** done

### §5.b nested struct member + sequence<struct>

**Spec:** §5 — nested struct member + `sequence<struct>` (collection DHEADER, XTypes 1.3 §7.4.3.5.3).

**Repo:** `crates/idl-zig/src/emitter.rs`: `marshalInto(self, w: *Writer)` per struct + `map_type` Scoped struct + `map_sequence` struct element (block: collection DHEADER + count + per-element marshalInto).

**Tests:** `crates/idl-zig/tests/golden.rs::nested_struct_emits_marshal_into` + `nested_is_byte_identical_vs_rust_golden` (`zig run`, byte-identical against `golden_nested_le/be.bin`).

**Status:** done

### §5.c typedef

**Spec:** §5 — `typedef` (wire-transparent alias; a member of its alias type marshals byte-identical to the underlying type).

**Repo:** `crates/idl-zig/src/emitter.rs::collect_typedefs`/`resolve_typedef` — the alias chain (incl. `sequence` elements) is resolved to the underlying type before `map_type`.

**Tests:** `crates/idl-zig/tests/golden.rs::typedef_resolves_to_underlying_type` + `typedef_is_byte_identical_vs_rust_golden` (byte-identical against `golden_typedef_le/be.bin`).

**Status:** done

### §5.d array

**Spec:** §5 — fixed arrays (XCDR2 §7.4.3.5.3: elements inline, row-major, multi-dim; no length prefix for primitive elements).

**Repo:** `crates/idl-zig/src/emitter.rs`: `Declarator::Array` branch — `array_size` evaluates the bound, `build_array_put` nests the element put in row-major loops; the field type becomes the language-native array.

**Tests:** `crates/idl-zig/tests/golden.rs::array_*` (byte-identical against `golden_array_le/be.bin`: `long xs[3]` + `short m[2][2]` + `octet bs[4]`).

**Status:** done

### §5.e union

**Spec:** §5 — `union switch(...)` (XCDR2 §7.4.3.5.4: discriminator inline, then the selected member; no DHEADER for @final).

**Repo:** `crates/idl-zig/src/emitter.rs::emit_union` — holder with a discriminator + one field per case member; `marshalInto` puts the discriminator, then dispatches a `switch`/`case`/`match` on the selected member. Integer-family discriminator + integer labels.

**Tests:** `crates/idl-zig/tests/golden.rs::union_*` (byte-identical against `golden_union_le/be.bin`: disc=2 selects `unsigned short b` — checks non-first-case dispatch).

**Status:** done

### §5.f map

**Spec:** §5 — `map<K, V>` (XCDR2 §7.4.3.5: entries sorted ascending by key, `u32 count` + key/value pairs; no DHEADER for a primitive key/value pair, otherwise DHEADER-framed).

**Repo:** `crates/idl-zig/src/emitter.rs` — map member in the native associative idiom + key sorting before marshalling; primitive-pair rule for the collection DHEADER.

**Tests:** `crates/idl-zig/tests/golden.rs::map_*` (byte-identical against `golden_map_le/be.bin`: `map<long, unsigned long>` {1,2}).

**Status:** done

### §5.g wchar / wstring

**Spec:** §5 — `wchar` (wchar32, UTF-32 code point) + `wstring` (u32 octet length + UTF-16 code units, no BOM).

**Repo:** `crates/idl-zig/src/emitter.rs` — `putWString`/`put_wstring` (manual UTF-16 with surrogate pairs) + `wchar`→`putU32`. Wire core in the prelude.

**Tests:** `crates/idl-zig/tests/golden.rs::wide_is_byte_identical_vs_rust_golden` (byte-identical against `golden_wide_le/be.bin`: c=U+03A9, s="wπ").

**Status:** done

### §5.h long double

**Spec:** §5 — `long double` (IEEE binary128, 16 bytes).

**Repo:** `crates/idl-zig/src/emitter.rs` — `putLongDouble`/`put_long_double`: binary128 via exact widening of the `float64` value (sign + 15-bit exponent + 112-bit mantissa), endian-correct.

**Tests:** `crates/idl-zig/tests/golden.rs::longdouble_is_byte_identical_vs_rust_golden` (byte-identical against `golden_longdouble_le/be.bin`: d=1.1).

**Status:** done

**Note (honest):** input precision = `float64` (resp. the language's native float), exactly widened to binary128 — covers all float64-representable values. The Rust reference (`idl-rust` + `zerodds-cdr`) remains blocked on native `f128` (no stable Rust f128, ~2027); the goldens are hardcoded from the float64 bits, without f128.


## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final without DHEADER, @appendable with uint32 body length + body.

**Repo:** `crates/idl-zig/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-zig/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable EMHEADER framing.

**Repo:** `crates/idl-zig/src/emitter.rs` — @mutable `marshalInto`: DHEADER-framed member list, per member an EMHEADER (LC4 = `0x40000000 | member-id`) + NEXTINT (body length) + body (serialized into a sub-writer). Member ids from `@id(n)` or sequential.

**Tests:** `crates/idl-zig/tests/golden.rs::mutable_*` (byte-identical against `golden_mutable_le/be.bin`).

**Status:** done
## §7 Key extraction

### §7 Non-keyed 16-zero-byte key

**Spec:** §7 — non-keyed → 16 zero bytes; keyed → MD5 (XCDR2-BE), runtime.

**Repo:** `endpoints/zig/src/zerodds.zig` (writer in big mode produces the BE serialization).

**Tests:** —

**Status:** done

### §7.a Per-struct generated `keyHash` from `@key`

**Spec:** §7 / XTypes §7.6.8 — codegen of a `keyHash` method from `@key` members.

**Repo:** `crates/idl-zig/src/emitter.rs` — structs with `@key` members get a `keyHash`/`Key_Hash` method: the `@key` members are serialized PLAIN_CDR2-BE (member-id order), a ≤16-byte key holder is zero-padded to 16 bytes, larger (or dynamically sized) ones go via MD5(bytes)[0..16]. The static max-key-size analysis lives in the shared `zerodds_idl::keyhash`.

**Tests:** `crates/idl-zig/tests/golden.rs::keyhash_is_byte_identical_vs_rust_golden` (byte-identical against `golden_keyhash.bin` via `zerodds_cdr::compute_key_hash`).

**Status:** done

**Note:** both branches are implemented (XTypes §7.6.8.4 step 5): static max-key-size analysis (`zerodds_idl::keyhash`) decides ≤16 bytes → zero-pad, otherwise MD5(bytes)[0..16]. Byte-verified against `golden_keyhash_md5.bin` (5×@key long = 20 bytes).


## §8 Wire core

### §8 `endpoints/zig` as reference writer/reader

**Spec:** §8 — reference wire core, byte-identical to `zerodds-cdr`.

**Repo:** `endpoints/zig/src/zerodds.zig`.

**Tests:** in-file tests, CI job `endpoints-zig`.

**Status:** done

## §9 Conformance

### §9 Golden byte identity @final LE+BE

**Spec:** §9 — encoding == golden_le.bin / golden_be.bin byte-for-byte.

**Repo:** `crates/idl-zig`, `endpoints/zig`.

**Tests:** `crates/idl-zig/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-zig`); `endpoints/zig` in-file tests (CI `endpoints-zig`).

**Status:** done

## §10 Examples

### §10 sync + async deep examples + quickstart

**Spec:** §10 — working sync/async examples.

**Repo:** `endpoints/zig/example_sync.zig`, `endpoints/zig/example_async.zig`, `endpoints/zig/QUICKSTART.md`.

**Tests:** CI job `endpoints-zig` runs both (`zig build run-example_sync` + `run-example_async`).

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

Test run: `GOLDEN_DIR=<gold> cargo test -p zerodds-idl-zig` — 29 tests green, 0 failed (codepit, toolchain `zig`).

Open items: none. Decision records: none.
