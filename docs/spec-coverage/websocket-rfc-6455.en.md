# IETF WebSocket RFC 6455 — Spec Coverage

**RFC:** [IETF RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) ("The WebSocket
Protocol", IETF December 2011).

**Context:** WebSocket is a bidirectional frame-based protocol
over TCP — a companion spec to DDS-WEB. ZeroDDS implements the opening
handshake (§4), the framing (§5), sending and receiving messages (§6), the
closing handshake (§7), the UTF-8 check (§8) and the negotiation of
extensions (§9) with `permessage-deflate` (RFC 7692) as a pure-Rust library
(no_std + alloc; the operating system's random source and DEFLATE with
`std`). The daemon `zerodds-ws-bridged` of the same crate is a server on
this library, with TLS (`wss://`).

Implementation:

- `crates/websocket-bridge/` — library: `handshake` (§4), `frame`, `codec`, `masking` (§5), `message` (§6), `close` (§7), `utf8` (§8), `negotiation`, `permessage_deflate` (§9), `connection` (one end of a connection: `Receiver`, `Sender`), `origin` (§10.2), `uri` (§3). Daemon: `daemon/`.
- `crates/mqtt-bridge/src/daemon/ws_transport.rs` — the WebSocket client of the MQTT daemon (`ws://`, `wss://`): handshake and codec of this crate, the receive loop with code of its own (item mqtt-5.0 §6).

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
Sec-WebSocket-Accept = SHA-1(key + MAGIC) base64, etc.). §4.2.1: the
request is a GET of HTTP/1.1 or higher, with `Host`, `Upgrade`,
`Connection`, a key of 16 octets and version 13; otherwise "the server MUST
stop processing the client's handshake and return an HTTP response with an
appropriate error code". §4.2.2 /version/, §4.4: for a version the server
does not speak, the response names its versions in `Sec-WebSocket-Version`.

**Repo:** `crates/websocket-bridge/src/handshake.rs::{ClientHandshake, ServerHandshake,
HandshakeError, compute_accept, parse_client_request, build_server_response,
render_server_response, client_key, render_client_request,
render_client_request_with_extensions, validate_server_response,
validate_server_response_with_extensions}`. Server: `parse_client_request`
checks method, HTTP version, `Host`, `Upgrade`, `Connection`, key and
version; `HandshakeError::{http_status, render_error_response}` give the
HTTP response of the error: 405 with `Allow: GET`, 426 with
`Sec-WebSocket-Version: 13`, otherwise 400. Client: the server's response is
checked against status 101, `Upgrade`, `Connection`, `Sec-WebSocket-Accept`
and against the subprotocols and extensions offered (§4.1). The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sends the response of the
error and closes; a request that is not UTF-8 gets 400.

**Tests:** `crates/websocket-bridge/src/handshake.rs::tests::{ws_4_1_rfc6455_section_1_3_accept_test_vector,
ws_4_1_parses_minimal_client_handshake, ws_4_1_parses_protocols_and_extensions,
ws_4_1_rejects_missing_upgrade, ws_4_1_rejects_wrong_version, ws_4_1_rejects_missing_key,
ws_4_1_server_response_includes_accept,
ws_4_1_server_agrees_to_no_extension_it_cannot_perform,
ws_4_1_server_selects_no_subprotocol_it_does_not_support,
ws_4_1_render_server_response_format, ws_4_1_base64_round_trip_known_vectors,
ws_4_1_sha1_known_vector_abc, ws_4_1_connection_header_with_keep_alive_still_detects_upgrade,
ws_4_2_2_an_unsupported_version_gets_426_and_the_version_the_server_speaks,
ws_4_2_1_a_request_that_is_not_get_gets_405, ws_4_2_1_a_request_below_http_1_1_is_refused,
ws_4_2_1_a_request_without_host_or_with_a_wrong_key_gets_400,
ws_4_2_1_upgrade_may_list_several_protocols,
ws_4_1_the_client_accepts_only_extensions_it_offered}`
— `ws_4_1_rfc6455_section_1_3_accept_test_vector` checks the Sec-WebSocket-Accept
test vector of the spec (SHA-1 over key + GUID, base64). Against the daemon:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`.
With the client of Python `websockets` as the other side:
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_round_trips_a_text_frame`.

