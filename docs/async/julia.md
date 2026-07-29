<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Julia (native endpoint)

A native **pure-Julia** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core,
no C binding, byte-identical to the Rust core and the other SDKs. **Sync**
(`poll`) and **async** — a `Task` filling a `Channel`, the idiomatic Julia
concurrency model.

Sources: [`endpoints/julia/zerodds.jl`](../../endpoints/julia/zerodds.jl) ·
example: [`endpoints/julia/example.jl`](../../endpoints/julia/example.jl)
(`julia example.jl`).

## Sync

```julia
c = ZeroDDS.Client(transport)          # transport: ZeroDDS.Transport(deliver, receive)
ZeroDDS.write!(c, sample)              # frame as XRCE WRITE_DATA + deliver
body = ZeroDDS.poll(c)                 # one non-blocking receive, or nothing
```

## Async (Task + Channel)

```julia
r = ZeroDDS.start_reader(transport)    # spawns an @async Task
body = ZeroDDS.recv(r)                 # take! from the channel (blocks)
ZeroDDS.stop!(r)
```

`start_reader` launches an `@async` Task that polls the transport and puts
decoded samples into a `Channel`; `recv` is a `take!` on that channel. Standard
Julia cooperative concurrency — composes with the caller's own tasks.

## Wire-core

`ZeroDDS.Writer`/`Reader` build the XCDR primitives on a `UInt8` buffer with
alignment relative to the buffer start (cap 4). `f32` goes through
`reinterpret(UInt32, Float32(v))`, `u64` through `UInt64` — both byte-identical
to the Rust core.

## Tests (CI job `endpoints-julia`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`julia test.jl`)
- the runnable example (`julia example.jl`)

Toolchain: the official Julia 1.10 tarball from julialang.org (no apt package).
Note: on hardened kernels that refuse an executable stack, the bundled libs need
`PT_GNU_STACK`'s exec bit cleared — [`clear_execstack.py`](../../endpoints/julia/clear_execstack.py)
does that (a no-op where the kernel doesn't restrict it).
