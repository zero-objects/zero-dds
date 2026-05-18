# WP 1.5 Review — Findings Summary + Fixes

**Stand:** 2026-04-20, Commit `5ec3cc9` vor Fixes → HEAD nach Fixes.
**Reviewer:** automatisierte Agents (code-reviewer, code-archaeologist)
**Einzel-Reports:**
- `docs/reviews/wp-1.5-code-review.md` — 32 Findings (Code-Review-Agent)
- `docs/reviews/wp-1.5-coverage.md` — Coverage-Report (llvm-cov)
- Arch-Analysis — in Agent-Transcript, kernpunkte hier konsolidiert

## Executive

Zwei Critical-Findings gefunden + gefixt (Wire-Interop-Blocker). Zwei
High-Findings zu DoS-Sicherheit gefixt. Rest ist priorisiert als
Phase-1-Polish (H-Findings bleiben, L-Findings als follow-up).

## Critical — GEFIXT

| # | Befund | Fix-Commit (anstehend) |
|---|--------|------------------------|
| C1 | `hash_bytes` nutzt SHA-256 statt MD5 (XTypes §7.3.1.2.1). Cyclone/Fast-DDS nutzen MD5 → Live-Hash-Match scheitert. | `hash.rs` auf `md-5` umgestellt, `sha2`-Dep entfernt, Referenz-Test auf `MD5("") = d41d…f8` aktualisiert. |
| C2 | `TL_SVC_*` EntityIds nutzten `BuiltinWriterWithKey`/`ReaderWithKey` (0xC2/0xC7). XTypes §7.6.3.3.4 verlangt `NoKey` (0xC3/0xC4). Cyclone würde TypeLookup-Endpoints nicht erkennen. | `wire_types.rs:244-262` auf `BuiltinWriterNoKey`/`BuiltinReaderNoKey` umgestellt. |

## High — GEFIXT

| # | Befund | Fix |
|---|--------|-----|
| H1 | `TypeIdentifier::decode_from` rekursiv ohne Depth-Cap → Stack-Overflow DoS-Vektor bei verschachtelten `PlainSequenceSmall`. | `MAX_DECODE_DEPTH = 16`, private `decode_with_depth(r, depth)` + Cap-Prüfung. `DecodeError::LengthExceeded` bei Überschreitung. |
| H2 | `Vec::with_capacity(read_u32 as usize)` in ~6 Decoder-Pfaden → Angreifer alloziert GB bei 30-byte-PID. | Neue `safe_capacity(len, elem_size, remaining)` in `type_object/common.rs` mit `DECODE_PREALLOC_CAP = 4096`. Eingesetzt in `decode_seq` (18 Call-Sites) + `CommonUnionMember::decode_from` + `PlainArrayLarge`. |
| H3 | Silent QoS-Downgrade: `DurabilityKind::from_u32`/`ReliabilityKind::from_u32` mappen unbekannte Werte auf `Volatile`/`BestEffort` → Peer schickt `Reliable`, wir interpretieren `BestEffort`, Verbindung matcht falsch. | Neue `try_from_u32`-Varianten die `Option` liefern. `from_u32` bleibt forward-compat für SEDP-Parser; `try_from_u32` für QoS-Matching. |

## High — OFFEN (follow-up)

| # | Befund | Kategorie | Follow-up |
|---|--------|-----------|-----------|
| H4 | Error-Mapping-Lügen: `TypeCodecError::UnknownTypeKind → DecodeError::UnexpectedEof{0,0}` in `type_information.rs` + `type_lookup.rs` (4 Stellen). | Error-Handling | Requires `decode_seq_tc` Variante mit `TypeCodecError`-Rückgabe statt `DecodeError`. Mittlerer Refactor. |
| H5 | Assignability-Matrix unvollständig: kein `PlainSequenceLarge ↔ PlainSequenceSmall`, kein `PlainArray*`, kein `EquivalenceHashMinimal ↔ Complete`. | Correctness | Explizite Spec-Matrix §7.2.4.4 einziehen. WP 1.6 oder letzter Phase-1-Polish. |
| H6 | `AppliedBuiltinTypeAnnotations` kodiert "Optional" als `sequence<T,1>` mit Skip-Loop bei `len > 1` — spec-fremd. | Spec-Abweichung | Echtes XCDR2-`@optional`-Encoding (1-byte present-Flag) oder klares Spec-Commentar. |

