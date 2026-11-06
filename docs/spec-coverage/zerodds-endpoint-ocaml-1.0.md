# `zerodds-endpoint-ocaml` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-ocaml-1.0.md` — ZeroDDS OCaml
Endpoint-SDK-Spec. Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Ocaml`; hier steht der Transport.

Implementation:

- `endpoints/ocaml/zerodds.ml` — Modul `Endpoint` (XRCE-Framing
  `write_frame`/`read_frame`), `transport`-Typ, `module Client` (sync),
  `module Mailbox` + `module AsyncReader` (async).
- `endpoints/ocaml/reliable.ml` — eigenständiges Modul `Reliable`
  (file-as-module, keine Abhängigkeit von `zerodds.ml`): reliable
  Sender/Receiver-State-Machine + HEARTBEAT/ACKNACK/WRITE_DATA-Wire-Codec +
  `module Writer` (async-entkoppelter reliable Writer mit Drain-`Thread`).
- `crates/endpoint-e2e/tests/ocaml.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/ocaml_reliable.rs` — reliable-Stream-E2E +
  Latenz-Vergleich.

Beide E2E-Testdateien sind auf `ocamlfind` (per `ocamlfind printconf`)
gegated: fehlt die Toolchain, wird laut geskippt (`eprintln!("SKIP ...")`),
kein false-green.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/ocaml/zerodds.ml::Endpoint` — `write_frame`/`read_frame`,
Konstanten `session_nokey` (`0x80`) und `stream_best_effort` (`0x01`).

**Tests:** kein isolierter Framing-Unit-Test (anders als Go's
`endpoint_go_5_1_the_generated_codec_pings_without_the_xrce_frame`); das Framing wird über `xrce_8_3_5_8_ocaml_sync_app_writes_the_ping_and_reads_the_pong`/
`xrce_8_3_5_8_ocaml_async_app_writes_the_ping_and_reads_the_pong` (§4) live geübt — die App framet dort explizit über
`Zerodds.Endpoint.write_frame`, bevor der `AsyncReader`/`Client` übernimmt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `write` framet + liefert synchron,
`poll` ist ein nicht-blockierender Einzel-Receive.

**Repo:** `endpoints/ocaml/zerodds.ml::transport`-Record
(`deliver`/`receive`, der einzige Integrationspunkt);
`endpoints/ocaml/zerodds.ml::Client` (`create`/`write`/`poll`, monotoner
`seq`-Zähler mit 16-Bit-Wraparound, Default-Session
`session_nokey`/`stream_best_effort`).

**Tests:** Live-E2E `xrce_8_3_5_8_ocaml_sync_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/ocaml.rs`) — die eingebettete OCaml-App
(`OCAML_MAIN`) baut ein `Gen.Ping`-Sample über den generierten
`zerodds-idlc generate --lang ocaml`-Codec, sendet es über `Zerodds.Client.write` an den
Rust-Peer und pollt `Zerodds.Client.poll` bis zum `Gen.Pong`-Reply
(10s-Deadline).

**Status:** done.

## §3 Async `AsyncReader`

**Spec:** §3 — ein `Thread` pollt den `transport` und legt entrahmte
Sample-Bodies in eine Mutex/Condition-`Mailbox` (FIFO); der Consumer
blockiert in `recv` auf `Condition.wait`. Kein Lwt, kein Async — nur
`Thread`/`Mutex`/`Condition` aus `threads.posix`.

**Repo:** `endpoints/ocaml/zerodds.ml::Mailbox` (`put`/`take`, generische
Mutex/Condition-FIFO); `endpoints/ocaml/zerodds.ml::AsyncReader`
(`start` spawnt den Empfangs-`Thread` via `Thread.create loop ()`,
`recv`/`stop`).

**Tests:** Live-E2E `xrce_8_3_5_8_ocaml_async_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/ocaml.rs`) — die App startet
`Zerodds.AsyncReader.start`, liefert das geframeте `Gen.Ping`-Sample direkt
über `transport.deliver`, blockiert in `AsyncReader.recv` bis zum
`Gen.Pong`-Reply und stoppt den Reader.

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine OCaml-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus, einmal über den sync
`Client`, einmal über `AsyncReader`, jeweils mit dem vollen Stack
(generierte `Gen.Ping`/`Gen.Pong`-Typen aus `zerodds-idlc generate --lang ocaml` +
`endpoints/ocaml`).

**Repo:** `crates/endpoint-e2e/tests/ocaml.rs` — `OCAML_MAIN` (eine
eingebettete OCaml-Quelle, per CLI-Argument `sync`/`async`), zusammen mit dem
generierten Modul aus `crates/endpoint-e2e/tests/ocaml.rs::gen_module`
(gen.ml — `Gen.Ping`/`Gen.Pong`, eigenes `module Gen.Wire`) und
dem SDK `zerodds.ml` (eigenes `module Zerodds.Wire`) kompiliert. Beide
Wire-Module bleiben getrennte Kompiliereinheiten, sodass keine
Namenskollision entsteht (Kommentar im Test dokumentiert das explizit).

