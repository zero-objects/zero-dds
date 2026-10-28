# zerodds-py-1.0 — offene Punkte

Aggregat aus `zerodds-py-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §3.1 Mapping

**Status:** `partial` — die Elementzahl eines festen Arrays prüft der Encoder nicht: zu viele Elemente werden gekürzt, zu wenige werfen `IndexError` (`test_fixed_array_count_is_refused_on_encode`, `xfail(strict=True)`).

## §5.3 Cross-Vendor Python ↔ andere Sprachen

**Status:** `partial` — die Byte-Identität Python ↔ Rust ist im Baum gemessen. Für C++, Java und C# gibt es keine Messung im Repo: die Interop-Matrix, die diese Spalten trägt, liegt auf dem Bench-Host und ist nicht Teil dieses Baums. Rest als §5.3b geführt.

## §5.3b Byte-Identität Python ↔ C++/Java/C# im Baum

**Status:** `open` — es gibt keinen Testfall im Repo, der die vom Python-Codec geschriebenen ShapeType-Oktetten gegen die der C++-, Java- oder C#-Bindung hält. `shape_reference_octets` liefert diese Referenz bisher nur für Rust. Plan: einen Golden-Vektor je Bindung wie in `endpoints/python/test_byte_identity.py` (dort für `SensorReading`) auch für `ShapeType` erzeugen und die vier Codecs gegen denselben Vektor halten.
