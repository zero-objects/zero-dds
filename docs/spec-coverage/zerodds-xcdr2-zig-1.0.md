# `zerodds-xcdr2-zig` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-zig-1.0.md` — ZeroDDS Zig XCDR2 TypeSupport-Codegen-Spec.

Implementation:

- `crates/idl-zig/` — IDL → Zig Codegen (`marshalXCDR`, self-contained Writer).
- `endpoints/zig/` — pure-Zig XCDR2 Wire-Core (Writer/Reader) + sync/async SDK.

## §1 Motivation

### §1 Kein OMG-IDL-to-Zig-Wire-Mapping

**Spec:** §1 — ZeroDDS definiert das Zig-XCDR2-Wire-Mapping.

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal-Pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — pro IDL-`@final struct` ein Zig-`struct` + `marshalXCDR`-Methode.

**Repo:** `crates/idl-zig/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-zig/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API-Surface

### §3 Writer/Reader-Primitiven + generierte marshalXCDR

**Spec:** §3 — Writer (putU8..putSeqU8, bytes), Reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generierte `marshalXCDR`.

**Repo:** `endpoints/zig/src/zerodds.zig` (Writer, Reader inkl. getU8/getU16/getU64/getF32/getString/getSeqU8 — byte-exakter Inverse); `crates/idl-zig/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/zig/src/zerodds.zig` In-File-Tests (byte identity + sync + async), `endpoints/zig/example_sync|async.zig` (voller Feld-Decode), `crates/idl-zig/tests/golden.rs`.

**Status:** done

### §3.a Generierte `unmarshalXCDR` (decode-codegen)

**Spec:** §3/§11 — bidirektionales Binding: generiertes `unmarshalXCDR` als exaktes Inverses von `marshalXCDR`.

**Repo:** `map_get` (Invers von `map_type`) + allocator-getragener `Reader` im Prelude + `readFrom`/`unmarshalXCDR` je struct+union (`crates/idl-zig/src/emitter.rs`).

**Tests:** `crates/idl-zig/tests/golden.rs` — 8 `decode_roundtrip_*` (final/nested/array/union/map/mutable/wide/longdouble), `marshal(unmarshal(golden)) == golden` LE+BE (codepit, `/opt/zig` 0.13).

**Status:** done

**Decision-Record:** Decode-codegen deckt jeden Feldtyp ab (prim/string/seq/enum/typedef/nested/array/union/map/@mutable/wchar/wstring/long double). `var v: T = undefined` in-place gefüllt; strings/seqs/maps allozieren via Reader-Allocator (Reads nutzen `try`). @final=inline, @appendable=DHEADER-skip, @mutable=DHEADER + je Member EMHEADER+NEXTINT skip.

## §4 Codegen-Pflicht (`idl-zig`)

### §4 struct + marshalXCDR + eingebetteter Writer

**Spec:** §4 — pro struct: Zig-struct, `marshalXCDR`, self-contained Writer.

**Repo:** `crates/idl-zig/src/emitter.rs`; `tools/idlc` `Backend::Zig`, `--zig`.

**Tests:** `crates/idl-zig/tests/golden.rs`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL → Zig → XCDR2 (Alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> mit exaktem Wire-Layout; signed via @bitCast.

**Repo:** `crates/idl-zig/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/zig/src/zerodds.zig` (Put*/getLE align cap 4).

**Tests:** `crates/idl-zig/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a enum

**Spec:** §5 — IDL `enum` (32-bit signed integer auf der Wire, XTypes 1.3 §7.4.5.1).

**Repo:** `crates/idl-zig/src/emitter.rs::emit_enum` + `map_type` (Scoped→enum→`putU32(@bitCast(@intFromEnum(...)))`).

**Tests:** `crates/idl-zig/tests/golden.rs::enum_emits_i32_enum_and_member_marshals` + `enum_member_is_byte_identical_i32` (`zig`, LE `02000000efbeadde`).

**Status:** done

### §5.b nested struct member + sequence<struct>

**Spec:** §5 — nested struct-Member + `sequence<struct>` (Collection-DHEADER, XTypes 1.3 §7.4.3.5.3).

**Repo:** `crates/idl-zig/src/emitter.rs`: `marshalInto(self, w: *Writer)` je struct + `map_type` Scoped-struct + `map_sequence` struct-Element (Block: Collection-DHEADER + count + per-element marshalInto).

