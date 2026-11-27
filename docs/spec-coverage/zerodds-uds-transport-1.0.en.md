# ZeroDDS UDS Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. For Unix domain sockets OMG
standardizes nothing beyond the vendor-reserved locator kind
(DDSI-RTPS 2.5 §9.3.2.4); path resolution, wire format and cleanup are
vendor-specific. Cyclone DDS and FastDDS have no official UDS transport.
The normative requirements live in the `##` sections §2 through §6; every
`###` item below carries a spec citation, a repo anchor, a test and a
status.

Implementation: `crates/transport-uds/src/lib.rs` — `SOCK_DGRAM` over
filesystem sockets; `crates/transport-uds/src/abstract_dgram.rs` — the
Linux abstract namespace, `#[cfg(target_os = "linux")]`;
`crates/transport-uds/src/stream.rs` — the stream mode over `SOCK_STREAM`
(§3.5), the UDS transport on Windows; `crates/transport-uds/src/windows.rs`
— the base directory with an access control list and the refusal of the
modes on Windows (§6.2).

Open items: `zerodds-uds-transport-1.0.open.en.md`.

## §1 Scope and spec status

### §1.1 Vendor-reserved locator kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` to `0x01ffffff` are reserved
for vendor-specific locator kinds. ZeroDDS takes `0x01F0_0002` for UDS. The port
field is meaningless and MUST be `0`; the 16 address bytes carry the
endpoint id.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Uds`,
`Locator::uds`); bound in `crates/transport-uds/src/lib.rs`
(`UdsTransport::local_locator`, the kind check in `Transport::send`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_uds_value`
(asserts the numeric value `0x01F0_0002`, the position in the range `0x00000003` to `0x01ffffff` and
`port == 0`),
`crates/transport-uds/src/lib.rs::tests::uds_1_1_local_locator_reflects_bind_id`,
`crates/transport-uds/src/lib.rs::tests::uds_1_1_send_rejects_non_uds_locator`,
`crates/rtps/src/wire_types.rs::tests::locator_uds_layout`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** done

### §1.2 What OMG does not standardize for UDS

**Spec:** DDSI-RTPS 2.5 knows no path scheme, no UDS wire format and no
cleanup protocol. Cyclone DDS and FastDDS have no official UDS transport;
they cover the same use case through iceoryx or POSIX SHM. The section
records the scope and states no implementation requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — scope statement; the requirements are the items from §2 on.

## §2 Path resolution

### §2.1 Filesystem mode (default)

**Spec:** §2 — the 16-byte locator address resolves to
`<base_dir>/<lowercase-hex32>.sock`. The default `base_dir` is
`/tmp/zerodds/uds`. The hex string is the address in 32 lowercase
characters, without separators.

**Repo:** `crates/transport-uds/src/lib.rs` (`socket_path`,
`DEFAULT_BASE_DIR`, `parse_hex_id`, `source_locator` as the reverse
direction).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_2_1_socket_path_is_hex_under_base_dir`
(checks prefix and `.sock` suffix on an address with edge bytes set),
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_roundtrip`,
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_rejects_wrong_length`,
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_rejects_non_hex`.

**Status:** done

### §2.2 Base directory: lazy, user-private, TOCTOU-safe

**Spec:** §2 — the base directory is created on bind when it is missing
and then carries the mode `base_dir_mode` (default `0700`, §5.1). An
existing path that is a symlink or not a directory MUST be refused
rather than chmodded — otherwise a planted symlink could set foreign
paths to that mode. No mode is set on an existing directory. When the
kernel does not set the mode of a created directory as asked (it drops
the set-group-ID bit without an error when the directory's group is none
of the process's groups, and macOS drops it for every process in a
sandbox: `sandbox-exec`, operation `file-write-setugid`), the bind MUST
fail and remove the directory again; otherwise the next bind would take it as existing. This holds for
both transports, the filesystem mode of `UdsAbstractDgramTransport`
included.

