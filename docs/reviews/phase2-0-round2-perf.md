# Phase-2.0 Round-2 Performance Review

Scope: Consolidation Batches A+C (`c3b9439`, `d5266d7`). Baseline:
`phase2-0-perf.md`.

## 1. Hot-path regressions

**`pop_frame` (`transport-shm/src/posix.rs:693`)**:

- DoS-Guard at `:732` — one compare on `len > max_datagram`,
  predictable branch, free.
- `padding_counter.fetch_add(Relaxed)` at `:720` fires **only on
  padding-frame**, not per real frame. Per-real-frame delta: a single
  extra `u32` compare (`len == PADDING_FRAME_LEN`). Effectively zero.

**`classify_send_error` (`transport-uds/src/lib.rs:232`)** — 5-arm
match on `ErrorKind`, only on the send-error path (`:196`). Happy
path unchanged.

**Owner-bind** (`posix.rs:504-528`): `OpenOptions::create` + `flock` +
`shm_unlink_by_os_id` + `remove_file(flink)` + `ShmemConf::create` —
4-5 syscalls once per owner-bind, not per send. Irrelevant steady-
state.

**UDS `ensure_base_dir` (`transport-uds/src/lib.rs:127`)** —
`symlink_metadata` on the existing-dir path short-circuits at `:155`
(1 syscall). Cold path: 3 syscalls once per process. Hot-path free.

## 2. Tail-latency (10 ms → 1 ms)

`posix.rs:769` caps `max_backoff = 1 ms`. Sleep sequence
`10µs, 20, 40, 80, 160, 320, 640, 1000, 1000, ...`. p50 unchanged:
frame arriving during the first poll returns in ~10 µs. p99 on a
reader parked between frames: worst-case wake bounded by 1 ms (was
10 ms). Factor-10 p99 reduction. CPU at idle: ~1000 polls/s, <0.1 %
(documented `:762`). No p50 / throughput regression.

## 3. New criterion-bench recommendations

Add under `crates/transport-shm/benches/` + `crates/transport-uds/benches/`:

1. **`shm_padding_amplification.rs`** — capacity tuned so every 2nd
   send wraps. Throughput + `padding_frames_seen()` vs. non-wrapping
   baseline. Pins the padding-counter atomic + validates the
   `capacity >= 4 * max_datagram` guidance.
2. **`shm_owner_bind_teardown.rs`** — `iter_batched` over `open_owner`
   + `Drop`. Fixes a startup-regression-tripwire for flock +
   `shm_unlink` + 2× `remove_file`.
3. **`uds_send_error_classify.rs`** — send to non-existent peer
   (NotFound) vs. live peer, 1 kB. Pins classify-cost and the
   `socket_path()` alloc (Round-1 §2).

## 4. Scalability: flock with N Readers

Only Owner flocks (`posix.rs:511`); Consumer skips (`:530`). N
Consumers ⇒ zero extra flocks. Concern applies only if one process
hosts N Owners for N distinct readers — then it is N bind-time
flocks on N distinct `.lock` paths, uncontended. At 1000 readers
behind one publisher: 1000 uncontended flocks, bind-cost linear.
Real bottleneck is fd + `/dev/shm`-inode count, not lock contention.
Suggest a DCPS-side bind-rate cap above a reader threshold.

## 5. WP 2.0a-2 iovec readiness

Unchanged. A+C touch SHM-unlink, padding-counter, TCP handshake-
timeouts, UDS TOCTOU. None touch MessageBuilder, `sendto` sites, or
TCP `write_frame`. Round-1 §3 touch-points stand.
