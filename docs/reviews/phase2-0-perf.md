# Phase-2.0 Performance Review

Scope: WP 2.0a (`Arc<[u8]>`) + WP 2.0b (UDS DGRAM/Abstract, POSIX-SHM,
TCP handshake). Baseline: `phase1-perf-audit.md`, `v1.2-zerocopy-bench.md`.

## 1. Resolved findings

**F14 (SHM send clones payload in peer lock)** — legacy
`shm_transport.rs:109` still does `buf.push(data.to_vec())`. The new
`posix.rs::push_frame` writes directly via `ptr::copy_nonoverlapping`
(`write_slice`, 223–229): one unavoidable memcpy into the mapped ring,
no `Vec`, no mutex. Resolved for the new path; legacy module should be
`#[deprecated]` or removed.

**F13 (TCP inbound Mutex across Condvar::wait)** — unchanged. The
handshake (`handshake.rs:326–391`) operates on the owned stream only,
no shared state. `TcpTransport::recv` still holds `inbound.lock()` over
`inbound_cv.wait(st)` (`tcp_transport.rs:412–422`). F13 stays open.

## 2. New hotspots

- **Per-send allocations (UDS)**: `UdsTransport::send` calls
  `socket_path()` (`lib.rs:155`) — `String`+`PathBuf` per datagram.
  Abstract mode has two `format!`s per send+recv
  (`abstract_dgram.rs:306–310`, `:401`). Fix: cache `SockAddr` per peer.
- **Recv allocation (UDS Abstract)**: `vec![0; recv_buf]` (212 KiB) per
  call (`abstract_dgram.rs:240`). Thread-local scratch, copy `rc` bytes
  into a right-sized `Arc<[u8]>` (post 2.0a-2).
- **POSIX-SHM spin-loop**: `SPIN_LIMIT = 1024` `core::hint::spin_loop()`
  with no yield/sleep (`posix.rs:86, 427–434, 452–458`). Tight-spin
  ~tens of µs, then `SendError::Io` — no backpressure upward. After N
  spins, `thread::yield_now()` then µs-sleep like `wait_for_frame`
  (`posix.rs:514–527`).
- **Padding-frame amplification**: with `max_datagram ≈ capacity/2`
  nearly every wrap costs a padding write+atomic store
  (`posix.rs:444–447`) and `pop_frame` recursion (`:488–501`). Document
  `capacity >= 4 * max_datagram` at the `InvalidConfig` check.
- **No new Arc/Mutex contention.** TCP keeps the Phase-1 two-step lock
  (`tcp_transport.rs:376–398`); UDS + POSIX-SHM are lock-free.

## 3. 2.0a-2 (sendmsg/iovec) readiness

- **Biggest win**: UDP + UDS-DGRAM. The MessageBuilder still
  concatenates header + submessages before `sendto` (the two remaining
  copies the zero-copy bench calls out). `sendmsg`+`iovec` passes RTPS
  header and each `Arc<[u8]>` submessage as separate slices.
- TCP: modest win via `writev` in `framing::write_frame`.
- SHM: **no benefit** — ring write is already one memcpy into mapped
  memory.
- Touch-points:
  - `transport-udp` send → `libc::sendmsg`.
  - `transport-uds/src/lib.rs:156` + `abstract_dgram.rs:213–222` →
    `libc::sendmsg`.
  - `transport-tcp/src/framing.rs::write_frame` → `writev`.
  - `zerodds-rtps` MessageBuilder: expose
    `as_iovecs() -> SmallVec<[IoSlice; N]>` alongside `build()`.

## 4. Recommended criterion benches

1. **`shm-posix-send-1kB-steady`** — owner/consumer tight loop,
   throughput without wraparound.
2. **`shm-posix-wrap-pathological`** — `capacity = 3 * max_datagram`,
   padding every ~2 sends; validates the `4 * max_datagram` sizing
   guidance.
3. **`uds-abstract-vs-filesystem-1kB`** — same payload, both modes;
   quantifies the FS-lookup saving the abstract module claims.
4. **`tcp-handshake-cost`** — connect + handshakes + first write_frame
   vs. plain TCP.

## Open / deferred

F4, F5, F12, F19 unchanged. F13 unaffected by handshake. Legacy
`shm_transport.rs` should be deprecated so F14 cannot regress.
