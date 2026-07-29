# `zerodds-endpoint-elixir` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-elixir-1.0.md` — ZeroDDS Elixir
Endpoint-SDK-Spec. Ergänzt die Codegen-Coverage `zerodds-xcdr2-elixir`
(`docs/spec-coverage/zerodds-xcdr2-elixir-1.0.md`) — dort das Marshalling,
hier der Transport.

Implementation:

- `endpoints/elixir/lib/zerodds.ex` — XRCE-Framing (`ZeroDDS.Endpoint`), sync
  `ZeroDDS.Client`, async `ZeroDDS.AsyncReader`, `ZeroDDS.MemTransport`.
- `endpoints/elixir/lib/reliable.ex` — reliable Sender/Receiver-State-Machine
  (`ZeroDDS.Reliable.Sender`/`Receiver`) + HEARTBEAT/ACKNACK-Wire-Codec
  (`ZeroDDS.Reliable`) + `ZeroDDS.Reliable.Drain`.
- `crates/endpoint-e2e/tests/elixir.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/elixir_reliable.rs` — reliable-Stream-E2E.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.Endpoint` —
`write_frame/4`/`read_frame/1`, Konstanten `session_nokey/0` (`0x80`) und
`stream_best_effort/0` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/elixir.rs::elixir_raw_udp` (rohes
XCDR2 ohne XRCE-Frame — eigener Mini-Harness); Framing selbst wird über
`elixir_endpoint_sync` und `elixir_endpoint_async` (§4) live geübt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `write/2` framet + liefert synchron,
`poll/1` ist ein nicht-blockierender Einzel-Receive; kein eingebautes
Timeout-Receive — der Aufrufer pollt selbst in einer Deadline-Schleife.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.Client` (`new/1`,
`write/2` mit modulo-`0x10000`-Sequenzzähler, `poll/1`); der
`transport`-Vertrag `%{deliver: fun/1, receive: fun/0}` als einziger
Integrationspunkt; `ZeroDDS.MemTransport` als In-Memory-Referenztransport für
Tests/Beispiele.

**Tests:** `endpoints/elixir/test.exs` — "sync loopback: 5 samples OK"
(`ZeroDDS.Client.write`/`poll` über `MemTransport`, voller Roundtrip von 5
Samples); Live-E2E `elixir_endpoint_sync` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — ein gespawnter Prozess pollt den `transport` und sendet
entrahmte Sample-Bodies als `{:zerodds_sample, body}` an die Mailbox des
`target`; kein separater `AsyncWriter`-Typ, der sync `Client` (§2) ist bereits
der Sendepfad.

**Repo:** `endpoints/elixir/lib/zerodds.ex::ZeroDDS.AsyncReader` (`start/2`
spawnt den Empfangs-Prozess, `stop/1` per `:zerodds_stop`-Message).

**Tests:** `endpoints/elixir/test.exs` — "async loopback: 5 samples OK"
(`ZeroDDS.AsyncReader.start/2` über `MemTransport`, `receive`-Block im
Consumer-Prozess für 5 Samples); Live-E2E `elixir_endpoint_async` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1/§5.2 — eine Elixir-App tauscht mit dem geteilten Rust-XRCE-Peer
über einen echten UDP-Socket ein typisiertes Sample aus: einmal roher
generierter Codec ohne XRCE-Frame, einmal voller Stack (generierte Typen +
Endpoint-SDK) sync und async.

**Repo:** `crates/endpoint-e2e/tests/elixir.rs` — `ELIXIR_RAW_MAIN` (rohes
UDP, kein XRCE-Frame, nutzt nur den generierten
`Zdgen.Ping.marshal_xcdr`/`Zdgen.Pong.unmarshal`), `ELIXIR_ENDPOINT_MAIN`
(`%{deliver:, receive:}`-Transport über `:gen_udp` +
`ZeroDDS`-Modul, Modus `sync`/`async` per CLI-Argument; generierte
`Zdgen.*`-Typen und die `ZeroDDS.*`-SDK liegen in disjunkten Namespaces und
werden nebeneinander geladen).

**Tests (codepit):**
- `elixir_raw_udp` — generierter `Ping`/`Pong`-Codec direkt über einen rohen
  UDP-Socket, ohne XRCE-Framing.
- `elixir_endpoint_sync` — voller Stack über `ZeroDDS.Client`.
- `elixir_endpoint_async` — voller Stack über `ZeroDDS.AsyncReader` (Poll für
  den Write-Teil, Prozess/Mailbox für den Read-Teil).

3/3 grün (codepit).

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ZeroDDS.Reliable.Sender.submit/2`/
`pending_heartbeat/2`/`recv_acknack/2`/`get_in_flight/2`;
`ZeroDDS.Reliable.Receiver.recv_data/3`/`drain_in_order/1`/
`pending_acknack/2`/`reset/1`. Window 16, Receiver-Buffer 64, Heartbeat
500 ms, Payload ≤ 65535, RFC-1982 16-bit Sequenznummern — als unveränderliche
Structs (jeder Call fädelt den Zustand durch und liefert ihn zurück, BEAM
kennt keine Mutation). Dazu der async-entkoppelte `ZeroDDS.Reliable.Drain`:
kein wait-free Ring, sondern ein `GenServer` — der Producer `submit/2`t (ein
`GenServer.cast`, ein Mailbox-Send, kein Kernel-Eintritt), eine dedizierte
Drain-Prozess-Instanz hält den `Sender`-State und den `:gen_udp`-Socket und
macht die gesamte I/O (senden, Heartbeat-Tick alle 50 ms,
ACKNACK-getriebenes Retransmit).

