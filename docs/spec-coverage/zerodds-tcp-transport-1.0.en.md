# ZeroDDS TCP Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. For TCP, OMG standardizes only
the locator kind (DDSI-RTPS 2.5 §9.4.5) and the wire mapping (§9.5); the
stream framing and the connection bring-up are vendor-specific. The
normative requirements of this document live in the `##` sections §2
through §6; every `###` item below carries a spec citation, a repo
anchor, a test and a status.

Implementation: `crates/transport-tcp/` — length-prefix framing
(`framing.rs`), 16-byte handshake (`handshake.rs`), connection pool
(`tcp_transport.rs`).

Open items and decision records: [`zerodds-tcp-transport-1.0.open.en.md`](zerodds-tcp-transport-1.0.open.en.md).

## §1 Scope and spec status

### §1.1 Locator kind TCPv4 / TCPv6

**Spec:** DDSI-RTPS 2.5 §9.4.5 — `LOCATOR_KIND_TCPv4 = 4`,
`LOCATOR_KIND_TCPv6 = 8`. The transport MUST carry exactly these values.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Tcpv4 = 0x01F0_0004`,
`LocatorKind::Tcpv6 = 0x01F0_0005`, `Locator::tcp_v4`, `Locator::tcp_v6`); bound in
`crates/transport-tcp/src/tcp_transport.rs` (`locator_to_socket`,
`TcpTransport::local_locator`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::locator_kinds_are_zerodds_vendor_range_values`
(asserts the two numeric values 4 and 8 directly),
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_to_socket_v4_maps_tcp`,
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_to_socket_v4_rejects_udp`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** partial — the transport carries the vendor-range values `0x01F0_0004` and `0x01F0_0005` (DDSI-RTPS 2.5 §9.3.2.4 reserves 3 to `0x01ffffff` for vendors); `set_tcp_kinds_fast_dds` writes the values 4 and 8 of the spec line (Fast DDS, DDS-TCP-PSM), and both are read. Remainder: 4 and 8 are not the default.

### §1.2 RTPS wire mapping over the stream

**Spec:** DDSI-RTPS 2.5 §9.5 — the frame body is a complete RTPS message
(RTPS header + submessages), byte-identical to the UDP PSM. The TCP
transport MUST pass it through unchanged and must not rewrite, coalesce
or split it.

**Repo:** `crates/transport-tcp/src/framing.rs` (`write_frame`,
`read_frame` — the payload is copied without interpretation);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` hands every
frame it read up as a `ReceivedDatagram`).

**Tests:** `crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(two frames over a real TCP connection, byte-equal and in order),
`crates/transport-tcp/tests/loopback.rs::concurrent_senders_to_one_server`
(100 frames from two threads, no loss),
`crates/transport-tcp/src/framing.rs::tests::roundtrip_small_frame`.

**Status:** done

### §1.3 What OMG does not standardize for TCP

**Spec:** DDSI-RTPS 2.5 knows no TCP connection bring-up, no logical-port
multiplexing and no connection-pool policy. Cyclone DDS (`ddsi_tcp`)
sends raw RTPS frames without a handshake, FastDDS uses
BindConnectionRequest/Response as submessages `0x71`/`0x72`, RTI a
TLS-oriented handshake. The section records the scope and states no
implementation requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 Wire format

Every RTPS datagram is framed over TCP as follows:

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

**Tests:** `crates/transport-tcp/src/framing.rs::tests::roundtrip_small_frame`
(checks the four header bytes against `5u32.to_be_bytes()`),
`crates/transport-tcp/src/framing.rs::tests::roundtrip_empty_frame`,
`crates/transport-tcp/src/framing.rs::tests::two_frames_back_to_back`.

**Status:** done

### §2.2 Reassembly across write boundaries

