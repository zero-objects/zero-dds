# `zerodds-endpoint-ocaml` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-ocaml-1.0.md` — ZeroDDS OCaml
Endpoint-SDK-Spec. Ergänzt die Codegen-Coverage `zerodds-xcdr2-ocaml`
(`docs/spec-coverage/zerodds-xcdr2-ocaml-1.0.md`) — dort das Marshalling,
hier der Transport.

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
  Unit/Golden + Example + Latenz-Bench.

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
`go_raw_udp`); das Framing wird über `ocaml_endpoint_sync`/
`ocaml_endpoint_async` (§4) live geübt — die App framet dort explizit über
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

**Tests:** Live-E2E `ocaml_endpoint_sync`
(`crates/endpoint-e2e/tests/ocaml.rs`) — die eingebettete OCaml-App
(`OCAML_MAIN`) baut ein `Gen.Ping`-Sample über den generierten
`zerodds-idlc --ocaml`-Codec, sendet es über `Zerodds.Client.write` an den
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

**Tests:** Live-E2E `ocaml_endpoint_async`
(`crates/endpoint-e2e/tests/ocaml.rs`) — die App startet
`Zerodds.AsyncReader.start`, liefert das geframeте `Gen.Ping`-Sample direkt
über `transport.deliver`, blockiert in `AsyncReader.recv` bis zum
`Gen.Pong`-Reply und stoppt den Reader.

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine OCaml-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus, einmal über den sync
`Client`, einmal über `AsyncReader`, jeweils mit dem vollen Stack
(generierte `Gen.Ping`/`Gen.Pong`-Typen aus `zerodds-idlc --ocaml` +
`endpoints/ocaml`).

**Repo:** `crates/endpoint-e2e/tests/ocaml.rs` — `OCAML_MAIN` (eine
eingebettete OCaml-Quelle, per CLI-Argument `sync`/`async`), zusammen mit dem
generierten `gen.ml` (`Gen.Ping`/`Gen.Pong`, eigenes `module Gen.Wire`) und
dem SDK `zerodds.ml` (eigenes `module Zerodds.Wire`) kompiliert. Beide
Wire-Module bleiben getrennte Kompiliereinheiten, sodass keine
Namenskollision entsteht (Kommentar im Test dokumentiert das explizit).

**Tests (codepit):**
- `ocaml_endpoint_sync` — voller Stack über `Zerodds.Client`.
- `ocaml_endpoint_async` — voller Stack über `Zerodds.AsyncReader`.

2/2 grün (codepit).

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
Socket); `endpoints/ocaml/reliable_app.ml` (live UDP-Sender-App für das E2E,
argv = `<peer-port> <N>`).

**Tests (codepit):**
- `ocaml_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/ocaml_reliable.rs`)
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
- `ocaml_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die
  App (`reliable_app`) retransmittiert auf ACKNACK; alle 12 Samples
  lückenlos in Reihenfolge geliefert.
- `ocaml_reliable_no_loss` — lossless Baseline; 12/12.
- `ocaml_reliable_example` — `example_reliable` läuft und meldet
  `delivered: 0 1 2 ... 11` + `RELIABLE OK`.

5/5 grün (codepit), davon 4 in diesem Abschnitt (Latenz-Bench in §6).

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

**Tests (codepit):** `ocaml_reliable_latency_bench`
(`crates/endpoint-e2e/tests/ocaml_reliable.rs`) — Mailbox-Enqueue-Median
**~30 ns** vs. inline `sendto`-Median **~4,1 µs** (~130–140×). Genauer
Messwert schwankt pro Lauf (Batch-Median über `Unix.gettimeofday`); Test
prüft nur, dass die Ausgabe die Zeile `producer latency: ...` enthält, nicht
den konkreten Faktor.

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test ocaml`
2/2 (Ping-Pong: `ocaml_endpoint_sync`/`ocaml_endpoint_async`);
`--test ocaml_reliable` 5/5 (`ocaml_reliable_unit_and_golden` — ~34
sequenzielle Checks inkl. Byte-Golden, `ocaml_reliable_loss_recovery`,
`ocaml_reliable_no_loss`, `ocaml_reliable_example`,
`ocaml_reliable_latency_bench`); Latenz-Bench Mailbox-Enqueue ~30 ns / inline
`sendto` ~4,1 µs (~130–140×).

Offene Punkte: keine.
