<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Zig (native endpoint)

A native **pure-Zig** endpoint SDK (ADR 0013): a from-scratch XCDR wire-core, no
C, byte-identical to the Rust core. Zig has no `async`/`await`, so the two
interfaces are **sync (pull)** and **async (push, a callback reactor)** —
allocation-free, no threads.

Module: [`endpoints/zig/src/zerodds.zig`](../../endpoints/zig/src/zerodds.zig) ·
example: [`endpoints/zig/example`](../../endpoints/zig/example) (`zig build run`).

## Sync (pull)

```zig
var c = zerodds.Client{ .transport = &transport };
_ = c.write(sample_xcdr);          // frame as XRCE WRITE_DATA + deliver
if (c.poll()) |body| {             // one non-blocking receive
    var r = zerodds.Reader.init(body, .little);
    const id = r.getU32();
}
```

## Async (push — callback reactor)

```zig
var reader = zerodds.AsyncReader{
    .transport = &transport, .on_sample = onSample, .ctx = &ctx,
};
_ = reader.run(0);                 // drain: dispatch each body to onSample
```

The `Transport` is a small function-pointer vtable (`deliver` / `receive`) — the
one integration point; `receive` returns `null` for "nothing ready".

## Tests (CI job `endpoints-zig`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (callback reactor)
- the runnable example (`zig build run`)

Built with Zig 0.13.