**Spec:** §2 — TCP is a stream: the receiver MUST reassemble a frame the
sender spread over several `write` calls, and must neither truncate it
nor hand it up as two frames. That holds when the length field itself
arrives split.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_exact_or_eof` — `read_exact` fills header and body completely, no
matter how many `read` calls that costs).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::reassembles_message_split_across_two_writes`
(length prefix plus first half in the first write, remainder plus a
follow-up frame in the second; the reader returns exactly one message and
then the follow-up),
`crates/transport-tcp/src/framing.rs::tests::reassembles_frame_split_across_reads`
(4096-byte payload at chunk sizes 1, 3, 4, 5 and 4095),
`crates/transport-tcp/tests/loopback.rs::frame_split_across_two_writes_is_reassembled`
(the same split over a real TCP socket, with an 80 ms pause between the
two writes; the server yields exactly one frame).

**Status:** done

### §2.3 Maximum frame size

**Spec:** §2 — the receiver MUST reject an announced length above the
limit instead of allocating the buffer (DoS protection). Limit: 16 MiB.
Larger samples are split at the RTPS level via `DATA_FRAG`
(DDSI-RTPS §8.3.7), not at the transport level.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`MAX_FRAME_SIZE = 16 * 1024 * 1024`, `FramingError::FrameTooLarge`);
send path in `crates/transport-tcp/src/tcp_transport.rs`
(`SendError::PayloadTooLarge`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::rejects_oversized_announcement`
(`MAX_FRAME_SIZE + 1` is rejected before any read),
`crates/transport-tcp/src/tcp_transport.rs::tests::send_oversized_payload_returns_payload_too_large`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_frame_too_large_errors`.

**Status:** done

### §2.4 Abort on EOF mid-frame

**Spec:** §2 — if the peer closes the connection in the middle of a
header or body, the receiver MUST report `UnexpectedEof` and must not
hand up a truncated frame. An EOF exactly on a frame boundary is the
regular connection teardown.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`FramingError::UnexpectedEof`, `read_exact_or_eof`);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` ends the frame
pump on `UnexpectedEof` with `Ok(())`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::unexpected_eof_mid_header`,
`crates/transport-tcp/src/framing.rs::tests::unexpected_eof_mid_payload`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_reads_one_frame_then_eof`.

**Status:** done

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
(4). The ZeroDDS version is `(1,0)`, the vendor id `[0x01, 0x0F]`. A
wrong magic MUST fail the connection.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionRequest`, `encode_request`, `decode_request`,
`HANDSHAKE_MAGIC_REQUEST`, `HANDSHAKE_WIRE_SIZE = 16`,
`VENDOR_ID_ZERODDS`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::request_wire_layout_is_sixteen_bytes_big_endian`
(compares all 16 bytes against the diagram's literal, including
`logical_port = 7400` as `00 00 1C E8`),
`crates/transport-tcp/src/handshake.rs::tests::request_roundtrip`,
`crates/transport-tcp/src/handshake.rs::tests::request_rejects_wrong_magic`.

**Status:** done

### §3.2 BindConnectionResponse — wire layout and status byte

**Spec:** §3 — 16 bytes big-endian: magic prefix `"ZDA"` plus a status
byte (`0x2B` = `'+'` accept, `0x2D` = `'-'` reject), `v_major`,
`v_minor`, `vendor_id`, `flags`, `reason_code` (4, big-endian; `0` on
accept). Any other magic MUST fail as `BadResponse`.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionResponse`, `ResponseStatus`, `encode_response`,
`decode_response`, `HANDSHAKE_MAGIC_ACCEPT`, `HANDSHAKE_MAGIC_REJECT`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::response_wire_layout_is_sixteen_bytes_big_endian`
(all 16 bytes for accept and for reject with `reason_code = 3`),
`crates/transport-tcp/src/handshake.rs::tests::response_roundtrip_accept`,
`crates/transport-tcp/src/handshake.rs::tests::response_roundtrip_reject`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_errors_on_bad_response_magic`.

**Status:** done

### §3.3 Reject reason codes

