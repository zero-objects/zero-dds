# IETF WebSocket RFC 6455 — Spec-Coverage

**RFC:** [IETF RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) ("The WebSocket
Protocol", IETF December 2011).

**Kontext:** WebSocket ist ein bidirektionales, Frame-basiertes Protokoll
über TCP — Begleitspec zu DDS-WEB. ZeroDDS implementiert den Opening
Handshake (§4), das Framing (§5), Senden und Empfangen von Nachrichten (§6),
den Closing Handshake (§7), die UTF-8-Prüfung (§8) und die Aushandlung von
Extensions (§9) mit `permessage-deflate` (RFC 7692) als pure-Rust-Bibliothek
(no_std + alloc; die Zufallsquelle des Betriebssystems und DEFLATE mit
`std`). Der Daemon `zerodds-ws-bridged` derselben Krate ist ein Server auf
dieser Bibliothek, mit TLS (`wss://`).

Implementation:

- `crates/websocket-bridge/` — Bibliothek: `handshake` (§4), `frame`, `codec`, `masking` (§5), `message` (§6), `close` (§7), `utf8` (§8), `negotiation`, `permessage_deflate` (§9), `connection` (ein Ende einer Verbindung: `Receiver`, `Sender`), `origin` (§10.2), `uri` (§3). Daemon: `daemon/`.
- `crates/mqtt-bridge/src/daemon/ws_transport.rs` — der WebSocket-Client des MQTT-Daemons (`ws://`, `wss://`): Handshake und Codec dieser Krate, die Empfangsschleife mit eigenem Code (Item mqtt-5.0 §6).

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
Sec-WebSocket-Accept = SHA-1(key + MAGIC) base64, etc.). §4.2.1: der
Request ist ein GET mit HTTP/1.1 oder höher, mit `Host`, `Upgrade`,
`Connection`, einem Schlüssel aus 16 Oktetten und Version 13; sonst „the
server MUST stop processing the client's handshake and return an HTTP
response with an appropriate error code". §4.2.2 /version/, §4.4: bei einer
Version, die der Server nicht spricht, nennt die Antwort seine Versionen in
`Sec-WebSocket-Version`.

**Repo:** `crates/websocket-bridge/src/handshake.rs::{ClientHandshake, ServerHandshake,
HandshakeError, compute_accept, parse_client_request, build_server_response,
render_server_response, client_key, render_client_request,
render_client_request_with_extensions, validate_server_response,
validate_server_response_with_extensions}`. Server: `parse_client_request`
prüft Methode, HTTP-Version, `Host`, `Upgrade`, `Connection`, Schlüssel und
Version; `HandshakeError::{http_status, render_error_response}` geben die
HTTP-Antwort des Fehlers: 405 mit `Allow: GET`, 426 mit
`Sec-WebSocket-Version: 13`, sonst 400. Client: die Antwort des Servers wird
gegen Status 101, `Upgrade`, `Connection`, `Sec-WebSocket-Accept` und gegen
die angebotenen Subprotokolle und Extensions geprüft (§4.1). Der Daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sendet die Antwort des
Fehlers und schließt; ein Request, der kein UTF-8 ist, bekommt 400.

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
— `ws_4_1_rfc6455_section_1_3_accept_test_vector` prüft den Sec-WebSocket-Accept-
Testvektor der Spec (SHA-1 aus Key + GUID, base64). Gegen den Daemon:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`.
Mit dem Client von Python `websockets` als Gegenseite:
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_round_trips_a_text_frame`.

**Status:** done — die Prüfung von `Origin` (§4.2.2 /origin/) ist das Item §10.2.

---

## §5 Data Framing

### §5.1 Overview

**Spec:** §5.1, S. 27 — „The client MUST mask all frames that it sends
to the server. […] The server MUST close the connection upon receiving
a frame that is not masked. […] A server MUST NOT mask any frames that
it sends to the client. A client MUST close a connection if it detects
a masked frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{Role, MaskingViolation, validate_masking,
check_masking}` — die Richtungsregel; der Verstoß hat den Close-Code 1002
(`MaskingViolation::close_code`). `crates/websocket-bridge/src/connection.rs::Receiver` prüft
das MASK-Bit jedes Frames am Header, bevor die Nutzlast gepuffert wird: als
Server weist er ein unmaskiertes, als Client ein maskiertes Frame mit
`Failure::Masking` ab. `crates/websocket-bridge/src/connection.rs::Sender::server` sendet unmaskiert,
`Sender::client` maskiert jedes Frame. Der Daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) liest jedes Frame über
`Receiver`, sendet bei dem Verstoß das Close-Frame mit 1002 und schließt die
Verbindung; das Frame wird nicht verarbeitet.

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_client_masks_and_server_does_not_per_spec_5_1`,
`crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_1_the_server_fails_an_unmasked_frame_with_1002,
ws_5_1_the_client_fails_a_masked_frame_with_1002,
ws_5_1_an_unmasked_frame_is_refused_from_its_header}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002`.
Dass der Server unmaskiert sendet, prüft die Gegenseite: Python `websockets`
(`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_round_trips_binary_and_ping`)
und der Fuzzing-Server der Autobahn|TestSuite, dem die Krate als Client
maskierte Frames sendet
(`crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_autobahn_1_framing_and_2_pings`).

