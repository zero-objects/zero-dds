# DDS-PSM-Cxx 1.0 — Open Items

Stand 2026-05-07 nach Layer-6-Vollaudit.

— **keine offenen Items.**

Total 122 Items: 104 done + 18 n/a.

## n/a-Klassifikation

Alle 18 n/a-Items sind PROCESS.md-§4.4-konform `n/a (informative)` —
Spec-eigene non-binding Aussagen (Notations-Konvention der Spec,
non-normative Querverweise auf andere Specs). Keine
`n/a (rejected)`-Decision-Records erforderlich.

## Cross-Reference

C-FFI-Wrapper-Pfad in `crates/cpp/include/dds/` ist eine **alternative
spec-konforme Implementations-Variante** parallel zum Codegen-Pfad
(via `idl-cpp` Templates). Audit dieser Wrapper-Schicht: siehe
`zerodds-c-api-1.0.md` + `zerodds-listener-callbacks-1.0.md`.
