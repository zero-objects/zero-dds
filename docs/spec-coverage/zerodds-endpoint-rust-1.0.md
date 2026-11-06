# `zerodds-endpoint-rust` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-rust-1.0.md` — ZeroDDS Rust-Endpoint-SDK-Spec
(ADR 0013 + DDS-XRCE 1.0 §8.3/§8.4.10/§8.4.11 + `reliable-endpoint-1.0`).

Implementation:

- `endpoints/rust/` — das native Rust-Endpoint-SDK (ADR 0013): `src/lib.rs` (XRCE-Framing,
  `Client`, `AsyncReader`), `src/reliable.rs` (reliable State-Machine + async-entkoppelter
  Writer), `examples/{example_sync,example_async,example_reliable}.rs`.
- `crates/endpoint-e2e/` — der geteilte Rust-XRCE-Peer (`src/lib.rs`) + das reliable-E2E
  (`tests/rust_reliable.rs`).

## §1 XRCE-Framing

### §1 WRITE_DATA-Rahmen (8-Byte-Header + Body)

**Spec:** §1 -- "Ein WRITE_DATA-Rahmen ist ein 8-Byte-Header + XCDR2-Sample-Body:
`[session, stream, seq_lo, seq_hi, submsg_id, flags, len_lo, len_hi] + body`. `session=0x80`,
`stream=0x01` (best-effort), `submsg_id=0x07`, `flags=0x03`."

**Repo:** `endpoints/rust/src/lib.rs::xrce_write_frame`/`xrce_read_frame` (best-effort
Stream `0x01`, Session `0x80`). Der geteilte Peer liest den Rahmen mit dem Codec von
`zerodds-xrce` (`crates/endpoint-e2e/src/lib.rs::xrce_unframe`).

**Tests:** `crates/endpoint-e2e/tests/xrce_write_framing.rs::xrce_8_3_5_8_rust_endpoint_frames_a_conformant_write_data`
(der Rahmen des SDK, dekodiert von `zerodds-xrce`: Kopf, `WRITE_DATA`, Flags,
`BaseObjectRequest`, Sample); dazu `example_sync`/`example_async` (Job
`integration-endpoints`, Spalte `rust`).

**Status:** done

### §1 Peer-Rolle im Ping-Pong (kein eigenständiger Rust-Ping-Pong-Test)

**Spec:** §1 -- "Weil Rust hier die Peer-Rolle einnimmt (die 13 anderen Sprachen sprechen
gegen ihn), gibt es keinen eigenständigen Rust-gegen-Rust-Ping-Pong-Test — die
Framing-Konformität wird transitiv durch jeden der 13 Sprach-E2E-Tests in
`crates/endpoint-e2e/tests/` geprüft."

**Repo:** `crates/endpoint-e2e/src/lib.rs::ping_pong` (Zeile 97–128) ist der Peer selbst.
In `crates/endpoint-e2e/tests/` gibt es keine Rust-Ping-Pong-Datei, weil Rust hier nicht "die App" ist,
sondern der Peer, den jede andere Sprache anspricht — ein eigener Rust-Ping-Pong-Test
gegen sich selbst würde nichts zusätzlich prüfen. Für den reliable Stream gilt das
nicht: dort ist Rust die App, und `crates/endpoint-e2e/tests/rust_reliable.rs`
existiert (siehe §4). `endpoints/rust`s eigenes
Client/Reader-Paar wird stattdessen über die MemTransport-Beispiele (§2/§3) geprüft, nicht
über den UDP-Peer-Pfad.

**Tests:** transitiv — jeder der 14 Sprach-E2E-Tests
(`crates/endpoint-e2e/tests/ada.rs`, `crates/endpoint-e2e/tests/c.rs`,
`crates/endpoint-e2e/tests/cpp.rs`, …) übt `ping_pong` aus; kein direkter
Rust-eigener Test.

**Status:** n/a (informative) — strukturell, nicht offen: Rust ist die Peer-Implementierung,
nicht ein getesteter Client dieses Peers.

## §2 sync Client

### §2 `Client::write`/`poll`

**Spec:** §2 -- "Ein pollender, nicht-blockierender Empfangspfad — das Idiom für Aufrufer,
die die Run-Loop selbst besitzen. `write` framet einen XCDR2-Sample-Body mit der aktuellen
`seq` (`wrapping_add`), `poll` liefert einen dekodierten Body oder `None`."

**Repo:** `endpoints/rust/src/lib.rs::Client` (Zeile 113–138): `write` framet einen
XCDR2-Sample-Body und liefert ihn über `MemTransport`; `poll` liefert einen dekodierten
Body oder `None` (nicht-blockierend).

