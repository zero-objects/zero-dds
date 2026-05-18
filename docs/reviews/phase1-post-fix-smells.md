# Phase-1 Post-Fix Smell-Audit

**Datum:** 2026-04-20. Re-Audit der 15 Findings aus
`phase1-smell-audit.md` nach 7 P1-Fixes. `cargo test --workspace`
gruen (500+ Tests, Build clean).

## Status pro Pre-Fix-Finding

| # | Sev | Status | Evidence |
|---|-----|--------|----------|
| 1 | High | Resolved | `reliable_writer.rs:249-254` — `debug_assert_eq!(advanced, Some(sn), ...)` + Begruendungs-Kommentar. |
| 2 | High | Open | 31 `recursion-depth`-Marker in 11 Files; `resolve.rs:253` auf 16, Rest weiter 64. Kein Audit-Pass. |
| 3 | High | Resolved | `rtps/participant_data.rs:40` `pub use zerodds_qos::Duration;`. |
| 4 | High | Resolved | `rtps/publication_data.rs:30,35,41` re-exportiert `DurabilityKind`/`ReliabilityKind`/`ReliabilityQosPolicy`. |
| 5 | Med | Resolved | 21 `#[non_exhaustive]` im Baum, u.a. `rtps::error:7`, `transport::lib:31,68`, `history_cache:99`, `sedp::reader:37`, `spdp:24`, `types::error:9`, `resolve:28`. |
| 6 | Med | Open (akzept.) | `transport-shm/registry.rs:78,94` weiter `if let Ok(...)`. Poisoned-Lock = toter Prozess. |
| 7 | Med | Open (akzept.) | `tcp_transport.rs:297,313` unveraendert, gleiche Policy. |
| 8 | Med | Open | Grep `SilentDowngrade` → 0 Treffer. Vermutlich umformuliert; Re-Check noetig. |
| 9 | Med | Open | `annotations.rs` unveraendert — lenient gegen partielle Annotations ist Phase-1-Design. |
| 10 | Med | Open | `qos/wire_helpers.rs:20-26` akzeptiert weiter Non-Null-Pad. Interop-Risiko. |
| 11 | Med | Open | `type_object/common.rs:444,470` verwirft `len>1` still. |
| 12 | Low | Partial | `assignability.rs:654`, `sedp/reader.rs:354`, 4× `idl/` ohne Tracking-Issue. |
| 13 | Low | Open | `tcp_transport.rs:871` `sink().write_all(b"tickle")`-Stub bleibt. |
| 14 | Low | Resolved | Alle 15 Platzhalter-Crates tragen `publish = false`. |
| 15 | Low | Open (akzept.) | identisch zu #6/#7. |

**Summary:** 5 Resolved, 1 Partial, 9 Open (3 bewusst akzeptiert).

## Neue Findings nach Fixes

| NF | Sev | Beschreibung |
|----|-----|--------------|
| N1 | Low | `CacheChange::alive_arc` (`history_cache:71`) `pub`, aber nur 1 externer Caller (`reliable_writer:234`). Kandidat `pub(crate)` — oder als Zero-Copy-Hook fuer Secure-Writer stabilisieren. |
| N2 | Med | Arc-Pfad endet vor Submessage-Build: `reliable_writer.rs:428,599,671` kopieren `Arc<[u8]>` via `.to_vec()` in `DataSubmessage.serialized_payload: Vec<u8>` (`submessages:416,868`). Perf-Win nur Cache↔Writer, nicht Writer↔Wire. |
| N3 | Low | `CacheChange::alive` alloziert `Arc::from(payload)` je Insert — Best-Effort-Writer/`datagram.rs:206` profitieren nicht, Konsistenz-Riss. |
| N4 | Info | `#[non_exhaustive]`-Welle: interne Matches haben `_ =>` (500+ Tests gruen). Risiko erst bei externem Downstream. |
| N5 | Low | Pre-Fix #8 per Grep nicht reproduzierbar — Re-Verify mit Original-Zeile 41-46. |

## Quick-Wins fuer Phase-2-Start

1. **N2 + Pre-Fix #10 bundeln**: `DataSubmessage` auf `Cow<[u8]>`/`Arc<[u8]>` **und** `read_bool_padded` strict (Pad=0). Wire-Hardening: Perf + Spec-Treue + Interop in einem Paket.
2. **Pre-Fix #2 Marker-Audit**: `zerodds-lint: recursion-depth N` auf realen Schaetzwert je Stelle; Marker wird Contract. Reiner Meta-Edit, ~1h.
3. **Pre-Fix #11 fix**: `read_opt_string`/`read_opt_bytes` bei `len>1` → `Err(DecodeError::ValueOutOfRange)`. Datenverlust-ohne-Diagnostic ist das teuerste offene Finding, ~30 LOC.

## Positives

Drift-Guard-Pattern (#1) uebertragbar auf Reliable-Stack. Re-Export-Konsolidierung (#3/#4) spec-treu ohne Callsite-Churn. `non_exhaustive`-Welle chirurgisch.
