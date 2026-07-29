<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-java` v1.0 — Java Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/java/`.

Baut auf [ADR 0013](../adr/0013-native-endpoint-sdks.md) (Native Endpoint-SDKs,
Frame-Hook-Vertrag) und DDS-XRCE 1.0 §8.3 (Framing) sowie §8.4.10/§8.4.11
(Reliable Streams) auf. Der Reliable-Delivery-Kontrakt selbst ist getrennt
normiert in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). Analog zu
den Endpoint-SDKs anderer Sprachen (`endpoints/c`, `endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/d`, `endpoints/ada`, ...): ein
reines Java-8-Endpoint (kein JNI) über XRCE-Framing, sync send/recv, async
Reader/Writer und den reliable Stream, so dass eine Java-App ein
XCDR2-Sample byte-identisch zu `crates/xrce` und `endpoints/c` mit dem
geteilten Rust-Peer austauscht.

Die Framing- und Wire-Core-Klassen (`Zdw`, `ZdwEndpoint`, `ZdwReflect`)
liegen im Default-Package (kein Modul-Overhead für ein Standalone-Sample);
der reliable Stream — State-Machine, Wire-Codec, async Writer — liegt im
Package `org.zerodds.endpoint`.

## §1 XRCE-Framing

`endpoints/java/ZdwEndpoint.java` (Default-Package) MUSS DDS-XRCE-1.0-
konforme Frames sowohl für paketbasierte Transporte (UDP) als auch für
serielle Byte-Strom-Transporte (RS-485, UART) erzeugen und parsen —
byte-identisch zu `crates/xrce` und `endpoints/c`.

### §1.1 WRITE_DATA-Frame

Ein 8-Byte-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE — DDS-XRCE 1.0 §8.3.2.3/§8.3.4) gefolgt vom
XCDR2-Sample-Body. Best-effort-Betrieb ohne ClientKey: `session ≥ 0x80`.

`endpoints/java` MUSS bereitstellen:

- `ZdwEndpoint.xrceWriteFrame(int session, int stream, int seq, byte[] sample) -> byte[]`
  — framet ein Sample.
- Konstanten `ZdwEndpoint.SESSION_NOKEY` (`0x80`) und
  `ZdwEndpoint.STREAM_BEST_EFFORT` (`0x01`).

### §1.2 Empfangspfad (DATA-Message)

Der Endpoint MUSS DATA- (id `0x09`) wie WRITE_DATA-Submessages (id `0x07`)
über denselben Unwrap-Pfad entrahmen können:
`ZdwEndpoint.xrceReadBody(byte[] frame) -> byte[]` liest die Submessage-ID
aus Byte 4, prüft sie gegen `0x07`/`0x09` und liefert den Body ab Offset 8
gemäß der `len`-Feld-Länge.

### §1.3 Serielles HDLC-Framing (Annex C)

Für serielle Byte-Strom-Transporte MUSS der Endpoint das DDS-XRCE-1.0-
Annex-C-Framing implementieren: `7E [byte-stuffed(payload)
byte-stuffed(crc16-BE)] 7E`, Byte-Stuffing von `0x7E`/`0x7D` (RFC 1662),
CRC-16-CCITT-FALSE (Init `0xFFFF`, Polynom `0x1021`) über den rohen
Payload — `ZdwEndpoint.serialFrame`/`serialDeframe`/`crc16CcittFalse`,
byte-identisch zu `endpoints/c/src/zerodds_endpoint.c`.

## §2 sync send/recv

### §2.1 Frame-Hook-Vertrag

Der Endpoint ist transport-opak (ADR 0013, Invariante 5): `ZdwEndpoint`
besitzt kein eigenes `Client`- oder `Transport`-Objekt — Framing/Entrahmen
sind zustandslose statische Methoden. Der Integrator besitzt Socket bzw.
In-Memory-Queue selbst und ruft `xrceWriteFrame`/`xrceReadBody` direkt auf
(Frame-Hook-Symmetrie: Encode+Send und Receive+Decode laufen über denselben
statischen Aufrufpfad, kein verstecktes Objekt hält Transport-State).

### §2.2 Poll-Loop-Idiom

Die Baseline-Nutzung ist synchron: der Integrator besitzt den Run-Loop und
ruft die Empfangs-Funktion selbst auf (Poll bzw. blockierendes `receive()`
auf einem echten `DatagramSocket`). Referenz: `endpoints/java/ExampleSync.java`
(In-Memory-`ArrayDeque`-Poll-Loop) und der `sync`-Modus in
`crates/endpoint-e2e/tests/java.rs` (blockierendes
`DatagramSocket.receive()`).

## §3 async Reader/Writer

Das idiomatische Java-Async-Modell (`java.util.concurrent`): ein
Reader-`Thread` drainiert den Transport und schiebt entrahmte Sample-Bodies
auf eine `BlockingQueue`; der Consumer blockiert auf `take()`. Anders als
in `endpoints/go` gibt es für den Plain-(nicht-reliable)-Pfad keine
dedizierte `AsyncReader`/`AsyncWriter`-Klasse — der Integrator komponiert
Reader-Thread + `BlockingQueue` selbst aus den zustandslosen
`ZdwEndpoint`-Framing-Methoden (§1), analog zum `endpoints/c`-Reactor-Muster,
aber ohne eigenen Callback-Typ. Für den reliable Stream (§4) existiert mit
`AsyncReliableWriter` dagegen eine dedizierte, purpose-built Klasse.

