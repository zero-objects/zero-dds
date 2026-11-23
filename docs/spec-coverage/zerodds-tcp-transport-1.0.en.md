# ZeroDDS TCP Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. DDSI-RTPS 2.5 defines no
mapping to TCP: §9.3.2.1 names only UDPv4 (1) and UDPv6 (2) as locator
kinds, §9.5 maps RTPS messages to UDP/IP, and §9.3.2.4 leaves the locator
kinds 3 to `0x01ffffff` to the vendors. §8.3.3 states that a stream
transport like TCP would need the message length in the RTPS
HeaderExtension. The locator kind, the stream framing and the connection
bring-up of this transport are ZeroDDS's own. The normative requirements
of this document live in the `##` sections §1 through §6; every `###`
item below carries a spec citation, a repo anchor, a test and a status.

Implementation: `crates/transport-tcp/` — length-prefix framing
(`framing.rs`), 16-byte handshake (`handshake.rs`), stream framing by the
HeaderExtension (`rtps_stream.rs`), the stream of Cyclone DDS
(`cyclone_stream.rs`), the TCP protocol of Fast DDS (`rtcp.rs`), connection
pool (`tcp_transport.rs`).

Open items: `zerodds-tcp-transport-1.0.open.en.md`.

## §1 Scope and spec status

### §1.1 Locator kind TCPv4 / TCPv6

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — the spec names no locator kind for TCP
(§9.3.2.1: `LOCATOR_KIND_UDPv4 = 1`, `LOCATOR_KIND_UDPv6 = 2`) and
reserves 3 to `0x01ffffff` for vendors. A locator of the own dialect (§2,
§3) MUST carry `0x01F0_0004` (TCPv4) and `0x01F0_0005` (TCPv6). The values
4 and 8 MUST be selectable per transport and per runtime, not
process-wide; Cyclone DDS writes them (`ddsi_protocol.h`:
`DDSI_LOCATOR_KIND_TCPv4 = 4`, `DDSI_LOCATOR_KIND_TCPv6 = 8`), for Fast DDS
this is not checked against its source. A locator announced for the
dialect of another vendor (§4.1, §6.1, §6.2) MUST carry that vendor's kind
and port layout. Both pairs of values are read; a locator that was read
keeps the kind it was written with, and a destination of either pair is
reached over the same transport.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Tcpv4 = 0x01F0_0004`,
`LocatorKind::Tcpv6 = 0x01F0_0005`, `LocatorKind::Tcpv4Kind4 = 4`,
`LocatorKind::Tcpv6Kind8 = 8`, `TcpKindSet`, `LocatorKind::tcp_v4`,
`tcp_v6`, `is_tcp_v4`, `is_tcp_v6`, `tcp_kind_set`, `transport_kind`,
`Locator::tcp_v4`, `Locator::tcp_v6`, `Locator::with_tcp_kind_set`);
`crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_locator_kinds`, `locator_to_socket`,
`TcpTransport::local_locator`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::tcp_kind_set`, key `transport.tcp_locator_kinds`,
`select_user_transport`, `announce_locator`, `is_routable_user_locator`);
`crates/dcps/src/layered_transport.rs` (a destination is assigned to a
transport by `LocatorKind::transport_kind`).

**Tests:** `crates/rtps/src/wire_types.rs::tests::tcp_1_1_the_wire_kind_follows_the_kind_set`,
`crates/rtps/src/wire_types.rs::tests::tcp_1_1_two_kind_sets_in_one_process_do_not_interfere`
(two threads encode locators of one pair of values each at the same time),
`crates/rtps/src/wire_types.rs::tests::the_tcp_kinds_are_vendor_range_by_default_and_fast_dds_by_setting`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_a_transport_announces_the_kinds_of_its_set`
(two transports of one process, TCPv4 and TCPv6; the source of a received
frame carries the pair of values of the receiver),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_a_destination_of_either_kind_set_uses_one_connection`,
`crates/transport-tcp/src/tcp_transport.rs::tests::rtps_9_3_2_4_locator_kinds_are_zerodds_vendor_range_values`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_locator_to_socket_v4_maps_tcp`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_locator_to_socket_v4_rejects_udp`,
`crates/dcps/src/runtime.rs::tests::tcp_1_1_the_announced_tcp_kind_follows_the_runtime_setting`
(two runtimes of one process with TCP as the user-data transport; the SPDP
message of each runtime carries the locator with the kind of its setting
and an address other than `0.0.0.0`, and `SpdpReader` reads it back that
way),
`crates/dcps/src/runtime.rs::tests::tcp_1_1_both_kind_sets_are_one_transport_per_address_family`
(under either kind set: a listener on `::` is announced with `::1`, one
on `0.0.0.0` with an address of the host or of the chosen interface, each
with its kind and its port; a destination of either set goes over the
same TCP transport),
`crates/dcps/tests/runtime_config_layering.rs::rtps_9_3_2_4_the_tcp_locator_kinds_are_a_setting`.

**Status:** partial — built: the kind is a property of the locator, the process-wide switch `set_tcp_kinds_fast_dds` is removed; `TcpTransport::with_locator_kinds` and `transport.tcp_locator_kinds` select 4 and 8 per transport and per runtime; reading keeps the kind that was written; a destination with 4 or 8 is sent to over the TCP transport. With the dialect `cyclone` (§4.1) a runtime announces 4 and 8 and one listener port for metatraffic and user traffic, as Cyclone DDS does with `TCP/Port` (`crates/dcps/tests/runtime_config_layering.rs::tcp_4_1_the_cyclone_dialect_is_a_setting`; with a running Cyclone DDS: `crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`). The cited tests in `zerodds-rtps`, `zerodds-transport-tcp` and `zerodds-dcps` are green on the Linux test host. With the dialect `fastdds` (§6.1) a runtime announces 4 and 8 with the logical port in the upper 16 bits of the port, and a locator of kind 4 or 8 with a port above 65535 is read and reached as one of Fast DDS (`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination`, `crates/dcps/tests/fastdds_live_tcp.rs`). Remaining: the kinds and the port layout of the dialect of RTI Connext (§6.2) are missing; `transport.tcp_locator_kinds: fast-dds` alone announces 4 and 8 without a logical port, a locator Fast DDS does not address (that takes the dialect `fastdds`).

### §1.2 RTPS wire mapping over the stream

**Spec:** §2 — the frame body is a complete RTPS message (RTPS header +
submessages, DDSI-RTPS 2.5 §8.3.3), byte-identical to the message the UDP
mapping (§9.5) carries in one datagram. The TCP transport MUST pass it
through unchanged and must not rewrite, merge or split it.

**Repo:** `crates/transport-tcp/src/framing.rs` (`write_frame`,
`read_frame` — the payload is copied without interpretation);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` hands every
frame it read up as a `ReceivedDatagram`).

**Tests:** `crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(two frames over a real TCP connection, byte-equal and in order),
`crates/transport-tcp/tests/loopback.rs::tcp_1_2_concurrent_senders_to_one_server`
(100 frames from two threads, no loss),
`crates/transport-tcp/src/framing.rs::tests::tcp_1_2_roundtrip_small_frame`.

**Status:** done

### §1.3 What OMG does not standardize for TCP

**Spec:** DDSI-RTPS 2.5 fixes nothing for TCP: no locator kind
(§9.3.2.1), no mapping of the messages (§9.5 is for UDP/IP), no connection
bring-up, no logical-port multiplexing and no connection-pool policy. On
stream framing §8.3.3 says that a stream transport like TCP "would need to
include the length in the RTPS HeaderExtension"; §2.5 carries this
requirement as the dialect `rtps`. Cyclone DDS (`ddsi_tcp`) has no handshake and begins every
message with the RTPS header and a submessage of its own, `0x81`, whose
length field counts the whole message (`ddsi_receive.c`,
`read_packet_from_stream`; §4.1). Fast DDS and RTI Connext have formats of
their own: Fast DDS the protocol RTCP with a header, a bind and logical
ports of its own (§6.1, read in the source of Fast DDS 3.6.1), RTI Connext
one that is not checked against its source (§6.2). The section
records the scope and states no implementation requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — scope statement; the requirements are the items from §1.1 and §2 on.

## §2 Wire format

In the default dialect (`zerodds`) every RTPS datagram is framed over TCP
as follows; the dialect `rtps` (§2.5) delimits a message by the
HeaderExtension it carries instead:

```text
+---------+---------+---------+---------+
| length (u32, big-endian)              |   frame length in bytes (header excluded)
+---------+---------+---------+---------+
| RTPS frame body (length bytes)        |   as in the UDP PSM
+---------+---------+---------+---------+
```

### §2.1 Length-prefix frame (4 bytes big-endian)

**Spec:** §2 — the sender MUST prefix every frame with a 4-byte
big-endian length counting the body without the header itself. An empty
body is allowed (`length = 0`). Consecutive frames follow each other
without a separator.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`write_frame`, `read_frame`, `FRAME_HEADER_LEN = 4`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_1_2_roundtrip_small_frame`
(checks the four header bytes against `5u32.to_be_bytes()`),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_1_roundtrip_empty_frame`,
`crates/transport-tcp/src/framing.rs::tests::tcp_2_1_two_frames_back_to_back`.

**Status:** done

### §2.2 Reassembly across write boundaries

