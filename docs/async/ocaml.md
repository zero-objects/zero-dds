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

`Zerodds.Wire` builds the XCDR primitives on a growable `Bytes` with alignment
relative to the writer's origin — growable and not a `Buffer`, because the
DHEADER, the NEXTINT and a parameter length are all slots filled in after the
body and `Buffer` cannot write into a position it has passed. `f32` goes
through `Int32.bits_of_float` (single-format IEEE bits), `u64` and the
eight-octet `bitset` anchor through `Int64` — OCaml's own `int` is 63 bits, so
`0x8000000000000000` has no `int` and a path through one would drop the bit
that distinguishes it from zero.

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

## Tests (CI job `endpoints-ocaml`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`make test`)
- the runnable example (`make example`)
- the XCDR surface against Cyclone, RTI, Fast DDS and omniORB octets
  (`make surface_test`)

Toolchain: `ocaml-nox` + `ocaml-findlib` from apt; built with
`ocamlfind ocamlopt -package threads.posix -linkpkg`.