**Repo:** `crates/transport-uds/src/lib.rs` (`ensure_base_dir` —
`symlink_metadata` instead of `exists`, refusal of symlink and
non-directory, `create_dir_all` plus `set_permissions(base_dir_mode)`
with a re-check, then the mode read back: when it differs,
`PermissionDenied` with the asked and the set mode, and `remove_dir`;
an existing directory left unchanged);
`crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport::bind` calls the same `ensure_base_dir` in
the filesystem mode).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_creates_dir_with_mode_0700`
(reads the mode back and compares it against `0o700`),
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_rejects_symlink`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_rejects_file_path`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_creates_missing_dir`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_leaves_the_mode_of_an_existing_dir`
(a directory with `0755` keeps `0755`, also after a bind with
`base_dir_mode` `0o3770`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_2_filesystem_mode_creates_base_dir_with_mode_0700`
(`0700` by default, `0o3770` with `base_dir_mode`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_2_filesystem_mode_rejects_a_symlink_base_dir`
(a symlink and a file are refused with `InvalidInput`, the link's
target keeps its mode),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind`
(UID 61001 without supplementary groups creates the directory with
`base_dir_mode` `3770` in a parent of the group 61099 with the
set-group-ID bit; the kernel sets `1770`, the bind fails with
`PermissionDenied` "uds base_dir_mode 3770: the kernel set 1770", and
the directory is gone afterwards).

**Status:** done

### §2.3 Abstract-namespace mode (Linux)

**Spec:** §2 — on Linux there is the alternative without a filesystem
inode: the socket name starts with a null byte, followed by
`<prefix>-<lowercase-hex32>`. No cleanup needed, no inode permissions, no
cross-mount volume. The name fits into `sun_path` (108 bytes, byte 0 is
the null prefix); a longer name is refused on bind with `InvalidInput`.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport`, `UdsAddress::Abstract`, `abstract_name`,
`build_abstract_sockaddr` through `socket2::SockAddr::unix` with a
leading null byte, `MAX_ABSTRACT_NAME = 107`,
`UdsAddress::abstract_default`); the module is declared in
`crates/transport-uds/src/lib.rs` under `#[cfg(target_os = "linux")]`.

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_bind_does_not_create_file`
(`/proc/net/unix` lists the name as `@<name>` and no filesystem socket
of that name, no file lies under the working directory or under `/tmp`,
after the drop the entry is gone),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_name_is_nul_prefix_dash_lowercase_hex32`
(the name read back with `getsockname` is byte for byte
`<prefix>-ab000000000000cd00000000000000ef`, of length
`offsetof(sun_path) + 1 + name` without a terminator),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_name_above_107_bytes_is_refused`
(107 bytes bind, 108 bytes give `InvalidInput`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_preserves_message_boundaries`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_local_locator_reflects_bind_id`.
The module sits behind `#[cfg(target_os = "linux")]` and is not compiled
on macOS; measured on the Linux test host.

**Status:** done

### §2.4 Filesystem mode inside the abstract transport (A/B comparison)

**Spec:** §2 — the same transport MUST also do filesystem addressing so
abstract and filesystem mode can be measured against each other with
identical socket code. The difference is one config line. The filesystem
mode addresses as §2.1 (`<base_dir>/<lowercase-hex32>.sock`, default
`base_dir` `/tmp/zerodds/uds`), so a `UdsTransport` and this mode reach
each other under one `base_dir`.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAddress::Filesystem`, `UdsAddress::filesystem_default` with
`DEFAULT_BASE_DIR`, `build_sockaddr` and `Drop` through
`crate::socket_path`, `decode_source` through `crate::source_locator`).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_bind_creates_socket_file`
(the socket file appears on bind and is gone after drop),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_mode_binds_the_path_of_2_1`
(the bound path is literally
`<tmp>/c000000000000000000000000000000c.sock`, also by `getsockname`;
`filesystem_default` is `/tmp/zerodds/uds` with `0700`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_mode_exchanges_datagrams_with_uds_transport`
(a `UdsTransport` and the filesystem mode under one `base_dir`, in both
directions, each with the other's locator as the source).
Linux-gated as §2.3.

**Status:** done

## §3 Wire format

### §3.1 `SOCK_DGRAM` with preserved message boundaries

**Spec:** §3 — one datagram message per RTPS frame. The kernel preserves
the message boundaries, unlike a TCP stream; no framing layer on top is
needed. Two sends MUST arrive as two receives with the original lengths.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport` over
`std::os::unix::net::UnixDatagram`, `Transport::send` with `send_to`,
`Transport::recv` with `recv_from`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_preserves_message_boundaries_two_datagrams`
(two sends of different lengths, two receives with exactly those
lengths),
`crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`
(the same delivery across a real process boundary).

**Status:** done

### §3.2 Sender locator from the socket address

**Spec:** §3 — the receiver MUST recover the locator from the sender
address so RTPS can answer. Only a name the transport gives itself
resolves: in the filesystem mode `<base_dir>/<lowercase-hex32>.sock`
under its own `base_dir`, in the abstract mode
`<prefix>-<lowercase-hex32>` with its own prefix — a reply goes to
exactly that name. If the address is unnamed (an anonymous client that
did not bind) or unresolvable, `Locator::INVALID` MUST be returned — not
a guess.

**Repo:** `crates/transport-uds/src/lib.rs` (`source_locator` — parent
directory equal to `base_dir`, extension `.sock`, `parse_hex_id` with
lowercase digits only; `Locator::INVALID`);
`crates/transport-uds/src/abstract_dgram.rs` (`decode_source`,
`decode_abstract_name` through `SockAddr::as_abstract_namespace`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`
(checks `got.source` against the sender's locator),
`crates/transport-uds/src/lib.rs::tests::uds_3_2_unbound_sender_yields_locator_invalid`
(a socket without a bind sends),
`crates/transport-uds/src/lib.rs::tests::uds_3_2_sender_outside_base_dir_yields_locator_invalid`,
`crates/transport-uds/src/lib.rs::tests::uds_3_2_a_name_socket_path_does_not_give_yields_locator_invalid`
(no hex, uppercase hex, extension `.seqp`, a subdirectory — all
`INVALID`; the lowercase name as the control resolves),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_abstract_unbound_sender_yields_locator_invalid`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_abstract_foreign_prefix_yields_locator_invalid`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_filesystem_mode_sender_outside_base_dir_yields_locator_invalid`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`.

**Status:** done

### §3.3 Datagram upper bound

**Spec:** §3 — `max_datagram` (default 65 536) bounds, in both
transports, what is sent and what is received. A larger payload MUST be
refused on send with `PayloadTooLarge`; a larger datagram from a peer
with a higher bound MUST be reported on receive as
`RecvError::BufferTooSmall`. A silent truncation is not acceptable,
because at the RTPS level it would be indistinguishable from a short
message. The DCPS runtime counts such an error of its user transport and
reports it as an event instead of discarding it.

**Repo:** `crates/transport-uds/src/lib.rs` (`DEFAULT_MAX_DATAGRAM`, the
length check in `Transport::send`, `recv_datagram` — a buffer of
`max_datagram + 1`, on Linux `MSG_TRUNC` for the real length, a datagram
above `max_datagram` gives `BufferTooSmall` with both sizes);
`crates/transport-uds/src/abstract_dgram.rs`
(`AbstractDgramConfig::max_datagram`, default `DEFAULT_MAX_DATAGRAM`,
`recv` through `recv_datagram`); `crates/dcps/src/runtime.rs`
(`recv_user_data_loop` counts `BufferTooSmall` in
`USER_DATAGRAMS_ABOVE_LIMIT`, read with
`DcpsRuntime::user_datagrams_above_limit`, and reports the event
`transport.user_recv.datagram_above_limit` with size and limit).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_3_send_rejects_oversize_payload`
(a payload above the configured limit is refused with
`PayloadTooLarge`),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_recv_of_a_datagram_above_max_datagram_is_an_error_not_a_short_message`
(receiver 1024, sender 8192, 4000 bytes: `BufferTooSmall { datagram_size:
4000, buffer_size: 1024 }` on Linux; the next datagram arrives whole),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_oversized_for_the_kernel_fails_loudly_and_never_truncates`
(1 KiB arrives untruncated; at `max_datagram` either the delivery is
whole or the error names EMSGSIZE — and then nothing partial is in the
queue),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_3_abstract_recv_above_max_datagram_is_an_error`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_3_abstract_default_max_datagram_is_65536`,
`crates/dcps/src/runtime.rs::tests::uds_3_3_a_datagram_above_the_transport_limit_is_counted_by_the_user_receive_loop`
(a transport reports `BufferTooSmall` once: the counter rises, the loop
goes on receiving, and the runtime's sink receives exactly one event
`transport.user_recv.datagram_above_limit` with `size` 4000 and `limit`
1024).

**Status:** done — on macOS (without `MSG_TRUNC`) `datagram_size`
reports `max_datagram + 1` instead of the real length; that branch ran
green on a Mac (macOS 26.2, arm64) and on runner 8 (job 238162, §6.1).

### §3.4 Kernel bound versus configured bound

**Spec:** §3 — the real per-message bound is the kernel's: Linux via
`wmem_max` (default 212 992), macOS via `net.local.dgram.maxdgram`
(default 2048). The transport MUST reconcile the configured bound with
the operating system's, surface it at bind time or size the socket
buffers accordingly — otherwise a sample between the two bounds fails
only at runtime. The bind sets the buffers and reads back what the
kernel granted; if that does not carry `max_datagram`, the bind fails
with `InvalidInput` naming the bound, the granted size and the limiting
setting. Linux refuses a datagram above `SO_SNDBUF − 32` and doubles a
request it caps at `wmem_max`, so the largest possible bound there is
`2 · wmem_max − 32`; macOS needs `SO_SNDBUF` and `SO_RCVBUF` of at least
`max_datagram`.

**Repo:** `crates/transport-uds/src/lib.rs` (`size_buffers`: send
buffer at twice, receive buffer at eight times `max_datagram`, on a
refused request at what is needed, then a read-back and a check against
`max_datagram + SEND_BUFFER_OVERHEAD`; called in `UdsTransport::bind` and
`UdsAbstractDgramTransport::bind`, a failure removes the socket file
again).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_4_a_datagram_near_the_configured_limit_arrives_whole`
(four datagrams of `max_datagram` − 512 bytes arrive whole),
`crates/transport-uds/src/lib.rs::tests::uds_3_4_bind_refuses_a_max_datagram_the_kernel_does_not_grant`
(1 GiB: `InvalidInput` with `max_datagram 1073741824` and
`net.core.wmem_max`, no socket file stays),
`crates/transport-uds/src/lib.rs::tests::uds_3_4_bind_accepts_exactly_the_largest_max_datagram_the_kernel_grants`
(Linux: `2 · wmem_max − 32` binds, one byte more does not; the message
names `SO_SNDBUF 2 · wmem_max`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_4_abstract_bind_refuses_a_max_datagram_the_kernel_does_not_grant`
(both modes),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_oversized_for_the_kernel_fails_loudly_and_never_truncates`.

**Status:** done — measured on Linux; the macOS branch (`SO_RCVBUF`
checked as well, `kern.ipc.maxsockbuf` in the message) ran green on a
Mac (macOS 26.2, arm64) and on runner 8 (job 238162, §6.1).

### §3.5 Stream mode: frames over `SOCK_STREAM`

**Spec:** §3 — Beside the datagram mode there is the stream mode over
`AF_UNIX` sockets of type `SOCK_STREAM`; it is the only mode on Windows
(§6.2). Each transport listens at `<base_dir>/<hex32>.sock` (§2.1). A
connection begins with the hello: the four octets `ZUDS` and the
sender's 16-byte id. The id is the source locator of every message on
the connection (`Locator::uds(id)`); the connecting socket is not bound,
its address does not name the sender. After it the connection carries
each RTPS message as a frame: the length as a little-endian `u32`, then
the octets. A frame above `max_datagram` is reported on receive as
`BufferTooSmall` with its length and skipped; the frames after it on the
connection arrive. A connection that has not sent the whole hello within
5 s of its accept is closed and counted; the 5 s hold for all reads of the
hello together. A connection that ends inside a frame is counted as a
truncated frame, and the partial frame is not handed on. A connection
whose peer wrote and closed before it was accepted is read as any other:
its frames arrive. The sender keeps
one connection per destination and opens it on the first send; a send
waits at most `send_timeout` for room and then fails with "uds: kernel
send buffer full (EAGAIN)" (§4.3). A frame that went out only in part
closes the connection, so that the peer does not read the rest as the
start of the next; a connection the peer has closed (a restart) is
replaced by a new one once, within the same send. The receive queue holds
128 frames; when it is full the connections stop reading, the kernel
buffers fill, and the peer's send fails after its `send_timeout` as a
datagram to a full queue does (§4.3); nothing is dropped. Path, base
directory (§2.2), socket-file mode (§5.1) and error classes (§4.3) are
those of the datagram mode; the kernel limit of §3.4 does not apply, a
stream has no limit per message. In the datagram mode the bound source
path proves the id; in the stream mode the sender states it in the
hello, and whoever may write to the socket file can state any id. The
runtime chooses the mode with `transport.uds.mode` (`datagram` or
`stream`; default `datagram`, on Windows `stream`).

**Repo:** `crates/transport-uds/src/stream.rs` (`UdsStreamTransport`,
`STREAM_MAGIC`, `HELLO_TIMEOUT`, `STREAM_QUEUE_FRAMES`; an accept thread,
a reader thread per connection, a receive queue with back pressure;
`read_connection` waits with `poll` (on Windows `WSAPoll`) before each
read of the hello at most the time left until 5 s after the accept and
refuses the connection when it has run out; `accept` sets no socket
option on an accepted connection (on Apple systems `accept_raw` and
`set_cloexec`: macOS refuses every socket option with `EINVAL` on a
connection whose peer has closed, and `Socket::accept` sets
`SO_NOSIGPIPE` there); the accept thread waits for a connection with
`poll` (on Windows `WSAPoll`) at most `ACCEPT_POLL` (50 ms) at a time and
looks in between whether the transport closed, so that a close does not
go through the socket path, which another transport may have bound
since; counters `accepted_connections`, `truncated_frames`, `refused_hellos`,
`connection_errors`; `recv_batch`, `close`, `forget_destination`);
`crates/transport-uds/src/lib.rs` (`classify_send_error`: `TimedOut` as
`WouldBlock`, a connection the peer closed as "uds: the peer closed the
connection (EPIPE)"); `crates/dcps/src/runtime.rs` (`UdsMode`,
`DEFAULT_UDS_MODE`, `UdsUserConfig::mode`, the row `transport.uds.mode`
in `SETTINGS`, `uds_config_from`, `select_user_transport` binds the
stream transport in mode `stream`, the event `transport.uds.bind_failed`
names the mode); `man/man5/zerodds.yaml.5`;
`packaging/linux/configs/zerodds.yaml.example`;
`docs/doc-map/config.md`; `tests/integration/src/kernel.rs`
(`unix_stream_sockets`); `ci/integration_scenarios.py` (scenario
`dcps-uds-stream`).

**Tests:** `crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_frames_keep_message_boundaries`
(messages of 0, 1 and 8192 octets arrive whole and in order),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_source_locator_is_the_senders_id_and_reaches_it`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_frame_above_max_datagram_is_reported_and_the_next_arrives`
(a raw client sends 8193 octets to a transport with 8192:
`BufferTooSmall { 8193, 8192 }`, the next frame arrives),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_send_refuses_a_payload_above_max_datagram`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted`
(also: no connection error; on macOS the client that closed before the
accept made the accept fail with `EINVAL`, and the test failed after 5 s,
job 238162),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_peer_that_closed_before_the_accept_is_read_whole`
(a raw client connects, writes the hello, a whole frame and the start of
a second and closes; then the connection is accepted and read: the whole
frame arrives with the client's id as source, the cut one is counted, no
connection error; on macOS the test fails with the former `accept`
(`EINVAL`) and with the former `SO_RCVTIMEO` for the hello (refused)),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_connection_without_a_hello_within_5_s_is_refused`
(a connection that sends no octet and one that sends the hello in pieces
1.5 s apart, the rest after 6 s: at 4.5 s neither is refused, after 5 s
both are refused and counted, the frame after the late hello does not
arrive; with the former timeout per read the test failed),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_reconnects_after_the_peer_restarts`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_recv_batch_takes_the_queued_messages`,
`crates/transport-uds/src/stream.rs::tests::uds_4_1_stream_bind_replaces_a_leftover_file_and_drop_removes_it`,
`crates/transport-uds/src/stream.rs::tests::uds_4_1_stream_a_transport_whose_path_another_took_is_dropped`
(two transports bind the same id in one directory; the drop of the
first returns within 1 s, the second accepts no connection; with the
former wake-up through the path the drop blocked),
`crates/transport-uds/src/stream.rs::tests::uds_4_4_stream_recv_times_out_when_idle_and_close_ends_it`,
`crates/transport-uds/src/stream.rs::tests::uds_4_3_stream_send_names_a_missing_socket_and_a_refusal`,
`crates/transport-uds/src/stream.rs::tests::uds_4_3_stream_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error`
(after at least 128 accepted frames a send fails with "uds: kernel send
buffer full (EAGAIN)" in less than 1 s with a `send_timeout` of 50 ms;
once the peer reads, the next send goes out),
`crates/transport-uds/src/stream.rs::tests::uds_5_1_stream_socket_mode_is_applied_to_the_socket_file`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_5_l1_stream_frames_cross_the_process_boundary`,
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(`transport.uds.mode` from a file and the environment),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_a_bad_uds_value_is_refused_instead_of_falling_back`
(`seqpacket`, `Stream`),
`crates/dcps/tests/contract_transport_uds.rs::uds_3_5_a_refused_stream_bind_is_reported_with_its_mode`
(a participant in mode `stream` whose `base_dir` is a file: the start
fails with `TransportError` "user unicast bind (UDS)", exactly one event
`transport.uds.bind_failed` of level `Error` with `mode` `stream`,
`base_dir` and "uds base_dir exists but is not a directory"; on Linux and
in the Windows job `platform-windows-dcps-uds`),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_a_refused_bind_is_reported_with_its_cause`
(`mode` `datagram` in the event),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_in_stream_mode`
(writer and reader processes over the DCPS API with `transport.uds.mode`
`stream`: every sample arrives; each process listens with a stream
socket in the configured directory and holds a connection the other
opened to it; no UDP socket on the user-traffic ports).

**Status:** done — measured on the Linux test host: `cargo nextest run --profile
merge -p zerodds-transport-uds --all-targets` 91 tests green, the 15
stream tests of `src/stream.rs` green in 10 runs in a row; the scenario
`dcps_over_uds_in_stream_mode` green. On a Mac (macOS 26.2, arm64) the
15 stream tests green in 10 runs in a row in the sandbox of
`ci/macos-loopback.sb` (§6.1); Windows §6.2.

### §4.1 Filesystem mode: cleanup on drop, takeover on bind

**Spec:** §4 — bind creates the socket file, drop removes it best-effort
(an unmounted container volume must not make the drop fail). If a bind
finds a leftover file from a crashed predecessor, it MUST remove it and
rebind: `UnixDatagram::bind` cannot bind over an existing file and
otherwise reports `EADDRINUSE`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport::bind` with
`remove_file` on an existing path, `Drop for UdsTransport`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_1_bind_creates_socket_file`
(the file exists after bind and is gone after drop),
`crates/transport-uds/src/lib.rs::tests::uds_4_1_bind_reuses_path_after_stale_leftover`
(a leftover file with foreign content does not block the bind).

**Status:** done

### §4.2 Abstract mode: no cleanup needed

**Spec:** §4 — the abstract namespace has no inode; Linux releases the
name once the last file descriptor closes. A crash-cleanup path is
therefore not required, and a bind must not create a file.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport::bind` in the `Abstract` branch without
filesystem access, `Drop` removing only in the `Filesystem` branch).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_bind_does_not_create_file`
(the name is in `/proc/net/unix` as `@<name>`, no file is created, after
the drop the entry is gone),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_2_abstract_name_is_free_after_drop`
(a second bind of the same name fails with `AddrInUse`, after the drop
of the first it succeeds without cleanup),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_bind_creates_socket_file`
(the counter-check: in the filesystem branch the file appears and
disappears). Linux-gated as §2.3.

**Status:** done

### §4.3 Differentiated send-error classification

**Spec:** §4 — a send error MUST name its cause: a missing destination
socket, a denied permission, a full kernel buffer, a refused connection
and an exceeded message bound are different operational states. A
catch-all `Io` makes them indistinguishable. This holds for both
transports; in the abstract namespace `ECONNREFUSED` means that no
socket holds the name.

A full kernel buffer MUST arrive as a send error and must not hold the
sending thread until the peer reads. Linux queues datagrams for a unix
datagram socket until its queue holds `net.unix.max_dgram_qlen + 1`
(10 in a new network namespace, 512 in the host's namespace of the
Linux test host and in the CI container) or the sender's send buffer is
full: each queued datagram charges its buffer size to the sender's
`SO_SNDBUF` until the peer reads it (with `SO_SNDBUF` 212 992, 278 short
datagrams). When either is full, a blocking send to a peer that does not
read waits without a time limit. A send therefore waits at most `send_timeout`
(default 100 ms) for room in the peer's queue or in its own send buffer
and then fails with "uds: kernel send buffer full (EAGAIN)". A peer
that reads empties its queue within that time and the datagram goes
out; a peer that has stopped reading costs each send to it at most
`send_timeout`. A `send_timeout` of zero does not wait. macOS does not
queue a datagram for a full receive buffer and reports `ENOBUFS` at
once.

**Repo:** `crates/transport-uds/src/lib.rs` (`classify_send_error` —
`NotFound`, `PermissionDenied`, `WouldBlock`, `ConnectionRefused`,
`EMSGSIZE` and `ENOBUFS` via `raw_os_error`; `UdsConfig::send_timeout`,
`DEFAULT_SEND_TIMEOUT`, `set_send_timeout` sets `SO_SNDTIMEO` at bind,
`send_flags` gives `MSG_DONTWAIT` for a `send_timeout` of zero and
`MSG_NOSIGNAL` on Linux, `Transport::send` sends with
`send_to_with_flags`); `crates/transport-uds/src/abstract_dgram.rs`
(`AbstractDgramConfig::send_timeout`, the same bind and send path;
`classify_send_error`: `ECONNREFUSED` in the abstract mode with a
message of its own, otherwise the crate root's);
`crates/dcps/src/runtime.rs` (`select_user_transport` binds the UDS user
transport with `DEFAULT_SEND_TIMEOUT`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_3_classify_send_error_maps_kinds`
(every named `io::ErrorKind` value against its message),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_classify_send_error_names_emsgsize_and_enobufs`
(raw error numbers `EMSGSIZE`, `ENOBUFS`, `EAGAIN`, `EACCES`, `ENOENT`,
`ECONNREFUSED` against their message),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_send_to_missing_peer_names_the_missing_socket`,
`crates/transport-uds/src/lib.rs::tests::uds_4_3_send_to_a_leftover_socket_file_names_the_refusal`
(a socket file without a socket gives the `ECONNREFUSED` message),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_abstract_send_to_missing_peer_names_the_cause`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_filesystem_mode_send_errors_name_the_cause`,
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied`
(the permission message across a real UID boundary, §5.1),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error`
(a receiver that never reads: on Linux datagrams go out until the
peer's queue or the sender's send buffer is full, at most
`max_dgram_qlen + 1`; the next send waits at least `send_timeout`
(50 ms) and fails with the `EAGAIN` message; a datagram of a second
sender is refused as well when the queue was full, and goes in when the
first sender's send buffer was full; the queued datagrams arrive whole,
nothing of the failed one; on macOS the same test expects the `ENOBUFS`
message),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_with_send_timeout_zero_a_full_queue_fails_the_send_at_once`
(`send_timeout` zero: the send that finds no room fails with `EAGAIN`
without waiting, and so does the next),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_a_send_to_a_full_queue_goes_out_when_the_peer_reads_within_send_timeout`
(the queue or the send buffer filled with `MSG_DONTWAIT` until `EAGAIN`,
the peer starts reading after 200 ms: the send waits and goes out, every
datagram arrives in order),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_the_default_send_timeout_is_100_ms`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_a_full_peer_queue_fails_the_send_after_send_timeout_in_both_modes`
(abstract and filesystem mode, filled until the first `EAGAIN`).

**Status:** done — the states of the full kernel buffer measured on
the Linux test host, as root and as `nobody`, with
`net.unix.max_dgram_qlen` 10 (the queue full first) and 512 (the send
buffer full first, after 278 datagrams, as in the CI container: there
the former tests, which expected `max_dgram_qlen + 1`, failed, job
238176). The macOS branch (`ENOBUFS` on a full receive buffer) ran green
on a Mac (macOS 26.2, arm64) and on runner 8 (job 238162, §6.1).

### §4.4 Idle behaviour of `recv`

**Spec:** §4 — with `recv_timeout` set, an empty socket MUST yield
`RecvError::Timeout` after it elapses, not an Io error. Without a timeout
`recv` blocks.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::recv_timeout`,
`set_read_timeout` in bind, the `WouldBlock`/`TimedOut` branch in
`Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_4_recv_times_out_when_idle`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_4_an_abstract_socket_recv_times_out_when_idle`
(the same evidence in the abstract transport, Linux-gated).

**Status:** done

## §5 Container use case

### §5.1 Volume mount as the permission boundary

**Spec:** §5 — the Docker and Kubernetes pattern is a shared mounted
volume under `base_dir`; every container binds its own 16-byte locator as
a socket file and the kernel routes between the file descriptors. UDS is
the choice there when multicast is blocked by a cluster network policy
and POSIX SHM is impractical cross-container (UID mapping, `/dev/shm`
visibility, SELinux profiles).

Permission model: a sender needs write permission on the socket file and
search permission on the directory; a binder needs write and search
permission on the directory. When both sides run under the same UID,
`base_dir` is enough. Under different UIDs they need a group they share,
socket files with group write permission (`0660`) and a directory of
that group with the mode `3770`. The set-group-ID bit gives new socket
files the directory's group; without it a socket file carries the
binder's primary group, and containers with different primary groups
that have the shared group only as a supplementary group refuse each
other. The sticky bit lets only the owner of a socket file (and the
directory's owner) remove or rename it; without it any UID of the group
can delete another's socket file and put its own socket or a symlink at
its path. The directory gets the group when the operator prepares it
(`chgrp`, `chmod 3770`) or when the runtime creates it in a parent of
the group with the set-group-ID bit: a new directory takes the parent's
group. The transport sets the mode of the socket file right after the
bind (`socket_mode`; without it the umask applies, `0755` with `022`,
and another UID is refused with "permission denied on destination"),
and only on the socket it bound, never through a symlink; when the path
holds anything but a socket of its own effective UID, the bind fails.
The mode of a directory it creates it sets to `base_dir_mode` (default
`0700`, which shuts every other UID out) and checks (§2.2). The runtime
reads both, with the directory and the datagram bound, from
`transport.uds.base_dir`, `transport.uds.socket_mode` (`umask` or
octal), `transport.uds.base_dir_mode` (octal) and
`transport.uds.max_datagram`. Under user namespaces the rule applies to
the IDs the volume has on the host; under SELinux the volume needs a
label both containers may use (`:z` with Docker and Podman) — both are
matters of the container runtime. Discovery (SPDP/SEDP) stays on UDPv4:
without multicast the containers need unicast peers
(`discovery.peers`).

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::base_dir`,
`UdsConfig::socket_mode`, `UdsConfig::base_dir_mode`, `check_modes`,
`apply_socket_mode` — on Linux `O_PATH | O_NOFOLLOW`, the descriptor
checked by `check_own_socket` (a socket owned by the effective UID),
`chmod` through `/proc/self/fd`; elsewhere an `lstat` check and
`fchmodat` with `AT_SYMLINK_NOFOLLOW` —, `ensure_base_dir`; the drop
cleanup is best-effort so an unmounted volume does not make the drop
fail);
`crates/transport-uds/src/abstract_dgram.rs` (`UdsAddress::Filesystem`
with `socket_mode` and `base_dir_mode`); `crates/dcps/src/runtime.rs`
(`UdsUserConfig`, `RuntimeConfig::uds`, the `transport.uds.*` rows of
`SETTINGS`, `uds_config_from`, `parse_octal_mode`,
`parse_uds_socket_mode`, the check in `valid_runtime_value`,
`select_user_transport` binds with them and reports a bind failure as
the event `transport.uds.bind_failed`); `man/man5/zerodds.yaml.5`;
`packaging/linux/configs/zerodds.yaml.example`;
`docs/doc-map/config.en.md`.

**Tests:** In `crates/transport-uds/tests/l2_cross_uid.rs` the
processes run under the UIDs 61001 and 61002, each with a primary group
of its own (61001, 61002) and the group 61042 as a supplementary group;
the first binder creates the directory in a parent of the group 61042
with the set-group-ID bit.
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_two_uids_with_a_shared_group_exchange_datagrams_in_both_directions`
(`socket_mode` `0660`, `base_dir_mode` `3770`: a datagram there and
back, each source locator the other's, socket files `0660` owned by the
respective UID with the group 61042, which neither process has as its
primary group; the directory `3770`),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_the_set_group_id_bit_a_socket_file_keeps_the_binders_group_and_is_refused`
(directory `0770` of the group 61042: the socket file carries the
binder's primary group 61001, the second UID binds, its send is refused
with "uds: permission denied on destination"),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_under_3770_the_other_uid_cannot_remove_a_socket_file_it_can_remove_under_2770`
(under `3770` the removal of UID 61001's socket file by UID 61002 fails
with `PermissionDenied` and the file stays; under `2770` it succeeds),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind`
(§2.2),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied`
(socket file `0755` from the umask, the second UID gets "uds: permission
denied on destination"),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_base_dir_created_with_the_default_mode_shuts_the_other_uid_out`
(directory `0700`, the second UID's bind fails with `PermissionDenied`),
`crates/transport-uds/src/lib.rs::tests::uds_5_1_apply_socket_mode_refuses_a_symlink_and_a_file_that_is_no_socket`
(a symlink to a file with `0600` and a file that is no socket are
refused with `InvalidInput`, the link's target keeps `0600`; a socket of
its own takes the mode),
`crates/transport-uds/src/lib.rs::tests::uds_5_1_socket_mode_is_applied_to_the_socket_file`,
`crates/transport-uds/src/lib.rs::tests::uds_5_1_base_dir_mode_is_applied_to_a_created_dir`,
`crates/transport-uds/src/lib.rs::tests::uds_5_1_a_mode_outside_the_permission_bits_is_refused`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_5_1_filesystem_mode_applies_socket_mode_and_base_dir_mode`,
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(default, file and environment),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_a_bad_uds_value_is_refused_instead_of_falling_back`,
`crates/dcps/tests/contract_transport_uds.rs::uds_5_1_the_runtimes_uds_settings_default_to_the_transports_defaults`,
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_in_a_configured_base_dir`
(a writer and a reader process through the DCPS API with
`transport.uds.base_dir`, `socket_mode` `0660` and `base_dir_mode`
`3770`: every sample arrives, both sockets lie in the configured
directory with `0660`, the directory carries `3770`),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_a_refused_bind_is_reported_with_its_cause`
(a participant with the UDS user transport and
`transport.uds.max_datagram` 1 GiB: the start fails with
`TransportError` "user unicast bind (UDS)", the sink receives exactly
one event `transport.uds.bind_failed` of level `Error` with `base_dir`,
`mode` `datagram` and the transport's message "uds max_datagram 1073741824 exceeds what
the kernel grants … (capped by net.core.wmem_max)"),
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`.

**Status:** partial — the permission model, the keys and the runtime
wiring are built and measured on the Linux test host across a real UID
boundary (as root, which starts the two processes under other UIDs), and
so is the volume under Docker and under Podman with a user namespace per
container (§5.2). Not measured: the SELinux label; it needs a host with
SELinux in enforcing mode on which the container tests of §5.2 run
(their mounts carry `:z`).

### §5.2 Evidence across a real container boundary

**Spec:** §5 — that the pattern of §5.1 holds under a container runtime
MUST be shown by a test with two containers and one mounted volume:
different UID mappings on both sides, the socket file on the volume, a
datagram in each direction.

**Repo:** `tools/isolation-smoke/src/main.rs` (roles `echo` — sends
each datagram back to its source locator — and `ping` — sends `--count`
datagrams one at a time and checks each echo for its content and its
source; `--socket-mode`, `--hold-ms`);
`tools/isolation-smoke/tests/container_volume.rs` (starts the two roles
in two containers from `debian:trixie-slim` with the built binary,
without a network, the volume at `/run/zerodds-uds` in both with `:z`;
the volume's root belongs to root and the group 61042 with `3770`; a
thread of the test reads the socket files from the host);
`ci/manual-tests.tsv` (the three container tests);
`testing/isolation/docker/docker-compose.uds.yml` and
`testing/isolation/docker/Dockerfile` (the same pattern with Docker
Compose). Podman gives a new named volume, on its first mount, the owner
and group of the container's user and so removes the shared group
(measured with Podman 5.4.2); the Podman test therefore uses a host
directory as its volume.

**Tests:** `tools/isolation-smoke/tests/container_volume.rs::uds_5_2_two_containers_under_different_uids_exchange_datagrams_over_a_volume_in_both_directions`
(Docker, a named volume; the echo under UID 61001, the ping under UID
61002, each with its own primary group and the supplementary group
61042, both with the socket-file mode `0660`: 50 datagrams there and back, each
echo unchanged and from the echo's locator, the echo answers the ping's
locator; on the host both socket files are on the volume, sockets with
`0660`, owners 61001 and 61002, group 61042),
`tools/isolation-smoke/tests/container_volume.rs::uds_5_2_without_socket_mode_the_container_of_the_other_uid_is_refused`
(the echo without a socket-file mode of its own: its file has neither group nor
other write permission, the ping's first send is refused with "uds:
permission denied on destination", exit code 3),
`tools/isolation-smoke/tests/container_volume.rs::uds_5_2_two_containers_with_different_uid_mappings_exchange_datagrams_in_both_directions`
(Podman, a user namespace per container: UID 1000 is 101000 on the host
for the echo and 201000 for the ping, the group 61042 is 61042 in both;
50 datagrams in both directions; on the host the socket files carry the
owners 101000 and 201000, the group 61042 and `0660`),
`tools/isolation-smoke/tests/roundtrip.rs::uds_5_2_ping_and_echo_exchange_datagrams_in_both_directions`,
`tools/isolation-smoke/tests/roundtrip.rs::uds_5_2_socket_mode_and_the_echo_roles_are_refused_where_they_do_not_apply`.

**Status:** done — the three container tests are manual tests on the
Linux test host (`ci/manual-tests.tsv`, decision 11). They need root on
the machine the container daemon runs on: the test sets the owner, the
group 61042 and the mode of the volume's root and reads the socket files
on the volume from the host; the third test needs Podman as well. The
GitLab runners have no admin rights (decision 11); runner 8 starts
Docker containers in colima's VM, where the volume lies as well, not on
the job's machine; no runner has Podman. Green there as root with Docker
26.1.5 and Podman 5.4.2 (`cargo test -p zerodds-isolation-smoke --test
container_volume -- --ignored`: 3 passed). The two tests of
`roundtrip.rs` run in the crate tier. A host with SELinux in enforcing
mode was not available; the mounts carry `:z`, which both runtimes
ignore without SELinux.

## §6 Platform support

### §6.1 Linux and macOS

**Spec:** §6 — Linux is the primary platform with both modes
(filesystem and abstract namespace), macOS supports the filesystem mode.
Both have the datagram and the stream mode (§3.5); the datagram transport
is `#[cfg(unix)]`, §6.2 covers Windows.

**Repo:** `crates/transport-uds/src/lib.rs` (the datagram transport
under `cfg(unix)`,
`#[cfg(target_os = "linux")] pub mod abstract_dgram`; the macOS branches
`SEND_BUFFER_OVERHEAD`, `BUFFER_CAP_SYSCTL`, `RECV_TRUNC_FLAG` and the
`SO_RCVBUF` check in `size_buffers` under
`cfg(not(any(target_os = "linux", target_os = "android")))`);
`ci/platform_jobs.py` and `ci/platforms.yml` (job
`platform-macos-transport-uds` on runner 8, weekly, on a release
candidate and as `merge-macos-transport-uds` when the package changes);
`ci/test_platform_jobs.py`
(`test_a_change_to_transport_uds_runs_its_macos_job`). The tests of
`src/lib.rs` and `tests/l1_cross_process.rs` create their temporary
directories under `/tmp` (`short_tempdir`): `sun_path` holds 104 bytes
on macOS, and under the per-user `TMPDIR` (`/var/folders/<2>/<30>/T/`,
49 bytes) a socket in a subdirectory of a temporary directory would have
104 bytes, which the bind refuses; under `/tmp` the longest path has 59.
The directory gets the process's group, because on macOS it takes the
group `wheel` of `/tmp` and a set-group-ID bit needs a group of the
process. `ci/macos-loopback.sb`, the sandbox the job runs the tests in,
admits the operation `file-write-setugid`: without it macOS drops the
set-user-ID and set-group-ID bits of a `chmod` in the sandbox without an
error (measured: `chmod 3770` of a directory of the process's group gives
`1770` inside the sandbox, `3770` outside), and
`uds_5_1_base_dir_mode_is_applied_to_a_created_dir` failed with the
refusal of §2.2 (job 238162).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`,
`crates/transport-uds/src/lib.rs::tests::uds_3_4_a_datagram_near_the_configured_limit_arrives_whole`,
`crates/transport-uds/src/lib.rs::tests::uds_3_3_recv_of_a_datagram_above_max_datagram_is_an_error_not_a_short_message`
(on both platforms; the macOS job runs every test of the package),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_send_recv_roundtrip` (Linux only).

**Status:** partial — Linux measured on the Linux test host. macOS
measured on a Mac (macOS 26.2, arm64): `cargo nextest run --profile
merge -p zerodds-transport-uds --all-targets` 56 tests green, without a
sandbox and in that of `ci/macos-loopback.sb`, as the job runs them; the
sockets of the tests lie under `/tmp`, so their path lengths are
measured. Runner 8 (`TMPDIR` `/var/folders/80/<30>/T/`, 49 bytes) ran
the job `merge-macos-transport-uds` in pipeline 9916: 55 tests, 53
green; the two failures,
`uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted`
(§3.5) and `uds_5_1_base_dir_mode_is_applied_to_a_created_dir` (the
sandbox above), are fixed. Open: a green run of that job on runner 8.

### §6.2 Windows

**Spec:** §6 — Windows offers sockets of the family `AF_UNIX` since
Windows 10 (build 17063), but only of type `SOCK_STREAM`, and has no mode
bits; a socket file carries the access control list (ACL) of its
directory, and whoever has write access to the file may connect. On
Windows the UDS transport is therefore the stream mode of §3.5
(`UdsStreamTransport`); the runtime there has `transport.uds.mode`
`stream` as its default, and `datagram` fails the start. Path as in §2.1;
the default `/tmp/zerodds/uds` lies on the current drive
(`\tmp\zerodds\uds`), `transport.uds.base_dir` takes a path from the
root of a drive, and the path of the socket file must fit `sun_path`
(108 octets). A base directory the bind creates gets a protected ACL
with full access for the process's user and `SYSTEM`
(`D:P(A;OICI;FA;;;<SID>)(A;OICI;FA;;;SY)`; a process that runs as
`SYSTEM` has the second entry alone), which the socket files in it
inherit: the directory is private as `0700` makes it on Unix (§2.2).
An existing directory keeps its ACL; a reparse point (a symbolic link, a
junction) is refused. `socket_mode` and a `base_dir_mode` other than
`0700` are refused with `Unsupported`; a directory for several users is
prepared by the operator with an ACL. The error classes are those of
§4.3; an expired `SO_SNDTIMEO` (`WSAETIMEDOUT`) counts as a full send
buffer, and the connection is not used again after it, because Microsoft
describes its state after a timeout as indeterminate. The close of the
transport shuts the connections down, which on Windows does not end a
read that waits on one; a read there waits with `WSAPoll`, at most
`ACCEPT_POLL` at a time, and looks at the closed flag in between. A
write after the peer closed succeeds on Windows, so a send there looks
first whether the peer has closed and connects anew if so (§3.5). Named
pipes are not part of this spec.

**Repo:** `crates/transport-uds/src/lib.rs` (`#![cfg(any(unix,
windows))]`; the datagram transport, the modes and `abstract_dgram`
under `cfg(unix)`; `socket_path` writes the directory with the
platform's separators on Windows; `classify_send_error` with
`WSAENOBUFS`); `crates/transport-uds/src/windows.rs` (`check_modes`,
`ensure_base_dir`, `create_private_dir` with `CreateDirectoryW` and the
ACL of `private_dacl_sddl`, `current_user_sid`);
`crates/transport-uds/src/stream.rs` (§3.5);
`crates/transport-uds/Cargo.toml` (`windows-sys` for Windows, `libc`
for Unix only); `crates/dcps/src/runtime.rs` (`UserTransportKind::Uds`
under `any(unix, windows)`, `DEFAULT_UDS_MODE`, in mode `datagram` on
Windows an `Unsupported` error with the event
`transport.uds.bind_failed`, `transport.uds.base_dir` on Windows from the
root of a drive); `ci/platform_jobs.py` (`WINDOWS_EXTRA`,
`WINDOWS_FEATURE_RUNS`) and `ci/platforms.yml` (on the Windows runner,
weekly and on a release candidate: job `platform-windows-transport-uds`
with every test of the package, job `platform-windows-dcps-uds` with the
target `contract_transport_uds` of `zerodds-dcps` and the feature
`same-host-uds`, the runtime's UDS arm); `ci/platform-jobs.yml` (the
Windows template passes `PLATFORM_FEATURES` as `--features` and
`PLATFORM_TESTS` as `--test`); `ci/cutover_gate.py` (`platform_runs`
counts with the job's features); `ci/test_platform_jobs.py`
(`test_windows_runs_the_uds_transport_after_the_shm_transport`,
`test_windows_runs_the_runtimes_uds_arm_with_its_feature`).

**Tests:** `crates/transport-uds/src/windows.rs::tests::uds_6_2_a_created_base_dir_is_private_to_the_user`
(the ACL of the created directory is the user's `private_dacl_sddl`, the
socket file inherits full access for the user and `SYSTEM`),
`crates/transport-uds/src/windows.rs::tests::uds_6_2_mode_bits_are_refused_on_windows`,
`crates/transport-uds/src/windows.rs::tests::uds_6_2_a_junction_as_base_dir_is_refused`,
`crates/transport-uds/src/windows.rs::tests::uds_6_2_the_stream_transport_carries_messages_on_windows`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_5_l1_stream_frames_cross_the_process_boundary`
(on Windows the UDS transport across a process boundary),
`crates/dcps/tests/contract_transport_uds.rs::uds_6_2_the_runtimes_default_mode_is_the_platforms`,
`crates/dcps/tests/contract_transport_uds.rs::uds_6_2_the_mode_datagram_fails_the_start_on_windows`
(Windows only: a participant in mode `datagram` fails with
`TransportError` "user unicast bind (UDS)", exactly one event
`transport.uds.bind_failed` with `mode` `datagram` and the reason,
nothing is created in `base_dir`),
`crates/dcps/tests/contract_transport_uds.rs::uds_3_5_a_refused_stream_bind_is_reported_with_its_mode`
(on Windows through the base directory check of
`crates/transport-uds/src/windows.rs`),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(default `stream` on Windows; `/run/zerodds-uds` counts there as a path
from the root); the tests of `src/stream.rs` (§3.5) run in the Windows
job `platform-windows-transport-uds` as well, those of
`contract_transport_uds` in the job `platform-windows-dcps-uds`.

**Status:** partial — built. Measured on the Linux test host: the stream
mode (§3.5), the runtime in mode `stream` and `contract_transport_uds`
with `same-host-uds` (7 tests, 6 of them compiled and green on Linux).
Compiled for `x86_64-pc-windows-gnu`: `cargo clippy --target
x86_64-pc-windows-gnu -p zerodds-transport-uds --all-targets -- -D
warnings` and `cargo clippy --target x86_64-pc-windows-gnu -p
zerodds-dcps --features same-host-uds --all-targets -- -D warnings`
without findings. Not run on Windows: the jobs
`platform-windows-transport-uds` (the transport) and
`platform-windows-dcps-uds` (the runtime's UDS arm: the default
`stream`, `datagram` failing the start, the event
`transport.uds.bind_failed` with the mode) need the Windows runner and a
pipeline that starts them; whether the second job fits the build of
`zerodds-dcps` under `same-host-uds` into 5 minutes is not measured. The statements about Windows (only
`SOCK_STREAM`, permissions through the file's ACL, `SO_SNDTIMEO`, the
error codes of a missing path) come from Microsoft's documentation and
are not measured.

### §6.3 Cross-vendor interop

**Spec:** §6 — UDS is intra-container and intra-host IPC. Cross-vendor
interop with Cyclone DDS or FastDDS stays in the UDP, TCP and SHM
domains; neither vendor has a UDS transport a wire comparison could run
against. The section records the scope and states no implementation
requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — neither Cyclone DDS nor FastDDS has a UDS transport against which a wire comparison would be possible.

---

## Audit-Status

15 done / 3 partial / 0 open / 0 blocked / 2 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-uds-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (job `merge-rust-rtps`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-transport-uds --all-targets` (job `merge-rust-transport-uds`) — 29 tests green.

31 cited tests green, 0 not green.

Open items: `zerodds-uds-transport-1.0.open.en.md`.
