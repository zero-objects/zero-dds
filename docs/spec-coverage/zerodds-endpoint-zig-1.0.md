# `zerodds-endpoint-zig` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-zig-1.0.md` — ZeroDDS Zig Endpoint-SDK-Spec
(XRCE-Framing, sync `Client`, async `Reader`/`Writer`, reliable Stream via
[`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md)). Ergänzt die
Codegen-Coverage `zerodds-xcdr2-zig` (`docs/spec-coverage/zerodds-xcdr2-zig-1.0.md`) —
dort das Marshalling, hier der Transport.

Implementation:

- `endpoints/zig/` — das pure-Zig-Endpoint-SDK (ADR 0013: from-scratch XCDR-Wire-Core,
  kein C): `src/zerodds.zig` (XRCE-Framing, `Client`, `AsyncReader`), `src/reliable.zig`
  (reliable State-Machine + async-entkoppelter `AsyncWriter`), `example_sync.zig` /
  `example_async.zig` / `example_reliable.zig`.
- `crates/endpoint-e2e/tests/zig.rs` — das Live-Ping-Pong-E2E (raw/sync/async) gegen den
  geteilten Rust-XRCE-Peer.
- `crates/endpoint-e2e/tests/zig_reliable.rs` — das reliable-E2E (Loss-Recovery,
  lossless Baseline, Example, Latenz-Bench).

## §1 XRCE-Framing

### §1 WRITE_DATA/DATA-Rahmen (8-Byte-Header + Body)

**Spec:** 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07` WRITE_DATA,
flags, len LE) + Body, byte-identisch zu `crates/xrce` + `endpoints/c`.

**Repo:** `endpoints/zig/src/zerodds.zig` — `xrceWriteFrame`/`xrceReadFrame`,
Konstanten `XRCE_SESSION_NOKEY` (`0x80`) und `XRCE_STREAM_BEST_EFFORT` (`0x01`);
`Transport` als Funktionszeiger-Vtable (`deliver`/`receive`, kein Heap).

**Tests:** In-File-Test `byte identity vs Rust goldens` in `zerodds.zig`
(`Writer` LE+BE gegen `build/golden_le.bin`/`golden_be.bin`).

**Status:** done.

### §1 Roher Codec ohne XRCE-Frame (Ping-Pong-E2E)

**Spec:** Nachweis, dass der generierte Sample-Codec (`zerodds-xcdr2-zig`) auch ohne
XRCE-Framing über einen echten Kanal roundtrippt — die Grundlage, auf der das Framing
aufsetzt.

**Repo:** `crates/endpoint-e2e/tests/zig.rs::ZIG_RAW_MAIN` — rohes UDP, kein
XRCE-Frame, nutzt nur `gen.zig` (den generierten `Ping`/`Pong`-Codec).

**Tests (codepit):** `zig_raw_udp` — generierter `Ping`/`Pong`-Codec direkt über einen
rohen UDP-Socket, ohne XRCE-Framing. Grün.

**Status:** done.

## §2 sync Client

### §2 `Client.write`/`poll`

**Spec:** Ein blockierender Pull-Pfad: `write(sample)` framet + liefert über den
`Transport`, `poll()` empfängt ein Frame und liefert den entrahmten Body.

**Repo:** `endpoints/zig/src/zerodds.zig::Client` (`write`/`poll`, `seq`-Zähler,
feste `txbuf`/`rxbuf`, kein Heap).

**Tests:** In-File-Test `sync loopback (pull)` (In-Memory-FIFO-Transport, 5 Samples
roundtrip); Live-E2E `crates/endpoint-e2e/tests/zig.rs::ZIG_ENDPOINT_MAIN` (Modus
`sync`) über `UdpTransport` + `zerodds.zig`.

**Tests (codepit):** `zig_endpoint_sync` — voller Stack über `Client.write`/`poll`
gegen den geteilten Rust-XRCE-Peer. Grün.

**Status:** done.

## §3 async Reader/Writer

### §3 `AsyncReader` (Callback-Reaktor)

**Spec:** Ein Callback-Reaktor (push) als Gegenstück zum sync `poll` — Zig hat
kein async/await, der Reaktor dispatcht jedes bereite Frame an einen
Consumer-Callback.

**Repo:** `endpoints/zig/src/zerodds.zig::AsyncReader` (`on_sample`-Callback,
`poll()` dispatcht ein Frame, `run(max)` drained bis `max` Frames oder bis
nichts mehr bereit ist).

**Tests:** In-File-Test `async loopback (push / callback reactor)` (5 Samples,
`Collector.on`-Callback sammelt IDs in Reihenfolge); Live-E2E
`crates/endpoint-e2e/tests/zig.rs::ZIG_ENDPOINT_MAIN` (Modus `async`) über
`UdpTransport` + `zerodds.zig`.

**Tests (codepit):** `zig_endpoint_async` — voller Stack über `AsyncReader.run`
gegen den geteilten Rust-XRCE-Peer. Grün.

**Status:** done.

**Anmerkung:** ein eigenständiger best-effort `AsyncWriter` (ohne Reliability) ist für
Zig nicht angelegt — die async-entkoppelte Schreibseite wird ausschließlich vom
reliable `AsyncWriter` (§4) abgedeckt (Spec §3, `docs/specs/zerodds-endpoint-zig-1.0.md`).
Kein offener Punkt, sondern eine bewusste Spec-Entscheidung.

**Ping-Pong-E2E-Summe (§1/§2/§3):** `zig_raw_udp`/`zig_endpoint_sync`/`zig_endpoint_async`
— 3/3 grün (codepit).

## §4 reliable Stream

### §4 State-Machine (Sender + Receiver) + Byte-golden HEARTBEAT/ACKNACK

**Spec:** XRCE reliable Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die
Referenz `crates/xrce/src/reliable.rs`: `Sender.submit`/`pendingHeartbeat`/
`recvAckNack`/`getInFlight` (History + Retransmit); `Receiver.recvData`/
`drainInto`/`pendingAckNack`/`reset` (Reorder + Dedup). Window 16, Receiver-Buffer
64, Heartbeat 500 ms, Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. HEARTBEAT
(`0x0B`) und ACKNACK (`0x0A`) byte-identisch zu den Referenz-Goldens des C-SDKs.
Volle Kontraktdetails in [`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md).

**Repo:** `endpoints/zig/src/reliable.zig` — `writeDataFrame`/`parseWriteData`,
`heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAckNack`; `Sender`, `Receiver`.

**Tests (codepit):** `zig_reliable_unit` — `zig test` auf `reliable.zig`: die
Mirror-of-Reference-Unit-Suite (monotone seq, Payload-zu-groß, Window-full, Heartbeat
first/silence/leer, ACKNACK Teil-/Voll-Clear, Receiver Reorder/Dedup/Buffer-full,
Pending-ACKNACK-Bitmap, Reset, In-Process-End-to-End-Loss-Recovery) plus die
Byte-Golden-Assertion in derselben `zig test`-Runde:
`heartbeatFrame(1,3)` == `80 00 01 00 0b 01 05 00 01 00 03 00 80`,
`acknackFrame(1,0)` == `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identisch
zu den Referenz-Goldens. Grün.

**Status:** done.

### §4 async-entkoppelter `AsyncWriter` (SPSC-Ring) + Loss-Recovery-E2E

**Spec:** Der Producer enqueued wait-free in einen SPSC-Ring, ein dedizierter
Drain-Thread hält den `Sender`-State und macht die gesamte I/O (senden, Heartbeat,
ACKNACK-getriebenes Retransmit) — der Producer geht nie in den Kernel. Reliable
Delivery überlebt Datagramm-Verlust — verifiziert live gegen den geteilten
Rust-Peer mit injiziertem Loss.

**Repo:** `endpoints/zig/src/reliable.zig::AsyncWriter` (wait-freier SPSC-Ring
`RING_CAP=1024`/`SLOT_CAP=512` via `std.atomic.Value`, `write`/`pop`/`drainLoop`/
`drainAckNacks`/`close`); `endpoints/zig/example_reliable.zig` (lauffähige
In-Process-Demo, kein Socket).

**Tests (codepit):**
- `zig_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die App
  retransmittet auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert.
- `zig_reliable_no_loss` — lossless Baseline; 12/12.
- `zig_reliable_example` — `example_reliable.zig` läuft und meldet
  `sequence 0..11 verified in order`.

3/3 grün (codepit) — zusammen mit `zig_reliable_unit` (§4 oben) 4/5 des reliable-Tests;
Latenz-Bench ist §5.

**Anmerkung (ehrlich):** Beim Aufbau von `AsyncWriter.close()` wurde ein
Shutdown-Deadlock in `drainLoop` gefunden und gefixt — ein hängendes Fenster
ohne eingehende ACKNACKs durfte `close()` nicht blockieren; `drainLoop` prüft
`running` jetzt in jeder Iteration statt nur beim Warten auf ein ACKNACK.

**Status:** done.

## §5 Latenz

### §5 SPSC-Ring-Push vs. inline `sendto`

**Spec:** Der Producer-Pfad des `AsyncWriter` (`write` → Ringslot-Memcpy +
Release-Store) muss messbar unter dem inline `sendto`-Syscall liegen — das ist
der Beleg, dass Async-Write die Syscall-Latenz aus dem Producer-Pfad nimmt,
nicht das Warten auf ACKNACK.

**Repo:** `crates/endpoint-e2e/tests/zig_reliable.rs::runBench` (Zig-App,
Modus `bench`): 4000 Iterationen inline `sendto` vs. 4000 Iterationen
`AsyncWriter.write` nach 100 Warmup-Pushes.

**Tests (codepit):** `zig_reliable_latency_bench` — Producer-Push (wait-free
Ring) **14 ns** vs. inline `sendto` **7950 ns** (~568×), Assert
`decoupled_ns < inline_ns`. Grün.

**Status:** done.

---

## Audit-Status

7 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-run (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test zig`
3/3 (Ping-Pong: `zig_raw_udp`/`zig_endpoint_sync`/`zig_endpoint_async`);
`--test zig_reliable` 5/5 (`zig_reliable_unit` inkl. Byte-Golden,
`zig_reliable_loss_recovery`, `zig_reliable_no_loss`, `zig_reliable_example`,
`zig_reliable_latency_bench`); Latenz-Bench decoupled 14 ns / inline
`sendto` 7950 ns (~568×).

Offene Punkte: keine.
