# WP 1.7 / 1.8 Test-Coverage Report

**Generated:** `cargo llvm-cov --workspace --summary-only` am 2026-04-20

## Workspace-Total vs. WP 1.5

| Metric    | WP 1.5     | WP 1.7/1.8 | Delta       |
|-----------|------------|------------|-------------|
| Regions   | 77.08%     | **80.76%** | **+3.68 pp** |
| Functions | 90.13%     | **92.01%** | **+1.88 pp** |
| Lines     | 89.30%     | **91.79%** | **+2.49 pp** |
| Branches  | n/a        | –          | –           |

Die zwei neuen Crates heben die Workspace-Regions-Quote signifikant,
weil `zerodds-qos` breit getestet ist (Durchschnitt ~85%+ Regions) und
grosse Neu-Codeflaechen mit ~95% Lines einbringt.

## zerodds-qos — Per-File

| Datei                                   | Regions | Functions | Lines   |
|-----------------------------------------|---------|-----------|---------|
| `compatibility.rs`                      | 100%    | 100%      | 100%    |
| `defaults.rs`                           | 100%    | 100%      | 100%    |
| `duration.rs`                           | 93.33%  | 100%      | 100%    |
| `policies/data_lifecycle.rs`            | 79.59%  | 100%      | 100%    |
| `policies/deadline.rs`                  | 88.89%  | 100%      | 100%    |
| `policies/destination_order.rs`         | 72.73%  | 85.71%    | 85.00%  |
| `policies/durability.rs`                | 92.31%  | 100%      | 96.88%  |
| `policies/durability_service.rs`        | 73.81%  | 100%      | 100%    |
| `policies/entity_factory.rs`            | 77.42%  | 100%      | 100%    |
| `policies/generic_data.rs`              | 78.95%  | 91.67%    | 95.45%  |
| `policies/history.rs`                   | 91.18%  | 100%      | 100%    |
| `policies/latency_budget.rs`            | 87.50%  | 100%      | 100%    |
| `policies/lifespan.rs`                  | 88.89%  | 100%      | 100%    |
| `policies/liveliness.rs`                | 87.80%  | 100%      | 96.83%  |
| `policies/ownership.rs`                 | 86.36%  | 100%      | 93.94%  |
| `policies/ownership_strength.rs`        | 90.00%  | 100%      | 100%    |
| `policies/partition.rs`                 | 81.82%  | 87.50%    | 94.12%  |
| `policies/presentation.rs`              | 83.05%  | 100%      | 98.81%  |
| `policies/qos_set.rs`                   | 81.25%  | 100%      | 86.81%  |
| `policies/reliability.rs`               | 86.49%  | 90.00%    | 93.85%  |
| `policies/resource_limits.rs`           | 86.49%  | 100%      | 100%    |
| `policies/time_based_filter.rs`         | 87.50%  | 100%      | 100%    |
| `policies/transport_priority.rs`        | 87.50%  | 100%      | 100%    |

## zerodds-transport-tcp — Per-File

| Datei                 | Regions | Functions | Lines   |
|-----------------------|---------|-----------|---------|
| `framing.rs`          | 75.44%  | 83.33%    | 82.61%  |
| `tcp_transport.rs`    | 67.80%  | 68.42%    | 74.83%  |

## zerodds-rtps/qos_bridge.rs (neu)

| Datei           | Regions | Functions | Lines   |
|-----------------|---------|-----------|---------|
| `qos_bridge.rs` | 97.50%  | 92.86%    | 95.59%  |

Qos-Bridge ist nahe am 99%-Target.

## TOP-5 Coverage-Gaps zerodds-qos

1. `destination_order.rs` (72.73% R, 1 Fn untested) — unknown-Kind-
   Dispatch beim Decode nicht abgedeckt.
2. `durability_service.rs` (73.81% R) — `cleanup_delay=INFINITE` / history-
   kind-Mismatch-Pfade fehlen.
3. `entity_factory.rs` (77.42% R) — non-default autoenable-Roundtrip
   fehlt.
4. `generic_data.rs` (78.95% R, 1 Fn untested) — leeres `GroupData`
   Decode-Pfad.
5. `data_lifecycle.rs` (79.59% R) — writer-vs-reader Varianten-Switch.

## TOP-5 Coverage-Gaps zerodds-transport-tcp

1. `tcp_transport.rs::send` — Fehlerpfad `peers`-Mutex-poisoned und
   `PeerConn::send` I/O-Error.
2. `tcp_transport.rs::accept_one` — IPv6-SocketAddr-Rejection-Zweig.
3. `tcp_transport.rs::bind_v4` — V6-local-addr-Fehlerzweig.
4. `tcp_transport.rs::set_accept_timeout` — `Some(d)` vs. `None`
   Branches beide fehlen Assertions.
5. `framing.rs::read_frame` — `Io`-Fehlerklasse (nicht EOF) bei mid-frame-
   Read; oversized-Frame-Reject im Reader (nicht nur Writer).

## Empfohlene Tests (1-Zeiler)

zerodds-qos:
- test decode of `DestinationOrder` with unknown kind value returns error
- test roundtrip of `DurabilityService` with `cleanup_delay = INFINITE`
- test decode of `EntityFactory` with `autoenable_created_entities = false`
- test decode of `GroupData` with empty payload
- test writer-variant decode of `DataLifecycle` vs reader-variant

zerodds-transport-tcp:
- test `send` to un-connectable address returns `SendError::Io`
- test `accept_one` rejects incoming IPv6 peer with `UnsupportedLocator`
- test `bind_v4` returns `Bind` error on already-bound port
- test `read_frame` returns `Io` error when the underlying reader errors
  mid-payload (not EOF)
- test `read_frame` rejects an oversized announced length (> 64 MiB)
  on the read side

zerodds-rtps/qos_bridge.rs:
- test bridge of unknown QoS PID is ignored/roundtrips to empty
