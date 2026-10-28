# dds-ts-1.0 — offene Punkte

Aggregat aus `dds-ts-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §D.1 Diagnostic Code Registry

**Status:** `open` — der Baum führt eine stabile Code-Menge, aber eine andere: `crates/idl-parse/src/diagnostic.rs::DiagnosticCode` vergibt `IDL0001` bis `IDL0018`. Die Annex-D-Codes `E001`-`E004`, `W001`-`W004` und `I001` kommen im Baum nicht vor, und kein Test prüft sie.
