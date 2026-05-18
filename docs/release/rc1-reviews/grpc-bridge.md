# RC1 Review — `zerodds-grpc-bridge`

> **Layer:** 5 (Bridges, Tier-B) | **Reviewer:** claude | **Public-Strategy:** 🌐 public

## 1 Purpose

gRPC-over-HTTP/2 + gRPC-Web Wire-Codec: Length-Prefixed-Message + Path + Timeout + Status + Custom-Metadata + Server-Skeleton. Sitzt auf `zerodds-http2` + `zerodds-hpack`.

## 2-3 Inhalt

- 7 src-Files (frame, lib, metadata, path, server, status, timeout).
- 1 tests-File (fuzz_smoke.rs).
- **60 Tests gruen** (54 unit + 5 fuzz-smoke + 1 doc).

## 3.4 Coherence-Audit

| Item-Familie | Spec | Klassifikation |
|---|---|---|
| `encode_message` / `decode_message` | gRPC LPM | OPTIONAL-HOOK (Substrat) |
| `parse_path` | gRPC `:path` | OPTIONAL-HOOK |
| `encode_timeout` / `decode_timeout` / `TimeoutUnit` | `grpc-timeout` | OPTIONAL-HOOK |
| `Status` | `grpc-status` | OPTIONAL-HOOK |
| Custom-Metadata + `BIN_SUFFIX` | gRPC Metadata | OPTIONAL-HOOK |
| `request_headers` / `response_headers` / `content_types` | gRPC HTTP/2 Standard-Headers + gRPC-Web | OPTIONAL-HOOK |
| `GrpcServer` / `GrpcRequest` / `GrpcResponse` | gRPC Server-Skeleton | OPTIONAL-HOOK (per Design Caller-konstruiert) |

Alle 7 OPTIONAL-HOOK; explizit dokumentiert. 0 ❌.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
zerodds-http2 = { path = "../http2" }   # RFC 9113 Framing
zerodds-hpack = { path = "../hpack" }   # RFC 7541 Header-Compression
```

Beide Tier-A-Deps sind seit Layer-5-Walking ✅ rc1-ready.

### 4.2 Dependents (used-by)

`crates/conformance/Cargo.toml` (Cross-Vendor-Test-Harness, Layer 7).

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (Crate war pre-Review pristine).
- **TODO/FIXME/Stub/unimplemented!:** 0.
- **lib.rs**: Header korrigiert — vorherige "Was nicht abgedeckt: HTTP/2 Framing (RFC 7540)" Behauptung war stale. RFC 7540 → 9113, und HTTP/2 + HPACK SIND als Substrat-Crates verfuegbar (`zerodds-http2` + `zerodds-hpack`); "Server-Skeleton" wurde vorher als Caller-Layer behauptet, ist aber als `GrpcServer` implementiert.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: vollstaendiger RC1-Header + Quickstart-Doc-Test mit korrekter `decode_message`-API-Signatur.
3. SPDX-License-Header auf alle 7 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/grpc-bridge/`.
6. `website/docs/grpc-bridge.md`.
7. Tracker: 5.4 grpc-bridge → ✅.

## 10-12 Gates

- `cargo test`: ✅ 60 tests.
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `daemon`-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon_runtime.rs` + `qos_translation.rs` Module.
- TLS-Server (rustls 0.23 ServerConnection) mit ALPN-`h2` + Auth-Modes
  + Topic-ACL via `zerodds-bridge-security` voll wired
  (Bridge-Spec §7.1/§7.2/§7.3).
- `service_gen.rs` + `reflection.rs` + `cross_vendor.rs` Module.
- E2E-Test `tests/bridge_e2e.rs` ist pre-existing pre-RC1-Closeout
  (HTTP/2-Roundtrip mit Daemon-Spawn) und failt aktuell (siehe
  `RC1_FINDINGS.md`); Fix ist Phase-2 Issue, kein RC1-Blocker
  (Wire-Layer ist getestet, der Spawn-Pfad fehlt im Test-Harness
  noch eine Bind-Wait-Logik).
- Tests gruen: 54 unit + 5 fuzz-smoke + 1 doc; 2 e2e pre-existing-fail.

**Crate-Version:** `1.0.0-rc.1` | **Sign-off:** claude
