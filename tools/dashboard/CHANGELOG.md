# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

* OMG DDS-DCPS 1.4 §2.2.5 — Built-in topics consumed for the
  topic / participant / publication / subscription views.
* OMG DDSI-RTPS 2.5 §8.5 — Discovery edges in the graph view.

### CLI

* `--bind <addr>` — bind address (default `127.0.0.1:8089`).
* `--demo` — start a synthetic ticker thread driving the state.

### HTTP API

* `GET /api/topics`, `/api/participants`, `/api/histograms`,
  `/api/graph`, `/api/recording`.
* `POST /api/recording/toggle`.

### Public API (library `zerodds_dashboard`)

* `DashboardState` — single-Mutex state seam.
* `serve(addr, state)` — blocking HTTP server entry point.
* `web::*` — SPA assets (vanilla JS + d3 from CDN).
* `state::*` — typed state structs and JSON serialisation.
* `server::*` — pure-Rust `TcpListener` + per-connection thread.

### Implementation

A pure-Rust HTTP/1.1 server with no `hyper` / `axum` dependency:
each accepted connection runs in its own thread, parses request
lines + headers manually, dispatches against a static route table,
and serialises responses with hand-written JSON. The SPA is bundled
into the binary at compile time (`include_str!`), no `npm` step.

The demo ticker thread mutates the `DashboardState` once per
second to drive the histograms, the topic-rate counters and the
graph edge updates. Production state ingest is the same
single-Mutex seam — a real deployment hooks `monitor::Reader`
events into the same setters.

### Architecture

* Layer: Tools.
* Dependencies (in): `zerodds-foundation`, `zerodds-monitor`
  (features `std` + `prometheus-server`).

### Stability

CLI and HTTP API are RC1-stable. Breaking changes require a major
version bump.
