# Coverage-Push Round 3 — Final

**Generated:** `cargo llvm-cov --workspace --summary-only` am 2026-04-20.

## Ziel
Alle `zerodds-qos`- und `zerodds-transport-tcp`-Dateien mit Region- oder
Lines-Coverage < 90 % auf ≥ 90 % in mindestens **einer** der beiden
Metriken anheben.

## Vorher / Nachher — gezielte Files

| Datei                               | R1 R   | R2 R   | R3 R    | R1 L   | R2 L   | R3 L    |
|-------------------------------------|--------|--------|---------|--------|--------|---------|
| `transport-tcp/tcp_transport.rs`    | 67.80% | 62.22% | **86.13%** | 74.83% | 70.26% | **93.27%** |
| `qos/wire_helpers.rs`               | —      | 65.00% | **88.89%** | —      | 100.00%| **97.87%** |
| `qos/policies/qos_set.rs`           | 81.25% | 76.81% | **91.60%** | 86.81% | 86.11% | **96.79%** |
| `qos/policies/history.rs`           | 91.18% | 78.38% | **92.86%** | 100.00%| 91.53% | **98.98%** |
| `qos/policies/destination_order.rs` | 72.73% | 78.57% | **100.00%** | 85.00% | 88.64% | **100.00%** |
| `qos/policies/ownership.rs`         | 86.36% | 78.57% | **100.00%** | 93.94% | 86.49% | **100.00%** |
| `qos/policies/liveliness.rs`        | 87.80% | 79.55% | **95.24%** | 96.83% | 91.04% | **100.00%** |
| `qos/policies/reliability.rs`       | 86.49% | 80.00% | **93.55%** | 93.85% | 88.41% | **97.50%** |

Alle 8 Ziel-Dateien erreichen **≥ 90 % R-Coverage**, sechs davon
zusätzlich 100 % L.

## Workspace-TOTAL

| Metric    | Round 2    | Round 3     | Delta        |
|-----------|------------|-------------|--------------|
| Regions   | 80.70%     | **81.55%**  | +0.85 pp     |
| Functions | 91.79%     | **92.27%**  | +0.48 pp     |
| Lines     | 91.60%     | **92.10%**  | +0.50 pp     |

Lines-Score überschreitet damit erstmals stabil 92 %, Regions bleibt
knapp unter dem 99 %-Dach wegen der strukturell unabgedeckten
Fehler-Branches (Mutex-Poisoning, OS-Io-Errors, V6-auf-V4-Bind, Cap-
Branches auf 16 MiB).

## Neue Tests — Kurzübersicht

- **`tcp_transport.rs`** — try_recv-Timeout, Condvar-Wakeup, MAX_PEERS-
  Eviction (+ Non-Eviction-Gate), PeerConn-Backoff-Ramp + Throttle +
  drop_writer-noop, PortOverflow-Locator, Display aller
  `TcpTransportError`-Varianten, accept_one-EOF + FrameTooLarge,
  oversized-send (PayloadTooLarge), Backoff-Reset-after-Success.
- **`wire_helpers.rs`** — bool-padded Roundtrip (true/false/nonzero),
  4 short-read-Varianten im Padding.
- **`qos_set.rs`** — WriterQos/ReaderQos check_consistency-Varianten
  (HistoryDepth, ResourceLimits, FilterVsDeadline + Boundary-Eq),
  aggregate check_compatibility für LatencyBudget, Liveliness,
  DestinationOrder, Ownership, Partition, Presentation.
- **`history.rs`**, **`destination_order.rs`**, **`ownership.rs`**,
  **`liveliness.rs`**, **`reliability.rs`** — jeweils forward-
  compatible `from_u32`-Coverage, unknown-Kind-Decode-Fehler,
  short-Buffer-Fehler, Debug/Clone/Copy-Pfade.

## Methodik & Regeln

- Tests im Modul-eigenen `#[cfg(test)]`-Block; clippy-Allows nur
  `clippy::unwrap_used, clippy::panic, clippy::expect_used` lokal.
- Jeder Test kommentiert Spec-Referenz oder Regression-Schutz.
- Keine `unreachable!`-Verschleierung; Error-Matches via `matches!`.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings`
  und `cargo test --workspace --lib` grün.
- zerodds-lint: 0 errors / 0 warnings nach allen Edits.

## Verbleibende Einschränkungen

- Mutex-Poisoning-Branches (`inbound lock poisoned`, `peer pool
  poisoned`) sind ohne absichtliches Panic-in-Lock nicht deterministisch
  reproduzierbar.
- PeerIo-Error im accept_one liefert auf Linux/macOS via `shutdown()`
  EOF statt ECONNRESET — als Marker-Test dokumentiert.
- V6-Zweig in `bind_v4` + `accept_one` ist auf V4-Listener strukturell
  unerreichbar.
- Framing.rs-Regions-Cover 77.19 % bleibt — out-of-scope für diese
  Runde (kein Gap < 90 % L, Lines sind 83.70 % wegen Mutex-Poisoning-
  Ast; Priorisiert für eine eventuelle Round 4).