**Status:** done

### §5.2 Base Framing Protocol — Header Layout

**Spec:** §5.2, S. 28-31 — Frame-Header mit FIN/RSV1-3/Opcode/MASK/
Payload-Length. RSV1-3: „MUST be 0 unless an extension is negotiated that
defines meanings for non-zero values. If a nonzero value is received and
none of the negotiated extensions defines the meaning of such a nonzero
value, the receiving endpoint MUST _Fail the WebSocket Connection_."
Opcode: „If an unknown opcode is received, the receiving endpoint MUST
_Fail the WebSocket Connection_."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame` +
`crates/websocket-bridge/src/codec.rs::{encode, decode, decode_with_limit, peek_header,
FrameHeader, CodecError}`. `crates/websocket-bridge/src/connection.rs::Receiver` weist am
Header ab: ein RSV-Bit, das keine ausgehandelte Extension definiert
(`Failure::ReservedBit`; mit `permessage-deflate` ist RSV1 auf dem ersten
Frame einer Datennachricht definiert), einen reservierten Opcode
(`Failure::ReservedOpcode`) und einen Header gegen §5.2 oder §5.5
(`Failure::Frame`), jeweils mit dem Close-Code 1002. Der Daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sendet das Close-Frame und
schließt.

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::{ws_5_2_smallest_text_frame_encodes_to_2_byte_header_plus_payload,
ws_5_7_a_single_frame_text_message_unmasked_and_masked, ws_5_2_rsv_bits_propagate_to_decoded_frame,
ws_5_7_a_fragmented_text_message}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_2_a_reserved_bit_without_an_extension_fails_with_1002,
ws_5_2_a_reserved_opcode_fails_with_1002, ws_5_2_a_length_that_is_not_minimal_fails_with_1002}`;
gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002`;
gegen die Autobahn|TestSuite (Fälle 1 bis 4)
`crates/websocket-bridge/tests/autobahn_e2e.rs::{ws_5_autobahn_1_framing_and_2_pings,
ws_5_2_autobahn_3_reserved_bits_and_4_opcodes}`.

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
octet at index i modulo 4 of the masking key". „The masking key needs to be
unpredictable; thus, the masking key MUST be derived from a strong source
of entropy, and the masking key for a given frame MUST NOT make it simple
for a server/proxy to predict the masking key for a subsequent frame."

**Repo:** `crates/websocket-bridge/src/masking.rs::{apply_mask, generate_masking_key,
MaskingKeyProvider, OsRngProvider, ClosureMaskingKeyProvider,
EntropyError}`. `generate_masking_key` und `OsRngProvider` (Feature `std`)
lesen je Schlüssel vier Oktette aus der Zufallsquelle des Betriebssystems
(`getrandom`); kein Zustand der Krate geht in den Schlüssel ein. Ohne `std`
liefert der Aufrufer die Quelle über `MaskingKeyProvider`; die Krate hat
keinen eigenen Generator. `crates/websocket-bridge/src/connection.rs::Sender::client` und
`crates/websocket-bridge/src/message.rs::fragment_message` ziehen je Frame einen Schlüssel;
liefert die Quelle keinen, entsteht kein Frame (`SendError::Entropy`).

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
`ws_5_3_the_masking_key_comes_from_the_operating_system` zieht 4096
Schlüssel: sie sind nicht die Folge eines festen Startwerts, jedes der 32
Bits ist in 40 bis 60 Prozent gesetzt, weniger als drei wiederholen sich.

**Status:** done

### §5.4 Fragmentation

**Spec:** §5.4, S. 33-34 — FIN=0 markiert non-final Fragment;
Continuation-Frames mit Opcode 0x0 + FIN=1 für letzten. Control-Frames
dürfen zwischen den Fragmenten stehen; die Fragmente einer Nachricht
dürfen nicht mit denen einer anderen verschachtelt sein.

