<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — OCaml (native endpoint)

A native **pure-OCaml** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core,
no C binding, byte-identical to the Rust core and the other SDKs. **Sync**
(`poll`) and **async** — a `Thread` filling a `Mutex`/`Condition` mailbox, from
the stdlib (no Lwt, no Async).

Sources: [`endpoints/ocaml/zerodds.ml`](../../endpoints/ocaml/zerodds.ml) ·
example: [`endpoints/ocaml/example.ml`](../../endpoints/ocaml/example.ml)
(`make example`).

## Sync

```ocaml
let c = Zerodds.Client.create transport   (* { deliver : bytes -> unit; receive : unit -> bytes option } *)
Zerodds.Client.write c sample;            (* frame as XRCE WRITE_DATA + deliver *)
match Zerodds.Client.poll c with          (* one non-blocking receive, or None *)
| Some body -> ...
| None -> ()
```

## Async (Thread + mailbox)

```ocaml
let r = Zerodds.AsyncReader.start transport in
let body = Zerodds.AsyncReader.recv r in  (* blocks on the mailbox *)
Zerodds.AsyncReader.stop r
```

`AsyncReader` spawns a `Thread` that polls the transport and pushes decoded
samples into a `Mutex`/`Condition` mailbox; `recv` blocks until one arrives.
Stays on the stdlib, so it composes with any concurrency choice the caller makes.

## Wire-core

`Zerodds.Wire` builds the XCDR primitives on `Buffer`/`Bytes` with alignment
relative to the buffer start (cap 4). `f32` goes through
`Int32.bits_of_float` (single-format IEEE bits), `u64` through `Int64` — both
byte-identical to the Rust core.

## Tests (CI job `endpoints-ocaml`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`make test`)
- the runnable example (`make example`)

Toolchain: `ocaml-nox` + `ocaml-findlib` from apt; built with
`ocamlfind ocamlopt -package threads.posix -linkpkg`.
