# `zerodds-dashboard` — Live-Monitoring-Dashboard

> Browser-based live dashboard for ZeroDDS domains: topic list,
> participant list, latency histograms, discovery graph, recording
> control.

[![Crates.io](https://img.shields.io/crates/v/zerodds-dashboard.svg)](https://crates.io/crates/zerodds-dashboard)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS operator UI. An embedded HTTP server delivers a vanilla-JS
SPA (no npm build) plus JSON endpoints; the back-end accepts state
either from a synthetic demo ticker or from a live DcpsRuntime
through the built-in topics reader.

## Spec mapping

| Spec | Section |
| --- | --- |
| OMG DDS-DCPS 1.4 | §2.2.5 — Built-in topics |
| OMG DDSI-RTPS 2.5 | §8.5 — Discovery |

## Safety classification

**COMFORT** — operator tooling.

## Quickstart

```bash
cargo run -p zerodds-dashboard --release -- --demo --bind 127.0.0.1:8089
# → open http://127.0.0.1:8089/
```

Use `--bind` only without `--demo` to attach to a running domain
through the published `DashboardState`.

## Features at a glance

* Topic list with pub / sub counts and sample rate.
* Participant list with GUID + domain + vendor.
* Latency histograms (write / read / heartbeat-RTT) with p50 / p99 / max.
* Discovery graph (participants + edges) as a d3 force layout.
* Recording start / stop toggle.

## API

| Method | Path | Body |
|---|---|---|
| `GET` | `/` or `/index.html` | SPA shell. |
| `GET` | `/api/topics` | topic array |
| `GET` | `/api/participants` | participant array |
| `GET` | `/api/histograms` | histogram array |
| `GET` | `/api/graph` | `{ nodes, edges }` |
| `GET` | `/api/recording` | `{ active, output_path, frames }` |
| `POST` | `/api/recording/toggle` | toggles + returns new state |

## Architecture

```
  Browser (HTML + JS + d3)
       │  HTTP/JSON polling
       ▼
  zerodds-dashboard HTTP server (pure Rust — no hyper/axum)
       │  DashboardState (Mutex<Inner>)
       ▼
  consumer app (bridge / tool / DcpsRuntime hook)
```

The `DashboardState` is the only read/write seam. The demo ticker
thread (`--demo`) provides synthetic state for UI development; a
production deployment populates it from the built-in
`DCPSParticipant` / `Publication` / `Subscription` topics.

## Stability

`1.0.0-rc.1` — public CLI and HTTP API are stable. Breaking
changes require a major version bump.

## Build & test

```bash
cargo build -p zerodds-dashboard
cargo test  -p zerodds-dashboard
```

## Links

* [`tools/dashboard-tauri/`](../dashboard-tauri/) — native Tauri shell.
* [`crates/monitor/`](../../crates/monitor/) — built-in monitor topics.
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
