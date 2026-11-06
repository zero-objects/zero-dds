# IETF WebSocket RFC 6455 — Spec-Coverage

**RFC:** [IETF RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) ("The WebSocket
Protocol", IETF December 2011).

**Kontext:** WebSocket ist ein bidirektionales Frame-basiertes Protokoll
über TCP — Begleitspec zu DDS-WEB. ZeroDDS implementiert das
**Base Framing Protocol** (RFC 6455 §5.2 + §5.3) als pure-Rust
no_std+alloc Library. Opening Handshake (HTTP-Upgrade, §4) ist
Caller-Aufgabe, ebenso TLS (`wss://`) und Extension-Negotiation.

Implementation:

- `crates/websocket-bridge/` — Base Framing Protocol (§5.2 + §5.3), Handshake (§4), Message-Layer (§6), Close-State-Machine (§7) und Extension-Negotiation (§9) als pure-Rust no_std+alloc Library.

---

## §1 Introduction

### §1.1-§1.9 Background + Goals

**Spec:** §1, S. 4-12 (RFC) — Abstract, Background, Goals,
Conformance-Requirements.

**Repo:** `crates/websocket-bridge/src/lib.rs` Crate-Doc.

**Tests:** —

**Status:** n/a (informative) — jeder Abschnitt §1.1 bis §1.9 trägt in der RFC "_This section is non-normative._"

---

## §2 Conformance Requirements

### §2 RFC 2119 Keywords

**Spec:** §2, S. 12 — RFC 2119 MUST/SHALL/etc. Spec.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Lesart der Schlüsselwörter und Konformitätsregeln für Algorithmen; keine eigene Anforderung an das Protokollverhalten.

---

## §3 WebSocket URIs

### §3 ws:// + wss:// URI-Scheme

**Spec:** §3, S. 13-14 — `ws://host[:port]/path` + `wss://`.

**Repo:** `crates/websocket-bridge/src/uri.rs::{parse_websocket_uri,
WebSocketUri, default_port, resource_name, is_local_loopback,
UriError}`. Default-Port 80 (ws) / 443 (wss); Fragment-Identifier
explizit rejected (Spec §3); Query-String wird abgespalten;
IPv4-Hostnames + DNS-Names; IPv6-Literale [::1] sind
Caller-Layer (RFC 6874).

**Tests:** `crates/websocket-bridge/src/uri.rs::tests::{ws_3_parses_basic_ws_uri,
ws_3_parses_basic_wss_uri, ws_3_parses_explicit_port, ws_3_parses_query_string,
ws_3_parses_default_path_when_missing, ws_3_rejects_unknown_scheme,
ws_3_rejects_missing_host, ws_3_rejects_missing_host_before_port,
ws_3_rejects_invalid_port, ws_3_rejects_fragment, ws_3_default_port_returns_443_for_wss,
ws_3_resource_name_combines_path_and_query, ws_3_resource_name_without_query_is_path,
ws_3_is_local_loopback_recognizes_localhost}` (14).

**Status:** done — URI-Scheme-Parser live.

---

## §4 Opening Handshake

### §4.1-§4.4 Client + Server Handshake

**Spec:** §4, S. 14-25 — HTTP-Upgrade (Sec-WebSocket-Key,
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
— `ws_4_1_rfc6455_section_1_3_accept_test_vector` prüft den Sec-WebSocket-Accept-
Testvektor der Spec (SHA-1 aus Key + GUID, base64).

**Status:** done

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, S. 27 — „The client MUST mask all frames that it sends
to the server. […] The server MUST close the connection upon receiving
a frame that is not masked. […] A server MUST NOT mask any frames that
it sends to the client. A client MUST close a connection if it detects
a masked frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{Role,
MaskingViolation, validate_masking}` — prüft ein empfangenes Frame
gegen die Richtungsregel und liefert für den Verstoß den Close-Code
1002 (`MaskingViolation::close_code`).

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`,
`crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`.

**Status:** done

### §5.2 Base Framing Protocol — Header Layout

**Spec:** §5.2, S. 28-31 — Frame-Header mit FIN/RSV1-3/Opcode/MASK/
Payload-Length.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::encode/decode`.

**Tests:** `codec::tests::ws_5_2_smallest_text_frame_encodes_to_2_byte_header_plus_payload`,
`ws_5_7_a_single_frame_text_message_unmasked_and_masked`, `ws_5_2_rsv_bits_propagate_to_decoded_frame`,
`ws_5_7_a_fragmented_text_message`.

**Status:** done

### §5.2 Opcode Values

**Spec:** §5.2, S. 28-29 — 0x0 Continuation, 0x1 Text, 0x2 Binary,
0x8 Close, 0x9 Ping, 0xA Pong, 0x3-0x7 reserved non-control,
0xB-0xF reserved control.

**Repo:** `crates/websocket-bridge/src/frame.rs::Opcode`.

**Tests:** `frame::tests::ws_5_2_the_opcodes_are_the_numbers_of_the_rfc`,
`ws_5_2_opcode_well_known_values_match_spec`, `ws_5_2_opcode_is_control_predicate`.

**Status:** done

### §5.2 Payload Length Encoding (7 / 7+16 / 7+64)

**Spec:** §5.2, S. 29 — "minimal number of bytes MUST be used".
* 0..=125 = direkt im 7-bit-Feld.
* 126 + 2 byte BE = 16-bit Length.
* 127 + 8 byte BE (MSB=0) = 64-bit Length.

**Repo:** `crates/websocket-bridge/src/codec.rs::encode_payload_length`
+ Decode-Validation (NonMinimalLength + PayloadLengthMsbSet).

**Tests:** `codec::tests::ws_5_2_medium_payload_uses_extended_16_bit_length`,
`ws_5_2_large_payload_uses_extended_64_bit_length`,
`ws_5_2_extended_64_bit_length_msb_set_rejected`,
`ws_5_2_non_minimal_16_bit_length_rejected`,
`ws_5_2_non_minimal_64_bit_length_rejected`,
`ws_5_2_extended_16_bit_length_truncated_fails`,
`ws_5_7_a_256_and_a_65536_octet_binary_frame`.

**Status:** done

### §5.3 Client-to-Server Masking

**Spec:** §5.3, S. 32-33 — XOR-Maskierung mit 32-bit Key. "octet i of
the transformed data is the XOR of octet i of the original data with
octet at index i modulo 4 of the masking key". Spec §5.3 verlangt
"unpredictable" Key; "MUST be derived from a strong source of entropy".

**Repo:** `crates/websocket-bridge/src/masking.rs::apply_mask`
(symmetrisch); `generate_masking_key` (Splitmix64-based,
**explicitly not for security** — Anwender MUSS eigenen RNG einsetzen).

**Tests:** `masking::tests::ws_5_3_apply_mask_is_symmetric`,
`ws_5_3_apply_mask_xors_with_key_modulo_4`,
`ws_5_3_apply_mask_handles_partial_key_alignment`, `ws_5_3_empty_payload_is_unchanged`,
`ws_5_3_generate_masking_key_returns_4_bytes`,
`ws_5_3_generate_masking_key_returns_distinct_values_across_calls`,
`ws_5_3_insecure_splitmix_provider_implements_trait`,
`ws_5_3_closure_provider_calls_user_supplied_fn`,
`codec::tests::ws_5_7_a_single_frame_text_message_unmasked_and_masked`.

**Status:** done — XOR-Logik + `MaskingKeyProvider`-Trait für
caller-supplied secure RNGs (OsRng/getrandom/Hardware-RNG) live;
`InsecureSplitmixProvider` als explizit `not for security`-Default;
`ClosureMaskingKeyProvider` zur einfachen Anbindung externer
RNG-Quellen.

### §5.4 Fragmentation

**Spec:** §5.4, S. 33-34 — FIN=0 markiert non-final Fragment;
Continuation-Frames mit Opcode 0x0 + FIN=1 für letzten.

**Repo:** Codec liefert FIN-Bit + Opcode 1:1 durch; Re-Assembly ist
Caller-Aufgabe.

**Tests:** `codec::tests::ws_5_7_a_fragmented_text_message`.

**Status:** done — Frame-level support; Caller-side reassembly out
of codec scope.

### §5.5 Control Frames

**Spec:** §5.5, S. 36-38 — Control-Frames (opcode 0x8-0xF) MUST have
payload <= 125 bytes UND MUST NOT be fragmented (FIN=1).

**Repo:** `crates/websocket-bridge/src/codec.rs` enforced beim Encode +
Decode (ControlFrameTooLong / FragmentedControlFrame Errors).

**Tests:** `codec::tests::ws_5_5_control_frame_with_long_payload_rejected_on_encode`,
`ws_5_5_fragmented_control_frame_rejected_on_encode`.

**Status:** done

### §5.5.1 Close Frame

**Spec:** §5.5.1 + §7.4, S. 36 + 45-46 — Close-Payload startet mit
2-byte Status-Code (BE), optional gefolgt von UTF-8 Reason.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`.

**Tests:** `frame::tests::ws_5_5_1_close_frame_includes_status_code_in_be_payload`,
`ws_5_5_1_close_frame_with_reason_carries_utf8_bytes`,
`codec::tests::ws_5_5_1_close_frame_carries_status_code`.

**Status:** done — Status-Code-Semantik (1000=Normal, 1001=Going-Away,
etc.) ist Caller-Validierungs-Aufgabe.

### §5.5.2 Ping Frame

**Spec:** §5.5.2, S. 37 — "The Ping frame contains an opcode of 0x9.
[...] A Ping frame MAY include 'Application data'. [...] Upon
receipt of a Ping frame, an endpoint MUST send a Pong frame in
response, unless it already received a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::ping`
(opcode 0x9, FIN=1, masking gemäß §5.3).

**Tests:** `codec::tests::ws_5_7_an_unmasked_ping_and_the_masked_pong_with_its_body`.

**Status:** done — Echo-Logik (Pong-Reply mit Ping-Payload) ist
Caller-Aufgabe (Connection-State-Machine, siehe §6.1).

### §5.5.3 Pong Frame

**Spec:** §5.5.3, S. 37 — "The Pong frame contains an opcode of 0xA.
[...] A Pong frame sent in response to a Ping frame must have
identical 'Application data' as found in the message body of the
Ping frame being replied to."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::pong`
(opcode 0xA).

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::ws_5_7_an_unmasked_ping_and_the_masked_pong_with_its_body`.

**Status:** done — Wire-Format + Payload-Identität Ping→Pong belegt;
wann ein Pong gesendet wird, entscheidet die Connection-State-Machine
des Callers.

### §5.6 Data Frames

**Spec:** §5.6, S. 38 — „Data frames (e.g., non-control frames) are
identified by opcodes where the most significant bit of the opcode is
0." Text-Frame (0x1): Payload ist UTF-8-Text; Binary-Frame (0x2):
Payload ist beliebige Binärdaten.

**Repo:** `crates/websocket-bridge/src/frame.rs::{Frame, Opcode}` —
`Frame::text`/`Frame::binary`; `Opcode::is_control` trennt Daten- von
Control-Frames über das MSB.

**Tests:** `crates/websocket-bridge/src/frame.rs::tests::{ws_5_6_data_frame_opcodes_have_msb_zero_per_spec_5_6,
ws_5_6_text_and_binary_data_frames_carry_their_payload_unchanged,
ws_5_6_text_frame_constructor_sets_fin_and_opcode}`;
`crates/websocket-bridge/src/codec.rs::tests::{ws_5_7_a_single_frame_text_message_unmasked_and_masked,
ws_5_7_a_256_and_a_65536_octet_binary_frame}`.

**Status:** done — UTF-8-Validierung von Text-Frames ist
Caller-Aufgabe (Codec-API ist `Vec<u8>`-basiert, keine UTF-8-Pflicht).

### §5.7-§5.8 Examples + Extensibility

**Spec:** §5.7-§5.8, S. 38-39 — Beispiele + Reserved-Bits.

**Repo:** RSV1/RSV2/RSV3 als public Felder im `Frame`-Struct.

**Tests:** `codec::tests::ws_5_2_rsv_bits_propagate_to_decoded_frame`.

**Status:** done

---

## §6 Sending and Receiving Data

### §6.1 Sending Data

**Spec:** §6.1, S. 39-40 — Send-Algorithmus: Frame mit FIN, Opcode,
Mask, Payload-Länge bestimmen; bei Text-Frames UTF-8-Validierung;
bei Continuation-Frames Reassembly-Pflicht.

**Repo:** `crates/websocket-bridge/src/message.rs::fragment_message`
+ `SendError`-Enum (InvalidFrameLimit, InvalidUtf8). Splittet logische
Messages in Frame-Sequenzen (Text/Binary mit FIN=0, Continuation-
Frames, letztes Frame mit FIN=1); UTF-8-Validation auf Text-Payload
vor Splitting; Mask-Field wird gesetzt wenn Caller einen non-Null
Mask-Key übergibt (Client-Pfad).

**Tests:** `message::tests::{ws_6_1_fragment_empty_message_yields_one_frame,
ws_6_1_fragment_message_within_limit_single_frame,
ws_6_1_fragment_message_splits_into_text_plus_continuations,
ws_6_1_fragment_text_rejects_invalid_utf8, ws_6_1_fragment_zero_limit_rejected,
ws_6_1_fragment_with_mask_sets_mask_field}`.

**Status:** done — Send-Algorithmus + Frame-Sequencing live.

### §6.2 Receiving Data

**Spec:** §6.2, S. 40-42 — Receive-Algorithmus: Reassembly von
Continuation-Frames; UTF-8-Validation für Text-Frames; Connection-
State-Tracking.

**Repo:** `crates/websocket-bridge/src/message.rs::Reassembler` +
`Message`-Struct + `ReceiveError`-Enum. Pflegt Continuation-State
über `feed(&Frame) -> Option<Message>`-API; Streaming-UTF-8-
Validation pro Frame; DoS-Cap via `max_message_size`. Control-Frames
(Close/Ping/Pong) werden direkt durchgereicht (Spec §5.5: nicht
fragmentierbar). Reserved-Opcodes triggern Fail.

**Tests:** `message::tests::{ws_6_2_reassembler_single_frame_message_complete,
ws_6_2_reassembler_continuation_sequence_reassembles,
ws_6_2_reassembler_continuation_without_preceding_text_rejected,
ws_6_2_reassembler_interleaved_text_during_pending_rejected,
ws_6_2_reassembler_rejects_invalid_utf8_in_text,
ws_6_2_reassembler_rejects_message_above_limit,
ws_6_2_reassembler_passes_through_control_frames,
ws_6_2_reassembler_has_pending_during_continuation,
ws_6_2_fragment_send_then_reassemble_round_trip}`.

**Status:** done — Receive-Algorithmus + Reassembly + UTF-8-Streaming
+ DoS-Cap live; Round-Trip Fragment→Reassemble verifiziert.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, S. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one."

**Repo:** Close-Frame-Codec via `crates/websocket-bridge/src/frame.rs::Frame::close` +
`close.rs::{CloseHandshake, CloseState}` State-Machine mit Open →
ClosingInitiator/ClosingResponder → Closed/Failed Transitions.
Operations: `initiator_send_close`, `recv_close_response`,
`responder_recv_close`, `responder_send_close_response`, `fail`.

**Tests:** `close::tests::{ws_7_1_handshake_starts_in_open_state,
ws_7_1_initiator_send_close_transitions_to_closing,
ws_7_1_initiator_recv_close_response_transitions_to_closed,
ws_7_1_responder_recv_close_transitions_to_closing_responder,
ws_7_1_responder_send_close_response_completes_normally,
ws_7_1_second_close_send_in_closing_is_rejected,
ws_7_1_recv_close_in_open_state_is_responder_path}`.

**Status:** done — Wire-Format + Close-Handshake-State-Machine live.

### §7.2 Abnormal Closures

**Spec:** §7.2, S. 43-44 — Abnormal-Close-Bedingungen (Server-
Failure-zu-Client, Client-Failure-zu-Server, recoverable Conditions).

**Repo:** `close.rs::CloseHandshake::fail(reason)` + `CloseState::
Failed` mit `failure_reason`-Tracking.

**Tests:** `close::tests::ws_7_2_fail_marks_abnormal_closure`.

**Status:** done — Failure-State + Recovery-Reason-Tracking live.

### §7.3 Normal Closure of Connections

**Spec:** §7.3, S. 44 — Normal Closure (after Close-Handshake).

**Repo:** `close.rs::CloseState::Closed` als Final-State; `is_closed()`
unterscheidet Closed vs. Failed.

**Tests:** `crates/websocket-bridge/src/close.rs::tests::ws_7_1_initiator_recv_close_response_transitions_to_closed`,
`crates/websocket-bridge/src/close.rs::tests::ws_7_1_responder_send_close_response_completes_normally`.

**Status:** done — Normal-Closure-State-Tracking ausgewiesen.

### §7.4 Status Codes

**Spec:** §7.4, S. 44-46 — Status-Code-Registry (1000-1015 reserved
+ 3000-4999 vendor). §7.4.1 listet 14 spezifische Codes
(1000=Normal, 1001=Going-Away, 1002=Protocol-Error, etc.).
§7.4.2 sagt 1004/1005/1006 dürfen nicht über die Wire gesendet werden.

**Repo:** Status-Code als 16-bit BE in der Payload (`close.rs`) +
Range-Validation `close.rs::{StatusCodeRange, classify_status_code,
is_forbidden_on_wire, validate_wire_status_code}` mit allen vier
Registry-Ranges (Invalid <1000, ProtocolReserved 1000-2999,
LibraryDefined 3000-3999, ApplicationDefined 4000-4999) +
Forbidden-on-Wire-Set (1004/1005/1006/1015).

**Tests:** `close::tests::{ws_7_4_classify_status_code_recognizes_protocol_range,
ws_7_4_classify_status_code_recognizes_library_range,
ws_7_4_classify_status_code_recognizes_app_range,
ws_7_4_classify_status_code_recognizes_invalid_below_1000,
ws_7_4_classify_status_code_recognizes_out_of_range_above_5000,
ws_7_4_is_forbidden_on_wire_covers_all_four,
ws_7_4_validate_wire_status_code_accepts_normal,
ws_7_4_validate_wire_status_code_rejects_forbidden,
ws_7_4_validate_wire_status_code_rejects_out_of_range}`.

**Status:** done — Wire-Format + Range-Validation +
§7.4.2-Restrictions (alle vier Forbidden-Codes) live.

---

## §8-§9 Error Handling + Extensions

### §8.1 Handling Errors in UTF-8 from the Server

**Spec:** §8.1, S. 46 — "When a client receives a Text-frame from
the server that contains malformed UTF-8 data [...] the client MUST
fail the WebSocket Connection."

**Repo:** `crates/websocket-bridge/src/utf8.rs::{validate, Utf8Error,
StreamingValidator}`. Strict RFC-3629-Validation: Surrogate-
Codepoints (U+D800..=U+DFFF), Overlong-Encoding, Codepoints
> U+10FFFF werden rejected. `StreamingValidator` hält einen
4-Byte-Puffer für fragmentierte Text-Frames (Spec §6.2 — FIN=0
Continuation-Frames).

**Tests:** `crates/websocket-bridge/src/utf8.rs::tests::{ws_8_1_empty_is_valid,
ws_8_1_ascii_is_valid, ws_8_1_valid_2_byte_codepoint, ws_8_1_valid_3_byte_codepoint,
ws_8_1_valid_4_byte_codepoint, ws_8_1_rejects_overlong_2_byte_for_ascii,
ws_8_1_rejects_unexpected_continuation_byte, ws_8_1_rejects_invalid_lead_byte,
ws_8_1_rejects_truncated_2_byte, ws_8_1_rejects_truncated_3_byte,
ws_8_1_rejects_invalid_continuation, ws_8_1_rejects_surrogate_codepoint,
ws_8_1_rejects_codepoint_above_max, ws_8_1_streaming_handles_split_codepoint,
ws_8_1_streaming_finalize_with_pending_is_truncated,
ws_8_1_streaming_complete_codepoint_in_one_chunk}` (16); der Fail-the-Connection-
Pfad mit Close-Code 1007:
`crates/websocket-bridge/src/message.rs::tests::ws_8_1_client_fails_connection_with_1007_on_malformed_utf8_from_server`.

**Status:** done — Strict-UTF-8-Validation + Streaming-Variante +
Close-Code-1007-Pfad live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, S. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** Derselbe Validator `crates/websocket-bridge/src/utf8.rs::validate`
für beide Richtungen; das Fail-the-Connection-Verhalten in
`crates/websocket-bridge/src/message.rs::ReceiveError::{close_code,
close_frame}` bildet den UTF-8-Fehler auf Close-Code 1007
(`CloseCode::InvalidPayloadData`, Spec §7.4.1) ab.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_8_2_server_fails_connection_with_1007_on_malformed_utf8_from_client,
ws_8_2_malformed_utf8_split_across_continuation_frames_still_yields_1007,
ws_8_2_receive_error_close_codes_match_spec_7_4_1}`.

**Status:** done — maskiertes Client-Text-Frame mit defektem UTF-8
schließt die Verbindung mit 1007, auch über Continuation-Frames
hinweg.

### §9 Extensibility

**Spec:** §9, S. 47-49 — Extension-Negotiation via
`Sec-WebSocket-Extensions`-Header; Beispiele inkl. permessage-deflate
(RFC 7692).

**Repo:** `crates/websocket-bridge/src/permessage_deflate.rs::
{PermessageDeflateParams, parse_offer, render_accept, append_tail,
strip_tail}` (RFC 7692-Implementation) +
`crates/websocket-bridge/src/negotiation.rs::{ExtensionOffer,
parse_extensions, parse_subprotocols, select_subprotocol,
SUBPROTOCOL_HEADER, EXTENSIONS_HEADER}` (generic Extension- +
Subprotocol-Negotiation-Framework).

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

**Status:** done — permessage-deflate + generic Extension-Listen-
Parser + Subprotocol-Negotiation live.

---

## §10-§14 Security + IANA + Misc

### §10.1, §10.5, §10.8 Nicht-Browser-Clients, Client-Authentifizierung, SHA-1

**Spec:** §10.1, §10.5, §10.8, S. 50–54 (RFC) — Hinweise zu Clients außerhalb eines Browsers, zur Authentifizierung über HTTP-Mechanismen und dazu, dass SHA-1 im Handshake keine Sicherheitsfunktion hat.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Erläuterungen ohne eigene Anforderung.

### §10.2 Origin prüfen

**Spec:** §10.2, S. 50 (RFC) — "Servers that are not intended to process input from any web page but only for certain sites SHOULD verify the |Origin| field is an origin they expect. If the origin indicated is unacceptable to the server, then it SHOULD respond to the WebSocket handshake with a reply containing HTTP 403 Forbidden status code."

**Repo:** —

**Tests:** —

**Status:** open — der Handshake (`crates/websocket-bridge/src/handshake.rs`) und der Daemon prüfen das Feld `Origin` nicht.

### §10.3 Neuer, nicht vorhersagbarer Masking-Key je Frame

**Spec:** §10.3, S. 51 (RFC) — "Clients MUST choose a new masking key for each frame, using an algorithm that cannot be predicted by end applications that provide data."

**Repo:** `crates/websocket-bridge/src/masking.rs` (`generate_masking_key`)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_3_generate_masking_key_returns_distinct_values_across_calls`, `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`

**Status:** partial — je Frame entsteht ein neuer Schlüssel; der Test prüft, dass aufeinanderfolgende Schlüssel verschieden sind. Kein Test prüft die Quelle der Zufallszahlen; das Modul bietet mit `ws_5_3_insecure_splitmix_provider_implements_trait` auch einen vorhersagbaren Provider an.

### §10.4 Grenzen für Frame- und Nachrichtengröße

**Spec:** §10.4, S. 52 (RFC) — "Implementations that have implementation- and/or platform-specific limitations regarding the frame size or total message size after reassembly from multiple frames MUST protect themselves against exceeding those limits. [...] Such an implementation SHOULD impose a limit on frame sizes and the total message size after reassembly from multiple frames."

**Repo:** `crates/websocket-bridge/src/message.rs` (`Reassembler::with_limit`, `max_message_size`)

**Tests:** `crates/websocket-bridge/src/message.rs::tests::ws_6_2_reassembler_rejects_message_above_limit`

**Status:** partial — die Gesamtgröße nach dem Zusammensetzen ist begrenzt und getestet; kein Test prüft eine Grenze für die Größe eines einzelnen Frames (z. B. eine angekündigte Länge von 2^60).

### §10.6 TLS unterstützen

**Spec:** §10.6, S. 53 (RFC) — "WebSocket implementations MUST support TLS and SHOULD employ it when communicating with their peers."

**Repo:** `crates/websocket-bridge/src/daemon/security.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`, `crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`

**Status:** done

### §10.7 Ungültige Daten

**Spec:** §10.7, S. 53 (RFC) — "Incoming data MUST always be validated by both clients and servers." Nach dem Handshake "the endpoint SHOULD send a Close frame with an appropriate status code (Section 7.4)"; während des Handshakes "the server SHOULD return an appropriate HTTP [RFC2616] status code."

**Repo:** `crates/websocket-bridge/src/codec.rs`, `crates/websocket-bridge/src/masking.rs`, `crates/websocket-bridge/src/utf8.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`

**Status:** partial — ein Masking-Verstoß schließt mit 1002, eine ungültige Anmeldung im Handshake bekommt HTTP 401. Nicht einzeln geprüft: der Close-Code für jede andere Art ungültiger Daten nach dem Handshake.

### §11 IANA Considerations

**Spec:** §11, S. 54–66 (RFC) — Registrierung der URI-Schemes `ws` und `wss`, des Upgrade-Tokens, der Header-Felder und der Registries für Extensions, Subprotokolle, Versionen, Close-Codes und Opcodes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Anweisungen an die IANA; die Werte selbst sind als Items zu §3, §4, §5.2 und §7.4 geführt.

### §12 Using the WebSocket Protocol from Other Specifications

**Spec:** §12, S. 66 (RFC) — wie eine andere Spezifikation eine WebSocket-Verbindung aufbaut und nutzt.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — richtet sich an andere Spezifikationen.

### §13, §14 Acknowledgements und References

**Spec:** §13–§14, S. 67–71 (RFC) — Danksagung, Verweise.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Danksagung und Verweislisten.

---

## Audit-Status

24 done / 3 partial / 1 open / 0 blocked / 6 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document websocket-rfc-6455.md`):

* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (Job `merge-rust-websocket-bridge`) — 138 Tests grün.

138 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `websocket-rfc-6455.open.md`.
