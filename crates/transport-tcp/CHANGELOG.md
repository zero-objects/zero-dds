# Changelog — `zerodds-transport-tcp`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The TCP locator kinds are `0x01F0_0004` / `0x01F0_0005` by default (see `zerodds-rtps`); `max_message_size` has a test.
- BREAKING: `framing::FramingError` is `#[non_exhaustive]` and has the variant `Closed`: `read_frame` returns it when the stream ends before the first octet of a frame, and `UnexpectedEof` only when it ends inside the length or the body (§2.4). It returned `UnexpectedEof` for both.
- `TcpTransport::accept_one` returns `PeerIo` of kind `UnexpectedEof` for a connection closed inside a frame (it returned `Ok`); the close on a frame boundary stays `Ok`.
- `Transport::send` refuses a payload above `MAX_FRAME_SIZE` with `PayloadTooLarge` before it connects (§2.3); the result depended on the connection.
- The handshake runs under one deadline of 2 s on each side (§3.8). On the accepting side the deadline was compared only after the request had been read, so a peer sending octet by octet was read to the end; on the connecting side the 2 s bounded each read.
- A send to a peer that has closed its connection connects anew in the same call (§5.1); the frame was written into the closed connection and lost with `Ok`.
- A destination with the locator kind 4 or 8 is taken as TCPv4 or TCPv6 (§1.1).
- The look for the peer's close before each write is one syscall on Unix (`recv` with `MSG_PEEK | MSG_DONTWAIT` through `socket2`), where it switched the mode of the socket twice around a `peek`; `socket2` and `libc` are dependencies on Unix.

- The accepting side decides a request against a policy in a fixed order: version, reserved flags, vendor id, peer limit, logical-port claim (§3.3). `handshake::server_handshake` is `read_request`, `decide` and `write_response` with the default `ServerPolicy`; `server_handshake_with` takes a policy.
- BREAKING: a request with a bit of the reserved `flags` set is answered with Reject, reason `RejectReason::ReservedFlagsSet` (code 5, new); `client_handshake` fails on a response with a bit set with `HandshakeError::ReservedFlags` (§3.5). Both were taken.
- BREAKING: `TcpTransport::serve` answers a connection beyond its limit with Reject/`ResourceLimit` from a thread of its own (`zdds-tcp-refuse`) instead of closing it unread (§3.7). On the connecting side a Reject is counted by reason (`connect_rejections`), the `SendError::Io` of the send names it (`tcp peer rejected: resource limit`), and after `ResourceLimit` the next attempt waits 1 s at least.
- BREAKING: an outgoing connection claims the port of the transport's listener as its logical port, and the source of a frame received over a claimed connection is the peer's address with the claimed port, the locator of the peer's listener; it was the ephemeral port. A second connection from the same peer address to the same listener address that claims a held port is answered with Reject/`LogicalPortConflict` (§3.6). `with_logical_port_claim(false)` claims no port.
- Accepted connections have TCP keepalive on (10 s idle, 3 probes 5 s apart where the platform has the options); `socket2` is a dependency on every platform.

### Added

- `TcpDialect::FastDds` and the module `rtcp`: the TCP protocol of Fast DDS, RTCP (§6.1). A 14-octet header (`RTCP`, length, checksum, logical port) in front of each message; the connecting side binds the connection with a BindConnectionRequest under the 2 s budget, either side asks the other with an OpenLogicalPortRequest before its first message to a logical port, and the control messages of Fast DDS (CheckLogicalPorts, KeepAlive, LogicalPortIsClosed, UnbindConnection, unknown kinds) are answered as Fast DDS answers them. A locator of kind 4 or 8 with a port above 65535 is a Fast DDS destination: physical port below, logical port above, reached in this dialect from a transport of any dialect, at the loopback address when the address is one of this host, at the WAN address of a TCPv4 locator when the address takes no connection. A listener detects `RTCP` (§4.2); `with_logical_port`, `set_logical_ports`, `logical_ports`, `listener_port`, `with_vendor_id`; `local_locator` holds the logical port in this dialect; `rtcp_closed_port_messages`, `rtcp_refused_ports`, `rtcp_unmatched_responses`, `rtcp_bad_requests`; `TcpTransportError::Rtcp`; `close` sends an UnbindConnectionRequest on each connection of the dialect. A message above 65500 octets is refused with `PayloadTooLarge`. Tested against octets Fast DDS 3.6.1 wrote and against a running Fast DDS 3.6.1.

