# IETF WebSocket RFC 6455 — Spec Coverage

**RFC:** [IETF RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) ("The WebSocket
Protocol", IETF December 2011).

**Context:** WebSocket is a bidirectional frame-based protocol
over TCP — a companion spec to DDS-WEB. ZeroDDS implements the
**Base Framing Protocol** (RFC 6455 §5.2 + §5.3) as a pure-Rust
no_std+alloc library. The opening handshake (HTTP upgrade, §4) is the
caller's task, as is TLS (`wss://`) and extension negotiation.

Implementation:

- `crates/websocket-bridge/` — Base Framing Protocol (§5.2 + §5.3), handshake (§4), message layer (§6), close state machine (§7) and extension negotiation (§9) as a pure-Rust no_std+alloc library.

---

## §1 Introduction

### §1.1-§1.9 Background + Goals

**Spec:** §1, p. 4-12 (RFC) — Abstract, Background, Goals,
conformance requirements.

**Repo:** `crates/websocket-bridge/src/lib.rs` crate doc.

**Tests:** —

**Status:** n/a (informative) — every section §1.1 to §1.9 carries "_This section is non-normative._" in the RFC.

---

## §2 Conformance Requirements

### §2 RFC 2119 keywords

**Spec:** §2, p. 12 — RFC 2119 MUST/SHALL/etc. spec.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reading of the keywords and conformance rules for algorithms; no requirement of its own on the protocol behaviour.

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

**Tests:** `crates/websocket-bridge/src/uri.rs::tests::{ws_3_parses_basic_ws_uri,
ws_3_parses_basic_wss_uri, ws_3_parses_explicit_port, ws_3_parses_query_string,
ws_3_parses_default_path_when_missing, ws_3_rejects_unknown_scheme,
ws_3_rejects_missing_host, ws_3_rejects_missing_host_before_port,
ws_3_rejects_invalid_port, ws_3_rejects_fragment, ws_3_default_port_returns_443_for_wss,
ws_3_resource_name_combines_path_and_query, ws_3_resource_name_without_query_is_path,
ws_3_is_local_loopback_recognizes_localhost}` (14).

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

**Tests:** `crates/websocket-bridge/src/handshake.rs::tests::{ws_4_1_rfc6455_section_1_3_accept_test_vector,
ws_4_1_parses_minimal_client_handshake, ws_4_1_parses_protocols_and_extensions,
ws_4_1_rejects_missing_upgrade, ws_4_1_rejects_wrong_version, ws_4_1_rejects_missing_key,
ws_4_1_server_response_includes_accept,
ws_4_1_server_agrees_to_no_extension_it_cannot_perform,
ws_4_1_server_selects_no_subprotocol_it_does_not_support,
ws_4_1_render_server_response_format, ws_4_1_base64_round_trip_known_vectors,
ws_4_1_sha1_known_vector_abc, ws_4_1_connection_header_with_keep_alive_still_detects_upgrade}`
— `ws_4_1_rfc6455_section_1_3_accept_test_vector` checks the Sec-WebSocket-Accept
test vector of the spec (SHA-1 over key + GUID, base64).

**Status:** done

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, p. 27 — "The client MUST mask all frames that it sends to the
server. [...] The server MUST close the connection upon receiving a frame that
is not masked. [...] A server MUST NOT mask any frames that it sends to the
client. A client MUST close a connection if it detects a masked frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{Role, MaskingViolation,
validate_masking}` — checks a received frame against the directional rule and
supplies close code 1002 for a violation (`MaskingViolation::close_code`).

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`,
`crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`.

**Status:** done

### §5.2 Base Framing Protocol — header layout

**Spec:** §5.2, p. 28-31 — frame header with FIN/RSV1-3/Opcode/MASK/
payload length.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::encode/decode`.

**Tests:** `codec::tests::ws_5_2_smallest_text_frame_encodes_to_2_byte_header_plus_payload`,
`ws_5_7_a_single_frame_text_message_unmasked_and_masked`, `ws_5_2_rsv_bits_propagate_to_decoded_frame`,
`ws_5_7_a_fragmented_text_message`.

**Status:** done

### §5.2 Opcode values

**Spec:** §5.2, p. 28-29 — 0x0 Continuation, 0x1 Text, 0x2 Binary,
0x8 Close, 0x9 Ping, 0xA Pong, 0x3-0x7 reserved non-control,
0xB-0xF reserved control.

**Repo:** `crates/websocket-bridge/src/frame.rs::Opcode`.

**Tests:** `frame::tests::ws_5_2_the_opcodes_are_the_numbers_of_the_rfc`,
`ws_5_2_opcode_well_known_values_match_spec`, `ws_5_2_opcode_is_control_predicate`.

**Status:** done

### §5.2 Payload length encoding (7 / 7+16 / 7+64)

**Spec:** §5.2, p. 29 — "minimal number of bytes MUST be used".
* 0..=125 = directly in the 7-bit field.
* 126 + 2 byte BE = 16-bit length.
* 127 + 8 byte BE (MSB=0) = 64-bit length.

**Repo:** `crates/websocket-bridge/src/codec.rs::encode_payload_length`
+ decode validation (NonMinimalLength + PayloadLengthMsbSet).

**Tests:** `codec::tests::ws_5_2_medium_payload_uses_extended_16_bit_length`,
`ws_5_2_large_payload_uses_extended_64_bit_length`,
`ws_5_2_extended_64_bit_length_msb_set_rejected`,
`ws_5_2_non_minimal_16_bit_length_rejected`,
`ws_5_2_non_minimal_64_bit_length_rejected`,
`ws_5_2_extended_16_bit_length_truncated_fails`,
`ws_5_7_a_256_and_a_65536_octet_binary_frame`.

**Status:** done

### §5.3 Client-to-server masking

**Spec:** §5.3, p. 32-33 — XOR masking with a 32-bit key. "octet i of
the transformed data is the XOR of octet i of the original data with
octet at index i modulo 4 of the masking key". Spec §5.3 requires
an "unpredictable" key; "MUST be derived from a strong source of entropy".

**Repo:** `crates/websocket-bridge/src/masking.rs::apply_mask`
(symmetric); `generate_masking_key` (Splitmix64-based,
**explicitly not for security** — the user MUST use their own RNG).

**Tests:** `masking::tests::ws_5_3_apply_mask_is_symmetric`,
`ws_5_3_apply_mask_xors_with_key_modulo_4`,
`ws_5_3_apply_mask_handles_partial_key_alignment`, `ws_5_3_empty_payload_is_unchanged`,
`ws_5_3_generate_masking_key_returns_4_bytes`,
`ws_5_3_generate_masking_key_returns_distinct_values_across_calls`,
`ws_5_3_insecure_splitmix_provider_implements_trait`,
`ws_5_3_closure_provider_calls_user_supplied_fn`,
`codec::tests::ws_5_7_a_single_frame_text_message_unmasked_and_masked`.

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

**Tests:** `codec::tests::ws_5_7_a_fragmented_text_message`.

**Status:** done — frame-level support; caller-side reassembly out
of codec scope.

### §5.5 Control frames

**Spec:** §5.5, p. 36-38 — control frames (opcode 0x8-0xF) MUST have
payload <= 125 bytes AND MUST NOT be fragmented (FIN=1).

**Repo:** `crates/websocket-bridge/src/codec.rs` enforced on encode +
decode (ControlFrameTooLong / FragmentedControlFrame errors).

**Tests:** `codec::tests::ws_5_5_control_frame_with_long_payload_rejected_on_encode`,
`ws_5_5_fragmented_control_frame_rejected_on_encode`.

**Status:** done

### §5.5.1 Close frame

**Spec:** §5.5.1 + §7.4, p. 36 + 45-46 — the close payload starts with a
2-byte status code (BE), optionally followed by a UTF-8 reason.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`.

**Tests:** `frame::tests::ws_5_5_1_close_frame_includes_status_code_in_be_payload`,
`ws_5_5_1_close_frame_with_reason_carries_utf8_bytes`,
`codec::tests::ws_5_5_1_close_frame_carries_status_code`.

**Status:** done — status-code semantics (1000=Normal, 1001=Going-Away,
etc.) is the caller's validation task.

### §5.5.2 Ping frame

**Spec:** §5.5.2, p. 37 — "The Ping frame contains an opcode of 0x9.
[...] A Ping frame MAY include 'Application data'. [...] Upon
receipt of a Ping frame, an endpoint MUST send a Pong frame in
response, unless it already received a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::ping`
(opcode 0x9, FIN=1, masking per §5.3).

**Tests:** `codec::tests::ws_5_7_an_unmasked_ping_and_the_masked_pong_with_its_body`.

**Status:** done — echo logic (Pong reply with the Ping payload) is
the caller's task (connection state machine, see §6.1).

### §5.5.3 Pong frame

**Spec:** §5.5.3, p. 37 — "The Pong frame contains an opcode of 0xA.
[...] A Pong frame sent in response to a Ping frame must have
identical 'Application data' as found in the message body of the
Ping frame being replied to."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::pong`
(opcode 0xA).

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::ws_5_7_an_unmasked_ping_and_the_masked_pong_with_its_body`.

**Status:** done — wire format + Ping->Pong payload identity shown; when a Pong
is sent is decided by the caller's connection state machine.

### §5.6 Data frames

**Spec:** §5.6, p. 38 — "Data frames (e.g., non-control frames) are identified
by opcodes where the most significant bit of the opcode is 0." Text frame
(0x1): the payload is UTF-8 text; binary frame (0x2): the payload is arbitrary
binary data.

**Repo:** `crates/websocket-bridge/src/frame.rs::{Frame, Opcode}` —
`Frame::text`/`Frame::binary`; `Opcode::is_control` separates data from control
frames via the MSB.

**Tests:** `crates/websocket-bridge/src/frame.rs::tests::{ws_5_6_data_frame_opcodes_have_msb_zero_per_spec_5_6,
ws_5_6_text_and_binary_data_frames_carry_their_payload_unchanged,
ws_5_6_text_frame_constructor_sets_fin_and_opcode}`;
`crates/websocket-bridge/src/codec.rs::tests::{ws_5_7_a_single_frame_text_message_unmasked_and_masked,
ws_5_7_a_256_and_a_65536_octet_binary_frame}`.

**Status:** done — UTF-8 validation of text frames is the
caller's task (the codec API is `Vec<u8>`-based, no UTF-8 obligation).

### §5.7-§5.8 Examples + Extensibility

**Spec:** §5.7-§5.8, p. 38-39 — examples + reserved bits.

**Repo:** RSV1/RSV2/RSV3 as public fields in the `Frame` struct.

**Tests:** `codec::tests::ws_5_2_rsv_bits_propagate_to_decoded_frame`.

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

**Tests:** `message::tests::{ws_6_1_fragment_empty_message_yields_one_frame,
ws_6_1_fragment_message_within_limit_single_frame,
ws_6_1_fragment_message_splits_into_text_plus_continuations,
ws_6_1_fragment_text_rejects_invalid_utf8, ws_6_1_fragment_zero_limit_rejected,
ws_6_1_fragment_with_mask_sets_mask_field}`.

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

**Tests:** `message::tests::{ws_6_2_reassembler_single_frame_message_complete,
ws_6_2_reassembler_continuation_sequence_reassembles,
ws_6_2_reassembler_continuation_without_preceding_text_rejected,
ws_6_2_reassembler_interleaved_text_during_pending_rejected,
ws_6_2_reassembler_rejects_invalid_utf8_in_text,
ws_6_2_reassembler_rejects_message_above_limit,
ws_6_2_reassembler_passes_through_control_frames,
ws_6_2_reassembler_has_pending_during_continuation,
ws_6_2_fragment_send_then_reassemble_round_trip}`.

**Status:** done — receive algorithm + reassembly + UTF-8 streaming
+ DoS cap live; round-trip fragment→reassemble verified.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, p. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one."

**Repo:** close-frame codec via `crates/websocket-bridge/src/frame.rs::Frame::close` +
`close.rs::{CloseHandshake, CloseState}` state machine with Open →
ClosingInitiator/ClosingResponder → Closed/Failed transitions.
Operations: `initiator_send_close`, `recv_close_response`,
`responder_recv_close`, `responder_send_close_response`, `fail`.

**Tests:** `close::tests::{ws_7_1_handshake_starts_in_open_state,
ws_7_1_initiator_send_close_transitions_to_closing,
ws_7_1_initiator_recv_close_response_transitions_to_closed,
ws_7_1_responder_recv_close_transitions_to_closing_responder,
ws_7_1_responder_send_close_response_completes_normally,
ws_7_1_second_close_send_in_closing_is_rejected,
ws_7_1_recv_close_in_open_state_is_responder_path}`.

**Status:** done — wire format + close-handshake state machine live.

### §7.2 Abnormal Closures

**Spec:** §7.2, p. 43-44 — abnormal-close conditions (server-
failure-to-client, client-failure-to-server, recoverable conditions).

**Repo:** `close.rs::CloseHandshake::fail(reason)` + `CloseState::
Failed` with `failure_reason` tracking.

**Tests:** `close::tests::ws_7_2_fail_marks_abnormal_closure`.

**Status:** done — failure state + recovery-reason tracking live.

### §7.3 Normal Closure of Connections

**Spec:** §7.3, p. 44 — normal closure (after the close handshake).

**Repo:** `close.rs::CloseState::Closed` as a final state; `is_closed()`
distinguishes Closed vs. Failed.

**Tests:** `crates/websocket-bridge/src/close.rs::tests::ws_7_1_initiator_recv_close_response_transitions_to_closed`,
`crates/websocket-bridge/src/close.rs::tests::ws_7_1_responder_send_close_response_completes_normally`.

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

**Tests:** `close::tests::{ws_7_4_classify_status_code_recognizes_protocol_range,
ws_7_4_classify_status_code_recognizes_library_range,
ws_7_4_classify_status_code_recognizes_app_range,
ws_7_4_classify_status_code_recognizes_invalid_below_1000,
ws_7_4_classify_status_code_recognizes_out_of_range_above_5000,
ws_7_4_is_forbidden_on_wire_covers_all_four,
ws_7_4_validate_wire_status_code_accepts_normal,
ws_7_4_validate_wire_status_code_rejects_forbidden,
ws_7_4_validate_wire_status_code_rejects_out_of_range}`.

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

**Tests:** `crates/websocket-bridge/src/utf8.rs::tests::{ws_8_1_empty_is_valid,
ws_8_1_ascii_is_valid, ws_8_1_valid_2_byte_codepoint, ws_8_1_valid_3_byte_codepoint,
ws_8_1_valid_4_byte_codepoint, ws_8_1_rejects_overlong_2_byte_for_ascii,
ws_8_1_rejects_unexpected_continuation_byte, ws_8_1_rejects_invalid_lead_byte,
ws_8_1_rejects_truncated_2_byte, ws_8_1_rejects_truncated_3_byte,
ws_8_1_rejects_invalid_continuation, ws_8_1_rejects_surrogate_codepoint,
ws_8_1_rejects_codepoint_above_max, ws_8_1_streaming_handles_split_codepoint,
ws_8_1_streaming_finalize_with_pending_is_truncated,
ws_8_1_streaming_complete_codepoint_in_one_chunk}` (16); the fail-the-connection path
with close code 1007:
`crates/websocket-bridge/src/message.rs::tests::ws_8_1_client_fails_connection_with_1007_on_malformed_utf8_from_server`.

**Status:** done — strict UTF-8 validation + streaming variant + the
close-code-1007 path live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, p. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** the same validator `crates/websocket-bridge/src/utf8.rs::validate`
for both directions; the fail-the-connection behaviour in
`crates/websocket-bridge/src/message.rs::ReceiveError::{close_code,
close_frame}` maps the UTF-8 error to close code 1007
(`CloseCode::InvalidPayloadData`, spec §7.4.1).

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_8_2_server_fails_connection_with_1007_on_malformed_utf8_from_client,
ws_8_2_malformed_utf8_split_across_continuation_frames_still_yields_1007,
ws_8_2_receive_error_close_codes_match_spec_7_4_1}`.

**Status:** done — a masked client text frame with malformed UTF-8 closes the
connection with 1007, across continuation frames too.

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

**Tests:** `crates/websocket-bridge/src/negotiation.rs::tests::{ws_9_parse_extensions_single,
ws_9_parse_extensions_with_param, ws_9_parse_extensions_with_value_param,
ws_9_parse_extensions_strips_quoted_value, ws_9_parse_extensions_multiple_offers,
ws_9_parse_extensions_empty_returns_empty, ws_9_parse_subprotocols_basic,
ws_9_parse_subprotocols_empty_returns_empty, ws_9_select_subprotocol_picks_first_match,
ws_9_select_subprotocol_returns_none_when_no_match,
ws_9_select_subprotocol_is_case_insensitive, ws_9_header_constants_match_spec}` (12);
`crates/websocket-bridge/src/permessage_deflate.rs::tests::{ws_9_parse_no_params_yields_defaults,
ws_9_parse_no_takeover_flags, ws_9_parse_window_bits, ws_9_rejects_invalid_window_bits,
ws_9_rejects_unknown_param, ws_9_rejects_boolean_with_value,
ws_9_render_default_is_bare_extension_name, ws_9_render_includes_params,
ws_9_tail_round_trip, ws_9_strip_tail_no_op_when_absent,
ws_9_parameterless_client_max_window_bits_accepted}` (11).

**Status:** done — permessage-deflate + a generic extension-list
parser + subprotocol negotiation live.

---

## §10-§14 Security + IANA + Misc

### §10.1, §10.5, §10.8 Non-browser clients, client authentication, SHA-1

**Spec:** §10.1, §10.5, §10.8, pp. 50–54 (RFC) — notes on clients outside a browser, on authentication via HTTP mechanisms and on SHA-1 in the handshake having no security function.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — explanations without a requirement of their own.

### §10.2 Verify the origin

**Spec:** §10.2, p. 50 (RFC) — "Servers that are not intended to process input from any web page but only for certain sites SHOULD verify the |Origin| field is an origin they expect. If the origin indicated is unacceptable to the server, then it SHOULD respond to the WebSocket handshake with a reply containing HTTP 403 Forbidden status code."

**Repo:** —

**Tests:** —

**Status:** open — the handshake (`crates/websocket-bridge/src/handshake.rs`) and the daemon do not check the `Origin` field.

### §10.3 A new, unpredictable masking key per frame

**Spec:** §10.3, p. 51 (RFC) — "Clients MUST choose a new masking key for each frame, using an algorithm that cannot be predicted by end applications that provide data."

**Repo:** `crates/websocket-bridge/src/masking.rs` (`generate_masking_key`)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_3_generate_masking_key_returns_distinct_values_across_calls`, `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`

**Status:** partial — every frame gets a new key; the test checks that consecutive keys differ. No test checks the source of the random numbers; the module also offers a predictable provider (`ws_5_3_insecure_splitmix_provider_implements_trait`).

### §10.4 Limits on frame and message size

**Spec:** §10.4, p. 52 (RFC) — "Implementations that have implementation- and/or platform-specific limitations regarding the frame size or total message size after reassembly from multiple frames MUST protect themselves against exceeding those limits. [...] Such an implementation SHOULD impose a limit on frame sizes and the total message size after reassembly from multiple frames."

**Repo:** `crates/websocket-bridge/src/message.rs` (`Reassembler::with_limit`, `max_message_size`)

**Tests:** `crates/websocket-bridge/src/message.rs::tests::ws_6_2_reassembler_rejects_message_above_limit`

**Status:** partial — the total size after reassembly is limited and tested; no test checks a limit on the size of a single frame (e.g. an announced length of 2^60).

### §10.6 Support TLS

**Spec:** §10.6, p. 53 (RFC) — "WebSocket implementations MUST support TLS and SHOULD employ it when communicating with their peers."

**Repo:** `crates/websocket-bridge/src/daemon/security.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`, `crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`

**Status:** done

### §10.7 Invalid data

**Spec:** §10.7, p. 53 (RFC) — "Incoming data MUST always be validated by both clients and servers." After the handshake "the endpoint SHOULD send a Close frame with an appropriate status code (Section 7.4)"; during the handshake "the server SHOULD return an appropriate HTTP [RFC2616] status code."

**Repo:** `crates/websocket-bridge/src/codec.rs`, `crates/websocket-bridge/src/masking.rs`, `crates/websocket-bridge/src/utf8.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`

**Status:** partial — a masking violation closes with 1002, an invalid login in the handshake gets HTTP 401. Not checked one by one: the close code for every other kind of invalid data after the handshake.

### §11 IANA considerations

**Spec:** §11, pp. 54–66 (RFC) — registration of the URI schemes `ws` and `wss`, the upgrade token, the header fields and the registries for extensions, subprotocols, versions, close codes and opcodes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — instructions to IANA; the values themselves are items for §3, §4, §5.2 and §7.4.

### §12 Using the WebSocket protocol from other specifications

**Spec:** §12, p. 66 (RFC) — how another specification establishes and uses a WebSocket connection.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — addressed to other specifications.

### §13, §14 Acknowledgements and references

**Spec:** §13–§14, pp. 67–71 (RFC) — acknowledgements, references.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — acknowledgements and reference lists.

---

## Audit status

24 done / 3 partial / 1 open / 0 blocked / 6 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document websocket-rfc-6455.md`):

* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (job `merge-rust-websocket-bridge`) — 138 tests green.

138 cited tests green, 0 not green.

Open items: `websocket-rfc-6455.open.en.md`.