**Status:** done — the check of `Origin` (§4.2.2 /origin/) is the item §10.2.

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, p. 27 — "The client MUST mask all frames that it sends to the
server. [...] The server MUST close the connection upon receiving a frame that
is not masked. [...] A server MUST NOT mask any frames that it sends to the
client. A client MUST close a connection if it detects a masked frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{Role, MaskingViolation, validate_masking,
check_masking}` — the directional rule; the violation has close code 1002
(`MaskingViolation::close_code`). `crates/websocket-bridge/src/connection.rs::Receiver` checks
the MASK bit of every frame at its header, before the payload is buffered:
as a server it refuses an unmasked frame, as a client a masked one, with
`Failure::Masking`. `crates/websocket-bridge/src/connection.rs::Sender::server` sends unmasked,
`Sender::client` masks every frame. The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) reads every frame through
`Receiver`, sends the Close frame with 1002 for the violation and closes the
connection; the frame is not processed.

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`,
`crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_1_the_server_fails_an_unmasked_frame_with_1002,
ws_5_1_the_client_fails_a_masked_frame_with_1002,
ws_5_1_an_unmasked_frame_is_refused_from_its_header}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002`.
That the server sends unmasked is checked by the other side: Python `websockets`
(`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_round_trips_binary_and_ping`)
and the fuzzing server of the Autobahn|TestSuite, to which the crate as a
client sends masked frames
(`crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_autobahn_1_framing_and_2_pings`).

**Status:** done

### §5.2 Base Framing Protocol — header layout

**Spec:** §5.2, p. 28-31 — frame header with FIN/RSV1-3/Opcode/MASK/
payload length. RSV1-3: "MUST be 0 unless an extension is negotiated that
defines meanings for non-zero values. If a nonzero value is received and
none of the negotiated extensions defines the meaning of such a nonzero
value, the receiving endpoint MUST _Fail the WebSocket Connection_."
Opcode: "If an unknown opcode is received, the receiving endpoint MUST
_Fail the WebSocket Connection_."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::{encode, decode, decode_with_limit, peek_header,
FrameHeader, CodecError}`. `crates/websocket-bridge/src/connection.rs::Receiver` refuses at the
header: an RSV bit that no negotiated extension defines
(`Failure::ReservedBit`; with `permessage-deflate` RSV1 is defined on the
first frame of a data message), a reserved opcode
(`Failure::ReservedOpcode`) and a header against §5.2 or §5.5
(`Failure::Frame`), each with close code 1002. The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sends the Close frame and
closes.

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::{ws_5_2_smallest_text_frame_encodes_to_2_byte_header_plus_payload,
ws_5_7_a_single_frame_text_message_unmasked_and_masked, ws_5_2_rsv_bits_propagate_to_decoded_frame,
ws_5_7_a_fragmented_text_message}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_2_a_reserved_bit_without_an_extension_fails_with_1002,
ws_5_2_a_reserved_opcode_fails_with_1002, ws_5_2_a_length_that_is_not_minimal_fails_with_1002}`;
against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002`;
against the Autobahn|TestSuite (cases 1 to 4)
`crates/websocket-bridge/tests/autobahn_e2e.rs::{ws_5_autobahn_1_framing_and_2_pings,
ws_5_2_autobahn_3_reserved_bits_and_4_opcodes}`.

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
octet at index i modulo 4 of the masking key". "The masking key needs to be
unpredictable; thus, the masking key MUST be derived from a strong source
of entropy, and the masking key for a given frame MUST NOT make it simple
for a server/proxy to predict the masking key for a subsequent frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{apply_mask, generate_masking_key,
MaskingKeyProvider, OsRngProvider, ClosureMaskingKeyProvider,
EntropyError}`. `generate_masking_key` and `OsRngProvider` (feature `std`)
read four octets per key from the operating system's random source
(`getrandom`); no state of the crate takes part in the key. Without `std`
the caller supplies the source through `MaskingKeyProvider`; the crate has
no generator of its own. `crates/websocket-bridge/src/connection.rs::Sender::client` and
`crates/websocket-bridge/src/message.rs::fragment_message` draw one key per frame; when the
source delivers none, no frame is built (`SendError::Entropy`).

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::{ws_5_3_apply_mask_is_symmetric,
ws_5_3_apply_mask_xors_with_key_modulo_4,
ws_5_3_apply_mask_handles_partial_key_alignment, ws_5_3_empty_payload_is_unchanged,
ws_5_3_the_masking_key_comes_from_the_operating_system,
ws_5_3_the_os_provider_draws_a_key_per_call,
ws_5_3_a_source_without_entropy_is_an_error_not_a_key,
ws_5_3_closure_provider_calls_user_supplied_fn}`,
`crates/websocket-bridge/src/message.rs::tests::ws_5_3_without_a_key_no_frame_is_built`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_3_every_frame_of_a_client_carries_a_key_of_its_own,
ws_5_3_a_client_without_a_key_sends_no_frame,
ws_5_3_the_client_sender_draws_its_keys_from_the_operating_system}`,
`crates/websocket-bridge/src/codec.rs::tests::ws_5_7_a_single_frame_text_message_unmasked_and_masked`.
`ws_5_3_the_masking_key_comes_from_the_operating_system` draws 4096 keys:
they are not the sequence of a fixed seed, each of the 32 bits is set in 40
to 60 percent of them, fewer than three repeat.

**Status:** done

### §5.4 Fragmentation

