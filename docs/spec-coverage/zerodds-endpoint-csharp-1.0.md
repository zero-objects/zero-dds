# `zerodds-endpoint-csharp` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-csharp-1.0.md` — ZeroDDS C#
Endpoint-SDK-Spec. Ergänzt die Codegen-Coverage `zerodds-xcdr2-csharp`
(`docs/spec-coverage/zerodds-xcdr2-csharp-1.0.md`) — dort das Marshalling,
hier der Transport.

Implementation:

- `endpoints/csharp/Zerodds.cs` (namespace `ZeroDDS`) — XCDR2-Wire-Core
  (`Writer`/`Reader`), XRCE-Framing (`Xrce.WriteFrame`/`Xrce.ReadFrame`),
  `ITransport`/`MemTransport`, sync `Client`, async `AsyncReader`.
- `endpoints/csharp/Reliable.cs` (namespace `ZeroDDS.Endpoint`) — reliable
  Sender/Receiver-State-Machine + HEARTBEAT/ACKNACK-Wire-Codec +
  `AsyncReliableWriter`/`ReliableWriterHandle`.
- `crates/endpoint-e2e/tests/csharp.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/csharp_reliable.rs` — reliable-Stream-E2E +
  Unit/Golden + Latenz-Bench.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/csharp/Zerodds.cs::Xrce` — `WriteFrame`/`ReadFrame`,
Konstanten `SessionNoKey` (`0x80`) und `StreamBestEffort` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/csharp.rs::csharp_raw_udp` (rohes
XCDR2 über einen bloßen UDP-Socket, kein XRCE-Frame — eigener Mini-Harness);
das Framing selbst wird über `csharp_endpoint_sync` und `csharp_endpoint_async`
(§4) live geübt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `Write` framet + liefert synchron,
`Poll` ist ein nicht-blockierender Einzel-Receive (`null` wenn nichts
ansteht).

**Repo:** `endpoints/csharp/Zerodds.cs::ITransport` (`Deliver`/`Receive`, der
einzige Integrationspunkt), `MemTransport` (Lock-basierte In-Memory-Referenz),
`Client` (`Client(ITransport)`, `Write`/`Poll`, monotoner `ushort`-Seq-Zähler,
Default-Session `Xrce.SessionNoKey`/`Xrce.StreamBestEffort`).

**Tests:** `endpoints/csharp/ExampleSync.cs` (5 `Reading`-Samples über
`MemTransport`, volles Feld-Decode, druckt `ALL OK`); Live-E2E
`csharp_endpoint_sync` (§4).

**Status:** done.

## §3 Async `Reader`

**Spec:** §3 — `AsyncReader.Stream()` liefert ein `IAsyncEnumerable<byte[]>`;
der Consumer iteriert mit `await foreach`. Kein separater `AsyncWriter`-Typ
für den Best-Effort-Pfad — Sendeseitig teilt sich async denselben
`Client.Write`.

**Repo:** `endpoints/csharp/Zerodds.cs::AsyncReader`
(`AsyncReader(ITransport)`, `Stream(CancellationToken)` — nicht-blockierender
Poll pro Tick, sonst `Task.Delay(1)`).

**Tests:** `endpoints/csharp/ExampleAsync.cs` (5 `Reading`-Samples,
`await foreach`, volles Feld-Decode, druckt `ALL OK`); Live-E2E
`csharp_endpoint_async` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1/§5.2 — eine C#-App tauscht mit dem geteilten Rust-XRCE-Peer
über einen echten UDP-Socket ein typisiertes Sample aus: einmal roher
generierter Codec ohne XRCE-Frame, einmal voller Stack (generierte Typen +
Endpoint-SDK) sync und async.

