# dds-ts-1.0 — offene Punkte

Aggregat aus `dds-ts-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §7.1 Module Mapping

**Status:** `partial` — `export namespace` mit Verschachtelung und Declaration Merging sowie qualifizierte Namen als Punktpfad sind umgesetzt; es fehlt die optionale Ausgabe einer Datei je Modul der obersten Ebene (`--module-per-file`), eigenes Item.

## §7.1 Option `--module-per-file`

**Status:** `open` — weder `tools/idlc` noch `crates/lang-rules` kennen die Option.

## §C.6 Reservation of Future Extensions

**Status:** `open` — nach dem Code erfüllt: `crates/ts-wasm/` und `runtime/typescript/src/` verwenden keinen der vier Bezeichner; `crates/ts-node/src/dds.ts` nutzt `getInstance` für die `DomainParticipantFactory` (gleicher Zweck). Es fehlt ein Test, der die Exporte des WASM-Profils gegen die Liste prüft.

## §D.1 Diagnostic Code Registry

**Status:** `open` — der Baum führt eine stabile Code-Menge, aber eine andere: `crates/idl-parse/src/diagnostic.rs::DiagnosticCode` vergibt `IDL0001` bis `IDL0018`. Die Annex-D-Codes `E001`-`E004`, `W001`-`W004` und `I001` kommen im Baum nicht vor, und kein Test prüft sie.