**Spec:** §5.4, p. 33-34 — FIN=0 marks a non-final fragment;
continuation frames with opcode 0x0 + FIN=1 for the last. Control frames may
stand between the fragments; the fragments of one message must not be
interleaved with those of another.

**Repo:** Sending: `crates/websocket-bridge/src/message.rs::fragment_message`,
`crates/websocket-bridge/src/connection.rs::Sender::with_max_frame_payload`. Receiving:
`crates/websocket-bridge/src/connection.rs::Receiver` reassembles the fragments through
`crates/websocket-bridge/src/message.rs::Reassembler` and delivers the message as one
`Event::Message`; a continuation frame without a message in progress and a
new data frame inside one are `Failure::Message` with close code 1002. The
daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) processes the
reassembled message.

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::ws_5_7_a_fragmented_text_message`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_4_a_fragmented_message_is_delivered_whole,
ws_5_4_control_frames_may_come_between_the_fragments,
ws_5_4_a_continuation_without_a_message_fails_with_1002,
ws_5_4_a_new_data_frame_inside_a_message_fails_with_1002}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_5_4_the_daemon_reassembles_a_fragmented_message,
ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002}` (a
`publish` in two fragments is one sample in DDS); against the
Autobahn|TestSuite (cases 5) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_4_autobahn_5_fragmentation`;
with Python `websockets`, which sends a message in three fragments,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_negotiates_permessage_deflate_and_round_trips`.

**Status:** done

### §5.5 Control frames

**Spec:** §5.5, p. 36-38 — control frames (opcode 0x8-0xF) MUST have
payload <= 125 bytes AND MUST NOT be fragmented (FIN=1).

**Repo:** `crates/websocket-bridge/src/codec.rs` enforced on encode +
decode (ControlFrameTooLong / FragmentedControlFrame errors).

**Tests:** `codec::tests::ws_5_5_control_frame_with_long_payload_rejected_on_encode`,
`ws_5_5_fragmented_control_frame_rejected_on_encode`.

**Status:** done

### §5.5.1 Close frame

**Spec:** §5.5.1 + §7.4, p. 36 + 45-46 — the Close payload starts with
a 2-byte status code (BE), optionally followed by a UTF-8 reason. "If an
endpoint receives a Close frame and did not previously send a Close frame,
the endpoint MUST send a Close frame in response." "The application MUST NOT
send any more data frames after sending a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`;
`crates/websocket-bridge/src/close.rs::{parse_close_body, CloseBodyError}` — the body is empty
or a status code that may stand on the wire (§7.4), with a reason in UTF-8;
otherwise 1002 or 1007. `crates/websocket-bridge/src/connection.rs`: `Receiver` delivers
`Event::Close` and reads nothing after it; `Sender::close_received` gives
the Close frame of the response with the status code received,
`Sender::close` starts the handshake, after the Close it sent `Sender`
builds no frame (`SendError::Closing`). The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) answers the Close, ends its
sending direction and reads until the client closes.

**Tests:** `crates/websocket-bridge/src/frame.rs::tests::{ws_5_5_1_close_frame_includes_status_code_in_be_payload,
ws_5_5_1_close_frame_with_reason_carries_utf8_bytes}`,
`crates/websocket-bridge/src/codec.rs::tests::ws_5_5_1_close_frame_carries_status_code`,
`crates/websocket-bridge/src/close.rs::tests::{ws_5_5_1_a_close_body_without_reason_is_the_code_alone,
ws_5_5_1_a_close_body_is_empty_or_a_status_code_and_a_utf8_reason}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_5_1_a_close_is_delivered_with_its_status_and_ends_the_reading,
ws_5_5_1_an_invalid_close_body_fails_the_connection,
ws_5_5_1_a_close_is_answered_with_a_close_of_the_same_status,
ws_5_5_1_no_frame_follows_the_close_this_end_sent}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status,
ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007}`; against
the Autobahn|TestSuite (cases 7) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`;
with Python `websockets`, which prints the status code of the response,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`.

**Status:** done

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
mask, payload length; UTF-8 validation for text frames;
reassembly duty for continuation frames.

**Repo:** `crates/websocket-bridge/src/message.rs::fragment_message`
+ the `SendError` enum. Splits logical messages into frame sequences
(Text/Binary with FIN=0, continuation frames, last frame with FIN=1);
UTF-8 validation on the text payload before splitting; with a key source
(`keys`, the client path) every frame carries a masking key of its own.
`crates/websocket-bridge/src/connection.rs::Sender::message` builds the frames of a message on
it, compressed with `permessage-deflate`.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_6_1_fragment_empty_message_yields_one_frame,
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
the `Message` struct + the `ReceiveError` enum. Maintains the continuation
state through the `feed(&Frame) -> Option<Message>` API; streaming UTF-8
validation per frame; DoS cap via `max_message_size`.
`crates/websocket-bridge/src/connection.rs::Receiver::feed` is the receive algorithm over the
octet stream: check the frame, deliver control frames as `Event::Ping`,
`Event::Pong`, `Event::Close`, reassemble and decompress data messages and
deliver them as `Event::Message`. The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) reads through it.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_6_2_reassembler_single_frame_message_complete,
ws_6_2_reassembler_continuation_sequence_reassembles,
ws_6_2_reassembler_continuation_without_preceding_text_rejected,
ws_6_2_reassembler_interleaved_text_during_pending_rejected,
ws_6_2_reassembler_rejects_invalid_utf8_in_text,
ws_6_2_reassembler_rejects_message_above_limit,
ws_6_2_reassembler_passes_through_control_frames,
ws_6_2_reassembler_has_pending_during_continuation,
ws_6_2_fragment_send_then_reassemble_round_trip}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_4_a_fragmented_message_is_delivered_whole,
ws_6_2_a_code_point_split_over_fragments_is_valid}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_reassembles_a_fragmented_message`.

