# gRPC over HTTP/2 + gRPC-Web — Spec Coverage

**Specs:** `docs/standards/cache/grpc/protocol-http2.md` +
`docs/standards/cache/grpc/protocol-web.md`.

Follows the format from `docs/spec-coverage/PROCESS.md`.

**Context:** gRPC is Google's cloud RPC spec. ZeroDDS implements the gRPC
stack spread across:

- `crates/grpc-bridge/` — gRPC wire elements (LPM, path, status, timeout)
- `crates/http2/` — its own no_std-capable HTTP/2 implementation (RFC 7540)
- `crates/hpack/` — HPACK header compression (RFC 7541)

Compression codecs (gzip/deflate) are caller-layer.

Crate mapping:

| Spec area | Crate(s) |
|---|---|
| gRPC wire (LPM, path, status, timeout) | `crates/grpc-bridge/` |
| HTTP/2 frame layer (RFC 7540) | `crates/http2/` |
| HPACK header compression (RFC 7541) | `crates/hpack/` |

---

## protocol-http2 §"Outline"

### Request/response message stream

**Spec:** `Request → Request-Headers *Length-Prefixed-Message EOS`,
`Response → (Response-Headers *Length-Prefixed-Message Trailers) /
Trailers-Only`.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message, encode_web_trailers}`.

**Tests:** `frame::tests::*` (9 tests incl. back-to-back stream).

**Status:** done

---

## protocol-http2 §"Requests"

### Request headers (path/method/authority/etc.)

**Spec:** `Method` `:method POST`, `Scheme` `:scheme http/https`,
`Path` `/<service>/<method>`, `Authority`, `TE: trailers`,
`grpc-timeout`, `Content-Type: application/grpc`,
`grpc-encoding`/`grpc-accept-encoding`, `User-Agent`,
`grpc-message-type`, custom metadata.

**Repo:** path parsing in `crates/grpc-bridge/src/path.rs::parse_path`;
the timeout header in `crates/grpc-bridge/src/timeout.rs`;
generic request-header constants in
`crates/grpc-bridge/src/metadata.rs::request_headers::{METHOD, SCHEME,
PATH, AUTHORITY, TE, CONTENT_TYPE, GRPC_ENCODING,
GRPC_ACCEPT_ENCODING, USER_AGENT, GRPC_TIMEOUT, GRPC_MESSAGE_TYPE}`
+ `content_types::{GRPC, GRPC_PROTO, GRPC_WEB, GRPC_WEB_TEXT}`.

**Tests:** `path::tests::*` (7 tests),
`timeout::tests::*` (10 tests),
`metadata::tests::{request_headers_constants_match_spec,
content_types_match_spec_strings}`.

**Status:** done — all required header names + the content-type set
declared as constants.

### Path parsing

**Spec:** `Path → ":path" "/" Service-Name "/" {method name}`.

**Repo:** `crates/grpc-bridge/src/path.rs::parse_path` rejecting relative
paths, a missing method, empty segments, a method with a slash.

**Tests:** `path::tests::parses_standard_grpc_path`,
`rejects_relative_path`, `rejects_path_without_method`,
`rejects_empty_service`, `rejects_empty_method`,
`rejects_method_with_slash`, `parses_dotted_service_name`.

**Status:** done

### Timeout header (`grpc-timeout`)

**Spec:** `Timeout → "grpc-timeout" TimeoutValue TimeoutUnit`,
TimeoutValue MUST be at most 8 digits, unit ∈ {H,M,S,m,u,n}.

**Repo:** `crates/grpc-bridge/src/timeout.rs::{TimeoutUnit,
encode_timeout, decode_timeout}` rejecting over-8-digit, non-digit,
unknown-unit, an empty header.

**Tests:** `timeout::tests::*` (10 tests incl. round-trip of all
6 units).

**Status:** done

### Length-prefixed message

**Spec:** `Length-Prefixed-Message → Compressed-Flag Message-Length
Message`, Compressed-Flag = 1 byte, Message-Length = 4 bytes BE.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message}` with empty/short/large/back-to-back stream tests.

**Tests:** `frame::tests::empty_message_encodes_to_5_byte_header`,
`uncompressed_message_has_compressed_flag_zero`,
`compressed_message_has_compressed_flag_one`, `round_trip_message`,
`message_length_uses_big_endian_4_bytes`, `header_too_short_decode_fails`,
`body_truncated_decode_fails`,
`back_to_back_messages_can_be_decoded_sequentially`.

**Status:** done

---

## protocol-http2 §"Responses"

### Response headers + trailers

