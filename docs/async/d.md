<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — D (native endpoint)

A native **pure-D** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core, no
C binding, byte-identical to the Rust core and the other SDKs. **Sync** (`poll`)
and **async** — a `std.concurrency` actor (`spawn`/`send`/`receive`, Tid message
passing), the idiomatic D concurrency model.

Sources: [`endpoints/d/zerodds.d`](../../endpoints/d/zerodds.d) · example:
[`endpoints/d/example.d`](../../endpoints/d/example.d) (`make example`).

## Sync

```d
auto c = new Client(transport);        // transport: Transport(deliver, receive)
c.write(sample);                       // frame as XRCE WRITE_DATA + deliver
auto body = c.poll();                  // one non-blocking receive, or null
```

## Async (std.concurrency actor)

```d
auto r = new AsyncReader();            // spawns a reader thread bound to thisTid
r.feed(frame);                         // send a frame to the reader
auto body = r.recv();                  // receiveOnly!(immutable(ubyte)[])
r.stop();
```

`AsyncReader` spawns a thread (`spawn(&readerLoop, thisTid)`) that receives
frames as messages, decodes them, and `send`s the sample bodies back to its
owner. Message passing is D's idiomatic concurrency; the payloads cross the
thread boundary as `immutable(ubyte)[]`, so no shared mutable state is involved.

## Wire-core

`Writer`/`Reader` build the XCDR primitives on a `ubyte[]` with alignment
relative to the buffer start (cap 4). `f32` goes through `*cast(uint*)&v`, `u64`
via native `ulong` — both byte-identical to the Rust core.

## Tests (CI job `endpoints-d`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`make test`)
- the runnable example (`make example`)

Toolchain: `gdc` (the GNU D compiler, part of GCC) from apt; links Phobos for
`std.concurrency`.
