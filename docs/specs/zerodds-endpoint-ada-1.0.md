<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-ada` v1.0 — Ada Endpoint-SDK (XRCE-Framing, sync/async, reliable Stream)

ZeroDDS Vendor-Spec. Implementiert in `endpoints/ada/`.

Ergänzt die Codegen-Spec [`zerodds-xcdr2-ada`](zerodds-xcdr2-ada-1.0.md): dort das
Wire-Mapping IDL→Ada (ADR 0013 Stage 1: `Interfaces.C`-Bindings über den
C89-Wire-Core `zdw`), hier der Endpoint-Baustein, der ein XCDR2-marshaltes Sample
als XRCE-Frame über einen UDP-Transport trägt — sync über ein protected
`Mailbox`-Objekt, async über einen Ada-Task, optional reliable (§4).

## §1 XRCE-Framing

Ein WRITE_DATA-Sample wird als 8-Byte-Header + Body gerahmt (DDS-XRCE 1.0
§8.3.2/§8.3.4), byte-identisch zu `crates/xrce` + `endpoints/c`:

```
[session][stream][seq_lo][seq_hi][0x07][flags][len_lo][len_hi][body...]
```

- `session` — No-Key-Session (`0x80`).
- `stream` — `0x01` (best-effort) auf dem Sync/Async-Pfad; `stream_id ≥ 128`
  (Bit 7 gesetzt) auf dem reliable Pfad (§4, siehe DDS-XRCE 1.0 §8.3.2).
- `seq` — 16-Bit-LE-Sequenznummer.
- Submessage-Id `0x07` (WRITE_DATA); `len` — 16-Bit-LE-Body-Länge.
- `body` — das XCDR2-kodierte Sample, gebunden über `endpoints/ada/src/zdw.ads`
  (Package `Zdw`, `Interfaces.C`-FFI über den C89-Wire-Core), byte-identisch zur
  `zerodds-cdr`-Referenz (siehe `zerodds-xcdr2-ada-1.0` §3/§5).

`Deep_Reading.Frame` / `Deep_Reading.Deframe` (`endpoints/ada/src/deep_reading.ads`)
MÜSSEN diesen Rahmen für den best-effort Stream (`0x01`) bauen bzw. parsen;
`Reliable.Write_Frame` (`endpoints/ada/src/reliable.adb`) für den reliable Stream
(`stream_id = 0x80`, §4).

## §2 sync Client

Ein blockierender Empfangspfad über das idiomatische Ada-Concurrency-Primitiv: das
protected object `Deep_Reading.Mailbox`.

MUSS:

- `Mailbox.Deliver(Frame)` — legt einen empfangenen, dekodierten Rahmen in die
  protected FIFO ab.
- `Mailbox.Receive(Frame)` / `Mailbox.Try_Receive(Frame, Success)` — der
  blockierende bzw. nicht-blockierende Entry; der Integrator besitzt den
  Poll-/Wait-Loop, kein Hintergrund-Thread auf diesem Pfad.

Referenzpfad: [`example_sync.adb`](../../endpoints/ada/test/example_sync.adb) — ein
Poll-Loop über `Mailbox.Try_Receive`, vollständiger Felddecode. Ping-Pong-Nachweis
gegen den geteilten Rust-XRCE-Peer: `crates/endpoint-e2e/tests/ada.rs` (sync-Modus,
echter UDP-Socket).

## §3 async Reader/Writer

Ein Hintergrund-Reader als eigener Ada-Task (`Reader_Task`) — kein Async-Runtime
jenseits von `task`/`protected object`, das idiomatische Ada-Concurrency-Modell.

MUSS:

- `Reader_Task` — drained den Transport in einer Schleife, dekodiert jeden
  WRITE_DATA-Body und legt ihn per `Mailbox.Deliver` ab.
- Der aufrufende Task blockiert auf `Mailbox.Receive` (derselbe Entry wie in §2) —
  kein separater Callback-Mechanismus.

Referenzpfad: [`example_async.adb`](../../endpoints/ada/test/example_async.adb) —
ein `Reader_Task` + eine protected `Mailbox`; der Haupt-Task blockiert auf
`Inbox.Receive`. Ping-Pong-Nachweis: `crates/endpoint-e2e/tests/ada.rs`
(async-Modus).

Ein entkoppelter async **Writer** ist kein eigener Baustein dieser Ebene, sondern
Teil des reliable Streams (§4).

## §4 reliable Stream

Der reliable Stream ist eine optionale Erweiterung von §1–§3: State-Machine,
Wire-Codec und async-entkoppelter Writer nach
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md). Ada implementiert den
kanonischen Vertrag (§3 dort) in `endpoints/ada/src/reliable.{ads,adb}`:

- Sender — `Submit`/`Pending_Heartbeat`/`Recv_Acknack`/`Get_In_Flight`.
- Receiver — `Recv_Data`/`Drain_Next`/`Pending_Acknack`/`Reset`.
- Wire-Codec — `Heartbeat_Frame`/`Acknack_Frame` + die zugehörigen `Parse_*`-
  Funktionen; byte-identisch zu `golden_heartbeat_le.bin`/`golden_acknack_le.bin`.
- `Reliable.Send_Ring` — ein protected Ring (`Enqueue`/`Dequeue`/`Close`) als
  async-entkoppelter Writer: der Producer macht nur ein wait-freies `Enqueue`,
  kein Syscall; ein dedizierter Drain-Task
  ([`example_reliable.adb`](../../endpoints/ada/test/example_reliable.adb),
  `GNAT.Sockets`) besitzt Socket und Sender-State und macht die gesamte I/O (Send,
  Heartbeat, ACKNACK-getriebenes Retransmit).

Konstanten, Fehlerfälle, Wire-Format und Test-/Belegpflicht sind in
`reliable-endpoint-1.0` §3–§5 normativ definiert und gelten unverändert für Ada.
