<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Nim (native endpoint)

A native **pure-Nim** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core,
no C binding (Nim compiles to C itself), byte-identical to the Rust core and the
other SDKs. **Sync** (`poll`) and **async** — `std/asyncdispatch`
(`async`/`await`/`Future`), the idiomatic Nim model.

Sources: [`endpoints/nim/zerodds.nim`](../../endpoints/nim/zerodds.nim) ·
example: [`endpoints/nim/example.nim`](../../endpoints/nim/example.nim)
(`nim c -r example.nim`).

## Sync

```nim
let c = newClient(transport)           # transport: Transport(deliver, receive)
c.write(sample)                        # frame as XRCE WRITE_DATA + deliver
let body = c.poll()                    # Option[seq[byte]] — none if nothing
```

## Async (asyncdispatch)

```nim
let r = newAsyncReader(transport)
let body = waitFor r.recv()            # recv(): Future[seq[byte]]
```

`recv` is an `{.async.}` proc: its loop polls the transport and `await
sleepAsync(1)` between empty reads, returning the next decoded sample as a
`Future[seq[byte]]` — composes with the caller's own async procs on the
`asyncdispatch` event loop.

## Wire-core

`Writer`/`Reader` build the XCDR primitives on a `seq[byte]` with alignment
relative to the buffer start (cap 4). `f32` goes through `cast[uint32](v)`,
`u64` via native `uint64` — both byte-identical to the Rust core.

## Tests (CI job `endpoints-nim`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`nim c -r test.nim`)
- the runnable example (`nim c -r example.nim`)

Toolchain: the official Nim 2.0 binary tarball from nim-lang.org (the Debian apt
package is absent here); Nim compiles to C via the system `gcc`.
