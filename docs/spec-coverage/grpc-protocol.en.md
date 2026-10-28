# gRPC over HTTP/2 + gRPC-Web — Spec Coverage

**Specs:** `internal/standards/cache/grpc/protocol-http2.md` +
`internal/standards/cache/grpc/protocol-web.md`.

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

**Tests:** `crates/grpc-bridge/src/frame.rs::tests::empty_message_encodes_to_5_byte_header`,
`crates/grpc-bridge/src/frame.rs::tests::uncompressed_message_has_compressed_flag_zero`,
`crates/grpc-bridge/src/frame.rs::tests::compressed_message_has_compressed_flag_one`,
`crates/grpc-bridge/src/frame.rs::tests::message_length_uses_big_endian_4_bytes`,
`crates/grpc-bridge/src/frame.rs::tests::round_trip_message`,
`crates/grpc-bridge/src/frame.rs::tests::header_too_short_decode_fails`,
`crates/grpc-bridge/src/frame.rs::tests::body_truncated_decode_fails`,
`crates/grpc-bridge/src/frame.rs::tests::back_to_back_messages_can_be_decoded_sequentially`.

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

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::request_headers_constants_match_spec`,
`crates/grpc-bridge/src/metadata.rs::tests::content_types_match_spec_strings`,
`crates/grpc-bridge/src/path.rs::tests::parses_standard_grpc_path`,
`crates/grpc-bridge/src/timeout.rs::tests::well_known_unit_chars_match_spec`.

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

**Tests:** `crates/grpc-bridge/src/timeout.rs::tests::timeout_unit_round_trip_for_all`
(all six units), `crates/grpc-bridge/src/timeout.rs::tests::encodes_30_seconds`,
`crates/grpc-bridge/src/timeout.rs::tests::encodes_500_milliseconds`,
`crates/grpc-bridge/src/timeout.rs::tests::round_trip_decode_encode`,
`crates/grpc-bridge/src/timeout.rs::tests::rejects_value_above_8_digits_on_encode`,
`crates/grpc-bridge/src/timeout.rs::tests::decode_rejects_unknown_unit`,
`crates/grpc-bridge/src/timeout.rs::tests::decode_rejects_non_digit_value`,
`crates/grpc-bridge/src/timeout.rs::tests::decode_rejects_empty`,
`crates/grpc-bridge/src/timeout.rs::tests::decode_rejects_only_unit_no_value`.

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

**Tests:** `crates/grpc-bridge/src/status.rs::tests::all_17_codes_round_trip_via_numeric`,
`crates/grpc-bridge/src/status.rs::tests::well_known_codes_match_spec_values`,
`crates/grpc-bridge/src/metadata.rs::tests::response_headers_constants_match_spec`.

**Status:** done — the status-code set + the required-response-header set
declared as constants.

### Status codes

**Spec:** `Status → "grpc-status" 1*DIGIT ; 0-9` with the canonical
values 0..=16 (OK/CANCELLED/UNKNOWN/.../UNAUTHENTICATED).

**Repo:** `crates/grpc-bridge/src/status.rs::Status` enum with
`code/from_code/is_ok/name` methods. All 17 values modeled.

**Tests:** `crates/grpc-bridge/src/status.rs::tests::all_17_codes_round_trip_via_numeric`,
`crates/grpc-bridge/src/status.rs::tests::unknown_codes_yield_none` (reject of code 17/255),
`crates/grpc-bridge/src/status.rs::tests::ok_is_only_status_with_is_ok_true`,
`crates/grpc-bridge/src/status.rs::tests::well_known_codes_match_spec_values`,
`crates/grpc-bridge/src/status.rs::tests::names_use_screaming_snake_case`.

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

**Tests:** frame layer — `crates/http2/src/frame.rs::tests::from_u8_round_trip`
(every frame type code), `crates/http2/src/frame.rs::tests::encode_decode_round_trip`,
`crates/http2/src/frame.rs::tests::unknown_frame_type_rejected`,
`crates/http2/src/frame.rs::tests::r_bit_stripped_from_stream_id`,
`crates/http2/src/frame.rs::tests::frame_too_large_rejected`.
SETTINGS — `crates/http2/src/settings.rs::tests::defaults_match_spec`,
`crates/http2/src/settings.rs::tests::round_trip_encode_decode`,
`crates/http2/src/settings.rs::tests::unknown_setting_id_ignored`.
WINDOW_UPDATE — `crates/http2/src/flow.rs::tests::round_trip_window_update_codec`,
`crates/http2/src/flow.rs::tests::window_update_zero_rejected`,
`crates/http2/src/flow.rs::tests::window_update_overflow_rejected`.
RST_STREAM and the stream states —
`crates/http2/src/stream.rs::tests::idle_to_open_via_headers`,
`crates/http2/src/stream.rs::tests::reset_from_any_state_closes`,
`crates/http2/src/stream.rs::tests::open_recv_headers_stays_open_for_trailers`.
Connection preface — `crates/http2/src/preface.rs::tests::correct_preface_passes`.
HPACK — `crates/hpack/src/integer.rs::tests::rfc7541_c1_1_encoded_10_in_5bit_prefix`,
`crates/hpack/src/integer.rs::tests::rfc7541_c1_2_encoded_1337_in_5bit_prefix`,
`crates/hpack/src/integer.rs::tests::rfc7541_c1_3_encoded_42_in_8bit_prefix`,
`crates/hpack/src/decoder.rs::tests::rfc7541_c2_1_literal_with_indexing`,
`crates/hpack/src/decoder.rs::tests::round_trip_huffman_strings`,
`crates/hpack/src/decoder.rs::tests::dynamic_table_size_update_applied`,
`crates/hpack/src/decoder.rs::tests::invalid_index_zero_rejected`.

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

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::encode_base64_empty`,
`crates/grpc-bridge/src/metadata.rs::tests::encode_base64_one_byte_pads_two`,
`crates/grpc-bridge/src/metadata.rs::tests::encode_base64_two_bytes_pads_one`,
`crates/grpc-bridge/src/metadata.rs::tests::encode_base64_three_bytes_no_padding`,
`crates/grpc-bridge/src/metadata.rs::tests::encode_base64_known_vector`,
`crates/grpc-bridge/src/metadata.rs::tests::decode_base64_round_trip`,
`crates/grpc-bridge/src/metadata.rs::tests::decode_base64_rejects_invalid_chars`,
`crates/grpc-bridge/src/metadata.rs::tests::decode_base64_rejects_bad_padding_length`.

**Status:** done — base64 subprotocol codec + content-type marker live.

---

## Audit status

11 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-grpc-bridge` — 36 lib + 5 integration = 41
  tests green, modules: `frame`, `path`, `server`, `status`, `timeout`.
* `cargo test -p zerodds-hpack` — 49 lib tests green (RFC 7541 HPACK).
* `cargo test -p zerodds-http2` — 45 lib tests green (RFC 7540 frames +
  stream state + settings).
