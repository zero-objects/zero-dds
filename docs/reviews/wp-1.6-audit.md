# WP 1.6 — Type Compatibility: Pre-Work Audit

**Datum:** 2026-04-20. **Scope:** Gap-Analyse vor Umsetzung.
**Ziel:** Vollstaendige XTypes-1.3-§7.2.4-Type-Assignability mit
`TypeConsistencyEnforcement`-Integration und Writer↔Reader-Matching.

## Bestand in `crates/types/src/assignability.rs`

- `AssignabilityConfig { allow_type_coercion: bool, max_depth: usize }`.
- `is_assignable(w, r, registry, cfg) -> Assignable::{Yes, No(&str)}`.
- Rules gelten fuer:
  - Primitive (Identitaet; Widening nur mit `allow_type_coercion`).
  - String8/16 (alle Varianten untereinander).
  - `PlainSequenceSmall` (Element + Bound-Check).
  - `EquivalenceHashMinimal` via Registry + `check_minimal_types`.
  - Minimal-Struct mit Extensibility-Trichotomie (Final/Mutable/Appendable).

## Gaps gegen XTypes §7.2.4

| Bereich | Gap | WP-Task |
|---|---|---|
| **TCE-Flags** | Nur `allow_type_coercion` wird respektiert. `ignore_sequence_bounds`, `ignore_string_bounds`, `ignore_member_names`, `force_type_validation`, `prevent_type_widening` fehlen. | T3 |
| **Collections** | `PlainSequenceLarge`, `PlainArray{Small,Large}`, `PlainMap{Small,Large}` ohne Assignability-Pfad. | T3 |
| **Enums** | Enum ↔ Enum Literal-Assignability nicht implementiert. | T3 |
| **Unions** | Union ↔ Union Case-Label + Discriminator nicht geprueft. | T3 |
| **Complete-TypeObjects** | Registry-Lookup vergleicht nur Minimal. | T3 |
| **Matching-API** | Keine oeffentliche `TypeMatcher`-Schnittstelle, die TCE-Policy entgegennimmt. | T2 |
| **SEDP-Integration** | `discovery::sedp` prueft nicht auf Type-Assignability beim Endpoint-Match. | T4 |
| **Referenz-Vektoren** | Keine Cyclone/Fast-DDS-Cross-Check-Cases; nur eigene Roundtrips. | T5 |

## Umsetzungs-Strategie

1. **T2 zuerst** — `TypeMatcher`-API als duenne Facade um `is_assignable`,
   die `TypeConsistencyEnforcement` nimmt und in den Config uebersetzt.
2. **T3** — TCE-Flags tatsaechlich in die Rules: Sequence/String-Bound
   per `ignore_*` skippen; Member-Names per `ignore_member_names`
   weglassen; `force_type_validation` erzwingt strikten Pfad auch
   wenn TCE "Kind=AllowTypeCoercion" sagt.
3. **T4** — `SedpStack::match_endpoints()` nutzt `TypeMatcher` +
   `zerodds_qos::check_compatibility` fuer die Matching-Entscheidung.
4. **T5** — Golden-Vektoren, hauptsaechlich negative Cases (der positive
   Pfad ist durch Unit-Tests gedeckt).

## Nicht in Scope (explizit deferred)

- **Type-Evolution-Regeln** (Version-Drift mit added/removed Members
  auf Mutable-Strukturen) — das ist Stretch fuer WP 2.x DCPS.
- **XCDR1-Assignability** — aktuell nur XCDR2 gepflegt; XCDR1 kommt
  als Compat-Layer in WP 2.5.

## Exit-Kriterien fuer WP 1.6

- [x] `TypeConsistencyEnforcement` durchgereicht (alle 5 Flags wirken).
- [x] `PlainArray`/`PlainMap`/`Enum`/`Union` haben Assignability-Pfad.
- [x] `TypeMatcher` als oeffentliche Facade in `zerodds-types`.
- [x] `SedpStack` nutzt `TypeMatcher`.
- [x] Mindestens 10 Cross-Impl-Testvektoren (pos/neg gemischt).
- [x] `cargo test --workspace` + `clippy -D warnings` + `zerodds-lint` gruen.
