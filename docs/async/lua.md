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
via `"<I8"` (Lua 5.4 has 64-bit integers) — with alignment relative to the
buffer start (cap 4). Byte-identical to the Rust core.

## Tests (CI job `endpoints-lua`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`lua5.4 test.lua`)
- the runnable example (`lua5.4 example.lua`)

Toolchain: `lua5.4` from apt (nothing else — pure Lua).
