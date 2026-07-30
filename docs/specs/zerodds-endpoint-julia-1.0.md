<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-julia` v1.0 — Julia Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/julia/`.

Analog zu [`zerodds-xcdr2-julia`](zerodds-xcdr2-julia-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/d`, `endpoints/c`,
`endpoints/ada`, ...): das native pure-Julia-Endpoint über XRCE-Framing, den
pollenden `Client`, den `Task`+`Channel`-basierten `AsyncReader` sowie den
reliable Stream, so dass eine Julia-App ein XCDR2-Sample byte-identisch zu
`crates/xrce` + `endpoints/c` mit dem geteilten Rust-Peer austauscht.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/julia/zerodds.jl` (`module ZeroDDS`) MUSS bereitstellen:

- `write_frame(session, stream, seq, sample::Vector{UInt8}) -> Vector{UInt8}`
  — framet ein Sample.
- `read_frame(frame::Vector{UInt8}) -> Union{Vector{UInt8},Nothing}` —
  entrahmt; `nothing` bei zu kurzem Frame oder falscher Submessage-ID.
- Konstanten `SESSION_NOKEY` (`0x80`, best-effort, ohne ClientKey) und
  `STREAM_BEST_EFFORT` (`0x01`).

## §2 Sync `Client`

Ein pollender Client — Gegenstück zum `Task`-basierten `AsyncReader`: `write!`
framet + liefert synchron über den `Transport`; `poll` ist ein
nicht-blockierender Einzel-Receive (`nothing` bei leer).

`endpoints/julia/zerodds.jl` MUSS bereitstellen:

- `struct Transport(deliver::Function, receive::Function)` — der einzige
  Integrationspunkt; der Integrator liefert seine eigenen `deliver`/`receive`-
  Closures für seinen Link (z. B. UDP, oder die in-memory `mem_transport()`
  für Tests/Beispiele).
- `mutable struct Client(transport, session, stream, seq)` mit
  `Client(t::Transport)`, `write!(c::Client, sample::Vector{UInt8})`,
  `poll(c::Client) -> Union{Vector{UInt8},Nothing}`.
- Eine monoton wachsende Sequenznummer pro `write!` (16-bit-Wraparound); per
  Default `SESSION_NOKEY`/`STREAM_BEST_EFFORT`.

## §3 Async `Reader`/`Writer`

Das idiomatische Julia-Async-Modell: ein `@async`-`Task` pollt den `Transport`
und schiebt entrahmte Sample-Bodies auf einen gepufferten `Channel` (push);
der Consumer blockiert auf `take!`. Es gibt keinen separaten `AsyncWriter`-Typ
— Senden bleibt `write!` auf dem sync `Client` (gleiches Framing, kein
eigener Rückkanal-State nötig); der reliable `ReliableAsyncWriter`-Vorgang
(§4) ist der Ort, an dem eine Sende-Entkopplung normativ verlangt ist.

`endpoints/julia/zerodds.jl` MUSS bereitstellen:

- `mutable struct AsyncReader(ch::Channel{Vector{UInt8}}, running::Ref{Bool})`
  mit `start_reader(t::Transport) -> AsyncReader` (spawnt den Empfangs-`Task`
  per `@async`, drainiert den `Transport` in einer Schleife mit
  Backoff-`sleep` bei leerem Poll) und `stop!(r::AsyncReader)` (setzt
  `running[] = false`, der `Task` beendet sich selbst).
- `recv(r::AsyncReader) -> Vector{UInt8}` — blockierendes `take!(r.ch)`.
- Der `AsyncReader` teilt sich das XRCE-Framing aus §1 und denselben
  `Transport`-Vertrag wie der sync `Client`.

## §4 Reliable Stream

`endpoints/julia/reliable.jl` (`module Reliable`) implementiert den reliable
Stream als Endpoint-Fähigkeit gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— Sender-/Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `ReliableAsyncWriter`-Vorgang: ein producer-seitiger
`Channel` (wait-free Enqueue aus Sicht des Producers) plus ein dedizierter
Drain-`Task` (`@async`), der den `UDPSocket` und den reliable Sender-State
besitzt, sendet, HEARTBEATs emittiert und auf ACKNACK retransmittiert —
der Producer geht nie in den Kernel.

Die Konstanten (`SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`HEARTBEAT_PERIOD_MS=500`, `MAX_PAYLOAD=65535`, reliable Stream-ID `0x80`),
der State-Machine-Kontrakt (`submit!`/`pending_heartbeat!`/`recv_acknack!`/
`get_in_flight` auf dem `Sender`; `recv_data!`/`drain_in_order!`/
`pending_acknack`/`reset!` auf dem `Receiver`) und das Wire-Format
(HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit Sequenznummern via
`seq_lt`/`seq_gt`) sind dort normativ definiert; `endpoints/julia/reliable.jl`
ist die Julia-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK. Das Modul ist
in `module Reliable` gekapselt, um Namenskollisionen mit `module ZeroDDS`
(`write_frame`, `Client`, ...) zu vermeiden.

## §5 Conformance

Eine Julia-Endpoint-Implementierung ist konform, wenn:

1. Der volle Stack (generierte Typen + `endpoints/julia`) ein typisiertes
   Sample sowohl über den sync `Client` als auch über den `Task`/`Channel`-
   basierten `AsyncReader` mit dem geteilten Rust-XRCE-Peer über einen echten
   UDP-Socket austauscht.
2. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
3. Ein Latenz-Messwert zeigt, dass `ReliableAsyncWriter`-Enqueue (`put!` auf
   den producer-seitigen `Channel`) messbar unter einem inline `send`
   (UDP-`sendto`) liegt — der Beleg, dass Async-Write die Syscall-Latenz aus
   dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/julia/example_sync.jl` — Poll-Loop, vollem Feld-Decode.
- Async: `endpoints/julia/example_async.jl` — `Task`/`Channel`-`AsyncReader`.
- Reliable: `endpoints/julia/example_reliable.jl` — In-Process-Demo (kein
  Socket) der Loss-Recovery.
- Quickstart: `endpoints/julia/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe `docs/spec-coverage/zerodds-endpoint-julia-1.0.md`).
Der `ReliableAsyncWriter`-Producer-Pfad nutzt Julias `Channel` (Lock+Condvar,
kein wait-free SPSC-Ring); das Enqueue bleibt trotzdem messbar günstiger als
der inline Syscall (§5.3) — ein echter wait-free Ring wäre eine separate
Optimierung, kein Spec-Verstoß (siehe `endpoints/julia/reliable_app.jl`).
