# ZeroDDS Zero-Copy 1.0

> Status: Draft v0.1 (2026-05-17)
> Scope: data path copy inventory + reduction strategy
> Implemented: partly (flatdata, transport-shm, loan API skeleton)
> Vendor-specific (not OMG-normative)

## 1 Purpose

This spec inventories all **copy operations** on the sample data path
between user application and wire, classifies them by
reducibility, and defines the reduction architecture for
ZeroDDS. Goal: every copy has a documented reason
(spec obligation / implementation / reducible) and a
named migration path.

## 2 Data path — write (user → wire)

| # | Location | Crate / file | Operation | Class |
|---:|---|---|---|---|
| W1 | C-FFI `zerodds_writer_write` | `zerodds-c-api/lib.rs:2129` | `slice::from_raw_parts(payload, len)`, lent to `write_user_sample_borrowed` | — (no copy) |
| W2 | DCPS `write_user_sample` (≤ 1.5 kB) | `dcps/runtime.rs:981` | `PoolBuffer::extend_from_slice(encap)` + `extend_from_slice(payload)` | wire framing |
| W2' | DCPS `write_user_sample` (> 1.5 kB) | `dcps/runtime.rs:18312` | `Vec::with_capacity(total) + extend_from_slice × 2` | wire framing |
| W3 | RTPS `MessageBuilder::finish` | `rtps/message_builder.rs` | submessage build → `OutboundDatagram { bytes: Vec<u8>, … }` | datagram construction |
| W4 | Security `secure_outbound_for_target` | `dcps/runtime.rs:5643` | sign/encrypt + tag append per target locator | spec obligation (security on) |
| W5 | Transport UDP `send` | `transport-udp/udp_transport.rs:990` | `sock.send(data)` over the target's connected socket (`transport.udp.connected_cache`, default), otherwise `socket.send_to(data, addr)` | kernel socket |

**Total: 2 user-space copies (W2, W3), with security 3 (W4), + 1 kernel copy
per sample** on the cross-host UDP path.

## 3 Data path — read (wire → user)

| # | Location | Crate / file | Operation | Class |
|---:|---|---|---|---|
| R1 | Transport UDP `recv` | `transport-udp/udp_transport.rs:1102` | `recv_from(&mut buf)` + `Arc::from(&buf[..len])` | owned return |
| R2 | DCPS `handle_user_datagram` | `dcps/runtime.rs:26668` | `decode_datagram_shared_into` for an `Arc` datagram, otherwise `decode_datagram` → submessage slices (borrowed) | — (no copy) |
| R3 | RTPS HistoryCache push | `rtps/history_cache.rs` | `Arc<[u8]>` push | — (shared ownership) |
| R4 | DCPS `strip_user_encap_arc` | `dcps/runtime.rs:26583` | range in the `Arc` of the datagram (`SampleBytes::from_arc_slice`) | — (no copy) |
| R5 | DCPS mpsc send | `dcps/runtime.rs:26451` | `sample_tx.send(item)` with `UserSample::Alive { payload: SampleBytes, … }` | move-only (no copy) |
| R6 | C-FFI `zerodds_reader_take` | `zerodds-c-api/lib.rs:3072` | `bs.to_vec().into_boxed_slice()` → `Box::into_raw` | Vec materialization at the FFI boundary (copy) |

**Total: 2 user-space copies + 1 kernel copy per sample**.

## 4 Existing zero-copy infrastructure

| Component | Crate | Status | Spec |
|---|---|---|---|
| `unsafe trait FlatStruct` | `crates/flatdata/` | live, feature-gated | `zerodds-flatdata-1.0.md` |
| POSIX SHM (`shm_open` + `mmap`) | `crates/flatdata/posix.rs` | live | `zerodds-flatdata-1.0.md` §5 |
| iceoryx2 backend | `crates/flatdata/iceoryx.rs` | feature `iceoryx2-bridge` | `zerodds-flatdata-1.0.md` §6 |
| SHM transport (SpSc ring buffer) | `crates/transport-shm/` | live | `zerodds-shm-transport-1.0.md` |
| `Arc<[u8]>` shared ownership | `crates/rtps/` (21 usages) | live | — implementation detail |

**Wired:** The same-host SHM channel per participant pair (§6) carries the
unicast datagrams between ZeroDDS participants of a host.
`zerodds_writer_commit_loan` (`zerodds-c-api/lib.rs:3831`) hands over an
active SHM loan with `shm_loan_ffi::try_commit` (feature `flatdata-loan`) and
otherwise writes the loaned buffer via `write_user_sample_borrowed`, without a
`Vec`.