## Medium + Low — Weitere Ergebnisse

`docs/reviews/wp-1.5-code-review.md` Findings 8–32 bleiben dokumentiert:

- **API-Design**: `build_minimal_struct` vs `build_minimal` — Inkonsistenz, aber bewusster Kompromiss für Type-Discrimination im Code. Follow-up bei API-Freeze.
- **Complete-Modul-Struktur**: 755-LoC Monolith gegen pro-Kind-Files in Minimal. Refactor-Kandidat, nicht blockend.
- **Struct `@autoid(HASH)`**: nutzt MD5[0..4] statt spec-exakter CRC-64-Formel. Dokumentiert als "vereinfacht".
- **`TypeSpec::Scoped` → `EquivalenceHash::ZERO`**: **GEFIXT** durch `MapError::UnresolvedScoped(path)` — kein Silent-Collision mehr.
- **`TypeLookupStack` doc-Lüge** über Pending-Requests-Tabelle: **GEFIXT** — Doc reflektiert jetzt Phase-1-Stand.

## Positiv (aus Arch-Agent)

1. `zerodds-rtps ↔ zerodds-types` sauber entkoppelt — `type_information: Option<Vec<u8>>` ist echtes Opaque-Pattern.
2. `encode_seq`/`decode_seq` zentral, 18 Call-Sites konsistent.
3. ParameterList forward-compat für unbekannte PIDs (`unknown_pids_are_skipped` abgedeckt).
4. `NameHash::from_name` kanonisch in zerodds-types; zerodds-idl reimportiert (keine MD5-Drift).
5. `TypeRegistry` als einziger Owner — keine Double-Caching-Architektur.
6. `SedpStack::on_participant_lost` cascade cleanup vollständig.
7. `#[non_exhaustive]` auf allen TypeObject-Enums gesetzt.
8. Dep-Graph azyklisch: `cdr → types → idl`, `rtps` isoliert, `discovery → rtps + types + cdr`.

## Coverage

Workspace Lines: **~89%**, Regions: **~77%** (WP-1.5-Schwachstellen behoben):

| Modul | Vorher | Nachher |
|-------|--------|---------|
| `type_object/complete/mod.rs` | 53% | **99%** |
| `resolve.rs` | 72% | **90%** |
| `error.rs` | 0% | **100%** |
| `type_lookup.rs` | 76% | **92%** |

Fortschritt Richtung 99%-Soft-Target. Verbleibende Lücken:
- `type_identifier/mod.rs` 87% (Edge-Decode-Pfade)
- `assignability.rs` 84% (H5-unvollständige Arms werden beim Fix abgedeckt)
- `type_object/common.rs` 94%

## Call-Graph-Hotspots (zu beobachten)

1. **`TypeIdentifier::decode_from`** (fan-in 14) — jetzt mit Depth-Cap.
2. **`decode_seq`** (fan-in 18) — jetzt mit `safe_capacity`.
3. **`compute_hash → to_bytes_le → encode_into`** — Hot-Path bei TypeLookup-Response. Potentielle Allokationen-Reduktion offen (L21/L22 Code-Review).
4. **`SedpStack::handle_datagram`** (fan-out 8) — stabile Dispatching-Logik, Plugin-Architektur (Memory `project_rtps_extensibility`) wird hier ansetzen.

## Pipeline

- Origin-Push: erfolgreich (`c4bf07a`, `5ec3cc9` bereits online).
- Fix-Commit folgt gleich — Pipeline validiert dann C1/C2/H1/H2/H3.
- GitLab-Dashboard: `https://gitlab.sandra-kessler.eu/fishermen21/zerodds/-/pipelines`