**Repo:** Senden: `crates/websocket-bridge/src/message.rs::fragment_message`,
`crates/websocket-bridge/src/connection.rs::Sender::with_max_frame_payload`. Empfangen:
`crates/websocket-bridge/src/connection.rs::Receiver` setzt die Fragmente über
`crates/websocket-bridge/src/message.rs::Reassembler` zusammen und liefert die Nachricht als
ein `Event::Message`; ein Continuation-Frame ohne begonnene Nachricht und
ein neues Daten-Frame in einer begonnenen sind `Failure::Message` mit dem
Close-Code 1002. Der Daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`)
verarbeitet die zusammengesetzte Nachricht.

**Tests:** `crates/websocket-bridge/src/codec.rs::tests::ws_5_7_a_fragmented_text_message`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_4_a_fragmented_message_is_delivered_whole,
ws_5_4_control_frames_may_come_between_the_fragments,
ws_5_4_a_continuation_without_a_message_fails_with_1002,
ws_5_4_a_new_data_frame_inside_a_message_fails_with_1002}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_5_4_the_daemon_reassembles_a_fragmented_message,
ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002}` (ein
`publish` in zwei Fragmenten ist ein Sample in DDS); gegen die
Autobahn|TestSuite (Fälle 5) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_4_autobahn_5_fragmentation`;
mit Python `websockets`, das eine Nachricht in drei Fragmenten sendet,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_negotiates_permessage_deflate_and_round_trips`.

**Status:** done

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
2-byte Status-Code (BE), optional gefolgt von UTF-8 Reason. „If an endpoint
receives a Close frame and did not previously send a Close frame, the
endpoint MUST send a Close frame in response." „The application MUST NOT
send any more data frames after sending a Close frame."

**Repo:** `crates/websocket-bridge/src/frame.rs::Frame::close`;
`crates/websocket-bridge/src/close.rs::{parse_close_body, CloseBodyError}` — der Body ist leer
oder ein Status-Code, der auf der Leitung stehen darf (§7.4), mit einem
Grund in UTF-8; sonst 1002 bzw. 1007. `crates/websocket-bridge/src/connection.rs`:
`Receiver` liefert `Event::Close` und liest danach nichts mehr;
`Sender::close_received` gibt das Close-Frame der Antwort mit dem
empfangenen Status-Code, `Sender::close` beginnt den Handshake, nach dem
gesendeten Close baut `Sender` kein Frame mehr (`SendError::Closing`). Der
Daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) beantwortet das Close,
beendet seine Senderichtung und liest, bis der Client schließt.

**Tests:** `crates/websocket-bridge/src/frame.rs::tests::{ws_5_5_1_close_frame_includes_status_code_in_be_payload,
ws_5_5_1_close_frame_with_reason_carries_utf8_bytes}`,
`crates/websocket-bridge/src/codec.rs::tests::ws_5_5_1_close_frame_carries_status_code`,
`crates/websocket-bridge/src/close.rs::tests::{ws_5_5_1_a_close_body_without_reason_is_the_code_alone,
ws_5_5_1_a_close_body_is_empty_or_a_status_code_and_a_utf8_reason}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_5_5_1_a_close_is_delivered_with_its_status_and_ends_the_reading,
ws_5_5_1_an_invalid_close_body_fails_the_connection,
ws_5_5_1_a_close_is_answered_with_a_close_of_the_same_status,
ws_5_5_1_no_frame_follows_the_close_this_end_sent}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status,
ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007}`; gegen
die Autobahn|TestSuite (Fälle 7) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`;
mit Python `websockets`, das den Status-Code der Antwort ausgibt,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`.

