<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — F# (native endpoint)

A native **pure-F#** endpoint SDK (ADR 0013) on .NET — a from-scratch XCDR
wire-core, no C binding, byte-identical to the Rust core and the other SDKs.
**Sync** (`Poll`) and **async** — an `async { }` `MailboxProcessor` agent, the
idiomatic F# actor.

Sources: [`endpoints/fsharp/zerodds.fs`](../../endpoints/fsharp/zerodds.fs) ·
example: [`endpoints/fsharp/example.fsx`](../../endpoints/fsharp/example.fsx)
(`dotnet fsi example.fsx`).

## Sync

```fsharp
let c = Client(transport)              // transport: { Deliver: byte[] -> unit; Receive: unit -> byte[] option }
c.Write(sample)                        // frame as XRCE WRITE_DATA + deliver
match c.Poll() with                    // one non-blocking receive, or None
| Some body -> ...
| None -> ()
```

## Async (MailboxProcessor + async{})

```fsharp
let r = AsyncReader(transport)
let! body = r.RecvAsync()              // Async<byte[]> — composes with other async
// or synchronously:
let body = r.Recv()
```

`AsyncReader` is a `MailboxProcessor` agent: its `async { }` loop cooperatively
polls the transport (`Async.Sleep` between empty reads) and replies to each
`RecvAsync` request with the next decoded sample. `RecvAsync` returns an
`Async<byte[]>` that composes with the caller's own async workflows; `Recv` is
the synchronous wrapper.

## Wire-core

`Writer`/`Reader` build the XCDR primitives on a `ResizeArray<byte>` with
alignment relative to the buffer start (cap 4). `f32` goes through
`BitConverter.GetBytes` (normalised to little-endian) — byte-identical to the
Rust core.

## Tests (CI job `endpoints-fsharp`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`dotnet fsi test.fsx`)
- the runnable example (`dotnet fsi example.fsx`)

Toolchain: the .NET 8 SDK via the official `dotnet-install.sh` (the Debian
`fsharp`/Mono package was removed); `dotnet fsi` runs the scripts, no project
file needed.
