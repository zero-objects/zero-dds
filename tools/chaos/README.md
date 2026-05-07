# `zerodds-chaos`

> Chaos-engineering CLI for DDS networks: UDP proxy with packet
> loss / latency / reordering, plus `tc` / `iptables` / `netem`
> wrappers and endpoint-flap simulation.

[![Crates.io](https://img.shields.io/crates/v/zerodds-chaos.svg)](https://crates.io/crates/zerodds-chaos)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS chaos-testing tool. Two operating modes:

* **In-process UDP proxy** — no root required; intercepts packets
  between two configured endpoints and applies deterministic chaos
  policies (drop, delay, dup, reorder).
* **Linux `tc` / `netem` wrapper** — orchestrates kernel-level
  qdiscs for whole-interface chaos. Requires root.

A pseudo-random sequence with a fixed seed makes every run
reproducible: same seed → same dropped packets at the same offsets.

## Spec mapping

This is internal tooling — no OMG spec mapping. The behaviour is
defined by the in-tree
[`docs/test-harness/chaos-policies.md`](../../docs/test-harness/chaos-policies.md).

## Safety classification

**COMFORT** — test tooling, never on a runtime path.

## Quickstart

```bash
# Drop 5% of packets between localhost:7400 and localhost:7401:
zerodds-chaos proxy --listen 127.0.0.1:7400 \
                    --upstream 127.0.0.1:7401 \
                    --drop 0.05 --seed 42

# Add 50ms latency on eth0:
sudo zerodds-chaos tc-apply --iface eth0 --delay 50ms

# Clean up:
sudo zerodds-chaos tc-clear --iface eth0
```

## Sub-commands

| Command | Purpose |
| --- | --- |
| `proxy` | In-process UDP chaos proxy (no root). |
| `tc-apply` | Apply a `netem` qdisc on a Linux interface (root). |
| `tc-clear` | Remove a `netem` qdisc (root). |
| `endpoint-flap` | Repeatedly create + destroy a discovery endpoint. |
| `partition` | Two-way proxy with a one-way blackhole. |

## Stability

`1.0.0-rc.1` — public CLI is stable. Breaking changes require a
major version bump.

## Build & test

```bash
cargo build -p zerodds-chaos
cargo test  -p zerodds-chaos
```

## Links

* [`crates/discovery/`](../../crates/discovery/) — endpoint-flap target.
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
