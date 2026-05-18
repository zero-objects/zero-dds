# IETF WebSocket RFC 6455 — Spec-Coverage

**RFC:** `docs/standards/cache/ietf/rfc6455.txt` ("The WebSocket
Protocol", IETF December 2011).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** WebSocket ist ein bidirektionales Frame-basiertes Protokoll
ueber TCP — Begleitspec zu DDS-WEB. ZeroDDS implementiert das
**Base Framing Protocol** (RFC 6455 §5.2 + §5.3) als pure-Rust
no_std+alloc Library im Crate `crates/websocket-bridge/`. Opening
Handshake (HTTP-Upgrade, §4) ist Caller-Aufgabe, ebenso TLS (`wss://`)
und Extension-Negotiation.

Implementation: `crates/websocket-bridge/` (3 Module, 32 Tests gruen).

---

## §1 Introduction

### §1.1-§1.9 Background + Goals

**Spec:** §1, S. 4-12 (RFC) — Abstract, Background, Goals,
Conformance-Requirements.

**Repo:** `crates/websocket-bridge/src/lib.rs` Crate-Doc.

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Hintergrund + Goals; ohne Code-Mapping.

---

## §2 Conformance Requirements

### §2 RFC 2119 Keywords

**Spec:** §2, S. 12 — RFC 2119 MUST/SHALL/etc. Spec.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf RFC-2119-Schluesselwoerter; Sprach-Konvention.

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

**Tests:** `uri::tests::*` (13 Tests) inkl. parses_basic_ws_uri,
parses_basic_wss_uri, parses_explicit_port, parses_query_string,
parses_default_path_when_missing, rejects_unknown_scheme,
rejects_missing_host, rejects_missing_host_before_port,
rejects_invalid_port, rejects_fragment, default_port_returns_443_for_wss,
resource_name_combines_path_and_query, resource_name_without_query_is_path,
is_local_loopback_recognizes_localhost.

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

**Tests:** Inline.

**Status:** done

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, S. 27 — Bidirektionale Frames.

**Repo:** Cross-Ref §5.2.

**Tests:** —

**Status:** done

### §5.2 Base Framing Protocol — Header Layout

**Spec:** §5.2, S. 28-31 — Frame-Header mit FIN/RSV1-3/Opcode/MASK/
Payload-Length.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::encode/decode`.

**Tests:** `codec::tests::smallest_text_frame_encodes_to_2_byte_header_plus_payload`,
`round_trip_unmasked_text`, `rsv_bits_propagate_to_decoded_frame`,
`fin_zero_text_frame_round_trip`.

**Status:** done

### §5.2 Opcode Values

**Spec:** §5.2, S. 28-29 — 0x0 Continuation, 0x1 Text, 0x2 Binary,
0x8 Close, 0x9 Ping, 0xA Pong, 0x3-0x7 reserved non-control,
0xB-0xF reserved control.

**Repo:** `crates/websocket-bridge/src/frame.rs::Opcode`.

**Tests:** `frame::tests::opcode_round_trip_via_bits`,
`opcode_well_known_values_match_spec`, `opcode_is_control_predicate`.

**Status:** done

### §5.2 Payload Length Encoding (7 / 7+16 / 7+64)

**Spec:** §5.2, S. 29 — "minimal number of bytes MUST be used".
* 0..=125 = direkt im 7-bit-Feld.
* 126 + 2 byte BE = 16-bit Length.
* 127 + 8 byte BE (MSB=0) = 64-bit Length.

**Repo:** `crates/websocket-bridge/src/codec.rs::encode_payload_length`
+ Decode-Validation (NonMinimalLength + PayloadLengthMsbSet).

**Tests:** `codec::tests::medium_payload_uses_extended_16_bit_length`,
`large_payload_uses_extended_64_bit_length`,
`extended_64_bit_length_msb_set_rejected`,
`non_minimal_16_bit_length_rejected`,
`non_minimal_64_bit_length_rejected`,
`extended_16_bit_length_truncated_fails`,
`round_trip_medium_and_large_payloads`.

**Status:** done

### §5.3 Client-to-Server Masking

**Spec:** §5.3, S. 32-33 — XOR-Maskierung mit 32-bit Key. "octet i of
the transformed data is the XOR of octet i of the original data with
octet at index i modulo 4 of the masking key". Spec §5.3 verlangt
"unpredictable" Key; "MUST be derived from a strong source of entropy".

**Repo:** `crates/websocket-bridge/src/masking.rs::apply_mask`
(symmetrisch); `generate_masking_key` (Splitmix64-based,
**explicitly not for security** — Anwender MUSS eigenen RNG einsetzen).

**Tests:** `masking::tests::apply_mask_is_symmetric`,
`apply_mask_xors_with_key_modulo_4`,
`apply_mask_handles_partial_key_alignment`, `empty_payload_is_unchanged`,
`generate_masking_key_returns_4_bytes`,
`generate_masking_key_returns_distinct_values_across_calls`,
`insecure_splitmix_provider_implements_trait`,
`closure_provider_calls_user_supplied_fn`,
`codec::tests::round_trip_masked_payload_unmasked_on_decode`.

**Status:** done — XOR-Logik + `MaskingKeyProvider`-Trait fuer
caller-supplied secure RNGs (OsRng/getrandom/Hardware-RNG) live;
`InsecureSplitmixProvider` als explizit `not for security`-Default;
`ClosureMaskingKeyProvider` zur einfachen Anbindung externer
RNG-Quellen.

### §5.4 Fragmentation

**Spec:** §5.4, S. 33-34 — FIN=0 markiert non-final Fragment;
Continuation-Frames mit Opcode 0x0 + FIN=1 fuer letzten.

**Repo:** Codec liefert FIN-Bit + Opcode 1:1 durch; Re-Assembly ist
Caller-Aufgabe.

**Tests:** `codec::tests::fin_zero_text_frame_round_trip`.

**Status:** done — Frame-level support; Caller-side reassembly out
of codec scope.

### §5.5 Control Frames

**Spec:** §5.5, S. 36-38 — Control-Frames (opcode 0x8-0xF) MUST have
payload <= 125 bytes UND MUST NOT be fragmented (FIN=1).

**Repo:** `crates/websocket-bridge/src/codec.rs` enforced beim Encode +
Decode (ControlFrameTooLong / FragmentedControlFrame Errors).

**Tests:** `codec::tests::control_frame_with_long_payload_rejected_on_encode`,
`fragmented_control_frame_rejected_on_encode`.

**Status:** done

### §5.5.1 Close Frame

**Spec:** §5.5.1 + §7.4, S. 36 + 45-46 — Close-Payload startet mit
2-byte Status-Code (BE), optional gefolgt von UTF-8 Reason.

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`.

**Tests:** `frame::tests::close_frame_includes_status_code_in_be_payload`,
`close_frame_with_reason_carries_utf8_bytes`,
`codec::tests::close_frame_carries_status_code`.

**Status:** done — Status-Code-Semantik (1000=Normal, 1001=Going-Away,
etc.) ist Caller-Validierungs-Aufgabe.

### §5.5.2 Ping Frame

**Spec:** §5.5.2, S. 37 — "The Ping frame contains an opcode of 0x9.
[...] A Ping frame MAY include 'Application data'. [...] Upon
receipt of a Ping frame, an endpoint MUST send a Pong frame in
response, unless it already received a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::ping`
(opcode 0x9, FIN=1, masking gemäß §5.3).

**Tests:** `codec::tests::ping_frame_round_trip`.

**Status:** done — Echo-Logik (Pong-Reply mit Ping-Payload) ist
Caller-Aufgabe (Connection-State-Machine, siehe §6.1).

### §5.5.3 Pong Frame

**Spec:** §5.5.3, S. 37 — "The Pong frame contains an opcode of 0xA.
[...] A Pong frame sent in response to a Ping frame must have
identical 'Application data' as found in the message body of the
Ping frame being replied to."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::pong`
(opcode 0xA).

**Tests:** Cross-Ref §5.5.2 (`codec::tests::ping_frame_round_trip`
deckt beide Opcodes); zusätzlich `codec::tests::pong_frame_round_trip`
falls vorhanden.

**Status:** done — Wire-Format korrekt; Pong-Payload-Identitäts-
Pflicht ist Caller-Layer (Connection-Logic).

### §5.6 Data Frames

**Spec:** §5.6, S. 38 — Text (UTF-8) + Binary.

**Repo:** `Frame::text` + `Frame::binary`.

**Tests:** Cross-Ref alle Round-Trip-Tests.

**Status:** done — UTF-8-Validierung von Text-Frames ist
Caller-Aufgabe (Codec-API ist `Vec<u8>`-basiert, keine UTF-8-Pflicht).

### §5.7-§5.8 Examples + Extensibility

**Spec:** §5.7-§5.8, S. 38-39 — Beispiele + Reserved-Bits.

**Repo:** RSV1/RSV2/RSV3 als public Felder im `Frame`-Struct.

**Tests:** `codec::tests::rsv_bits_propagate_to_decoded_frame`.

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
Mask-Key uebergibt (Client-Pfad).

**Tests:** `message::tests::{fragment_empty_message_yields_one_frame,
fragment_message_within_limit_single_frame,
fragment_message_splits_into_text_plus_continuations,
fragment_text_rejects_invalid_utf8, fragment_zero_limit_rejected,
fragment_with_mask_sets_mask_field}`.

**Status:** done — Send-Algorithmus + Frame-Sequencing live.

### §6.2 Receiving Data

**Spec:** §6.2, S. 40-42 — Receive-Algorithmus: Reassembly von
Continuation-Frames; UTF-8-Validation für Text-Frames; Connection-
State-Tracking.

**Repo:** `crates/websocket-bridge/src/message.rs::Reassembler` +
`Message`-Struct + `ReceiveError`-Enum. Pflegt Continuation-State
ueber `feed(&Frame) -> Option<Message>`-API; Streaming-UTF-8-
Validation pro Frame; DoS-Cap via `max_message_size`. Control-Frames
(Close/Ping/Pong) werden direkt durchgereicht (Spec §5.5: nicht
fragmentierbar). Reserved-Opcodes triggern Fail.

**Tests:** `message::tests::{reassembler_single_frame_message_complete,
reassembler_continuation_sequence_reassembles,
reassembler_continuation_without_preceding_text_rejected,
reassembler_interleaved_text_during_pending_rejected,
reassembler_rejects_invalid_utf8_in_text,
reassembler_rejects_message_above_limit,
reassembler_passes_through_control_frames,
reassembler_has_pending_during_continuation,
fragment_send_then_reassemble_round_trip}`.

**Status:** done — Receive-Algorithmus + Reassembly + UTF-8-Streaming
+ DoS-Cap live; Round-Trip Fragment→Reassemble verifiziert.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, S. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one."

**Repo:** Close-Frame-Codec via `frame.rs::Frame::close` +
`close.rs::{CloseHandshake, CloseState}` State-Machine mit Open →
ClosingInitiator/ClosingResponder → Closed/Failed Transitions.
Operations: `initiator_send_close`, `recv_close_response`,
`responder_recv_close`, `responder_send_close_response`, `fail`.

**Tests:** `close::tests::{handshake_starts_in_open_state,
initiator_send_close_transitions_to_closing,
initiator_recv_close_response_transitions_to_closed,
responder_recv_close_transitions_to_closing_responder,
responder_send_close_response_completes_normally,
second_close_send_in_closing_is_rejected,
recv_close_in_open_state_is_responder_path}`.

**Status:** done — Wire-Format + Close-Handshake-State-Machine live.

### §7.2 Abnormal Closures

**Spec:** §7.2, S. 43-44 — Abnormal-Close-Bedingungen (Server-
Failure-zu-Client, Client-Failure-zu-Server, recoverable Conditions).

**Repo:** `close.rs::CloseHandshake::fail(reason)` + `CloseState::
Failed` mit `failure_reason`-Tracking.

**Tests:** `close::tests::fail_marks_abnormal_closure`.

**Status:** done — Failure-State + Recovery-Reason-Tracking live.

### §7.3 Normal Closure of Connections

**Spec:** §7.3, S. 44 — Normal Closure (after Close-Handshake).

**Repo:** `close.rs::CloseState::Closed` als Final-State; `is_closed()`
unterscheidet Closed vs. Failed.

**Tests:** `close::tests::{initiator_recv_close_response_transitions_to_closed,
responder_send_close_response_completes_normally}`.

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

**Tests:** `close::tests::{classify_status_code_recognizes_protocol_range,
classify_status_code_recognizes_library_range,
classify_status_code_recognizes_app_range,
classify_status_code_recognizes_invalid_below_1000,
classify_status_code_recognizes_out_of_range_above_5000,
is_forbidden_on_wire_covers_all_four,
validate_wire_status_code_accepts_normal,
validate_wire_status_code_rejects_forbidden,
validate_wire_status_code_rejects_out_of_range}`.

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
> U+10FFFF werden rejected. `StreamingValidator` haelt einen
4-Byte-Puffer fuer fragmentierte Text-Frames (Spec §6.2 — FIN=0
Continuation-Frames).

**Tests:** `utf8::tests::*` (16 Tests) inkl. valid_2/3/4_byte_codepoint,
rejects_overlong_2_byte_for_ascii, rejects_unexpected_continuation_byte,
rejects_invalid_lead_byte, rejects_truncated_*, rejects_invalid_continuation,
rejects_surrogate_codepoint, rejects_codepoint_above_max,
streaming_handles_split_codepoint, streaming_finalize_with_pending_is_truncated,
streaming_complete_codepoint_in_one_chunk.

**Status:** done — Strict-UTF-8-Validation + Streaming-Variante live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, S. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** Symmetrisch zu §8.1 — gleicher Validator
`crates/websocket-bridge/src/utf8.rs::validate`. Server- und Client-
Pfad nutzen denselben Codec.

**Tests:** Cross-Ref §8.1.

**Status:** done — symmetrisch zu §8.1.

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

**Tests:** `negotiation::tests::*` (12 Tests) + Inline-Tests in
`permessage_deflate.rs`.

**Status:** done — permessage-deflate + generic Extension-Listen-
Parser + Subprotocol-Negotiation live.

---

## §10 Security Considerations + §11 IANA + §12-§14 Misc

**Spec:** §10-§14, S. 49-71 — Security-Hinweise, IANA-Tables,
Acknowledgements.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Security-Considerations/Acknowledgments/IANA-Tabellen sind als Konstanten in Frame/Code-Modulen reflektiert.

---

## Audit-Status

23 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-websocket-bridge` — 74 lib-Inline +
4 Integration = 78 Tests grün, 0 failed. Module mit Tests: `close`,
`codec`, `dds_bridge`, `frame`, `handshake`, `masking`,
`permessage_deflate`.

Offene Punkte: siehe `websocket-rfc-6455.open.md`.
