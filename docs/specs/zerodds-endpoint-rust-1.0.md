# `zerodds-endpoint-rust` v1.0 — natives Rust-Endpoint-SDK

ZeroDDS Vendor-Spec. Implementiert in `endpoints/rust/`. Baut auf ADR 0013
(Native-Endpoint-SDKs, Frame-Hook-Vertrag) und DDS-XRCE 1.0 §8.3 (Framing) auf.
Reliable-Stream-Kontrakt siehe
[`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). Codegen/Wire-Format-Details
(XCDR2-Encoding der Sample-Bodies) siehe
[`zerodds-xcdr2-rust-1.0`](zerodds-xcdr2-rust-1.0.md).

## §1 XRCE-Framing

Rust ist einer der vier nativen Endpoint-Sprachen aus ADR 0013 (C/C++/Python/
Rust) und zugleich der **geteilte Referenz-Peer**, gegen den die anderen 13
Endpoint-Sprachen ihre eigenen E2E-Tests fahren (`crates/endpoint-e2e`). Das
Framing ist transport-opak: der Frame-Hook-Kontrakt (`deliver(frame)` /
`receive() -> frame`) kennt keinen Socket, kein serielles Gerät — der
Aufrufer hängt seinen Transport dahinter (hier: `MemTransport` für
Tests/Examples, UDP im geteilten Peer).

Ein WRITE_DATA-Rahmen ist ein 8-Byte-Header + XCDR2-Sample-Body:

```
[session, stream, seq_lo, seq_hi, submsg_id, flags, len_lo, len_hi] + body
```

- `session` = `0x80` (No-Key-Session).
- `stream` = `0x01` (best-effort) im Sync/Async-Pfad; `0x80` (Bit 7 gesetzt)
  im reliable Pfad (§4).
- `seq` = 16-Bit-LE-Sequenznummer, monoton pro Stream.
- `submsg_id` = `0x07` (WRITE_DATA).
- `flags` = `0x03` (E-Flag = LE plus `DataFormat::Sample`-Bit).
- `len` = 16-Bit-LE-Länge des Body.
- `body` = der XCDR2-kodierte Sample (LE), byte-identisch zur
  `zerodds-cdr`-Referenz (siehe `zerodds-xcdr2-rust-1.0` §2/§5).

Der Body trägt **keinen** eigenen XRCE-Header — der 8-Byte-Rahmen ist die
einzige Framing-Schicht; Sample-Encoding beginnt direkt nach Byte 8.

MUSS: `xrce_write_frame`/`xrce_read_frame` bauen bzw. parsen exakt diesen
Rahmen; ein Frame ist nur dann ein WRITE_DATA, wenn Byte 4 `0x07` ist.

Weil Rust hier die Peer-Rolle einnimmt (die 13 anderen Sprachen sprechen
gegen ihn), gibt es keinen eigenständigen Rust-gegen-Rust-Ping-Pong-Test —
die Framing-Konformität wird transitiv durch jeden der 13
Sprach-E2E-Tests in `crates/endpoint-e2e/tests/` geprüft.

## §2 sync Client

`Client` ist der pollende, nicht-blockierende Endpoint für Aufrufer, die
ihre eigene Run-Loop besitzen (kein Hintergrund-Thread, kein Callback).

MUSS:

- `Client::new(transport)` — bindet an einen Frame-Hook-Transport, initiale
  Sequenznummer `1`.
- `write(sample: &[u8])` — framet den XCDR2-Sample-Body per §1 mit der
  aktuellen `seq`, inkrementiert `seq` (`wrapping_add`), liefert über den
  Transport aus. Kein Reliability-State — Fire-and-forget.
- `poll() -> Option<Vec<u8>>` — genau ein nicht-blockierender Empfangsversuch;
  `None` wenn der Transport leer ist; sonst der dekodierte Sample-Body (der
  8-Byte-Rahmen ist entfernt).

Der `write→poll`-Roundtrip ist der Referenzpfad für die Deep-Example
(`examples/example_sync.rs`): N `Reading{id,value,label}`-Samples,
Feld-für-Feld dekodiert.

## §3 async Reader/Writer

`AsyncReader` ist der Hintergrund-Reader-Halbteil: kein Async-Runtime
(kein `tokio`/`async-std`), sondern das idiomatische std-Concurrency-Modell
— ein OS-Thread plus `mpsc`-Channel.

MUSS:

- `AsyncReader::start(transport)` — spawnt einen Thread, der den Transport
  in einer Schleife pollt (Backoff `1ms` bei leerem Transport), jeden
  dekodierten WRITE_DATA-Body über einen `mpsc::channel` an `recv` liefert.
- `recv() -> Vec<u8>` — blockiert bis zum nächsten dekodierten Sample-Body.
- `stop()` — signalisiert den Reader-Thread über ein `AtomicBool` zum
  Beenden; kein erzwungenes Abbrechen laufender Arbeit.

Der async Writer-Halbteil (entkoppelter Producer, SPSC-Ring, Drain-Thread)
ist Teil des reliable Streams (§4) — im best-effort Pfad existiert kein
separater `AsyncWriter`, weil `Client::write` bereits nicht-blockierend
ist (kein Syscall-Warten, direkte Queue-Einreihung in `MemTransport`).

Referenzpfad: `examples/example_async.rs` — N Samples, `reader.recv()`
blockierend je Sample, vollständiger Felddecode.

## §4 reliable Stream

Rust implementiert den vollen, in
[`reliable-endpoint-1.0`](reliable-endpoint-1.0.md) §3 kanonisch definierten
State-Machine-Kontrakt (Sender + Receiver, RFC-1982-Sequenznummern,
`HEARTBEAT_PERIOD=500ms`, `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`MAX_PAYLOAD=65535`) **self-contained** in `endpoints/rust/src/reliable.rs`
— bewusst dupliziert statt aus `crates/xrce` importiert (kein
`zerodds-xrce`-Laufzeit-Abhängigkeit vom Endpoint-SDK auf den Hub-Crate).

