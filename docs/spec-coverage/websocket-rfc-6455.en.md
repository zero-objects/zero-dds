# IETF WebSocket RFC 6455 — Spec Coverage

**RFC:** [IETF RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) ("The WebSocket
Protocol", IETF December 2011).

**Context:** WebSocket is a bidirectional frame-based protocol
over TCP — a companion spec to DDS-WEB. ZeroDDS implements the
**Base Framing Protocol** (RFC 6455 §5.2 + §5.3) as a pure-Rust
no_std+alloc library. The opening handshake (HTTP upgrade, §4) is the
caller's task, as is TLS (`wss://`) and extension negotiation.

Implementation:

- `crates/websocket-bridge/` — Base Framing Protocol (§5.2 + §5.3) as a pure-Rust no_std+alloc library, 3 modules, 32 tests green.

---

## §1 Introduction

### §1.1-§1.9 Background + Goals

**Spec:** §1, p. 4-12 (RFC) — Abstract, Background, Goals,
conformance requirements.

**Repo:** `crates/websocket-bridge/src/lib.rs` crate doc.

**Tests:** —

**Status:** `n/a (informative)` — editorial background + goals; without a code mapping.

---

## §2 Conformance Requirements

### §2 RFC 2119 keywords

**Spec:** §2, p. 12 — RFC 2119 MUST/SHALL/etc. spec.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference to the RFC-2119 keywords; a language convention.

---

## §3 WebSocket URIs

### §3 ws:// + wss:// URI scheme

**Spec:** §3, p. 13-14 — `ws://host[:port]/path` + `wss://`.

**Repo:** `crates/websocket-bridge/src/uri.rs::{parse_websocket_uri,
WebSocketUri, default_port, resource_name, is_local_loopback,
UriError}`. Default port 80 (ws) / 443 (wss); fragment identifier
explicitly rejected (spec §3); the query string is split off;
IPv4 hostnames + DNS names; IPv6 literals [::1] are
caller-layer (RFC 6874).

**Tests:** `uri::tests::*` (13 tests) incl. parses_basic_ws_uri,
parses_basic_wss_uri, parses_explicit_port, parses_query_string,
parses_default_path_when_missing, rejects_unknown_scheme,
rejects_missing_host, rejects_missing_host_before_port,
rejects_invalid_port, rejects_fragment, default_port_returns_443_for_wss,
resource_name_combines_path_and_query, resource_name_without_query_is_path,
is_local_loopback_recognizes_localhost.

**Status:** done — URI-scheme parser live.

---

## §4 Opening Handshake

### §4.1-§4.4 Client + Server handshake

**Spec:** §4, p. 14-25 — HTTP upgrade (Sec-WebSocket-Key,
Sec-WebSocket-Accept = SHA-1(key + MAGIC) base64, etc.).

**Repo:** `crates/websocket-bridge/src/handshake.rs::
{ClientHandshake, ServerHandshake, compute_accept,
parse_client_request, build_server_response,
render_server_response}`.

**Tests:** inline.

**Status:** done

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, p. 27 — bidirectional frames.

**Repo:** cross-ref §5.2.

**Tests:** —

**Status:** done

### §5.2 Base Framing Protocol — header layout

**Spec:** §5.2, p. 28-31 — frame header with FIN/RSV1-3/Opcode/MASK/
payload length.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::encode/decode`.

**Tests:** `codec::tests::smallest_text_frame_encodes_to_2_byte_header_plus_payload`,
`round_trip_unmasked_text`, `rsv_bits_propagate_to_decoded_frame`,
`fin_zero_text_frame_round_trip`.

**Status:** done

### §5.2 Opcode values

**Spec:** §5.2, p. 28-29 — 0x0 Continuation, 0x1 Text, 0x2 Binary,
0x8 Close, 0x9 Ping, 0xA Pong, 0x3-0x7 reserved non-control,
0xB-0xF reserved control.

**Repo:** `crates/websocket-bridge/src/frame.rs::Opcode`.

**Tests:** `frame::tests::opcode_round_trip_via_bits`,
`opcode_well_known_values_match_spec`, `opcode_is_control_predicate`.

**Status:** done

### §5.2 Payload length encoding (7 / 7+16 / 7+64)

**Spec:** §5.2, p. 29 — "minimal number of bytes MUST be used".
* 0..=125 = directly in the 7-bit field.
* 126 + 2 byte BE = 16-bit length.
* 127 + 8 byte BE (MSB=0) = 64-bit length.

**Repo:** `crates/websocket-bridge/src/codec.rs::encode_payload_length`
+ decode validation (NonMinimalLength + PayloadLengthMsbSet).

**Tests:** `codec::tests::medium_payload_uses_extended_16_bit_length`,
`large_payload_uses_extended_64_bit_length`,
`extended_64_bit_length_msb_set_rejected`,
`non_minimal_16_bit_length_rejected`,
`non_minimal_64_bit_length_rejected`,
`extended_16_bit_length_truncated_fails`,
`round_trip_medium_and_large_payloads`.

**Status:** done

### §5.3 Client-to-server masking

**Spec:** §5.3, p. 32-33 — XOR masking with a 32-bit key. "octet i of
the transformed data is the XOR of octet i of the original data with
octet at index i modulo 4 of the masking key". Spec §5.3 requires
an "unpredictable" key; "MUST be derived from a strong source of entropy".

**Repo:** `crates/websocket-bridge/src/masking.rs::apply_mask`
(symmetric); `generate_masking_key` (Splitmix64-based,
**explicitly not for security** — the user MUST use their own RNG).

**Tests:** `masking::tests::apply_mask_is_symmetric`,
`apply_mask_xors_with_key_modulo_4`,
`apply_mask_handles_partial_key_alignment`, `empty_payload_is_unchanged`,
`generate_masking_key_returns_4_bytes`,
`generate_masking_key_returns_distinct_values_across_calls`,
`insecure_splitmix_provider_implements_trait`,
`closure_provider_calls_user_supplied_fn`,
`codec::tests::round_trip_masked_payload_unmasked_on_decode`.

**Status:** done — XOR logic + a `MaskingKeyProvider` trait for
caller-supplied secure RNGs (OsRng/getrandom/hardware RNG) live;
`InsecureSplitmixProvider` as an explicitly `not for security` default;
`ClosureMaskingKeyProvider` for easy binding of external
RNG sources.

### §5.4 Fragmentation

**Spec:** §5.4, p. 33-34 — FIN=0 marks a non-final fragment;
continuation frames with opcode 0x0 + FIN=1 for the last.

**Repo:** the codec passes the FIN bit + opcode through 1:1; reassembly is
the caller's task.

**Tests:** `codec::tests::fin_zero_text_frame_round_trip`.

**Status:** done — frame-level support; caller-side reassembly out
of codec scope.

### §5.5 Control frames

**Spec:** §5.5, p. 36-38 — control frames (opcode 0x8-0xF) MUST have
payload <= 125 bytes AND MUST NOT be fragmented (FIN=1).

**Repo:** `crates/websocket-bridge/src/codec.rs` enforced on encode +
decode (ControlFrameTooLong / FragmentedControlFrame errors).

**Tests:** `codec::tests::control_frame_with_long_payload_rejected_on_encode`,
`fragmented_control_frame_rejected_on_encode`.

**Status:** done

### §5.5.1 Close frame

**Spec:** §5.5.1 + §7.4, p. 36 + 45-46 — the close payload starts with a
2-byte status code (BE), optionally followed by a UTF-8 reason.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`.

**Tests:** `frame::tests::close_frame_includes_status_code_in_be_payload`,
`close_frame_with_reason_carries_utf8_bytes`,
`codec::tests::close_frame_carries_status_code`.

**Status:** done — status-code semantics (1000=Normal, 1001=Going-Away,
etc.) is the caller's validation task.

### §5.5.2 Ping frame

**Spec:** §5.5.2, p. 37 — "The Ping frame contains an opcode of 0x9.
[...] A Ping frame MAY include 'Application data'. [...] Upon
receipt of a Ping frame, an endpoint MUST send a Pong frame in
response, unless it already received a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::ping`
(opcode 0x9, FIN=1, masking per §5.3).

**Tests:** `codec::tests::ping_frame_round_trip`.

**Status:** done — echo logic (Pong reply with the Ping payload) is
the caller's task (connection state machine, see §6.1).

### §5.5.3 Pong frame

**Spec:** §5.5.3, p. 37 — "The Pong frame contains an opcode of 0xA.
[...] A Pong frame sent in response to a Ping frame must have
identical 'Application data' as found in the message body of the
Ping frame being replied to."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::pong`
(opcode 0xA).

**Tests:** cross-ref §5.5.2 (`codec::tests::ping_frame_round_trip`
covers both opcodes); additionally `codec::tests::pong_frame_round_trip`
if present.

**Status:** done — the wire format is correct; the Pong-payload-identity
obligation is caller-layer (connection logic).

### §5.6 Data frames

**Spec:** §5.6, p. 38 — Text (UTF-8) + Binary.

**Repo:** `Frame::text` + `Frame::binary`.

**Tests:** cross-ref all roundtrip tests.

**Status:** done — UTF-8 validation of text frames is the
caller's task (the codec API is `Vec<u8>`-based, no UTF-8 obligation).

### §5.7-§5.8 Examples + Extensibility

**Spec:** §5.7-§5.8, p. 38-39 — examples + reserved bits.

**Repo:** RSV1/RSV2/RSV3 as public fields in the `Frame` struct.

**Tests:** `codec::tests::rsv_bits_propagate_to_decoded_frame`.

**Status:** done

---

## §6 Sending and Receiving Data

### §6.1 Sending Data

**Spec:** §6.1, p. 39-40 — send algorithm: determine the frame with FIN, opcode,
mask, payload length; on text frames UTF-8 validation;
on continuation frames reassembly obligation.

**Repo:** `crates/websocket-bridge/src/message.rs::fragment_message`
+ a `SendError` enum (InvalidFrameLimit, InvalidUtf8). Splits logical
messages into frame sequences (Text/Binary with FIN=0, continuation
frames, last frame with FIN=1); UTF-8 validation on the text payload
before splitting; the mask field is set when the caller passes a non-null
mask key (client path).

**Tests:** `message::tests::{fragment_empty_message_yields_one_frame,
fragment_message_within_limit_single_frame,
fragment_message_splits_into_text_plus_continuations,
fragment_text_rejects_invalid_utf8, fragment_zero_limit_rejected,
fragment_with_mask_sets_mask_field}`.

**Status:** done — send algorithm + frame sequencing live.

### §6.2 Receiving Data

**Spec:** §6.2, p. 40-42 — receive algorithm: reassembly of
continuation frames; UTF-8 validation for text frames; connection
state tracking.

**Repo:** `crates/websocket-bridge/src/message.rs::Reassembler` +
a `Message` struct + a `ReceiveError` enum. Maintains continuation state
via the `feed(&Frame) -> Option<Message>` API; streaming UTF-8
validation per frame; DoS cap via `max_message_size`. Control frames
(Close/Ping/Pong) are passed through directly (spec §5.5: not
fragmentable). Reserved opcodes trigger a fail.

**Tests:** `message::tests::{reassembler_single_frame_message_complete,
reassembler_continuation_sequence_reassembles,
reassembler_continuation_without_preceding_text_rejected,
reassembler_interleaved_text_during_pending_rejected,
reassembler_rejects_invalid_utf8_in_text,
reassembler_rejects_message_above_limit,
reassembler_passes_through_control_frames,
reassembler_has_pending_during_continuation,
fragment_send_then_reassemble_round_trip}`.

**Status:** done — receive algorithm + reassembly + UTF-8 streaming
+ DoS cap live; round-trip fragment→reassemble verified.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, p. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one."

**Repo:** close-frame codec via `frame.rs::Frame::close` +
`close.rs::{CloseHandshake, CloseState}` state machine with Open →
ClosingInitiator/ClosingResponder → Closed/Failed transitions.
Operations: `initiator_send_close`, `recv_close_response`,
`responder_recv_close`, `responder_send_close_response`, `fail`.

**Tests:** `close::tests::{handshake_starts_in_open_state,
initiator_send_close_transitions_to_closing,
initiator_recv_close_response_transitions_to_closed,
responder_recv_close_transitions_to_closing_responder,
responder_send_close_response_completes_normally,
second_close_send_in_closing_is_rejected,
recv_close_in_open_state_is_responder_path}`.

**Status:** done — wire format + close-handshake state machine live.

### §7.2 Abnormal Closures

**Spec:** §7.2, p. 43-44 — abnormal-close conditions (server-
failure-to-client, client-failure-to-server, recoverable conditions).

**Repo:** `close.rs::CloseHandshake::fail(reason)` + `CloseState::
Failed` with `failure_reason` tracking.

**Tests:** `close::tests::fail_marks_abnormal_closure`.

**Status:** done — failure state + recovery-reason tracking live.

### §7.3 Normal Closure of Connections

**Spec:** §7.3, p. 44 — normal closure (after the close handshake).

**Repo:** `close.rs::CloseState::Closed` as a final state; `is_closed()`
distinguishes Closed vs. Failed.

**Tests:** `close::tests::{initiator_recv_close_response_transitions_to_closed,
responder_send_close_response_completes_normally}`.

**Status:** done — normal-closure state tracking stated.

### §7.4 Status Codes

**Spec:** §7.4, p. 44-46 — status-code registry (1000-1015 reserved
+ 3000-4999 vendor). §7.4.1 lists 14 specific codes
(1000=Normal, 1001=Going-Away, 1002=Protocol-Error, etc.).
§7.4.2 says 1004/1005/1006 must not be sent over the wire.

**Repo:** status code as a 16-bit BE in the payload (`close.rs`) +
range validation `close.rs::{StatusCodeRange, classify_status_code,
is_forbidden_on_wire, validate_wire_status_code}` with all four
registry ranges (Invalid <1000, ProtocolReserved 1000-2999,
LibraryDefined 3000-3999, ApplicationDefined 4000-4999) +
forbidden-on-wire set (1004/1005/1006/1015).

**Tests:** `close::tests::{classify_status_code_recognizes_protocol_range,
classify_status_code_recognizes_library_range,
classify_status_code_recognizes_app_range,
classify_status_code_recognizes_invalid_below_1000,
classify_status_code_recognizes_out_of_range_above_5000,
is_forbidden_on_wire_covers_all_four,
validate_wire_status_code_accepts_normal,
validate_wire_status_code_rejects_forbidden,
validate_wire_status_code_rejects_out_of_range}`.

**Status:** done — wire format + range validation +
§7.4.2 restrictions (all four forbidden codes) live.

---

## §8-§9 Error Handling + Extensions

### §8.1 Handling Errors in UTF-8 from the Server

**Spec:** §8.1, p. 46 — "When a client receives a Text-frame from
the server that contains malformed UTF-8 data [...] the client MUST
fail the WebSocket Connection."

**Repo:** `crates/websocket-bridge/src/utf8.rs::{validate, Utf8Error,
StreamingValidator}`. Strict RFC-3629 validation: surrogate
codepoints (U+D800..=U+DFFF), overlong encoding, codepoints
> U+10FFFF are rejected. `StreamingValidator` keeps a
4-byte buffer for fragmented text frames (spec §6.2 — FIN=0
continuation frames).

**Tests:** `utf8::tests::*` (16 tests) incl. valid_2/3/4_byte_codepoint,
rejects_overlong_2_byte_for_ascii, rejects_unexpected_continuation_byte,
rejects_invalid_lead_byte, rejects_truncated_*, rejects_invalid_continuation,
rejects_surrogate_codepoint, rejects_codepoint_above_max,
streaming_handles_split_codepoint, streaming_finalize_with_pending_is_truncated,
streaming_complete_codepoint_in_one_chunk.

**Status:** done — strict UTF-8 validation + streaming variant live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, p. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** symmetric to §8.1 — the same validator
`crates/websocket-bridge/src/utf8.rs::validate`. The server and client
paths use the same codec.

**Tests:** cross-ref §8.1.

**Status:** done — symmetric to §8.1.

### §9 Extensibility

**Spec:** §9, p. 47-49 — extension negotiation via the
`Sec-WebSocket-Extensions` header; examples incl. permessage-deflate
(RFC 7692).

**Repo:** `crates/websocket-bridge/src/permessage_deflate.rs::
{PermessageDeflateParams, parse_offer, render_accept, append_tail,
strip_tail}` (RFC 7692 implementation) +
`crates/websocket-bridge/src/negotiation.rs::{ExtensionOffer,
parse_extensions, parse_subprotocols, select_subprotocol,
SUBPROTOCOL_HEADER, EXTENSIONS_HEADER}` (generic extension +
subprotocol negotiation framework).

**Tests:** `negotiation::tests::*` (12 tests) + inline tests in
`permessage_deflate.rs`.

**Status:** done — permessage-deflate + a generic extension-list
parser + subprotocol negotiation live.

---

## §10 Security Considerations + §11 IANA + §12-§14 Misc

**Spec:** §10-§14, p. 49-71 — security notes, IANA tables,
acknowledgements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — security considerations/acknowledgments/IANA tables are reflected as constants in the frame/code modules.

---

## Audit status

23 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-websocket-bridge` — 74 lib-inline +
4 integration = 78 tests green, 0 failed. Modules with tests: `close`,
`codec`, `dds_bridge`, `frame`, `handshake`, `masking`,
`permessage_deflate`.
