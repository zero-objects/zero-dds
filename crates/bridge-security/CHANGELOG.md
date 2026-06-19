# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).
Per-entry date markers are a Keep-a-Changelog convention; everything
else is git-tracked.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization.

### Spec references

- ZeroDDS Bridge Spec 1.0 §7.1 — transport-layer security (TLS).
- ZeroDDS Bridge Spec 1.0 §7.2 — auth modes `none|bearer|jwt|mtls|sasl`.
- ZeroDDS Bridge Spec 1.0 §7.3 — topic ACL (read / write per topic pattern).

### Public API

**Re-exports from `lib.rs`:**

- `Acl`, `AclEntry`, `AclOp` (module `acl`) — topic ACL with wildcard
  and group matching.
- `AuthError`, `AuthMode`, `AuthSubject` (module `auth`) — auth modes
  and subject data type with group memberships + free-form claims.
- `RotatingTlsConfig`, `build_client_tls_connector`, `parse_server_name`,
  `serve_tls_handshake` (module `connection`) — per-connection TLS helpers.
- `SecurityConfig`, `SecurityCtx`, `SecurityError`, `authenticate`,
  `authorize`, `build_ctx`, `extract_mtls_subject` (module `ctx`) —
  aggregate ctx of auth + ACL + TLS, plus the top-level helpers
  `authenticate` / `authorize`.
- `TlsConfigError`, `load_server_config` (module `tls`) — `rustls`
  ServerConfig builder with PEM cert/key loader and optional client
  CA trust for mTLS.

### Implementation

`load_server_config` parses PEM files with `rustls-pemfile`, feeds them
into `rustls::ServerConfig::builder()` and supports three paths:
server cert only, server cert with client CA trust (mTLS), and the
reload pattern `RotatingTlsConfig::reload()` for SIGHUP-triggered cert
rotation without connection drop.

`AuthMode::validate` is the single entry point per connection: it
receives the wire-specific material (HTTP headers, MQTT CONNECT
properties, SASL-PLAIN tokens, mTLS peer cert) and deterministically
produces an `AuthSubject` or an `AuthError`. JWT validation runs through
`ring` (RS256 signature), avoiding a second crypto dep alongside rustls.

`Acl` matches in two stages: subject name (wildcard `*`) and group
membership; per topic, each stage can grant read or write.

### Architecture

- **Layer:** 5 (Bridges) — shared substrate layer for all six bridge
  daemons.
- **Dependencies (in):** `rustls 0.23` (with `ring` backend, no
  `aws-lc-rs`), `rustls-pemfile`, `rustls-pki-types`, `ring`, `base64`.
  No ZeroDDS internals.
- **Dependents (out):** `zerodds-websocket-bridge`, `zerodds-mqtt-bridge`,
  `zerodds-coap-bridge`, `zerodds-amqp-endpoint`, `zerodds-grpc-bridge`,
  `zerodds-corba-dds-bridge` — all as optional deps in the `daemon` feature.
- **Feature flags:**
  | Flag | Default | Purpose |
  |------|---------|---------|
  | `std` | ✅ | Required (rustls 0.23 needs std). |

### Stability

- All `pub` items are RC1-stable; breaking changes require a major bump.
- New `AuthMode` discriminants or new `AclOp` variants are
  major-additive (rustc rejects non-exhaustive without
  `#[non_exhaustive]` — we keep exhaustive match syntax and bump major on
  extension).
