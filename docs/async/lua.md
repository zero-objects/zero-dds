<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Lua (native endpoint)

A native **pure-Lua** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core
built on `string.pack`/`string.unpack` (Lua 5.3+), byte-identical to the Rust
core and the other SDKs. **No C FFI needed** — Lua's own binary packing carries
the wire. **Sync** (`poll`) and **async** — a coroutine producer, the idiomatic
Lua concurrency model.

Sources: [`endpoints/lua/zerodds.lua`](../../endpoints/lua/zerodds.lua) ·
example: [`endpoints/lua/example.lua`](../../endpoints/lua/example.lua)
(`lua5.4 example.lua`).

> The Lua community usually reaches for the C FFI, which is why the plan lists
> Lua as the FFI exception — but `string.pack` makes a genuinely native,
> byte-identical implementation possible with no FFI at all, so that is what
> ships here.

## Sync

```lua
local c = z.Client.new(transport)      -- transport: { deliver = fn, receive = fn }
c:write(sample)                        -- frame as XRCE WRITE_DATA + deliver
local body = c:poll()                  -- one non-blocking receive, or nil
```

## Async (coroutine producer)

```lua
local reader = z.asyncReader(transport) -- a coroutine.wrap producer
local body
repeat body = reader() until body ~= nil  -- resume to pull the next sample
```

`asyncReader` is a `coroutine.wrap` producer: resume it to get the next decoded
sample; it yields `nil` when the transport is momentarily empty, so a
cooperative consumer can retry or do other work in between.

## Wire-core

`Writer`/`Reader` use `string.pack`/`string.unpack` with the endian prefix
(`"<"`/`">"`) for the XCDR primitives — `f32` via `string.pack("<f", v)`, `u64`
via `"<i8"` (Lua 5.4 has 64-bit integers, and the signed format takes both
spellings of one pattern) — with alignment relative to the writer's origin.
Byte-identical to the Rust core. A float handed to an integer write is refused:
Lua has one number type with two subtypes, `2^53 + 1` is `9007199254740992.0`
because `^` always returns a float, and `string.pack` takes an integral float
without a word.

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

## Tests (CI job `endpoints-lua`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`lua5.4 test.lua`)
- the runnable example (`lua5.4 example.lua`)
- the XCDR surface against Cyclone, RTI, Fast DDS and omniORB octets
  (`lua5.4 surface_test.lua`)

Toolchain: `lua5.4` from apt (nothing else — pure Lua).