MUSS — Sender (`ReliableSender`):

- `submit(payload) -> Result<seq, ReliableError>` — `PayloadTooLarge` bei
  `len > MAX_PAYLOAD`, `WindowFull` bei `in_flight >= SENDER_WINDOW`.
- `pending_heartbeat(now) -> Option<Heartbeat>`.
- `recv_acknack(AckNack)` — RFC-1982-Vergleich, entfernt Acked aus
  `in_flight`.
- `get_in_flight(seq)` — Retransmit-Lookup.

MUSS — Receiver (`ReliableReceiver`):

- `recv_data(seq, payload) -> Result<(), ReliableError>` — Duplikate
  verworfen, `BufferFull` bei `RECEIVER_BUFFER`.
- `drain_in_order() -> Vec<(seq, payload)>`.
- `pending_acknack(hint_last_seen) -> AckNack`.
- `reset()`.

MUSS — Wire: `write_frame`/`unframe` für den reliable Datenpfad
(`stream_id = 0x80`, Header-Seq = Sample-Seq); `heartbeat_frame`/
`acknack_frame`/`parse_heartbeat`/`parse_acknack` für den Control-Pfad
(Stream `0x00`, `submsg_id` `0x0B`/`0x0A`, `flags=0x01`); byte-identisch zu
den C-SDK-Goldens (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`).

MUSS — Async-entkoppelter Writer (`AsyncReliableWriter` + `SpscRing` +
`ReliableWriterHandle`): der Producer ruft ausschließlich
`enqueue(sample)` auf einem wait-free SPSC-Ring auf (kein Syscall, kein
Lock); ein dedizierter Drain-Thread besitzt Socket + `ReliableSender`-State
und macht die gesamte I/O (gebündelter `WRITE_DATA`-Send, periodisches
HEARTBEAT, ACKNACK-getriebenes Retransmit aus der History). `shutdown()`
blockiert, bis Ring **und** Sender-Fenster leer sind, mit einer
Sicherheits-Deadline gegen einen verstummten Peer.

Rust ist zugleich der geteilte reliable Peer (`crates/endpoint-e2e`,
`reliable_collect`) für alle anderen Sprachen — analog zu §1 existiert kein
eigenständiger Rust-gegen-Rust-Loss-Recovery-Test; die Loss-Recovery-
Konformität des `endpoints/rust`-eigenen `AsyncReliableWriter` wird über
`crates/endpoint-e2e/tests/rust_reliable.rs` geprüft (Rust-SDK als Sender,
geteilter Peer als droppender Receiver).

MUSS — Latenz-Nachweis (Beleg-Pflicht 4 aus `reliable-endpoint-1.0` §5):
`enqueue` (wait-free, Ring) gemessen gegen ein inline `send`
(Kernelübergang) auf demselben Producer-Pfad, gegen denselben leerlaufenden
UDP-Sink. Referenzpfad: `examples/example_reliable.rs::bench`.
