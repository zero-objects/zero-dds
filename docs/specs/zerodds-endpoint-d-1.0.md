<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-d` v1.0 — D Endpoint-SDK (XRCE-Framing, sync/async, reliable Stream)

ZeroDDS Vendor-Spec. Implementiert in `endpoints/d/`.

Ergaenzt die Codegen-Spec [`zerodds-xcdr2-d`](zerodds-xcdr2-d-1.0.md): dort das
Wire-Mapping IDL→D, hier der Endpoint-Baustein, der ein `marshalXCDR`-Sample als
XRCE-Frame ueber einen UDP-Transport traegt — sync gepollt, async ueber
`std.concurrency`, optional reliable (§4).

## §1 XRCE-Framing

Ein WRITE_DATA-Sample wird als 8-Byte-Header + Body gerahmt:

```
[session][stream][seq_lo][seq_hi][0x07][0x03][len_lo][len_hi][body...]
```

- `session` — `SessionNoKey = 0x80`.
- `stream` — `StreamBestEffort = 0x01` fuer den Best-Effort-Pfad dieses Bausteins;
  reliable Streams verwenden `stream_id >= 128` (§4, `reliable-endpoint-1.0`).
- `seq` — 16-bit little-endian, wrapt modulo `0x10000`.
- Submessage-ID `0x07` (WRITE_DATA), Flags `0x03` (E-Flag = LE, plus
  `DataFormat::Sample`).
- `len` — 16-bit little-endian Body-Laenge.

`writeFrame(session, stream, seqNo, sample) -> ubyte[]` MUSS den Header wie oben
emittieren und den Body unveraendert anhaengen. `readFrame(frame) -> ubyte[]` MUSS
den Body zurueckgeben, wenn `frame.length >= 8 && frame[4] == 0x07`, sonst `null`
(kein WRITE_DATA-Frame).

## §2 Sync Client

Ein blockierend gepollter `Client` ueber eine austauschbare `Transport`:

```d
struct Transport {
    void delegate(ubyte[]) deliver;
    ubyte[] delegate() receive; // null, wenn nichts anliegt
}

class Client {
    this(Transport t);
    void write(ubyte[] sample);   // framet + deliver(); seqNo++ (mod 0x10000)
    ubyte[] poll();                // ein non-blocking receive(); Body oder null
}
```

`write` MUSS `writeFrame` mit der aktuellen `seqNo` aufrufen, an `transport.deliver`
uebergeben und `seqNo` danach modulo `0x10000` inkrementieren. `poll` MUSS genau
einen `transport.receive()`-Aufruf machen und, falls ein Frame vorliegt, dessen
`readFrame`-Ergebnis zurueckgeben; liegt nichts an, `null`.

Der `Transport` ist ein reiner Delegate-Vertrag — das SDK schreibt keinen Socket
vor. `memTransport()` liefert eine In-Memory-FIFO-Instanz fuer Tests/Beispiele;
Live-Betrieb bindet `deliver`/`receive` an einen UDP-Socket.

## §3 Async Reader/Writer

Auf dieser Ebene existiert ein async **Reader** als Hintergrund-Actor via
`std.concurrency` (`spawn`/`send`/`receive`, Tid-Message-Passing):

```d
void readerLoop(Tid owner);   // laueft im Reader-Thread

class AsyncReader {
    this();                              // spawnt readerLoop
    void feed(ubyte[] frame);            // send(tid, frame) — roher Frame rein
    immutable(ubyte)[] recv();           // receiveOnly — dekodierter Body raus
    void stop();                         // send(tid, true) — Terminate
}
```

`readerLoop` MUSS jede ankommende `immutable(ubyte)[]`-Message als Frame
deframen (`readFrame`) und, falls das Ergebnis nicht leer ist, den Body an
`owner` senden; eine `bool`-Message terminiert die Loop. Payloads sind
`immutable`, damit sie die Thread-Grenze sicher ueberqueren — D erzwingt das
strukturell (kein `shared`/Lock noetig).

Ein entkoppelter async **Writer** ist kein eigener Baustein dieser Ebene,
sondern Teil des reliable Streams (§4): der Sync-`Client` (§2) sendet inline;
der reliable-Stream-Sender entkoppelt ueber den `SpscRing`.

## §4 Reliable Stream

Der reliable Stream ist eine optionale Erweiterung von §1–§3: State-Machine,
Wire-Codec und async-entkoppelter Writer nach
[`reliable-endpoint` v1.0](reliable-endpoint-1.0.md). D implementiert den
kanonischen Vertrag (§3 dort) selbststaendig in `endpoints/d/reliable.d` —
kein Import von `zerodds.d`s `Endian`/`Writer`, um Namenskollisionen zu
vermeiden:

- `Sender` — `submit`/`pendingHeartbeat`/`recvAcknack`/`getInFlight`.
- `Receiver` — `recvData`/`drainInOrder`/`pendingAcknack`/`reset`.
- Wire-Codec — `Heartbeat`/`AckNack`, `heartbeatFrame`/`acknackFrame`/
  `reliableWriteFrame` + die zugehoerigen `parse*`-Funktionen; byte-identisch
  zu `golden_heartbeat_le.bin`/`golden_acknack_le.bin`.
- `SpscRing` — ein wait-freier Single-Producer/Single-Consumer-Ring
  (`CAP = 1024`) als async-entkoppelter Writer: der Producer macht nur einen
  Slot-Store + einen Release-Store auf `head`, kein Lock, kein Syscall; ein
  separater Drain-Thread besitzt Socket + Sender-State.

Konstanten, Fehlerfaelle, Wire-Format und Test-/Belegpflicht sind in
`reliable-endpoint-1.0` §3–§5 normativ definiert und gelten unveraendert fuer D.
