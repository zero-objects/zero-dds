# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

* OMG DDS-AMQP 1.0 §2.1 — Endpoint Profile (Connection Acceptance,
  Sender + Receiver Links, SASL).
* OMG DDS-AMQP 1.0 §9.2 — XML configuration.
* OMG DDS-AMQP 1.0 §10.1 — TLS termination via rustls.
* OASIS AMQP 1.0 — Protocol Header, Frame layout, Performatives,
  Connection state.

### CLI

* `amqp-dds-endpoint --listen <addr>` — start listener.
* `amqp-dds-endpoint --config <path>` — load XML config (Spec §9.2).
* `amqp-dds-endpoint --help` — print usage.

### Public API (library layer `amqp_dds_endpoint`)

* `ServerConfig` — listener configuration (address, frame limits,
  container ID, TLS state).
* `run_server(cfg, metrics, shutdown)` — multi-threaded
  accept-loop entry point.
* `handle_connection(stream, cfg, metrics)` — per-connection
  state-machine driver.
* `bridge::*` — DDS-side bridge (DataWriter / DataReader bindings).
* `client::*` — Outbound client mode (connect to remote AMQP peer).
* `dds_host::*` — DCPS host bindings.
* `tls::*` — `tls`-feature TLS bracket.

### Implementation

Synchronous, `std`-only. Each accepted TCP connection runs in its
own thread, blocking on the underlying `TcpStream`. AMQP 1.0 frames
are read with a fixed 8-byte header + body and dispatched to the
`zerodds-amqp-endpoint` state machine. SASL ANONYMOUS / PLAIN
authentication is supported per Spec §6. The `tls` feature
intercepts the protocol header bytes, performs a rustls 0.23
TLS 1.2/1.3 handshake, and continues with the encrypted stream.

Designed for low-latency message brokerage in mixed AMQP/DDS
deployments. Memory overhead per connection is bounded by
`max_frame_size` (default 65 536 bytes); CPU cost is dominated by
TLS encryption when enabled.

### Architecture

* Layer: Tools.
* Dependencies (in): `zerodds-amqp-bridge` (alloc + std), `zerodds-amqp-endpoint`.
* Optional: `rustls` 0.23, `rustls-pemfile` 2 (feature `tls`).
* Dev-dependencies: `rcgen` 0.13 for self-signed certs in tests.

| Feature | Default | Purpose |
| --- | --- | --- |
| `tls` | ❌ | Enable rustls TLS termination per Spec §10.1. |

### Stability

All `pub` items are RC1-stable. Breaking changes require a major
version bump.
