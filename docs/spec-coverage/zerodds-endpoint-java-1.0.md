# `zerodds-endpoint-java` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-java-1.0.md` — ZeroDDS Java Endpoint-SDK-Spec.
Reliable-Stream-Kontraktdetails in `docs/spec-coverage/reliable-endpoint-1.0.md`.
Ergänzt die Codegen-Coverage `zerodds-xcdr2-java`
(`docs/spec-coverage/zerodds-xcdr2-java-1.0.md`) — dort das Marshalling, hier
der Transport.

Implementation:

- `endpoints/java/ZdwEndpoint.java` (Default-Package) — XRCE-Framing
  (`xrceWriteFrame`/`xrceReadBody`), serielles HDLC-Framing
  (`serialFrame`/`serialDeframe`/`crc16CcittFalse`).
- `endpoints/java/Zdw.java` (Default-Package) — Wire-Core (`Writer`/`Reader`,
  XCDR2).
- `endpoints/java/org/zerodds/endpoint/ReliableWire.java` — reliable
  Wire-Codec (HEARTBEAT/ACKNACK/WRITE_DATA) + RFC-1982-Vergleiche.
- `endpoints/java/org/zerodds/endpoint/ReliableSender.java` /
  `ReliableReceiver.java` — reliable Sender-/Receiver-State-Machine.
- `endpoints/java/org/zerodds/endpoint/AsyncReliableWriter.java` —
  async-entkoppelter reliable Writer (`BlockingQueue` + Drain-`Thread`).
- `crates/endpoint-e2e/tests/java.rs` — Ping-Pong-E2E; `crates/endpoint-e2e/tests/java_reliable.rs` —
  reliable-Stream-E2E + Unit/Golden + Latenz-Bench.
- `endpoints/java/EndpointTest.java` — Byte-Golden für WRITE_DATA/serial/DATA/HEARTBEAT-Read/ACKNACK
  (manuell gegen `cargo run -p zerodds-endpoint-golden` gefahren, nicht über `cargo test` verdrahtet).

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`; DATA-Empfangspfad (id `0x09`) über denselben Unwrap; seriell
HDLC-Framing (Annex C, RFC 1662, CRC-16-CCITT-FALSE).

**Repo:** `endpoints/java/ZdwEndpoint.java` — `xrceWriteFrame`/`xrceReadBody`,
Konstanten `SESSION_NOKEY` (`0x80`)/`STREAM_BEST_EFFORT` (`0x01`),
`serialFrame`/`serialDeframe`/`crc16CcittFalse`, `heartbeatRead`,
`acknackFrame`.

**Tests:** `endpoints/java/EndpointTest.java` gegen echte, mit
`cargo run -p zerodds-endpoint-golden` erzeugte Rust-Goldens. Lokal
nachgefahren (dieses Environment, macOS, `javac`/`java` OpenJDK 21):

```
XRCE WRITE_DATA byte-identical (48 bytes)
serial byte-identical
DATA receive: body ok
serial deframe+crc round-trip ok
HEARTBEAT parsed: first=1 last=3
ACKNACK byte-identical
ALL OK
```

`EndpointTest.java` ist nicht über `cargo test`/CI verdrahtet (kein
Java-Äquivalent zum `make test GOLDEN_DIR=...` aus `endpoints/c/Makefile`) —
manueller Lauf, s.o.; das Framing selbst wird zusätzlich live über
`java_endpoint_sync`/`java_endpoint_async` (§4) geübt.

**Status:** partial — Byte-Golden lokal verifiziert (s.o.), aber
`EndpointTest.java` ist nicht über `cargo test`/CI verdrahtet, nur manuell
reproduzierbar (offen: Java-Äquivalent zu `endpoints/c/Makefile`s
`make test GOLDEN_DIR=...`).

## §2 sync send/recv

**Spec:** §2 — transport-opak, kein `Client`-Objekt: `ZdwEndpoint` ist
zustandslos; der Integrator besitzt den Run-Loop und ruft
`xrceWriteFrame`/`xrceReadBody` direkt gegen seinen eigenen Transport (Socket
oder In-Memory-Queue) auf.

**Repo:** `endpoints/java/ExampleSync.java` — In-Memory-`ArrayDeque`-Poll-Loop,
volles Feld-Decode (`Reading{id,value,label}`); `sync`-Modus in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA` — blockierendes
`DatagramSocket.receive()` gegen den echten Rust-Peer.

