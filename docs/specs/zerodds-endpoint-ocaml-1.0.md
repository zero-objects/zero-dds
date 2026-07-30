<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-ocaml` v1.0 — OCaml Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/ocaml/`.

Analog zu [`zerodds-xcdr2-ocaml`](zerodds-xcdr2-ocaml-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): das
native, pure-OCaml-Endpoint (ADR 0013) über XRCE-Framing, sync `Client`, async
`AsyncReader` (ein `Thread`, der eine Mutex/Condition-Mailbox füllt) und den
reliable Stream, so dass eine OCaml-App ein XCDR2-Sample byte-identisch zu
`crates/xrce` + `endpoints/c` mit dem geteilten Rust-Peer austauscht — ohne
Lwt oder Async, nur die Stdlib (`Thread`, `Mutex`, `Condition`, `Unix`) über
`threads.posix`.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/ocaml/zerodds.ml` (Modul `Endpoint`) MUSS bereitstellen:

- `write_frame : int -> int -> int -> bytes -> bytes` — framet ein Sample
  (`session`, `stream`, `seq`, `sample`).
- `read_frame : bytes -> bytes option` — entrahmt; `None` bei zu kurzem Frame
  oder falscher Submessage-ID.
- Konstanten `session_nokey` (`0x80`, best-effort, ohne ClientKey) und
  `stream_best_effort` (`0x01`).

## §2 Sync `Client`

Ein blockierender Client — Gegenstück zum Thread-basierten `AsyncReader`:
`write` framet + liefert synchron über den `transport`; `poll` ist ein
nicht-blockierender Einzel-Receive.

`endpoints/ocaml/zerodds.ml` MUSS bereitstellen:

- `type transport = { deliver : bytes -> unit; receive : unit -> bytes option }`
  — der einzige Integrationspunkt; der Integrator implementiert ihn für seinen
  Link (z. B. UDP über `Unix.sendto`/`Unix.recvfrom`).
- `module Client` mit `create : transport -> t`, `write : t -> bytes -> unit`,
  `poll : t -> bytes option`.
- Eine monoton wachsende Sequenznummer (16-bit-Wraparound) pro `write`;
  `session`/`stream` per Default `session_nokey`/`stream_best_effort`.

## §3 Async `Reader`/`Writer`

Das idiomatische OCaml-Stdlib-Async-Modell (kein Lwt, kein Async): ein
`Thread` pollt den `transport` und legt entrahmte Sample-Bodies in eine
`Mailbox` (Mutex + Condition, FIFO-Liste) ab; der Consumer blockiert in
`recv` auf `Condition.wait`.

`endpoints/ocaml/zerodds.ml` MUSS bereitstellen:

- `module Mailbox` — generische Mutex/Condition-FIFO (`put`, `take`), die
  Grundlage sowohl des Ping-Pong-`AsyncReader` als auch (in `reliable.ml`)
  des `ReliableAsyncWriter`.
- `module AsyncReader` mit `start : transport -> t` (spawnt den
  Empfangs-`Thread`), `recv : t -> bytes` (blockiert bis zu einem
  entrahmten Sample) und `stop : t -> unit` (stoppt die Poll-Schleife).
- Das Senden über den async Pfad teilt sich das XRCE-Framing aus §1 und den
  `transport`-Vertrag mit dem sync `Client`: eine App framet mit
  `Endpoint.write_frame` und liefert direkt über `transport.deliver`
  (kein separates `AsyncWriter`-Modul für den unreliable Pfad — siehe §4 für
  den reliable Async-Writer mit eigenem Drain-`Thread`).

## §4 Reliable Stream

`endpoints/ocaml/reliable.ml` implementiert den reliable Stream als
Endpoint-Fähigkeit gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— Sender-/Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `Reliable.Writer`, dessen Drain-`Thread` den UDP-`Unix`-
Socket und den reliable Sender-State besitzt, während der Producer nur
wait-free auf eine Mutex/Condition-`Mailbox` enqueued (`Mailbox.put` — Mutex
sperren, Cons, Signal, entsperren — nie in den Kernel geht).

Die Konstanten (`window=16`, `recv_buf=64`, `heartbeat_period=0.5s`,
`max_payload=65535`, reliable Stream-ID `0x80`), der State-Machine-Kontrakt
(`Sender.submit`/`pending_heartbeat`/`recv_acknack`/`get_in_flight`;
`Receiver.recv_data`/`drain_in_order`/`pending_acknack`/`reset`) und das
Wire-Format (HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit
Sequenznummern) sind dort normativ definiert; `endpoints/ocaml/reliable.ml`
ist die OCaml-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK. Das Modul heißt
`Reliable` (file-as-module) und hängt bewusst nicht von `zerodds.ml` ab —
beide definieren unabhängig ein `module Wire`/eigene Wire-Primitiven, damit
eine App, die generierten `idlc`-Code (der ebenfalls ein `module Wire`
exportiert) neben dem SDK linkt, nicht kollidiert.

## §5 Conformance

Eine OCaml-Endpoint-Implementierung ist konform, wenn:

1. Der volle Stack (generierte Typen aus `zerodds-idlc --ocaml` +
   `endpoints/ocaml`) ein typisiertes Sample sowohl über den sync `Client`
   als auch über `AsyncReader` mit dem geteilten Rust-XRCE-Peer über einen
   echten UDP-Socket austauscht.
2. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
3. Ein Latenz-Messwert zeigt, dass `Reliable.Writer.enqueue`
   (`Mailbox.put` — Mutex+Cons+Signal, kein Syscall) messbar unter einem
   inline `Unix.sendto` liegt — der Beleg, dass Async-Write die
   Syscall-Latenz aus dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/ocaml/example_sync.ml` — `Client.poll` in einer Schleife,
  vollem Feld-Decode.
- Async: `endpoints/ocaml/example_async.ml` — `AsyncReader.start` +
  blockierendes `recv` auf der Mutex/Condition-Mailbox.
- Reliable: `endpoints/ocaml/example_reliable.ml` — In-Process-Demo (kein
  Socket) der Loss-Recovery.
- Quickstart: `endpoints/ocaml/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe `docs/spec-coverage/zerodds-endpoint-ocaml-1.0.md`).
