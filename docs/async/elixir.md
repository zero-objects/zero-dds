<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Elixir (native endpoint)

A native **pure-Elixir** endpoint SDK (ADR 0013) on the BEAM — a from-scratch
XCDR wire-core, no NIF, byte-identical to the Rust core and the other SDKs.
**Sync** (`poll`) and **async** — a process that forwards decoded samples to a
consumer's mailbox, the idiomatic OTP concurrency model.

Sources: [`endpoints/elixir/lib/zerodds.ex`](../../endpoints/elixir/lib/zerodds.ex) ·
example: [`endpoints/elixir/example.exs`](../../endpoints/elixir/example.exs)
(`elixir -r lib/zerodds.ex example.exs`).

## Sync

```elixir
c = ZeroDDS.Client.new(transport)     # transport: %{deliver: fun/1, receive: fun/0}
c = ZeroDDS.Client.write(c, sample)   # frame as XRCE WRITE_DATA + deliver
body = ZeroDDS.Client.poll(c)         # one non-blocking receive, or nil
```

## Async (process/mailbox)

```elixir
reader = ZeroDDS.AsyncReader.start(transport, self())
receive do
  {:zerodds_sample, body} ->
    {id, _} = ZeroDDS.Wire.reader(body, :little) |> ZeroDDS.Wire.get_u32()
end
ZeroDDS.AsyncReader.stop(reader)
```

The reader is a plain BEAM process; decoded samples arrive as messages, so back
pressure, supervision, and distribution follow the usual OTP patterns.

## Wire-core

`ZeroDDS.Wire` uses Elixir's bitstring syntax (`<<v::little-32>>`,
`<<v::float-little-32>>`) for the XCDR primitives, with alignment relative to
the buffer start (cap 4) and `:big` handled by reversing the little-endian
encoding — byte-identical to the Rust core.

## Embedded (Nerves)

The wire-core is pure Elixir with no host dependencies, so it runs unchanged on
[Nerves](https://nerves-project.org/) firmware (Elixir on embedded Linux). No
separate build is wired here; the same `lib/zerodds.ex` compiles into a Nerves
project as-is.

## Tests (CI job `endpoints-elixir`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`elixir -r lib/zerodds.ex test.exs`)
- the runnable example (`elixir -r lib/zerodds.ex example.exs`)

Toolchain: `erlang` + `elixir` from apt.
