<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS OCaml endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id; value; label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/ocaml
make example_sync     # subscriber owns the run-loop and polls
make example_async    # subscriber blocks on a mailbox filled by a Thread
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.start` spawns a `Thread` that fills a
  Mutex/Condition mailbox; the consumer blocks on `AsyncReader.recv`. The
  idiomatic OCaml stdlib concurrency model — no Lwt, no Async.

## Threads

The wire-core is pure OCaml; `AsyncReader`/`Mailbox` use `Thread`, `Mutex`, and
`Condition`, so every target links `threads.posix`:

```sh
ocamlfind ocamlopt -thread -package threads.posix -linkpkg zerodds.ml example_sync.ml -o bin && ./bin
```

## Transport

`transport` is `{ deliver : bytes -> unit; receive : unit -> bytes option }`.
The examples use the in-memory `MemTransport` (a Mutex-guarded FIFO); a real UDP
or shared-memory link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Int32.bits_of_float`). The same types can be generated from IDL with
`zerodds-idlc --ocaml` — see
[`docs/specs/zerodds-xcdr2-ocaml-1.0.md`](../../docs/specs/zerodds-xcdr2-ocaml-1.0.md).
