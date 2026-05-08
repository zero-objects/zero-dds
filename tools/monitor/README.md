# zerodds-monitor

Live snapshot or Prometheus `/metrics` server for the **ZeroDDS runtime
registry**.

The runtime (DCPS, RTPS, transport) registers metrics into a process-
global registry. This tool starts a `DcpsRuntime`, lets it run for a
configurable duration, then either prints the snapshot or exposes it
over HTTP so Prometheus / Grafana can scrape.

## Sub-Commands

```text
zerodds-monitor snapshot [-d DOMAIN] [--duration DUR] [-f FORMAT]
zerodds-monitor serve    [-d DOMAIN] [-a ADDR] [--duration DUR]
zerodds-monitor names
```

| Flag                | Meaning                                            | Default                |
|---------------------|----------------------------------------------------|------------------------|
| `-d, --domain`      | DDS Domain ID                                      | 0                      |
| `--duration`        | Run duration (`5`, `30s`, `2m`, `1h`)              | snapshot 5s · serve ∞  |
| `-f, --format`      | `text` or `prometheus` (snapshot only)             | text                   |
| `-a, --addr`        | Listen address (serve only)                        | 127.0.0.1:9991         |

## Examples

```bash
# Quick 5s text snapshot of all metrics
zerodds-monitor snapshot

# Prometheus-format snapshot
zerodds-monitor snapshot -f prometheus

# Run /metrics server until Ctrl-C
zerodds-monitor serve

# Auto-stop after 60s
zerodds-monitor serve --duration 60s

# List known metric names
zerodds-monitor names
```

## Metric Names

Defined in `crates/monitor/src/metric_names.rs`. `zerodds-monitor names`
prints the canonical list.

## Exit Codes

| Code | Meaning           |
|------|-------------------|
| 0    | Success           |
| 2    | CLI parse error   |
| 3    | DDS / I/O error   |

## Backend

`crates/monitor` — published as `zerodds-monitor` on crates.io. This
CLI crate (`zerodds-monitor-cli`) is a thin frontend that ships the
`zerodds-monitor` binary.
