<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-csharp` v1.0 — C# Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/csharp/`.

Analog zu [`zerodds-xcdr2-csharp`](zerodds-xcdr2-csharp-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): das
native C#-Endpoint (ADR 0013, from-scratch, kein DDS-Vendor-Binding) über
XRCE-Framing, sync `Client`, async `AsyncReader`/`Stream()` (idiomatisches
`IAsyncEnumerable`) und den reliable Stream, so dass eine C#-App ein
XCDR2-Sample byte-identisch zu `crates/xrce` + `endpoints/c` mit dem
geteilten Rust-Peer austauscht.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/csharp` (namespace `ZeroDDS`) MUSS bereitstellen:

- `Xrce.WriteFrame(ushort seq, byte[] sample) -> byte[]` — framet ein Sample.
- `Xrce.ReadFrame(byte[] frame) -> byte[]?` — entrahmt; `null` bei zu kurzem
  Frame oder falscher Submessage-ID.
- Konstanten `Xrce.SessionNoKey` (`0x80`, best-effort, ohne ClientKey) und
  `Xrce.StreamBestEffort` (`0x01`).

## §2 Sync `Client`

Ein blockierender Client — Gegenstück zum `AsyncReader`: `Write` framet +
liefert synchron über den `ITransport`; `Poll` ist ein nicht-blockierender
Einzel-Receive (`null` wenn nichts ansteht).

`endpoints/csharp` MUSS bereitstellen:

- `interface ITransport { void Deliver(byte[] frame); byte[]? Receive(); }`
  — der einzige Integrationspunkt; der Integrator implementiert ihn für
  seinen Link (z. B. UDP, siehe `csharp_endpoint_sync`/`csharp_endpoint_async`
  in `crates/endpoint-e2e/tests/csharp.rs`).
- `sealed class Client` mit `Client(ITransport transport)`,
  `Write(byte[] sample) -> void`, `Poll() -> byte[]?`.
- Eine monoton wachsende `ushort`-Sequenznummer pro `Write`; `Session`/`Stream`
  per Default `Xrce.SessionNoKey`/`Xrce.StreamBestEffort`.
- `MemTransport` als lock-basierte In-Memory-Referenzimplementierung von
  `ITransport` (für Beispiele/Tests ohne echten Socket).

## §3 Async `Reader`/`Writer`

Das idiomatische C#-Async-Modell: `AsyncReader.Stream()` liefert ein
`IAsyncEnumerable<byte[]>`, das der Consumer mit `await foreach` iteriert —
keine separate Empfangs-Task, kein Channel; das Pollen liegt im
Enumerator selbst (nicht-blockierender `Receive()` pro Tick, sonst
`Task.Delay(1)` bis zur Cancellation).

`endpoints/csharp` MUSS bereitstellen:

- `sealed class AsyncReader` mit `AsyncReader(ITransport transport)` und
  `IAsyncEnumerable<byte[]> Stream(CancellationToken ct = default)`.
- Sendeseitig teilt sich der async Pfad den sync `Client.Write` (gleiches
  Framing, gleicher `ITransport`-Vertrag) — es gibt keinen separaten
  `AsyncWriter`-Typ für den Best-Effort-Stream; der reliable Stream hat
  seinen eigenen async-entkoppelten Writer (§4).

## §4 Reliable Stream

`endpoints/csharp` implementiert den reliable Stream als Endpoint-Fähigkeit
gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `AsyncReliableWriter`, dessen dedizierter Drain-`Thread`
den UDP-`Socket` und den reliable Sender-State besitzt, während der Producer
über `ReliableWriterHandle.Enqueue` wait-free (non-blocking `Channel`-Write,
`System.Threading.Channels`) einreiht und nie in den Kernel geht.

Die Konstanten (`ReliableWire.Window=16`, `ReliableWire.ReceiverBuffer=64`,
`ReliableWire.HeartbeatPeriod=500ms`, `ReliableWire.MaxPayload=65535`,
reliable Stream-ID `0x80`), der State-Machine-Kontrakt (`Submit`/
`PendingHeartbeat`/`RecvAckNack`/`GetInFlight`/`InFlightSeqs` auf
`ReliableSender`; `RecvData`/`DrainInOrder`/`PendingAckNack`/`Reset` auf
`ReliableReceiver`) und das Wire-Format (HEARTBEAT `0x0B`, ACKNACK `0x0A`,
RFC-1982 16-bit Sequenznummern über `ReliableWire.SeqLt`/`SeqGt`) sind dort
normativ definiert; `endpoints/csharp/Reliable.cs` (namespace
`ZeroDDS.Endpoint`) ist die C#-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK.

`AsyncReliableWriter.Start(Socket sock, int capacity = 1024)` spawnt den
Drain-`Thread` (Name `zerodds-reliable-drain`); `Handle.Enqueue(byte[])`
liefert `false` (Backpressure) bei vollem Ring; `Shutdown()`/`Dispose()`
signalisiert Close und blockt bis der Drain-Thread das Fenster geleert hat
oder eine 5-Sekunden-Deadline verstreicht (Best-effort-Ausstieg bei einem
verschwundenen Peer).

## §5 Conformance

Eine C#-Endpoint-Implementierung ist konform, wenn:

1. Ein rohes UDP-Ping-Pong mit dem generierten `TypeSupport.Encode`/`Decode`
   (ohne XRCE-Frame) mit dem Rust-Referenz-Peer byte-korrekt läuft.
2. Der volle Stack (generierte Typen + `endpoints/csharp`) ein typisiertes
   Sample sowohl über den sync `Client` als auch über `AsyncReader.Stream()`
   mit dem geteilten Rust-XRCE-Peer austauscht.
3. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
4. Ein Latenz-Messwert zeigt, dass `ReliableWriterHandle.Enqueue`
   (Channel-Push) messbar unter einem inline `Socket.Send` liegt — der
   Beleg, dass Async-Write die Syscall-Latenz aus dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/csharp/ExampleSync.cs` — `Client.Poll()`-Loop, vollem
  Feld-Decode (5 `Reading`-Samples über `MemTransport`).
- Async: `endpoints/csharp/ExampleAsync.cs` — `AsyncReader.Stream()` mit
  `await foreach`.
- Reliable: `endpoints/csharp/ExampleReliable.cs` — drei Modi: `<port>`
  (live UDP-Sender fürs E2E), `bench` (Enqueue- vs. inline-`Send`-Latenz),
  ohne Argumente (In-Process-Demo mit simuliertem Verlust).
- Quickstart: `endpoints/csharp/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe `docs/spec-coverage/zerodds-endpoint-csharp-1.0.md`).