**Repo:** `endpoints/elixir/lib/reliable.ex` — `ZeroDDS.Reliable.write_frame/2`,
`heartbeat_frame/6`/`parse_heartbeat/1`, `acknack_frame/6`/`parse_acknack/1`;
`ZeroDDS.Reliable.Sender`, `ZeroDDS.Reliable.Receiver`;
`ZeroDDS.Reliable.Drain` (`start_link/1`/`activate/1`/`submit/2`/`finish/2`,
FIFO-`drain_pending`-Kaskade, Socket-Ownership-Handoff
`controlling_process` → `activate`); `endpoints/elixir/example_reliable.exs`
(lauffähige In-Process-Demo, kein Socket); `endpoints/elixir/reliable_app.exs`
(live UDP-Sender-App für das E2E, inkl. `bench`-Modus).

**Tests (codepit):**
- `elixir_reliable_unit_and_golden`
  (`crates/endpoint-e2e/tests/elixir_reliable.rs`) läuft
  `elixir -r lib/reliable.ex reliable_test.exs [golden_dir]` — 22
  Unit-Checks: monotone seq (`submit_assigns_monotonic_seqnrs`,
  `submit_two_in_flight`), Payload-zu-groß
  (`submit_rejects_payload_too_large`), Window-full
  (`submit_rejects_when_window_full`), Heartbeat first/body/t0/silence/danach/leer
  (`pending_heartbeat_fires_first_time`/`_body_first_last_stream`/`_fires_at_t0`/
  `_silenced_before_period`/`_fires_after_period`/`_none_when_window_empty`),
  ACKNACK Teil-/Voll-Clear (`recv_acknack_clears_acked_keeps_missing`/
  `_full_clear_when_no_bits_set`), Receiver In-Order/Reorder/Dedup/Buffer-full
  (`recv_data_buffers_in_order`/`_reorder_blocks_on_gap`/`_reorder_delivers`/
  `_drops_duplicates`/`_rejects_when_buffer_full`), Pending-ACKNACK-Bitmap
  (`pending_acknack_marks_missing_slots`), Reset
  (`reset_clears_state_completely`), End-to-End-Loss-Recovery in drei Schritten
  (`e2e_drain_blocks_on_lost_middle_sample`/`e2e_missing_sample_retransmittable`/
  `e2e_delivers_all_after_retransmit`). Dazu — wenn der Rust-Golden-Generator
  (`zerodds-endpoint-golden`) lief — 4 weitere Checks: Byte-Golden
  (`byte_golden_heartbeat`/`byte_golden_acknack`:
  `heartbeat_frame(0x80,0x00,1,1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`,
  `acknack_frame(0x80,0x00,1,1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identisch zu den
  Referenz-Goldens) sowie Parse-Roundtrip
  (`golden_heartbeat_parse`/`golden_acknack_parse`). Kein false-green: läuft
  der Golden-Generator nicht, wird das best-effort geloggt statt stumm
  übersprungen.
- `elixir_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die
  App (`reliable_app.exs`) retransmittet via `ZeroDDS.Reliable.Drain` auf
  ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert.
- `elixir_reliable_no_loss` — lossless Baseline; 12/12.
- `elixir_reliable_example` — `example_reliable.exs` läuft und meldet
  `RELIABLE OK` (12 Samples, Drop-Simulation + ACKNACK-Recovery im
  In-Process-Modell).

5/5 grün (codepit), davon 4 in diesem Abschnitt (Latenz-Bench in §6).

**Status:** done.

## §6 Latenz — GenServer-`submit`-Cast vs. inline `:gen_udp.send`

**Spec:** §5.4 — der Producer-Pfad von `ZeroDDS.Reliable.Drain.submit/2`
(`GenServer.cast` → Mailbox-Send) muss messbar unter dem inline
`:gen_udp.send`-Syscall liegen — der Beleg, dass BEAM-Message-Passing die
Syscall-Latenz aus dem Producer-Pfad nimmt, nicht das Warten auf ACKNACK.
Anders als bei einem wait-free Ring (Go/Zig/Nim) ist die Entkopplung hier ein
GenServer-Cast — ein Mailbox-Send mit Scheduler-Beteiligung, dadurch ein
kleinerer, aber weiterhin klarer Faktor gegenüber dem inline-Syscall.

**Repo:** `endpoints/elixir/reliable_app.exs::ReliableApp.bench/1` — 20000
Iterationen inline `:gen_udp.send` (verbundener Socket) vs. 20000 Iterationen
`ZeroDDS.Reliable.Drain.submit/2`, kein Live-Peer nötig (ein beliebiger
gebundener UDP-Port, nur lokale Dispatch-Kosten unter Messung).

**Tests (codepit):** `elixir_reliable_producer_latency`
(`crates/endpoint-e2e/tests/elixir_reliable.rs`) — `submit`-Cast **452 ns**
vs. inline `:gen_udp.send` **5618 ns** (~12×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test elixir`
3/3 (Ping-Pong: `elixir_raw_udp`/`elixir_endpoint_sync`/`elixir_endpoint_async`);
`--test elixir_reliable` 5/5 (`elixir_reliable_unit_and_golden` — 22
Elixir-Unit-Checks inkl. Byte-Golden, `elixir_reliable_loss_recovery`,
`elixir_reliable_no_loss`, `elixir_reliable_example`,
`elixir_reliable_producer_latency`); Latenz-Bench `submit`-Cast 452 ns /
inline `:gen_udp.send` 5618 ns (~12×).

Offene Punkte: keine.