**Repo:** `crates/endpoint-e2e/tests/csharp.rs` — baut eine echte
`dotnet`-App (net8.0, `ProjectReference` auf das reale
`ZeroDDS.Cdr.csproj`), die einen `Ping` mit dem GENERIERTEN
`PingTypeSupport.Encode` kodiert, über `UdpTransport : ZeroDDS.ITransport`
(sync `Client.Write`/`Poll` bzw. async `AsyncReader.Stream`) verschickt und
den `Pong` mit `PongTypeSupport.Decode` dekodiert; Modus per CLI-Argument
(`sync`/`async`/`raw`).

**Tests (codepit, `dotnet`-gated):**
- `csharp_raw_udp` — generierter `Ping`/`Pong`-Codec direkt über einen rohen
  UDP-Socket, ohne XRCE-Framing.
- `csharp_endpoint_sync` — voller Stack über `ZeroDDS.Client`.
- `csharp_endpoint_async` — voller Stack über `ZeroDDS.AsyncReader`.

3/3 grün (codepit, `dotnet` >= .NET 8 auf PATH oder `~/.dotnet/dotnet`; kein
`dotnet` ⇒ lauter Skip, kein false-green).

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ReliableSender.Submit`/`PendingHeartbeat`/
`RecvAckNack`/`GetInFlight`/`InFlightSeqs`; `ReliableReceiver.RecvData`/
`DrainInOrder`/`PendingAckNack`/`Reset`. Window 16, Receiver-Buffer 64,
Heartbeat 500 ms, Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der
async-entkoppelte `AsyncReliableWriter`: der Producer enqueued über
`ReliableWriterHandle.Enqueue` wait-free in einen gebundenen
`System.Threading.Channels`-Ring (`TryWrite`, `false` bei vollem Ring), ein
dedizierter Drain-`Thread` hält den `ReliableSender`-State und den
UDP-`Socket` exklusiv und macht die gesamte I/O (Senden, Heartbeat,
ACKNACK-getriebenes Retransmit) — der Producer geht nie in den Kernel.

**Repo:** `endpoints/csharp/Reliable.cs` — `ReliableWire`
(`WriteFrame`/`TryUnframeWrite`, `HeartbeatFrame`/`TryParseHeartbeat`,
`AckNackFrame`/`TryParseAckNack`, Konstanten); `ReliableSender`,
`ReliableReceiver`; `WriterCloseState`, `ReliableWriterHandle`
(`Enqueue`/`Close`), `AsyncReliableWriter` (`Start`/`Shutdown`/`Dispose`,
Drain-`Thread` `zerodds-reliable-drain`); `endpoints/csharp/ExampleReliable.cs`
(alle drei Modi: E2E-Sender, Bench, In-Process-Demo).

**Tests (codepit, `dotnet`-gated):**
- `csharp_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/csharp_reliable.rs`) baut + läuft
  `endpoints/csharp/ReliableTests.cs` (`ZeroDDS.Tests.ReliableTests.Main`) —
  ein einzelner Treiber mit 32 `Check(...)`-Assertions über: Sender
  monotone Seq (`"monotonic seq 0"`/`"monotonic seq 1"`,
  `"in-flight count"`), Payload-zu-groß (`"payload too large"`), Window-Fill
  + Window-full (`"fill window"` × 16, `"window full"`), Heartbeat
  first/body/silenced/nach-500ms/leer (`"heartbeat fires first"`/
  `"heartbeat body"`/`"heartbeat silenced <500ms"`/`"heartbeat after 500ms"`/
  `"no heartbeat when empty"`), ACKNACK Teil-/Voll-Clear
  (`"acknack clears acked"`/`"seq2 retransmittable"`/
  `"acknack full clear"`), Receiver In-Order/Reorder/Duplikat-Drop/
  Buffer-full (`"in-order drain"`/`"expected advanced"`/
  `"reorder: only seq0"`/`"reorder: seq1+2"`/`"duplicate dropped"`/
  `"fill recv buffer"` × 64/`"recv buffer full"`), Pending-ACKNACK-Bitmap
  (`"slot 0 missing"`/`"slot 2 missing"`/`"slot 1 present"`/
  `"slot 3 present"`), Reset (`"reset clears receiver"`), In-Process
  Loss-Recovery End-to-End (`"only seq0 before recovery"`/
  `"seq1 retransmittable"`/`"seq1+2 after recovery"`), Byte-Golden
  (`"heartbeat byte-golden (hardcoded)"` == `80 00 01 00 0b 01 05 00 01 00
  03 00 80`, `"acknack byte-golden (hardcoded)"` == `80 00 01 00 0a 01 05 00
  01 00 00 00 80`; zusätzlich, wenn die Rust-Goldens per
  `zerodds-endpoint-golden` generiert werden konnten,
  `"heartbeat byte-identical to golden file"`/
  `"acknack byte-identical to golden file"` gegen die echten
  `golden_heartbeat_le.bin`/`golden_acknack_le.bin`). Druckt `ALL OK` bei
  0 Failures.
- `csharp_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die
  `ExampleReliable`-App (Modus `<port>`) retransmittiert via
  `AsyncReliableWriter` auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge
  geliefert (Assert auf `Reading.Id == Index`).