`endpoints/java` MUSS bereitstellen:

- Ein lauffähiges Beispiel des Reader-Thread/`BlockingQueue`-Musters:
  `endpoints/java/ExampleAsync.java` (`ConcurrentLinkedQueue`-Transport,
  `LinkedBlockingQueue<byte[]>` als Inbox, Consumer blockiert auf `take()`).
- Live-Transport-Nachweis über einen echten `DatagramSocket` (nicht nur
  In-Memory-Queue): der `async`-Modus in
  `crates/endpoint-e2e/tests/java.rs` — ein Reader-`Thread` liest vom Socket
  und legt den entrahmten Body in eine `LinkedBlockingQueue`, der Consumer
  blockiert auf `take()`.

## §4 Reliable Stream

`endpoints/java` implementiert den reliable Stream als Endpoint-Fähigkeit
gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `AsyncReliableWriter`, dessen Drain-`Thread` den
`ReliableSender`-State und die gesamte I/O trägt, während der Producer nur
wait-free in eine gepufferte `BlockingQueue` enqueued (nie in den Kernel
geht). Package `org.zerodds.endpoint`.

Die Konstanten (`ReliableWire.WINDOW=16`, `ReliableWire.RECV_BUF=64`,
`ReliableWire.HEARTBEAT_PERIOD_MS=500`, `ReliableWire.MAX_PAYLOAD=65535`,
reliable Stream-ID `ReliableWire.STREAM_RELIABLE=0x80`), der
State-Machine-Kontrakt (`ReliableSender.submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight` auf dem Sender; `ReliableReceiver.recvData`/
`drainInOrder`/`pendingAcknack`/`reset` auf dem Receiver) und das
Wire-Format (`ReliableWire.heartbeatFrame`/`parseHeartbeat` — Submessage-ID
`0x0B`; `acknackFrame`/`parseAckNack` — Submessage-ID `0x0A`; RFC-1982
16-bit Sequenznummern via `seqLt`/`seqGt`) sind in `reliable-endpoint-1.0`
§3/§4 normativ definiert; `endpoints/java/org/zerodds/endpoint/*.java` ist
die Java-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK.

`AsyncReliableWriter` (`org.zerodds.endpoint.AsyncReliableWriter`) MUSS
bereitstellen:

- `submit(byte[] payload)` — blockierender Enqueue in eine
  `ArrayBlockingQueue<byte[]>` (Kapazität 4096), kein Socket-Syscall auf dem
  Producer-Pfad.
- `offer(byte[] payload) -> boolean` — nicht-blockierender Enqueue (für den
  Latenz-Bench, §6).
- Einen dedizierten Drain-`Thread`, der den `ReliableSender`-State besitzt:
  gequeute Samples ins Sendefenster übernehmen (`submit` → `writeFrame` →
  `DatagramSocket.send`), HEARTBEAT-Timer bedienen
  (`pendingHeartbeat`/`heartbeatFrame`), eingehende ACKNACKs verarbeiten
  (`parseAckNack` → `recvAcknack` → Retransmit der noch als missing
  markierten Sequenznummern aus dem In-Flight-Puffer).
- `finish(long timeoutMs) -> boolean` — signalisiert Produzenten-Ende und
  blockiert bis Queue und Sendefenster leer sind (alle Samples acked) oder
  der Timeout abläuft.

## §5 Conformance

Eine Java-Endpoint-Implementierung ist konform, wenn:

1. Der volle Stack (generierte `idl-java`-TypeSupport-Klassen +
   `endpoints/java`) ein typisiertes Sample sowohl über den sync-Poll-Loop
   als auch über den Reader-Thread/`BlockingQueue`-Async-Pfad mit dem
   geteilten Rust-XRCE-Peer über einen echten `DatagramSocket` austauscht.
2. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens
   sind und der reliable Stream Datagramm-Verlust lückenlos in-order
   aufholt (§4, `reliable-endpoint` v1.0 §5).
3. Ein Latenz-Messwert zeigt, dass `AsyncReliableWriter.offer`
   (`BlockingQueue`-Enqueue) messbar unter einem inline
   `DatagramSocket.send` liegt — der Beleg, dass Async-Write die
   Syscall-Latenz aus dem Producer-Pfad nimmt.

## §6 Beispiele

- Sync: `endpoints/java/ExampleSync.java` — In-Memory-Poll-Loop, vollem
  Feld-Decode (`Reading { id, value, label }`).
- Async: `endpoints/java/ExampleAsync.java` — Reader-`Thread` +
  `BlockingQueue`, vollem Feld-Decode.
- Reliable: `endpoints/java/ExampleReliable.java` — `AsyncReliableWriter`
  gegen einen echten UDP-Peer (kein In-Process-Stub).
- Latenz-Bench: `endpoints/java/ReliableBench.java` — `BlockingQueue`-
  Enqueue vs. inline `DatagramSocket.send`.
- Unit-/Byte-Golden-Suite: `endpoints/java/ReliableSelfTest.java`.
- Quickstart: `endpoints/java/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, async und reliable sind vollständig implementiert und
byte-verifiziert (siehe `docs/spec-coverage/zerodds-endpoint-java-1.0.md`).
