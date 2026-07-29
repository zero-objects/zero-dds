<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-go` v1.0 — Go Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/go/`.

Analog zu [`zerodds-xcdr2-go`](zerodds-xcdr2-go-1.0.md) (dort das Marshalling)
und den Endpoint-SDKs anderer Sprachen (`endpoints/zig`, `endpoints/nim`,
`endpoints/c`, `endpoints/ada`, ...): das native Go-Endpoint über XRCE-Framing,
sync `Client`, async `AsyncReader`/`AsyncWriter` und den reliable Stream, so
dass eine Go-App ein XCDR2-Sample byte-identisch zu `crates/xrce` +
`endpoints/c` mit dem geteilten Rust-Peer austauscht.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/go` (package `zerodds`) MUSS bereitstellen:

- `XrceWriteFrame(session, stream byte, seq uint16, sample []byte) []byte` —
  framet ein Sample.
- `XrceReadFrame(frame []byte) (body []byte, ok bool)` — entrahmt; `ok=false`
  bei zu kurzem Frame oder falscher Submessage-ID.
- Konstanten `XrceSessionNoKey` (`0x80`, best-effort, ohne ClientKey) und
  `XrceStreamBestEffort` (`0x01`).

## §2 Sync `Client`

Ein blockierender Client — Gegenstück zum goroutine-basierten
`AsyncReader`/`AsyncWriter`: `Write` framet + liefert synchron über den
`Transport`; `Poll` ist ein nicht-blockierender Einzel-Receive; `Receive`
blockiert bis zu einem Timeout (pollend).

`endpoints/go` MUSS bereitstellen:

- `type Transport interface { Deliver(frame []byte) error; Receive(buf []byte) (n int, again bool, err error) }`
  — der einzige Integrationspunkt; der Integrator implementiert ihn für seinen
  Link (z. B. UDP).
- `type Client struct { ... }` mit `NewClient(t Transport) *Client`,
  `(*Client).Write(sample []byte) error`, `(*Client).Poll() (body []byte, ok bool, err error)`,
  `(*Client).Receive(timeout time.Duration) (body []byte, ok bool, err error)`.
- Eine monoton wachsende Sequenznummer pro `Write`; `Session`/`Stream` per
  Default `XrceSessionNoKey`/`XrceStreamBestEffort`.

## §3 Async `Reader`/`Writer`

Das idiomatische Go-Async-Modell: eine Goroutine pollt den `Transport` und
schiebt entrahmte Sample-Bodies auf einen Channel (push); der Consumer rangt
über den Channel. Der `AsyncWriter` ist das Gegenstück auf der Sendeseite
(gleiches Framing wie `Client.Write`, aber ohne Rückkanal).

`endpoints/go` MUSS bereitstellen:

- `type AsyncReader struct { Samples chan []byte; ... }` mit
  `NewAsyncReader(t Transport) *AsyncReader` (spawnt die Empfangs-Goroutine)
  und `(*AsyncReader).Close()` (stoppt die Goroutine; `Samples` wird
  drainiert und geschlossen).
- `type AsyncWriter struct { ... }` mit `NewAsyncWriter(t Transport) *AsyncWriter`
  und `(*AsyncWriter).Write(sample []byte) error`.
- Beide teilen sich das XRCE-Framing aus §1 und denselben `Transport`-Vertrag
  wie der sync `Client`.

## §4 Reliable Stream

`endpoints/go` implementiert den reliable Stream als Endpoint-Fähigkeit gemäß
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `ReliableAsyncWriter`, dessen Drain-Goroutine den
`Transport` und den reliable Sender-State besitzt, während der Producer nur
wait-free auf einen gepufferten Channel enqueued (nie in den Kernel geht).

Die Konstanten (`SenderWindow=16`, `ReceiverBufferCap=64`,
`HeartbeatPeriod=500ms`, `ReliableMaxPayload=65535`, reliable Stream-ID
`0x80`), der State-Machine-Kontrakt (`Submit`/`PendingHeartbeat`/
`RecvAcknack`/`GetInFlight` auf dem Sender; `RecvData`/`DrainInOrder`/
`PendingAcknack`/`Reset` auf dem Receiver) und das Wire-Format
(HEARTBEAT `0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit Sequenznummern) sind dort
normativ definiert; `endpoints/go/reliable.go` ist die Go-Bindung dieses
Kontrakts, byte-identisch zu `crates/xrce/src/reliable.rs` und jedem anderen
Endpoint-SDK.

## §5 Conformance

Eine Go-Endpoint-Implementierung ist konform, wenn:

1. Ein rohes UDP-Ping-Pong mit dem generierten `MarshalXCDR`/`UnmarshalXCDR*`
   (ohne XRCE-Frame) mit dem Rust-Referenz-Peer byte-korrekt läuft.
2. Der volle Stack (generierte Typen + `endpoints/go`) ein typisiertes Sample
   sowohl über den sync `Client` als auch über `AsyncReader`/`AsyncWriter`
   mit dem geteilten Rust-XRCE-Peer austauscht.
3. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
4. Ein Latenz-Messwert zeigt, dass `ReliableAsyncWriter.Enqueue` (Ringpuffer-
   Push) messbar unter einem inline `sendto` liegt — der Beleg, dass
   Async-Write die Syscall-Latenz aus dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/go/example_sync` — Poll-Loop mit Deadline, vollem
  Feld-Decode.
- Async: `endpoints/go/example_async` — Goroutine/Channel-`AsyncReader`.
- Reliable: `endpoints/go/example_reliable` — In-Process-Demo (kein Socket)
  der Loss-Recovery.
- Quickstart: `endpoints/go/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe `docs/spec-coverage/zerodds-endpoint-go-1.0.md`).
