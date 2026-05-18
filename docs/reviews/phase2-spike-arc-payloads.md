# Phase-2-Spike: Arc&lt;[u8]&gt;-Payloads (ex P1-6)

**Stand:** 2026-04-20, aus Phase-1-Audit `phase1-rollup.md#P1-6`
defered auf eigenen WP in Phase 2.

## Problem

`rtps::history_cache::CacheChange` haelt die Payload als `Vec<u8>`.
Jeder Writer/Reader/Cache-Insert erzeugt einen `clone()`. Der
Perf-Audit (`phase1-perf-audit.md#F7/F8/F10`) hat die clone-Sites
identifiziert:

- `reliable_writer.rs:231` - Insert ins Cache mit Clone.
- `reliable_writer.rs:270` - Fragment-Resend mit Clone.
- `reliable_writer.rs:288` - Built-Datagram mit Clone.
- `reliable_writer.rs:412/583/655` - HEARTBEAT/Tick-Pfade.
- `reader.rs` + `reliable_reader.rs` - 3× Payload-Copy.

Laut Agent: **30–50% Throughput-Gewinn** fuer Reliable-Writer-Tick
unter Last, wenn `Vec<u8>` durch `Arc<[u8]>` ersetzt wird — Cache und
Datagram teilen sich die Allocation.

## Scope

1. `CacheChange::payload: Vec<u8>` → `Arc<[u8]>`.
2. Writer-API: `write_change(&[u8])` cloned einmal in ein `Arc`,
   danach nur noch `Arc::clone` (refcount-increment).
3. Reader-Path: eingehende Datagrams erhalten `Arc<[u8]>`, gleicher
   Arc landet im Cache.
4. `build_sample_datagrams` / `build_data_frag_datagram` nehmen
   `&Arc<[u8]>` statt `&Vec<u8>`.
5. Tests + Benchmarks.

## Warum nicht in Kurz-Session

- **~40 Call-Sites** ueber 6 Module (history_cache, reliable_writer,
  reader, reliable_reader, writer, SEDP-Cache, Tests).
- **API-Breite**: `write_change` und Cache-Insertion-API aendern
  Signatur → alle Aufrufer muessen mit.
- **Benchmarking notwendig**: ohne vorher/nachher-Messung kann man
  nicht verifizieren, dass die 30-50% realistisch sind.
- **Potentieller Subtlety**: Arc-Mutation-Pfade (wenn jemand die
  Payload im Cache modifizieren wollte) muessen geprueft werden.

## Naechster Schritt

Eigener Task in Phase 2 als **WP 2.0a Zero-Copy-Payload-Spike**:
1. Baseline-Benchmark auf Reliable-Writer-Tick mit aktueller Vec-API.
2. Refactor auf Arc&lt;[u8]&gt;.
3. Same-Benchmark, Delta-Vergleich.
4. PR mit Benchmark-Plot + Delta-Commit.

Aufwand geschaetzt: 1–1.5 Personentage. Blocking: nein (WP 2.1 DCPS
kann parallel starten; Payload-API ist intern zum Protokoll-Crate).