**Status:** done — receive algorithm + reassembly + UTF-8 streaming
+ DoS cap live; round trip fragment->reassemble verified.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, p. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one." §7.1.1: the server
closes the TCP connection first. §7.1.7: Fail the WebSocket Connection.

**Repo:** `crates/websocket-bridge/src/close.rs::{CloseHandshake, CloseState}` — state machine
with Open -> ClosingInitiator/ClosingResponder -> Closed/Failed
(`initiator_send_close`, `recv_close_response`, `responder_recv_close`,
`responder_send_close_response`, `fail`). `crates/websocket-bridge/src/connection.rs::Sender`
drives it: `close` (§7.1.2), `close_received` (the response, §7.1.4),
`fail` (§7.1.7: the Close frame with the status code of the failure, once).
The daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`): a Close of the
client is answered, invalid data ends the connection with the Close frame of
its status code, on shutdown the daemon sends 1001 on every open connection
(`WsOut::going_away`, from the thread of the connection as soon as it sees
the stop flag). In all three cases it then ends its sending direction (TLS
`close_notify`, FIN) and reads until the client closes, at most 500 ms.
`DaemonHandle::shutdown` waits until the connections have ended, at most 2 s
(`SHUTDOWN_CLOSE_WAIT`, `ConnectionThreads`); the process ends after that.

**Tests:** `crates/websocket-bridge/src/close.rs::tests::{ws_7_1_handshake_starts_in_open_state,
ws_7_1_initiator_send_close_transitions_to_closing,
ws_7_1_initiator_recv_close_response_transitions_to_closed,
ws_7_1_responder_recv_close_transitions_to_closing_responder,
ws_7_1_responder_send_close_response_completes_normally,
ws_7_1_second_close_send_in_closing_is_rejected,
ws_7_1_recv_close_in_open_state_is_responder_path,
ws_7_1_the_responder_echoes_the_code_it_received}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_5_1_a_close_is_answered_with_a_close_of_the_same_status,
ws_5_5_1_no_frame_follows_the_close_this_end_sent,
ws_7_1_7_a_failed_connection_sends_its_close_once}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status`
(behind the Close frame the client reads the end of the TCP connection),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_closes_the_connection_when_the_client_does_not`
(after a Close of the client and after an unmasked frame: 250 ms after the
daemon's Close frame it holds the connection and takes octets, after 1 s the
connection has ended); the shutdown
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_daemon_closes_every_connection_with_1001_when_it_shuts_down`
(16 connections, each with the Close 1001 and the end of the TCP connection),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_reads_on_behind_the_close_of_its_shutdown`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_binary_closes_every_connection_with_1001_on_sigterm`
(the binary, SIGTERM, 40 connections); over TLS
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`
(1002, the answered Close, 1001 on shutdown, each followed by `close_notify`
and FIN);
against the Autobahn|TestSuite (cases 7) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`.

**Status:** done

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
StreamingValidator}`. Strict RFC 3629 validation: surrogate
code points (U+D800..=U+DFFF), overlong encoding, code points
> U+10FFFF are rejected. `StreamingValidator` checks octet by octet across
frame boundaries (spec §6.2 — FIN=0 continuation frames) and reports the
error with the octet that makes the stream malformed.
`crates/websocket-bridge/src/connection.rs::Receiver` (as a client) checks every text message
with it, also a frame that is not complete yet, and fails with
`Failure::Message`, close code 1007.

