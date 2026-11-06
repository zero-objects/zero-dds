# dds-psm-cxx-1.0 — offene Punkte

Aggregat aus `dds-psm-cxx-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §7.3 Konstruktoren und Kopierzuweisung von Werttypen ausnahmesicher

**Status:** `open` — eine Empfehlung („preferably“), die das Produkt erfüllen kann. `Value<D>` (`crates/cpp/include/dds/core/Value.hpp`) kopiert den Delegate mit seiner Kopierzuweisung; die Policy-Klassen halten `std::vector`, `std::string` und Zahlen. Kein Test lässt eine Kopie scheitern, und die Dokumentation von `crates/cpp` nennt keine Abweichung.