## 5 Copy classification

### 5.1 Spec obligation (not reducible without breaking the wire)

| # | What | Why mandatory |
|---|---|---|
| CDR encode | struct → wire bytes | XCDR2 format ≠ Rust layout; XCDR2 spec §7 |
| W4 Security | append tag/MAC | DDS-Security 1.2 §9.5 |
| W5 UDP `send_to` kernel | user buffer → socket buffer | POSIX socket API |
| R1 UDP `recv_from` kernel | socket buffer → user buffer | POSIX socket API |

### 5.2 Implementation choice (reducible)

| # | What | Current reason | Reduction path |
|---|---|---|---|
| **W2/W2'** | encap header mounting | linearization for the write API | scatter-gather: `write_segments(&[&[u8]])` |
| **W3** | submessage build → Vec | RTPS header preparation | `BytesMut` builder + `freeze() → Bytes` |
| **R1** | `recv_from` → `Arc::from(&buf[..len])` | owned return for the caller | slab pool + `Arc<[u8]>` with pool recycle |
| **R6** | C-FFI `to_vec().into_boxed_slice()` | ownership transition Rust→C | loan of the payload to the caller with a return call |

### 5.3 Architecture investment (largest lever)

| Path | Today | After wiring |
|---|---|---|
| Same-host (SHM) | SHM channel (§6): W2 and W3, the datagram into the ring (1 copy), `pop_frame` into a `Vec` and `try_recv` as `Arc<[u8]>` (2 copies, `transport-shm/posix.rs:2062`, `:1926`) | **0 copies** via flatdata + iceoryx slot backend |
| Loan API (C-FFI) | with `flatdata-loan` `shm_loan_ffi::try_commit`, otherwise `write_user_sample_borrowed` (W2, W3) | slot pointer from the SHM backend, `commit_loan` ⇒ slot publish |

## 6 Zero-copy paths

### Reader path (UDP pool + Arc)

UDP recv reads into a buffer and returns it as `Arc::from(&buf[..len])`
(R1, 1 copy); `strip_user_encap_arc` works offset-based in the `Arc`
without `.to_vec()` (R4);
`UserSample::Alive` holds the payload as an Arc slice onto the wire datagram
(no heap re-alloc). Cross-language bindings still see `(*mut u8, len)`;
the Vec materialization step lies only at the FFI boundary.
→ −2 copies per received sample.

### Same-host SHM hot path

Two ZeroDDS participants on the same host (same `host_id`,
`is_same_host`) connect via an SHM channel per participant pair:
two rings, one per direction, created as soon as SPDP reports the other
participant, and released when it leaves (lease, SPDP dispose)
or the own participant ends. Every unicast datagram to a
locator of this participant goes through the channel instead of over UDP:
user data (DATA, DATA_FRAG), HEARTBEAT, ACKNACK, GAP, NACK_FRAG,
repetitions, SEDP, TypeLookup and WLP unicast. Every frame carries at the front
one byte for the target port (metatraffic or user unicast); the
receiver passes it to the same handler as the corresponding
UDP socket. Multicast (SPDP announcements) stays on UDP.

- The sender only routes once the receiver has opened its ring
  (flag `consumer_attached` in the segment header); until then, and for a
  participant without a channel (older state, other vendor), UDP stays.
- If the ring does not accept a datagram (full, larger than
  `max_datagram`), it goes via UDP; counted in
  `same_host_shm_udp_fallbacks`.
- What goes over UDP although a channel could carry it is counted per peer:
  rejected by the ring (`refused`) and sent before the peer
  has opened its end (`unattached`). At most every 10 s the
  tick reports `shm.channel_udp_sends` (warning) for a peer with new
  rejected frames or with sends while the peer had still not opened its end 5 s
  (`ATTACH_GRACE`) after the channel was opened;
  sends before that are the normal course of discovery. Datagrams to
  a loopback locator without a channel are reported by `shm.unrouted_loopback_sends`
  (info): a participant of this host without a channel or a peer of
  another vendor.
- If the reader of a full ring takes nothing for 2 s (`STALL_LIMIT`),
  the sender closes the channel (`shm.channel_closed_stalled_reader`).
  A reader terminated via `kill` still counts as attached; its ring
  would otherwise stay full until lease expiry, and every datagram would go
  over UDP only after a spin. A reader that is still there attaches to
  the new segment as soon as SPDP reports the peer again.
- A channel holds two descriptors per peer, the liveness locks of its
  two rings; the mappings themselves hold none (`crate::mapping`).
