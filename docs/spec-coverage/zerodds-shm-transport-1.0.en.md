# ZeroDDS SHM Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. For shared memory OMG
standardizes nothing beyond the vendor-reserved locator kind
(DDSI-RTPS 2.5 §9.4.5); segment layout, synchronization and the cleanup
protocol are vendor-specific. The normative requirements live in the
`##` sections §2 through §7; every `###` item below carries a spec
citation, a repo anchor, a test and a status.

Implementation: `crates/transport-shm/src/posix.rs` — segment layout,
SpSc ring buffer, crash recovery. The zero-copy loan path of §7 lives in
`crates/flatdata/` and `crates/zerodds-c-api/src/shm_loan_ffi.rs`; see
§7.2 for the delineation.

Open items and decision records: [`zerodds-shm-transport-1.0.open.en.md`](zerodds-shm-transport-1.0.open.en.md).

## §1 Scope and spec status

### §1.1 Vendor-reserved locator kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` to `0x01ffffff` are reserved
for vendor-specific locator kinds. ZeroDDS takes `0x01F0_0001` for shared memory.
The port field is meaningless for SHM and MUST be `0`; the 16 address
bytes carry the segment id.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Shm`,
`Locator::shm`, `Locator::SHM_ANY`); bound in
`crates/transport-shm/src/posix.rs` (`PosixShmTransport::local_locator`,
`peer_locator`, the kind check in `Transport::send`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::locator_kind_is_the_vendor_reserved_shm_value`
(asserts the numeric value `0x01F0_0001`, the position in the range `0x00000003` to `0x01ffffff` and
`port == 0`),
`crates/transport-shm/src/posix.rs::tests::local_locator_is_shm_with_own_id`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_non_shm_locator`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** done

### §1.2 What OMG does not standardize for shared memory

**Spec:** DDSI-RTPS 2.5 knows no SHM wire format, no segment layout, no
synchronization model and no cleanup protocol. Cyclone DDS integrates
iceoryx as a third-party component, FastDDS and RTI each have their own
SHM transports. The section records the scope and states no
implementation requirement.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 Segment layout

```text
  offset 0:   magic: u32 BE   "ZSHM"
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (data region, header excluded)
  offset 16:  head: AtomicU64          (next write offset, writer-owned)
  offset 24:  tail: AtomicU64          (next read offset, reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  reserved (padding to the 64-byte cache line)
  offset 64:  data region [capacity bytes]
```

### §2.1 Header fields

**Spec:** §2 — the creator MUST write the magic `"ZSHM"` big-endian at
offset 0, the layout version little-endian at offset 4 and the capacity
of the data region little-endian at offset 8. The header is 64 bytes long
so `head` and `tail` do not share a cache line with the data region.

**Repo:** `crates/transport-shm/src/posix.rs` (`SHM_MAGIC`,
`SHM_VERSION`, `HEADER_BYTES = 64`, initialization in the `is_creator`
branch of `PosixShmTransport::open`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::owner_writes_magic_and_version_into_the_header`
(reads magic, version and capacity straight out of the mapping and
compares the magic against `b"ZSHM"`),
`crates/transport-shm/src/posix.rs::tests::invalid_config_capacity_too_small_for_max_datagram`.

**Status:** done

### §2.2 Refusing foreign segments

**Spec:** §2 — on attach the opener MUST check magic and version and
refuse a foreign or newer segment instead of interpreting its bytes as a
ring. A layout change bumps the version.

**Repo:** `crates/transport-shm/src/posix.rs` (the `else` branch of
`PosixShmTransport::open`, `PosixShmError::InvalidHeader`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::attach_rejects_foreign_magic`
(a segment with a foreign magic, built through the same `shared_memory`
primitives, is refused with `InvalidHeader`),
`crates/transport-shm/src/posix.rs::tests::attach_rejects_unknown_version`
(`SHM_VERSION + 1` is refused).

**Status:** done

### §2.3 Memory ordering of `head` and `tail`

**Spec:** §2 — the writer MUST store `head` with `Release` after writing
the frame bytes; the reader MUST load `head` with `Acquire` before
reading them. A frame becomes visible exactly when the reader sees the
new `head` value — across process boundaries too.

**Repo:** `crates/transport-shm/src/posix.rs` (`SegmentLayout::head`,
`SegmentLayout::tail`, the `Ordering::Release` and `Ordering::Acquire`
sites in `push_frame` and `pop_frame`).

**Tests:** `crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`
(two OS processes, one segment; the receiver sees the sender's bytes),
`crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::many_frames_roundtrip_with_wraparound`.

**Status:** done

## §3 Frame format

Length-prefix format inside the data region:

```text
+---------+---------+---------+---------+----- ...
| len: u32 LE                           | bytes [len]
+---------+---------+---------+---------+----- ...
```

### §3.1 Length-prefix frame in the ring

**Spec:** §3 — every frame starts with its length as a `u32`
little-endian, followed by exactly that many bytes. `head` and `tail` are
monotonically growing counters; the ring position is `head % capacity`.
Occupancy is `head - tail`.

**Repo:** `crates/transport-shm/src/posix.rs`
(`SegmentLayout::write_u32`, `read_u32`, `push_frame`, `pop_frame`,
`occupied_bytes`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::occupied_bytes_reflects_send_state`,
`crates/transport-shm/src/posix.rs::tests::recv_after_send_drains_occupied_bytes`.

**Status:** done

### §3.2 Padding frame at the ring end

**Spec:** §3 — a frame is never split across the ring end. If the
contiguous space at the end does not suffice, the writer MUST place a
padding frame with `len = 0xFFFF_FFFE` and jump to the start; the reader
skips it and counts it for diagnosis. If fewer than 4 bytes remain, the
rest is skipped without a marker.

**Repo:** `crates/transport-shm/src/posix.rs` (`PADDING_FRAME_LEN`, the
wrap branch in `push_frame` with the padding and the skip of a rest under
4 bytes, the padding and `tail_space < 4` branches in `pop_frame`,
`padding_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::fill_ring_triggers_wraparound_padding`
(300-byte ring, five 60-byte frames — the wrap is forced and every frame
arrives unchanged),
`crates/transport-shm/src/posix.rs::tests::a_remainder_too_small_for_a_padding_header_is_skipped`
(a frame ends 2 bytes before the ring end; the next one goes through),
`crates/transport-shm/src/posix.rs::tests::frames_of_every_length_cross_the_ring_end_whole_and_in_order`
(5 000 rounds of lengths 1–200 bytes; without the skip of the rest the run
stopped in round 16 with "shm ring full near wraparound"),
`crates/transport-shm/src/posix.rs::tests::many_frames_roundtrip_with_wraparound`,
`crates/transport-shm/src/posix.rs::tests::corrupt_frame_counter_default_zero`.

**Status:** done

### §3.3 Length sanity check with a counter

**Spec:** §3 — the reader MUST discard a length above half the ring
instead of allocating the announced size, and MUST count the event. The
bound is the ring's, not the reader's own `max_datagram`: an owner that
configured a larger one sends valid frames up to it. A
silent drop is not acceptable: the counter is the ops signal for a broken
or hostile owner.

**Repo:** `crates/transport-shm/src/posix.rs` (the DoS branch in
`pop_frame`, `corrupt_frame_counter`, `corrupt_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::corrupt_length_is_dropped_and_counted`
(writes half the ring + 1 as the length header into the ring; `pop_frame`
returns nothing and `corrupt_frames_seen()` reads 1),
`crates/transport-shm/src/posix.rs::tests::corrupt_frame_counter_default_zero`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_oversize_payload`.

**Status:** done

## §4 Synchronization model

### §4.1 One segment per (owner, consumer) pair

**Spec:** §4 — a segment belongs to exactly one pair, not to one owner
with multi-reader fan-out. Lock-free SpSc scales linearly with the reader
count and has no global contention; SpmC as in iceoryx would slow the
writer down to the slowest reader. Price: N segments for N readers.

**Repo:** `crates/transport-shm/src/posix.rs` (`segment_os_id` and
`segment_flink` are functions of `(owner_id, consumer_id)`; `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_wrong_peer`
(a locator that is not this segment's peer is refused),
`crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`.

**Status:** done

### §4.2 Role separation of `send` and `recv`

**Spec:** §4 — the owner writes, the consumer reads. A `send` on the
consumer side and a `recv` on the owner side MUST fail with an error;
otherwise a second write direction would break the ring's SpSc
assumption.

**Repo:** `crates/transport-shm/src/posix.rs` (the role check in
`Transport::send` and `Transport::recv`, `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::consumer_cannot_send`,
`crates/transport-shm/src/posix.rs::tests::owner_cannot_recv`.

**Status:** done

### §4.3 Idempotent open regardless of role

**Spec:** §4 — the SEDP match order is not predictable: the reader side
may open before the writer side. Whoever comes first creates the segment;
whoever comes later attaches. The role determines only send/recv, not who
creates.

**Repo:** `crates/transport-shm/src/posix.rs` (`open` — flock, attach
attempt, otherwise create; `open_owner` and `open_consumer` share that
path).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_consumer_first_then_owner_roundtrip`,
`crates/transport-shm/src/posix.rs::tests::consumer_open_succeeds_without_owner_creates_segment`,
`crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`.

**Status:** done

### §4.4 Event-driven waiting instead of a poll loop

**Spec:** §4 — a waiting consumer MUST be woken by the writer rather than
spinning in a loop. The `head` value is read before `pop_frame` so a
frame published in between makes the park fail immediately (no lost
wakeup).

**Repo:** `crates/transport-shm/src/posix.rs` (`ring_park` / `ring_wake`
on the low 32 bits of `head`, called from `wait_parked` and
`push_frame`: `ring_futex` on Linux, `ring_os_sync` on macOS from 14.4
with `os_sync_wait_on_address` and `OS_SYNC_WAIT_ON_ADDRESS_SHARED`,
resolved at run time through `dlsym`; `wait_polling` otherwise).

**Tests:** `crates/transport-shm/src/posix.rs::tests::the_consumer_parks_instead_of_polling`
(Linux and macOS),
`crates/transport-shm/src/posix.rs::tests::owner_drop_signals_consumer_via_shutdown_flag`
(the waiting consumer returns in under 200 ms although `recv_timeout` is
10 s),
`crates/transport-shm/src/posix.rs::tests::interrupt_recv_returns_a_waiting_recv_at_once`,
`crates/transport-shm/src/posix.rs::tests::recv_times_out_when_idle`,
`crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings`.
Measured on macOS 26 (a Mac under load, load average 97), three runs:
100 waiting consumers 0.3 % of a core; wake latency p50 22 to 24 µs
against 63 to 69 µs over UDP loopback in the same run. With the
sleep-poll a measurement on the same Mac gave 4.45 % of a core per
consumer and p50 177 µs.

Big-endian Linux parks on the same low 32 bits of `head`, at byte 20
there (`head_word`); green on s390x under qemu-user (every
`transport-shm` test, Linux test host). Windows waits on a named auto-reset event
per segment (`RingEvent`) the owner sets after every frame; in nightly
6555 (`windows-smoke`, `c704c3ccfaa`)
`interrupt_recv_returns_a_waiting_recv_at_once` and
`bell::tests::a_ring_wakes_the_parked_receiver_and_none_is_lost` ran
green on Windows.

**Status:** partial — macOS before 14.4 waits by sleep-poll.

### §4.5 Backpressure on a full ring

**Spec:** §4 — if the ring is full because the reader cannot keep up,
`send` MUST return a named error after a bounded spin phase. Blocking
forever and discarding quietly are both unacceptable.

**Repo:** `crates/transport-shm/src/posix.rs` (`SPIN_LIMIT = 1024`, the
two `SendError::Io` branches in `push_frame`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::full_ring_returns_an_error_instead_of_dropping`
(512-byte ring, 128-byte frames, no consumer drain; the error text names
"ring full"),
`crates/transport-shm/src/posix.rs::tests::send_rejects_oversize_payload`.

**Status:** done

## §5 Cleanup semantics

### §5.1 Predictable `os_id`

**Spec:** §5 — the segment name MUST be a pure function of the pair so a
zombie segment can be found again after a crash. The name starts with `/`
and stays under the platform limit (macOS `PSHMNAMLEN` = 31); the
direction `(A,B)` differs from `(B,A)`. The name MUST depend on every
byte of both ids: where it has to be shorter than the ids in hex (macOS),
it carries a fixed hash over both. A slice of the ids is not enough, since
the entity ids repeat between participants: every participant numbers its
endpoints from the same start.

**Repo:** `crates/transport-shm/src/posix.rs` (`segment_os_id`,
`segment_os_id_in`, `fnv1a_128`, `segment_flink`, `DEFAULT_FLINK_DIR`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::segment_os_id_is_deterministic_and_within_platform_limit`
(the same pair yields the same name, the reversed pair a different one,
length under 31 on macOS),
`crates/transport-shm/src/posix.rs::tests::short_names_of_pairs_that_differ_only_in_the_prefix_differ`
(the short form, on every platform: two pairs with the same entity ids and
other participants get different names),
`crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §5.2 Crash recovery

**Spec:** §5 — before creating a segment a zombie of the same name MUST
be removed (`shm_unlink`, idempotent — `ENOENT` is ignored), otherwise
`/dev/shm` leaks on the Nth restart. A leftover flink file must not make
the open fail.

**Repo:** `crates/transport-shm/src/posix.rs` (`shm_unlink_by_os_id`, the
create branch in `open` with a preceding `remove_file` on the flink).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_recovers_from_stale_flink_of_crashed_owner`
(a crashed owner's flink file points at a segment that no longer exists;
`open_owner` cleans up and a frame passes through afterwards),
`crates/transport-shm/src/posix.rs::tests::consumer_open_succeeds_without_owner_creates_segment`.

**Status:** done

### §5.3 Shutdown flag

**Spec:** §5 — the owner MUST set `shutdown = 1` on drop (release store).
The consumer checks the flag after every empty poll and returns a
targeted error instead of waiting out `recv_timeout`. Frames published
before the drop still arrive — the check sits behind `pop_frame`.

**Repo:** `crates/transport-shm/src/posix.rs` (`SegmentLayout::shutdown`,
`Drop for PosixShmTransport`, the shutdown check in `wait_for_frame`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::owner_drop_signals_consumer_via_shutdown_flag`
(measures the return time against a 10 s timeout and checks the error
text for "owner"),
`crates/transport-shm/src/posix.rs::tests::posix_shm_error_display_covers_all_variants`.

**Status:** done

### §5.4 Race protection on create

**Spec:** §5 — concurrent openers of the same `os_id` MUST be serialized
across threads and processes, otherwise one removes the segment the other
just created. The lock is released automatically when the handle closes
or the process dies (the same crash resilience as the segment itself).

**Repo:** `crates/transport-shm/src/posix.rs` (`acquire_flock_excl` —
`flock(LOCK_EX)` on Unix, `LockFileEx` with `LOCKFILE_EXCLUSIVE_LOCK` on
Windows —, `FlockGuard`, the sentinel file `<flink>.lock`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`
(two threads open the same `os_id` concurrently; both end up validly
bound),
`crates/transport-shm/src/posix.rs::tests::users_in_different_flink_directories_share_the_segment`
(owner and consumer look the segment up in different flink directories,
under another `TMPDIR` say; the second attaches to the held segment
instead of removing it and making its own).

**Status:** done

### §5.5 Liveness lock and orphaned segments

**Spec:** §5 — every user of a segment, owner and consumer alike, MUST
hold a shared `flock` on the segment for as long as it uses it. The kernel
releases it when the process ends, also by `SIGKILL`; a segment without a
lock holder is orphaned. An opener that finds an orphaned segment under
its pair's name MUST remove it and create it anew instead of taking it
over: its ring holds the frames of the ended session. Orphaned segments
nobody opens again are removed by a sweep over `/dev/shm`. It leaves
alone segments younger than a minimum age (which covers the gap between
the creation and the creator's lock) and segments of another layout
version. Version 2 marks this protocol: a version-1 user holds no lock,
so its live segments would look orphaned.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment`,
`SegmentHold`, `sweep_orphaned_segments`, `SweepReport`,
`sweep_stale_pair_files` for the `.lock` files and, on Linux, the flinks
of pairs whose segment is gone — they stayed after every process that did
not end cleanly (a test chain on a MacBook Air: 3 504 `.lock` files in one domain
directory); `lock_pair` checks after locking that the path still names
the locked file, `SHM_VERSION = 2`; a ring with a blank header, whose creator ended before
it wrote the header, counts as removable, since no attacher accepts it),
`crates/transport-shm/src/bell.rs` (a participant's doorbell: a shared
`flock` on its file in `/dev/shm`, `sweep_orphaned_doorbells`);
`crates/dcps/src/same_host_shm.rs` (`sweep_orphaned_segments_once`: once
per process when a participant with `transport.same_host_shm` starts,
rings and doorbells, then per flink directory the pair files, minimum age
60 s, event `shm.orphaned_segments_removed`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::an_orphaned_segment_is_made_anew_not_attached`
(a process of its own opens the owner, publishes a frame and ends without
a drop; the consumer then finds an empty ring and receives only the frame
of the new session),
`crates/transport-shm/src/posix.rs::tests::the_sweep_removes_orphaned_segments_and_keeps_held_ones`
(the orphaned segment stays below the minimum age and is removed above
it; a held segment stays, and a frame passes afterwards; a segment with a
blank header is removed),
`crates/transport-shm/src/bell.rs::tests::the_sweep_removes_orphaned_bells_and_keeps_held_ones`,
`crates/transport-shm/src/posix.rs::tests::the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::a_lock_taken_on_a_removed_lock_file_is_taken_anew`.
Measured on the Linux test host: two participants ended by `kill -9`; the start of a
third after 60 s removed their 2 rings and 2 doorbells (`removed: 4`).
Before, the doorbell of every process that did not end cleanly stayed
(138 after a day of probes).

On macOS and the other Unixes without shared memory in the file system
the lock is on the pair's flink, and `sweep_orphaned_flinks` finds
orphaned segments through the flink directory (dcps: once per directory,
that is per host and domain). A doorbell leaves a marker `<name>.bell`
there on which its participant holds the lock; the sweep removes bell and
marker when nobody holds it. Tests
`the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones` and
`bell.rs::tests::the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`,
green on macOS 26; `an_orphaned_segment_is_made_anew_not_attached` runs on every
Unix. Windows: §6.3.

**Status:** done

## §6 Platform support

### §6.1 Linux and macOS

**Spec:** §6 — Linux is the primary platform, macOS is supported. On
macOS the `PSHMNAMLEN` limit of 31 characters applies to the segment
name; the name is truncated accordingly there.

**Repo:** `crates/transport-shm/src/posix.rs`
(the `#[cfg(target_os = "macos")]` branch in `segment_os_id`,
`#[cfg(any(target_os = "linux", target_os = "macos"))]` for
`shm_unlink_by_os_id`, `#[cfg(unix)]` for `acquire_flock_excl`),
`crates/transport-shm/src/mapping.rs` (a mapping holds no descriptor:
`shm_open`, `ftruncate`, `mmap`, then `close`. With `shared_memory` one
stayed open per mapping, two rings and a doorbell per peer; a process of
a multi-process test chain ran out at macOS' default limit of 256 descriptors, 62 of
them shared-memory objects. What stays per peer is the liveness lock of
each of the two rings).

**Tests:** `crates/transport-shm/src/mapping.rs::tests::a_mapping_holds_no_file_descriptor`
(100 mappings, the number of open descriptors unchanged in a process of
its own; macOS 26),
`crates/transport-shm/src/mapping.rs::tests::create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/posix.rs::tests::segment_os_id_is_deterministic_and_within_platform_limit`
(checks the 31-character limit under `cfg(target_os = "macos")`),
`crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`,
`crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §6.2 Windows

**Spec:** §6 — on Windows `CreateFileMapping` supplies the mappings, the
create race is serialized via `LockFileEx`. The transport MUST run there
with the same test suite as on Unix.

**Repo:** `crates/transport-shm/src/posix.rs` (the `#[cfg(windows)]`
variant of `acquire_flock_excl` with
`LockFileEx`/`LOCKFILE_EXCLUSIVE_LOCK`, the
`#[cfg(not(any(target_os = "linux", target_os = "macos")))]` no-op of
`shm_unlink_by_os_id`); the CI job `windows-smoke` in `.gitlab-ci.yml`
calls `cargo test -p zerodds-transport-shm` on the Windows runner.

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`
(the test that drives the `LockFileEx` path on Windows),
`crates/transport-shm/src/posix.rs::tests::open_recovers_from_stale_flink_of_crashed_owner`.

**Status:** partial — in nightly 6555 (`windows-smoke`, `c704c3ccfaa`)
39 of 40 `transport-shm` lib tests ran green on Windows; the failing one
is §6.3.

### §6.3 Evidence for the Windows cleanup semantics

**Spec:** §6 — that a crashed owner leaves no mapping with its session's
frames on Windows MUST be shown by a test on the Windows runner, not
merely asserted by the documentation.

**Repo:** `crates/transport-shm/src/posix.rs` (the `#[cfg(windows)]`
variant of `hold_segment` with `LockFileEx` on the flink, as the macOS
lock; `discard_orphaned_segment` removes the backing file). The earlier
assumption that Windows releases the mapping with its last handle does
not hold for `shared_memory`: it puts every mapping on a file
`%TEMP%\shared_memory-rs\<name>` that outlives the process (the
crate's file windows.rs, function `new_map`). Measured in nightly 6555: the test found 29 bytes of the
ended session in the ring.

**Tests:** `crates/transport-shm/src/posix.rs::tests::an_orphaned_segment_is_made_anew_not_attached`
(a child process opens the owner, publishes a frame and ends without a
drop; the same name then opens with an empty ring). Runs on every
platform, on Windows in the `windows-smoke` job.

**Status:** partial — the lock is built and clippy-checked for
`x86_64-pc-windows-gnu`; it has not run, the next `windows-smoke` run
measures it.

### §6.4 `no_std`

**Spec:** §6 — the SHM transport is `std`-only. `shm_open`, `mmap`,
`flock` and the flink files are OS calls; a `no_std` target without an
operating system has no equivalent. The crate is therefore gated behind
`#[cfg(feature = "std")]`.

**Repo:** —

**Tests:** —

**Status:** `open` — the transport runs with `std` only
(`crates/transport-shm/src/lib.rs`: `#[cfg(feature = "std")] pub mod posix`):
`shm_open`, `mmap` and `flock` are POSIX calls. A target without POSIX would need a slot store
over the memory its platform shares between processes or cores; there is
none.

## §7 Zero-copy loan (same host)

### §7.1 Loan path writer → reader

**Spec:** §7 — for samples on the same host a path MUST exist that avoids
a staging copy: the writer receives a pointer **into a shared-memory
slot**, fills it, and the commit finalizes the slot in place. A reader on
the same host maps the same segment and reads the slot through its own
pointer — the bytes never cross the network and are never re-copied.

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator`,
`reserve_slot`, `slot_data_ptr`, `commit_in_place`, `slot_read_ptr`,
`next_unread_slot`, `mark_read`);
`crates/zerodds-c-api/src/shm_loan_ffi.rs` (`try_loan`, `try_commit`,
`try_take`, `zerodds_dw_enable_shm_loan`, `zerodds_dr_enable_shm`,
`zerodds_dr_take_shm`, `zerodds_dr_release_shm`).

**Tests:** `crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`
(measures the loan path itself, not its setup: the test writes the
payload straight into the loaned pointer, commits in place and asserts on
the reader side with `assert_ne!` that its read pointer carries a
different address than the writer pointer — its own mapping of the same
segment — and that the bytes are equal; then release, `NoData` on the
next take and a second sample through the same ring),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_runtime_path_writer_to_reader_zero_copy`
(the same path over the runtime FFI the ROS 2 RMW uses),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::loan_without_shm_falls_back_to_heap`
(without an enabled loan the heap path stays),
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`
(slot backend without the FFI). The `flatdata-loan` feature is in
`default` of `crates/zerodds-c-api/Cargo.toml`; the tests run in the
default run, not behind a switch.

**Status:** done

### §7.2 Delineation: `crates/transport-shm` copies

**Spec:** §7 — the ring of §2 through §5 is **not** a zero-copy path:
`push_frame` copies the payload into the ring region, `pop_frame` copies
it out into a `Vec`. The ring carries RTPS datagrams between processes;
the loan path of §7.1 carries sample bytes without a copy. The two
mechanisms are separate and are not substituted for each other. The
section records the delineation and states no implementation requirement
of its own.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

18 done / 3 partial / 1 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-transport-shm` — 27 tests (lib) + 1 test
(`tests/l1_cross_process.rs`) green, 0 failed;
`cargo test -p zerodds-c-api --test shm_loan_e2e` — 5 tests green, 0
failed. Measured on macOS 15 (Rust host `aarch64-apple-darwin`); the
Windows run of §6.2 and the Linux futex path of §4.4 are not included.

Open items and decision records: [`zerodds-shm-transport-1.0.open.en.md`](zerodds-shm-transport-1.0.open.en.md).
