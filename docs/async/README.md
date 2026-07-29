<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — cross-language doc track

Async is a **vertical** feature: it cuts across every language surface, so it
gets its own doc track instead of being scattered per binding. This is the map;
each language page links the concrete API.

- Foundations: [ADR 0002 — runtime-agnostic async API](../adr/0002-async-api-runtime-agnostic.md)
- Spec: [`zerodds-async-1.0`](../specs/zerodds-async-1.0.md) · coverage audit: [status](../spec-coverage/zerodds-async-1.0.md)

## The model (same everywhere)

The async surface wraps the sync DCPS API without state duplication — the async
types share the same handle, so there is **no overhead** and both APIs interoperate.

- **Runtime-agnostic** — the core exposes `Future`/`Stream` (or the language's
  native equivalent), never pinning a runtime. Tokio/asyncio/CompletableFuture/…
  are the caller's choice.
- **Futures** — `write`, `take`, `read`, instance lifecycle
  (register/dispose/unregister), `wait_for_matched_*`.
- **Streams** — sample stream (`take_stream`), a with-`SampleInfo` variant,
  `data_available` events, and match-status changes.
- **Content filters** — a filtered reader (closure and SQL `ContentFilteredTopic`).
- **No thread block, no polling** — in live mode the reader wakes the future
  directly on sample arrival; offline falls back to a bounded poll.
- **Backpressure** — a reliable `write` suspends (not spins) under
  `RESOURCE_LIMITS` and resumes on drain.

## Status by surface

| Surface | Kind | Async status |
|---|---|---|
| **Rust** (`zerodds-dcps-async`) | binding | ✅ complete — [rust.md](rust.md) |
| **TypeScript** (`ts-node`) | binding | ✅ Promise + async-iterator surface — [ts.md](ts.md) |
| **C#** (`ZeroDDS`) | binding | ✅ Task + `IAsyncEnumerable` surface — [csharp.md](csharp.md) |
| **Python** (`zerodds`, pyo3) | binding | ✅ asyncio wrapper (`zerodds.aio`) — [python.md](python.md) |
| **Java** (`org.omg.dds`) | binding | ✅ `CompletableFuture` surface (`DdsAsync`) — [java.md](java.md) |
| **Native C** (`endpoints/c`) | native client | ✅ modern C11 async reactor (additive) — [c.md](c.md) |
| **Native C++** (`endpoints/cpp`) | native client | ✅ modern C++17 async facade (additive) — [cpp.md](cpp.md) |
| **Native Ada** (`endpoints/ada-native`) | native client | ✅ Object-Ada (OOP) async — [ada.md](ada.md) |
| **pre-Object Ada 83** (`endpoints/ada-83`) | native client | ✅ strict Ada-83 poll-based async — [ada-83.md](ada-83.md) |
| **Go** (`endpoints/go`) | native client | ✅ pure-Go, sync + async (goroutine/channel) — [go.md](go.md) |
| **Zig** (`endpoints/zig`) | native client | ✅ pure-Zig, sync (pull) + async (callback reactor) — [zig.md](zig.md) |
| **Kotlin** (`endpoints/kotlin`) | native client | ✅ pure-Kotlin/JVM, sync + async (thread + channel) — [kotlin.md](kotlin.md) |
| **Node** (`endpoints/node`) | native client | ✅ pure-JS, sync + async (async iterator) — [node.md](node.md) |
| **Elixir** (`endpoints/elixir`) | native client | ✅ pure-Elixir/BEAM, sync + async (process/mailbox) — [elixir.md](elixir.md) |
| **OCaml** (`endpoints/ocaml`) | native client | ✅ pure-OCaml, sync + async (Thread + mailbox) — [ocaml.md](ocaml.md) |
| **Julia** (`endpoints/julia`) | native client | ✅ pure-Julia, sync + async (Task + Channel) — [julia.md](julia.md) |
| **F#** (`endpoints/fsharp`) | native client | ✅ pure-F#/.NET, sync + async (MailboxProcessor agent) — [fsharp.md](fsharp.md) |
| **Nim** (`endpoints/nim`) | native client | ✅ pure-Nim, sync + async (asyncdispatch Future) — [nim.md](nim.md) |
| **D** (`endpoints/d`) | native client | ✅ pure-D, sync + async (std.concurrency actor) — [d.md](d.md) |
| **Lua** (`endpoints/lua`) | native client | ✅ pure-Lua (string.pack, no FFI), sync + async (coroutine) — [lua.md](lua.md) |
| **Swift** (`endpoints/swift`) | native client | ✅ pure-Swift, sync + async (AsyncStream) — [swift.md](swift.md) |

## Native clients — "native means native"

Native clients are **true in-language implementations** (byte-identical to the
Rust core, no C dependency), like `endpoints/ada-native` — not FFI shims. The
async add-on is layered on the transport's non-blocking receive as a
reactor/callback dispatch, then wrapped idiomatically per language
(`std::future` in C++, a dispatching tagged type in Object-Ada, `context`/
channels in Go, …). The conservative variants (C89 / C++98 / Ada-83) stay; the
modern async ones are **additional**.

Exception: **Lua** — that community expects C-FFI, so the FFI path is acceptable there.

### Language roadmap (curated by strategic surface)

| Tier | Languages | Why |
|---|---|---|
| 1 — core new | Go, Zig, Swift, Kotlin, TS/JS (Node) | real, broad surfaces |
| 2 — natural fit | **Elixir/Erlang** (BEAM/OTP = distributed reliable messaging, DDS's home domain; **Nerves** = embedded Elixir/edge), **OCaml** (high-assurance/formal, ties to the provable line), **Julia** (scientific/robotics, academic entry) | domain- or assurance-aligned |
| 3 — cheap / easy | **F#** (rides the .NET surface), Nim, D | low extra cost |
| FFI-ok | Lua | that community expects C-FFI |
| parked | Haskell, Simulink, LabVIEW, VHDL | thin pull or unassessed |
| dropped | Fortran, (Dart?) | no endpoint community |

The right filter is **paradigm-community pull**, not language count: the two with
genuine DDS/messaging/edge gravity are distributed-fault-tolerant (Elixir/Erlang)
and high-assurance-functional (OCaml). The rest is surface breadth, added
opportunistically.
