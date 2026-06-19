# Spec-Completeness Re-Audit — 2026-06-11

**Anlass:** Programm „A+B+C+E+F2" über 13 Spec-Coverage-`.open.md`-Aggregate.
**Zentrales Finding:** Die `.open.md`-Aggregate sind **repo-weit systematisch
stale** — sie werden „vor jedem Audit-Lauf neu generiert", aber das passiert
nicht automatisch, also driften sie gegenüber dem Code/Haupt-Doc. Code-Audit
ergab: die **meisten gelisteten „Opens" sind längst implementiert + getestet**.
Methode: pro Item gegen den tatsächlichen Code + Test-Suite verifiziert, nicht
gegen das Aggregat.

## Ergebnis pro Punkt (CODE-verifiziert)

| Punkt | `.open.md` sagte | CODE-Realität |
|---|---|---|
| **A1** dds-amqp SASL-Code | partial | ✅ done (`SaslCode::Auth`, kein `UnsupportedMechanism`) |
| **A2** mqtt Property-Decode | partial | ✅ done (`decode_*` in `properties.rs`) |
| **A3** mqtt Keep-Alive | partial | ✅ done (`keep_alive.rs`) |
| **B1** amqp Composite-Codec | partial | ✅ done (per-Performative-Composite) |
| **B2** idl-cpp wstring | open | ✅ **wstring done + e2e (2026-06-11)**; nested/struct/enum/map/array = echte WP |
| **C1** async Phase-2 | „Phase-2 offen" | ✅ Waker + Backpressure + tokio-glue + proptest + cyclone-e2e gelandet; Suite grün (Live-e2e env-gated) |
| **C2** flatdata Phase-2 | „Phase-2 offen" | ✅ derive-Macro + posix-mmap + reader_mask + type_hash; 49 Tests grün (Live-e2e env-gated) |
| **D** idl-4.2 long-double | partial BLOCKED | ⛔ BLOCKED auf Rust-`f128` (~2027), Memory `project_idl_longdouble_blocked` |
| **E** zerodds-py ROS-2-pytest | partial | ⛔ **env-blocked** — codepit hat KEIN ROS-2; braucht ROS-2-CI-Image + `rmw_zerodds_shim` |
| **F1** ros2-rmw/-bridge Rejects | 3+2 rejected | ✅ architektur-korrekt (Layer über rmw) — bleiben rejected (zu bestätigen) |
| **F2c** listener-callbacks DRs | 5 DRs | ⚖️ bewusste Alternativ-Impls; Reconsider gegen Spec-Vollständigkeits-Doktrin |
| **F2d** coap OSCORE/DTLS | 2 rejected | ⚠️ **OSCORE echt offen** (kein COSE-Crate); DTLS blocked (pure-Rust-DTLS 2026 nicht audit-ready) |

## Genuine verbleibende Code-Arbeit (nach Re-Audit)

Nur noch **zwei** echte offene Code-WPs (massiv reduziert von der 13-Spec-Liste):

1. **B2-Rest: idl-cpp-XCDR2-Encoder vervollständigen** — nested seq/struct, enum,
   map, array. Braucht Emitter-Type-Registry (Scoped→enum/struct) + per-struct
   encode-into + Rekursion.
2. **F2d: coap OSCORE** (RFC 8613, COSE-Stack) — optionales Spec-Profil als
   Differenzierung (Spec-Vollständigkeits-Doktrin). DTLS bleibt blocked.

## Blocker (kein Code möglich, dokumentiert)

- **D** idl-4.2 long-double → Rust-`f128` (~2027).
- **E** zerodds-py ROS-2-pytest → ROS-2-CI-Environment + `rmw_zerodds_shim`.
- **F2d-DTLS** → reifer pure-Rust-DTLS-Stack.

## Decisions (User-Klärung)

- **F1** ros2-Rejects — architektur-korrekt, sollten rejected bleiben (User bestätigt).
- **F2c** listener-callbacks — sind die 5 Alternativ-Impls (Polling py/ts, Multi-Bind
  statt Bubble-Up) als spec-konform akzeptiert, oder aktive Callbacks gewünscht?

## Empfehlung (Hygiene)

Die stale `.open.md`-Aggregate sollten **regeneriert** werden (wie hier für
async/flatdata/amqp/dds-amqp/mqtt geschehen), damit die Open-Liste die
Code-Realität widerspiegelt statt einen veralteten Audit-Stand.