- `TcpDialect::Cyclone` and the module `cyclone_stream` (`stream_head`, `read_body`, `read_message`, `check_message`, `write_message`): the stream of Cyclone DDS (`ddsi_tcp`), RTPS messages with no handshake, each with the length submessage `0x81` behind its RTPS header (§4.1). The transport puts the submessage into a message it writes and takes it out of one it reads; a message above 65528 octets is refused with `PayloadTooLarge`, and `Transport::max_message_size` is 65528 in this dialect. Tested against octets Cyclone DDS 0.10.5 wrote.
- A listener finds the dialect of an accepted connection at its first four octets (§4.2): `ZDDS` is the request of §3, `RTPS` a stream whose first submessage tells `cyclone` from `rtps`. `TcpTransport::with_accepted_dialects` and `accepted_dialects` restrict it; `TcpTransportError::DialectNotAccepted`; `TcpDialect::ALL`, `name`, `from_name`, `max_message_size`. BREAKING: `with_dialect` sets the dialect of the connections a transport opens only; a listener took its own dialect alone before and takes all by default now.
- Connections are used in both directions (§4.1): a thread (`zdds-tcp-conn`) reads each outgoing connection into the inbound queue, with the peer's address as the source, and a `send` to the address a live accepted connection came from (one without a logical-port claim) is written on that connection in its dialect instead of connecting to an address nothing listens at. `TcpTransport::answered_on_accepted` counts those sends; `connection_errors` counts outgoing connections as well. A `PeerConn` that is dropped shuts its connection down.
- `rtps_stream::read_body`.
- `TcpDialect` and `TcpTransport::with_dialect`, `dialect`: in `TcpDialect::Rtps` a connection has no handshake and carries RTPS messages back to back, each delimited by the messageLength of its HeaderExtension (DDSI-RTPS 2.5 §8.3.3; §2.5). Module `rtps_stream` (`message_length`, `read_message`, `check_message`, `write_message`); `FramingError::NotRtps`, `NoMessageLength`, `LengthTooSmall`, `LengthMismatch`; `TcpTransportError::StreamFraming`, `VendorNotAccepted`. A `send` in this dialect refuses a message that does not state its own length.
- `handshake::ServerPolicy`, `read_request`, `decide`, `write_response`, `server_handshake_with`; `RejectReason::ReservedFlagsSet`; `HandshakeError::ReservedFlags`.
- `TcpTransport::with_accepted_vendors` and `accepted_vendors`: the vendor ids a listener takes connections from; another vendor is answered with Reject/`VendorNotAccepted` (§3.3).
- `TcpTransport::with_accept_limit` and `accept_limit` (default `MAX_PEERS`), `refused_unanswered`, `connect_rejections` (§3.7).
- `TcpTransport::with_logical_port_claim`, `claims_logical_port`, `claimed_logical_ports` (§3.6).
- `Transport::max_message_size` is `framing::MAX_FRAME_SIZE` (DDSI-RTPS 2.5 §8.4.14.1).
- `TcpTransport::truncated_frames` and `TcpTransport::connection_errors` (§2.4).
- `TcpTransport::peer_closed_connections`: the outgoing connections a `send` found closed by the peer and replaced; a frame written into such a connection before its close arrived can be lost with `Ok` (§5.1).
- `TcpTransport::with_locator_kinds` and `locator_kinds`: the `LocatorKind` values of the transport's own locator and of the source of received frames, per transport (`zerodds_rtps::wire_types::TcpKindSet`, §1.1).

## [1.0.0-rc.1] — 2026-05-05

### RC1 audit
- License header (SPDX-Apache-2.0) on all 4 src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate header rewrite: honest spec story (DDSI-RTPS §9.4+§9.5
  OMG-normative, handshake ZeroDDS-own) instead of the non-existent
  "DDS-TCP-PSM §5.2.1" reference.
- `handshake.rs` header rewrite: phase-2b TODO markers removed;
  cross-vendor interop status clearly documented.
- ZeroDDS TCP Transport 1.0 spec materialized in
  `docs/spec-coverage/zerodds-tcp-transport-1.0.md` (§1-§8 with
  wire format, reject codes, Cyclone compat, test mapping).
- README + CHANGELOG.

### Features
- `std`-only, `forbid(unsafe_code)`.
- Safety class **STANDARD**.
- 55 tests green (50 lib + 5 integration); clippy clean.
- Length-prefix frame (DDSI-RTPS §9.5 conformant) + ZeroDDS handshake
  + Cyclone compat mode + connection pool.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