**Spec:** §3 — five stable wire codes: `0 Unknown`, `1 VersionMismatch`,
`2 ResourceLimit`, `3 LogicalPortConflict`, `4 VendorNotAccepted`. An
unknown code MUST map to `Unknown` so a newer peer's reject is still
processable.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`RejectReason`, `RejectReason::from_code`, `RejectReason::as_code`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::reject_reason_roundtrip_known_codes`,
`crates/transport-tcp/src/handshake.rs::tests::reject_reason_unknown_code_maps_to_unknown`,
`crates/transport-tcp/src/handshake.rs::tests::server_rejects_version_mismatch`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_resource_limit`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_logical_port_conflict`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_vendor_not_accepted`.

**Status:** partial — the codec and the client-side handling exist and
are tested for all five codes. `server_handshake` however emits only
`VersionMismatch`; `ResourceLimit`, `LogicalPortConflict` and
`VendorNotAccepted` never reach the wire from a ZeroDDS server. The three
missing sender paths are carried separately as §3.5, §3.6 and §3.7.

### §3.4 Handshake sequence and its precedence over the first frame

**Spec:** §3 — the client MUST send the request before the first
length-prefix frame, the server MUST answer with the response before the
first frame. A peer without a handshake MUST be refused; on reject both
sides drop the connection.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`client_handshake`,
`server_handshake`); `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected` calls `client_handshake` right after
`connect`; `accept_one` calls `server_handshake` before the frame pump).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::full_roundtrip_accept_via_paired_cursors`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_rejects_peer_without_handshake`
(an HTTP probe is refused with `TcpTransportError::Handshake`),
`crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(handshake and frames over a real connection).

**Status:** done

### §3.5 Reserved `flags` — receiver check

**Spec:** §3 — the `flags` field is reserved and MUST be `0`. A receiver
that accepts a field with bits set blocks the later use of those bits: an
old peer would silently ignore a new flag instead of refusing the
connection.

**Repo:** —

**Tests:** —

**Status:** open

### §3.6 Logical-port claim

**Spec:** §3 — `logical_port` is the DDS endpoint port the client claims
on this connection; `0` means "no claim". The server MUST refuse an
already-bound port with `LogicalPortConflict` and otherwise associate the
connection with the claimed port.

**Repo:** —

**Tests:** —

**Status:** open

### §3.7 ResourceLimit reject at the peer cap

**Spec:** §3 — once the server has reached its peer limit it MUST refuse
the handshake with `ResourceLimit` instead of accepting the connection
and dropping it afterwards. Only then can the client tell the backoff
apart from a connection failure.

**Repo:** —

**Tests:** —

**Status:** open

### §3.8 Time budget in the accept path

**Spec:** §3 — the accept path MUST carry a total budget for the
handshake, not only a per-syscall timeout. A peer dribbling byte by byte
would otherwise pin the accept thread for `16 × slice` seconds.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs` (`accept_one`:
`handshake_total_budget` = 2 s, `handshake_syscall_slice` = 200 ms,
deadline check after `server_handshake`; the timeouts are lifted again
after accept).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_gives_up_on_a_stalling_handshake_peer`
(the peer sends one handshake byte and goes quiet while holding the
connection open; `accept_one` returns `Handshake(_)` in under 3 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_rejects_peer_without_handshake`.

**Status:** done

## §4 Cyclone `ddsi_tcp` compatibility

### §4.1 Handshake-skip mode

**Spec:** §4 — Cyclone DDS speaks raw RTPS frames with a length prefix
over TCP, without a handshake. For interop a client variant MUST exist
that skips §3 and sends §2.1 frames directly, plus a server side that
accepts such connections.

**Repo:** —

**Tests:** —

**Status:** open

### §4.2 Auto-detect on the first frame byte

**Spec:** §4 — the server side MUST recognize the Cyclone mode from the
first byte: `'Z'` is the ZeroDDS handshake, `'R'` the first byte of the
RTPS magic. Without detection a deployment would have to configure the
mode per port up front.

**Repo:** —

**Tests:** —