**Spec:** §2 — TCP is a stream: the receiver MUST reassemble a frame the
sender spread over several `write` calls, and must neither truncate it
nor hand it up as two frames. That holds when the length field itself
arrives split.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_frame` — a loop over `read` fills the four octets of the length;
`read_exact_or_eof` — `read_exact` fills the body; both no matter how
many `read` calls that costs).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_2_reassembles_message_split_across_two_writes`
(length prefix plus first half in the first write, remainder plus a
follow-up frame in the second; the reader returns exactly one message and
then the follow-up),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_2_reassembles_frame_split_across_reads`
(4096-byte payload at chunk sizes 1, 3, 4, 5 and 4095),
`crates/transport-tcp/tests/loopback.rs::tcp_2_2_frame_split_across_two_writes_is_reassembled`
(the same split over a real TCP socket, with an 80 ms pause between the
two writes; the server yields exactly one frame).

**Status:** done

### §2.3 Maximum frame size

**Spec:** §2 — the receiver MUST refuse an announced length above the
limit instead of allocating the buffer (DoS protection). Limit: 16 MiB.
The sender MUST refuse a payload above the limit with `PayloadTooLarge`
before it opens a connection; the result must not depend on the state of
the connection. Larger samples are split at the RTPS level by `DATA_FRAG`
(DDSI-RTPS §8.3.7), not at the transport level.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`MAX_FRAME_SIZE = 16 * 1024 * 1024`, `FramingError::FrameTooLarge`,
`read_frame` checks the length before the allocation, `write_frame`
before the first octet); send path in
`crates/transport-tcp/src/tcp_transport.rs` (`Transport::send` compares
the length before it touches the pool, `SendError::PayloadTooLarge`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_3_rejects_oversized_announcement`
(`MAX_FRAME_SIZE + 1` is refused before anything is read),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_3_write_frame_takes_the_limit_and_refuses_one_octet_more`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_send_oversized_payload_returns_payload_too_large`
(a destination without a listener: a connect attempt would have returned
`Io`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_send_oversized_payload_does_not_connect`
(a listener at the destination sees no connect, the pool stays empty),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_accept_one_frame_too_large_errors`.

**Status:** done

### §2.4 Abort on EOF mid-frame

**Spec:** §2 — if the peer closes the connection in the middle of the
header or the body, the receiver MUST report that as `UnexpectedEof` and
count it, and must not hand a truncated frame upward. An EOF exactly on a
frame boundary is the regular end of the connection and no error.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_frame`: `FramingError::Closed` for the end before the first octet
of a frame, `FramingError::UnexpectedEof` after 1 to 3 octets of the
length or with an incomplete body);
`crates/transport-tcp/src/tcp_transport.rs` (`read_connection`: `Closed`
ends the frame pump with `Ok(())`, `UnexpectedEof` with
`TcpTransportError::PeerIo` of kind `UnexpectedEof` and the counter
`truncated_frames`; `serve` counts every connection whose reading ended
in an error in `connection_errors`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_4_unexpected_eof_mid_header`
(1, 2 and 3 octets of the length),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_4_unexpected_eof_mid_payload`,
`crates/transport-tcp/src/framing.rs::tests::tcp_2_4_eof_on_a_frame_boundary_is_closed`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reads_one_frame_then_eof`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reports_eof_mid_body`
(10 octets announced, 3 sent: error, queue empty, counter 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reports_eof_mid_header`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_serve_counts_a_connection_that_ended_inside_a_frame`.

**Status:** done

### §2.5 Stream framing by the HeaderExtension

**Spec:** DDSI-RTPS 2.5 §8.3.3 — a stream transport like TCP would need
the length of the message in the RTPS HeaderExtension to identify the
boundary of the message. §9.4.5.2 defines the HeaderExtension
(`messageLength` as the first field when the L flag is set, the
submessage directly after the RTPS header, §8.3.7.3). A mode that
delimits the messages on the stream in this way MUST be selectable
(dialect `rtps`, `transport.tcp_dialect: rtps`); the framing of §2.1 with
the handshake of §3 stays the default of this document (dialect
`zerodds`). In the dialect `rtps` a connection carries RTPS messages back
to back, with no handshake and no length in front of a message. Every
message begins with 28 octets:

```text
 0  'R' 'T' 'P' 'S'                RTPS header (§9.4.4), 20 octets
 4  major minor                    protocol version, 2.5 or above
 6  vendorId (2)
 8  guidPrefix (12)
20  0x00                           submessageId RTPS_HE (§9.4.5.1.1)
21  flags                          E = 0x01, L = 0x02 set
22  octetsToNextHeader (u16)       in the byte order of E, 4 at least
24  messageLength (u32)            in the byte order of E
```

`messageLength` counts the whole message from the first octet of the RTPS
header (§8.3.7.5). The receiver MUST read the 28 octets, then
`messageLength − 28` more, and hand the message on unchanged, the
HeaderExtension included (§1.2). It MUST close the connection when a
message does not begin with `RTPS`, its protocol version is below 2.5, its
first submessage is no HeaderExtension with the L flag, `messageLength` is
smaller than the RTPS header and the HeaderExtension together or larger
than the limit of §2.3; a stream has no second way to the next message
boundary. §2.4 holds for messages as for frames. The transport adds
nothing to a message: a `send` MUST refuse a message that does not carry
its own length so, before it makes a connection. The runtime puts the
HeaderExtension with `messageLength` into every message it sends over
such a connection. Without a handshake there is no request: the vendor
list of §3.3 is held against the vendor id in the RTPS header of each
message, a connection beyond the peer limit (§3.7) is closed without an
answer and counted, and there is no claim (§3.6). Both ends of a
connection have the same dialect; the detection at the first octet is
§4.2.

A message protected as a whole (DDS-Security 1.2, `rtps_protection_kind`
other than NONE, or a pre-shared key) carries the HeaderExtension between
the RTPS header and the SRTPS_PREFIX (DDS-Security 1.2 §10.5.3.3.1 Table
80: "the original RTPS Header and, if present, the original
HeaderExtension, followed by the SecureRTPSPrefixSubMsg"). The receiver
MUST find the SRTPS_PREFIX behind a leading HeaderExtension. Two cases:

- The protection authenticates the RTPS header and the HeaderExtension
  (flag A of the SRTPS_PREFIX: a pre-shared key, Table 89, or the form
  with "Additional Authenticated Data", Table 80, 2). The HeaderExtension
  MUST be in the message before the protection: `messageLength` counts as
  zero for the tag and describes the protected message afterwards (Table
  80, 3). A HeaderExtension put in behind the protection is not the
  authenticated one, and the receiver refuses the message; the leg that
  puts the length in MUST refuse such a message without a HeaderExtension.
- The protection does not authenticate the header (no flag A, the default
  with the exchanged key). `encode_rtps_message` then takes no
  HeaderExtension (Table 80, 1); the leg puts it in behind the protection.
  No tag covers it: the receiver MUST discard it once the message is
  delimited and must not hand on any of its fields. DDS-Security 1.2 does
  not show this form: §10.5.3.3.4.6.4 has no HeaderExtension without AAD.

**Repo:** `crates/transport-tcp/src/rtps_stream.rs` (`STREAM_HEAD_LEN`,
`message_length`, `read_message`, `check_message`, `write_message`);
`crates/transport-tcp/src/framing.rs` (`FramingError::NotRtps`,
`NoMessageLength`, `LengthTooSmall`, `LengthMismatch`);
`crates/transport-tcp/src/tcp_transport.rs` (`TcpDialect`,
`TcpTransport::with_dialect`, `dialect`, `read_rtps_stream`,
`PeerConn::speaking`, the check in `Transport::send`,
`TcpTransportError::StreamFraming`, `VendorNotAccepted`);
`crates/rtps/src/header_extension.rs`
(`HeaderExtension::with_message_length`);
`crates/dcps/src/stream_length_leg.rs` (`StreamLengthLeg`,
`authenticated_without_extension`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_dialect`, key
`transport.tcp_dialect` with `zerodds` and `rtps`, environment variable
`ZERODDS_TCP_DIALECT`, `RuntimeConfig::tcp_leg`,
`RuntimeConfig::apply_tcp_settings`; protected messages:
`secure_outbound_bytes`, `stated_for_authenticated_stream`,
`protect_for_stream`, `secure_inbound_bytes`);
`crates/security-rtps/src/srtps.rs` (`srtps_prefix_offset`,
`is_secured_rtps_message`, `srtps_prefix_flags`, `srtps_psk_flag`,
`decode_secured_rtps_message`);
`crates/security-runtime/src/shared.rs` (`classify_inbound`,
`transform_inbound_from`, `transform_inbound`,
`SharedSecurityGate::is_message_protected`,
`authenticates_header_without_extension`),
`crates/security-runtime/src/gate.rs` (`decode_inbound_message`),
`crates/security-runtime/src/psk_gate.rs` (`is_psk_protected`);
`crates/security-crypto/src/plugin.rs` (`decode_rtps_message_of`: without
the flag A the HeaderExtension is not part of the decoded message).

