# dds-dcps-1.4 — offene Punkte

Aggregat aus `dds-dcps-1.4.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2.2.3.20 ENTITY_FACTORY — RxO No

**Status:** `open` — nach dem Code erfüllt: ENTITY_FACTORY steht nicht in `RxoView` (`crates/dcps/src/runtime.rs`) und wird nicht angekündigt; es fehlt ein Test, der zeigt, dass unterschiedliche ENTITY_FACTORY-Werte von Publisher und Subscriber die Zuordnung nicht beeinflussen.
