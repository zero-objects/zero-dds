# `zerodds-endpoint-java` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-java-1.0.md` — ZeroDDS Java Endpoint-SDK-Spec.
Reliable-Stream-Kontraktdetails in `docs/spec-coverage/reliable-endpoint-1.0.md`.
Hier der Transport; das Marshalling erzeugt der gemeinsame Codegen
(`crates/lang-rules`, Spalte `Language::Java`) und nicht mehr ein
Java-eigener Generator. Die frühere Codegen-Coverage `zerodds-xcdr2-java`
ist mit `crates/idl-java` entfallen (Commit `fa2ed8b0a9c`).

Implementation:

- `endpoints/java/ZdwEndpoint.java` (Default-Package) — XRCE-Framing
  (`xrceWriteFrame`/`xrceReadBody`), serielles Framing nach Annex C
  (`serialFrame`/`serialDeframe`/`fcs16`).
- `endpoints/java/Zdw.java` (Default-Package) — Wire-Core (`Writer`/`Reader`,
  XCDR2).
- `endpoints/java/org/zerodds/endpoint/ReliableWire.java` — reliable
  Wire-Codec (HEARTBEAT/ACKNACK/WRITE_DATA) + RFC-1982-Vergleiche.
- `endpoints/java/org/zerodds/endpoint/ReliableSender.java` /
  `ReliableReceiver.java` — reliable Sender-/Receiver-State-Machine.
- `endpoints/java/org/zerodds/endpoint/AsyncReliableWriter.java` —
  async-entkoppelter reliable Writer (`BlockingQueue` + Drain-`Thread`).
- `crates/endpoint-e2e/tests/java.rs` — Ping-Pong-E2E; `crates/endpoint-e2e/tests/java_reliable.rs` —
  reliable-Stream-E2E + Latenz-Vergleich.
- `endpoints/java/EndpointTest.java` — Byte-Golden für WRITE_DATA/serial/DATA/HEARTBEAT-Read/ACKNACK
  (in der Spalte `java` von `tools/endpoints-gate` gegen die Goldens aus
  `cargo run -p zerodds-endpoint-golden`).

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`; DATA-Empfangspfad (id `0x09`) über denselben Unwrap; serielles
Framing nach Annex C.1 (Adressen, Länge, FCS-16 nach RFC 1662).

**Repo:** `endpoints/java/ZdwEndpoint.java` — `xrceWriteFrame`/`xrceReadBody`,
Konstanten `SESSION_NOKEY` (`0x80`)/`STREAM_BEST_EFFORT` (`0x01`),
`serialFrame`/`serialDeframe`/`fcs16`, `heartbeatRead`,
`acknackFrame`.

**Tests:** `endpoints/java/EndpointTest.java` in der Spalte `java` von
`tools/endpoints-gate` (Job `integration-endpoints`, `java EndpointTest
"$goldens"`) gegen die fünf mit `zerodds-endpoint-golden` erzeugten
Golden-Dateien (`golden_xrce_le.bin`, `golden_serial_le.bin`,
`golden_data_le.bin`, `golden_heartbeat_le.bin`, `golden_acknack_le.bin`).
Anders als `ReliableSelfTest` trägt `EndpointTest` keine fest verdrahteten
Ersatz-Goldens. Der Rahmen des SDK gegen den Decoder von `zerodds-xrce`:
`crates/endpoint-e2e/tests/xrce_write_framing.rs::xrce_8_3_5_8_java_endpoint_frames_a_conformant_write_data`.

Lauf auf dem Linux-Testhost (`tools/endpoints-gate`, Stand `8bcb1d00161`):

```
XRCE WRITE_DATA byte-identical (52 bytes)
serial byte-identical
DATA receive: body ok
serial deframe+crc round-trip ok
wrong frameCheck refused
0x7E inside the frame refused
HEARTBEAT parsed: first=1 last=3
ACKNACK byte-identical
appended submessage bounded out
over-long length rejected
truncated header rejected
writer >0xFFFF refused
writer maximal body framed
ALL OK
```

Das Framing wird zusätzlich live über `xrce_8_3_5_8_java_sync_app_writes_the_ping_and_reads_the_pong`/`xrce_8_3_5_8_java_async_app_writes_the_ping_and_reads_the_pong`
(§4) geübt.

**Status:** done

## §2 sync send/recv

**Spec:** §2 — transport-opak, kein `Client`-Objekt: `ZdwEndpoint` ist
zustandslos; der Integrator besitzt den Run-Loop und ruft
`xrceWriteFrame`/`xrceReadBody` direkt gegen seinen eigenen Transport (Socket
oder In-Memory-Queue) auf.

**Repo:** `endpoints/java/ExampleSync.java` — In-Memory-`ArrayDeque`-Poll-Loop,
volles Feld-Decode (`Reading{id,value,label}`); `sync`-Modus in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA_TAIL` — blockierendes
`DatagramSocket.receive()` gegen den echten Rust-Peer.

**Tests:** `xrce_8_3_5_8_java_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 async Reader/Writer

**Spec:** §3 — Reader-`Thread` drainiert den Transport in eine
`BlockingQueue`, Consumer blockiert auf `take()`; keine dedizierte
`AsyncReader`/`AsyncWriter`-Klasse für den Plain-Pfad (anders als
`endpoints/go`) — der Integrator komponiert sie aus den zustandslosen
`ZdwEndpoint`-Framing-Methoden.

**Repo:** `endpoints/java/ExampleAsync.java` — Reader-`Thread` +
`LinkedBlockingQueue<byte[]>`-Inbox, volles Feld-Decode; `async`-Modus in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA_TAIL` — Reader-`Thread` liest vom
echten `DatagramSocket`, Consumer blockiert auf `take()`.

