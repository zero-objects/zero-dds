# ZeroDDS UDS Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. For Unix domain sockets OMG
standardizes nothing beyond the vendor-reserved locator kind
(DDSI-RTPS 2.5 §9.4.5); path resolution, wire format and cleanup are
vendor-specific. Cyclone DDS and FastDDS have no official UDS transport.
The normative requirements live in the `##` sections §2 through §6; every
`###` item below carries a spec citation, a repo anchor, a test and a
status.

Implementation: `crates/transport-uds/src/lib.rs` — `SOCK_DGRAM` over
filesystem sockets; `crates/transport-uds/src/abstract_dgram.rs` — the
Linux abstract namespace, `#[cfg(target_os = "linux")]`.

Open items and decision records: [`zerodds-uds-transport-1.0.open.en.md`](zerodds-uds-transport-1.0.open.en.md).

## §1 Scope and spec status

### §1.1 Vendor-reserved locator kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` to `0x01ffffff` are reserved
for vendor-specific locator kinds. ZeroDDS takes `0x01F0_0002` for UDS. The port
field is meaningless and MUST be `0`; the 16 address bytes carry the
endpoint id.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Uds`,
`Locator::uds`); bound in `crates/transport-uds/src/lib.rs`
(`UdsTransport::local_locator`, the kind check in `Transport::send`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::locator_kind_is_the_vendor_reserved_uds_value`
(asserts the numeric value `0x01F0_0002`, the position in the range `0x00000003` to `0x01ffffff` and
`port == 0`),
`crates/transport-uds/src/lib.rs::tests::local_locator_reflects_bind_id`,
`crates/transport-uds/src/lib.rs::tests::send_rejects_non_uds_locator`,
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

**Status:** n/a (informative)

## §2 Path resolution

### §2.1 Filesystem mode (default)

**Spec:** §2 — the 16-byte locator address resolves to
`<base_dir>/<lowercase-hex32>.sock`. The default `base_dir` is
`/tmp/zerodds/uds`. The hex string is the address in 32 lowercase
characters, without separators.

**Repo:** `crates/transport-uds/src/lib.rs` (`socket_path`,
`DEFAULT_BASE_DIR`, `parse_hex_id`, `source_locator` as the reverse
direction).

**Tests:** `crates/transport-uds/src/lib.rs::tests::socket_path_is_hex_under_base_dir`
(checks prefix and `.sock` suffix on an address with edge bytes set),
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_roundtrip`,
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_rejects_wrong_length`,
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_rejects_non_hex`.

**Status:** done

### §2.2 Base directory: lazy, user-private, TOCTOU-safe

**Spec:** §2 — the base directory is created on bind when it is missing
and then carries mode `0700`. An existing path that is a symlink or not a
directory MUST be refused rather than chmodded — otherwise a planted
symlink could set foreign paths to `0700`. No mode is set on an existing
foreign directory.

**Repo:** `crates/transport-uds/src/lib.rs` (`ensure_base_dir` —
`symlink_metadata` instead of `exists`, refusal of symlink and
non-directory, `create_dir_all` plus `set_permissions(0o700)` with a
re-check).

**Tests:** `crates/transport-uds/src/lib.rs::tests::ensure_base_dir_creates_dir_with_mode_0700`
(reads the mode back and compares it against `0o700`),
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_rejects_symlink`,
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_rejects_file_path`,
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_creates_missing_dir`.

**Status:** done

### §2.3 Abstract-namespace mode (Linux)

**Spec:** §2 — on Linux there is the alternative without a filesystem
inode: the socket name starts with a null byte, followed by
`<prefix>-<lowercase-hex32>`. No cleanup needed, no inode permissions, no
cross-mount volume. The name fits into `sun_path` (108 bytes, byte 0 is
the null prefix).

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport`, `UdsAddress::Abstract`,
`MAX_ABSTRACT_NAME = 107`, `UdsAddress::abstract_default`); the module is
declared in `crates/transport-uds/src/lib.rs` under
`#[cfg(target_os = "linux")]`.

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::abstract_bind_does_not_create_file`,
`crates/transport-uds/src/abstract_dgram.rs::tests::abstract_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::abstract_preserves_message_boundaries`,
`crates/transport-uds/src/abstract_dgram.rs::tests::local_locator_reflects_bind_id`.
These tests run **0 cases** on macOS: the module sits behind
`#[cfg(target_os = "linux")]` and is not compiled there. They were
measured on the Linux test host against the same, unchanged
`abstract_dgram.rs` (command `cargo test -p zerodds-transport-uds`) — 11
cases from `abstract_dgram::tests` green, 0 failed.

