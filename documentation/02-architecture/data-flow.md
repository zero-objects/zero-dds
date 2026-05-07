# Data flow on `write_user_sample`

What actually happens when application code calls `write`? Trace
of a single sample on the publisher side.

```
app code
   │ rt.write_user_sample(eid, payload)
   ▼
DcpsRuntime::write_user_sample (crates/dcps/src/runtime.rs)
   │ 1. read-lock the user_writers RwLock, find Arc<Mutex<Slot>>
   │    for `eid`, drop read-lock
   │ 2. lock the per-slot Mutex (per-slot, no global contention)
   │ 3. update slot.last_write for the deadline timer
   │ 4. branch on size:
   │    - small (≤ 1.5 KiB): write_user_sample_pooled — uses a
   │      stack-allocated PoolBuffer<1536> for encap framing,
   │      no heap touch in the framing step
   │    - big: Vec::with_capacity fallback
   ▼
ReliableWriter::write (crates/rtps/src/reliable_writer.rs)
   │ 1. assign next sequence number, atomic increment
   │ 2. Arc::from(&[u8]) — single allocation for fanout
   │ 3. cache.insert(CacheChange::alive_arc(...)) — BTreeMap
   │    insert + atomic stats refresh
   │ 4. for each ReaderProxy that is "in sync" with the cache:
   │    next_unsent_change advances; build a DATA submessage
   │    and an OutboundDatagram for that proxy's targets
   ▼
back in DcpsRuntime: drop slot Mutex, then
   │ for each OutboundDatagram:
   │   - per-target security wrap (if security feature on)
   │   - send_on_best_interface routes to the right outbound
   │     socket (multi-interface binding pool)
   ▼
UDP socket (transport-udp)
   │ sendto(2)
   ▼
the wire
```

Key points:

- The hot path takes **two locks** (registry read + slot mutex)
  and the slot lock is per-writer, so two app threads writing to
  different topics never serialize.
- For small samples there is **one** heap allocation in the path:
  `Arc::from(&[u8])`. Everything else is stack or refcount ops.
  See for the journey
  from "every sample = 3 allocations" to "every small sample = 1".
- The cache update updates atomic stats (`HistoryCacheStats`), so
  monitoring threads can poll latency / fill-level numbers without
  taking the writer lock.

## On the receiver

```
the wire
   │ recvfrom(2) on the user-data unicast socket
   ▼
DcpsRuntime event-loop (crates/dcps/src/runtime.rs)
   │ handle_user_datagram(parsed)
   │ for each submessage:
   │   - DATA / DATA_FRAG: look up reader_slot(reader_id),
   │     lock per-slot mutex, call reader.handle_data(d), push
   │     each delivered sample to the slot's mpsc::Sender
   │   - HEARTBEAT: same lookup, reader.handle_heartbeat
   │   - GAP, ACKNACK, NACK_FRAG: handled at the matching slot
   ▼
mpsc::Receiver in app thread
   │ rx.recv() returns a Vec<u8> (CDR-encoded sample)
   ▼
app code (decodes via the IDL-generated reader stub)
```

The reader-side hot path is symmetric to the writer side:
per-submessage slot lookup, no global lock.

## What's *not* on the hot path

- Discovery (SPDP/SEDP) is on a separate thread, runs every 5 s
  by default.
- Liveliness assertions and deadline checks run on the
  event-loop thread at `tick_period` (default 50 ms).
- Cache eviction (lifespan, KeepLast) runs at tick.
- Statistics aggregation is opportunistic — atomic counters,
  read by monitoring without disturbing the hot path.

## Reading further

- The `ReliableWriter` state machine is described in detail in
  the rustdoc on `crates/rtps/src/reliable_writer.rs`.
- The history cache, including the new lock-free read variant,
  is in `crates/rtps/src/history_cache.rs`.
- The full event-loop sequence (what runs at every tick) is in
  the `event_loop` function inside `crates/dcps/src/runtime.rs`.
