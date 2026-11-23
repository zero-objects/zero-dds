# ZeroDDS SHM Transport 1.0 — Spec Coverage

**Source:** this document is also the spec. For shared memory OMG
standardizes nothing beyond the vendor-reserved locator kind
(DDSI-RTPS 2.5 §9.4.5); segment layout, synchronization and the cleanup
protocol are vendor-specific. The normative requirements live in the
`##` sections §2 through §7; every `###` item below carries a spec
citation, a repo anchor, a test and a status.

Implementation: `crates/transport-shm/src/ring.rs` — segment layout and
SpSc ring buffer in `core`; `crates/transport-shm/src/posix.rs` — named
segments, locks, crash recovery; `crates/transport-shm/src/region.rs` —
the transport over an integrator's region without an operating system
(§6.4). The zero-copy loan path of §7 lives in
`crates/flatdata/` and `crates/zerodds-c-api/src/shm_loan_ffi.rs`; see
§7.2 for the delineation.

Open items: `zerodds-shm-transport-1.0.open.en.md`.

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

**Tests:** `crates/transport-shm/src/posix.rs::tests::rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_shm_value`
(asserts the numeric value `0x01F0_0001`, the position in the range `0x00000003` to `0x01ffffff` and
`port == 0`),
`crates/transport-shm/src/posix.rs::tests::shm_1_1_local_locator_is_shm_with_own_id`,
`crates/transport-shm/src/posix.rs::tests::shm_1_1_send_rejects_non_shm_locator`,
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

**Status:** n/a (informative) — scope statement; the requirements are the items from §2 on.

## §2 Segment layout

```text
  offset 0:   magic: u32 BE   "ZSHM"
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (data region, header excluded)
  offset 16:  head: AtomicU64          (next write offset, writer-owned)
  offset 24:  tail: AtomicU64          (next read offset, reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  consumer_attached: AtomicU32 (consumers that have the segment open)
  offset 40:  owner_epoch: AtomicU32   (every owner that opens it counts up)
  offset 44:  reserved
  offset 48:  session_head: u64        (§6.4; 0 in a named segment)
  offset 56:  reserved (padding to the 64-byte cache line)
  offset 64:  data region [capacity bytes]
```

### §2.1 Header fields

**Spec:** §2 — the creator MUST write the magic `"ZSHM"` big-endian at
offset 0, the layout version little-endian at offset 4 and the capacity
of the data region little-endian at offset 8. The header is 64 bytes long
so `head` and `tail` do not share a cache line with the data region. At
offset 36 `consumer_attached` counts the consumer handles that have the
segment open: every consumer adds 1 when it opens and subtracts 1 on its
drop; the owner reads from it whether a consumer is attached
(`peer_attached`). At offset 40 every owner that opens the segment adds 1
to `owner_epoch` and keeps the new value; which owner ends the segment on
its drop is §5.3's rule. Both fields are `u32` in the host's byte order.
Offset 48 holds `session_head` (`u64`), which only an owner over an
integrator's region writes (§6.4); in a named segment it stays zero, as do
offsets 44 to 48 and 56 to 64. The capacity lies between 32 bytes and 2³¹
bytes: each side reads the other's position only in its low 32 bits
(§6.4). An attacher takes the capacity from offset 8
instead of its own configuration and lowers its `max_datagram` to
`(capacity - 16) / 2` when it is larger; the configuration requires
`capacity >= 2 * max_datagram + 16`. Both fields came before the first
release of version 2 and belong to every version from 2 on; the current
version 3 marks where the locks lie (§5.4).