**Status:** done

### §2.4 Filesystem mode inside the abstract transport (A/B comparison)

**Spec:** §2 — the same transport MUST also do filesystem addressing so
abstract and filesystem mode can be measured against each other with
identical socket code. The difference is one config line.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAddress::Filesystem`, `UdsAddress::filesystem_default`, `fs_path`).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_bind_creates_socket_file`
(the socket file appears on bind and is gone after drop),
`crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_send_recv_roundtrip`.
These tests too are `#[cfg(target_os = "linux")]`-gated and run 0 cases
on macOS; measured on the Linux test host in the same run as §2.3.

**Status:** done

## §3 Wire format

### §3.1 `SOCK_DGRAM` with preserved message boundaries

**Spec:** §3 — one datagram message per RTPS frame. The kernel preserves
the message boundaries, unlike a TCP stream; no framing layer on top is
needed. Two sends MUST arrive as two receives with the original lengths.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport` over
`std::os::unix::net::UnixDatagram`, `Transport::send` with `send_to`,
`Transport::recv` with `recv_from`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::preserves_message_boundaries_two_datagrams`
(two sends of different lengths, two receives with exactly those
lengths),
`crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(the same delivery across a real process boundary).

**Status:** done

### §3.2 Sender locator from the socket address

**Spec:** §3 — the receiver MUST recover the locator from the sender
address so RTPS can answer. If the address is unnamed (an anonymous
client that did not bind) or unresolvable, `Locator::INVALID` MUST be
returned — not a guess.

**Repo:** `crates/transport-uds/src/lib.rs` (`source_locator`,
`parse_hex_id`, `Locator::INVALID`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`
(checks `got.source` against the sender's locator),
`crates/transport-uds/src/lib.rs::tests::preserves_message_boundaries_two_datagrams`,
`crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`.

**Status:** done

### §3.3 Datagram upper bound

**Spec:** §3 — `max_datagram` (default 65 536) bounds what is sent and
what is read into the receive buffer. A larger payload MUST be refused
with `PayloadTooLarge`; a silent truncation is not acceptable, because at
the RTPS level it would be indistinguishable from a short message.

**Repo:** `crates/transport-uds/src/lib.rs` (`DEFAULT_MAX_DATAGRAM`, the
length check in `Transport::send`, the buffer size in `Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_rejects_oversize_payload`
(a payload above the configured limit is refused with
`PayloadTooLarge`),
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`
(1 KiB arrives untruncated; at `max_datagram` either the delivery is
whole or the error names EMSGSIZE — and then nothing partial is in the
queue).

**Status:** done

### §3.4 Kernel bound versus configured bound

**Spec:** §3 — the real per-message bound is the kernel's: Linux via
`wmem_max` (default 212 992), macOS via `net.local.dgram.maxdgram`
(default 2048). The transport MUST reconcile the configured bound with
the operating system's, surface it at bind time or size the socket
buffers accordingly — otherwise a sample between the two bounds fails
only at runtime.

**Repo:** `crates/transport-uds/src/lib.rs` (`size_buffers`: send
buffer at twice, receive buffer at eight times `max_datagram`, called in
`UdsTransport::bind` and `UdsAbstractDgramTransport::bind`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::a_datagram_near_the_configured_limit_arrives_whole`
(four datagrams of `max_datagram` − 512 bytes arrive whole; measured on
macOS 26, where anything above 2048 bytes failed with EMSGSIZE before),
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`.

**Status:** done

## §4 Cleanup semantics

### §4.1 Filesystem mode: cleanup on drop, takeover on bind

**Spec:** §4 — bind creates the socket file, drop removes it best-effort
(an unmounted container volume must not make the drop fail). If a bind
finds a leftover file from a crashed predecessor, it MUST remove it and
rebind: `UnixDatagram::bind` cannot bind over an existing file and
otherwise reports `EADDRINUSE`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport::bind` with
`remove_file` on an existing path, `Drop for UdsTransport`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::bind_creates_socket_file`
(the file exists after bind and is gone after drop),
`crates/transport-uds/src/lib.rs::tests::bind_reuses_path_after_stale_leftover`
(a leftover file with foreign content does not block the bind).

**Status:** done

### §4.2 Abstract mode: no cleanup needed

**Spec:** §4 — the abstract namespace has no inode; Linux releases the
name once the last file descriptor closes. A crash-cleanup path is
therefore not required, and a bind must not create a file.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport::bind` in the `Abstract` branch without
filesystem access, `Drop` removing only in the `Filesystem` branch).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::abstract_bind_does_not_create_file`,
`crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_bind_creates_socket_file`
(the counter-check: in the filesystem branch the file appears and
disappears). Both `#[cfg(target_os = "linux")]`-gated, 0 cases on macOS;
measured on the Linux test host as in §2.3.

**Status:** done

### §4.3 Differentiated send-error classification

**Spec:** §4 — a send error MUST name its cause: a missing destination
socket, a denied permission, a full kernel buffer, a refused connection
and an exceeded message bound are different operational states. A
catch-all `Io` makes them indistinguishable.

**Repo:** `crates/transport-uds/src/lib.rs` (`classify_send_error` —
`NotFound`, `PermissionDenied`, `WouldBlock`, `ConnectionRefused`,
`EMSGSIZE` via `raw_os_error`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::classify_send_error_maps_kinds`
(every named `io::ErrorKind` value against its message),
`crates/transport-uds/src/lib.rs::tests::send_to_missing_peer_is_io_error`,
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`
(the EMSGSIZE branch, where the kernel refuses the message).

**Status:** done

### §4.4 Idle behaviour of `recv`

**Spec:** §4 — with `recv_timeout` set, an empty socket MUST yield
`RecvError::Timeout` after it elapses, not an Io error. Without a timeout
`recv` blocks.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::recv_timeout`,
`set_read_timeout` in bind, the `WouldBlock`/`TimedOut` branch in
`Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::recv_times_out_when_idle`,
`crates/transport-uds/src/abstract_dgram.rs::tests::recv_times_out_when_idle`
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

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::base_dir` as a
free path, `socket_path` below it; the drop cleanup is best-effort so an
unmounted volume does not make the drop fail).

**Tests:** `crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(two OS processes over a shared directory — the same topology as two
containers over a shared volume, without the container-runtime part),
`crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`.

**Status:** partial — the process boundary is shown, the container
boundary is not: no test starts two containers with a mounted volume and
actually exercises the UID-mapping and SELinux questions. The missing
evidence is carried as open item §5.2.

### §5.2 Evidence across a real container boundary

**Spec:** §5 — that the pattern of §5.1 holds under a container runtime
MUST be shown by a test with two containers and one mounted volume:
different UID mappings on both sides, the socket file on the volume, a
datagram in each direction.

**Repo:** —

**Tests:** —

**Status:** open

## §6 Platform support

### §6.1 Linux and macOS

**Spec:** §6 — Linux is the primary platform with both modes (filesystem
and abstract namespace), macOS supports the filesystem mode. The crate as
a whole is `#[cfg(unix)]`.

**Repo:** `crates/transport-uds/src/lib.rs` (`#![cfg(unix)]`,
`#[cfg(target_os = "linux")] pub mod abstract_dgram`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`
and `crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(measured on both platforms: macOS in this audit, Linux on the Linux test host),
`crates/transport-uds/src/abstract_dgram.rs::tests::abstract_send_recv_roundtrip`
(Linux only).

**Status:** done

### §6.2 Windows

**Spec:** §6 — Unix domain sockets are Unix-specific. On Windows the
crate compiles to an empty library; consumers gate their UDS usage with
`#[cfg(unix)]`. The obvious analogy would be named pipes — a different
protocol with different addressing, and not part of this spec.

**Repo:** —

**Tests:** —

**Status:** `open` — on Windows the crate compiles to an empty library
(`crates/transport-uds/src/lib.rs`: `#![cfg(unix)]`).
Windows offers sockets of the family `AF_UNIX` since Windows 10 (build 17063)
(Microsoft's statement, not checked here); the transport does not use them.

### §6.3 Cross-vendor interop

**Spec:** §6 — UDS is intra-container and intra-host IPC. Cross-vendor
interop with Cyclone DDS or FastDDS stays in the UDP, TCP and SHM
domains; neither vendor has a UDS transport a wire comparison could run
against. The section records the scope and states no implementation
requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

14 done / 1 partial / 2 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-transport-uds` — on macOS 15 (Rust host
`aarch64-apple-darwin`) 20 tests (lib) + 1 test
(`tests/l1_cross_process.rs`) green, 0 failed. The
`#[cfg(target_os = "linux")]`-gated `abstract_dgram` module runs 0 cases
there; the same command on the Linux test host against the unchanged
`abstract_dgram.rs` — 11 cases from `abstract_dgram::tests` green, 0
failed.

Open items and decision records: [`zerodds-uds-transport-1.0.open.en.md`](zerodds-uds-transport-1.0.open.en.md).