**Spec:** `Response-Headers → HTTP-Status [Message-Encoding]
[Message-Accept-Encoding] Content-Type *Custom-Metadata`,
`Trailers → Status [Status-Message] *Custom-Metadata`.

**Repo:** the status code in `crates/grpc-bridge/src/status.rs`;
response-header constants in
`crates/grpc-bridge/src/metadata.rs::response_headers::{STATUS,
CONTENT_TYPE, GRPC_ENCODING, GRPC_STATUS, GRPC_MESSAGE}`.

**Tests:** cross-ref `status::tests::*`,
`metadata::tests::response_headers_constants_match_spec`.

**Status:** done — the status-code set + the required-response-header set
declared as constants.

### Status codes

**Spec:** `Status → "grpc-status" 1*DIGIT ; 0-9` with the canonical
values 0..=16 (OK/CANCELLED/UNKNOWN/.../UNAUTHENTICATED).

**Repo:** `crates/grpc-bridge/src/status.rs::Status` enum with
`code/from_code/is_ok/name` methods. All 17 values modeled.

**Tests:** `status::tests::*` (5 tests incl. round-trip of all 17 +
reject of code 17/255 + name() screaming-snake-case).

**Status:** done

---

## protocol-http2 §"Custom-Metadata"

### Binary + ASCII headers

**Spec:** `Binary-Header → {Header-Name "-bin"} {base64 encoded value}`,
`ASCII-Header → Header-Name ASCII-Value`. Header names with `grpc-`
are reserved.

**Repo:** `crates/grpc-bridge/src/metadata.rs::{is_binary_header,
encode_base64, decode_base64, encode_header_value,
decode_header_value, BIN_SUFFIX}`. RFC-4648 base64 without an external
crate dependency; rejection of non-ASCII bytes in text headers.

**Tests:** `metadata::tests::{is_binary_header_recognizes_bin_suffix,
is_binary_header_rejects_text_headers, encode_base64_empty,
encode_base64_one_byte_pads_two, encode_base64_two_bytes_pads_one,
encode_base64_three_bytes_no_padding, encode_base64_known_vector,
decode_base64_round_trip, decode_base64_rejects_invalid_chars,
decode_base64_rejects_bad_padding_length,
encode_header_value_text_passes_ascii,
encode_header_value_text_rejects_non_ascii,
encode_header_value_bin_uses_base64,
decode_header_value_round_trip_bin,
decode_header_value_text_passes_through}` (15 tests).

**Status:** done — binary-header codec + ASCII validation live.

---

## protocol-http2 §"HTTP2 Transport Mapping"

### HTTP/2 frame types + HPACK + connection management

**Spec:** HEADERS, CONTINUATION, DATA, RST_STREAM, SETTINGS, PING,
GOAWAY, WINDOW_UPDATE.

**Repo:** `crates/http2/src/{frame,settings,stream,flow,preface,
error}.rs` (1191 SLOC) for the RFC 7540 frame layer + connection
management; `crates/hpack/src/{encoder,decoder,table,huffman,
integer,string}.rs` (1704 SLOC) for RFC 7541 header compression.

**Tests:** inline tests in both crates.

**Status:** done — its own no_std-capable implementation (a greenfield
gap closed in 2026).

---

## protocol-web

### gRPC-Web trailers in an LPM frame

**Spec:** gRPC-Web encodes trailers in the Length-Prefixed-Message frame
with the Compressed-Flag MSB=1 (`0x80`).

**Repo:** `crates/grpc-bridge/src/frame.rs::encode_web_trailers`.

**Tests:** `frame::tests::web_trailers_encoded_with_msb_set`.

**Status:** done — the caller provides the trailers as UTF-8 bytes
(typically `grpc-status: 0\r\ngrpc-message: ...\r\n`).

### gRPC-Web base64 encoding (text subprotocol)

**Spec:** gRPC-Web has `application/grpc-web` (binary) and
`application/grpc-web-text` (base64-encoded).

**Repo:** `crates/grpc-bridge/src/metadata.rs::{encode_base64,
decode_base64}` (the RFC-4648 standard variant with padding) +
`content_types::GRPC_WEB_TEXT` as a marker.

**Tests:** cross-ref `metadata::tests::{encode_base64_*,
decode_base64_round_trip}`.

**Status:** done — base64 subprotocol codec + content-type marker live.

---

## Audit status

12 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-grpc-bridge` — 36 lib + 5 integration = 41
  tests green, modules: `frame`, `path`, `server`, `status`, `timeout`.
* `cargo test -p zerodds-hpack` — 49 lib tests green (RFC 7541 HPACK).
* `cargo test -p zerodds-http2` — 45 lib tests green (RFC 7540 frames +
  stream state + settings).