**Status:** done

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
+ `SendError`-Enum. Splittet logische Messages in Frame-Sequenzen
(Text/Binary mit FIN=0, Continuation-Frames, letztes Frame mit FIN=1);
UTF-8-Validation auf Text-Payload vor Splitting; mit einer Schlüsselquelle
(`keys`, der Client-Pfad) trägt jedes Frame einen eigenen Masking-Key.
`crates/websocket-bridge/src/connection.rs::Sender::message` baut darauf die Frames einer
Nachricht, mit `permessage-deflate` komprimiert.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_6_1_fragment_empty_message_yields_one_frame,
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
Validation pro Frame; DoS-Cap via `max_message_size`.
`crates/websocket-bridge/src/connection.rs::Receiver::feed` ist der Empfangsalgorithmus über
dem Oktettstrom: Frame prüfen, Control-Frames als `Event::Ping`,
`Event::Pong`, `Event::Close` liefern, Datennachrichten zusammensetzen,
dekomprimieren und als `Event::Message` liefern. Der Daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) liest darüber.

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
ws_6_2_a_code_point_split_over_fragments_is_valid}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_reassembles_a_fragmented_message`.

**Status:** done — Receive-Algorithmus + Reassembly + UTF-8-Streaming
+ DoS-Cap live; Round-Trip Fragment→Reassemble verifiziert.

---

## §7 Closing the Connection

### §7.1 Closing the Connection

**Spec:** §7.1, S. 42-43 — "To Start the WebSocket Closing Handshake
[...] one endpoint sends a Close control frame to the other endpoint
[...] After receiving such a frame, the other endpoint sends a Close
frame in response, if it hasn't already sent one." §7.1.1: der Server
schließt die TCP-Verbindung zuerst. §7.1.7: Fail the WebSocket Connection.

**Repo:** `crates/websocket-bridge/src/close.rs::{CloseHandshake, CloseState}` — State-Machine
mit Open → ClosingInitiator/ClosingResponder → Closed/Failed
(`initiator_send_close`, `recv_close_response`, `responder_recv_close`,
`responder_send_close_response`, `fail`). `crates/websocket-bridge/src/connection.rs::Sender`
führt sie: `close` (§7.1.2), `close_received` (die Antwort, §7.1.4),
`fail` (§7.1.7: das Close-Frame mit dem Status-Code des Fehlers, einmal).
Der Daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`): ein Close des
Clients wird beantwortet, ungültige Daten beenden die Verbindung mit dem
Close-Frame ihres Status-Codes, beim Herunterfahren sendet der Daemon auf
jeder offenen Verbindung 1001 (`WsOut::going_away`, vom Thread der
Verbindung, sobald er das Stop-Flag sieht). In allen drei Fällen beendet er
danach die Senderichtung (TLS `close_notify`, FIN) und liest, bis der Client
schließt, höchstens 500 ms. `DaemonHandle::shutdown` wartet, bis die
Verbindungen beendet sind, höchstens 2 s (`SHUTDOWN_CLOSE_WAIT`,
`ConnectionThreads`); der Prozess endet danach.

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
ws_7_1_7_a_failed_connection_sends_its_close_once}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status`
(nach dem Close-Frame liest der Client das Ende der TCP-Verbindung),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_closes_the_connection_when_the_client_does_not`
(nach einem Close des Clients und nach einem unmaskierten Frame: 250 ms nach
dem Close-Frame des Daemons hält er die Verbindung und nimmt Oktette an,
nach 1 s ist sie beendet); das Herunterfahren
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_daemon_closes_every_connection_with_1001_when_it_shuts_down`
(16 Verbindungen, je Close 1001 und das Ende der TCP-Verbindung),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_reads_on_behind_the_close_of_its_shutdown`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_binary_closes_every_connection_with_1001_on_sigterm`
(das Binary, SIGTERM, 40 Verbindungen); über TLS
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`
(1002, das beantwortete Close, 1001 beim Herunterfahren, je danach
`close_notify` und FIN); gegen
die Autobahn|TestSuite (Fälle 7) `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`.

**Status:** done

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
> U+10FFFF werden rejected. `StreamingValidator` prüft Oktett für Oktett
über Frame-Grenzen (Spec §6.2 — FIN=0 Continuation-Frames) und meldet den
Fehler mit dem Oktett, das den Strom ungültig macht.
`crates/websocket-bridge/src/connection.rs::Receiver` (als Client) prüft damit jede
Textnachricht, auch ein Frame, das noch nicht vollständig ist, und scheitert
mit `Failure::Message`, Close-Code 1007.

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
ws_8_1_streaming_complete_codepoint_in_one_chunk}` (18); der Fail-the-Connection-
Pfad mit Close-Code 1007:
`crates/websocket-bridge/src/message.rs::tests::ws_8_1_client_fails_connection_with_1007_on_malformed_utf8_from_server`,
`crates/websocket-bridge/src/connection.rs::tests::ws_8_1_malformed_utf8_from_the_server_fails_with_1007`.
Die Krate als Client gegen den Fuzzing-Server der Autobahn|TestSuite (Fälle
6, 145 Fälle mit gültigem und ungültigem UTF-8, ganz, in Fragmenten und
oktettweise gesendet): `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_8_1_autobahn_6_utf8`.

**Status:** done — Strict-UTF-8-Validation + Streaming-Variante +
Close-Code-1007-Pfad live.

### §8.2 Handling Errors in UTF-8 from the Client

**Spec:** §8.2, S. 46 — "When a server receives a Text-frame from
the client that contains malformed UTF-8 data [...] the server MUST
fail the WebSocket Connection."

**Repo:** Derselbe Validator `crates/websocket-bridge/src/utf8.rs::{validate,
StreamingValidator}` für beide Richtungen;
`crates/websocket-bridge/src/message.rs::ReceiveError::{close_code, close_frame}` bildet den
UTF-8-Fehler auf Close-Code 1007 (`CloseCode::InvalidPayloadData`, Spec
§7.4.1) ab. `crates/websocket-bridge/src/connection.rs::Receiver` (als Server) prüft jede
Textnachricht, über Fragmente hinweg und in einem Frame, das noch nicht
vollständig ist (die vorhandenen Oktette, demaskiert). Der Daemon
(`crates/websocket-bridge/src/daemon/server.rs::serve_connection`) sendet das Close-Frame mit
1007 und schließt; die Nachricht erreicht die Verarbeitung nicht.

**Tests:** `crates/websocket-bridge/src/message.rs::tests::{ws_8_2_server_fails_connection_with_1007_on_malformed_utf8_from_client,
ws_8_2_malformed_utf8_split_across_continuation_frames_still_yields_1007,
ws_8_2_receive_error_close_codes_match_spec_7_4_1}`,
`crates/websocket-bridge/src/connection.rs::tests::{ws_8_2_malformed_utf8_from_the_client_fails_with_1007,
ws_8_2_malformed_utf8_fails_with_the_octet_that_makes_it_malformed}`; gegen
den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007`
(ein Frame, über Fragmente, in einem unvollständigen Frame); die Server-Rolle
gegen den Fuzzing-Client der Autobahn|TestSuite (Fälle 6)
`crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_8_1_autobahn_server_6_utf8`.