**Tests:** (Linux)
- `xrce_8_3_5_8_ocaml_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `Zerodds.Client`.
- `xrce_8_3_5_8_ocaml_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `Zerodds.AsyncReader`.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `Sender.submit`/`pending_heartbeat`/
`recv_acknack`/`get_in_flight`; `Receiver.recv_data`/`drain_in_order`/
`pending_acknack`/`reset`. Window 16, Receiver-Buffer 64, Heartbeat 500 ms,
Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der async-entkoppelte
`Reliable.Writer`: der Producer enqueued wait-free über `Mailbox.put`
(Mutex sperren, Cons, `Condition.signal`, entsperren — kein Syscall), ein
dedizierter Drain-`Thread` hält den `Sender`-State und den UDP-`Unix`-Socket
und macht die gesamte I/O (`Writer.tick`: Mailbox drainen → `Sender.submit`
→ WRITE_DATA senden, Heartbeat ticken, ACKNACK per `Unix.select` mit 20ms-
Timeout pollen und darauf retransmittieren) — der Producer geht nie in den
Kernel.

**Repo:** `endpoints/ocaml/reliable.ml` — `reliable_write_frame`/
`parse_write_frame`, `heartbeat_frame`/`parse_heartbeat`,
`acknack_frame`/`parse_acknack`; `module Sender`, `module Receiver`;
`module Mailbox` (eigene Kopie, unabhängig von `zerodds.ml`s `Mailbox`);
`module Writer` (`create` spawnt den Drain-`Thread`,
`enqueue`/`wait_drained`/`stop`/`in_flight_count`);
`endpoints/ocaml/example_reliable.ml` (lauffähige In-Process-Demo, kein
Socket).

**Tests:** (Linux)
- `endpoints/ocaml/reliable_test.ml` in der Spalte `ocaml` von `tools/endpoints-gate`
  (Job `integration-endpoints`)
  kompiliert und läuft `endpoints/ocaml/reliable_test.ml` — ein einzelnes
  Script (keine benannten `Test*`-Funktionen wie bei Go, sondern ~34
  sequenzielle `check`-Assertions in einem `let () = ...`-Block), das
  abdeckt: monotone seq (`monotonic seq 0`/`1`, `in-flight count`),
  Payload-zu-groß (`payload too large`), Window-full (`fill window`/
  `window full`), Heartbeat first/silence/nach-Periode/leer (`heartbeat
  body`, `heartbeat silenced <500ms`, `heartbeat after 500ms`, `no heartbeat
  when empty`), ACKNACK Teil-/Voll-Clear (`acknack clears acked`/`seq2
  retransmittable`, `acknack full clear`), Receiver In-Order/Reorder/Dedup/
  Buffer-full (`in-order drain[0]`/`[1]`/`shape`, `expected advanced`,
  `reorder: only seq0`/`reorder: 1+2`, `duplicate dropped`, `fill recv
  buffer`/`recv buffer full`), Pending-ACKNACK-Bitmap (`slot 0 missing`/
  `slot 2 missing`/`slot 1 present`/`slot 3 present`), Reset (`reset clears
  receiver`), In-Process-End-to-End-Loss-Recovery (`submit e2e`, `only seq0
  before recovery`, `seq1 retransmittable`, `seq1+2 after recovery`) und
  Byte-Golden (`heartbeat byte-golden (hardcoded)` ==
  `80 00 01 00 0b 01 05 00 01 00 03 00 80`, `acknack byte-golden
  (hardcoded)` == `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identisch zu
  den Referenz-Goldens; optional zusätzlich Byte-Identität gegen die von
  `zerodds-endpoint-golden` frisch generierten `.bin`-Dateien, wenn das
  Rust-Golden-Tool erfolgreich lief). Druckt `ALL OK` und Exit-Code 0 bei
  Erfolg.
- `reliable_5_3_ocaml_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die
  App, die der Test aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und dem `Writer` von `reliable.ml` baut,
  retransmittiert auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge
  geliefert, der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`).
- `endpoints/ocaml/example_reliable.ml` in der Spalte `ocaml` von `tools/endpoints-gate`
  (Job `integration-endpoints`) — `example_reliable` läuft und meldet
  `delivered: 0 1 2 ... 11` + `RELIABLE OK`.

**Status:** done.

## §6 Latenz — Mailbox-Enqueue vs. inline `sendto`

**Spec:** §5.4 — der Producer-Pfad des `Reliable.Writer`
(`enqueue` → `Mailbox.put`) muss messbar unter dem inline `sendto`-Syscall
liegen — der Beleg, dass Async-Write die Syscall-Latenz aus dem
Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/ocaml/reliable_bench.ml` — Median über 500 Batches à
200 Iterationen (`Unix.gettimeofday` hat nur Mikrosekunden-Auflösung, daher
Batch-Timing) von inline `Unix.sendto` (echter Kernelübergang) vs.
`Mailbox.put` (Mutex sperren, Cons, Signal, entsperren, kein Syscall), kein
Live-Peer nötig (Loopback-Socket, nur lokale Dispatch-Kosten unter Messung).

**Tests:** (Linux) `reliable_5_4_ocaml_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/ocaml_reliable.rs`) — Mailbox-Enqueue-Median
**~30 ns** vs. inline `sendto`-Median **~4,1 µs** (~130–140×). Genauer
Messwert schwankt pro Lauf (Batch-Median über `Unix.gettimeofday`); der Test
prüft, dass der Enqueue-Median der Zeile `producer latency: ...` unter dem
inline-Median liegt, nicht den konkreten Faktor.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-ocaml-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 1 Test grün.
* `python3 ci/run_integration.py integration-bindings-gnat-beam` (Job `integration-bindings-gnat-beam`) — 4 Tests grün.

5 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
