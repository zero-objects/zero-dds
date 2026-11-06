# `zerodds-endpoint-zig` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-zig-1.0.md` — ZeroDDS Zig Endpoint-SDK-Spec
(XRCE-Framing, sync `Client`, async `Reader`/`Writer`, reliable Stream via
[`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md)). Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Zig`;
hier steht der Transport.

Implementation:

- `endpoints/zig/` — das pure-Zig-Endpoint-SDK (ADR 0013: from-scratch XCDR-Wire-Core,
  kein C): `src/zerodds.zig` (XRCE-Framing, `Client`, `AsyncReader`), `src/reliable.zig`
  (reliable State-Machine + async-entkoppelter `AsyncWriter`), `example_sync.zig` /
  `example_async.zig` / `example_reliable.zig`.
- `crates/endpoint-e2e/tests/zig.rs` — das Live-Ping-Pong-E2E (raw/sync/async) gegen den
  geteilten Rust-XRCE-Peer.
- `crates/endpoint-e2e/tests/zig_reliable.rs` — das reliable-E2E (Loss-Recovery,
  Latenz-Vergleich).

## §1 XRCE-Framing

### §1 WRITE_DATA/DATA-Rahmen (8-Byte-Header + Body)

**Spec:** 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07` WRITE_DATA,
flags, len LE) + Body, byte-identisch zu `crates/xrce` + `endpoints/c`.

**Repo:** `endpoints/zig/src/zerodds.zig` — `xrceWriteFrame`/`xrceReadFrame`,
Konstanten `XRCE_SESSION_NOKEY` (`0x80`) und `XRCE_STREAM_BEST_EFFORT` (`0x01`);
`Transport` als Funktionszeiger-Vtable (`deliver`/`receive`, kein Heap).

**Tests:** `crates/endpoint-e2e/tests/xrce_write_framing.rs::xrce_8_3_5_8_zig_endpoint_frames_a_conformant_write_data`
baut den Frame mit `zig run` über die SDK-Datei im Baum und gibt ihn an
WriteDataPayload::try_from_submessage aus zerodds-xrce — dieselbe Prüfung, die jede
andere Sprachspalte des Gatters durchläuft; sie deckt Submessage-Id, Flag-Byte
(`0x01`), `BaseObjectRequest` und Sample ab. Dazu die In-File-Tests
`"byte identity vs Rust goldens"` in `endpoints/zig/src/zerodds.zig` (`Writer` LE+BE
gegen `build/golden_le.bin`/`build/golden_be.bin`),
`"a WRITE_DATA frame carries the BaseObjectRequest under FORMAT_DATA flags"`,
`"read frame bounds body to declared length, ignoring appended submessage"`
und `"read frame rejects over-long length and truncation, writer refuses >0xFFFF"`.

**Status:** done.

### §1 Roher Codec ohne XRCE-Frame (Ping-Pong-E2E)

**Spec:** Nachweis, dass der generierte Sample-Codec (`zerodds-xcdr2-zig`) auch ohne
XRCE-Framing über einen echten Kanal roundtrippt — die Grundlage, auf der das Framing
aufsetzt.

**Repo:** `crates/endpoint-e2e/tests/zig.rs::ZIG_RAW_MAIN` — rohes UDP, kein
XRCE-Frame, nutzt nur den generierten `Ping`/`Pong`-Codec aus
`crates/endpoint-e2e/tests/zig.rs::gen_module` (zur Testlaufzeit als gen.zig
ins Build-Verzeichnis geschrieben).

**Tests:** (Linux) `endpoint_c_3_4_zig_generated_codec_pings_without_the_xrce_frame` — generierter `Ping`/`Pong`-Codec direkt über einen
rohen UDP-Socket, ohne XRCE-Framing.

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

**Tests:** (Linux) `xrce_8_3_5_8_zig_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `Client.write`/`poll`
gegen den geteilten Rust-XRCE-Peer.

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

**Tests:** (Linux) `xrce_8_3_5_8_zig_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `AsyncReader.run`
gegen den geteilten Rust-XRCE-Peer.

**Status:** done.

**Anmerkung:** ein eigenständiger best-effort `AsyncWriter` (ohne Reliability) ist für
Zig nicht angelegt — die async-entkoppelte Schreibseite wird ausschließlich vom
reliable `AsyncWriter` (§4) abgedeckt (Spec §3, `docs/specs/zerodds-endpoint-zig-1.0.md`).
Kein offener Punkt, sondern eine bewusste Spec-Entscheidung.

**Ping-Pong-E2E-Summe (§1/§2/§3):** `endpoint_c_3_4_zig_generated_codec_pings_without_the_xrce_frame`/`xrce_8_3_5_8_zig_sync_app_writes_the_ping_and_reads_the_pong`/`xrce_8_3_5_8_zig_async_app_writes_the_ping_and_reads_the_pong`
im Job `integration-bindings-zig-nim`.

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

**Tests:** `crates/endpoint-e2e/tests/xrce_write_framing.rs::xrce_8_3_5_8_zig_reliable_frames_a_conformant_write_data`
— `writeDataFrame` hat eine eigene Spalte im Framing-Gatter, weil `reliable.zig` ein
eigenes Framing mit eigenen Konstanten ist und `zerodds.zig` nicht importiert; der
Frame geht durch WriteDataPayload::try_from_submessage aus zerodds-xrce. Dazu
`endpoints/zig/src/reliable.zig` in der Spalte `zig` von `tools/endpoints-gate`
(Job `integration-bindings-zig-nim`, `zig build test`, das `reliable.zig` über
`src/zerodds.zig` einbindet): die
Mirror-of-Reference-Unit-Suite (monotone seq, Payload-zu-groß, Window-full, Heartbeat
first/silence/leer, ACKNACK Teil-/Voll-Clear, Receiver Reorder/Dedup/Buffer-full,
Pending-ACKNACK-Bitmap, Reset, In-Process-End-to-End-Loss-Recovery) plus die
Byte-Golden-Assertion in derselben `zig test`-Runde:
`heartbeatFrame(1,3)` == `80 00 01 00 0b 01 05 00 01 00 03 00 80`,
`acknackFrame(1,0)` == `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identisch
zu den Referenz-Goldens.

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

**Tests:** (Linux)
- `reliable_5_3_zig_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die App
  retransmittet auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert.
- `endpoints/zig/example_reliable.zig` in der Spalte `zig` von `tools/endpoints-gate`
  (Job `integration-bindings-zig-nim`) — `example_reliable.zig` läuft und meldet
  `sequence 0..11 verified in order`.

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

**Tests:** (Linux) `reliable_5_4_zig_the_producer_enqueue_costs_less_than_the_inline_send` — Producer-Push (wait-free
Ring) **14 ns** vs. inline `sendto` **7950 ns** (~568×), Assert
`decoupled_ns < inline_ns`.

**Status:** done.

---

## Audit-Status

7 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-zig-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-zig-nim` (Job `integration-bindings-zig-nim`) — 10 Tests grün.

10 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