**Tests:** `crates/websocket-bridge/src/utf8.rs::tests::{ws_8_1_empty_is_valid,
ws_8_1_ascii_is_valid, ws_8_1_valid_2_byte_codepoint, ws_8_1_valid_3_byte_codepoint,
ws_8_1_valid_4_byte_codepoint, ws_8_1_rejects_overlong_2_byte_for_ascii,
ws_8_1_rejects_unexpected_continuation_byte, ws_8_1_rejects_invalid_lead_byte,
ws_8_1_rejects_truncated_2_byte, ws_8_1_rejects_truncated_3_byte,
ws_8_1_rejects_invalid_continuation, ws_8_1_rejects_surrogate_codepoint,
ws_8_1_rejects_codepoint_above_max, ws_8_1_streaming_handles_split_codepoint,
ws_8_1_streaming_agrees_with_validate_at_every_split,
ws_8_1_streaming_fails_with_the_octet_that_makes_the_stream_malformed,
ws_8_1_streaming_finalize_with_pending_is_truncated,
ws_8_1_streaming_complete_codepoint_in_one_chunk}` (18); the fail-the-connection
path with close code 1007:
`crates/websocket-bridge/src/message.rs::tests::ws_8_1_client_fails_connection_with_1007_on_malformed_utf8_from_server`,
`crates/websocket-bridge/src/connection.rs::tests::ws_8_1_malformed_utf8_from_the_server_fails_with_1007`.
The crate as a client against the fuzzing server of the Autobahn|TestSuite
(cases 6, 145 cases with valid and malformed UTF-8, sent whole, in fragments
and octet by octet): `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_8_1_autobahn_6_utf8`.

**Status:** done — strict UTF-8 validation + streaming variant + the
close-code-1007 path live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, p. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** The same validator `crates/websocket-bridge/src/utf8.rs::{validate,
StreamingValidator}` for both directions;
`crates/websocket-bridge/src/message.rs::ReceiveError::{close_code, close_frame}` maps the
UTF-8 error to close code 1007 (`CloseCode::InvalidPayloadData`, spec
§7.4.1). `crates/websocket-bridge/src/connection.rs::Receiver` (as a server) checks every text
message, across fragments and in a frame that is not complete yet (the
octets that are there, unmasked). The daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sends the Close frame with
1007 and closes; the message does not reach the processing.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_8_2_server_fails_connection_with_1007_on_malformed_utf8_from_client,
ws_8_2_malformed_utf8_split_across_continuation_frames_still_yields_1007,
ws_8_2_receive_error_close_codes_match_spec_7_4_1}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_8_2_malformed_utf8_from_the_client_fails_with_1007,
ws_8_2_malformed_utf8_fails_with_the_octet_that_makes_it_malformed}`; against
the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007`
(one frame, across fragments, in an incomplete frame); the server role
against the fuzzing client of the Autobahn|TestSuite (cases 6)
`crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_8_1_autobahn_server_6_utf8`.

**Status:** done — the Autobahn|TestSuite runs against the client role (§8.1)
and with its fuzzing client against the server role, an echo server on the
same `Receiver` (weekly job `integration-bridges-vendor-autobahn`; not run in
CI yet, green by hand on the Linux test host).

### §9 Extensibility

**Spec:** §9, p. 47-49 — extension negotiation via the
`Sec-WebSocket-Extensions` header; examples incl. permessage-deflate
(RFC 7692). §9.1: the client offers extensions in the order of its
preference, the server names in its response those it agrees to; an
extension the client did not offer makes the client fail (§4.1).

