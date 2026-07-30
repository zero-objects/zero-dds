<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-elixir` v1.0 — Elixir Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/elixir/`.

Analog zu [`zerodds-xcdr2-elixir`](zerodds-xcdr2-elixir-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, ...): das native
Elixir-Endpoint über XRCE-Framing, sync `Client`, async
`AsyncReader`/`AsyncWriter` (BEAM-Prozess/Mailbox) und den reliable Stream, so
dass eine Elixir-App ein XCDR2-Sample byte-identisch zu `crates/xrce` +
`endpoints/c` mit dem geteilten Rust-Peer austauscht. Kein natives Addon, kein
NIF: Elixir-Binaries + Bitstring-Syntax tragen die Wire.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/elixir` (Modul `ZeroDDS.Endpoint`) MUSS bereitstellen:

- `write_frame(session, stream, seq, sample)` — framet ein Sample.
- `read_frame(frame)` — entrahmt; `:error` bei zu kurzem Frame oder falscher
  Submessage-ID.
- Konstanten `session_nokey/0` (`0x80`, best-effort, ohne ClientKey) und
  `stream_best_effort/0` (`0x01`).

## §2 Sync `Client`

Ein blockierender Client — Gegenstück zum Prozess-basierten `AsyncReader`:
`write/2` framet + liefert synchron über den `transport`; `poll/1` ist ein
nicht-blockierender Einzel-Receive (kein eingebautes `receive/2` mit Timeout —
der Aufrufer pollt selbst in einer Deadline-Schleife, siehe
`example_sync.exs`).

`endpoints/elixir` (Modul `ZeroDDS.Client`) MUSS bereitstellen:

- Einen `transport`-Vertrag `%{deliver: fun/1, receive: fun/0}` — der einzige
  Integrationspunkt; der Integrator implementiert ihn für seinen Link (z. B.
  `:gen_udp`, oder das In-Memory-`ZeroDDS.MemTransport` für Tests/Beispiele).
- `new(transport)` — Client mit Default-`session`/`stream`
  (`ZeroDDS.Endpoint.session_nokey/0`/`stream_best_effort/0`) und `seq: 1`.
- `write(client, sample)` — framet, ruft `transport.deliver.(frame)`,
  liefert den Client mit inkrementierter (modulo `0x10000`) Sequenznummer
  zurück (Elixir-Structs sind unveränderlich — jeder Call gibt den neuen
  Zustand zurück).
- `poll(client)` — ein nicht-blockierender Einzel-Receive über
  `transport.receive.()`; liefert den entrahmten Sample-Body oder `nil`.

## §3 Async `Reader`/`Writer`

Das idiomatische BEAM-Async-Modell: ein gespawnter Prozess pollt den
`transport` und sendet entrahmte Sample-Bodies als
`{:zerodds_sample, body}`-Message an das Ziel-`target` (push in die Mailbox
des Consumers) statt über einen Channel. Es gibt keinen separaten
`AsyncWriter`-Typ: der sync `Client` (§2) ist bereits der Sendepfad — Producer
und `AsyncReader` teilen sich denselben `transport`.

`endpoints/elixir` (Modul `ZeroDDS.AsyncReader`) MUSS bereitstellen:

- `start(transport, target)` — spawnt den Empfangs-Prozess (`spawn/1`), der in
  einer Schleife `transport.receive.()` pollt und bei einem entrahmten Frame
  `{:zerodds_sample, body}` an `target` sendet; bei leerem Poll ein kurzer
  `Process.sleep(1)` vor dem nächsten Versuch.
- `stop(pid)` — sendet `:zerodds_stop`; der Prozess terminiert nach der
  laufenden Poll-Iteration.
- Dasselbe XRCE-Framing aus §1 und denselben `transport`-Vertrag wie der sync
  `Client`.

## §4 Reliable Stream

`endpoints/elixir` implementiert den reliable Stream als Endpoint-Fähigkeit
gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine (Modul `ZeroDDS.Reliable`, `Sender`/`Receiver` als
unveränderliche Structs, da BEAM keine Mutation kennt — jeder Call fädelt den
Zustand durch und liefert den neuen zurück) sowie HEARTBEAT/ACKNACK-Wire-Codec.

Die Konstanten (`sender_window/0`=16, `receiver_buffer/0`=64,
`heartbeat_period_ms/0`=500, `max_payload/0`=65535, reliable Stream-ID
`0x80`), der State-Machine-Kontrakt (`Sender.submit/2`/`pending_heartbeat/2`/
`recv_acknack/2`/`get_in_flight/2` auf dem Sender; `Receiver.recv_data/3`/
`drain_in_order/1`/`pending_acknack/2`/`reset/1` auf dem Receiver) und das
Wire-Format (HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit
Sequenznummern) sind dort normativ definiert; `endpoints/elixir/lib/reliable.ex`
ist die Elixir-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK.

Der async-entkoppelte Sender ist `ZeroDDS.Reliable.Drain`, ein `GenServer`
statt eines wait-free Rings: der Producer `submit/2`t (ein `GenServer.cast`,
zahlt einen Mailbox-Send und kehrt sofort zurück, ohne den Kernel zu
betreten), der Drain-Prozess hält exklusiv den `Sender`-State und den
`:gen_udp`-Socket und erledigt Submit-in-History, WRITE_DATA-Send, periodisches
HEARTBEAT (Tick alle 50 ms) und ACKNACK-getriebenes Retransmit abseits des
Producer-Pfads. `finish/2` blockiert (`GenServer.call`), bis das Sendefenster
vollständig gedraint (alles acked) ist. Socket-Ownership-Reihenfolge (BEAM-
Eigenheit): `start_link` (kein Socket-I/O in `init/1`) → `:gen_udp.controlling_process`
(noch vom aktuellen Owner aufgerufen) → `activate/1` (übergibt dem
Drain-Prozess die Erlaubnis, `active: true` zu setzen und die Ticks zu
starten).

## §5 Conformance

Eine Elixir-Endpoint-Implementierung ist konform, wenn:

1. Ein rohes UDP-Ping-Pong mit dem generierten `marshal_xcdr`/`unmarshal`
   (ohne XRCE-Frame) mit dem Rust-Referenz-Peer byte-korrekt läuft.
2. Der volle Stack (generierte Typen + `endpoints/elixir`) ein typisiertes
   Sample sowohl über den sync `Client` als auch über den `AsyncReader`
   (Prozess/Mailbox) mit dem geteilten Rust-XRCE-Peer austauscht.
3. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt (§4,
   `reliable-endpoint` v1.0 §5).
4. Ein Latenz-Messwert zeigt, dass `ZeroDDS.Reliable.Drain.submit/2`
   (`GenServer.cast`, Mailbox-Send) messbar unter einem inline
   `:gen_udp.send` liegt — der Beleg, dass die BEAM-Message-Passing-Entkopplung
   die Syscall-Latenz aus dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/elixir/example_sync.exs` — Poll-Loop mit vollem
  Feld-Decode einer `Reading { id, value, label }`.
- Async: `endpoints/elixir/example_async.exs` — Prozess/Mailbox-`AsyncReader`,
  gleiches Telemetrie-Beispiel.
- Reliable: `endpoints/elixir/example_reliable.exs` — In-Process-Demo (keine
  Sockets) der Loss-Recovery über 12 Samples.
- Quickstart: `endpoints/elixir/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe
`docs/spec-coverage/zerodds-endpoint-elixir-1.0.md`).
