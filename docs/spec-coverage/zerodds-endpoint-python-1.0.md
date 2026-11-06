# `zerodds-endpoint-python` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-python-1.0.md` — ZeroDDS Python
Endpoint-SDK-Spec. Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Python`; hier steht der Transport.

Implementation:

- `endpoints/python/zerodds_endpoint.py` — XRCE-Framing
  (`xrce_write_frame`/`xrce_read_frame`), serielles Framing nach Annex C
  (`serial_frame`/`serial_deframe`), sync `Client`, `MemTransport`.
- `endpoints/python/zerodds_reliable.py` — reliable Sender/Receiver-
  State-Machine + HEARTBEAT/ACKNACK-Wire-Codec + `ReliableWriter`.
- `endpoints/python/example_async.py` — asyncio-`AsyncReader`-Muster (§3).
- `crates/endpoint-e2e/tests/python.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/python_reliable.rs` — reliable-Stream-E2E.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA/`0x09` DATA, flags, len LE) + Body, byte-identisch zu
`crates/xrce` + `endpoints/c`; dazu serielles Framing nach Annex C.1
(Adressen, Länge, Byte-Stuffing, FCS-16 nach RFC 1662).

**Repo:** `endpoints/python/zerodds_endpoint.py` —
`xrce_write_frame`/`xrce_read_frame`, `fcs16`,
`serial_frame`/`serial_deframe`, Konstanten `XRCE_SESSION_NOKEY` (`0x80`),
`XRCE_STREAM_BEST_EFFORT` (`0x01`), `XRCE_STREAM_NONE` (`0x00`).

**Tests:** `endpoints/python/test_endpoint.py` (`python3 test_endpoint.py
<golden_dir>`) — WRITE_DATA-Framing, Serial-Framing, DATA-Receive,
Serial-Deframe-Roundtrip, Abweisung eines falschen `frameCheck` und eines
`0x7E` im Rahmen, HEARTBEAT-Parse, ACKNACK-Framing, je gegen
`golden_xrce_le.bin`/`golden_serial_le.bin`/`golden_data_le.bin`/
`golden_heartbeat_le.bin`/`golden_acknack_le.bin`, meldet `ALL OK`. Framing
selbst wird zusätzlich über `xrce_8_3_5_8_python_sync_app_writes_the_ping_and_reads_the_pong`/`xrce_8_3_5_8_python_async_app_writes_the_ping_and_reads_the_pong`
(§4) live geübt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierungsfreier `Client`: `write` framet + liefert über
den Transport, `poll` ist ein nicht-blockierender Einzel-Receive.

**Repo:** `endpoints/python/zerodds_endpoint.py::Client`
(`__init__(transport, session, stream)`/`write`/`poll`, monotoner
`seq`-Zähler mod 2¹⁶, Default-Session `XRCE_SESSION_NOKEY`/
`XRCE_STREAM_BEST_EFFORT`); `MemTransport` (In-Memory-FIFO,
`deliver`/`receive`) als Referenz-Transport; ein Transport ist reines
Duck-Typing (`deliver(frame)`/`receive() -> bytes|None`), kein formales
Interface wie Gos `Transport`.

**Tests:** `endpoints/python/example_sync.py` — 5 typisierte
`Reading(id, value, label)`-Samples über `Client.write`/`.poll()`, voller
Feld-Decode, meldet `ALL OK`; Live-E2E `xrce_8_3_5_8_python_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 Async (asyncio-Empfangsmuster)

**Spec:** §3 — kein eigener SDK-Typ (anders als Gos `AsyncReader`/
`AsyncWriter`): ein `async def stream(self)`-Generator pollt
`transport.receive()` nicht-blockierend und yieldet den entrahmten Body;
Consumer iteriert mit `async for`. Kein separater `AsyncWriter` — `Client.write`
bedient sync wie async, da es bereits nicht-blockierend ist.

**Repo:** `endpoints/python/example_async.py::AsyncReader.stream()` — das
Referenzmuster über `MemTransport`; dieselbe Klasse (inline dupliziert, gleiche
Semantik) in der E2E-App `crates/endpoint-e2e/tests/python.rs::PY_APP::run_async`
über einen echten `UdpTransport`.

