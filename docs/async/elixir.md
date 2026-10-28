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
    {id, _} = ZeroDDS.Wire.reader(body, :little, :xcdr2) |> ZeroDDS.Wire.get_u32()
end
ZeroDDS.AsyncReader.stop(reader)
```

The reader is a plain BEAM process; decoded samples arrive as messages, so back
pressure, supervision, and distribution follow the usual OTP patterns.

## Wire-core

`ZeroDDS.Wire` uses Elixir's bitstring syntax (`<<v::little-32>>`,
`<<v::float-little-32>>`) for the XCDR primitives, with alignment relative to
the writer's origin and `:big` handled by reversing the little-endian encoding
— byte-identical to the Rust core. Elixir's integers are arbitrary precision
and a bitstring field truncates a value that does not fit without a word, so
every integer write carries a guard: `<<70000::little-16>>` is `<<0x70, 0x11>>`
and `put_u16(w, 70000)` raises instead. The BEAM has no infinity and no NaN, so
two of binary64's values have no Elixir literal at all.

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

## Embedded (Nerves)

The wire-core is pure Elixir with no host dependencies, so it runs unchanged on
[Nerves](https://nerves-project.org/) firmware (Elixir on embedded Linux). No
separate build is wired here; the same `lib/zerodds.ex` compiles into a Nerves
project as-is.

## Tests (CI job `endpoints-elixir`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`elixir -r lib/zerodds.ex test.exs`)
- the runnable example (`elixir -r lib/zerodds.ex example.exs`)
- the XCDR surface against Cyclone, RTI, Fast DDS and omniORB octets
  (`elixir -r lib/zerodds.ex surface_test.exs`)

Toolchain: `erlang` + `elixir` from apt.