**Status:** done — die Autobahn|TestSuite läuft gegen die Client-Rolle (§8.1)
und mit ihrem Fuzzing-Client gegen die Server-Rolle, einen Echo-Server auf
demselben `Receiver` (Wochenjob `integration-bridges-vendor-autobahn`; in CI
noch nicht gelaufen, auf dem Linux-Testhost von Hand grün).

### §9 Extensibility

**Spec:** §9, S. 47-49 — Extension-Negotiation via
`Sec-WebSocket-Extensions`-Header; Beispiele inkl. permessage-deflate
(RFC 7692). §9.1: der Client bietet Extensions in der Reihenfolge seiner
Präferenz an, der Server nennt in der Antwort die, denen er zustimmt; eine
Extension, die der Client nicht angeboten hat, lässt den Client scheitern
(§4.1).

**Repo:** `crates/websocket-bridge/src/negotiation.rs::{ExtensionOffer,
parse_extensions, parse_subprotocols, select_subprotocol,
SUBPROTOCOL_HEADER, EXTENSIONS_HEADER}` (Extension- und
Subprotocol-Listen); `crates/websocket-bridge/src/handshake.rs::{build_server_response,
validate_server_response_with_extensions}` (der Server nennt keine
Extension, die er nicht ausführt; der Client lehnt eine nicht angebotene
ab). `permessage-deflate` (RFC 7692) in
`crates/websocket-bridge/src/permessage_deflate.rs`: Aushandlung beider Seiten
(`parse_deflate_offer`, `agree`, `accept_offers`, `render_accept`,
`render_offer`, `accept_response`, §7.1: ein Angebot mit unbekanntem,
ungültigem oder wiederholtem Parameter wird abgelehnt, das nächste
geprüft) und der Codec (`Deflater`, `Inflater` über `flate2` mit `zlib-rs`,
§7.2: Fenster nach `server_max_window_bits`/`client_max_window_bits`,
Kontextübernahme, Grenze für die dekomprimierte Größe).
`crates/websocket-bridge/src/connection.rs::{Receiver, Sender}::with_permessage_deflate` ist
der Frame-Pfad: RSV1 auf dem ersten Frame einer Datennachricht, sonst 1002
(RFC 7692 §6). Der Daemon (`crates/websocket-bridge/src/daemon/server.rs::serve_connection`)
stimmt dem ersten gültigen Angebot zu, nennt es in der Antwort und
komprimiert die Nachrichten der Verbindung in beiden Richtungen. Ein
Fenster von 8 Bit für die eigene Senderichtung hält der Sender ein, indem
er unkomprimiert sendet (der DEFLATE-Kompressor hat kein Fenster von 256
Oktetten).

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
(die Oktette der Beispiele aus RFC 7692 §7.2.3);
`crates/websocket-bridge/src/connection.rs::tests::deflate::{rfc7692_6_1_the_frames_of_the_rfc_are_received_as_hello,
rfc7692_7_2_3_2_the_second_hello_refers_to_the_window_of_the_first,
rfc7692_6_1_rsv1_on_a_control_or_continuation_frame_fails_with_1002,
rfc7692_6_a_sent_message_is_compressed_and_received_again,
rfc7692_7_2_1_context_takeover_makes_the_repeated_message_shorter,
rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009,
rfc7692_6_2_a_compressed_text_message_is_utf8_after_decompression,
rfc7692_7_1_2_a_sender_with_a_window_of_8_bits_sends_uncompressed,
rfc7692_7_the_roles_use_the_parameters_of_their_direction}`; gegen den Daemon
`crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions,
ws_9_the_daemon_declines_an_offer_that_is_invalid}`. Mit dem zlib der
Gegenseite: Python `websockets`
(`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::python_websockets_client_negotiates_permessage_deflate_and_round_trips`,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`)
und die Autobahn|TestSuite (Fälle 12 und 13, 216 Fälle über Nachrichtengrößen
und jede Parameterkombination): die Tests `ws_9_autobahn_12_*` und `ws_9_autobahn_13_*` des Targets `crates/websocket-bridge/tests/autobahn_e2e.rs`, 54 Tests, darunter `crates/websocket-bridge/tests/autobahn_e2e.rs::{ws_9_autobahn_12_1_cases_1_to_10, ws_9_autobahn_12_2_case_10, ws_9_autobahn_12_3_case_18, ws_9_autobahn_12_4_cases_16_to_18, ws_9_autobahn_12_5_case_15, ws_9_autobahn_13_1_cases_1_to_10, ws_9_autobahn_13_3_cases_16_to_18, ws_9_autobahn_13_5_cases_11_to_15, ws_9_autobahn_13_7_cases_16_to_18}`.

**Status:** done

---

## §10-§14 Security + IANA + Misc

### §10.1, §10.5, §10.8 Nicht-Browser-Clients, Client-Authentifizierung, SHA-1

**Spec:** §10.1, §10.5, §10.8, S. 50–54 (RFC) — Hinweise zu Clients außerhalb eines Browsers, zur Authentifizierung über HTTP-Mechanismen und dazu, dass SHA-1 im Handshake keine Sicherheitsfunktion hat.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Erläuterungen ohne eigene Anforderung.

### §10.2 Origin prüfen

**Spec:** §10.2, S. 50 (RFC) — "Servers that are not intended to process input from any web page but only for certain sites SHOULD verify the |Origin| field is an origin they expect. If the origin indicated is unacceptable to the server, then it SHOULD respond to the WebSocket handshake with a reply containing HTTP 403 Forbidden status code."

**Repo:** `crates/websocket-bridge/src/origin.rs` (`OriginPolicy`, `Origin`: Vergleich nach RFC 6454 §5), `crates/websocket-bridge/src/handshake.rs` (`ClientHandshake::origin`, `HandshakeError::OriginNotExpected`, `HandshakeError::MissingOrigin`: HTTP 403), `crates/websocket-bridge/src/daemon/config.rs` (`origin.allowed`, `origin.required`), `crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`)

**Tests:** `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_an_origin_is_scheme_host_and_port`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_text_that_is_no_serialized_origin_is_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_an_expected_origin_passes_and_any_other_is_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_scheme_and_host_compare_without_case_and_the_default_port_is_no_port`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_handshake_without_origin_passes_unless_the_field_is_required`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_without_a_list_nothing_is_checked`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_the_star_expects_every_origin_and_null_is_its_own_entry`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_every_origin_of_a_list_in_the_field_has_to_be_expected`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_that_is_no_origin_and_required_without_a_list_are_refused`, `crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_with_a_pattern_in_its_host_is_refused`, `crates/websocket-bridge/src/handshake.rs::tests::ws_4_2_1_the_origin_field_is_read`, `crates/websocket-bridge/src/handshake.rs::tests::ws_10_2_an_origin_the_server_does_not_expect_gets_403`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_the_origin_keys_are_read`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_an_origin_setting_that_cannot_be_served_is_refused_with_its_line`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_accepts_an_origin_it_expects_and_a_client_without_one`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_refuses_a_handshake_without_origin_when_one_is_required`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_without_a_list_the_daemon_accepts_every_origin`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_checks_the_origin_before_the_credentials`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_does_not_start_with_an_origin_setting_it_cannot_serve`, `crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::ws_10_2_the_daemon_checks_the_origin_python_websockets_sends`

**Status:** done — der Daemon prüft `Origin` gegen die Liste `origin.allowed` und beantwortet einen Origin außerhalb der Liste mit HTTP 403, vor der Authentifizierung; ohne Liste nimmt er jeden Origin an (§4.2.2 /origin/). Ein Handshake ohne `Origin` wird angenommen (Client außerhalb eines Browsers), mit `origin.required: true` abgewiesen. Gegenstelle im Test: Python websockets (wöchentlicher Job `integration-bridges-vendor`; auf dem Linux-Testhost von Hand grün).

### §10.3 Neuer, nicht vorhersagbarer Masking-Key je Frame

**Spec:** §10.3, S. 51 (RFC) — "Clients MUST choose a new masking key for each frame, using an algorithm that cannot be predicted by end applications that provide data."

**Repo:** `crates/websocket-bridge/src/masking.rs` (`generate_masking_key`, `OsRngProvider`: die Zufallsquelle des Betriebssystems), `crates/websocket-bridge/src/message.rs` (`fragment_message`: ein Schlüssel je Frame), `crates/websocket-bridge/src/connection.rs` (`Sender::client`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream::send`: der WebSocket-Client des MQTT-Daemons, je Frame vier Oktette aus der Zufallsquelle des TLS-Providers), `examples/tutorials/dds-chat/integrations/websocket/src/bin/ws_e2e.rs` (Beispiel-Client: `Sender::client`)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_3_the_masking_key_comes_from_the_operating_system`, `crates/websocket-bridge/src/message.rs::tests::ws_10_3_each_frame_of_a_fragmented_message_has_its_own_key`, `crates/websocket-bridge/src/message.rs::tests::ws_10_3_a_zero_key_still_sets_the_mask_bit`, `crates/websocket-bridge/src/connection.rs::tests::ws_5_3_every_frame_of_a_client_carries_a_key_of_its_own`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_3_every_frame_of_the_mqtt_client_carries_a_key_of_its_own`

**Status:** done — die Krate zieht je Frame einen Schlüssel aus der Zufallsquelle des Betriebssystems und hat keinen vorhersagbaren Generator. Der WebSocket-Client in `crates/mqtt-bridge` zieht je Frame vier Oktette aus der Zufallsquelle des TLS-Providers; im Test sind alle seine Frames (CONNECT, 100 PUBLISH, DISCONNECT, Close) maskiert und kein Schlüssel kommt zweimal vor. Der Beispiel-Client `ws_e2e.rs` sendete seinen Frame mit dem festen Schlüssel 0 und einem festen `Sec-WebSocket-Key`; er zieht beide jetzt aus der Zufallsquelle des Betriebssystems.

### §10.4 Grenzen für Frame- und Nachrichtengröße

**Spec:** §10.4, S. 52 (RFC) — "Implementations that have implementation- and/or platform-specific limitations regarding the frame size or total message size after reassembly from multiple frames MUST protect themselves against exceeding those limits. [...] Such an implementation SHOULD impose a limit on frame sizes and the total message size after reassembly from multiple frames."

**Repo:** `crates/websocket-bridge/src/codec.rs` (`decode_with_limit`, `CodecError::FrameTooLarge`: am Header, bevor die Nutzlast gepuffert wird), `crates/websocket-bridge/src/message.rs` (`Reassembler::with_limit`, `max_message_size`), `crates/websocket-bridge/src/connection.rs` (`Limits`, `Receiver`: Close-Code 1009; die Grenze der Nachricht gilt nach dem Dekomprimieren), `crates/websocket-bridge/src/daemon/config.rs` (`limits.max_frame_size`, `limits.max_message_size`: je 16 MiB als Default), `crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`MAX_FRAME_PAYLOAD`: die Grenze des WebSocket-Clients des MQTT-Daemons, ein MQTT-Paket maximaler Größe)

**Tests:** `crates/websocket-bridge/src/message.rs::tests::ws_6_2_reassembler_rejects_message_above_limit`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_a_frame_announcing_2_pow_60_is_refused_from_its_header`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_a_frame_at_the_limit_decodes`, `crates/websocket-bridge/src/codec.rs::tests::ws_10_4_incomplete_is_told_apart_from_invalid`, `crates/websocket-bridge/src/connection.rs::tests::ws_10_4_a_frame_over_the_limit_fails_with_1009_from_its_header`, `crates/websocket-bridge/src/connection.rs::tests::ws_10_4_a_message_of_many_small_frames_over_the_limit_fails_with_1009`, `crates/websocket-bridge/src/connection.rs::tests::deflate::rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_the_limits_are_read_and_default_to_16_mib`, `crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_a_limit_that_cannot_be_served_is_refused_with_its_line`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_message_of_many_frames_over_its_limit_with_1009`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_frame_limit_of_its_configuration`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_message_limit_of_its_configuration`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_does_not_start_with_a_limit_it_cannot_serve`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_4_the_mqtt_client_refuses_a_frame_over_its_limit_from_the_header`

**Status:** done — Frame und Nachricht sind in Bibliothek und Daemon begrenzt, der Verstoß schließt mit 1009, ein zu großer Frame schon an seinem Header. Die Grenzen des Daemons sind konfigurierbar; eine Frame-Grenze unter 125 Oktetten und eine Nachrichtengrenze von 0 verhindern den Start.

### §10.6 TLS unterstützen

**Spec:** §10.6, S. 53 (RFC) — "WebSocket implementations MUST support TLS and SHOULD employ it when communicating with their peers."

**Repo:** `crates/websocket-bridge/src/daemon/security.rs`, `crates/websocket-bridge/src/daemon/server.rs`

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`, `crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`

**Status:** done

### §10.7 Ungültige Daten

**Spec:** §10.7, S. 53 (RFC) — "Incoming data MUST always be validated by both clients and servers." Nach dem Handshake "the endpoint SHOULD send a Close frame with an appropriate status code (Section 7.4)"; während des Handshakes "the server SHOULD return an appropriate HTTP [RFC2616] status code."

**Repo:** `crates/websocket-bridge/src/connection.rs` (`Receiver`, `Failure::close_code`, `Sender::fail`), `crates/websocket-bridge/src/handshake.rs` (`HandshakeError::render_error_response`), `crates/websocket-bridge/src/codec.rs`, `crates/websocket-bridge/src/masking.rs`, `crates/websocket-bridge/src/utf8.rs`, `crates/websocket-bridge/src/close.rs`, `crates/websocket-bridge/src/daemon/server.rs`, `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`: die Empfangsschleife des WebSocket-Clients des MQTT-Daemons)

