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
alignment relative to the writer's origin. `f32` goes through
`reinterpret(UInt32, Float32(v))`, `u64` through `UInt64` — both byte-identical
to the Rust core. A float handed to an integer write is refused: `2^53 + 1` is
`9007199254740992.0` in Julia, because `^` returns a float whatever its
arguments, and the range arithmetic is `Int128` so it does not depend on what
`Int` is on the target.

The alignment cap is the encoding's: `Writer` and `Reader` take one of the two
XCDR versions and nothing has a default, because XTypes 1.3 §7.4.1 puts
MAXALIGN at eight under XCDR1 and four under XCDR2. Beside the eight primitive
writes it carries `double`, the DHEADER (§7.4.3.5), the member header in all
eight length codes (§7.4.3.4.2), the PL_CDR1 parameter list with both header
forms and the PID_LIST_END sentinel (§7.4.1.2), `wchar`/`wstring` at two
octets a unit with the length in octets, `bitset` at its anchor's own width,
`fixed<P,S>` as packed BCD (CDR §9.3.2.8) and `char` refused above U+00FF.
Each one has a reader, so a vendor's octets go in and the sample comes out.

Every integer write is bounded by the union of the signed and the unsigned
range of its width and refuses rather than masks; `surface_test` pins each
operation against octets Cyclone, RTI, Fast DDS and omniORB wrote, at both
ends of every range.

## Tests (CI job `endpoints-julia`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`julia test.jl`)
- the runnable example (`julia example.jl`)
- the XCDR surface against Cyclone, RTI, Fast DDS and omniORB octets
  (`julia surface_test.jl`)

Toolchain: the official Julia 1.10 tarball from julialang.org (no apt package).
Note: on hardened kernels that refuse an executable stack, the bundled libs need
`PT_GNU_STACK`'s exec bit cleared — [`clear_execstack.py`](../../endpoints/julia/clear_execstack.py)
does that (a no-op where the kernel doesn't restrict it).
