# DDS-Java-PSM 1.0 — Open Items

Stand 2026-05-07 nach Layer-6-Vollaudit.

— **keine offenen Items.**

Total 171 Items: 156 done + 15 n/a.

## n/a-Klassifikation

Alle 15 n/a-Items sind PROCESS.md-§4.4-konform `n/a (informative)` —
Glossar-Definitionen aus der Java-Plattform, non-normative
Querverweise (z.B. JMS-Vergleich), Meta-Aussagen zur Spec-Gliederung.
Keine `n/a (rejected)`-Decision-Records erforderlich.

## Cross-Reference

ZeroDDS realisiert das Java-PSM als **Pure-Java**-Implementation
(`zerodds-java-omgdds`) — kein JNI, keine `libzerodds`-Native-Lib
auf der Java-Seite. Eine fruehere JNI-Bridge (`crates/zerodds-java-
jni/`) wurde am 2026-05-07 (Commit `49b9b4c6`) entfernt. Vendor-Spec:
`docs/specs/zerodds-java-omgdds-1.0.md`. Audit der Pure-Java-
Surface ist out-of-scope dieses OMG-Audit-Files (separate Vendor-
Spec-Coverage falls erforderlich).