**Repo:** `crates/transport-shm/src/ring.rs` (`SHM_MAGIC`,
`SHM_VERSION`, `HEADER_BYTES = 64`, `MAX_CAPACITY`,
`Ring::create_in_zeroed`, `Ring::attach`, `Ring::consumer_attached`,
`Ring::owner_epoch`), `crates/transport-shm/src/posix.rs`
(initialization in the `is_creator` branch of `PosixShmTransport::open`,
the capacity taken in the attacher branch, `peer_attached`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_1_owner_writes_magic_and_version_into_the_header`
(reads magic, version and capacity straight out of the mapping and
compares the magic against `b"ZSHM"`),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_attach_and_epoch_words_sit_at_offsets_36_and_40`
(reads both words raw from the mapping: one owner gives epoch 1, two
consumer handles give 2, a second owner epoch 2, offsets 44 to 64 are
zero),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_the_owner_sees_the_consumer_attach_and_leave`,
`crates/transport-shm/src/posix.rs::tests::shm_2_1_sides_with_different_ring_sizes_share_the_creators_ring`
(owner and consumer with different ring sizes share the creator's ring,
in both orders),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_an_attacher_lowers_its_max_datagram_to_the_creators_ring`
(a ring of 4096 bytes, the attacher with `max_datagram` 32 752: as owner
it reports `max_message_size` 2040, refuses 2041 bytes with
`PayloadTooLarge` and sends 2040 bytes; as consumer it reports 2040 as
well),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_invalid_config_capacity_too_small_for_max_datagram`.

**Status:** done

### §2.2 Refusing foreign segments

**Spec:** §2 — on attach the opener MUST check magic and version and
refuse a foreign or newer segment instead of interpreting its bytes as a
ring. A layout change bumps the version.

**Repo:** `crates/transport-shm/src/posix.rs` (the `else` branch of
`PosixShmTransport::open`, `PosixShmError::InvalidHeader`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_2_attach_rejects_foreign_magic`
(a segment with a foreign magic, built through the same `shared_memory`
primitives, is refused with `InvalidHeader`),
`crates/transport-shm/src/posix.rs::tests::shm_2_2_attach_rejects_unknown_version`
(`SHM_VERSION + 1` is refused).

**Status:** done

### §2.3 Memory ordering of `head` and `tail`

**Spec:** §2 — the writer MUST store `head` with `Release` after writing
the frame bytes; the reader MUST load `head` with `Acquire` before
reading them. A frame becomes visible exactly when the reader sees the
new `head` value — across process boundaries too.

**Repo:** `crates/transport-shm/src/ring.rs` (`store_position`,
`load_position`, `low`, the `Ordering::Release` and `Ordering::Acquire`
sites in `Ring::try_push` and `Ring::next`; without 64-bit atomics the
module `split`).

**Tests:** `crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`
(two OS processes, one segment; the receiver sees the sender's bytes),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::shm_2_3_many_frames_roundtrip_with_wraparound`.

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

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::write_u32`,
`Ring::read_u32`, `Ring::try_push`, `Ring::next`, `Ring::occupied`),
`crates/transport-shm/src/posix.rs` (`push_frame`, `pop_frame`,
`occupied_bytes`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_occupied_bytes_reflects_send_state`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_recv_after_send_drains_occupied_bytes`.

**Status:** done

### §3.2 Padding frame at the ring end

**Spec:** §3 — a frame is never split across the ring end. If the
contiguous space at the end does not suffice, the writer MUST place a
padding frame with `len = 0xFFFF_FFFE` and jump to the start; the reader
skips it and counts it for diagnosis. If fewer than 4 bytes remain, the
rest is skipped without a marker.

**Repo:** `crates/transport-shm/src/ring.rs` (`PADDING_FRAME_LEN`, the
wrap branch in `Ring::try_push` with the padding and the skip of a rest
under 4 bytes, the padding and `tail_space < 4` branches in `Ring::next`),
`crates/transport-shm/src/posix.rs` (`padding_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_2_fill_ring_triggers_wraparound_padding`
(300-byte ring, five 60-byte frames — the wrap is forced and every frame
arrives unchanged; before the fifth is read, offset 256 holds the marker
`0xFFFF_FFFE`, and after it the consumer counts exactly one padding
frame),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_a_padding_frame_is_counted_once_per_wrap`
(201 frames of 64 bytes in a 300-byte ring cross the ring end 50 times;
`padding_frames_seen()` then reads 50),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_a_remainder_too_small_for_a_padding_header_is_skipped`
(a frame ends 2 bytes before the ring end; the next one goes through),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_frames_of_every_length_cross_the_ring_end_whole_and_in_order`
(5 000 rounds of lengths 1–200 bytes; without the skip of the rest the run
stopped in round 16 with "shm ring full near wraparound"),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_many_frames_roundtrip_with_wraparound`,
`crates/transport-shm/src/posix.rs::tests::shm_3_2_corrupt_frame_counter_default_zero`.

**Status:** done

### §3.3 Length sanity check with a counter

**Spec:** §3 — the reader MUST discard a length above half the ring
instead of allocating the announced size, and MUST count the event. The
bound is the ring's, not the reader's own `max_datagram`: an owner that
configured a larger one sends valid frames up to it. In the same way it
discards and counts a frame whose `4 + len` reaches past the ring end or
past the published `head`, a padding marker or a remainder under 4 bytes
that would skip past `head`, and a `head` more than one ring ahead of
`tail` or behind it. No byte outside the data region is read or written;
the check holds in every build. After the drop the reader sets `tail` to
`head`: behind a wrong length the next frame boundary is unknown, and
what the owner published up to `head` is lost. A waiting `recv` then
returns `Io` ("shm corrupt frame dropped") instead of waiting on. A
silent drop is not acceptable: the counter is the ops signal for a broken
or hostile owner, and the DCPS receive paths report every drop as the
event `shm.corrupt_frame` (level error, attributes `peer` and `count`).

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::next` with the
bounds, `Ring::drop_published`, `Ring::in_ring` for every access to the
data region), `crates/transport-shm/src/posix.rs` (`pop_frame`,
`corrupt_frame_counter`, `corrupt_frames_seen`, `CORRUPT_FRAME_DROPPED`
in `wait_parked`);
`crates/dcps/src/same_host_channel.rs` (`SameHostChannels::receive`,
`corrupt_frames`, called in the receive loop of the same-host channels in
`crates/dcps/src/runtime.rs`); `crates/dcps/src/shm_user.rs`
(`report_corrupt_frames` in each peer's receive thread,
`ShmUserTransport::with_observability`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_3_corrupt_length_is_dropped_and_counted`
(writes half the ring + 1 as the length header into the ring; `pop_frame`
returns nothing and `corrupt_frames_seen()` reads 1),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_a_length_past_the_ring_end_is_dropped_and_counted`
(length 200, 100 bytes before the ring end; before, the reader read past
the mapping in a release build),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_a_length_past_head_is_dropped_and_counted`
(length 100 with 8 bytes published; before, the reader handed up
unpublished bytes and moved `tail` past `head`),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_skips_and_heads_beyond_what_was_published_are_dropped`,
`crates/transport-shm/src/posix.rs::tests::shm_3_3_the_ring_carries_frames_again_after_a_corrupt_one`
(the waiting `recv` reports the drop, it counts once, then 200 frames
pass),
`crates/dcps/src/same_host_channel.rs::tests::shm_3_3_a_corrupt_frame_is_reported_as_an_event`,
`crates/dcps/src/shm_user.rs::tests::shm_3_3_a_corrupt_frame_on_the_user_transport_is_reported_as_an_event`
(both write the wrong length through the segment's file in `/dev/shm`, as
another process would; Linux only),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_corrupt_frame_counter_default_zero`,
`crates/transport-shm/src/posix.rs::tests::shm_3_3_send_rejects_oversize_payload`.

**Status:** done

## §4 Synchronization model

### §4.1 One segment per (owner, consumer) pair

**Spec:** §4 — a segment belongs to exactly one pair, not to one owner
with multi-reader fan-out. Lock-free SpSc scales linearly with the reader
count and has no global contention; SpmC as in iceoryx would slow the
writer down to the slowest reader. Price: N segments for N readers.

**Repo:** `crates/transport-shm/src/posix.rs` (`segment_os_id` and
`segment_flink` are functions of `(owner_id, consumer_id)`; `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::shm_4_1_send_rejects_wrong_peer`
(a locator that is not this segment's peer is refused),
`crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`.

**Status:** done

### §4.2 Role separation of `send` and `recv`

**Spec:** §4 — the owner writes, the consumer reads. A `send` on the
consumer side and a `recv` on the owner side MUST fail with an error;
otherwise a second write direction would break the ring's SpSc
assumption.

**Repo:** `crates/transport-shm/src/posix.rs` (the role check in
`Transport::send` and `Transport::recv`, `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_2_consumer_cannot_send`,
`crates/transport-shm/src/posix.rs::tests::shm_4_2_owner_cannot_recv`.

**Status:** done

### §4.3 Idempotent open regardless of role

**Spec:** §4 — the SEDP match order is not predictable: the reader side
may open before the writer side. Whoever comes first creates the segment;
whoever comes later attaches. The role determines only send/recv, not who
creates. The SHM user transport of the DCPS runtime opens both segments of
a peer when SPDP reports it, not at its own first send to the peer: a
best-effort reader sends nothing (DDSI-RTPS 2.5 §8.4.2.3).

**Repo:** `crates/transport-shm/src/posix.rs` (`open` — flock, attach
attempt, otherwise create; `open_owner` and `open_consumer` share that
path); `crates/dcps/src/shm_user.rs` (`add_destination`),
`crates/dcps/src/runtime.rs` (`note_user_transport_peer`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_3_open_consumer_first_then_owner_roundtrip`,
`crates/transport-shm/src/posix.rs::tests::shm_4_3_consumer_open_succeeds_without_owner_creates_segment`,
`crates/transport-shm/src/posix.rs::tests::shm_4_3_a_consumer_that_made_the_segment_leaves_it_to_the_owner`
(a consumer that created the segment before the owner does not remove it
on its drop; a consumer that opens again finds the owner's ring),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/dcps/tests/shm_user_transport.rs::shm_4_3_a_best_effort_reader_takes_what_a_process_it_never_sent_to_writes`.

**Status:** done

### §4.4 Event-driven waiting instead of a poll loop

**Spec:** §4 — a waiting consumer MUST be woken by the writer rather than
spinning in a loop. The `head` value is read before `pop_frame` so a
frame published in between makes the park fail immediately (no lost
wakeup). This holds on every platform: where there is no wait on a
shared address, the consumer waits on a FIFO per segment, `<name>.wake`
in the pair directory (§5.4), into which the writer writes one byte after
each frame; a byte written between the look and the park stays in the
pipe. A participant's doorbell waits the same way. Both sides of a
segment run on one host and use the same way: a futex on Linux,
`os_sync_wait_on_address` on macOS from 14.4, `__ulock_wait` with
`UL_COMPARE_AND_WAIT_SHARED` on macOS from 10.15, the FIFO on the other
Unixes and on an older macOS, a named auto-reset event on Windows. The
owner that ends a segment wakes the waiting consumer and removes the
FIFO.

**Repo:** `crates/transport-shm/src/wait.rs` (`Backend`, `Waiter` with
`open`, `park`, `wake`, `remove_files`; the backends `futex`, `os_sync`
and `ulock`, both resolved at run time through `dlsym`, `ulock` only from
Darwin 19, `Fifo` and `Event`); `crates/transport-shm/src/posix.rs`
(`wait_parked` and `wake_ring` on the low 32 bits of `head`, the wake in
`push_frame` and in the owner's drop, the waiter opened under the pair
lock); `crates/transport-shm/src/bell.rs` (`ShmDoorbell::park` and
`ring` through the same `Waiter`, the FIFO made under the bell's pair
lock); `sweep_pair_dir` removes a FIFO whose segment is gone
(`is_pair_file_type`: `.wake` as a FIFO, `.lock` and `.hold` as a
regular file; before, the sweep took regular files only and left every
FIFO that processes ended without their drop had left).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_4_the_consumer_parks_instead_of_polling`
(on every backend of the host: an idle consumer with a 300 ms timeout
parks at most 3 times; the sleep-poll looked at the ring a few hundred
times),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_a_push_wakes_a_parked_consumer`
(on every backend: the consumer has parked, the push wakes it in under
5 s against a 20 s timeout),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_no_wakeup_is_lost_between_the_look_and_the_park`
(on every backend 20 000 rounds in which the writer sends the next frame
only after the consumer took the last; no `recv` takes 1.5 s, a lost wake
would cost its 3 s timeout),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_wakes_a_waiting_consumer`
(on every backend: the owner's drop wakes the parked consumer, which
returns the owner-gone error in under 5 s against a 20 s timeout),
`crates/transport-shm/tests/l1_cross_process.rs::shm_4_4_l1_a_push_wakes_a_consumer_in_another_process`
(the owner in a child process pushes 300 ms after the consumer of this
process parked; the consumer wakes in under 5 s against a 20 s timeout),
`crates/transport-shm/src/bell.rs::tests::shm_4_4_a_ring_wakes_the_parked_receiver_and_none_is_lost`
(on every backend; an idle park lasts its 200 ms),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_signals_consumer_via_shutdown_flag`
(the owner is dropped before the `recv`; it returns in under 200 ms
although `recv_timeout` is 10 s),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_interrupt_recv_returns_a_waiting_recv_at_once`,
`crates/transport-shm/src/posix.rs::tests::shm_4_4_interrupt_recv_reaches_a_recv_about_to_park`,
`crates/transport-shm/src/posix.rs::tests::shm_4_4_recv_times_out_when_idle`,
`crates/flatdata/src/posix.rs::tests::flatdata_4_1_futex_notify_wakes_consumer_across_mappings`.
"Every backend of the host": the tests force each backend the host has
in turn (Linux: futex and FIFO; macOS 26: os_sync, ulock and FIFO;
Windows: the event).
Measured on macOS 26 before the ulock and FIFO backends (a Mac under
load, load average 97), three runs: 100 waiting consumers 0.3 % of a
core; wake latency p50 22 to 24 µs against 63 to 69 µs over UDP loopback
in the same run. With the sleep-poll a measurement on the same Mac gave
4.45 % of a core per consumer and p50 177 µs.

Big-endian Linux parks on the same low 32 bits of `head`, at byte 20
there (`head_word`); green on s390x under qemu-user (every
`transport-shm` test at `8f94dbd7d6e`, Linux test host; that was before
`wait.rs` and before the tests per backend). Windows waits on a named auto-reset event
per segment (`Event` in `wait.rs`, before `RingEvent`) the owner sets after every frame; in nightly
6555 (`windows-smoke`, `81d8500d495`)
`shm_4_4_interrupt_recv_returns_a_waiting_recv_at_once` and
`crates/transport-shm/src/bell.rs::tests::shm_4_4_a_ring_wakes_the_parked_receiver_and_none_is_lost` ran
green on Windows; that was the code before `3861278cf5e`, which moved the
event into `wait.rs` and wrote the doorbell test anew per backend.

**Status:** partial — built; measured on the Linux test host with the
futex and the FIFO (`cargo nextest run --profile merge -p
zerodds-transport-shm --all-targets`). The macOS backends (os_sync,
ulock, the FIFO on macOS) ran green on a Mac (macOS 26.2, arm64: the
same command, 92 tests, in the sandbox of `ci/macos-loopback.sb`); the
job `merge-macos-transport-shm` on runner 8 failed in pipeline 9916 on
the pair directory (§5.4), its green run there is still to come. A
macOS between 10.15 and 14.3, on which ulock is the native backend, has
not been measured; on macOS 26 the tests force it. The Windows event
backend and the tests written anew per backend have since `3861278cf5e`
been compiled with Clippy for `x86_64-pc-windows-gnu` and are green under
Wine 10.0 on the Linux test host (§6.2); they have not run on Windows:
the job `platform-windows-transport-shm` does that. The
FIFO as the native backend of the other Unixes (`native()` in `wait.rs`)
is compiled with Clippy for `x86_64-unknown-freebsd`, with libc on the
FreeBSD 11 ABI (`RUST_LIBC_UNSTABLE_FREEBSD_VERSION=11`): with libc's
default ABI (FreeBSD 12) `nix` 0.23.2, which `shared_memory` 0.12 brings
in, does not compile, and so the crate does not compile for FreeBSD. It
has not run on such a system; on Linux the tests above run the FIFO as a
forced backend.

### §4.5 Backpressure on a full ring

**Spec:** §4 — if the ring is full because the reader cannot keep up,
`send` MUST return a named error after a bounded spin phase. Blocking
forever and discarding quietly are both unacceptable.

**Repo:** `crates/transport-shm/src/posix.rs` (`SPIN_LIMIT = 1024`, the
two `SendError::Io` branches in `push_frame` for `Push::Full` and
`Push::FullAtWrap` of `crates/transport-shm/src/ring.rs`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_5_full_ring_returns_an_error_instead_of_dropping`
(512-byte ring, 128-byte frames, no consumer drain; the error text names
"ring full"),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_send_rejects_oversize_payload`.

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

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_1_segment_os_id_is_deterministic_and_within_platform_limit`
(the same pair yields the same name, the reversed pair a different one,
length under 31 on macOS),
`crates/transport-shm/src/posix.rs::tests::shm_5_1_short_names_of_pairs_that_differ_only_in_the_prefix_differ`
(the short form, on every platform: two pairs with the same entity ids and
other participants get different names),
`crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §5.2 Crash recovery

**Spec:** §5 — before creating a segment a zombie of the same name MUST
be removed (`shm_unlink`, idempotent — `ENOENT` is ignored), otherwise
`/dev/shm` leaks on the Nth restart. A leftover flink file must not make
the open fail.

**Repo:** `crates/transport-shm/src/posix.rs` (`shm_unlink_by_os_id` on
every Unix, before on Linux and macOS only; on Windows it removes the name
of the segment's file (`crate::mapping::remove_name`, §6.2), before it was
empty there; the create branch in `open` with a preceding removal of the
flink).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_2_a_zombie_segment_without_a_flink_is_unlinked_before_the_create`
(a segment under the pair's name with a valid header and a published
frame of the ended session, without a flink and without a holder; the
pair opens with an empty ring and carries only the new session's frame;
`open` counts no orphaned segment on the way: the
unlink before the create took the zombie, not the path of §5.5, which
would end at the same empty ring; on Windows too, where before the path
of §5.5 took the zombie),
`crates/transport-shm/src/posix.rs::tests::shm_5_2_open_recovers_from_stale_flink_of_crashed_owner`
(a crashed owner's flink file points at a segment that no longer exists;
`open_owner` cleans up and a frame passes through afterwards),
`crates/transport-shm/src/posix.rs::tests::shm_4_3_consumer_open_succeeds_without_owner_creates_segment`.

**Status:** done — measured on the Linux test host. Without the unlink
before the create the first test fails (the count stands at 1, a probe
with the call removed on the Linux test host). On macOS and the other
Unixes only compiled (§4.4), not run; the Windows branch is green under
Wine on the Linux test host (§6.2), not run on Windows.

### §5.3 Shutdown flag

**Spec:** §5 — the owner MUST set `shutdown = 1` on drop (release store).
The consumer checks the flag after every empty poll and returns a
targeted error instead of waiting out `recv_timeout`. Frames published
before the drop still arrive — the check sits behind `pop_frame`. When a
further owner opened the same segment after this one (the header's
`owner_epoch` is no longer its own, §2.1), the earlier owner's drop
neither sets the flag nor removes the name: the segment belongs to the
later one, and only that one's drop ends it. An owner's drop checks the
epoch and ends the segment under the pair lock (§5.4); it removes the
lock file last. An opener so runs entirely before or entirely after the
drop: it takes the segment over, or it finds no name and makes a new
segment, which the consumer follows after the shutdown flag. A
superseded owner sends no more: its `send` returns an error
("superseded"), since the ring has one producer. The owners of a segment
in one process share the producer lock, and the epoch goes up under it;
a push of the earlier one ends before the bump or sees it. The DCPS
same-host channels send a frame the superseded owner refuses through the
channel that is open now.

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::shutdown`),
`crates/transport-shm/src/posix.rs` (`Drop for PosixShmTransport` under `lock_pair` with the `owner_epoch`
comparison and the mapping's drop before the lock file goes,
`is_superseded`, `owner_producer_lock`, the refusal in
`send_with_prefix`, the shutdown check in `wait_parked` and `try_recv`);
`crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send_on`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_3_frames_published_before_the_drop_still_arrive`
(three frames sent before the drop arrive in order, the fourth call
returns the error with "owner" at once),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_signals_consumer_via_shutdown_flag`
(measures the return time against a 10 s timeout and checks the error
text for "owner"),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_superseded_owner_leaves_the_segment_to_its_successor`
(the first owner's drop leaves the segment to the second; only the
second's drop gives the consumer the error),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read`
(500 rounds: a thread drops the owner while the test opens the next one
behind a barrier; in every round the consumer reads the next owner's
ring; before the lock in the drop 16 of 20 runs failed, with a lock that
removed the lock file early 5 of 30),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_superseded_owner_refuses_to_send`,
`crates/transport-shm/src/posix.rs::tests::shm_5_3_two_owners_of_one_segment_never_push_at_once`
(the earlier owner sends without a pause, the later one opens and sends
5000 frames; every frame sent arrives whole and in order, no frame is
dropped as corrupt, the earlier one's sends end with the open),
`crates/dcps/src/same_host_channel.rs::tests::shm_5_3_the_peer_follows_a_channel_made_anew`,
`crates/dcps/src/same_host_channel.rs::tests::shm_5_3_a_send_on_a_superseded_channel_goes_through_the_new_one`,
`crates/dcps/src/runtime.rs::tests::a_same_host_peer_keeps_receiving_when_its_peer_opens_the_channel_again`
(two runtimes; a closes its channel to b and opens it again; b reads the
new owner's ring, whichever of the two ways it took, and the next sample
comes through the channel).

**Status:** done

### §5.4 Race protection on create

**Spec:** §5 — concurrent openers of the same `os_id` MUST be serialized
across threads and processes, otherwise one removes the segment the other
just created. The lock is released automatically when the handle closes
or the process dies (the same crash resilience as the segment itself).
The lock depends on the segment's name alone, not on the flink
directory: `<name>.lock` in the pair directory, on Linux `/dev/shm`
beside the segments, on macOS `zerodds-shm` in the user's temporary
directory as the system names it (`confstr(_CS_DARWIN_USER_TEMP_DIR)`, not
`TMPDIR`), on the other Unixes `/tmp/zerodds-shm-<euid>`, on Windows
`zerodds-shm` in the temporary directory. A segment has mode 0600, so the
two sides of a pair run under one user; on macOS and the other Unixes a
pair directory that is there must belong to that user, must not be a
symlink and must not be writable by group and others, or the open
fails; read and search access for group and others is taken away
(mode 0700). On macOS the pair directory is also the root of the flink
directories `zerodds-dcps` makes in `TMPDIR`
(`zerodds-shm/<host>/d<domain>`), and `create_dir_all` makes a missing
root with the mode the umask leaves, 0755 with 022. On Unix the pair files are opened with mode 0600 and without
following a symlink. On Windows neither the pair directory nor a pair
file may be a reparse point (a symbolic link, a junction), or the open
fails; the directory's owner and access list are not checked there: it
inherits those of the temporary directory, normally the user's own in
the profile, which only the user, `SYSTEM` and the administrators
enter. An opener that
finds under the lock no flink in its directory but a held segment name
(§5.5) attaches to it. Version 3 marks this place of the locks: a
version-2 user locked `<flink>.lock` and on macOS held the flink, and a
version-3 opener would take its live segment for an orphan.

**Repo:** `crates/transport-shm/src/posix.rs` (`pair_dir`, `pair_file`,
`private_dir` — opens the directory with `O_NOFOLLOW | O_DIRECTORY`,
checks owner, type and the write bits of group and others on that
descriptor and sets 0700 there —, `plain_dir` and `is_reparse_point` on Windows,
`darwin_user_temp_dir`, `open_pair_file`, `lock_pair`,
`acquire_flock_excl` — `flock(LOCK_EX)` on Unix, `LockFileEx` with
`LOCKFILE_EXCLUSIVE_LOCK` on Windows —, `FlockGuard`, `segment_in_use` on
every platform, `SHM_VERSION = 3`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`
(two threads open the same `os_id` concurrently; both end up validly
bound),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_users_in_different_flink_directories_share_the_segment`
(owner and consumer look the segment up in different flink directories,
under another `TMPDIR` say; the second attaches to the held segment
instead of removing it and making its own; Linux only before),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_openers_in_different_flink_directories_race_to_one_segment`
(200 rounds: owner and consumer, each in a flink directory of its own,
open at once behind a barrier; the frame arrives in every round),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_two_processes_open_the_pair_at_once`
(10 rounds: a child process opens the owner in its directory at the same
moment as the test opens the consumer in another; the child's frame
arrives),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_the_pair_lock_lies_in_the_pair_dir_not_in_the_flink_directory`
(the lock lies in the pair directory, not in the flink directory; outside
Linux the owner holds `<name>.hold` and the pair directory is private; the
owner's drop removes the lock),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_file_behind_a_symlink_is_refused`
(Unix: a symlink under the name of the lock file makes the open fail, and
the file it points to stays unchanged),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_dir_others_may_only_read_is_made_private_one_they_may_write_is_refused`
(`private_dir` on every Unix: a missing directory is made 0700, one with
0755 is taken and made 0700; 0720, 0702, 0775 and 1777 are refused with
`PermissionDenied` and keep their mode, so are a symlink to a private
directory and a file),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_an_open_takes_a_pair_dir_made_0755_and_makes_it_private`
(macOS and the other Unixes: the pair directory set to 0755, as
`create_dir_all` makes it for a flink directory under `TMPDIR`; owner and
consumer open, a frame arrives, the directory is 0700 afterwards; with
the former check an open failed so on a Mac with "… is not a directory
private to user 501", as every open on runner 8 did, job 238161),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_dir_that_is_a_junction_is_refused`
(Windows: a pair directory that is a junction is refused, a plain one is
taken).

**Status:** done — measured on the Linux test host (`cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets`, as root and as
`nobody`). The macOS branch ran green on a Mac (macOS 26.2, arm64; the
same command, 92 tests, in the sandbox of `ci/macos-loopback.sb`, with a
pair directory made 0755 beforehand). The Windows branch is compiled
with Clippy for `x86_64-pc-windows-gnu` but has not run there: that is
what the job `platform-windows-transport-shm` (§6.2) does, also for
`shm_5_4_a_pair_dir_that_is_a_junction_is_refused`. Under Wine 10.0 on
the Linux test host the tests of this item are green but that one: Wine's
`cmd` makes no junction with `mklink /J` ("Failed to open"). The branch of the
other Unixes (`/tmp/zerodds-shm-<euid>`, `private_dir`) is compiled with
Clippy for `x86_64-unknown-freebsd`, with libc on the FreeBSD 11 ABI;
with the default ABI the dependency `nix` 0.23.2 (through
`shared_memory` 0.12) does not compile (§4.4). It has not run on such a
system; `private_dir` itself runs in the unit test above on Linux and
macOS as well.

### §5.5 Liveness lock and orphaned segments

**Spec:** §5 — every user of a segment, owner and consumer alike, MUST
hold a shared `flock` on the segment for as long as it uses it; where the
segment is no file (every platform but Linux), on its hold file
`<name>.hold` in the pair directory (§5.4). The kernel
releases it when the process ends, also by `SIGKILL`; a segment without a
lock holder is orphaned. An opener that finds an orphaned segment under
its pair's name MUST remove it and create it anew instead of taking it
over: its ring holds the frames of the ended session. Orphaned segments
nobody opens again are removed by a sweep over `/dev/shm`. It leaves
alone segments younger than a minimum age (which covers the gap between
the creation and the creator's lock). It applies the lock test only to
segments of this layout version. From version 2 on this protocol holds: a
version-1 user holds no lock, so its live segments would look orphaned.
Rings of an earlier layout version (magic `ZSHM`, another version) are
therefore removed by the sweep on Linux only when no process maps them:
`/proc/<pid>/maps` names every mapping with its path in `/dev/shm`. When
the sweep cannot read the maps of a process of root, it leaves them all.
On macOS they stay: the mappings of a process cannot be listed there.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment`,
`SegmentHold`, `sweep_orphaned_segments`, `SweepReport`,
`sweep_stale_pair_files` for the `.lock` files and, on Linux, the flinks
of pairs whose segment is gone — they stayed after every process that did
not end cleanly (a test chain on a MacBook Air: 3 504 `.lock` files in one domain
directory); `lock_pair` checks after locking that the path still names
the locked file, `SHM_VERSION`; a ring with a blank header, whose creator ended before
it wrote the header, counts as removable, since no attacher accepts it),
`sweep_pair_dir` for the files in the pair directory: on Linux the
`.lock` files whose segment is gone, elsewhere the `.hold` files nobody
holds, with the orphaned segment they name, and `.lock` files without a
`.hold`; everywhere the wait FIFOs `.wake` (§4.4) whose segment is gone;
each under the pair's lock; `is_segment_os_id`: a name from a
flink goes into the paths of the pair files only in the shape
`segment_os_id` gives (`/zd-` and the two ids in hex, or the 26 hex
digits of the short form), a flink with other content counts as
`foreign` and stays; before, the Linux branch of `sweep_stale_pair_files`
checked only the start `/zd-`, and a flink holding
`/zd-x/../../../<dir>/<name>` made it remove `<dir>/<name>.lock` outside
`/dev/shm`),
`crates/transport-shm/src/bell.rs` (a participant's doorbell: a shared
`flock` on its file in `/dev/shm`, `sweep_orphaned_doorbells`);
`crates/dcps/src/same_host_shm.rs` (`sweep_orphaned_segments_once`: once
per process when a participant with `transport.same_host_shm` starts,
rings, doorbells and the pair directory, then per flink directory the
pair files, minimum age 60 s, event `shm.orphaned_segments_removed`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_5_an_orphaned_segment_is_made_anew_not_attached`
(a process of its own opens the owner, publishes a frame and ends without
a drop; the consumer then finds an empty ring and receives only the frame
of the new session),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_sweep_removes_orphaned_segments_and_keeps_held_ones`
(the orphaned segment stays below the minimum age and is removed above
it; a held segment stays, and a frame passes afterwards; a segment with a
blank header is removed),
`crates/transport-shm/src/bell.rs::tests::shm_5_5_the_sweep_removes_orphaned_bells_and_keeps_held_ones`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_lock_taken_on_a_removed_lock_file_is_taken_anew`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_dev_shm`
(Linux: a flink holding `/zd-<dir>/../../../<path>/secret` stays, and
`<path>/secret.lock` stays; without the check the test fails, the sweep
reports `removed: 1` and the file is gone),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_name_from_a_flink_has_the_shape_segment_os_id_gives`
(the long and the short form hold; empty, without `/`, upper case, one
digit too many or too few, a line end, `/`, `.` and `..` do not),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_ring_of_an_earlier_layout_goes_unless_a_process_maps_it`
(Linux: a version-1 ring nobody maps is removed, a mapped one stays until
the mapping ends),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_dir_sweep_removes_what_gone_segments_left`
(Linux: the lock of a segment that is gone goes, that of a present one
and a held one stay, below the minimum age everything stays; elsewhere:
the hold file of an ended process goes with its segment, that of an open
pair stays; on every Unix: a `.wake` FIFO made with `mkfifo` stays while
its segment is there and goes after; before, the test made `.wake` as a
regular file and could not fail, with a FIFO it fails against the earlier
sweep on the Linux test host: `young: 2` instead of 3, the FIFO stays).
Measured on the Linux test host: two participants ended by `kill -9`; the start of a
third after 60 s removed their 2 rings and 2 doorbells (`removed: 4`).
Before, the doorbell of every process that did not end cleanly stayed
(138 after a day of probes).

On macOS and the other Unixes without shared memory in the file system
the lock is on the segment's hold file in the pair directory (§5.4;
before, on the pair's flink, which a user in another flink directory did
not see). `sweep_orphaned_flinks` finds orphaned segments through the
flink directory (dcps: once per directory, that is per host and domain)
and removes them under the pair's lock with flink, hold and lock file; a
segment of another layout version stays, and so does a flink whose
content `is_segment_os_id` does not accept (test
`shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_the_pair_dir`:
a flink holding `/zd-<dir>/../../<victim>/secret` stays, `secret.hold`
stays, and no `secret.lock` is made; without the check the sweep made
`secret.lock` outside the pair directory and locked it before the
mapping refused the name; measured under Wine, with and without the
check). `sweep_pair_dir` finds through
the hold files also those whose flink is gone (dcps: once per process). A doorbell leaves a marker `<name>.bell`
there on which its participant holds the lock; the sweep removes bell and
marker when nobody holds it. Tests
`shm_5_5_the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones`
(with a version-1 ring that stays) and
`bell.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`;
the doorbell marker is unchanged and green on macOS 26, the segment part
with the hold file is compiled with Clippy for `aarch64-apple-darwin` and
has not run. `shm_5_5_an_orphaned_segment_is_made_anew_not_attached` runs
on every Unix. Windows: §6.3.

**Status:** done — measured on the Linux test host; the macOS branch of
the sweeps over the hold files and the `.wake` FIFOs ran green on a Mac
(macOS 26.2, arm64, §5.4); the branch of FreeBSD is compiled with
Clippy for `x86_64-unknown-freebsd` and has not run (there is no job
for FreeBSD); the flink with a name outside the pair directory is green
under Wine 10.0 on the Linux test host and has not run on Windows (§6.2).

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

**Tests:** `crates/transport-shm/src/mapping.rs::tests::shm_6_1_a_mapping_holds_no_file_descriptor`
(100 mappings, the number of open descriptors unchanged in a process of
its own; macOS 26),
`crates/transport-shm/src/mapping.rs::tests::shm_6_1_create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/posix.rs::tests::shm_5_1_segment_os_id_is_deterministic_and_within_platform_limit`
(checks the 31-character limit under `cfg(target_os = "macos")`),
`crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §6.2 Windows

**Spec:** §6 — on Windows `CreateFileMapping` supplies the mappings, the
create race is serialized via `LockFileEx`. The transport MUST run there
with the same test suite as on Unix.

**Repo:** `crates/transport-shm/src/mapping.rs` (Windows: one file per
segment in `%TEMP%\shared_memory-rs`, where `shared_memory` made it,
mapped through a mapping object without a name (`CreateFileMappingW`,
`MapViewOfFile`); the views of one file are coherent across processes
whichever mapping object they come from. `remove_name` renames a file to
`<name>.<pid>-<n>.gone` and then removes it: the name is free at once,
also where Windows deletes the old way and under Wine. Before,
`shared_memory` mapped through a mapping object named after the segment,
and Windows keeps a named object while a handle to it is open: whoever
made a segment anew under the name of an ended ring a consumer still
mapped got that ended ring (its header with the shutdown flag set, its
old `head` and `tail`) over a new, empty file);
`crates/transport-shm/src/posix.rs` (`file_lock`: `flock` on Unix,
`LockFileEx` on Windows, for the pair lock, the hold and the sweeps;
`still_named` compares the volume serial number and the file index on
Windows (`GetFileInformationByHandle`) and sees a file marked for
deletion (`delete_pending`), before it answered yes there without a look;
`lock_pair` retries the open of a lock file marked for deletion for up to
5 s; `hold_segment`, `segment_in_use`, `sweep_pair_dir`,
`sweep_orphaned_flinks` and `sweep_stale_pair_files` hold for Windows as
for macOS, `sweep_backing_files` for the segments' files;
`shm_unlink_by_os_id` removes the name of the segment's file on Windows,
before it was empty), `crates/transport-shm/src/bell.rs` (a doorbell has a
marker in the flink directory on Windows as on macOS, on which its
participant holds the lock), `crates/dcps/src/same_host_shm.rs`
(`sweep_orphaned_segments_once` on Windows too; before, nothing cleaned up
there). CI: the job `platform-windows-transport-shm` in
`ci/platforms.yml` (on the Windows CI runner, weekly, on a release
candidate and with `PLATFORMS=1`); the job `platform-windows-dcps-shm`
(`ci/platform_jobs.py`, `WINDOWS_FEATURE_RUNS`): builds `zerodds-dcps`
with the feature `same-host-shm` for Windows and runs its contract test
with `zerodds-transport-shm`, the sweep a participant runs at its start
among them; before, no job built the runtime with `same-host-shm` for
Windows.

**Tests:** `ci/test_platform_jobs.py::Gate::test_transport_shm_runs_on_windows_what_it_runs_on_linux`
(the Windows job runs every test of the crate the Linux run runs but 10;
the test names 8 of them with their counterpart on Windows (`/dev/shm` and
`/proc/<pid>/maps`, the name from a flink that leads out of `/dev/shm`, a
file symlink, the Unix mapping and an object not sized yet), 2 with the
reason: a Unix mapping closes its descriptor at
once, a Windows mapping holds the file and the mapping object; a test
taken away from Windows later makes it fail),
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_a_segment_made_anew_while_the_old_one_is_mapped_is_new`
(a segment made anew under the name of one still mapped is new and
zeroed, the old mapping keeps its bytes; with a mapping object named after
the segment the test reads 119 instead of 0: a probe under Wine on the
Linux test host),
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_an_unsized_segment_file_does_not_open`
(the segment's file between `create_new` and `set_len` does not open,
`MapOpenFailed(ERROR_INVALID_PARAMETER)`; sized, it opens),
`crates/transport-shm/src/bell.rs::tests::shm_6_2_a_bell_its_creator_has_not_sized_yet_is_waited_for`
(a sender waits for the size of a doorbell whose file its creator has not
sized yet),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_removed_pair_file_frees_its_name_at_once`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_lock_taken_on_a_removed_lock_file_is_taken_anew`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_dir_sweep_removes_what_gone_segments_left`
(these three Unix-only before),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones`,
`crates/transport-shm/src/bell.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`
(these two only on the Unixes but Linux before),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_the_pair_dir`
(the counterpart of the Linux test over `/dev/shm`, §5.5),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read`,
`crates/dcps/tests/contract_transport_shm.rs::the_participants_start_sweep_runs_without_a_failure`
(the sweep at a participant's start logs no `shm.orphan_sweep_failed`),
`crates/dcps/tests/contract_transport_shm.rs::a_sweep_of_a_fresh_or_missing_flink_directory_removes_nothing`
(Unix-only before).

**Status:** partial — measured on the Linux test host: `cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets` 91 of 91 green; the same
run with `--target x86_64-pc-windows-gnu` under Wine 10.0 as the target
runner 89 of 90 green (nextest reports one
of them as `leaky`); the one is
`shm_5_4_a_pair_dir_that_is_a_junction_is_refused`, since Wine's `cmd`
makes no junction. The run of the job
`platform-windows-dcps-shm` under Wine, `cargo nextest run --profile merge
-p zerodds-dcps --features same-host-shm --test contract_transport_shm
--target x86_64-pc-windows-gnu`: 5 of 5 green. Wine is not Windows: it frees the name of
a removed file only with its last handle and does not report it as marked
for deletion (measured; hence `remove_name`). On Windows itself this state
has not run: the jobs `platform-windows-transport-shm` and
`platform-windows-dcps-shm` run on the Windows CI runner, which this run
did not reach. An earlier state of the crate ran there in pipeline 8782
(44 tests green).

### §6.3 Evidence for the Windows cleanup semantics

**Spec:** §6 — that a crashed owner leaves no mapping with its session's
frames on Windows MUST be shown by a test on the Windows runner, not
merely asserted by the documentation.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment` with
`LockFileEx` on the segment's hold file in the pair directory (§5.4), as
the macOS lock; an opener that finds a segment without a holder discards
it with `discard_orphaned_segment`; when nobody opens the pair again,
`sweep_pair_dir` with `sweep_backing_files` and `sweep_orphaned_flinks`
remove at a participant's start (`crates/dcps/src/same_host_shm.rs`,
`sweep_orphaned_segments_once`; built and run for Windows in the job
`platform-windows-dcps-shm`) what the ended owner left: the segment's
file with the frames of its session, the hold, lock and flink files;
before, no sweep ran on Windows and each such file stayed),
`crates/transport-shm/src/mapping.rs` (the segment is a file in
`%TEMP%\shared_memory-rs` that outlives the process). The earlier
assumption that Windows releases the mapping with its last handle did not
hold for `shared_memory` either (the crate's file windows.rs, function
`new_map`); measured in nightly 6555: the test found 29 bytes of the ended
session in the ring.

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_5_an_orphaned_segment_is_made_anew_not_attached`
(a child process opens the owner, publishes a frame and ends without a
drop; the same name then opens with an empty ring),
`crates/transport-shm/src/posix.rs::tests::shm_6_3_the_sweep_removes_what_an_ended_owner_left_on_windows`
(of what an owner that ended without its drop left, the sweep of the pair
directory removes the segment's file with the frame, the hold and lock
files, the flink sweep the flink; a ring whose creator ended before its
hold goes with the sweep of the segments' files and stays under the
minimum age; a held pair stays and carries a frame after).

**Status:** partial — both tests green under Wine 10.0 on the Linux test
host (§6.2); not run on Windows: the job `platform-windows-transport-shm`
on the Windows CI runner measures it, which this run did not reach.

### §6.4 `no_std`

**Spec:** §6 — the ring of §2 to §4 and the shutdown flag of §5.3 hold on
every target, also without an operating system and without `std`, but
for §4.3: over a region the owner alone makes the ring. A consumer that
comes before it gets `RingError::NotReady` and opens again later. Without
an operating system there is no lock that orders two sides writing the
header at the same time, and the owner makes the ring anew over any
other contents. Without
an operating system the integrator gives the region: memory two cores of
one chip share, or a page a hypervisor gives two partitions; one region
per pair, aligned to 8 bytes, at an address both sides see, with at most
one owner and one consumer at a time. Names, flinks, the removal before a
create and the locks (§5.1, §5.2, §5.4, §5.5) are services of an
operating system and do not apply there. The session rule takes their
place: an owner makes the ring anew at its start where the region holds
no ring of this layout and capacity. Where it holds one, the owner goes on
with it and writes where its session begins to `session_head` (offset
48): `head` when the previous owner ended without its drop (`shutdown` 0,
a core that was reset, say), so that its frames are not delivered (as
§5.5 has it); the consumer's `tail` when the previous owner ended its side
(`shutdown` 1), so that its frames still arrive (as §5.3 has it). Then it
counts `owner_epoch` up; an owner after which another one opened sends no
more (§5.3). The owner's drop sets the shutdown flag and wakes the
consumer: a `recv` that waits returns the error of §5.3, with and without
a timeout. A consumer skips to the session's start when it attaches and
whenever `owner_epoch` changes. Each side reads the other's position only
in its low 32 bits; the capacity is therefore at most 2³¹ bytes, and a
core without 64-bit atomics takes part by writing its position as two
32-bit words, the low one last with `Release`. How one side waits for the
other is the integrator's choice (an event between cores, an interrupt);
the wait ends on a frame and on the owner's drop, otherwise only with its
timeout. A wait that returns before that ends a `recv` with a timeout
with `Timeout`, since the side has no clock for the rest of the timeout;
a `recv` without a timeout waits again. Without a choice of its
own the side reads the low word of `head` and the shutdown flag in a
loop. The crate builds without `std`
for a target without an operating system: the ring without `alloc`, the
transport with `alloc`.

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring` in `core` alone:
the header, `create`, `create_in_zeroed`, `attach`, `try_push`, `next`,
`release`, `drop_published`, `skip_to_session`, the positions through
`store_position`, `load_position` and `low`, without 64-bit atomics the
module `split`), `crates/transport-shm/src/region.rs`
(`RegionShmTransport`: `open_owner` and `open_consumer` over the caller's
region, the session rule, `Transport`; `RingWait`, `Parked` (what a
`recv` waits on: the low word of `head` and the shutdown flag; before, it
waited on the word alone, and the owner's drop, which leaves `head` as it
is, did not end the wait; `RingWait::park` returns `true` only after
`Parked::is_over` was true, and otherwise `false` after its timeout, and
`Parked` keeps whether `is_over` was true; before, `park` was allowed to
return early, and `recv` began the whole timeout anew on every turn),
`SpinWait`, `RegionConfig`,
`RegionShmError`), `crates/transport-shm/src/posix.rs`
(the transport of the named segments uses the same `Ring`; before, the
ring lay there alone and was tied to `Mapping`, `std::sync::Mutex` and
`Vec`), `crates/transport-shm/src/lib.rs` (`#![cfg_attr(not(feature =
"std"), no_std)]`; `ring` always, `region` with `alloc` on targets with
32-bit compare-and-swap, `posix`, `bell`, `mapping` and `wait` with
`std`), `crates/transport-shm/Cargo.toml` (`shared_memory` and `libc`
optional under `std`, `zerodds-rtps` and `zerodds-transport` without
default features, the category `no-std`: with it
`tools/no-std-claims-gate.sh` in the job `no-std`
(`ci/weekly-checks.yml`) builds the crate for `thumbv7em-none-eabihf`
without default features with `alloc`; the same job builds it without
`alloc`),
`crates/transport-shm/bare-metal/two-cores` (a program without an
operating system for `aarch64-unknown-none`, a workspace of its own: core
0 of a QEMU `virt` machine with two Cortex-A53 owns a ring in a region of
RAM, core 1, which core 0 starts over PSCI, is its consumer; both with the
MMU and caches on, waiting with `WFE`/`SEV`, a bump allocator; a
`Cargo.lock` of its own), `tools/shm-region-two-cores.sh` (builds it with
`--locked` and runs it under `qemu-system-aarch64 -smp 2`), the CI job
`no-std-shm-two-cores` in `ci/weekly-checks.yml`;
`tools/shm-ring-armv5te.sh` (runs the tests of `ring` and `region`, built
without `std`, for `armv5te-unknown-linux-gnueabi` under `qemu-arm`; that
target has 32-bit atomics only, so the ring writes its positions as two
words through `split`; the script checks this with `rustc --print cfg`),
the CI job `no-std-shm-armv5te` in `ci/weekly-checks.yml`;
`ci/Dockerfile.rust` puts `qemu-system-arm`, `qemu-user`,
`gcc-arm-linux-gnueabi`, `libc6-dev-armel-cross` and the targets
`aarch64-unknown-none` and `armv5te-unknown-linux-gnueabi` into the
image.

**Tests:** `crates/transport-shm/src/region.rs::tests::shm_6_4_a_ring_over_a_caller_supplied_region_carries_frames`
(an owner and a consumer on two threads carry 5 000 frames of 4 to 603
bytes through a 4 KiB ring in a region the test gives, in order and
whole, across the ring's end and across 2³², where the low word of a
position wraps),
`crates/transport-shm/src/region.rs::tests::shm_6_4_a_new_owner_session_discards_the_frames_of_an_ended_one`
(an owner without its drop leaves two frames, the next owner begins
behind them, the consumer receives only its frame; an owner with its drop
leaves its frames to the consumer; a superseded owner sends no more),
`crates/transport-shm/src/region.rs::tests::shm_6_4_the_owner_makes_the_ring_anew_where_the_region_holds_none`,
`crates/transport-shm/src/region.rs::tests::shm_6_4_recv_waits_with_the_integrators_wait_up_to_its_timeout`,
`crates/transport-shm/src/region.rs::tests::shm_6_4_the_owners_drop_ends_a_parked_recv`
(the owner's drop ends a `recv` waiting in `SpinWait`, without a timeout
and with 30 s, with `shm owner terminated`; with the wait on the word
alone the test fails after 10 s),
`crates/transport-shm/src/region.rs::tests::shm_6_4_a_park_that_returns_early_does_not_restart_the_timeout_without_end`
(a wait that returns `true` after 1 ms although the wait is not over: a
`recv` with 50 ms ends with `Timeout` after one wait, one without a
timeout waits again and gets the frame that comes after; with the earlier
`recv` the `recv` with 50 ms had not returned after 3 s, and the test
failed),
`crates/transport-shm/src/ring.rs::tests::shm_6_4_positions_cross_the_low_word_wrap_and_split_words_match_whole_ones`
(positions past 2³², where a 32-bit core's low word wraps: 400 frames
whole and in order; a position written as two 32-bit words has the bytes
of the whole word),
`crates/transport-shm/src/ring.rs::tests::shm_6_4_create_and_attach_check_the_header_and_the_region`,
`crates/transport-shm/src/posix.rs::tests::shm_6_4_region_and_posix_transport_share_one_layout`
(a consumer over a region reads a POSIX owner's frames from its mapping,
a POSIX consumer those of an owner over the region),
`tools/shm-region-two-cores.sh` (two cores without an operating system:
10 000 frames of 4 to 203 bytes through an 8 KiB ring, each checked byte
by byte; then core 1 waits in a `recv` without a timeout, and core 0
drops the owner; QEMU ends with status 0 only when all frames arrived and
the drop ended the wait with the error of §5.3),
`tools/shm-ring-armv5te.sh` (the eight tests of `ring` and `region` on a
core without 64-bit atomics).

**Status:** done — measured on the Linux test host: `cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets` 91 of 91 green;
`tools/shm-ring-armv5te.sh` 8 of 8 green (`qemu-arm` 10.0.13);
`tools/shm-region-two-cores.sh` under QEMU 10.0.13 green five times in a
row ("sent 10000, received 10000, ..., the owner's drop ended the parked
recv", 4 to 18 sends met a full ring). With the earlier wait on the word
alone `shm_6_4_the_owners_drop_ends_a_parked_recv` fails after 10 s, and
the program of the two cores does not end (probe; QEMU ended after
120 s). `cargo build -p zerodds-transport-shm --no-default-features
--target thumbv7em-none-eabihf` with and without `--features alloc`
compiles; Clippy for `thumbv7em-none-eabihf` (with and without `alloc`),
for `armv5te-unknown-linux-gnueabi` and for the bare-metal program
without findings. The ring without 64-bit atomics ran as a Linux process
under `qemu-arm`, not on a core without an operating system. The jobs
`no-std-shm-two-cores` and `no-std-shm-armv5te` have not run in CI yet:
they need the image the change to `ci/Dockerfile.rust` rebuilds at the
merge.

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

**Tests:** `crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_shm_loan_writer_to_reader_zero_copy`
(measures the loan path itself, not its setup: the test writes the
payload straight into the loaned pointer and commits in place; the
reader's take pointer carries another address than the writer's — its own
mapping of the same segment — and the bytes are equal. That the two
pointers address the segment's bytes and not a copy, the test shows with a
third mapping of the segment that it makes itself: it holds the payload;
a pattern written through the writer's loan pointer after the commit is
read through the reader's pointer and through the third mapping, and one
written through the third mapping is read through both. A commit that
copied a heap buffer into the slot, or a take that copied the slot out,
fails these reads. Then release, `NoData` on the next take and a second
sample through the same ring),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_shm_loan_runtime_path_writer_to_reader_zero_copy`
(the same path, with the same proof over a third mapping, over the
runtime FFI the ROS 2 RMW uses),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_loan_without_shm_falls_back_to_heap`
(without an enabled loan the heap path stays),
`crates/flatdata/src/posix.rs::tests::flatdata_3_2_write_read_through_shm`
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

**Status:** n/a (informative) — boundary between ring and loan path; the loan path is item §7.1.

---

## Audit-Status

19 done / 3 partial / 0 open / 0 blocked / 2 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-shm-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-c-api --all-targets` (job `merge-rust-c-api`) — 3 tests green.
* `cargo nextest run --profile merge -p zerodds-flatdata --all-targets` (job `merge-rust-flatdata`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (job `merge-rust-rtps`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-transport-shm --all-targets` (job `merge-rust-transport-shm`) — 41 tests green.
* `cargo nextest run --profile merge -p zerodds-dcps --all-targets --features same-host-shm` (job `merge-rust-dcps-features-same-host-shm`) — 1 test green.
* job `merge-macos-transport-shm` — 2 tests green.

50 cited tests green, 0 not green.

Open items: `zerodds-shm-transport-1.0.open.en.md`.