**Tests:** `endpoints/rust/examples/example_sync.rs` — 5 `Reading{id,value,label}`-Samples,
Feld-für-Feld dekodiert (`id`, `value`, `label`), `assert!(got == total)` + `"ALL OK"`;
Job `integration-endpoints` (Spalte `rust`).

**Status:** done

## §3 async Reader/Writer

### §3 `AsyncReader` (Thread + `mpsc`)

**Spec:** §3 -- "Ein Hintergrund-Reader, der dekodierte Samples über einen Kanal liefert —
kein Async-Runtime, sondern das idiomatische std-Concurrency-Modell (Thread + Channel).
`start` spawnt einen Thread, `recv` blockiert, `stop` signalisiert per `AtomicBool`."

**Repo:** `endpoints/rust/src/lib.rs::AsyncReader` (Zeile 142–180): `start` spawnt einen
Thread, der die Transport pollt und dekodierte Bodies über `mpsc::channel` an `recv`
liefert; `stop` signalisiert den Thread per `AtomicBool`.

**Tests:** `endpoints/rust/examples/example_async.rs` — 5 Samples, `reader.recv()`
blockierend, alle Felder dekodiert, `"ALL OK"`; Job `integration-endpoints` (Spalte `rust`).

**Status:** done

## §4 reliable Stream

### §4 State-Machine (Sender + Receiver)

**Spec:** §4 -- "XRCE reliable Stream (`stream_id >= 128`, §8.4.10/§8.4.11), Fenster 16
(entspricht der 16-bit-ACKNACK-Bitmap), Receiver-Puffer 64, Heartbeat 500 ms, Payload
≤ 65535, RFC-1982 16-bit Sequenznummern — spiegelt `crates/xrce/src/reliable.rs`."

**Repo:** `endpoints/rust/src/reliable.rs` — `ReliableSender` (`submit`/
`pending_heartbeat`/`recv_acknack`/`get_in_flight`/`in_flight_seqs`, Zeile 201–292),
`ReliableReceiver` (`recv_data`/`drain_in_order`/`pending_acknack`/`reset`, Zeile
300–376). Self-contained, keine `zerodds-xrce`-Laufzeit-Abhängigkeit (bewusst dupliziert
statt importiert, siehe Modul-Doc-Kommentar Zeile 1–21).

**Tests:** `cargo test -p zerodds-endpoint-rust --lib` — 14 State-Machine-Tests
(`submit_assigns_monotonic_seqnrs`, `submit_rejects_payload_too_large`,
`submit_rejects_when_window_full`, `heartbeat_fires_first_then_silences_until_period`,
`heartbeat_none_when_window_empty`, `recv_acknack_clears_acked_keeps_missing`,
`recv_acknack_full_clear_when_no_bits`, `recv_data_delivers_in_order`,
`recv_data_reorders_out_of_order`, `recv_data_drops_duplicates`,
`recv_data_rejects_when_buffer_full`, `pending_acknack_marks_missing_slots`,
`reset_clears_receiver`, `end_to_end_loss_recovery_in_process`) + 2 Wire-/Ring-Tests
(`write_frame_round_trips`, `spsc_ring_fifo_and_backpressure`) + 2 Byte-Golden (nächstes
Item) = 18 von 18 im Crate. Lokal reproduziert (dieser Lauf): 18/18 passed, 0 failed.

**Status:** done

### §4 Wire (byte-golden HEARTBEAT/ACKNACK)

**Spec:** §4 -- "HEARTBEAT (`0x0B`) und ACKNACK (`0x0A`) byte-identisch zu den Referenz-
Goldens des C-SDKs (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`); XRCE-Control-
Konvention: Header-Stream = NONE (`0x00`) + Control-Message-Seq, Ziel-Stream-Id im
letzten Body-Byte."

**Repo:** `endpoints/rust/src/reliable.rs::heartbeat_frame`/`acknack_frame`/
`parse_heartbeat`/`parse_acknack` (Zeile 125–192).

**Tests:** `heartbeat_frame_byte_golden` assertiert
`heartbeat_frame(Heartbeat{first:1,last:3,stream_id:0x80},1)` ==
`[128,0,1,0,11,1,5,0,1,0,3,0,128]`; `acknack_frame_byte_golden` assertiert
`acknack_frame(AckNack{first_unacked:1,nack_bitmap:[0,0],stream_id:0x80},1)` ==
`[128,0,1,0,10,1,5,0,1,0,0,0,128]` — byte-identisch zu den C-Goldens.

**Status:** done

### §4 Async-entkoppelter Writer + Loss-Recovery (E2E)

**Spec:** §4 -- "Der Producer enqueued nur (wait-free) in einen SPSC-Ring; ein dedizierter
Drain-Thread besitzt Socket + reliable Sender-State und macht die gesamte I/O
(`WRITE_DATA`-Send, HEARTBEAT-Timer, ACKNACK-getriebenes Retransmit). Der Producer geht nie
in den Kernel. Reliable Delivery überlebt Datagramm-Verlust — verifiziert live gegen den
geteilten Rust-Peer mit injiziertem Loss."

**Repo:** `endpoints/rust/src/reliable.rs::SpscRing` (Zeile 384–443, wait-free `push`/`pop`
über zwei Atomics, kein Lock), `ReliableWriterHandle::enqueue` (Zeile 458–460, nie im
Kernel), `AsyncReliableWriter::start`/`drain_loop` (Zeile 469–574: Ring → Sender-Window →
`send`, HEARTBEAT wenn fällig, ACKNACK-Drain → Retransmit, `shutdown()` blockiert bis Ring
leer UND Fenster leer, mit 5s-Sicherheits-Deadline gegen einen verstummten Peer).
`crates/endpoint-e2e/src/lib.rs::reliable_collect` treibt den Peer als Receiver (dropt bei
`drop_every=Some(3)` jedes 3. distinct Sample genau einmal).

**Tests:** `crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams`
(Peer dropt jedes 3. Sample) — 12/12 Samples lückenlos in-order geliefert
(`N = 12` in `tests/rust_reliable.rs`). In der Krate-Ebene `#[ignore]` (der
Test baut das Beispiel mit `cargo`), im Job `integration-bindings` gefahren.

