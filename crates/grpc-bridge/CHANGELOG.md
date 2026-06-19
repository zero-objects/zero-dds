# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-grpc-bridge` crate.

### Spec references

- **gRPC HTTP/2 Protocol** (`grpc.io/docs/what-is-grpc/core-concepts/`): length-prefixed message, `:path` format `/<service>/<method>`, `grpc-timeout` header (H/M/S/m/u/n), `grpc-status` status codes (0..=16), custom metadata (`-bin` suffix → Base64).
- **gRPC-Web Specification**: trailer frame (LPM with compressed-flag MSB=1), content types (`application/grpc-web`, `application/grpc-web+proto`, `application/grpc-web-text`, `application/grpc-web+json`).

### Public API

**Frame (`frame` module):**
- `encode_message(payload: &[u8], compressed: bool) -> Result<Vec<u8>, FrameError>`.
- `decode_message(bytes: &[u8]) -> Result<(u8 /* flag */, Vec<u8>, usize), FrameError>`.
- `FrameError`.

**Path (`path` module):**
- `parse_path(path: &str) -> Result<(&str /* service */, &str /* method */), PathError>`.
- `PathError`.

**Timeout (`timeout` module):**
- `TimeoutUnit::{Hours, Minutes, Seconds, Milliseconds, Microseconds, Nanoseconds}`.
- `encode_timeout(value: u64, unit: TimeoutUnit) -> Result<String, TimeoutError>`.
- `decode_timeout(s: &str) -> Result<(u64, TimeoutUnit), TimeoutError>`.

**Status (`status` module):**
- `Status::{Ok, Cancelled, Unknown, InvalidArgument, DeadlineExceeded, NotFound, AlreadyExists, PermissionDenied, ResourceExhausted, FailedPrecondition, Aborted, OutOfRange, Unimplemented, Internal, Unavailable, DataLoss, Unauthenticated}` (all 17 §"Status" codes).

**Metadata (`metadata` module):**
- `BIN_SUFFIX = "-bin"`.
- `is_binary_header(name) -> bool`.
- `encode_header_value(value: &[u8], binary: bool) -> Result<String, MetadataError>` / `decode_header_value(name, value) -> Result<Vec<u8>, MetadataError>`.
- `encode_base64` / `decode_base64`.
- `request_headers(...)` / `response_headers(...)` — Standard-Header-Sets (incl. `:method=POST`, `content-type`, `te=trailers`).
- `content_types::{GRPC, GRPC_PROTO, GRPC_WEB, GRPC_WEB_PROTO, GRPC_WEB_TEXT, GRPC_WEB_JSON}`.
- `MetadataError`.

**Server (`server` module):**
- `GrpcRequest { service, method, headers, body }`.
- `GrpcResponse { status, headers, body }`.
- `GrpcServer::{new, dispatch}` — skeleton for caller-configured HTTP/2 listeners; the caller hooks its own dispatch logic via a trait closure.

### Implementation

`encode_message` / `decode_message` implements the LPM wire format strictly per spec: `[flag: u8, length: u32 BE, bytes...]`. `flag & 0x80` signals a gRPC-Web trailer frame; `flag & 0x01` signals a compressed body. The decoder rejects when `bytes.len() < 5 + length`.

`parse_path` rejects non-`/<svc>/<method>` forms (e.g. missing slash, wrong number of slashes, empty components).

`encode_timeout` automatically selects the most compact unit bytes; `decode_timeout` accepts all standard units. The spec mandates an 8-digit maximum (e.g. `99999999H`); the encoder rejects oversized values.

Custom metadata: `is_binary_header` checks only the `-bin` suffix (case-insensitive). `encode_header_value` does Base64 when binary, otherwise raw ASCII (rejects non-printable). `decode_header_value` reverses the Base64 encoding.

`GrpcServer` is intentionally a skeleton — the crate ships **no** TCP/TLS/HTTP/2 connection management (the caller handles that via `zerodds-http2`); it provides only the application-layer dispatch hooks.

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`.

### Architecture

- **Layer:** 5 (Bridges).
- **Dependencies (in):** `zerodds-http2` (framing + streams), `zerodds-hpack` (HEADERS compression).
- **Dependents (out):** (planned) DDS-RPC service endpoint layer.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by gRPC HTTP/2 Protocol + gRPC-Web.
- Error discriminants: stable; new discriminants are major-additive.

### Added — daemon wireup

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), the catalog/healthz/metrics admin endpoint
  (§5.2), a signal watcher for graceful shutdown (§9.2), and an
  OTLP span exporter (§8.3).
- Bridge security: TLS server (rustls 0.23) with ALPN-`h2` + auth modes
  + topic ACL via `zerodds-bridge-security` (bridge spec §7.1/§7.2/§7.3).
- Service codegen helper (`service_gen.rs`) +
  reflection API module (`reflection.rs`) +
  cross-vendor interop module.
- DDS QoS → gRPC behavior translation in `qos_translation`.