- `csharp_reliable_no_loss_baseline` — lossless Baseline; 12/12.
- `csharp_reliable_standalone_example` — `ExampleReliable` ohne Argumente
  (In-Process-Demo: lossy Receiver-Thread + `AsyncReliableWriter`-Sender)
  meldet `OK: 12 samples delivered gap-free in order despite injected loss`.

5/5 grün (codepit), davon 4 in diesem Abschnitt (Latenz-Bench in §6).

**Status:** done.

## §6 Latenz — Ring-Enqueue vs. inline `Socket.Send`

**Spec:** §5.4 — der Producer-Pfad des `AsyncReliableWriter`
(`ReliableWriterHandle.Enqueue` → Channel-Push) muss messbar unter dem
inline `Socket.Send`-Syscall liegen — der Beleg, dass Async-Write die
Syscall-Latenz aus dem Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/csharp/ExampleReliable.cs::Bench` — 50000 Iterationen
`ReliableWriterHandle.Enqueue` (Drain-Thread + separater Sink-Thread leeren
den Ring/Socket, damit `Enqueue` nie Backpressure trifft) vs. 50000
Iterationen inline `Socket.Send` (kein Live-Peer nötig, Loopback-Sink), via
`Stopwatch` gemessen (1 Tick = 100 ns).

**Tests (codepit):** `csharp_reliable_latency_bench`
(`crates/endpoint-e2e/tests/csharp_reliable.rs`) — Modus `bench`; Assert nur
auf Vorhandensein der `LATENCY`-Zeile (kein hartcodierter Schwellwert im
Rust-Test), Messwert wird geloggt. Beobachtete Größenordnung (codepit):
Ring-Enqueue **~6–10 µs** vs. inline `Socket.Send` **~442–504 µs**
(~50–70×) — der .NET-JIT/GC-Overhead macht die absoluten Werte gegenüber
Go/Rust höher, das Verhältnis (Enqueue ohne Kernel-Übergang vs. inline
Syscall) bleibt dieselbe Evidenz.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test csharp`
3/3 (Ping-Pong: `csharp_raw_udp`/`csharp_endpoint_sync`/`csharp_endpoint_async`);
`--test csharp_reliable` 5/5 (`csharp_reliable_loss_recovery`,
`csharp_reliable_no_loss_baseline`, `csharp_reliable_unit_and_golden` — 32
inline Assertions inkl. Byte-Golden, `csharp_reliable_standalone_example`,
`csharp_reliable_latency_bench`); Latenz-Bench Ring-Enqueue ~6–10 µs / inline
`Socket.Send` ~442–504 µs (~50–70×).

Gated auf `dotnet` (>= .NET 8, PATH oder `~/.dotnet/dotnet`); ohne Toolchain
lauter Skip, kein false-green.

Offene Punkte: keine.