**Status:** done

### §4 Rust als geteilter Peer (kein eigenständiger Rust-Loss-Recovery-Test)

**Spec:** §4 -- "Rust ist zugleich der geteilte reliable Peer (`crates/endpoint-e2e`,
`reliable_collect`) für alle anderen Sprachen — analog zu §1 existiert kein
eigenständiger Rust-gegen-Rust-Loss-Recovery-Test; die Loss-Recovery-Konformität des
`endpoints/rust`-eigenen `AsyncReliableWriter` wird über
`crates/endpoint-e2e/tests/rust_reliable.rs` geprüft (Rust-SDK als Sender, geteilter
Peer als droppender Receiver)."

**Repo:** identisch zum vorigen Item — `rust_reliable.rs` treibt `endpoints/rust`s
`AsyncReliableWriter` als Sender gegen `crates/endpoint-e2e`s `reliable_collect` als
Receiver; strukturell ist das der einzige "Rust-Loss-Recovery-Test", weil Rust hier
Sender-unter-Test **und** Referenz-Receiver-Implementierung zugleich stellt.

**Tests:** siehe voriges Item (`reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams`).

**Status:** n/a (informative) — strukturell, nicht offen: die Test-Pflicht ist bereits im
vorigen Item erfüllt; dieser Eintrag dokumentiert nur, warum es keinen *separaten*
Rust-gegen-Rust-Test gibt.

### §4 Latenz-Nachweis: entkoppelt vs. inline

**Spec:** §4 -- "MUSS — Latenz-Nachweis (Beleg-Pflicht 4 aus `reliable-endpoint-1.0` §5):
`enqueue` (wait-free, Ring) gemessen gegen ein inline `send` (Kernelübergang) auf demselben
Producer-Pfad, gegen denselben leerlaufenden UDP-Sink."

**Repo:** `endpoints/rust/examples/example_reliable.rs::bench` (Zeile 71–115) — 50 000
Iterationen, `handle.enqueue` gegen den Ring vs. `isock.send(&write_frame(...))` inline,
beide gegen denselben leerlaufenden UDP-Sink gemessen.

**Tests:** (Linux) `cargo run -p zerodds-endpoint-rust --example example_reliable --
bench` → `enqueue(decoupled)=30ns inline(send)=3985ns` (~133×); der
Loss-Recovery-Pfad, den der Bench belastet, ist über
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams`
abgesichert. `crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_4_rust_the_producer_enqueue_costs_less_than_the_inline_send`
läuft den Bench und prüft, dass das Enqueue unter dem inline Send liegt (Job
`integration-bindings`). Lokal reproduziert (dieser Lauf, andere Maschine):
`enqueue=124ns inline=4422ns` (~35×) — Größenordnung bestätigt, Absolutwert
maschinenabhängig.

**Status:** done

---

## Audit-Status

7 done / 0 partial / 0 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-rust-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-endpoint-e2e --all-targets` (Job `merge-rust-endpoint-e2e`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-endpoint-rust --all-targets` (Job `merge-rust-endpoint-rust`) — 18 Tests grün.
* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 7 Tests grün.
* `python3 ci/run_integration.py integration-bindings-gnat-beam` (Job `integration-bindings-gnat-beam`) — 2 Tests grün.
* `python3 ci/run_integration.py integration-endpoints` (Job `integration-endpoints`) — 4 Tests grün.

32 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
