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

**Tests:** `crates/grpc-bridge/src/frame.rs::tests::grpc_outline_empty_message_encodes_to_5_byte_header`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_uncompressed_message_has_compressed_flag_zero`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_compressed_message_has_compressed_flag_one`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_message_length_uses_big_endian_4_bytes`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_requests_a_length_prefixed_message_is_read_from_its_octets`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_header_too_short_decode_fails`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_body_truncated_decode_fails`,
`crates/grpc-bridge/src/frame.rs::tests::grpc_outline_back_to_back_messages_can_be_decoded_sequentially`.

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

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::grpc_requests_request_headers_constants_match_spec`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_requests_content_types_match_spec_strings`,
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_standard_grpc_path`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n`.

**Status:** done — all required header names + the content-type set
declared as constants.

### Path parsing

**Spec:** `Path → ":path" "/" Service-Name "/" {method name}`.

**Repo:** `crates/grpc-bridge/src/path.rs::parse_path` rejecting relative
paths, a missing method, empty segments, a method with a slash.

**Tests:** `path::tests::grpc_requests_parses_standard_grpc_path`,
`grpc_requests_rejects_relative_path`, `grpc_requests_rejects_path_without_method`,
`grpc_requests_rejects_empty_service`, `grpc_requests_rejects_empty_method`,
`grpc_requests_rejects_method_with_slash`, `grpc_requests_parses_dotted_service_name`.

**Status:** done

### Timeout header (`grpc-timeout`)

**Spec:** `Timeout → "grpc-timeout" TimeoutValue TimeoutUnit`,
TimeoutValue MUST be at most 8 digits, unit ∈ {H,M,S,m,u,n}.

**Repo:** `crates/grpc-bridge/src/timeout.rs::{TimeoutUnit,
encode_timeout, decode_timeout}` rejecting over-8-digit, non-digit,
unknown-unit, an empty header.

**Tests:** `crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n`
(all six units), `crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_encodes_30_seconds`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_encodes_500_milliseconds`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_a_timeout_header_is_read_as_value_and_unit`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_rejects_value_above_8_digits_on_encode`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_unknown_unit`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_non_digit_value`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_empty`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_only_unit_no_value`.

**Status:** done

### Length-prefixed message

**Spec:** `Length-Prefixed-Message → Compressed-Flag Message-Length
Message`, Compressed-Flag = 1 byte, Message-Length = 4 bytes BE.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message}` with empty/short/large/back-to-back stream tests.

**Tests:** `frame::tests::grpc_outline_empty_message_encodes_to_5_byte_header`,
`grpc_outline_uncompressed_message_has_compressed_flag_zero`,
`grpc_outline_compressed_message_has_compressed_flag_one`, `grpc_requests_a_length_prefixed_message_is_read_from_its_octets`,
`grpc_outline_message_length_uses_big_endian_4_bytes`, `grpc_outline_header_too_short_decode_fails`,
`grpc_outline_body_truncated_decode_fails`,
`grpc_outline_back_to_back_messages_can_be_decoded_sequentially`.

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

**Tests:** `crates/grpc-bridge/src/status.rs::tests::grpc_responses_the_status_codes_are_the_table_of_seventeen`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_responses_response_headers_constants_match_spec`.

**Status:** done — the status-code set + the required-response-header set
declared as constants.

### Status codes

**Spec:** `Status → "grpc-status" 1*DIGIT ; 0-9` with the canonical
values 0..=16 (OK/CANCELLED/UNKNOWN/.../UNAUTHENTICATED).

**Repo:** `crates/grpc-bridge/src/status.rs::Status` enum with
`code/from_code/is_ok/name` methods. All 17 values modeled.

**Tests:** `crates/grpc-bridge/src/status.rs::tests::grpc_responses_the_status_codes_are_the_table_of_seventeen`,
`crates/grpc-bridge/src/status.rs::tests::grpc_responses_unknown_codes_yield_none` (reject of code 17/255),
`crates/grpc-bridge/src/status.rs::tests::grpc_responses_ok_is_only_status_with_is_ok_true`.

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

**Tests:** `metadata::tests::{grpc_metadata_is_binary_header_recognizes_bin_suffix,
grpc_metadata_is_binary_header_rejects_text_headers, grpc_metadata_encode_base64_empty,
grpc_metadata_encode_base64_one_byte_pads_two, grpc_metadata_encode_base64_two_bytes_pads_one,
grpc_metadata_encode_base64_three_bytes_no_padding, grpc_metadata_encode_base64_known_vector,
grpc_metadata_base64_reads_the_vectors_of_rfc_4648, grpc_metadata_decode_base64_rejects_invalid_chars,
grpc_metadata_decode_base64_rejects_bad_padding_length,
grpc_metadata_encode_header_value_text_passes_ascii,
grpc_metadata_encode_header_value_text_rejects_non_ascii,
grpc_metadata_encode_header_value_bin_uses_base64,
grpc_metadata_a_bin_header_value_is_read_from_base64,
grpc_metadata_decode_header_value_text_passes_through}` (15 tests).

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

**Status:** done — its own no_std-capable implementation.

---

## protocol-web

### gRPC-Web trailers in an LPM frame

**Spec:** gRPC-Web encodes trailers in the Length-Prefixed-Message frame
with the Compressed-Flag MSB=1 (`0x80`).

**Repo:** `crates/grpc-bridge/src/frame.rs::encode_web_trailers`.

**Tests:** `frame::tests::grpc_web_web_trailers_encoded_with_msb_set`.

**Status:** done — the caller provides the trailers as UTF-8 bytes
(typically `grpc-status: 0\r\ngrpc-message: ...\r\n`).

### gRPC-Web base64 encoding (text subprotocol)

**Spec:** gRPC-Web has `application/grpc-web` (binary) and
`application/grpc-web-text` (base64-encoded).

**Repo:** `crates/grpc-bridge/src/metadata.rs::{encode_base64,
decode_base64}` (the RFC-4648 standard variant with padding) +
`content_types::GRPC_WEB_TEXT` as a marker.

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_empty`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_one_byte_pads_two`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_two_bytes_pads_one`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_three_bytes_no_padding`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_known_vector`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_base64_reads_the_vectors_of_rfc_4648`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_decode_base64_rejects_invalid_chars`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_decode_base64_rejects_bad_padding_length`.

**Status:** done — base64 subprotocol codec + content-type marker live.

---

## Audit status

11 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document grpc-protocol.md`):

* `cargo nextest run --profile merge -p zerodds-grpc-bridge --all-targets` (job `merge-rust-grpc-bridge`) — 46 tests green.
* `cargo nextest run --profile merge -p zerodds-hpack --all-targets` (job `merge-rust-hpack`) — 7 tests green.
* `cargo nextest run --profile merge -p zerodds-http2 --all-targets` (job `merge-rust-http2`) — 15 tests green.

68 cited tests green, 0 not green.

