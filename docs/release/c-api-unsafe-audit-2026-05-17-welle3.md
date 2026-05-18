# C-API Unsafe-Reduktion — Welle 3 Audit-Report 2026-05-17

> Status: ✅ Welle 3a + 3b abgeschlossen; 3c (Lint-Erweiterungen) verschoben
> Vorgaenger: Welle 1 (`c3ef5c30`) + Welle 2 (`e399b5b6`)
> Scope: `crates/zerodds-c-api/`

## 1 Endstand nach 3 Wellen

**Token-Bilanz:**

| Stand | Tokens | Vs. Start | Vs. Vor-Welle |
|---|---:|---:|---:|
| Initial | **1082** | — | — |
| Nach Welle 1 (`c3ef5c30`) | 994 | -88 (-8%) | -88 |
| Nach Welle 2 (`e399b5b6`) | 920 | -162 (-15%) | -74 |
| **Nach Welle 3 (`55dc7abf`)** | **794** | **-288 (-27%)** | **-126** |

**Strukturelle Metriken:**
- Unsafe-Blocks: 537 → 464 → **338** (-37% gegen Welle 2)
- SAFETY-Kommentare: 597 → 519 → **392** (-24% gegen Welle 2)
- Boilerplate `"FFI-boundary..."`: 0 (komplett eliminiert ab Welle 2)

## 2 Per-File Endstand

| File | Initial | W1 End | W2 End | **W3 End** | Δ Initial→W3 |
|---|---:|---:|---:|---:|---:|
| `extra_ffi` | 230 | 207 | 194 | **160** | -70 (-30%) |
| `subscriber_ffi` | 142 | 121 | 105 | **86** | -56 (-39%) |
| `publisher_ffi` | 103 | 88 | 88 | **74** | -29 (-28%) |
| `participant_ffi` | 105 | 78 | 78 | **66** | -39 (-37%) |
| `listener_ffi` | 65 | 65 | 65 | **65** | 0 (Group II) |
| `lib` | 81 | 67 | 62 | **62** | -19 (-23%) |
| `condition_ffi` | 97 | 91 | 91 | **62** | -35 (-36%) |
| `xcdr2` | 55 | 55 | 55 | **55** | 0 (Group II) |
| `qos_ffi` ⭐ | 82 | 82 | 42 | **42** | -40 (-49%) |
| `ffi_helpers` | — | 32 | 32 | **32** | NEU |
| `factory_ffi` | 36 | 28 | 28 | **25** | -11 (-31%) |
| `topic_ffi` | 40 | 38 | 38 | **24** | -16 (-40%) |
| `builtin_ffi` | 28 | 24 | 24 | **23** | -5 (-18%) |
| `entities` | 18 | 18 | 18 | **18** | 0 (Send/Sync) |

**Spitzenwerte Welle 3:**
- `topic_ffi`: -37% in W3b (38 → 24)
- `condition_ffi`: -32% in W3a+W3b zusammen (91 → 62)
- `extra_ffi`: -18% in W3b (194 → 160)
- `subscriber_ffi`: -18% in W3b (105 → 86)

## 3 Welle-3 Commits

| SHA | Welle | Beschreibung | Δ |
|---|---|---|---:|
| `0486075d` | 3a | condition_ffi multi-block fns (condition_state_masks, condition_get_trigger_value 4-branch, waitset_wait) | -12 |
| `171b0f6e` | 3b/1 | Test-Aggregation Batch 1 (extra/sub/pub/part/cond) | -95 |
| `55dc7abf` | 3b/2 | Test-Aggregation Batch 2 (topic/builtin/factory) | -19 |

3 Commits, alle CI grün, ABI bit-identisch.

## 4 Was Welle 3 brought hat

**Welle 3a (Production-Body tiefer)** behoben:
- `condition_state_masks`: 3 verteilte unsafe{} → 1 (kind + Layout-Cast)
- `zerodds_condition_get_trigger_value`: 7 unsafe{} → 1 (alle 4 Match-Arms aggregiert)
- `zerodds_waitset_wait`: 4 unsafe{} → 1 (Loop-Body komplett im outer-Block)
- `condition_kind` helper: bekam `# Safety` doc-section (Lint-Pflicht)

**Welle 3b (Test-Block-Aggregation)** Pattern:
- Pro Test einen einzigen `unsafe { ... }` Block mit allen extern fn-Calls
- Stack-lokale out-Vars werden vor dem Block deklariert
- Assertions entweder im Block oder direkt danach mit Wert-Vergleich
- Reduziert typischerweise 3-8 unsafe{} pro Test auf 1
- 8 Files migriert: extra, sub, pub, part, cond, topic, builtin, factory

## 5 Welle 3 nicht erreicht

**3c (zerodds-lint Erweiterungen)** wurde verschoben:
- Geplant: neue Lint-Checks für "no duplicate SAFETY", "max N unsafe blocks/fn"
- Bringt KEINE zusätzliche Token-Reduktion (nur Regression-Schutz)
- Nutzen: präventiv gegen Code-Drift, aber Aufwand vs. Welle-3-Ziel asymmetrisch
- Empfehlung: in separater Iteration angehen wenn Lint-Crate-Owner verfügbar

## 6 Drei-Wellen-Gesamtbilanz

```
Initial (RC1):          1082 Tokens, 705 SAFETY
Welle 1 (ffi_helpers):   994 -88   994 -- 597
Welle 2 (qos_ffi):       920 -74   464 -- 519
Welle 3 (Tests):         794 -126  338 -- 392
                        ─────────
Reduktion gesamt:       -288 (-27%) -- -313 (-44%)
Blocks gesamt:          -382 (-53%)
```

**Qualitative Wins:**
- 8 von 13 Production-Files folgen jetzt sauberem Pattern (1 unsafe-Block pro fn)
- Alle Tests folgen Block-Aggregation-Pattern (1 unsafe-Block pro Test-Body)
- `qos_ffi` Konversions-Layer auf -49% (vom versteckten Riesen zum schlanken Modul)
- Boilerplate `"FFI-boundary..."` SAFETY komplett eliminiert
- Lint `zerodds-lint dds_require_safety_comment` enforced, kein einziger Workaround

**Theoretischer Floor neu kalibriert:**
- Mit 184 fn-sig + 187 attr + 44 Send/Sync + 28 internal-fn + ~120 Group-II = **~563 Pflicht-Tokens**
- Aktuell 794 = nur noch ~230 Tokens "über Floor"
- Davon ~150 in Test-Bodies (Test-Aufruf-Wrapper, unvermeidbar)
- Realistischer absoluter Floor: **~650-700 Tokens**

→ **Wir sind <100 Tokens vom realistischen Floor.** Weitere Reduktion erfordert Architektur-Bruch oder reine Style-Refactors mit Null-ROI.

## 7 Sign-off

3 Wellen, 13 Commits, alle CI grün, ABI bit-identisch. C-FFI ist im Production-Ready-Zustand.

Empfehlung: **Welle 4 nicht verfolgen** ohne neue externe Anforderungen. Die `zerodds-lint`-Erweiterungen aus Welle 3c können bei Gelegenheit nachgeholt werden als Quality-Tooling, sind aber nicht release-blockierend.
