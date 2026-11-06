# dds4ccm-1.1 — offene Punkte

Aggregat aus `dds4ccm-1.1.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §7.4.2.1 XML File Syntax

**Status:** `partial` — UTF-8, Kommentare, Wurzel `<dds>`, abgeschnittene Leerzeichen und hexadezimale Zahlen sind umgesetzt. Die Boolean-Formen von Table 7.1 fehlen: `parse_bool_strict` nimmt nach DDS-XML 1.0 §7.1.4 nur `true`, `1`, `false`, `0` in dieser Schreibweise; eigenes Item.

## §7.4.2.1 Table 7.1 — Boolean `yes`/`no`/`BOOLEAN_TRUE`/`BOOLEAN_FALSE` ohne Beachtung der Schreibweise

**Status:** `open` — `parse_bool_strict` (`crates/xml/src/qos_parser.rs`) weist `yes`, `no`, `BOOLEAN_TRUE`, `BOOLEAN_FALSE` und `True`, `TRUE` ab (`boolean_case_sensitive_rejected`); der Leser folgt DDS-XML 1.0 §7.1.4.