- One receive thread per participant for all peers: after every frame
  the sender rings the receiver's doorbell (a word in a
  small segment, `ShmDoorbell`), the thread parks on it and empties
  all incoming rings.
- A ring holds four datagrams of `transport.shm_max_datagram`
  (default 64 KiB, so one ring is 256 KiB); what does not fit goes over
  UDP.
- Participants of the same process (same bytes `[0..8]` of the prefix)
  get no channel; the in-process path serves them.
- Several threads of a process send into the same ring; the transport
  serializes them.
- If the peer's ring ends (shutdown flag) while the peer is reported,
  the receiver reopens the current ring.
- Bytes in the channel are the same as on UDP, so they are also secured where
  DDS-Security protects; the receiver runs through the same
  `secure_inbound`.

Feature `same-host-shm`: off in `zerodds-dcps`, in the default features
in `zerodds-c-api`; can be switched off at runtime via `transport.same_host_shm` /
`ZERODDS_SAME_HOST_SHM=0`. Separately from this,
`transport.user_transport = SHM` replaces UDP for user data entirely
(`ShmUserTransport`, also without reference to discovery).

| Component | Status |
|---|---|
| `GuidPrefix::host_id` + `is_same_host` (FNV1a hash of gethostname in `bytes[0..4]`) | ✓ done |
| `dcps::same_host_channel`: channel per participant pair, routing by peer locator, port byte per frame | ✓ done |
| Send path: `user_unicast` as a routing transport, metatraffic via `send_metatraffic`; fallback to UDP on a rejected frame | ✓ done |
| Receive: one thread `zdds-recv-shm` per participant on the doorbell, dispatch like the respective UDP socket; reopen after shutdown flag | ✓ done |
| Lifecycle: SPDP report creates, loss of the participant and `shutdown` release | ✓ done |
| E2E `crates/dcps/tests/same_host_e2e.rs` (two participants in one process with the same host_id) | ✓ done |

### C-FFI loan API

`zerodds_writer_loan_message` / `zerodds_dw_loan_message` with
`commit_loan`/`discard_loan` (ABI: `(*mut u8, len)`, signatures stable). The
loan buffer is currently heap-allocated (`commit_loan` = Vec copy); an
SHM-slot-backed loan (slot pointer from the SHM backend, `commit_loan` =
slot publish) is still **open**.

**Architecture note on the backend wireup:**

`zerodds-transport-shm::PosixShmTransport` is a ring buffer with one
producer and one consumer on POSIX `shm_open` + `mmap` (Linux/macOS)
or `CreateFileMapping` (Windows); the transport serializes the senders of a process.
One segment per participant pair and direction,
name from both channel IDs, flink directory
`${TMPDIR}/zerodds-shm/${host_id_hex}/d${domain}/`. Whoever opens first
creates it; the other one attaches and takes the ring size from the header.
Until the receiver has opened its ring, the sender does not route
there, so that no datagram goes into an unread ring.

**Optional feature gates:**

- `same-host-iceoryx2`: uses `zerodds-flatdata::iceoryx` instead of
  POSIX shm. Delivers real zero-copy for `T: FlatStruct` samples,
  for generic bytes it falls back to the POSIX shm path.
- `same-host-uds`: `transport.user_transport = UDS` carries the
  user data via `zerodds-transport-uds` (UnixDatagram) instead of UDP.
  No zero-copy (the kernel copies); useful on targets without
  `/dev/shm` (sandboxed containers).

## 7 Tests + verification

| Path | Test |
|---|---|
| C-FFI loan API | bench `loan_message + commit_loan` cycle: heap vs. iceoryx slot comparison |
| Reader path | bench `recv_loop` throughput; Miri on the Arc<[u8]> lifecycle |
| Same-host SHM | same-host cross-process test with two processes, verify 0 bytes through UDP |

The ABI snapshot test stays green throughout — no extern fn signatures
change.

## 8 Spec cross-references

- `zerodds-flatdata-1.0.md` — FlatStruct + slot layout
- `zerodds-shm-transport-1.0.md` — SpSc ring buffer on SHM
- `zerodds-c-api-1.0.md` — loan API signatures (§2.6)
- DDS 1.4 §2.2.2.4.2 — loan API spec contract
- DDSI-RTPS 2.5 §9.4 — locator kind LOCATOR_KIND_SHM

## 9 Open items

- [ ] Decide on `BytesMut`/`Bytes` as the canonical wire buffer type
- [ ] Evaluate `recvmmsg` batch recv (Linux-specific performance win)
- [ ] `sendmsg(iovec)` scatter-gather for W2 encap framing
- [ ] Measure the bench baseline: throughput + heap allocs per sample