**Status:** open

## §5 Connection pool

### §5.1 Connection reuse and lazy connect

**Spec:** §5 — the transport holds exactly one outbound connection per
destination socket address, opened on the first `send` and reused
afterwards. The pool lock must not be held across the blocking I/O.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected`, `TcpTransport::send` — step 1 takes the
pool lock only for lookup/insert, step 2 the peer lock for the I/O).

**Tests:** `crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(two sends over the same reused connection),
`crates/transport-tcp/tests/loopback.rs::reconnect_after_peer_restart`,
`crates/transport-tcp/src/tcp_transport.rs::tests::send_unsupported_locator_does_not_pollute_pool`.

**Status:** done

### §5.2 Exponential backoff after a connection failure

**Spec:** §5 — after a failed connect the next attempt MUST be throttled
(50 ms, doubling, capped at 5 s) so a dead peer cannot trigger a connect
loop. A successful connect resets the backoff.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::bump_backoff`, `INITIAL_BACKOFF_MS = 50`,
`MAX_BACKOFF_MS = 5_000`, reset in `ensure_connected`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::peer_conn_bump_backoff_sequence`
(50 → 100 → … → 5000, then constant),
`crates/transport-tcp/src/tcp_transport.rs::tests::peer_conn_backoff_throttles_connect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::backoff_resets_after_successful_connect`,
`crates/transport-tcp/tests/loopback.rs::backoff_prevents_tight_loop_on_dead_peer`
(a second send to a dead port returns in under 20 ms).

**Status:** done

### §5.3 Peer cap with deterministic eviction

**Spec:** §5 — the pool is capped at `MAX_PEERS = 256`. When it is full
and a new destination arrives, the lowest key is evicted
deterministically; a destination already in the pool triggers no
eviction.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_PEERS`, eviction branch in `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::max_peers_eviction_removes_first_key_on_new_peer`,
`crates/transport-tcp/src/tcp_transport.rs::tests::max_peers_no_eviction_for_existing_peer`.

**Status:** done

### §5.4 Bounded inbound queue with a counted drop

**Spec:** §5 — the receive queue is capped at `MAX_INBOUND_QUEUE = 1024`.
On overflow the oldest frame is discarded **and counted**; a silent loss
is not acceptable. `recv` blocks on a condvar and wakes on push, without
a poll loop.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_INBOUND_QUEUE`, `push_inbound`, `dropped_frames`,
`Transport::recv` with `Condvar::wait`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::inbound_overflow_drops_oldest_and_counts`,
`crates/transport-tcp/src/tcp_transport.rs::tests::dropped_frames_zero_on_fresh_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::recv_wakes_on_push_from_other_thread`,
`crates/transport-tcp/src/tcp_transport.rs::tests::push_preserves_fifo_order`.

**Status:** done

## §6 Cross-vendor extension points

### §6.1 FastDDS BindConnection (submessages 0x71/0x72)

**Spec:** §6 — FastDDS carries request and response as RTPS submessages
`0x71` and `0x72`. For interop with FastDDS TCPv4 this format MUST be
selectable as an additional handshake dialect; the ZeroDDS-own handshake
from §3 stays the default.

**Repo:** —

**Tests:** —

**Status:** open

### §6.2 RTI TLS-oriented handshake

**Spec:** §6 — RTI DDS TCP uses a TLS-oriented bring-up. For interop with
RTI this dialect MUST be selectable as an additional mode; the
ZeroDDS-own handshake from §3 stays the default.

**Repo:** —

**Tests:** —

**Status:** open

---

## Audit-Status

14 done / 1 partial / 7 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-transport-tcp` — 58 tests (lib) + 6
tests (`tests/loopback.rs`) green, 0 failed. Measured on macOS 15
(Apple Clang toolchain, Rust host `aarch64-apple-darwin`).

Open items and decision records: [`zerodds-tcp-transport-1.0.open.en.md`](zerodds-tcp-transport-1.0.open.en.md).
