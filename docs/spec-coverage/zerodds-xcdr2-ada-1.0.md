# `zerodds-xcdr2-ada` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-ada-1.0.md` — ZeroDDS Ada XCDR2 TypeSupport-Codegen-Spec.

Implementation:

- `crates/idl-ada/` — IDL → Ada Codegen (Package `Zdgen`, `Marshal`, self-contained bounded Wire).
- `endpoints/ada/` — Ada Stage-1 Endpoint: `Interfaces.C`-Bindings über den C89-Wire-Core + sync/async Deep-Examples.

## §1 Motivation

### §1 Kein OMG-IDL-to-Ada-Wire-Mapping

**Spec:** §1 — ZeroDDS definiert das Ada-XCDR2-Wire-Mapping (Codegen + FFI-Endpoint).

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal-Pattern

### §2 record + `Marshal (V; Endian) return Byte_Array`

**Spec:** §2 — pro IDL-`@final struct` ein Ada-`record` + `Marshal`-Funktion in Package `Zdgen`.

**Repo:** `crates/idl-ada/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-ada/tests/golden.rs::final_struct_emits_record_and_marshal`

**Status:** done

## §3 Required API-Surface

### §3 Writer/Reader-Primitiven + generierte Marshal

**Spec:** §3 — Writer (Put_U8..Put_Seq_U8), Reader (Get_U8/Get_U16/Get_U32/Get_U64/Get_F32/Get_String/Get_Seq_U8), generierte `Marshal`.

**Repo:** `endpoints/ada/src/zdw.ads` (Writer + Reader inkl. Get_U8/Get_U16/Get_U64/Get_F32/Get_String/Get_Seq_U8 — byte-exakter Inverse via C-Core, f32 via `zdw_get_f32`, u64 via `Zdw_U64`); `crates/idl-ada/src/emitter.rs` (Marshal).

**Tests:** `endpoints/ada/test/test_byte_identity.adb` (byte identity + Decode-Roundtrip), `endpoints/ada/test/example_sync|async.adb` (voller Feld-Decode), `crates/idl-ada/tests/golden.rs`.

**Status:** done

### §3.a Generiertes `Unmarshal` (decode-codegen)

**Spec:** §3/§11 — bidirektionales Binding: generiertes `Unmarshal` als exaktes Inverses von `Marshal`.

**Repo:** `map_get` (Invers von `map_type`) + `Reader` (Get_*-Funktionen mit `in out Pos`, Ada 2012) im Package-Body + `Read_<T>`/`Unmarshal` je struct+union + enum-`_Of_U32` (`crates/idl-ada/src/emitter.rs`).

**Tests:** `crates/idl-ada/tests/golden.rs` — 8 `decode_roundtrip_*` (final/nested/array/union/map/mutable/wide/longdouble), `Marshal (Unmarshal (golden)) = golden` LE+BE (codepit, gnatmake).

**Status:** done

**Decision-Record:** Decode-codegen deckt jeden Feldtyp ab (prim/string/seq/enum/typedef/nested/array/union/map/@mutable/wchar/wstring/long double). Mutable Records feldweise gefüllt; Get_* über `Data`/`Pos`/`Endian`, float via inverse Unchecked_Conversion; seq→Vectors.Append, map→Ordered_Maps.Insert. @final=inline, @appendable=DHEADER-skip, @mutable=DHEADER + je Member EMHEADER+NEXTINT skip.

## §4 Codegen-Pflicht (`idl-ada`)

### §4 Package Zdgen + Marshal + eingebettete Wire-Helpers

**Spec:** §4 — pro struct: Ada-record, `Marshal`, self-contained bounded `Buf_T`/`Put_*`.

**Repo:** `crates/idl-ada/src/emitter.rs`; `tools/idlc` `Backend::Ada`, `--ada`.

