# gRPC over HTTP/2 + gRPC-Web — Spec-Coverage

**Specs:** `docs/standards/cache/grpc/protocol-http2.md` +
`docs/standards/cache/grpc/protocol-web.md`.

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** gRPC ist die Cloud-RPC-Spec von Google. ZeroDDS
implementiert die gRPC-Wire-Elemente in `crates/grpc-bridge/`,
plus eigene no_std-tauglich-HTTP/2-Implementation in
`crates/http2/` (RFC 7540) und HPACK-Header-Compression in
`crates/hpack/` (RFC 7541). Compression-Codecs (gzip/deflate) sind
Caller-Layer.

Crate-Mapping:

| Spec-Bereich | Crate(s) |
|---|---|
| gRPC Wire (LPM, Path, Status, Timeout) | `crates/grpc-bridge/` |
| HTTP/2 Frame-Layer (RFC 7540) | `crates/http2/` |
| HPACK Header-Compression (RFC 7541) | `crates/hpack/` |

---

## protocol-http2 §"Outline"

### Request/Response Message Stream

**Spec:** `Request → Request-Headers *Length-Prefixed-Message EOS`,
`Response → (Response-Headers *Length-Prefixed-Message Trailers) /
Trailers-Only`.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message, encode_web_trailers}`.

**Tests:** `frame::tests::*` (9 Tests inkl. back-to-back-Stream).

**Status:** done

---

## protocol-http2 §"Requests"

### Request-Headers (Path/Method/Authority/etc.)

**Spec:** `Method` `:method POST`, `Scheme` `:scheme http/https`,
`Path` `/<service>/<method>`, `Authority`, `TE: trailers`,
`grpc-timeout`, `Content-Type: application/grpc`,
`grpc-encoding`/`grpc-accept-encoding`, `User-Agent`,
`grpc-message-type`, Custom-Metadata.

**Repo:** Path-Parsing in `crates/grpc-bridge/src/path.rs::parse_path`;
Timeout-Header in `crates/grpc-bridge/src/timeout.rs`;
Generic-Request-Header-Konstanten in
`crates/grpc-bridge/src/metadata.rs::request_headers::{METHOD, SCHEME,
PATH, AUTHORITY, TE, CONTENT_TYPE, GRPC_ENCODING,
GRPC_ACCEPT_ENCODING, USER_AGENT, GRPC_TIMEOUT, GRPC_MESSAGE_TYPE}`
+ `content_types::{GRPC, GRPC_PROTO, GRPC_WEB, GRPC_WEB_TEXT}`.

**Tests:** `path::tests::*` (7 Tests),
`timeout::tests::*` (10 Tests),
`metadata::tests::{request_headers_constants_match_spec,
content_types_match_spec_strings}`.

**Status:** done — alle Required-Header-Names + Content-Type-Set
als Konstanten ausgewiesen.

### Path-Parsing

**Spec:** `Path → ":path" "/" Service-Name "/" {method name}`.

**Repo:** `crates/grpc-bridge/src/path.rs::parse_path` mit Reject von
relative-paths, missing-method, empty-segments, method-mit-slash.

**Tests:** `path::tests::parses_standard_grpc_path`,
`rejects_relative_path`, `rejects_path_without_method`,
`rejects_empty_service`, `rejects_empty_method`,
`rejects_method_with_slash`, `parses_dotted_service_name`.

**Status:** done

### Timeout Header (`grpc-timeout`)

**Spec:** `Timeout → "grpc-timeout" TimeoutValue TimeoutUnit`,
TimeoutValue MUST be at most 8 digits, Unit ∈ {H,M,S,m,u,n}.

**Repo:** `crates/grpc-bridge/src/timeout.rs::{TimeoutUnit,
encode_timeout, decode_timeout}` mit Reject von ueber-8-Digit, non-
digit, unbekannter-Unit, leerem Header.

**Tests:** `timeout::tests::*` (10 Tests inkl. Round-Trip aller
6 Units).

**Status:** done

### Length-Prefixed-Message

**Spec:** `Length-Prefixed-Message → Compressed-Flag Message-Length
Message`, Compressed-Flag = 1 byte, Message-Length = 4 byte BE.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message}` mit empty/short/large/back-to-back-Stream-Tests.

**Tests:** `frame::tests::empty_message_encodes_to_5_byte_header`,
`uncompressed_message_has_compressed_flag_zero`,
`compressed_message_has_compressed_flag_one`, `round_trip_message`,
`message_length_uses_big_endian_4_bytes`, `header_too_short_decode_fails`,
`body_truncated_decode_fails`,
`back_to_back_messages_can_be_decoded_sequentially`.