**Tests:** `endpoints/python/example_async.py` — 5 `Reading`-Samples über
`async for`, voller Feld-Decode, meldet `ALL OK`; Live-E2E
`xrce_8_3_5_8_python_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1/§5.2 — eine Python-App tauscht mit dem geteilten Rust-XRCE-Peer
über einen echten UDP-Socket ein typisiertes Sample aus: einmal roher
generierter Codec (`.encode()`/`.decode()`) ohne XRCE-Frame, einmal voller
Stack (generierte Dataclasses + Endpoint-SDK) sync und async.

**Repo:** `crates/endpoint-e2e/tests/python.rs` — `PY_APP` (`run_raw` roh
über UDP ohne XRCE-Frame, nutzt nur `Ping.encode()`/`Pong.decode()`;
`run_sync` über `ze.Client`; `run_async` über das asyncio-Muster aus §3),
Modus per `sys.argv[1]`.

**Tests:** (Linux)
- `endpoint_python_5_1_the_generated_codec_pings_without_the_xrce_frame` — generierter `Ping`/`Pong`-Codec direkt über einen rohen
  UDP-Socket, ohne XRCE-Framing.
- `xrce_8_3_5_8_python_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `ze.Client`.
- `xrce_8_3_5_8_python_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über das asyncio-Empfangsmuster.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `ReliableReceiver.recv_data`/
`drain_in_order`/`pending_acknack`/`reset`. Window 16, Receiver-Buffer 64,
Heartbeat 500 ms, Payload ≤ 65535, RFC-1982 16-bit Sequenznummern
(`seq_lt`/`seq_gt`). Dazu der async-entkoppelte `ReliableWriter`: der
Producer `enqueue()`t in eine `queue.Queue`, eine dedizierte Drain-
`threading.Thread` hält den `ReliableSender`-State und macht die gesamte I/O
(Senden, Heartbeat, ACKNACK-getriebenes Retransmit).

**Honest note:** `ReliableWriter.enqueue()` ist **kein** wait-free
Ringpuffer-Push wie in Rust/C, sondern ein lock-geschützter
`queue.Queue.put()`. Real ist die I/O-Entkopplung: der GIL wird um die
blockierenden `socket.send`/`recv`-Aufrufe des Drain-Threads freigegeben, so
dass `enqueue()` nie auf einen Syscall wartet — Thread + GIL-Release-um-
Syscalls, keine lock-freie Datenebene (Spec §4, Honest-Note).

**Repo:** `endpoints/python/zerodds_reliable.py` —
`reliable_write_frame`/`reliable_unframe`, `heartbeat_frame`/
`parse_heartbeat`, `acknack_frame`/`parse_acknack`; `ReliableSender`,
`ReliableReceiver`; `ReliableWriter` (`queue.Queue`-basiert,
`enqueue`/`start`/`close`, Drain-`threading.Thread`);
`endpoints/python/example_reliable.py` (In-Process-Demo mit lossy
Receiver-Thread, UDP-Sender-Modus `run`, Latenz-Bench-Modus `bench`).

**Tests:** (Linux)
- `endpoints/python/reliable_test.py` in der Spalte `python` von `tools/endpoints-gate`
  (Job `integration-endpoints`) läuft
  `endpoints/python/reliable_test.py` gegen von `zerodds-xrce` erzeugte
  Goldens — 21 Prüfungen (`check()`-Aufrufe) über 13 Testfunktionen: monotone
  seq + in-flight-Count (`endpointpython_5_submit_assigns_monotonic_seqnrs`: 3 Checks),
  Payload-zu-groß (`endpointpython_5_submit_rejects_payload_too_large`), Window-full
  (`endpointpython_5_submit_rejects_when_window_full`), Heartbeat
  Body/silenced/fires-after/none-when-empty
  (`endpointpython_5_pending_heartbeat`: 4 Checks), ACKNACK Teil-/Voll-Clear
  (`endpointpython_5_recv_acknack_clears_acked_keeps_missing`/
  `endpointpython_5_recv_acknack_full_clear_when_no_bits_set`), Receiver
  in-order/Reorder/Dedup/Buffer-full (`endpointpython_5_recv_data_buffers_in_order`/
  `endpointpython_5_recv_data_reorders_out_of_order`: 2 Checks/
  `endpointpython_5_recv_data_drops_duplicates`/`endpointpython_5_recv_data_rejects_when_buffer_full`),
  Pending-ACKNACK-Bitmap (`endpointpython_5_pending_acknack_marks_missing_slots`), Reset
  (`endpointpython_5_reset_clears_state`), In-Process-End-to-End-Loss-Recovery
  (`endpointpython_5_end_to_end_sender_receiver_with_loss_recovery`: 3 Checks), plus
  bedingt (Golden-Dir übergeben) Byte-Golden `endpointpython_5_byte_golden`:
  `heartbeat_frame(1,1,3,0x80)` == von `zerodds-xrce` erzeugtem
  `golden_heartbeat_le.bin`, `acknack_frame(1,1,0,0x80)` == erzeugtem
  `golden_acknack_le.bin`, plus Roundtrip-Parse — meldet `ALL OK`.
- `reliable_5_3_python_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die
  App, die der Test aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und dem `ReliableWriter` baut, retransmittet
  auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert, der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`).
- `reliable_5_4_python_the_producer_enqueue_costs_less_than_the_inline_send` — Latenz-Bench (§6).

**Status:** done.

## §6 Latenz — `enqueue()` vs. inline `socket.send`

**Spec:** §5.4 — der Producer-Pfad des `ReliableWriter` (`enqueue()` →
`queue.Queue.put()`) muss messbar unter dem inline `socket.send`-Syscall
liegen — der Beleg, dass die Syscall-Latenz aus dem Producer-Pfad entkoppelt
ist (Spec §4 Honest-Note: Thread+GIL-Release, keine wait-free Ring).

**Repo:** `endpoints/python/example_reliable.py::run_bench` — 20000
Iterationen inline `sock.send(reliable_write_frame(...))` vs. 20000
Iterationen `ReliableWriter.enqueue(sample)` gegen einen idle-drainenden
Sink-Socket (kein Live-Peer nötig, nur lokale Dispatch-Kosten unter Messung).

**Tests:** (Linux) `reliable_5_4_python_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/python_reliable.rs`) — Enqueue **704 ns** vs.
inline `send` **4108 ns** (~5,8×).

**Honest note:** der Faktor liegt deutlich unter Gos ~175–220× und Rusts/Cs
wait-free-Ring-Werten — erwartbar, weil `enqueue()` selbst schon ein
`queue.Queue.put()` (Lock + interner Deque-Append, kein reiner
Speicher-Store) ist und CPython-Interpreter-Overhead beide Seiten der Messung
dominiert. Der Messwert belegt die Entkopplung vom Syscall, nicht
Wait-Freedom.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-python-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 5 Tests grün.
* `python3 ci/run_integration.py integration-endpoints` (Job `integration-endpoints`) — 14 Tests grün.

19 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
