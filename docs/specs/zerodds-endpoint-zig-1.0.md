<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-zig` v1.0 — Zig Endpoint-SDK: XRCE-Framing, sync/async, reliable Stream

**Status:** normativ · ADR 0013 (native Endpoint-SDKs, kein Voll-Stack).

ZeroDDS Vendor-Spec. Implementiert in `endpoints/zig/`. Baut auf DDS-XRCE 1.0 §8.3
(Message-/Submessage-Framing, WRITE_DATA/DATA) und [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md)
(reliable Stream, §8.4.10/§8.4.11) auf. Ergänzt die Codegen-Spec
[`zerodds-xcdr2-zig`](zerodds-xcdr2-zig-1.0.md) (Marshalling der Sample-Bodies) um den
Transport: Framing, sync/async Zustellung, reliable Delivery.

## §1 XRCE-Framing

Der Endpoint ist transport-opak (ADR 0013, Invariante 5): er framet und entrahmt, liefert
das vollständige Frame aber über einen vom Integrator gefüllten `Transport` aus — kein
eigener Socket im SDK.

Der pure-Zig-Wire-Core (`endpoints/zig/src/zerodds.zig`, Modul `zerodds`) MUSS
bereitstellen:

- `XRCE_SESSION_NOKEY: u8 = 0x80` und `XRCE_STREAM_BEST_EFFORT: u8 = 0x01`.
- `xrceWriteFrame(out, session, stream, seq, sample) usize` — baut den 8-Byte-XRCE-Header
  (`session_id`, `stream_id`, `sequence_nr` LE, WRITE_DATA-Submessage-Id `0x07`, Flags,
  Länge LE) + den XCDR2-Sample-Body byte-identisch zu `crates/xrce` und `endpoints/c`.
- `xrceReadFrame(frame) ?[]const u8` — die byte-exakte Inverse: entrahmt WRITE_DATA- und
  DATA-Messages (§8.3.4) gleichermaßen und liefert den Body.
- `Transport` als Funktionszeiger-Vtable (`deliver`/`receive`), ohne Heap-Allokation —
  der Integrator bindet den realen Kanal (In-Memory, UDP, seriell, …).

## §2 sync Client

Ein blockierender Pull-Pfad für Aufrufer, die die Run-Loop selbst besitzen (kein
async/await in Zig).

`Client` MUSS bereitstellen:

- `write(sample) bool` — framet den XCDR2-Body mit monotoner `seq` und liefert ihn über
  den `Transport`.
- `poll() ?[]const u8` — empfängt ein Frame über den `Transport` und liefert den
  entrahmten Body, oder `null`.

Feste `txbuf`/`rxbuf` (keine Laufzeit-Allokation für Framing).

## §3 async Reader/Writer

Zig hat kein async/await; der async Empfangspfad ist ein Callback-Reaktor als
Gegenstück zum sync `poll`.

`AsyncReader` MUSS bereitstellen:

- `OnSample = *const fn (ctx: *anyopaque, body: []const u8) void` — der
  Consumer-Callback.
- `poll() bool` — dispatcht höchstens ein bereites Frame an `on_sample`.
- `run(max) usize` — drained bis zu `max` Frames oder bis der `Transport` nichts mehr
  liefert; Rückgabe = Anzahl dispatchter Frames.

Ein eigenständiger best-effort `AsyncWriter` (Gegenstück ohne Reliability) ist für Zig
nicht Teil dieses SDKs — die async-entkoppelte Schreibseite wird ausschließlich vom
reliable `AsyncWriter` (§4) abgedeckt, dessen wait-freier Producer-Pfad denselben
Latenz-Vorteil liefert, plus reliable Delivery.

## §4 reliable Stream

Der reliable Stream (Sender-/Receiver-State-Machine, Wire-Format, Async-Writer-Kontrakt,
Test- und Belegpflicht) ist sprachübergreifend in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md)
normiert. Die Zig-Implementierung (`endpoints/zig/src/reliable.zig`) MUSS diesen Kontrakt
1:1 spiegeln:

- `Sender.submit`/`pendingHeartbeat`/`recvAckNack`/`getInFlight` — §3.2 der Referenz.
- `Receiver.recvData`/`drainInto`/`pendingAckNack`/`reset` — §3.3 der Referenz.
- `writeDataFrame`/`heartbeatFrame`/`acknackFrame` (+ Parser) — byte-golden gegen die
  Referenz-Goldens, §4 der Referenz.
- `AsyncWriter` — wait-freies SPSC-Ring-Enqueue (`RING_CAP`/`SLOT_CAP`) auf dem
  Producer-Pfad, ein Drain-Thread hält den `Sender`-State und macht die gesamte I/O
  (Send, Heartbeat, ACKNACK-getriebenes Retransmit); der Producer geht nie in den
  Kernel — §2 der Referenz.
- Konstanten `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`, `MAX_PAYLOAD=65535`,
  `HEARTBEAT_PERIOD_MS=500`, `RELIABLE_STREAM_ID=0x80` — §3.1 der Referenz.

Test- und Belegpflicht (Unit, Byte-Golden, E2E-Loss-Recovery, Latenz-Bench, Example) —
§5 der Referenz.
