# gRPC over HTTP/2 + gRPC-Web — Spec-Coverage

**Specs:** `internal/standards/cache/grpc/protocol-http2.md` +
`internal/standards/cache/grpc/protocol-web.md`.

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** gRPC ist die Cloud-RPC-Spec von Google. ZeroDDS
implementiert den gRPC-Stack verteilt über:

- `crates/grpc-bridge/` — gRPC-Wire-Elemente (LPM, Path, Status, Timeout)
- `crates/http2/` — eigene no_std-taugliche HTTP/2-Implementation (RFC 7540)
- `crates/hpack/` — HPACK-Header-Compression (RFC 7541)

Compression-Codecs (gzip/deflate) sind Caller-Layer.

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

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::grpc_requests_request_headers_constants_match_spec`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_requests_content_types_match_spec_strings`,
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_standard_grpc_path`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n`.

**Status:** done — alle Required-Header-Names + Content-Type-Set
als Konstanten ausgewiesen.

### Path-Parsing

**Spec:** `Path → ":path" "/" Service-Name "/" {method name}`.

**Repo:** `crates/grpc-bridge/src/path.rs::parse_path` mit Reject von
relative-paths, missing-method, empty-segments, method-mit-slash.

**Tests:** `path::tests::grpc_requests_parses_standard_grpc_path`,
`grpc_requests_rejects_relative_path`, `grpc_requests_rejects_path_without_method`,
`grpc_requests_rejects_empty_service`, `grpc_requests_rejects_empty_method`,
`grpc_requests_rejects_method_with_slash`, `grpc_requests_parses_dotted_service_name`.

**Status:** done

### Timeout Header (`grpc-timeout`)

**Spec:** `Timeout → "grpc-timeout" TimeoutValue TimeoutUnit`,
TimeoutValue MUST be at most 8 digits, Unit ∈ {H,M,S,m,u,n}.

**Repo:** `crates/grpc-bridge/src/timeout.rs::{TimeoutUnit,
encode_timeout, decode_timeout}` mit Reject von über-8-Digit, non-
digit, unbekannter-Unit, leerem Header.

**Tests:** `crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n`
(alle sechs Units), `crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_encodes_30_seconds`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_encodes_500_milliseconds`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_a_timeout_header_is_read_as_value_and_unit`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_rejects_value_above_8_digits_on_encode`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_unknown_unit`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_non_digit_value`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_empty`,
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_decode_rejects_only_unit_no_value`.

**Status:** done

### Length-Prefixed-Message

**Spec:** `Length-Prefixed-Message → Compressed-Flag Message-Length
Message`, Compressed-Flag = 1 byte, Message-Length = 4 byte BE.

**Repo:** `crates/grpc-bridge/src/frame.rs::{encode_message,
decode_message}` mit empty/short/large/back-to-back-Stream-Tests.

**Tests:** `frame::tests::grpc_outline_empty_message_encodes_to_5_byte_header`,
`grpc_outline_uncompressed_message_has_compressed_flag_zero`,
`grpc_outline_compressed_message_has_compressed_flag_one`, `grpc_requests_a_length_prefixed_message_is_read_from_its_octets`,
`grpc_outline_message_length_uses_big_endian_4_bytes`, `grpc_outline_header_too_short_decode_fails`,
`grpc_outline_body_truncated_decode_fails`,
`grpc_outline_back_to_back_messages_can_be_decoded_sequentially`.

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

**Tests:** `crates/grpc-bridge/src/status.rs::tests::grpc_responses_the_status_codes_are_the_table_of_seventeen`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_responses_response_headers_constants_match_spec`.

**Status:** done — Status-Code-Set + Required-Response-Header-Set
als Konstanten ausgewiesen.

### Status Codes

**Spec:** `Status → "grpc-status" 1*DIGIT ; 0-9` mit kanonischen
Werten 0..=16 (OK/CANCELLED/UNKNOWN/.../UNAUTHENTICATED).

**Repo:** `crates/grpc-bridge/src/status.rs::Status` Enum mit
`code/from_code/is_ok/name`-Methods. Alle 17 Werte modelliert.

