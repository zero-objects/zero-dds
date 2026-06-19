# Bounded-Sequence-Bound-Enforcement (typisierter Pfad) — Followup

**Status**: **ERLEDIGT in allen vier Codegens (2026-06-11)** — der Encode
erzwingt die Bounds (XTypes §7.4.3) in **idl-rust, idl-java, idl-csharp und
idl-cpp**. Je ein Test pro Codegen, alle Suites grün.

- **idl-rust** (`struct_emit.rs` `emit_bound_checks_decl`/`emit_bound_checks`/
  `type_has_bounds` + `union_emit.rs`; `bounded_sequence.rs`): seq (Top-Level +
  **verschachtelt** rekursiv + **Array-Element** statt Array-Laenge), narrow
  string (UTF-8-Byte), **wstring** (UTF-16-Unit), **map** (size + values + keys),
  **union-arms**. (Der Array-Edge-Case-Bug des Erst-Fixes wurde dabei behoben.)
- **idl-java** / **idl-csharp** (TypeSupport-Encode; `bounded_collections.rs`):
  seq, narrow string (UTF-8-Byte), wide wstring (UTF-16-Unit) —
  `throw IllegalArgumentException` / `ArgumentException`.
- **idl-cpp** (`emitter.rs` beide Value-Emitter; `bounded_collections.rs`): seq,
  narrow string — `throw std::length_error` + **konditionales** `<stdexcept>`
  (Header ohne bounded Typen byte-identisch). `wstring`/nested sind im
  cpp-Encode generell „nicht unterstuetzt".

**Datum**: 2026-06-11
**Kontext**: aufgetaucht beim OpenDDS-Non-Secure-Interop-Closeout
(`docs/interop/opendds-interop-closeout.md`).

## Was ist offen

Erzwingt ZeroDDS' **typisierter** Codegen-/Encode-Pfad die Bound einer
**bounded sequence** (`sequence<octet, N>`, XTypes 1.3 §7.2.2.4.3 /
DDS-XTypes „bounded collection")?

Beobachtung aus dem Bench: Der Roundtrip-Typ ist
`@id(2) sequence<octet, 8192> payload` (`tests/perf/dds-roundtrip-bench/roundtrip.idl`).
Bei einem Stress-Payload **> 8192** Byte:

- **OpenDDS** wirft spec-korrekt `CORBA::BAD_PARAM` beim `write` (der
  generierte TypeSupport prüft die Bound). Reproduzierbar auch
  OpenDDS↔OpenDDS-self. → Korrektes Verhalten.
- **ZeroDDS** (im Bench über die **byte-orientierte C-FFI**
  `zerodds_writer_write` mit rohen Bytes) akzeptiert 16384 und liefert
  (zerodds-self grün bei 16384). Die Byte-FFI kennt den IDL-Typ
  bewusst nicht und prüft daher die Bound nicht — das ist genau die
  Eigenschaft, die `use_xtypes=no`-Cross-Vendor-Matching ermöglicht.

Die offene Frage betrifft **nicht** die Byte-FFI (die ist korrekt
type-agnostisch by design), sondern den **typisierten** Pfad: Wenn eine
App ein aus der IDL generiertes Struct mit `sequence<octet, 8192>`
benutzt, lehnt ZeroDDS' Encoder ein Sample mit > 8192 Elementen ab
(spec-korrekt) oder serialisiert es still (Bound-Verletzung)?

## Warum zurückgestellt

Kein Interop-Defekt — die ZeroDDS↔OpenDDS-Interop ist im Typ-Vertrag
(0–8192) **16/16 grün**. Dies ist eine **interne XTypes-Conformance-
Frage** (gehört zum cdr/idl/xtypes-Scope), getrennt von der Interop-
Achse, und vom Byte-FFI-Bench nicht berührt.

## Wann pickup

Nach Durability-Service P5 (User-Entscheidung 2026-06-11).

## Pick-up-Spec

1. **Verifizieren**: Hat der typisierte Encode-Pfad einen Bound-Check?
   - `crates/cdr/` Sequence-Encode (XCDR2): wird die deklarierte
     `max`-Bound gegen `len()` geprüft?
   - `crates/idl/` → Codegen: emittiert der generierte Rust-Typ die
     Bound (z.B. als Konstante / `try_push`-Guard) oder nur ein
     unbounded `Vec`?
2. **Spec-Anker**: DDS-XTypes 1.3 §7.2.2.4.3 (bounded collections),
   §7.4.3 (XCDR-Serialisierung bounded sequence) — out-of-bound ist ein
   Encode-Fehler, kein stiller Truncate/Overflow.
3. **Falls Lücke**: Bound-Check im typisierten Encode-Pfad (Rückgabe
   `EncodeError`, nicht panic), + Unit-Test (encode `len > bound` →
   Err), + ggf. Codegen-Bound-Konstante. Byte-FFI bleibt unverändert
   (type-agnostisch).
4. **Cross-Vendor-Gegenprobe**: ein typisierter ZeroDDS-Writer mit
   `sequence<octet, 8192>` + 16384-Byte-Sample muss denselben
   spec-konformen Reject zeigen wie OpenDDS.

## Referenzen

- `docs/interop/opendds-interop-closeout.md` (Fund-Kontext)
- `tests/perf/dds-roundtrip-bench/roundtrip.idl` (der bounded Typ)
- DDS-XTypes 1.3 §7.2.2.4.3 / §7.4.3