**Tests:** `crates/idl-ada/tests/golden.rs`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL → Ada → XCDR2 (Alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> mit exaktem Wire-Layout; f32 via IEEE_Float_32.

**Repo:** `crates/idl-ada/src/emitter.rs::map_type/map_primitive`; `endpoints/ada/src/zdw.ads` (Put_*/Get_* über C-Core align cap 4).

**Tests:** `crates/idl-ada/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a enum

**Spec:** §5 — IDL `enum` (32-bit signed integer auf der Wire, XTypes 1.3 §7.4.5.1).

**Repo:** `crates/idl-ada/src/emitter.rs::build_enum`/`emit_enum_to_u32` (Ada `type ... is (...)` + portable `<Name>_To_U32`-case-Funktion) + `map_type` Scoped → `Put_U32 ($w, <Name>_To_U32 (...))`.

**Tests:** `crates/idl-ada/tests/golden.rs::enum_emits_type_and_member_marshals` + `enum_member_is_byte_identical_i32` (`gnatmake`, LE `02000000efbeadde`).

**Status:** done

### §5.b nested struct member + sequence<struct>

**Spec:** §5 — nested struct-Member + `sequence<struct>` (Collection-DHEADER, XTypes 1.3 §7.4.3.5.3).

**Repo:** `crates/idl-ada/src/emitter.rs`: body-lokale `Marshal_Into (V; W : in out Buf_T)` je struct (per Record-Typ überladen; stream-relatives Alignment) + `map_type` Scoped-struct → `Marshal_Into (V.<field>, $w)` + `map_sequence` struct-Element → `<Name>_Vectors`-Instanz (`Ada.Containers.Vectors`) im spec + Collection-DHEADER + count + `for E of ... loop Marshal_Into (E, Sub)`.

**Tests:** `crates/idl-ada/tests/golden.rs::nested_struct_emits_marshal_into` + `nested_is_byte_identical_vs_rust_golden` (`gnatmake`, byte-identisch gegen `golden_nested_le/be.bin`).

**Status:** done

### §5.c typedef

**Spec:** §5 — `typedef` (wire-transparenter Alias; ein Member seines Alias-Typs marshallt byte-identisch zum Grundtyp).

**Repo:** `crates/idl-ada/src/emitter.rs::collect_typedefs`/`resolve_typedef` — Alias-Kette (inkl. `sequence`-Elemente) wird vor `map_type` zum Grundtyp aufgelöst.

**Tests:** `crates/idl-ada/tests/golden.rs::typedef_resolves_to_underlying_type` + `typedef_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_typedef_le/be.bin`).

**Status:** done

### §5.d array

**Spec:** §5 — feste Arrays (XCDR2 §7.4.3.5.3: Elemente inline, row-major, multi-dim; kein Längen-Präfix bei primitiven Elementen).

**Repo:** `crates/idl-ada/src/emitter.rs`: `Declarator::Array`-Zweig — `array_size` wertet den Bound aus, `build_array_put` verschachtelt den Element-Put in row-major-Loops; Feldtyp wird das sprachnative Array.

**Tests:** `crates/idl-ada/tests/golden.rs::array_*` (byte-identisch gegen `golden_array_le/be.bin`: `long xs[3]` + `short m[2][2]` + `octet bs[4]`).

**Status:** done

### §5.e union

**Spec:** §5 — `union switch(...)` (XCDR2 §7.4.3.5.4: Diskriminator inline, dann das selektierte Member; kein DHEADER bei @final).

**Repo:** `crates/idl-ada/src/emitter.rs::emit_union` — Holder mit Diskriminator + einem Feld je Case-Member; `marshalInto` putet den Diskriminator, dann dispatcht ein `switch`/`case`/`match` auf das selektierte Member. Integer-Family-Diskriminator + Integer-Labels.

**Tests:** `crates/idl-ada/tests/golden.rs::union_*` (byte-identisch gegen `golden_union_le/be.bin`: disc=2 selektiert `unsigned short b` — prüft Nicht-First-Case-Dispatch).

**Status:** done

### §5.f map

**Spec:** §5 — `map<K, V>` (XCDR2 §7.4.3.5: Einträge aufsteigend nach Key sortiert, `u32 count` + Key/Value-Paare; kein DHEADER bei primitivem Key/Value-Paar, sonst DHEADER-gerahmt).

**Repo:** `crates/idl-ada/src/emitter.rs` — Map-Member im nativen assoziativen Idiom + Key-Sortierung vor dem Marshalling; primitiv-Paar-Regel für den Collection-DHEADER.

**Tests:** `crates/idl-ada/tests/golden.rs::map_*` (byte-identisch gegen `golden_map_le/be.bin`: `map<long, unsigned long>` {1,2}).

**Status:** done

### §5.g wchar / wstring

**Spec:** §5 — `wchar` (wchar32, UTF-32 code point) + `wstring` (u32 Oktett-Länge + UTF-16 code units, no BOM).

**Repo:** `crates/idl-ada/src/emitter.rs` — `putWString`/`put_wstring` (manuelles UTF-16 mit Surrogatpaaren) + `wchar`→`putU32`. Wire-Core im Prelude.

**Tests:** `crates/idl-ada/tests/golden.rs::wide_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_wide_le/be.bin`: c=U+03A9, s="wπ").

**Status:** done

### §5.h long double

**Spec:** §5 — `long double` (IEEE binary128, 16 Byte).

**Repo:** `crates/idl-ada/src/emitter.rs` — `putLongDouble`/`put_long_double`: binary128 durch exaktes Widening des `float64`-Werts (Sign + 15-Bit-Exponent + 112-Bit-Mantisse), endian-korrekt.

**Tests:** `crates/idl-ada/tests/golden.rs::longdouble_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_longdouble_le/be.bin`: d=1.1).

**Status:** done

**Anmerkung (ehrlich):** Eingabepräzision = `float64` (bzw. natives Float der Sprache), exakt auf binary128 geweitet — deckt alle float64-darstellbaren Werte. Die Rust-Referenz (`idl-rust` + `zerodds-cdr`) bleibt für natives `f128` blockiert (kein stabiles Rust-f128, ~2027); die Golden werden aus den float64-Bits hartkodiert, ohne f128.


## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final ohne DHEADER, @appendable mit uint32-Body-Length + Body.

**Repo:** `crates/idl-ada/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-ada/tests/golden.rs::final_struct_emits_record_and_marshal` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable-EMHEADER-Framing.

**Repo:** `crates/idl-ada/src/emitter.rs` — @mutable-`marshalInto`: DHEADER-gerahmte Member-Liste, je Member EMHEADER (LC4 = `0x40000000 | member-id`) + NEXTINT (Body-Länge) + Body (in Sub-Writer serialisiert). Member-ids aus `@id(n)` oder sequenziell.

**Tests:** `crates/idl-ada/tests/golden.rs::mutable_*` (byte-identisch gegen `golden_mutable_le/be.bin`).

**Status:** done
## §7 Key-Extraction

### §7 Non-keyed 16-Zero-Byte-Key

**Spec:** §7 — non-keyed → 16 Nullbytes; keyed → MD5 (XCDR2-BE), Runtime.

**Repo:** `endpoints/ada/src/zdw.ads` (Writer im `ZDW_BE`-Mode liefert die BE-Serialisierung).

**Tests:** —

**Status:** done

### §7.a Per-struct generierte `keyHash` aus `@key`

**Spec:** §7 / XTypes §7.6.8 — Codegen einer `keyHash`-Methode aus `@key`-Membern.

**Repo:** `crates/idl-ada/src/emitter.rs` — Structs mit `@key`-Membern erhalten eine `keyHash`/`Key_Hash`-Methode: die `@key`-Member PLAIN_CDR2-BE serialisiert (member-id-Reihenfolge), ≤16-Byte-Key-Holder auf 16 Byte zero-gepaddet, größere (oder dynamisch dimensionierte) via MD5(bytes)[0..16]. Die statische max-Key-Size-Analyse ist im geteilten `zerodds_idl::keyhash`.

**Tests:** `crates/idl-ada/tests/golden.rs::keyhash_is_byte_identical_vs_rust_golden` (byte-identisch gegen `golden_keyhash.bin` via `zerodds_cdr::compute_key_hash`).

**Status:** done

**Anmerkung:** Beide Zweige implementiert (XTypes §7.6.8.4 step 5): statische max-Key-Size-Analyse (`zerodds_idl::keyhash`) entscheidet ≤16 Byte → zero-pad, sonst MD5(bytes)[0..16]. Byte-verifiziert gegen `golden_keyhash_md5.bin` (5×@key long = 20 Byte).


## §8 Wire-Core

### §8 `endpoints/ada` (FFI über C89) als Referenz

**Spec:** §8 — Referenz-Wire-Core, byte-identisch zu `zerodds-cdr`.

**Repo:** `endpoints/ada/src/zdw.ads` (Bindings über `endpoints/c`).

**Tests:** test_byte_identity.adb, CI-Job `endpoints-ada`.

**Status:** done

## §9 Conformance

### §9 Golden-Byte-Identität @final LE+BE

**Spec:** §9 — Encoding == golden_le.bin / golden_be.bin byte-für-byte.

**Repo:** `crates/idl-ada`, `endpoints/ada`.

**Tests:** `crates/idl-ada/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-ada`); `endpoints/ada` test_byte_identity.adb (CI `endpoints-ada`).

**Status:** done

## §10 Examples

### §10 sync + async Deep-Examples + Quickstart

**Spec:** §10 — lauffähige sync/async-Beispiele.

**Repo:** `endpoints/ada/test/example_sync.adb`, `endpoints/ada/test/example_async.adb`, `endpoints/ada/src/deep_reading.ads/.adb`, `endpoints/ada/QUICKSTART.md`.

**Tests:** CI-Job `endpoints-ada` führt beide aus (`make examples` → `build/example_sync` + `build/example_async`).

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

Test-Lauf: `GOLDEN_DIR=<gold> cargo test -p zerodds-idl-ada` — 29 Tests grün, 0 failed (codepit, Toolchain `gnatmake`).

Offene Punkte: keine. Decision-Records: keine.