**Tests:** `crates/idl-zig/tests/golden.rs::nested_struct_emits_marshal_into` + `nested_is_byte_identical_vs_rust_golden` (`zig run`, byte-identisch gegen `golden_nested_le/be.bin`).

**Status:** done

### §5.c typedef

**Spec:** §5 — `typedef` (wire-transparenter Alias; ein Member seines Alias-Typs marshallt byte-identisch zum Grundtyp).

**Repo:** `crates/idl-zig/src/emitter.rs::collect_typedefs`/`resolve_typedef` — Alias-Kette (inkl. `sequence`-Elemente) wird vor `map_type` zum Grundtyp aufgelöst.

**Tests:** `crates/idl-zig/tests/golden.rs::typedef_resolves_to_underlying_type` + `typedef_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_typedef_le/be.bin`).

**Status:** done

### §5.d array

**Spec:** §5 — feste Arrays (XCDR2 §7.4.3.5.3: Elemente inline, row-major, multi-dim; kein Längen-Präfix bei primitiven Elementen).

**Repo:** `crates/idl-zig/src/emitter.rs`: `Declarator::Array`-Zweig — `array_size` wertet den Bound aus, `build_array_put` verschachtelt den Element-Put in row-major-Loops; Feldtyp wird das sprachnative Array.

**Tests:** `crates/idl-zig/tests/golden.rs::array_*` (byte-identisch gegen `golden_array_le/be.bin`: `long xs[3]` + `short m[2][2]` + `octet bs[4]`).

**Status:** done

### §5.e union

**Spec:** §5 — `union switch(...)` (XCDR2 §7.4.3.5.4: Diskriminator inline, dann das selektierte Member; kein DHEADER bei @final).

**Repo:** `crates/idl-zig/src/emitter.rs::emit_union` — Holder mit Diskriminator + einem Feld je Case-Member; `marshalInto` putet den Diskriminator, dann dispatcht ein `switch`/`case`/`match` auf das selektierte Member. Integer-Family-Diskriminator + Integer-Labels.

**Tests:** `crates/idl-zig/tests/golden.rs::union_*` (byte-identisch gegen `golden_union_le/be.bin`: disc=2 selektiert `unsigned short b` — prüft Nicht-First-Case-Dispatch).

**Status:** done

### §5.f map

**Spec:** §5 — `map<K, V>` (XCDR2 §7.4.3.5: Einträge aufsteigend nach Key sortiert, `u32 count` + Key/Value-Paare; kein DHEADER bei primitivem Key/Value-Paar, sonst DHEADER-gerahmt).

**Repo:** `crates/idl-zig/src/emitter.rs` — Map-Member im nativen assoziativen Idiom + Key-Sortierung vor dem Marshalling; primitiv-Paar-Regel für den Collection-DHEADER.

**Tests:** `crates/idl-zig/tests/golden.rs::map_*` (byte-identisch gegen `golden_map_le/be.bin`: `map<long, unsigned long>` {1,2}).

**Status:** done

### §5.g wchar / wstring

**Spec:** §5 — `wchar` (wchar32, UTF-32 code point) + `wstring` (u32 Oktett-Länge + UTF-16 code units, no BOM).

**Repo:** `crates/idl-zig/src/emitter.rs` — `putWString`/`put_wstring` (manuelles UTF-16 mit Surrogatpaaren) + `wchar`→`putU32`. Wire-Core im Prelude.

**Tests:** `crates/idl-zig/tests/golden.rs::wide_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_wide_le/be.bin`: c=U+03A9, s="wπ").

**Status:** done

### §5.h long double

**Spec:** §5 — `long double` (IEEE binary128, 16 Byte).

**Repo:** `crates/idl-zig/src/emitter.rs` — `putLongDouble`/`put_long_double`: binary128 durch exaktes Widening des `float64`-Werts (Sign + 15-Bit-Exponent + 112-Bit-Mantisse), endian-korrekt.

**Tests:** `crates/idl-zig/tests/golden.rs::longdouble_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_longdouble_le/be.bin`: d=1.1).

**Status:** done

**Anmerkung (ehrlich):** Eingabepräzision = `float64` (bzw. natives Float der Sprache), exakt auf binary128 geweitet — deckt alle float64-darstellbaren Werte. Die Rust-Referenz (`idl-rust` + `zerodds-cdr`) bleibt für natives `f128` blockiert (kein stabiles Rust-f128, ~2027); die Golden werden aus den float64-Bits hartkodiert, ohne f128.


## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final ohne DHEADER, @appendable mit uint32-Body-Length + Body.

**Repo:** `crates/idl-zig/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-zig/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable-EMHEADER-Framing.

**Repo:** `crates/idl-zig/src/emitter.rs` — @mutable-`marshalInto`: DHEADER-gerahmte Member-Liste, je Member EMHEADER (LC4 = `0x40000000 | member-id`) + NEXTINT (Body-Länge) + Body (in Sub-Writer serialisiert). Member-ids aus `@id(n)` oder sequenziell.

**Tests:** `crates/idl-zig/tests/golden.rs::mutable_*` (byte-identisch gegen `golden_mutable_le/be.bin`).

**Status:** done
## §7 Key-Extraction

### §7 Non-keyed 16-Zero-Byte-Key

**Spec:** §7 — non-keyed → 16 Nullbytes; keyed → MD5 (XCDR2-BE), Runtime.

**Repo:** `endpoints/zig/src/zerodds.zig` (Writer im Big-Mode liefert die BE-Serialisierung).

**Tests:** —

**Status:** done

### §7.a Per-struct generierte `keyHash` aus `@key`

**Spec:** §7 / XTypes §7.6.8 — Codegen einer `keyHash`-Methode aus `@key`-Membern.

**Repo:** `crates/idl-zig/src/emitter.rs` — Structs mit `@key`-Membern erhalten eine `keyHash`/`Key_Hash`-Methode: die `@key`-Member PLAIN_CDR2-BE serialisiert (member-id-Reihenfolge), ≤16-Byte-Key-Holder auf 16 Byte zero-gepaddet, größere (oder dynamisch dimensionierte) via MD5(bytes)[0..16]. Die statische max-Key-Size-Analyse ist im geteilten `zerodds_idl::keyhash`.

**Tests:** `crates/idl-zig/tests/golden.rs::keyhash_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_keyhash.bin` via `zerodds_cdr::compute_key_hash`).

**Status:** done

**Anmerkung:** Beide Zweige implementiert (XTypes §7.6.8.4 step 5): statische max-Key-Size-Analyse (`zerodds_idl::keyhash`) entscheidet ≤16 Byte → zero-pad, sonst MD5(bytes)[0..16]. Byte-verifiziert gegen `golden_keyhash_md5.bin` (5×@key long = 20 Byte).


## §8 Wire-Core

### §8 `endpoints/zig` als Referenz-Writer/Reader

**Spec:** §8 — Referenz-Wire-Core, byte-identisch zu `zerodds-cdr`.

**Repo:** `endpoints/zig/src/zerodds.zig`.

**Tests:** In-File-Tests, CI-Job `endpoints-zig`.

**Status:** done

## §9 Conformance

### §9 Golden-Byte-Identität @final LE+BE

**Spec:** §9 — Encoding == golden_le.bin / golden_be.bin byte-für-byte.

**Repo:** `crates/idl-zig`, `endpoints/zig`.

**Tests:** `crates/idl-zig/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-zig`); `endpoints/zig` In-File-Tests (CI `endpoints-zig`).

**Status:** done

## §10 Examples

### §10 sync + async Deep-Examples + Quickstart

**Spec:** §10 — lauffähige sync/async-Beispiele.

**Repo:** `endpoints/zig/example_sync.zig`, `endpoints/zig/example_async.zig`, `endpoints/zig/QUICKSTART.md`.

**Tests:** CI-Job `endpoints-zig` führt beide aus (`zig build run-example_sync` + `run-example_async`).

**Status:** done

## §11 Errata + Open-Questions

### §11 Ehrliche Nicht-Ziele

**Spec:** §11 — vormalige Nicht-Ziele sind gebaut: decode-codegen, keyHash-codegen, @mutable, wchar/wstring/map/array/nested/union/long double. Voll-IDL4 byte-verifiziert.

**Repo:** siehe §3.a/§5.a/§6.a/§7.a (je `done` mit Decision-Record).

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

20 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `GOLDEN_DIR=<gold> cargo test -p zerodds-idl-zig` — 29 Tests grün, 0 failed (codepit, Toolchain `zig`).

Offene Punkte: keine. Decision-Records: keine.
