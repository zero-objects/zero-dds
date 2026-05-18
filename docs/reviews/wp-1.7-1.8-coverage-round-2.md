# WP 1.7 / 1.8 Test-Coverage Report — Round 2

**Generated:** `cargo llvm-cov --workspace --summary-only` am 2026-04-20

## Workspace-TOTAL — Round 1 vs. Round 2

| Metric    | Round 1    | Round 2    | Delta       |
|-----------|------------|------------|-------------|
| Regions   | 80.76%     | **80.70%** | -0.06 pp    |
| Functions | 92.01%     | **91.79%** | -0.22 pp    |
| Lines     | 91.79%     | **91.60%** | -0.19 pp    |

Marginaler Rückgang: zusätzliche Edge-Case-Tests haben die abgedeckten
Pfade leicht erhöht, aber gleichzeitig wurde neuer uncoverter Code in
`qos_set.rs`, `partition.rs`, `history.rs`, `qos_bridge.rs`,
`compatibility.rs` und `wire_helpers.rs` eingezogen (neue Varianten /
Validierungen), die die Prozent-Quoten um einige Zehntel drücken.

## Side-by-Side — geänderte Dateien

| Datei                                   | R1 Reg   | R2 Reg   | Delta   | R1 Lines | R2 Lines |
|-----------------------------------------|----------|----------|---------|----------|----------|
| `qos/compatibility.rs`                  | 100.00%  | 98.61%   | -1.39   | 100.00%  | 99.47%   |
| `qos/duration.rs`                       | 93.33%   | 93.33%   |  0.00   | 100.00%  | 100.00%  |
| `qos/policies/data_lifecycle.rs`        | 79.59%   | 86.67%   | **+7.08** | 100.00% | 100.00% |
| `qos/policies/deadline.rs`              | 88.89%   | 88.89%   |  0.00   | 100.00%  | 100.00%  |
| `qos/policies/destination_order.rs`     | 72.73%   | 78.57%   | **+5.84** | 85.00%  | 88.64%   |
| `qos/policies/durability.rs`            | 92.31%   | 90.48%   | -1.83   | 96.88%   | 95.45%   |
| `qos/policies/durability_service.rs`    | 73.81%   | 73.33%   | -0.48   | 100.00%  | 100.00%  |
| `qos/policies/entity_factory.rs`        | 77.42%   | 91.67%   | **+14.25** | 100.00% | 100.00% |
| `qos/policies/generic_data.rs`          | 78.95%   | 80.95%   | +2.00   | 95.45%   | 96.20%   |
| `qos/policies/history.rs`               | 91.18%   | 78.38%   | **-12.80** | 100.00% | 91.53% |
| `qos/policies/liveliness.rs`            | 87.80%   | 79.55%   | -8.25   | 96.83%   | 91.04%   |
| `qos/policies/ownership.rs`             | 86.36%   | 78.57%   | -7.79   | 93.94%   | 86.49%   |
| `qos/policies/partition.rs`             | 81.82%   | 88.71%   | **+6.89** | 94.12%  | 88.44%   |
| `qos/policies/presentation.rs`          | 83.05%   | 80.65%   | -2.40   | 98.81%   | 97.75%   |
| `qos/policies/qos_set.rs`               | 81.25%   | 76.81%   | -4.44   | 86.81%   | 86.11%   |
| `qos/policies/reliability.rs`           | 86.49%   | 80.00%   | -6.49   | 93.85%   | 88.41%   |
| `qos/policies/resource_limits.rs`       | 86.49%   | 86.49%   |  0.00   | 100.00%  | 100.00%  |
| `qos/wire_helpers.rs` *(neu)*           | —        | 65.00%   | new     | —        | 100.00%  |
| `qos/review_tests.rs` *(neu)*           | —        | 98.02%   | new     | —        | 98.66%   |
| `rtps/qos_bridge.rs`                    | 97.50%   | 92.86%   | -4.64   | 95.59%   | 89.04%   |
| `transport-tcp/framing.rs`              | 75.44%   | 75.44%   |  0.00   | 82.61%   | 82.61%   |
| `transport-tcp/tcp_transport.rs`        | 67.80%   | 62.22%   | **-5.58** | 74.83% | 70.26%   |

## TOP-5 verbleibende Gaps (<80% R)

1. `transport-tcp/tcp_transport.rs` (62.22% R) — simulate I/O error on
   established `PeerConn` to hit `send`-error + peer-remove branches.
2. `qos/wire_helpers.rs` (65.00% R) — feed malformed TLV so
   `read_*`/`skip_padding` helpers take their short-read error paths.
3. `qos/policies/destination_order.rs` (78.57% R) — decode with unknown
   `kind` discriminant to cover the error return.
4. `qos/policies/reliability.rs` (80.00% R, 2 Fn untested) — roundtrip
   `BestEffort` variant incl. Display/Debug of non-default max-blocking.
5. `qos/policies/history.rs` (78.38% R) — decode `KeepAll` with
   non-zero depth (inconsistency branch) + oversized-depth reject.

Weitere knapp unter 80%: `ownership.rs` 78.57%, `liveliness.rs`
79.55%, `qos_set.rs` 76.81%, `generic_data.rs` 80.95% (grenzwertig).

## Bewertung

Der Workspace-Score hält sich stabil bei ~80.7% R / 91.6% L trotz
deutlicher Neu-Codeeinzüge. `tcp_transport.rs` bleibt die größte
Einzel-Baustelle und hat sich sogar verschlechtert — hier müssen die
neu hinzugekommenen Error-Pfade (vermutlich aus Review-Fix-Runde 2)
noch getestet werden.