**Tests:** `crates/grpc-bridge/src/status.rs::tests::grpc_responses_the_status_codes_are_the_table_of_seventeen`,
`crates/grpc-bridge/src/status.rs::tests::grpc_responses_unknown_codes_yield_none` (Reject von Code 17/255),
`crates/grpc-bridge/src/status.rs::tests::grpc_responses_ok_is_only_status_with_is_ok_true`.

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
grpc_metadata_decode_header_value_text_passes_through}` (15 Tests).

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

**Tests:** Frame-Layer — `crates/http2/src/frame.rs::tests::from_u8_round_trip`
(alle Frame-Type-Codes), `crates/http2/src/frame.rs::tests::encode_decode_round_trip`,
`crates/http2/src/frame.rs::tests::unknown_frame_type_rejected`,
`crates/http2/src/frame.rs::tests::r_bit_stripped_from_stream_id`,
`crates/http2/src/frame.rs::tests::frame_too_large_rejected`.
SETTINGS — `crates/http2/src/settings.rs::tests::defaults_match_spec`,
`crates/http2/src/settings.rs::tests::round_trip_encode_decode`,
`crates/http2/src/settings.rs::tests::unknown_setting_id_ignored`.
WINDOW_UPDATE — `crates/http2/src/flow.rs::tests::round_trip_window_update_codec`,
`crates/http2/src/flow.rs::tests::window_update_zero_rejected`,
`crates/http2/src/flow.rs::tests::window_update_overflow_rejected`.
RST_STREAM und Stream-Zustände —
`crates/http2/src/stream.rs::tests::idle_to_open_via_headers`,
`crates/http2/src/stream.rs::tests::reset_from_any_state_closes`,
`crates/http2/src/stream.rs::tests::open_recv_headers_stays_open_for_trailers`.
Connection-Preface — `crates/http2/src/preface.rs::tests::correct_preface_passes`.
HPACK — `crates/hpack/src/integer.rs::tests::rfc7541_c1_1_encoded_10_in_5bit_prefix`,
`crates/hpack/src/integer.rs::tests::rfc7541_c1_2_encoded_1337_in_5bit_prefix`,
`crates/hpack/src/integer.rs::tests::rfc7541_c1_3_encoded_42_in_8bit_prefix`,
`crates/hpack/src/decoder.rs::tests::rfc7541_c2_1_literal_with_indexing`,
`crates/hpack/src/decoder.rs::tests::round_trip_huffman_strings`,
`crates/hpack/src/decoder.rs::tests::dynamic_table_size_update_applied`,
`crates/hpack/src/decoder.rs::tests::invalid_index_zero_rejected`.

**Status:** done — eigene no_std-taugliche Implementierung.

---

## protocol-web

### gRPC-Web Trailers in LPM-Frame

**Spec:** gRPC-Web encoded Trailers im Length-Prefixed-Message-Frame
mit Compressed-Flag-MSB=1 (`0x80`).

**Repo:** `crates/grpc-bridge/src/frame.rs::encode_web_trailers`.

**Tests:** `frame::tests::grpc_web_web_trailers_encoded_with_msb_set`.

**Status:** done — Caller liefert die Trailers als UTF-8-Bytes
(typisch `grpc-status: 0\r\ngrpc-message: ...\r\n`).

### gRPC-Web Base64-Encoding (text-Subprotocol)

**Spec:** gRPC-Web hat `application/grpc-web` (binary) und
`application/grpc-web-text` (base64-encoded).

**Repo:** `crates/grpc-bridge/src/metadata.rs::{encode_base64,
decode_base64}` (RFC-4648 Standard-Variante mit Padding) +
`content_types::GRPC_WEB_TEXT` als Marker.

**Tests:** `crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_empty`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_one_byte_pads_two`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_two_bytes_pads_one`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_three_bytes_no_padding`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_encode_base64_known_vector`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_base64_reads_the_vectors_of_rfc_4648`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_decode_base64_rejects_invalid_chars`,
`crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_decode_base64_rejects_bad_padding_length`.

**Status:** done — Base64-Subprotocol-Codec + Content-Type-Marker
live.

---

## Audit-Status

11 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document grpc-protocol.md`):

* `cargo nextest run --profile merge -p zerodds-grpc-bridge --all-targets` (Job `merge-rust-grpc-bridge`) — 46 Tests grün.
* `cargo nextest run --profile merge -p zerodds-hpack --all-targets` (Job `merge-rust-hpack`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-http2 --all-targets` (Job `merge-rust-http2`) — 15 Tests grün.

68 zitierte Tests grün, 0 nicht grün.

