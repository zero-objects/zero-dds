<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-python` v1.0 — Python Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/python/`.

Analog zu [`zerodds-xcdr2-python`](zerodds-xcdr2-python-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/nim`, `endpoints/c`, `endpoints/ada`, ...): das native
Python-Endpoint über XRCE-Framing, sync `Client`, das idiomatische
asyncio-Empfangsmuster und den reliable Stream, so dass eine Python-App ein
XCDR2-Sample byte-identisch zu `crates/xrce` + `endpoints/c` mit dem geteilten
Rust-Peer austauscht. Pure stdlib, kompatibel zu Python 2.7 und 3.x
(`zerodds_endpoint.py`, `zerodds_wire.py`); die asyncio-Schicht ist Python-3-only.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA/`0x09` DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body —
byte-identisch zu `crates/xrce` und `endpoints/c`. Zusätzlich die
Annex-C-Serial-HDLC-Framing (Byte-Stuffing `0x7E`/`0x7D`/XOR `0x20` +
CRC-16-CCITT-FALSE), Spiegel von `endpoints/c/src/zerodds_endpoint.c`.

`endpoints/python/zerodds_endpoint.py` MUSS bereitstellen:

- `xrce_write_frame(session, stream, seq, sample) -> bytes` — framet ein
  Sample als WRITE_DATA.
- `xrce_read_frame(frame) -> bytes` — entrahmt WRITE_DATA (`0x07`) oder DATA
  (`0x09`); wirft `ValueError` bei zu kurzem Frame oder falscher
  Submessage-ID.
- `serial_frame(payload) -> bytes` / `serial_deframe(frame) -> bytes` — HDLC
  `0x7E`-Rahmen mit Byte-Stuffing + CRC-16-CCITT-FALSE; `crc16_ccitt_false`
  als eigenständige Funktion.
- Konstanten `XRCE_SESSION_NOKEY` (`0x80`, best-effort, ohne ClientKey),
  `XRCE_STREAM_BEST_EFFORT` (`0x01`), `XRCE_STREAM_NONE` (`0x00`).

## §2 Sync `Client`

Ein blockierungsfreier Client für Poll-Loops: `write` framet + liefert über
den Transport; `poll` ist ein nicht-blockierender Einzel-Receive.

`endpoints/python/zerodds_endpoint.py` MUSS bereitstellen:

- Duck-typing statt formalem Interface: ein Transport ist jedes Objekt mit
  `deliver(frame)` und `receive() -> bytes|None`. `MemTransport` (In-Memory-
  FIFO) ist die mitgelieferte Referenzimplementierung für Tests/Beispiele;
  UDP- oder Serial-Transporte sind Drop-ins mit derselben Signatur.
- `class Client` mit `Client(transport, session=XRCE_SESSION_NOKEY,
  stream=XRCE_STREAM_BEST_EFFORT)`, `.write(sample)` (framet + liefert, seq
  monoton wachsend mod 2¹⁶), `.poll()` (ein nicht-blockierender Receive; gibt
  den entrahmten Body oder `None` zurück).

## §3 Async (asyncio-Empfangsmuster)

Anders als in Go — wo `AsyncReader`/`AsyncWriter` eigene, im SDK
mitgelieferte Typen sind — bleibt `zerodds_endpoint.py` bewusst
2.7/3.x-kompatibel und import-clean ohne `asyncio`-Abhängigkeit. Async ist
deshalb kein eigener SDK-Typ, sondern ein dokumentiertes, idiomatisches
asyncio-Muster **auf demselben Transport-Duck-Type wie `Client`**: ein
`async def stream(self)`-Generator pollt `transport.receive()` nicht-blockierend
und yieldet `ze.xrce_read_frame(frame)` bei jedem eingetroffenen Frame,
sonst `await asyncio.sleep(...)`. Der Consumer iteriert mit `async for`. Es
gibt keinen separaten `AsyncWriter`: `Client.write()` ist bereits
nicht-blockierend (kein Warten auf I/O), dieselbe Schreibseite bedient sync
wie async.

Referenzimplementierungen dieses Musters (kein eigenständiger SDK-Export,
aber byte-identisch im Framing zu §1/§2):

- `endpoints/python/example_async.py::AsyncReader` — Async-Generator über
  `MemTransport`.
- Die inline `AsyncReader`-Klasse der E2E-App in
  `crates/endpoint-e2e/tests/python.rs` (`run_async`) — dasselbe Muster über
  einen echten `UdpTransport`.

## §4 Reliable Stream

`endpoints/python/zerodds_reliable.py` implementiert den reliable Stream als
Endpoint-Fähigkeit gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
— Sender-/Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `ReliableWriter`.

Die Konstanten (`SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`HEARTBEAT_PERIOD_S=0.5`, `MAX_PAYLOAD=65535`, reliable Stream-ID `0x80`),
der State-Machine-Kontrakt (`ReliableSender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `ReliableReceiver.recv_data`/
`drain_in_order`/`pending_acknack`/`reset`) und das Wire-Format (HEARTBEAT
`0x0B`, ACKNACK `0x0A`, RFC-1982 16-bit Sequenznummern via
`seq_lt`/`seq_gt`) sind dort normativ definiert; `zerodds_reliable.py` ist
die Python-Bindung dieses Kontrakts, byte-identisch zu
`crates/xrce/src/reliable.rs` und jedem anderen Endpoint-SDK.

**Honest note (Producer-Entkopplung, keine wait-free Ring):** anders als der
Rust/C-`ReliableAsyncWriter` ist `ReliableWriter.enqueue()` **kein**
wait-free Ringpuffer-Push, sondern ein `queue.Queue.put()` — ein
lock-geschützter Deque-Append. Was real ist: eine dedizierte Drain-
`threading.Thread` hält den `ReliableSender`-State und die gesamte Socket-I/O
(Senden, Heartbeat, ACKNACK-getriebenes Retransmit); der GIL wird um die
blockierenden `socket.send`/`recv`-Aufrufe des Drain-Threads freigegeben, so
dass `enqueue()` nie auf einen Syscall wartet und der Drain-Thread währenddessen
echt nebenläufig arbeitet. Das ist "Thread + GIL-Release-um-Syscalls"-
Entkopplung, keine lock-freie Datenebene — die CPython-GIL macht echte
Parallelität auf Producer- und Drain-Seite grundsätzlich unmöglich; entkoppelt
wird die **Syscall-Latenz**, nicht die CPU-Zeit.

## §5 Conformance

Eine Python-Endpoint-Implementierung ist konform, wenn:

1. Ein rohes UDP-Ping-Pong mit dem generierten `.encode()`/`.decode()`
   (ohne XRCE-Frame) mit dem Rust-Referenz-Peer byte-korrekt läuft.
2. Der volle Stack (generierte Typen + `endpoints/python`) ein typisiertes
   Sample sowohl über den sync `Client` als auch über das asyncio-Muster aus
   §3 mit dem geteilten Rust-XRCE-Peer austauscht.
3. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
4. Ein Latenz-Messwert zeigt, dass `ReliableWriter.enqueue` messbar unter
   einem inline `socket.send` liegt — der Beleg, dass die Syscall-Latenz aus
   dem Producer-Pfad entkoppelt ist (§4, Honest-Note: Thread+GIL-Release,
   nicht wait-free).

## §6 Beispiele

- Sync: `endpoints/python/example_sync.py` — Poll-Loop über `MemTransport`,
  vollem Feld-Decode (`Reading(id, value, label)`).
- Async: `endpoints/python/example_async.py` — asyncio-Async-Generator
  `AsyncReader.stream()`.
- Reliable: `endpoints/python/example_reliable.py` — In-Process-Demo (lossy
  Receiver-Thread + `ReliableWriter`, kein externer Peer nötig), plus
  UDP-Sender-Modus (`run`) und Latenz-Bench-Modus (`bench`) für das E2E.
- Quickstart: `endpoints/python/QUICKSTART.md`.

## §7 Errata + Open-Questions

Keine. Sync, das asyncio-Empfangsmuster und reliable sind vollständig
implementiert und byte-verifiziert (siehe
`docs/spec-coverage/zerodds-endpoint-python-1.0.md`).
