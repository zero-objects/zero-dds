# WP 1.6 Code Review

**Datum:** 2026-04-20. **Verdict:** Good — 2 High, 6 Medium, 7 Low.
**Merge-Blocker:** Keine, aber 3 High-Priority-Fixes vor WP 1.7-Abschluss
empfohlen.

## Findings

| # | Sev | Area | Finding |
|---|-----|------|---------|
| 1 | High | Spec | `ignore_member_names` plumbed durch `TypeMatcher` in `AssignabilityConfig`, aber **nirgends gelesen**. Audit-Exit-Kriterium "alle 5 Flags wirken" unerfuellt. |
| 2 | High | Coverage | Keine Cross-Encoding-Arme: `PlainSequenceSmall↔Large`, `PlainArraySmall↔Large`, `PlainMapSmall↔Large`. Small/Large ist nur Wire-Detail; Writer-Large bound=300 ↔ Reader-Small bound=200 faellt aktuell auf "kinds do not match" zurueck. Fuer Strings existiert der Cross-Arm bereits. |
| 3 | Med | Lesbarkeit | `assignability.rs:137–138` Kommentar sagt "Small↔Small/Large, Large↔Large", Code deckt nur Small↔Small + Large↔Large. Kommentar irrefuehrend (Grund fuer #2). |
| 4 | Med | Spec | Primitive-Widening-Matrix asymmetrisch: `UInt8→Int*` ja, aber `UInt8→UInt16/UInt32/UInt64` fehlt. Cyclone akzeptiert same-sign Widening. |
| 5 | Med | Test-Quality | Kein Test fuer `ForceTypeValidation` ∧ `ignore_sequence_bounds=true`. Per §7.6.3.7.1 sollte ForceValidation die ignore-Flags ueberschreiben — aktuell tut `build_config` das nicht. Spec-Interpretation ist unkommentiert. |
| 6 | Med | Test-Coverage | `type_matcher_vectors.rs` hat **keine** Struct-Level-Vektoren (kein `EquivalenceHashMinimal`, keine Final/Appendable/Mutable via `TypeMatcher`). Struct-Goldens wuerden #1 direkt auffangen. |
| 7 | Med | API | `MatchInputs` mit 6 Ref-Feldern unhandlich; 4 sind praktisch optional. Builder-Pattern oder `Default`-`MatchOptions` vor WP 2.1 DCPS. |
| 8 | Med | API | `TypeMatcher::new(&tce)` erzwingt ref; kein `default_tce()`-Konstruktor. In `type_matcher_vectors.rs` wird das Muster 9x wiederholt. |
| 9 | Med | Semantik | `endpoint_match.rs:96–100`: wenn **eine** Seite TypeIdentifier und die andere `None` liefert, wird ohne Kommentar auf `type_name`-String gefallen. Wuerde legitime Matches ablehnen; entweder Kommentar oder Spec-Regel definieren. |
| 10 | Low | Smell | `TypeMatchResult` dupliziert `Assignable`. Lossless-Conversion, aber zwei Typen; `pub use Assignable as TypeMatchResult` oder `From`-Impl statt private `from_assignable`-Fn. |
| 11 | Low | Tests | `string8_vs_string16_mismatch`, `float_int_cross_type_mismatch` pruefen nur `!is_match()`, nicht den Reason. Typo-Insensitiv. |
| 12 | Low | Doku | `ignore_sequence_bounds` gilt nicht fuer Arrays — aktuell nur im Kommentar; sollte in rustdoc von `AssignabilityConfig`. |
| 13 | Low | Struktur | `type_matcher_vectors.rs` wird als `#[cfg(test)] mod` eingebunden — OK, aber per Konvention in `#[cfg(test)]`-Block am File-Level; check lib.rs. |
| 14 | Low | API | `Reason::TypeMismatch { detail: &'static str }` waehrend `TopicMismatch/QosMismatch` reichere Owned-Typen nutzen. Inkonsistent. |
| 15 | Low | Audit | `docs/reviews/wp-1.6-audit.md` Checkboxen all `[x]` — #1 und #2 sind nicht erfuellt. |

## Empfohlene Fixes vor WP 1.7-Abschluss

1. `ignore_member_names` wirklich in Mutable/Appendable verdrahten, oder Feld entfernen + Deviation dokumentieren.
2. Fehlende Small↔Large-Arme fuer Sequence/Array/Map + Struct-Level-Golden-Vektor.
3. UInt-Widening-Matrix korrigieren oder auf signed beschraenken + dokumentieren.

## Positives

- `force_type_validation`-Praezedenz in `type_matcher.rs:97–111` spec-konform zu §7.6.3.7.1.
- Drei-Werte-`StructExt`-Enum fix corner-case "no flag bits = Appendable".
- Enum-Assignability (Writer ⊆ Reader Literals, bit_bound-Equality) matcht §7.2.4.4.4.3.
- Alias-Resolution mit eigenem Reason-String je Seite erleichtert Debug.