**Tests:** `crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_message_delimited_by_header_extension_length`
(the 28 octets built by hand from §9.4.4 and §9.4.5.2; octets of a next
message stay in the stream),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_big_endian_message_length`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_header_extension_with_further_fields`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_refuses_a_stream_message_without_length`
(no HeaderExtension, no L flag, no room for the field, version 2.4, the
request of §3.1; the same on the sending side, with no octet written),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_refuses_a_message_length_below_28`
(0, 1, 20, 27; 28 is the message without a further submessage; 16 MiB + 1
is refused before anything is read),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_two_messages_back_to_back`
(at chunk sizes 1, 3, 27, 28, 29 and 4096),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_eof_inside_a_message_is_an_error`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_a_message_to_send_states_its_own_length`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_two_transports_exchange_messages_in_the_rtps_dialect`
(real sockets: a raw listener reads exactly the octets of the two
messages, beginning with `RTPS`, and nothing is written back; between two
transports the messages arrive unchanged),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_send_refuses_a_message_without_length`
(no connect, the pool stays empty),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_that_is_no_rtps_stream_is_closed`
(also: a listener of the dialect `zerodds` refuses an RTPS stream with
`BadMagic`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_the_vendor_list_applies_to_the_rtps_header`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_beyond_the_limit_is_closed_without_an_answer`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_puts_the_length_behind_the_header`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_keeps_the_fields_of_an_extension`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_refuses_what_is_no_rtps_2_5_message`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_a_message_sent_through_the_leg_states_its_length`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_refuses_what_cannot_carry_the_length`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_hands_everything_else_to_the_transport`,
`crates/dcps/src/runtime.rs::tests::tcp_2_5_the_tcp_leg_of_the_rtps_dialect_states_the_length_of_each_message`
(a raw listener reads two messages of 40 octets each, with the
HeaderExtension and without a handshake, from the TCP leg of a runtime),
`crates/dcps/tests/runtime_config_layering.rs::tcp_2_5_the_tcp_dialect_is_a_setting`,
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv4_rtps_dialect`
(two processes, RELIABLE and KEEP_ALL: all 200 samples arrive once and in
order, over a TCP connection between the processes).
Protected messages:
`crates/security-rtps/src/srtps.rs::tests::security_10_5_3_3_1_the_srtps_prefix_is_found_behind_a_header_extension`
(SRTPS_PREFIX behind a HeaderExtension in both byte orders, the flags
behind it; plain and truncated messages are not),
`crates/security-crypto/src/plugin.rs::tests::security_10_5_3_3_1_an_unauthenticated_header_extension_is_not_part_of_the_decoded_message`
(without the flag A: the decoded message is the protected one, without
the HeaderExtension; a timestamp put in on the way does not arrive),
`crates/security-runtime/src/shared.rs::tests::security_10_5_3_3_1_classify_inbound_finds_srtps_behind_a_header_extension`
(the gate accepts the message; plain text behind a HeaderExtension stays
refused on a protected domain),
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_and_a_protected_message`
(without the flag A the leg puts the HeaderExtension in front of the
SRTPS_PREFIX; with the flag A and without a HeaderExtension it refuses the
message; with a HeaderExtension it sets the length only),
`crates/dcps/src/runtime.rs::tests::tcp_2_5_a_message_under_a_pre_shared_key_states_its_length_before_the_protection`
(pre-shared key: `RTPS header | HeaderExtension | SRTPS_PREFIX` with A and
P, `messageLength` equal to the length of the protected message; the
receiver reads the message back with its HeaderExtension; a
HeaderExtension put in behind the protection is refused),
`crates/dcps/src/runtime.rs::tests::tcp_2_5_a_protection_that_authenticates_the_header_gets_the_header_extension`
(the decision by the flag A of the output, with stand-in protections),
`tests/integration/tests/security.rs::security_rtps_protection_over_the_tcp_rtps_dialect`
(two processes, `rtps_protection_kind` ENCRYPT, user traffic over TCP in
the dialect `rtps`: all 50 samples arrive once and in order; the kernel
shows the TCP connection; every RTPS message `strace` recorded on a TCP
socket of the writer carries the HeaderExtension with the length and is
`RTPS header | HeaderExtension | SRTPS_PREFIX`, except the traffic the
protection of the whole message does not apply to (the RTPS Bootstrapping
Messages of DDS-Security 1.2 §7.5.7 and §9.4.2.4 Table 33,
attribute is_rtps_axk_protected: SPDP, authentication, key exchange; by §4.3 it runs over TCP as
well); the text of the samples is in no send. Without the search
behind the HeaderExtension the reader's acknowledgment does not come and
the scenario fails after 15 s).

**Status:** done — not run: the form with "Additional Authenticated Data" and the exchanged key end to end (no key of the configuration turns it on at the gate; what is tested is the runtime's decision with stand-in protections and the plugin by itself); the pre-shared key across two processes (tested in one runtime and between two runtimes of one process, without a socket); no other vendor, with or without protection.

## §3 Handshake (ZeroDDS-specific)

```text
Client                                    Server
  |-- BindConnectionRequest  (16 B) ------->|   §3.1
  |<-- BindConnectionResponse (16 B) -------|   §3.2
  |-- length-prefix frame [RTPS] ---------->|   §2.1
```

### §3.1 BindConnectionRequest — wire layout

**Spec:** §3 — 16 bytes big-endian: magic `"ZDDS"` (4), `v_major` (1),
`v_minor` (1), `vendor_id` (2), `flags` (4, reserved), `logical_port`
(4). The ZeroDDS version is `(1,0)`, the vendor id `[0x01, 0xF0]` (`VendorId::ZERODDS` of `zerodds-rtps`). A
wrong magic MUST fail the connection.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionRequest`, `encode_request`, `decode_request`,
`HANDSHAKE_MAGIC_REQUEST`, `HANDSHAKE_WIRE_SIZE = 16`,
`VENDOR_ID_ZERODDS`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_wire_layout_is_sixteen_bytes_big_endian`
(compares all 16 bytes against the diagram's literal, including
`logical_port = 7400` as `00 00 1C E8`),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_roundtrip`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_rejects_wrong_magic`.

**Status:** done

### §3.2 BindConnectionResponse — wire layout and status byte

**Spec:** §3 — 16 bytes big-endian: magic prefix `"ZDA"` plus a status
byte (`0x2B` = `'+'` accept, `0x2D` = `'-'` reject), `v_major`,
`v_minor`, `vendor_id`, `flags`, `reason_code` (4, big-endian; `0` on
accept). Any other magic MUST fail as `BadResponse`.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionResponse`, `ResponseStatus`, `encode_response`,
`decode_response`, `HANDSHAKE_MAGIC_ACCEPT`, `HANDSHAKE_MAGIC_REJECT`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_wire_layout_is_sixteen_bytes_big_endian`
(all 16 bytes for accept and for reject with `reason_code = 3`),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_roundtrip_accept`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_roundtrip_reject`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_client_handshake_errors_on_bad_response_magic`.

**Status:** done

### §3.3 Reject reason codes

**Spec:** §3 — six stable wire codes: `0 Unknown`, `1 VersionMismatch`,
`2 ResourceLimit`, `3 LogicalPortConflict`, `4 VendorNotAccepted`,
`5 ReservedFlagsSet`. An unknown code MUST map to `Unknown` so a newer
peer's reject is still processable; a peer without code 5 reads it so. The
server MUST check a request in a fixed order and refuse it with the reason
of the first check that fails:

1. Version (`VersionMismatch`): the fields behind it have the meaning of
   this document only if the version matches.
2. Reserved flags (`ReservedFlagsSet`, §3.5).
3. Vendor id (`VendorNotAccepted`).
4. Peer limit (`ResourceLimit`, §3.7).
5. Logical-port claim (`LogicalPortConflict`, §3.6). It is last because a
   granted claim is held: a request refused for another reason has claimed
   nothing.

Which vendor ids a listener takes MUST be a setting
(`transport.tcp_accepted_vendors`, a list of four hex digits each, `01f0`
for ZeroDDS); without the setting it takes every one. The list of the
setting has at least one entry: the empty list is refused as a value of
the key. At the transport (`TcpTransport::with_accepted_vendors`) an empty
list takes no vendor.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`RejectReason`, `RejectReason::from_code`, `RejectReason::as_code`,
`ServerPolicy`, `read_request`, `decide`, `write_response`,
`server_handshake_with`; `server_handshake` is their sequence with the
default policy); `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_accepted_vendors`, `accepted_vendors`,
`read_connection` and `answer_resource_limit` hand the list to `decide`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_accepted_vendors`, key
`transport.tcp_accepted_vendors`, environment variable
`ZERODDS_TCP_ACCEPTED_VENDORS`, `parse_vendor_list`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_reject_reason_roundtrip_known_codes`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_reject_reason_unknown_code_maps_to_unknown`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_rejects_version_mismatch`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_rejects_vendor_not_accepted`
(the response is `ZDA-` with `reason_code = 4`; an empty list takes no
vendor),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_accepts_a_listed_vendor`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_decide_checks_in_the_stated_order`
(a request that fails all five checks; with each check repaired the next
one names the reason, and the claim is asked for only when all others
have passed),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_resource_limit`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_logical_port_conflict`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_vendor_not_accepted`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_3_accept_one_rejects_an_unlisted_vendor_over_a_real_connection`
(over a real connection: `client_handshake` reads `VendorNotAccepted`,
the server closes, a frame behind the refused request does not reach the
queue, a listed vendor is read),
`crates/dcps/src/runtime.rs::tests::tcp_3_3_the_tcp_transport_of_a_runtime_has_its_vendor_list_and_claim`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_3_3_the_accepted_vendors_are_a_setting`
(also: the empty list and the empty value are a configuration error).

**Status:** done

### §3.4 Handshake sequence and its precedence over the first frame

**Spec:** §3 — the client MUST send the request before the first
length-prefix frame, the server MUST answer with the response before the
first frame. A peer whose first octets are neither the request nor the
beginning of another accepted dialect (§4.2) MUST be refused; on a Reject
both sides drop the connection.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`client_handshake`,
`server_handshake`); `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected` calls `client_handshake` right after
`connect` and keeps no connection after an error or a Reject;
`read_connection` calls `server_handshake_with` before the frame pump and
closes the connection after a Reject without reading a frame).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_4_full_roundtrip_accept_via_paired_cursors`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`
(an HTTP probe is refused with `TcpTransportError::Handshake`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_the_server_closes_after_a_reject`
(a request with version 2.0 over a real connection: the peer reads `ZDA-`
with `reason_code = 1` and then EOF; a frame behind the rejected request
does not reach the queue),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_the_client_keeps_no_connection_after_a_reject`
(the listener answers `ZDA-`: the client closes without a frame, the pool
holds no connection, the backoff is 50 ms),
`crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(handshake and frames over a real connection).

**Status:** done

### §3.5 Reserved `flags` — receiver check

**Spec:** §3 — the `flags` field is reserved and MUST be `0`. A receiver
that accepts a field with bits set blocks the later use of those bits: an
old peer would silently ignore a new flag instead of refusing the
connection. The server MUST refuse a request with a bit set with
`ReservedFlagsSet` (code 5, §3.3). The client MUST discard a response with
a bit set, whether it carries Accept or Reject, and close the connection;
the next attempt waits for the backoff of §5.2. Both sides send
`flags = 0`.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`decide`: the check
after the version; `client_handshake`: `HandshakeError::ReservedFlags`;
`RejectReason::ReservedFlagsSet`; `write_response` and `client_handshake`
write `flags = 0`); `crates/transport-tcp/src/tcp_transport.rs`
(`read_connection` closes after the reject, `PeerConn::ensure_connected`
keeps no connection after `ReservedFlags`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_server_rejects_a_request_with_reserved_flags_set`
(`ZDA-` with `reason_code = 5`; the claim of the request is not asked
for),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_client_drops_a_response_with_reserved_flags_set`
(Accept and Reject),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_each_single_flag_bit_is_rejected`
(each of the 32 bits alone, in both directions),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_5_accept_one_rejects_flags_over_a_real_connection`
(a frame behind the request does not reach the queue),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_5_the_client_drops_a_connection_accepted_with_reserved_flags`
(after its response the listener reads the end of the connection and no
frame; the pool holds no connection, backoff 50 ms).

**Status:** done

### §3.6 Logical-port claim

**Spec:** §3 — `logical_port` is the port the client claims on this
connection; `0` means "no claim". The client claims the port of its own
listener, that is the port of the locator it announces. A claim is of the
triple of the listener address the connection arrived at, the address of
the peer and the port, among the live connections of one listener. The
server MUST refuse a request whose triple a live connection holds with
`LogicalPortConflict` and otherwise associate the connection with the
claimed port: its frames carry the address of the peer with the claimed
port as their source, that is the locator of the peer's listener. Without
a claim they carry the port the peer connected from. A claim above 65535
names no port of a TCP listener: the server MUST refuse it with
`LogicalPortConflict` without asking the claim table, because the frames
of the connection would otherwise carry a source no `send` can send to.
The claim ends with the connection. Accepted connections MUST have TCP
keepalive on (10 s
idle, 3 probes 5 s apart, where the platform lets interval and count be
set), so that a peer that vanished without a FIN does not hold its claim
without bound. After `LogicalPortConflict` the client waits for the
backoff of §5.2 and tries again, because the server may not have read the
end of the previous connection yet. Two hosts behind one translated
address whose listeners have the same port collide; for them the client
MUST be able to turn the claim off
(`transport.tcp_logical_port_claim: none`, the request then carries `0`).

**Repo:** `crates/transport-tcp/src/handshake.rs` (`ServerPolicy::claim`,
`decide`); `crates/transport-tcp/src/tcp_transport.rs` (`ClaimTable`,
`ClaimGuard` — releases the claim at every end of `read_connection` —,
`read_connection`: claim, source of the frames, `set_keepalive` with
`KEEPALIVE_IDLE`, `KEEPALIVE_INTERVAL`, `KEEPALIVE_PROBES`;
`TcpTransport::claimed_logical_ports`; connecting side:
`TcpTransport::with_logical_port_claim`, `claims_logical_port`,
`outgoing_claim`, `PeerConn::claiming`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::tcp_logical_port_claim`, key
`transport.tcp_logical_port_claim` with `listener` and `none`, environment
variable `ZERODDS_TCP_LOGICAL_PORT_CLAIM`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_a_second_connection_claiming_a_bound_port_is_rejected`
(real connections: the second reads `ZDA-` with `reason_code = 3` and the
end of the connection, its frame does not arrive, the first stays),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_claim_is_released_when_the_connection_ends`
(after the end of the holder the same port is granted again; a request
refused for its version holds nothing),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_zero_is_no_claim`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_6_a_claim_above_65535_is_refused`
(65536 and `u32::MAX`: the response is `ZDA-` with `reason_code = 3`, the
policy is not asked; 65535 is granted),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_a_claim_above_65535_is_rejected_over_a_real_connection`
(the peer reads `ZDA-` with `reason_code = 3`, no claim is held, no frame
arrives; with 65535 the frames carry that port as their source),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_frames_of_a_claimed_connection_carry_the_claimed_port_as_source`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_client_claims_its_listener_port`
(octets 12 to 15 of the request are the port of the own listener, `0`
with the claim turned off; between two transports the source of a frame
is the locator of the sender, and a frame sent to it arrives there),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_same_port_from_another_address_is_accepted`
(the claim table: another peer, another listener address and another
port are granted, the same triple is not),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_two_peer_addresses_hold_the_same_port_at_once`
(Linux only: connections from 127.0.0.1 and 127.0.0.2),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_an_accepted_connection_has_keepalive_on`
(reads the options of the socket; on Linux 10 s, 5 s, 3),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_client_retries_after_a_logical_port_conflict`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_3_6_the_logical_port_claim_is_a_setting`,
`crates/dcps/src/runtime.rs::tests::tcp_3_3_the_tcp_transport_of_a_runtime_has_its_vendor_list_and_claim`.

**Status:** done — not measured: that the kernel gives up a connection without a peer after the keepalive probes; the test reads the options that are set.

### §3.7 ResourceLimit reject at the peer cap

**Spec:** §3 — once the server has reached its peer limit it MUST refuse
the handshake with `ResourceLimit` instead of accepting the connection
and dropping it afterwards. Only then can the client tell the backoff
apart from a connection failure. The limit counts the connections the
listener is reading (256, a setting of the transport). The server reads
the request of a connection beyond the limit before it answers and
closes: closing with unread data resets the connection and can cost the
peer the response. If the request fails a check that stands before the
limit in §3.3, the reject carries that reason. The accepting loop must not
read an octet of a refused connection. Refusing is bounded itself: request
and response have 250 ms together, and at most 16 refused connections wait
for their answer at a time. One thread answers the waiting connections in
order, and each silent one ahead costs 250 ms. The reject MUST be written
while the client still waits for it (2 s, §3.8): a refused connection is
answered until 1.5 s after it was accepted, and its exchange ends at that
instant at the latest. One more beyond the 16, one whose request is not
complete in that time, and one whose turn comes later, is closed without
an answer and counted in addition; it is counted before it is closed. A
connection for which no stream clone or no reading thread can be made is
refused like one beyond the limit. The client MUST tell a reject from a
connection failure: it counts it by reason, the error of the `send` names
the reason, and after `ResourceLimit` the next attempt waits 1 s at least
instead of the 50 ms of §5.2.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::serve`, `refuse_beyond_limit`, `refuse_unstarted`,
`answer_resource_limit` in the thread `zdds-tcp-refuse`, `REFUSE_BUDGET`,
`REFUSE_QUEUE`, `REFUSE_DEADLINE`,
`TcpTransport::with_accept_limit`, `accept_limit`, `refused_connections`,
`refused_unanswered`; connecting side: `PeerConn::ensure_connected`,
`PeerConn::last_reject`, `RESOURCE_LIMIT_BACKOFF_MS`, `reject_message`,
`TcpTransport::connect_rejections`, `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_serve_answers_resource_limit_beyond_the_peer_limit`
(real connections, limit 2: the third reads `ZDA-` with `reason_code = 2`
and the end of the connection, `refused_connections` 1,
`refused_unanswered` 0, the two that are read stay; a request of version
2.0 beyond the limit reads `reason_code = 1`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_silent_refused_peer_does_not_hold_the_accept_loop`
(eight silent connections beyond the limit: a connection that finds a
place in the meantime is read before all eight are given up on; each
silent one reads the end of the connection without a response),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_beyond_the_refuse_queue_a_connection_is_closed_unanswered`
(the 17th waiting connection is closed and counted, the 16 before it stay
open),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_client_behind_silent_refused_connections_is_answered_in_time`
(three silent connections ahead of the client: its `send` names
`tcp peer rejected: resource limit` in under 2 s, backoff 1000 ms; the
three silent ones are counted as unanswered, its connection is not),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_refused_connection_whose_turn_comes_too_late_is_closed_unanswered`
(with a complete request: before the deadline `ZDA-` with
`reason_code = 2`, at the deadline no octet of a response and
`refused_unanswered` 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_connection_without_a_reading_thread_is_refused`
(`refuse_unstarted`, the way of `serve` after a failed thread start: the
place is free again, the peer reads `ZDA-` with `reason_code = 2`; without
a registered stream the connection is counted as refused and unanswered),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_the_client_counts_the_resource_limit_reject_and_backs_off_one_second`
(`SendError::Io` with `tcp peer rejected: resource limit`,
`connect_rejections` 1, backoff 1000 ms; the throttled `send` opens no
second connection; a destination without a listener yields
`tcp write failed` with 50 ms),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_connection_ended_frees_a_place`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_resource_limit`.

**Status:** done — no test makes `try_clone` or the start of the reading thread itself fail; what is tested is the refusal `serve` calls after it (`refuse_beyond_limit`, `refuse_unstarted`).

### §3.8 Time budget in the accept path

**Spec:** §3 — the accept path MUST keep a total budget for the
handshake, not only a timeout per syscall. A peer dawdling byte by byte
would otherwise bind `16 × slice` seconds of the accept thread. The
budget is 2 s for request and response together; if it ends before the
request is complete, no response is written. The same budget holds on the
connecting side.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs` (`DeadlineStream`:
before every read and write syscall it sets the socket timeout to the
minimum of the slice and the rest of the budget, and returns `TimedOut`
once the budget has ended; `read_connection` with
`SERVER_HANDSHAKE_BUDGET` = 2 s and `SERVER_HANDSHAKE_SLICE` = 200 ms;
`PeerConn::ensure_connected` with `CLIENT_HANDSHAKE_BUDGET` = 2 s; after
the accept the timeouts are lifted again).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_accept_one_gives_up_on_a_peer_sending_octet_by_octet`
(one octet per 150 ms, each within the slice, so the peer needs at least
2.25 s: `accept_one` returns after 1.9 s or later with
`Handshake(Io(TimedOut))`, the peer has read no octet of a response; a
budget longer than the peer needs, or a timeout per read only, would let
the request complete and fails the test),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_the_client_gives_up_on_a_listener_answering_octet_by_octet`
(the send fails after 1.9 s or later, before the listener has answered
completely, the pool holds no connection),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_accept_one_gives_up_on_a_stalling_handshake_peer`
(the peer sends one handshake byte and goes silent but keeps the
connection open; `accept_one` returns after one slice, in under 1 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`.

**Status:** done

## §4 Cyclone `ddsi_tcp` compatibility

### §4.1 Handshake-skip mode

**Spec:** §4 — Cyclone DDS (`ddsi_tcp`) has no handshake over TCP and no
length field in front of the message. A connection carries RTPS messages
back to back, and the first submessage of each message is `0x81`
(`DDSI_RTPS_SMID_ADLINK_MSG_LEN`), whose 32-bit field counts the whole
message, the header and this submessage included (`ddsi_xmsg.c`:
`xp->msg_len`; `ddsi_receive.c`: `read_packet_from_stream` refuses a
message whose first submessage is not `0x81`). This mode MUST be
selectable (dialect `cyclone`, `transport.tcp_dialect: cyclone`) for the
connections the transport opens; those it accepts it detects by §4.2.
Every message begins on the stream with 28 octets:

```text
 0  'R' 'T' 'P' 'S'                RTPS header, 20 octets
 4  major minor                    protocol version
 6  vendorId (2)
 8  guidPrefix (12)
20  0x81                           submessageId of the length submessage
21  flags                          E = 0x01
22  octetsToNextHeader (u16)       in the byte order of E; 4
24  length (u32)                   in the byte order of E
```

The length submessage is framing and no part of the RTPS message: with
RTPS protection Cyclone DDS puts it behind the header of the protected
message and takes it out before it decodes (`ddsi_security_omg.c`). The
sender MUST put the eight octets behind the RTPS header of the message it
is given, and the receiver MUST take them out: above the transport stands
the message a UDP datagram carries (§1.2). The receiver reads the 28
octets and then `length − 28` more. It MUST close the connection if a
message does not begin with `RTPS`, its first submessage is not `0x81`,
that submessage's `octetsToNextHeader` has no room for the field, `length`
is smaller than the header and the length submessage together or larger
than the limit of §2.3; §2.4 applies to messages as to frames. Cyclone DDS
drops a connection on a message above 65536 octets (`ddsi_receive.c`,
`do_packet`). The sender MUST refuse a message that would be longer with
the length submessage (above 65528 octets) with `PayloadTooLarge` before
it opens a connection, and likewise one without an RTPS header; a
transport of this dialect states 65528 as its largest message. The vendor
list and the peer limit apply as in §2.5; there is no claim.

Connections are used in both directions. Cyclone DDS without `TCP/Port`
has no listener, announces no unicast locator and expects the answers on
the connection it opened (`ddsi_init.c`: `publish_uc_locators`;
`ddsi_tcp.c`: accepted and connecting connections in one table). The
source of the messages of an accepted connection without a claim is the
address the peer connected from; nothing listens there. A `send` to that
address MUST write on the accepted connection, in its dialect, instead of
connecting, as long as it lives. What the peer writes on a connection the
transport opened MUST be read and handed on with the address of the peer
as its source. Both apply to every dialect; in the dialect `zerodds` the
source of a connection with a claim stays the peer's listener (§3.6), and
a `send` there connects. A message the dialect of the accepted connection
cannot carry is refused before an octet is written; a failed write closes
the connection.

The runtime puts the HeaderExtension with the messageLength of the dialect
`rtps` (§2.5) into a message per destination: into every message to a
connection of that dialect, also to an accepted one while the runtime's own
dialect is another, and into no message to a connection of another
dialect. A message whose protection authenticates the header (flag A:
pre-shared key, or the form with Additional Authenticated Data) is
protected once for all its destinations, and the leg cannot add the
HeaderExtension behind that protection (§2.5). The runtime puts it in
before the protection in its own dialect `rtps` and whenever its TCP
listener takes connections of the dialect `rtps`
(`transport.tcp_accept_dialects`, every dialect by default), into every
message it protects so, whichever transport carries it. A runtime of
another dialect thus answers an accepted connection of the dialect `rtps`
under such a protection as well. Such a message to a connection of the
dialect `cyclone` keeps its HeaderExtension.

**Repo:** `crates/transport-tcp/src/cyclone_stream.rs` (`STREAM_HEAD_LEN`,
`MSG_LEN_SUBMESSAGE_ID`, `MAX_STREAM_MESSAGE_SIZE`, `MAX_MESSAGE_SIZE`,
`stream_head`, `read_body`, `read_message`, `check_message`,
`write_message`); `crates/transport-tcp/src/tcp_transport.rs`
(`TcpDialect::Cyclone`, `TcpDialect::max_message_size`, `Transport::send`
with the check before a connection is opened,
`Transport::max_message_size`; both directions: `AnswerTable`,
`TcpTransport::answer_on`, `send_on_accepted`, `answered_on_accepted`,
`PeerConn::reading_into`, `start_reading`, `closed_by_peer`,
`Inbound::pump`, `Drop` for `PeerConn`; the dialect per destination:
`TcpTransport::dialect_towards`); `crates/dcps/src/stream_length_leg.rs`
(`StreamLengthLeg::for_tcp`, `states_length_towards`, `for_destination`);
`crates/dcps/src/runtime.rs` (the value `cyclone` of the key
`transport.tcp_dialect`, `parse_tcp_dialect`, `RuntimeConfig::tcp_kind_set`
— the dialect `cyclone` brings the locator kinds 4 and 8 —,
`RuntimeConfig::apply_tcp_settings`, `RuntimeConfig::tcp_leg`;
the HeaderExtension before the protection:
`RuntimeConfig::may_write_tcp_dialect_rtps`, `RuntimeConfig::uses_tcp`,
`stated_for_authenticated_stream`, `secure_outbound_bytes`).

**Tests:** `crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_msg_len_submessage_layout`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reads_what_cyclone_dds_wrote`
(the first message a Cyclone DDS 0.10.5 participant wrote on its TCP
connection, without and with `TCP/Port`, captured on the Linux test host:
328 and 384 octets; written again, the message read is the capture),
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_the_message_handed_up_has_no_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reads_a_big_endian_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_a_first_submessage_other_than_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_a_length_below_28_and_above_the_limit`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_takes_out_a_longer_length_submessage`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reassembles_across_reads`
(at chunk sizes 1, 3, 27, 28, 29 and 4096),
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_eof_inside_a_message_is_an_error`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_to_write_what_cyclone_dds_does_not_read`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_two_transports_exchange_messages_in_the_cyclone_dialect`
(real sockets: a raw listener reads header, `81 01 04 00`, length and rest
per message; between two transports the messages arrive without the
length submessage),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_listener_reads_what_cyclone_dds_wrote`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_send_to_the_source_of_an_accepted_connection_uses_that_connection`
(in the dialects `cyclone`, `rtps` and `zerodds` without a claim: the raw
peer reads the answer on its connection, the pool stays empty, and
`dialect_towards` names the dialect of the connection for its source and
the transport's own for another destination; after the end of the
connection `dialect_towards` names the transport's own dialect and the
`send` fails),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_claimed_connection_is_answered_at_the_claimed_listener`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_what_the_peer_writes_on_an_outgoing_connection_is_read`
(a raw listener answers on the accepted connection; between two
transports in each dialect one connection carries both ways),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_an_outgoing_connection_ends_with_its_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_send_refuses_what_cyclone_dds_does_not_read`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_1_the_cyclone_dialect_is_a_setting`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_runtime_answers_an_accepted_connection_in_its_dialect`
(a runtime of each of the three dialects and a TCP peer without a listener
that writes the SPDP announcement of the recording: a peer of the dialect
`rtps` reads the runtime's answers with a HeaderExtension leading each
message, also from a runtime of the dialect `zerodds` or `cyclone`; a peer
of the dialect `cyclone` reads them without one, also from a runtime of
the dialect `rtps`; no metatraffic send fails),
`crates/dcps/src/runtime.rs::tests::tcp_4_1_a_runtime_taking_the_rtps_dialect_states_the_length_before_a_pre_shared_key`
(a runtime of the dialect `zerodds` with a TCPv4 leg that takes every
dialect: a message under a pre-shared key is `RTPS header |
HeaderExtension | SRTPS_PREFIX` with A and P, messageLength the length of
the protected message, and the leg finds nothing to change; with a
listener that does not take the dialect `rtps` the message is protected as
it is; the condition for each combination of dialect, transports and
accepted dialects).
With a running Cyclone DDS 0.10.5 (`ddsperf -T OU`, `General/Transport` =
`tcp`, so without a UDP socket; the ZeroDDS participant with
`transport.tcp_dialect: cyclone`, without SPDP multicast and without a UDP
metatraffic locator):
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`
(Cyclone without `TCP/Port` connects to the listener of ZeroDDS and is
answered on its connection; five samples with rising sequence numbers
arrive at the reader),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_with_a_listener_reaches_a_zerodds_reader`
(Cyclone with `TCP/Port`; ZeroDDS connects through `discovery.peers`),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`
(`ddsperf sub` counts the samples of the ZeroDDS writer).

**Status:** done — the three tests with Cyclone DDS run in the weekly job `vendor-live-dcps` and are green by hand on the Linux test host (Cyclone DDS 0.10.5). Not run: TCPv6 with Cyclone DDS; a protected connection (DDS-Security) with Cyclone DDS over TCP, and so how Cyclone DDS takes the HeaderExtension of a protected message of a runtime that speaks or accepts the dialect `rtps`; a runtime of another dialect that answers an accepted connection of the dialect `rtps` under a pre-shared key across two processes (tested is the protected message the runtime makes); OpenSplice over TCP.

### §4.2 Auto-detect on the first frame byte

**Spec:** §4 — the server side MUST detect the dialect of an accepted
connection from its first octets; without detection a deployment would
have to configure the mode per port up front. One octet does not decide
it: the stream of Cyclone DDS (§4.1) and the dialect `rtps` (§2.5) both
begin with `RTPS`. The listener reads the first four octets under the
deadline of the handshake (§3.8):

1. `ZDDS` is the request of §3.1: the rest of the handshake runs under the
   same deadline, then frames of §2.1 follow.
2. `RTPS` is the first message of a stream. Its first submessage (octet
   20) tells the dialect: `0x81` is the length submessage of Cyclone DDS,
   `0x00` the HeaderExtension of the dialect `rtps`. Every later message
   of the connection MUST have the same dialect; another first submessage
   closes the connection.
3. `RTCP` is the header of the dialect `fastdds` (§6.1): the
   BindConnectionRequest runs under the same deadline, then the messages
   of the connection follow.
4. Anything else closes the connection.

§3.4 applies to connections that begin with `ZDDS`; a peer whose first
octets begin no accepted dialect MUST be refused. Which dialects a
listener takes MUST be a setting (`transport.tcp_accept_dialects`, a list
of `zerodds`, `rtps`, `cyclone`, `fastdds`); without the setting it takes all. A
connection in a dialect that is not taken is closed and counted. The
stream dialects have no request and no response: the vendor list (§3.3)
is held against the vendor id in the RTPS header of each message, and a
connection beyond the peer limit (§3.7) is closed without an answer and
counted; if the listener takes the dialect `zerodds`, it waits at most
250 ms for a request it can answer with a Reject. The dialect a
connection was detected in is the one the transport answers on it in
(§4.1).

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_accepted_dialects`, `accepted_dialects`,
`read_connection` — the four octets under `SERVER_HANDSHAKE_BUDGET` and
`SERVER_HANDSHAKE_SLICE` —, `read_zerodds_connection` with `Prefixed`,
`read_message_stream`, `TcpTransportError::DialectNotAccepted`,
`refuse_beyond_limit`, `TcpDialect::ALL`, `name`, `from_name`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_accept_dialects`, key
`transport.tcp_accept_dialects`, environment variable
`ZERODDS_TCP_ACCEPT_DIALECTS`, `parse_tcp_dialect_list`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_one_listener_reads_a_peer_of_each_dialect`
(three connections at once to a listener with the default settings, one of
each of the dialects `zerodds`, `rtps` and `cyclone`: all three messages
are in the queue),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_unknown_first_octets_are_refused`
(HTTP probe: `BadMagic` with `GET `; an RTPS message as UDP carries it:
`NoMessageLength`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_detection_is_under_the_handshake_budget`
(the peer sends `R` and falls silent: `accept_one` returns in under 1 s
with `TimedOut`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_restricted_listener_refuses_the_other_dialects`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_an_unlisted_vendor_in_the_rtps_header_is_refused`
(the capture of Cyclone DDS, vendor id `01 10`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_connection_keeps_the_dialect_of_its_first_message`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_stream_peer_beyond_the_limit_is_closed_unanswered`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_that_is_no_rtps_stream_is_closed`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_2_the_accepted_dialects_are_a_setting`
(also: the empty list and a name that is no dialect are configuration
errors),
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`
(the listener of ZeroDDS detects the connections of a running Cyclone
DDS).

**Status:** done

### §4.3 Metatraffic over the TCP leg

**Spec:** §4 — a participant that has TCP only (Cyclone DDS with
`General/Transport` = `tcp`; a participant behind a network that carries
no UDP) MUST be discoverable over TCP and able to discover over it.
DDSI-RTPS 2.5 fixes nothing for that (§1.3). SPDP, SEDP, the liveliness
protocol, TypeLookup and the builtin endpoints of DDS-Security then run
over the same TCP connections as the user traffic:

1. The port of the TCP listeners of a runtime MUST be a setting
   (`transport.tcp_port`; `0`, the default, lets the system choose), so
   that a peer can name it. A TCPv4 and a TCPv6 listener of one runtime
   hold the same port; a port that is taken fails the start.
2. `discovery.peers` MUST take the TCP listener of a peer: `tcp://ip:port`
   and `tcp://[ipv6]:port`, the port is mandatory. The SPDP announcement
   goes there over the TCP leg in every announcement period as long as no
   discovered participant carries this locator.
3. The runtime MUST announce its TCP listeners as metatraffic unicast
   locators (`PID_METATRAFFIC_UNICAST_LOCATOR`), beside the UDP metatraffic
   socket. `discovery.unicast_transports` (a list of `udp` and `tcp`,
   default both) chooses what is announced; with `tcp` alone a peer has
   only TCP to send to, and a runtime without a TCP transport does not
   start with it.
4. Metatraffic to a TCP locator MUST be sent over the TCP leg; a send that
   fails is counted.
5. A message that arrives over the TCP leg and addresses a builtin entity
   (an EntityId with the builtin bits, DDSI-RTPS 2.5 §9.3.1.2) or carries
   protected submessages MUST take the way of a message of the metatraffic
   socket, which serves the user entities too. This holds whatever
   `discovery.unicast_transports` says. A message with user traffic alone
   keeps its way.
6. A participant whose SPDP announcement came over a TCP connection and
   that announces no unicast locator MUST be held under the source of that
   connection, each list (default and metatraffic) for itself; the answers
   then go on its connection (§4.1). Cyclone DDS does the same
   (`ddsi_discovery_spdp.c`, `get_locators`). A source that is no TCP
   locator changes nothing.
7. In the dialect `cyclone` the announced TCP locators carry the kinds 4
   and 8 (§1.1); Cyclone DDS reads no others.

Between two participants that announce UDP and TCP, metatraffic goes both
ways; the reliable protocol drops the duplicates.

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_port`, key
`transport.tcp_port`, environment variable `ZERODDS_TCP_PORT`,
`select_user_transport`, `select_tcp_transport`, `select_tcp_pair` — both
TCP listeners on one port, also one of the system's choosing —;
`parse_tcp_peer`, `expand_initial_peer`, key
`discovery.peers`; `RuntimeConfig::metatraffic_over_udp`,
`metatraffic_over_tcp`, key `discovery.unicast_transports`, environment
variable `ZERODDS_DISCOVERY_UNICAST_TRANSPORTS`,
`parse_unicast_transports`; `DcpsRuntime::start` — the announced
metatraffic locators —, `DcpsRuntime::send_metatraffic`,
`send_spdp_unicast`, `metatraffic_tcp_send_errors`; `recv_user_data_loop`,
`may_address_builtin_entity`, `dispatch_metatraffic_datagram`;
`adopt_connection_source`); `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::bind_v6_only`).

**Tests:** `crates/dcps/src/runtime.rs::tests::tcp_4_3_two_runtimes_discover_each_other_over_tcp_alone`
(two runtimes without SPDP multicast and without a UDP metatraffic
locator, one knows the TCP listener of the other as its initial peer: each
holds the other by TCP locators only, and the SEDP writers have the reader
of the other as a proxy),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_cyclone_participant_without_a_listener_is_answered_on_its_connection`
(a TCP peer writes the captured SPDP announcement of Cyclone DDS 0.10.5 on
a connection to the listener: the runtime holds the participant under the
source of the connection and writes its own SPDP announcement in the
stream framing of Cyclone DDS on the same connection),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_the_spdp_announcement_goes_to_a_tcp_initial_peer`
(a raw listener reads the SPDP announcement; it carries the listener of
the runtime with kind 4 and the port of `transport.tcp_port` as default
and as metatraffic unicast locator),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_the_metatraffic_locators_follow_the_unicast_transports`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_message_for_a_builtin_entity_is_told_from_user_traffic`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_an_empty_locator_list_takes_the_source_of_the_connection`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_tcp_peer_entry_expands_to_a_tcp_locator`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`
(a TCPv4 and a TCPv6 listener on the same port that is set),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_both_tcp_listeners_hold_one_port_of_the_systems_choosing`
(with `transport.tcp_port` 0 and both orders in
`transport.user_transports`: the TCPv4 and the TCPv6 listener have the
same port, and each takes a connection of its address family on it),
`crates/dcps/src/runtime.rs::tests::tcp_6_1_both_tcp_listeners_of_the_fastdds_dialect_hold_one_port`
(the same in the dialect `fastdds`, whose locator port carries the
logical port above the port of the listener),
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_the_listener_port_is_a_setting`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_discovery_peers_take_tcp_locators`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_the_unicast_transports_are_a_setting`,
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv4_without_udp_discovery`
(two processes without a UDP path to each other, RELIABLE and KEEP_ALL:
the writer finds the reader through its TCP listener, all 200 samples
arrive once and in order; the kernel shows the listener on the port of the
setting and the TCP connection),
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv6_without_udp_discovery`
(the same over TCPv6, the peer entry `tcp://[::1]:port`),
`tests/integration/tests/security.rs::security_over_tcp_alone`
(two processes with DDS-Security, `rtps_protection_kind` ENCRYPT, over TCP
alone in the dialect `rtps`: the authentication, the key exchange, the
protected SEDP and all 50 samples go over the TCP connection; `strace`
shows every message of the writer on the stream as protected as a whole
or as an RTPS Bootstrapping Message (DDS-Security 1.2 §7.5.7 and §9.4.2.4
Table 33, attribute is_rtps_axk_protected), and no RTPS message on a UDP
socket but the wake-up messages to its own sockets),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_with_a_listener_reaches_a_zerodds_reader`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`
(discovery and samples with a Cyclone DDS that has no UDP socket).

**Status:** done — the tests with Cyclone DDS run in the weekly job `vendor-live-dcps` and are green by hand on the Linux test host (Cyclone DDS 0.10.5). Not run: metatraffic over TCP with a vendor other than Cyclone DDS; DDS-Security over TCP with another vendor.

## §5 Connection pool

### §5.1 Connection reuse and lazy connect

**Spec:** §5 — per destination socket address the transport holds
exactly one outgoing connection, opened on the first `send` and reused
afterwards. The pool lock must not be held across the blocking I/O. If
the peer has closed the connection and that has arrived before the
`send`, the `send` MUST connect anew in the same call instead of writing
into the closed connection; the close does not count as a connection
failure and triggers no backoff. A close that arrives only after the
check is seen by the following `send`. `Ok` from `send` says that the
frame was handed to the TCP stack, not that the peer received it: a frame
written into the connection before its close arrived at this host can be
lost without its `send` having reported an error, and it is not sent
again. That is allowed because RTPS requires no delivery of the transport
(DDSI-RTPS 2.5 §7.6: "the transport need not guarantee each packet will
reach its destination"); the repetition is the reliable RTPS protocol's,
above the transport. The loss must not stay invisible: the transport MUST
count every outgoing connection that a `send` finds closed by the peer and
replaces. How many frames of a counted connection are lost is not known
on the sending side; a connection with no `send` after its close is not
counted.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected`, `PeerConn::send` with `peer_closed` — a
non-blocking look at the receive side before the write, one syscall on
Unix (`recv` with `MSG_PEEK | MSG_DONTWAIT`) —,
`TcpTransport::peer_closed_connections`, `Transport::send`: step 1 takes
the pool lock only for lookup/insert, step 2 the peer lock for the I/O).

**Tests:** `crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(two sends over the same reused connection),
`crates/transport-tcp/tests/loopback.rs::tcp_5_1_reconnect_after_peer_restart`
(the peer ends and a second one binds the same port; a frame sent after
the restart arrives at the second one within 2 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_send_after_peer_close_reconnects_instead_of_writing_into_the_closed_connection`
(the frame arrives on the second connection, the pool holds one entry,
backoff 0, the counter is 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_a_frame_written_before_the_close_arrived_is_lost_and_the_connection_counted`
(the peer closes without reading the two frames written: their `send`
returned `Ok` and counted nothing, the following `send` counts the
connection, and the second connection begins with the frame of that
`send`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_the_look_for_a_close_takes_no_octet_and_keeps_the_socket_blocking`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_a_stalled_peer_does_not_hold_sends_to_another_peer`
(peer A takes the connection and never reads, a send to it blocks; a send
to peer B returns in under 1 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_send_unsupported_locator_does_not_pollute_pool`.

**Status:** done

### §5.2 Exponential backoff after a connection failure

**Spec:** §5 — after a failed connect the next attempt MUST be throttled
(50 ms, doubling, capped at 5 s) so a dead peer cannot trigger a connect
loop. A successful connect resets the backoff. After a reject with
`ResourceLimit` the wait is 1 s at least (§3.7).

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::bump_backoff`, `INITIAL_BACKOFF_MS = 50`,
`MAX_BACKOFF_MS = 5_000`, `RESOURCE_LIMIT_BACKOFF_MS = 1_000`, reset in
`ensure_connected`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_peer_conn_bump_backoff_sequence`
(50 → 100 → … → 5000, then constant),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_peer_conn_backoff_throttles_connect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_backoff_resets_after_successful_connect`
(the listener stays bound for the whole test: it closes the first
connection without an answer, the backoff is 50 ms; it answers the
second, after which the backoff is 0 and the frame has arrived),
`crates/transport-tcp/tests/loopback.rs::tcp_5_2_backoff_prevents_tight_loop_on_dead_peer`
(a second send to a dead port returns in under 20 ms),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_the_client_counts_the_resource_limit_reject_and_backs_off_one_second`
(1000 ms after `ResourceLimit`).

**Status:** done

### §5.3 Peer cap with deterministic eviction

**Spec:** §5 — the pool is capped at `MAX_PEERS = 256`. When it is full
and a new destination arrives, the lowest key is evicted
deterministically; a destination already in the pool triggers no
eviction.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_PEERS`, eviction branch in `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_3_max_peers_eviction_removes_first_key_on_new_peer`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_3_max_peers_no_eviction_for_existing_peer`.

**Status:** done

### §5.4 Bounded inbound queue with a counted drop

**Spec:** §5 — the receive queue is capped at `MAX_INBOUND_QUEUE = 1024`.
When it overflows, the oldest frame is dropped **and counted**; a silent
loss is not allowed. `recv` waits on a Condvar and wakes on a push,
without a poll loop. If no frame comes for one second, `recv` returns
`RecvError::Timeout`, so that a thread looping on `recv` sees its stop
signal; after `close`, `recv` returns `RecvError::Io` at once.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_INBOUND_QUEUE`, `push_inbound`, `dropped_frames`,
`Transport::recv` with a `Condvar::wait_timeout` of 1 s, `close`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_inbound_overflow_drops_oldest_and_counts`
(1029 numbered frames: the counter is 5, the queue yields the numbers 5 to
1028 in order),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_dropped_frames_zero_on_fresh_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_recv_wakes_on_push_from_other_thread`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_push_preserves_fifo_order`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_an_idle_recv_times_out`
(`Timeout` after 0.95 to 3 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_a_closed_transport_neither_sends_nor_waits`.

**Status:** done

## §6 Cross-vendor extension points

### §6.1 Fast DDS TCP (RTCP)

**Spec:** §6 — Fast DDS speaks a protocol of its own over TCP, which it
calls RTCP (Fast DDS 3.6.1, `src/cpp/rtps/transport/tcp/RTCPHeader.h`,
`RTCPMessageManager.cpp`, `TCPControlMessage.cpp`,
`TCPTransportInterface.cpp`, `TCPChannelResource.cpp`,
`src/cpp/utils/IPLocator.cpp`). It does not carry request and response as
RTPS submessages `0x71`/`0x72`, as this text used to say, but as control
messages with a header of their own. For interop with the TCP transport of
Fast DDS this protocol MUST be selectable as a dialect
(`transport.tcp_dialect: fastdds`); the handshake of §3 stays the default.

Every message, RTPS or control, has a header of 14 octets in front:

```text
 0  'R' 'T' 'C' 'P'
 4  length (u32)         header and the rest
 8  crc (u32)            sum of the octets behind the header, carry added
                         back in (one's complement); 0 = not computed
12  logical_port (u16)   0 = control message
```

Fast DDS writes and reads the three fields in the byte order of its host,
without a flag; ZeroDDS writes and reads them little-endian, the order of
x86-64 and AArch64. A control message has a header of 16 octets:

```text
 0  kind (u8)            D1..D6 requests, E1..E4 responses
 1  flags (u8)           0x02 little-endian, 0x04 payload, 0x08 answer wanted
 2  length (u16)         control header and the rest
 4  transaction_id (12)
16  [code (u32)]         responses only: 0 Ok, 1 BadRequest,
                         2 ExistingConnection, 3 InvalidPort,
                         4 UnknownLocator, 5 IncompatibleVersion,
                         6 ServerError
    [payload]            encapsulation (u16), length (u32), CDR data with
                         its own encapsulation header (00 01 00 00)
```

The kinds: `0xD1`/`0xE1` BindConnection (payload of the request: protocol
version 1.0, vendor id, locator of the sender's listener; of the response:
the other side's locator), `0xD2`/`0xE2` OpenLogicalPort (one logical
port), `0xD3`/`0xE3` CheckLogicalPorts (a list of logical ports),
`0xD4`/`0xE4` KeepAlive (a locator), `0xD5` LogicalPortIsClosed (one
logical port), `0xD6` UnbindConnection (no payload). A locator is `kind`
(i32), `port` (u32) and 16 octets of address; the kinds are 4 (TCPv4) and
8 (TCPv6). `port` holds the physical port (the TCP port of the listener)
in the lower and the logical port in the upper 16 bits; in a TCPv4 locator
the octets 8 to 11 of the address are the WAN address, 12 to 15 the
address in the host's network.

Sequence: the connecting side sends the BindConnectionRequest as its first
message and waits for the response under the deadline of §3.8; the
accepting side MUST expect that request as the first message, refuse
another protocol version with `IncompatibleVersion` and answer with the
locator of its listener. Before the first RTPS message to a logical port
the sending side asks with an OpenLogicalPortRequest; the other side
answers `Ok` when the port is open there, `InvalidPort` otherwise. Both
sides send and ask on the same connection. An RTPS message for a port that
is not open MUST be counted and answered with a LogicalPortIsClosedRequest
(Fast DDS drops it with a log line). Control messages are answered as Fast
DDS answers them: CheckLogicalPorts with the open ports of the list,
KeepAlive with `Ok` when the logical port of the locator is 0 and
`UnknownLocator` otherwise, an unknown kind and a wrong length with the
same kind and `BadRequest`; LogicalPortIsClosed takes the port from the
opened ones and the next send asks again; UnbindConnection ends the
connection regularly, and a transport that closes sends it on each of its
connections. A checksum that is neither 0 nor the sum, a message without
`RTCP` and a length below 14 close the connection. Fast DDS does not read
an RTPS message above its `maxMessageSize` (65500); a send above it MUST be
refused with `PayloadTooLarge`.

A locator of kind 4 or 8 with a port above 65535 is one of Fast DDS and
MUST be reached in this dialect, also by a transport of another dialect;
in the dialect `fastdds` a destination without a logical port is an
error. An address of the own host counts as loopback, as Fast DDS takes
all addresses of its host for one and refuses a second connection between
two participants with `ServerError`. The addresses come from the locators
peers announce; the transport keeps its answer for at most as many
addresses as its pool has peers (256) and asks again for one it forgot. When the WAN address is set and the
address in the network takes no connection, the WAN address is tried. The
own locators in the dialect `fastdds` have the kinds 4 and 8 and the
metatraffic unicast port of the participant (DDSI-RTPS 2.5 §9.6.1.4.1) as
their logical port; open are that port and the user unicast port. An
entry `tcp://ip:port` of `discovery.peers` is asked, in the dialect
`fastdds`, at the metatraffic unicast ports of the first participant ids,
as Fast DDS asks an initial peer without a logical port
(`configureInitialPeerLocator`); `tcp://ip:port/logical-port` names the
port. The vendor list (§3.3) applies to the vendor id of the
BindConnectionRequest; a vendor that is not listed is refused with
`ServerError`.

**Repo:** `crates/transport-tcp/src/rtcp.rs` (`TcpHeader`, `crc`,
`ControlMessage`, `ResponseCode`, `kind`, `WireLocator`,
`ConnectionRequest`, `encode_connection_request`, `encode_locator`,
`encode_port`, `encode_ports` and their counterparts, `read_message`,
`data_message`, `MAX_MESSAGE_SIZE`, `Link::connect`, `Link::accept`,
`Link::run`, `Link::send`, `Link::unbind`, `RtcpError`);
`crates/transport-tcp/src/tcp_transport.rs` (`TcpDialect::FastDds`,
`destination`, `TcpTransport::host_alias`, `with_logical_port`,
`set_logical_ports`, `logical_ports`, `listener_port`, `with_vendor_id`, `local_locator`,
`read_rtcp_connection`, `answer_on_link`, `send_on_accepted`,
`PeerConn::start_reading_link`, `rtcp_source`, `rtcp_closed_port_messages`,
`rtcp_refused_ports`, `rtcp_unmatched_responses`, `rtcp_bad_requests`,
`close`); `crates/dcps/src/runtime.rs` (`transport.tcp_dialect: fastdds`,
`RuntimeConfig::tcp_kind_set`, `fastdds_initial_peers`, `parse_tcp_peer`
with `/logical-port`, the logical ports of the TCP listeners at start);
`tests/interop/ci/fastdds/fastdds_shapes.cpp` (option `tcp`: the TCPv4
transport of Fast DDS alone, listening port, initial peers).

**Tests:** `crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_rtcp_header_layout`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_bind_connection_request_layout`
(both against the BindConnectionRequest Fast DDS 3.6.1 wrote on the Linux
test host, `tests/data/fastdds_3_6_1_rtcp_bind_request.hex`: the encoder
writes the same octets),
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_listener_answers_of_fast_dds_decode`
(the BindConnectionResponse and two OpenLogicalPortResponses of a Fast DDS
3.6.1 listener, `Ok` and `InvalidPort`, octet for octet),
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_crc_adds_the_carry_back_in`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bad_crc_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_stream_without_rtcp_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_control_message_of_a_wrong_length_is_answered_bad_request`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_locator_port_splits_into_physical_and_logical`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_cdr_payloads_round_trip`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_open_logical_port_exchange`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_messages_reach_the_reader_with_their_port`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_closed_logical_port_is_reopened`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_keep_alive_and_check_logical_ports_are_answered`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bind_of_another_version_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bind_without_an_answer_times_out`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_locator_of_the_dialect_holds_the_logical_port`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_an_address_of_this_host_is_reached_at_loopback`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_answers_on_host_addresses_are_bounded`
(762 addresses of the documentation ranges leave 256 answers, the last
ones asked; an address of this host whose answer was forgotten is asked
again and reached at loopback),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_two_transports_exchange_messages_in_the_fastdds_dialect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_dialect_is_chosen_by_the_locator`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_listener_answers_what_fast_dds_wrote`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_listener_refuses_a_dialect_or_vendor_it_does_not_take`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_whose_address_refuses_is_reached_at_its_wan_address`
(the address in the network refuses, the WAN address accepts),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_an_entry_of_another_dialect_in_use_is_not_taken_for_a_fast_dds_destination`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_logical_port_on_a_connection_of_another_dialect_is_refused`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_the_fastdds_dialect_reaches_the_transport_with_the_logical_ports`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_a_tcp_peer_of_the_fastdds_dialect_is_asked_at_the_metatraffic_ports`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_both_tcp_listeners_of_the_fastdds_dialect_hold_one_port`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_6_1_the_fastdds_dialect_is_a_setting`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_fastdds_tcp_publisher_reaches_a_zerodds_subscriber`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_a_fastdds_client_without_a_listening_port_is_answered_on_its_connection`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_zerodds_publisher_reaches_a_fastdds_tcp_subscriber`
(against a running Fast DDS 3.6.1, `fastdds_shapes` with the option `tcp`,
no UDP; job `vendor-live-dcps`).

**Status:** done — the cited tests in `zerodds-transport-tcp` and `zerodds-dcps` are green on the Linux test host; `fastdds_live_tcp` 3 of 3 against Fast DDS 3.6.1, built from the source of the tag of the CI image. Not checked: a Fast DDS host of big-endian byte order, TCPv6 with Fast DDS, the WAN address across NAT (the fallback to the WAN address is checked on one host: the address in the network refuses, the WAN address is a loopback address), the TLS layer of Fast DDS.

### §6.2 RTI TLS-oriented handshake

**Spec:** §6 — RTI DDS TCP uses a TLS-oriented bring-up. For interop with
RTI this dialect MUST be selectable as an additional mode; the
ZeroDDS-own handshake from §3 stays the default.

**Repo:** —

**Tests:** —

**Status:** open — no mode with the protocol of RTI Connext. Missing: RTI's TCP protocol (control messages of its own, control and data connections apart, the class ids `tcpv4_lan`/`tcpv4_wan`/`tlsv4_lan`/`tlsv4_wan` in the locator kind), the TLS layer beneath it and a test against RTI Connext. The text above is not checked against RTI; RTI publishes no specification of the protocol. The Linux test host of this unit has no RTI Connext installed to check an implementation against.

---

## Audit-Status

22 done / 1 partial / 1 open / 0 blocked / 1 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-tcp-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (job `merge-rust-rtps`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-transport-tcp --all-targets` (job `merge-rust-transport-tcp`) — 45 tests green.

46 cited tests green, 0 not green.

Open items: `zerodds-tcp-transport-1.0.open.en.md`.