**Tests:** `java_endpoint_sync` (§4).

**Status:** done.

## §3 async Reader/Writer

**Spec:** §3 — Reader-`Thread` drainiert den Transport in eine
`BlockingQueue`, Consumer blockiert auf `take()`; keine dedizierte
`AsyncReader`/`AsyncWriter`-Klasse für den Plain-Pfad (anders als
`endpoints/go`) — der Integrator komponiert sie aus den zustandslosen
`ZdwEndpoint`-Framing-Methoden.

**Repo:** `endpoints/java/ExampleAsync.java` — Reader-`Thread` +
`LinkedBlockingQueue<byte[]>`-Inbox, volles Feld-Decode; `async`-Modus in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA` — Reader-`Thread` liest vom
echten `DatagramSocket`, Consumer blockiert auf `take()`.

**Tests:** `java_endpoint_async` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine Java-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus: generierte
`idl-java`-TypeSupport (`Ping`/`Pong`) + Endpoint-SDK (`ZdwEndpoint`), sync
und async.

**Repo:** `crates/endpoint-e2e/tests/java.rs` — `MAIN_JAVA` (Default-Package
`Main`, kompiliert gegen die reale ZeroDDS-Java-Runtime
`crates/java-omgdds` + `crates/idl-java/runtime`, Modus `sync`/`async` per
CLI-Argument).

**Tests (dieses Environment, OpenJDK 21, `cargo test -p zerodds-endpoint-e2e --test java`):**
- `java_endpoint_sync` — voller Stack über blockierendes `DatagramSocket.receive()`.
- `java_endpoint_async` — voller Stack über Reader-`Thread`/`LinkedBlockingQueue`.

2/2 grün (lokal verifiziert).

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

**Tests (dieses Environment, `cargo test -p zerodds-endpoint-e2e --test java_reliable`):**
- `java_reliable_unit_and_golden` — kompiliert und läuft
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
- `java_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die
  App (`ExampleReliable`, `AsyncReliableWriter`) retransmittet auf ACKNACK;
  alle 12 Samples lückenlos in Reihenfolge geliefert.
- `java_reliable_no_loss_baseline` — lossless Baseline; 12/12.

3/3 grün (lokal verifiziert; Latenz-Bench in §6).

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

**Tests (dieses Environment, `java_reliable_latency_bench`):**

```
producer latency: queue-enqueue median = 84 ns, inline-send median = 5917 ns (70x)
```

Median über 20000 Iterationen je Pfad, lokal gemessen (macOS, kein
Codepit-Lauf) — Größenordnung deckt sich mit `endpoints/go`s
codepit-verifizierten 20–25 ns / 4360 ns (~175–220×); der Absolutwert ist
maschinen-/lastabhängig, das Verhältnis (Enqueue deutlich unter Syscall)
ist der Beleg.

**Status:** done.

---

## Audit-Status

5 done / 1 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (dieses Environment — macOS, OpenJDK 21, `javac`/`java` auf PATH —
**nicht** codepit-verifiziert): `cargo test -p zerodds-endpoint-e2e --test java`
2/2 (Ping-Pong: `java_endpoint_sync`/`java_endpoint_async`); `--test java_reliable`
4/4 (`java_reliable_loss_recovery`, `java_reliable_no_loss_baseline`,
`java_reliable_unit_and_golden` — 33 Java-Unit-Assertionen über 17 Szenarien
inkl. Byte-Golden, `java_reliable_latency_bench` — Queue-Enqueue 84 ns /
inline `send` 5917 ns, ~70×); `endpoints/java/EndpointTest.java` manuell gegen
`cargo run -p zerodds-endpoint-golden`-Goldens gefahren — WRITE_DATA/serial/
DATA/HEARTBEAT/ACKNACK byte-identisch, `ALL OK`.

Offene Punkte: `EndpointTest.java` (§1 Byte-Golden für Framing/seriell) ist
nicht über `cargo test`/CI verdrahtet, nur manuell reproduzierbar (kein
Java-Äquivalent zu `endpoints/c/Makefile`s `make test`); alle Zahlen in
diesem Dokument sind lokal (dieses Environment) verifiziert, nicht auf
codepit.