**Tests:** `crates/websocket-bridge/src/masking.rs::tests::ws_5_1_masking_violation_fails_the_connection_with_1002`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_autobahn_1_framing_and_2_pings`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_2_autobahn_3_reserved_bits_and_4_opcodes`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_5_4_autobahn_5_fragmentation`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_8_1_autobahn_6_utf8`, `crates/websocket-bridge/tests/autobahn_e2e.rs::ws_7_autobahn_7_close_handling`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_autobahn_server_1_framing_and_2_pings`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_2_autobahn_server_3_reserved_bits_and_4_opcodes`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_5_4_autobahn_server_5_fragmentation`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_8_1_autobahn_server_6_utf8`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_7_autobahn_server_7_close_handling`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_10_4_autobahn_server_9_message_sizes_1_to_4`, `crates/websocket-bridge/tests/autobahn_server_e2e.rs::ws_10_4_autobahn_server_9_message_sizes_5_to_8_and_10`, `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_7_the_mqtt_client_refuses_invalid_frames_of_the_server`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_7_the_mqtt_client_refuses_an_invalid_close_frame_of_the_server`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_5_5_1_the_mqtt_client_answers_a_close_frame_of_the_server_with_its_status`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::ws_10_4_the_mqtt_client_refuses_a_frame_over_its_limit_from_the_header`

**Status:** done — der Daemon beantwortet jede Art ungültiger Daten nach dem Handshake mit dem Close-Frame ihres Status-Codes (1002, 1007, 1009) und einen ungültigen Handshake mit dem HTTP-Status (400, 401, 403, 405, 426). Die Autobahn|TestSuite bewertet beide Rollen der Bibliothek: ihr Fuzzing-Server die Client-Rolle (Fälle 1 bis 7), ihr Fuzzing-Client die Server-Rolle (`autobahn_server_e2e.rs`: ein Echo-Server aus dem Handshake und dem `Receiver` und `Sender`, die der Daemon nutzt; Fälle 1 bis 7, 9 und 10, 301 Fälle; je Fall `OK` für das Gesendete und für das Schließen der Verbindung, bei 7.1.6, 7.13.1 und 7.13.2 auch `INFORMATIONAL`). Die Tests der Server-Rolle sind `#[ignore]` und laufen im Wochenjob `integration-bridges-vendor-autobahn`; in CI sind sie noch nicht gelaufen, auf dem Linux-Testhost von Hand grün. Die Fälle 12 und 13 (`permessage-deflate`) laufen nur gegen die Client-Rolle. Der WebSocket-Client in `crates/mqtt-bridge/src/daemon/ws_transport.rs` liest Frames mit eigenem Code über dem Codec dieser Krate; bei zwölf geprüften Arten ungültiger Frames, bei einem Frame über seiner Grenze und bei einem Close-Frame mit ungültigem Body (ein Oktett, ein nicht erlaubter Status-Code, eine Reason, die kein UTF-8 ist) sendet er den Close-Frame mit dem Status-Code des Verstoßes (1002, 1003, 1007, 1009) und schließt die Netzwerkverbindung; einen gültigen Close-Frame beantwortet er mit dessen Status-Code. Die Tests verlangen je Fall den Grund, den der Client meldet, genau einen Close-Frame mit dem Status-Code und das Ende der Verbindung durch den Client, bevor der Stub des Tests aufgibt.

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

28 done / 0 partial / 0 open / 0 blocked / 6 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document websocket-rfc-6455.md`):

* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (Job `merge-rust-websocket-bridge`) — 138 Tests grün.

138 zitierte Tests grün, 0 nicht grün.