**Tests:** `xrce_8_3_5_8_java_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine Java-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus: generierter
`Ping`/`Pong`-Codec + Endpoint-SDK (`ZdwEndpoint`), sync und async.

**Repo:** `crates/endpoint-e2e/tests/java.rs` — `MAIN_JAVA_HEAD` (Imports und
Klassenkopf) und `MAIN_JAVA_TAIL` (Run-Loop, Modus `sync`/`async` per
CLI-Argument) umschließen die Member des generierten `ZdCodec`; `split_generated`
hebt sie aus der Klasse, weil der Generator sie `private static` schreibt und
sie sonst von außen nicht erreichbar wären. Kompiliert gegen `runtime/java`
(`org.zerodds.wire.*`) — die frühere Runtime `crates/java-omgdds` +
`crates/idl-java/runtime` ist mit dem IDL-Transplantat entfallen.

**Tests:** (`crates/endpoint-e2e/tests/java.rs`, Job `integration-bindings`)
- `xrce_8_3_5_8_java_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über blockierendes `DatagramSocket.receive()`.
- `xrce_8_3_5_8_java_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über Reader-`Thread`/`LinkedBlockingQueue`.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight`; `ReliableReceiver.recvData`/`drainInOrder`/
`pendingAcknack`/`reset`. Fenster 16, Receiver-Buffer 64, Heartbeat 500 ms,
Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der
async-entkoppelte `AsyncReliableWriter`: der Producer enqueued wait-free in
eine gepufferte `BlockingQueue` (`submit`/`offer`), ein dedizierter
Drain-`Thread` hält den `ReliableSender`-State und macht die gesamte I/O
(senden, Heartbeat, ACKNACK-getriebenes Retransmit) — der Producer geht nie
in den Kernel.

**Repo:** `endpoints/java/org/zerodds/endpoint/ReliableWire.java` —
`writeFrame`, `heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAckNack`,
`seqLt`/`seqGt`; `ReliableSender`, `ReliableReceiver`; `AsyncReliableWriter`
(`ArrayBlockingQueue<byte[]>` Kapazität 4096, Drain-`Thread`
`zdw-reliable-drain`, `submit`/`offer`/`finish`/`delivered`); Beispiel-App
`endpoints/java/ExampleReliable.java` (echter UDP-Peer, kein In-Process-Stub).

**Tests:**
- `endpoints/java/ReliableSelfTest.java` in der Spalte `java` von `tools/endpoints-gate`
  (Job `integration-endpoints`) — kompiliert und läuft
  `endpoints/java/ReliableSelfTest.java`: 33 `check()`-Assertionen über 17
  Testszenarien (Sender: monotone seq, in-flight count, payload-too-large,
  window-full, heartbeat first/silence/after-500ms, no-heartbeat-when-empty,
  acknack partial/full clear; Receiver: in-order drain, reorder, duplicate
  dropped, buffer-full, pending-acknack-Bitmap, reset; End-to-End
  Loss-Recovery in-process; Byte-Golden für HEARTBEAT/ACKNACK — hardcoded
  **und**, wenn `cargo run -p zerodds-endpoint-golden` verfügbar ist,
  zusätzlich gegen die generierten `golden_heartbeat_le.bin`/
  `golden_acknack_le.bin` geprüft). Byte-Golden: `HeartbeatFrame(1,3)` ==
  `80 00 01 00 0b 01 05 00 01 00 03 00 80`, `AckNackFrame(1,0)` ==
  `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identisch zu den
  Referenz-Goldens (gleiche Bytes wie `endpoints/go`). Ausgabe: `ALL OK`.
- `reliable_5_3_java_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die
  App, die der Test aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und `AsyncReliableWriter` baut, retransmittet
  auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert, der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`).

**Status:** done.

## §6 Latenz — `BlockingQueue`-Enqueue vs. inline `DatagramSocket.send`

**Spec:** §5.3 — der Producer-Pfad des `AsyncReliableWriter`
(`offer` → `BlockingQueue`-Push) muss messbar unter dem inline
`DatagramSocket.send`-Syscall liegen — der Beleg, dass Async-Write die
Syscall-Latenz aus dem Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/java/ReliableBench.java` — 20000 Iterationen inline
`DatagramSocket.send` (echter, gebundener Loopback-Zielsocket, nie gelesen)
vs. 20000 Iterationen `BlockingQueue.offer` (Drain-`Thread` hält die Queue
leer, wie `AsyncReliableWriter`s echter Drain-`Thread`), kein Live-Peer
nötig.

**Tests:** `reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/java_reliable.rs`, Job `integration-bindings`):
der Test prüft, dass der Queue-Enqueue-Median unter dem inline-send-Median
liegt. Früher gemessen:

```
producer latency: queue-enqueue median = 84 ns, inline-send median = 5917 ns (70x)
```

Median über 20000 Iterationen je Pfad, lokal gemessen (macOS, kein
Lauf auf dem Linux-Testhost) — Größenordnung deckt sich mit `endpoints/go`s
Linux-verifizierten 20–25 ns / 4360 ns (~175–220×); der Absolutwert ist
maschinen-/lastabhängig, das Verhältnis (Enqueue deutlich unter Syscall)
ist der Beleg.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-java-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 5 Tests grün.

5 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