**Repo:** `crates/websocket-bridge/src/negotiation.rs::{ExtensionOffer,
parse_extensions, parse_subprotocols, select_subprotocol,
SUBPROTOCOL_HEADER, EXTENSIONS_HEADER}` (extension and subprotocol lists);
`crates/websocket-bridge/src/handshake.rs::{build_server_response,
validate_server_response_with_extensions}` (the server names no extension
it does not perform; the client refuses one it did not offer).
`permessage-deflate` (RFC 7692) in `crates/websocket-bridge/src/permessage_deflate.rs`: the
negotiation of both sides (`parse_deflate_offer`, `agree`, `accept_offers`,
`render_accept`, `render_offer`, `accept_response`, §7.1: an offer with an
unknown, invalid or repeated parameter is declined, the next one is looked
at) and the codec (`Deflater`, `Inflater` over `flate2` with `zlib-rs`,
§7.2: window by `server_max_window_bits`/`client_max_window_bits`, context
takeover, a limit on the decompressed size).
`crates/websocket-bridge/src/connection.rs::{Receiver, Sender}::with_permessage_deflate` is
the frame path: RSV1 on the first frame of a data message, otherwise 1002
(RFC 7692 §6). The daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`)
agrees to the first valid offer, names it in the response and compresses
the messages of the connection in both directions. A window of 8 bits for
its own sending direction the sender keeps by sending uncompressed (the
DEFLATE compressor has no window of 256 octets).

**Tests:** `crates/websocket-bridge/src/negotiation.rs::tests::{ws_9_parse_extensions_single,
ws_9_parse_extensions_with_param, ws_9_parse_extensions_with_value_param,
ws_9_parse_extensions_strips_quoted_value, ws_9_parse_extensions_multiple_offers,
ws_9_parse_extensions_empty_returns_empty, ws_9_parse_subprotocols_basic,
ws_9_parse_subprotocols_empty_returns_empty, ws_9_select_subprotocol_picks_first_match,
ws_9_select_subprotocol_returns_none_when_no_match,
ws_9_select_subprotocol_is_case_insensitive, ws_9_header_constants_match_spec}` (12);
`crates/websocket-bridge/src/handshake.rs::tests::{ws_4_1_server_agrees_to_no_extension_it_cannot_perform,
ws_4_1_the_client_accepts_only_extensions_it_offered}`;
`crates/websocket-bridge/src/permessage_deflate.rs::tests::{ws_9_parse_no_params_yields_defaults,
ws_9_parse_no_takeover_flags, ws_9_parse_window_bits, ws_9_rejects_invalid_window_bits,
ws_9_rejects_unknown_param, ws_9_rejects_boolean_with_value,
ws_9_render_default_is_bare_extension_name, ws_9_render_includes_params,
ws_9_tail_round_trip, ws_9_strip_tail_no_op_when_absent,
ws_9_parameterless_client_max_window_bits_accepted,
rfc7692_7_an_offer_with_an_undefined_invalid_or_repeated_parameter_is_declined,
rfc7692_7_1_the_response_follows_the_offer,
rfc7692_5_1_an_invalid_offer_is_declined_for_the_next_one,
rfc7692_7_1_2_1_a_window_of_8_bits_is_kept_by_not_compressing,
rfc7692_7_the_client_fails_on_a_response_outside_its_offer}`,
`crates/websocket-bridge/src/permessage_deflate.rs::tests::codec::{rfc7692_7_2_1_hello_is_compressed_to_the_octets_of_the_rfc,
rfc7692_7_2_2_the_payloads_of_the_rfc_decompress_to_hello,
rfc7692_7_2_1_the_window_survives_a_block_with_bfinal,
rfc7692_7_2_2_what_is_not_deflate_is_an_error,
rfc7692_7_2_2_the_output_stops_at_the_limit,
rfc7692_7_1_2_the_compressor_keeps_to_the_negotiated_window,
rfc7692_7_2_2_no_context_takeover_starts_each_message_with_an_empty_window}`
(the octets of the examples of RFC 7692 §7.2.3);
`crates/websocket-bridge/src/connection.rs::tests::deflate::{rfc7692_6_1_the_frames_of_the_rfc_are_received_as_hello,
rfc7692_7_2_3_2_the_second_hello_refers_to_the_window_of_the_first,
rfc7692_6_1_rsv1_on_a_control_or_continuation_frame_fails_with_1002,
rfc7692_6_a_sent_message_is_compressed_and_received_again,
rfc7692_7_2_1_context_takeover_makes_the_repeated_message_shorter,
rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009,
rfc7692_6_2_a_compressed_text_message_is_utf8_after_decompression,
rfc7692_7_1_2_a_sender_with_a_window_of_8_bits_sends_uncompressed,
rfc7692_7_the_roles_use_the_parameters_of_their_direction}`; against the daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions,
ws_9_the_daemon_declines_an_offer_that_is_invalid}`. With the zlib of the
other side: Python `websockets`
(`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_negotiates_permessage_deflate_and_round_trips`,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`)
and the Autobahn|TestSuite (cases 12 and 13, 216 cases over message sizes
and every combination of parameters): the tests `ws_9_autobahn_12_*` and `ws_9_autobahn_13_*` of the target `crates/websocket-bridge/tests/autobahn_e2e.rs`, 54 tests, among them `crates/websocket-bridge/tests/autobahn_e2e.rs::{ws_9_autobahn_12_1_cases_1_to_10, ws_9_autobahn_12_2_case_10, ws_9_autobahn_12_3_case_18, ws_9_autobahn_12_4_cases_16_to_18, ws_9_autobahn_12_5_case_15, ws_9_autobahn_13_1_cases_1_to_10, ws_9_autobahn_13_3_cases_16_to_18, ws_9_autobahn_13_5_cases_11_to_15, ws_9_autobahn_13_7_cases_16_to_18}`.

**Status:** done

---

## §10-§14 Security + IANA + Misc

### §10.1, §10.5, §10.8 Non-browser clients, client authentication, SHA-1

**Spec:** §10.1, §10.5, §10.8, pp. 50–54 (RFC) — notes on clients outside a browser, on authentication via HTTP mechanisms and on SHA-1 in the handshake having no security function.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — explanations without a requirement of their own.

### §10.2 Verify the origin

**Spec:** §10.2, p. 50 (RFC) — "Servers that are not intended to process input from any web page but only for certain sites SHOULD verify the |Origin| field is an origin they expect. If the origin indicated is unacceptable to the server, then it SHOULD respond to the WebSocket handshake with a reply containing HTTP 403 Forbidden status code."

**Repo:** `crates/websocket-bridge/src/origin.rs` (`OriginPolicy`, `Origin`: comparison as in RFC 6454 §5), `crates/websocket-bridge/src/handshake.rs` (`ClientHandshake::origin`, `HandshakeError::OriginNotExpected`, `HandshakeError::MissingOrigin`: HTTP 403), `crates/websocket-bridge/src/daemon/config.rs` (`origin.allowed`, `origin.required`), `crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`)

**Tests:** `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_an_origin_is_scheme_host_and_port`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_text_that_is_no_serialized_origin_is_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_an_expected_origin_passes_and_any_other_is_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_scheme_and_host_compare_without_case_and_the_default_port_is_no_port`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_handshake_without_origin_passes_unless_the_field_is_required`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_without_a_list_nothing_is_checked`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_the_star_expects_every_origin_and_null_is_its_own_entry`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_every_origin_of_a_list_in_the_field_has_to_be_expected`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_that_is_no_origin_and_required_without_a_list_are_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_with_a_pattern_in_its_host_is_refused`, `crates/websocket-bridge/src/handshake.rs::tests::ws_4_2_1_the_origin_field_is_read`, `crates/websocket-bridge/src/handshake.rs::tests::ws_10_2_an_origin_the_server_does_not_expect_gets_403`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_the_origin_keys_are_read`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_an_origin_setting_that_cannot_be_served_is_refused_with_its_line`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_accepts_an_origin_it_expects_and_a_client_without_one`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_refuses_a_handshake_without_origin_when_one_is_required`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_without_a_list_the_daemon_accepts_every_origin`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_checks_the_origin_before_the_credentials`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_does_not_start_with_an_origin_setting_it_cannot_serve`, `crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::ws_10_2_the_daemon_checks_the_origin_python_websockets_sends`

**Status:** done — the daemon checks `Origin` against the list `origin.allowed` and answers an origin outside the list with HTTP 403, before the authentication; without a list it accepts every origin (§4.2.2 /origin/). A handshake without `Origin` is accepted (a client outside a browser), and refused with `origin.required: true`. Peer in the test: Python websockets (weekly job `integration-bridges-vendor`; green by hand on the Linux test host).

### §10.3 A new, unpredictable masking key per frame

**Spec:** §10.3, p. 51 (RFC) — "Clients MUST choose a new masking key for each frame, using an algorithm that cannot be predicted by end applications that provide data."

**Repo:** `crates/websocket-bridge/src/masking.rs` (`generate_masking_key`, `OsRngProvider`: the operating system's random source), `crates/websocket-bridge/src/message.rs` (`fragment_message`: one key per frame), `crates/websocket-bridge/src/connection.rs` (`Sender::client`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream::send`: the WebSocket client of the MQTT daemon, four octets per frame from the random source of the TLS provider), `examples/tutorials/dds-chat/integrations/websocket/src/bin/ws_e2e.rs` (example client: `Sender::client`)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_3_the_masking_key_comes_from_the_operating_system`, `crates/websocket-bridge/src/message.rs::tests::ws_10_3_each_frame_of_a_fragmented_message_has_its_own_key`, `crates/websocket-bridge/src/message.rs::tests::ws_10_3_a_zero_key_still_sets_the_mask_bit`, `crates/websocket-bridge/src/connection.rs::tests::ws_5_3_every_frame_of_a_client_carries_a_key_of_its_own`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_3_every_frame_of_the_mqtt_client_carries_a_key_of_its_own`

**Status:** done — the crate draws one key per frame from the operating system's random source and has no predictable generator. The WebSocket client in `crates/mqtt-bridge` draws four octets per frame from the random source of the TLS provider; in the test all of its frames (CONNECT, 100 PUBLISH, DISCONNECT, Close) are masked and no key occurs twice. The example client `ws_e2e.rs` sent its frame with the fixed key 0 and a fixed `Sec-WebSocket-Key`; it now draws both from the operating system's random source.

### §10.4 Limits on frame and message size

**Spec:** §10.4, p. 52 (RFC) — "Implementations that have implementation- and/or platform-specific limitations regarding the frame size or total message size after reassembly from multiple frames MUST protect themselves against exceeding those limits. [...] Such an implementation SHOULD impose a limit on frame sizes and the total message size after reassembly from multiple frames."

**Repo:** `crates/websocket-bridge/src/codec.rs` (`decode_with_limit`, `CodecError::FrameTooLarge`: at the header, before the payload is buffered), `crates/websocket-bridge/src/message.rs` (`Reassembler::with_limit`, `max_message_size`), `crates/websocket-bridge/src/connection.rs` (`Limits`, `Receiver`: close code 1009; the limit of the message holds after decompression), `crates/websocket-bridge/src/daemon/config.rs` (`limits.max_frame_size`, `limits.max_message_size`: 16 MiB each by default), `crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`MAX_FRAME_PAYLOAD`: the limit of the WebSocket client of the MQTT daemon, one MQTT packet of maximal size)

**Tests:** `crates/websocket-bridge/src/message.rs::tests::ws_6_2_reassembler_rejects_message_above_limit`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_a_frame_announcing_2_pow_60_is_refused_from_its_header`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_a_frame_at_the_limit_decodes`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_incomplete_is_told_apart_from_invalid`, `crates/websocket-bridge/src/connection.rs::tests::ws_10_4_a_frame_over_the_limit_fails_with_1009_from_its_header`, `crates/websocket-bridge/src/connection.rs::tests::ws_10_4_a_message_of_many_small_frames_over_the_limit_fails_with_1009`, `crates/websocket-bridge/src/connection.rs::tests::deflate::rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_the_limits_are_read_and_default_to_16_mib`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_a_limit_that_cannot_be_served_is_refused_with_its_line`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_message_of_many_frames_over_its_limit_with_1009`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_frame_limit_of_its_configuration`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_message_limit_of_its_configuration`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_does_not_start_with_a_limit_it_cannot_serve`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_4_the_mqtt_client_refuses_a_frame_over_its_limit_from_the_header`

**Status:** done — frame and message are limited in the library and in the daemon, the violation closes with 1009, a frame that is too large at its header already. The limits of the daemon are configurable; a frame limit below 125 octets and a message limit of 0 prevent the start.

### §10.6 Support TLS

**Spec:** §10.6, p. 53 (RFC) — "WebSocket implementations MUST support TLS and SHOULD employ it when communicating with their peers."

**Repo:** `crates/websocket-bridge/src/daemon/security.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`, `crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`

**Status:** done

### §10.7 Invalid data

**Spec:** §10.7, p. 53 (RFC) — "Incoming data MUST always be validated by both clients and servers." After the handshake "the endpoint SHOULD send a Close frame with an appropriate status code (Section 7.4)"; during the handshake "the server SHOULD return an appropriate HTTP [RFC2616] status code."

**Repo:** `crates/websocket-bridge/src/connection.rs` (`Receiver`, `Failure::close_code`, `Sender::fail`), `crates/websocket-bridge/src/handshake.rs` (`HandshakeError::render_error_response`), `crates/websocket-bridge/src/codec.rs`, `crates/websocket-bridge/src/masking.rs`, `crates/websocket-bridge/src/utf8.rs`, `crates/websocket-bridge/src/close.rs`, `crates/websocket-bridge/src/daemon/server.rs`, `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`: the receive loop of the WebSocket client of the MQTT daemon)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_autobahn_1_framing_and_2_pings`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_2_autobahn_3_reserved_bits_and_4_opcodes`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_4_autobahn_5_fragmentation`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_8_1_autobahn_6_utf8`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_autobahn_server_1_framing_and_2_pings`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_2_autobahn_server_3_reserved_bits_and_4_opcodes`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_4_autobahn_server_5_fragmentation`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_8_1_autobahn_server_6_utf8`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_7_autobahn_server_7_close_handling`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_10_4_autobahn_server_9_message_sizes_1_to_4`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_10_4_autobahn_server_9_message_sizes_5_to_8_and_10`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_7_the_mqtt_client_refuses_invalid_frames_of_the_server`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_7_the_mqtt_client_refuses_an_invalid_close_frame_of_the_server`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_5_5_1_the_mqtt_client_answers_a_close_frame_of_the_server_with_its_status`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_4_the_mqtt_client_refuses_a_frame_over_its_limit_from_the_header`

**Status:** done — the daemon answers every kind of invalid data after the handshake with the Close frame of its status code (1002, 1007, 1009) and an invalid handshake with the HTTP status (400, 401, 403, 405, 426). The Autobahn|TestSuite judges both roles of the library: its fuzzing server the client role (cases 1 to 7), its fuzzing client the server role (`autobahn_server_e2e.rs`: an echo server of the handshake and the `Receiver` and `Sender` the daemon uses; cases 1 to 7, 9 and 10, 301 cases; per case `OK` for what was sent and for how the connection closed, for 7.1.6, 7.13.1 and 7.13.2 `INFORMATIONAL` as well). The tests of the server role are `#[ignore]` and run in the weekly job `integration-bridges-vendor-autobahn`; they have not run in CI yet, green by hand on the Linux test host. The cases 12 and 13 (`permessage-deflate`) run against the client role only. The WebSocket client in `crates/mqtt-bridge/src/daemon/ws_transport.rs` reads frames with code of its own over the codec of this crate; for twelve kinds of invalid frames that were checked, for a frame over its limit and for a Close frame with an invalid body (one octet, a status code that is not allowed, a reason that is not UTF-8) it sends the Close frame with the status code of the violation (1002, 1003, 1007, 1009) and closes the network connection; a valid Close frame it answers with its status code. The tests require per case the reason the client reports, exactly one Close frame with the status code, and the connection ended by the client before the stub of the test gives up.

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

28 done / 0 partial / 0 open / 0 blocked / 6 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document websocket-rfc-6455.md`):

* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (job `merge-rust-websocket-bridge`) — 138 tests green.

138 cited tests green, 0 not green.