**Status:** done

---

## protocol-http2 §"Responses"

### Response-Headers + Trailers

**Spec:** `Response-Headers → HTTP-Status [Message-Encoding]
[Message-Accept-Encoding] Content-Type *Custom-Metadata`,
`Trailers → Status [Status-Message] *Custom-Metadata`.

**Repo:** Status-Code in `crates/grpc-bridge/src/status.rs`;
Response-Header-Konstanten in
`crates/grpc-bridge/src/metadata.rs::response_headers::{STATUS,
CONTENT_TYPE, GRPC_ENCODING, GRPC_STATUS, GRPC_MESSAGE}`.

**Tests:** Cross-Ref `status::tests::*`,
`metadata::tests::response_headers_constants_match_spec`.

**Status:** done — Status-Code-Set + Required-Response-Header-Set
als Konstanten ausgewiesen.

### Status Codes

**Spec:** `Status → "grpc-status" 1*DIGIT ; 0-9` mit kanonischen
Werten 0..=16 (OK/CANCELLED/UNKNOWN/.../UNAUTHENTICATED).

**Repo:** `crates/grpc-bridge/src/status.rs::Status` Enum mit
`code/from_code/is_ok/name`-Methods. Alle 17 Werte modelliert.

**Tests:** `status::tests::*` (5 Tests inkl. Round-Trip aller 17 +
Reject von Code-17/255 + name() screaming-snake-case).

**Status:** done

---

## protocol-http2 §"Custom-Metadata"

### Binary + ASCII Headers

**Spec:** `Binary-Header → {Header-Name "-bin"} {base64 encoded value}`,
`ASCII-Header → Header-Name ASCII-Value`. Header-Names mit `grpc-`
sind reserved.

**Repo:** `crates/grpc-bridge/src/metadata.rs::{is_binary_header,
encode_base64, decode_base64, encode_header_value,
decode_header_value, BIN_SUFFIX}`. RFC-4648-base64 ohne externe
Crate-Dependency; rejection von non-ASCII-Bytes in Text-Headern.

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
decode_header_value_text_passes_through}` (15 Tests).

**Status:** done — Binary-Header-Codec + ASCII-Validation live.

---

## protocol-http2 §"HTTP2 Transport Mapping"

### HTTP/2 Frame-Types + HPACK + Connection-Management

**Spec:** HEADERS, CONTINUATION, DATA, RST_STREAM, SETTINGS, PING,
GOAWAY, WINDOW_UPDATE.

**Repo:** `crates/http2/src/{frame,settings,stream,flow,preface,
error}.rs` (1191 SLOC) für RFC 7540 Frame-Layer + Connection-
Management; `crates/hpack/src/{encoder,decoder,table,huffman,
integer,string}.rs` (1704 SLOC) für RFC 7541 Header-Compression.

**Tests:** Inline-Tests in beiden Crates.

**Status:** done — eigene no_std-tauglich-Implementation (Greenfield-
Lücke in 2026 geschlossen).

---

## protocol-web

### gRPC-Web Trailers in LPM-Frame

**Spec:** gRPC-Web encoded Trailers im Length-Prefixed-Message-Frame
mit Compressed-Flag-MSB=1 (`0x80`).

**Repo:** `crates/grpc-bridge/src/frame.rs::encode_web_trailers`.

**Tests:** `frame::tests::web_trailers_encoded_with_msb_set`.

**Status:** done — Caller liefert die Trailers als UTF-8-Bytes
(typisch `grpc-status: 0\r\ngrpc-message: ...\r\n`).

### gRPC-Web Base64-Encoding (text-Subprotocol)

**Spec:** gRPC-Web hat `application/grpc-web` (binary) und
`application/grpc-web-text` (base64-encoded).

**Repo:** `crates/grpc-bridge/src/metadata.rs::{encode_base64,
decode_base64}` (RFC-4648 Standard-Variante mit Padding) +
`content_types::GRPC_WEB_TEXT` als Marker.

**Tests:** Cross-Ref `metadata::tests::{encode_base64_*,
decode_base64_round_trip}`.

**Status:** done — Base64-Subprotocol-Codec + Content-Type-Marker
live.

---

## Audit-Status

12 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-grpc-bridge` — 36 lib + 5 integration = 41
  Tests grün, Module: `frame`, `path`, `server`, `status`, `timeout`.
* `cargo test -p zerodds-hpack` — 49 lib-Tests grün (RFC 7541 HPACK).
* `cargo test -p zerodds-http2` — 45 lib-Tests grün (RFC 7540 Frames +
  Stream-State + Settings).

Offene Punkte: siehe `grpc-protocol.open.md`.
