# `zerodds-endpoint-julia` 1.0 — Spec-Coverage

**Quelle:** docs/specs/zerodds-endpoint-julia-1.0.md — ZeroDDS Julia
Endpoint-SDK-Spec. Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Julia`; hier steht der Transport.

Implementation:

- `endpoints/julia/zerodds.jl` (`module ZeroDDS`) — XRCE-Framing
  (`write_frame`/`read_frame`), sync `Client`, `Task`+`Channel`-`AsyncReader`.
- `endpoints/julia/reliable.jl` (`module Reliable`) — reliable Sender/
  Receiver-State-Machine + HEARTBEAT/ACKNACK-Wire-Codec.
- `endpoints/julia/reliable_app.jl` — Channel + Drain-`Task`
  (`ReliableAsyncWriter`-Vorgang) für das Live-E2E + den Latenz-Bench.
- `endpoints/julia/reliable_test.jl` — Unit- + Byte-Golden-Suite.
- `crates/endpoint-e2e/tests/julia.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/julia_reliable.rs` — reliable-Stream-E2E.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/julia/zerodds.jl::write_frame`/`read_frame`, Konstanten
`SESSION_NOKEY` (`0x80`) und `STREAM_BEST_EFFORT` (`0x01`).

**Tests:** Framing wird live über `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong` und
`xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong` (§4) geübt — kein separater roher-UDP-Test wie bei
Go/C (Julia hat keinen isolierten Framing-only-Testpfad).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — pollender `Client`: `write!` framet + liefert synchron, `poll`
ist ein nicht-blockierender Einzel-Receive (`nothing` bei leer).

**Repo:** `endpoints/julia/zerodds.jl::Transport` (`deliver`/`receive`-
Closures, der einzige Integrationspunkt); `endpoints/julia/zerodds.jl::Client`
(`Client(t)`, `write!`, `poll`, monotoner `seq`-Zähler, Default-Session
`SESSION_NOKEY`/`STREAM_BEST_EFFORT`).

**Tests:** Live-E2E `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/julia.rs`) — voller Stack (generierte
`Ping`/`Pong`-Typen + `ZeroDDS.Client`) über einen echten UDP-Socket gegen den
Rust-XRCE-Peer.

**Status:** done.

## §3 Async `AsyncReader`

**Spec:** §3 — ein `@async`-`Task` pollt den `Transport` und schiebt
entrahmte Sample-Bodies auf einen `Channel` (push); der Consumer blockiert
auf `take!`. Kein separater `AsyncWriter`-Typ (Senden bleibt `write!` auf dem
sync `Client`).

**Repo:** `endpoints/julia/zerodds.jl::AsyncReader` (`start_reader` spawnt den
Empfangs-`Task`, `Samples`-Channel `ch`, `running::Ref{Bool}`-Flag, `recv` =
`take!`, `stop!` setzt `running[] = false`).

**Tests:** Live-E2E `xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong`
(`crates/endpoint-e2e/tests/julia.rs`) — voller Stack über
`ZeroDDS.start_reader`/`ZeroDDS.Client.write!` gegen den Rust-XRCE-Peer.

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine Julia-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus: voller Stack (generierte
Typen aus `zerodds_lang_rules::generate::source`, dem Aufruf von `zerodds-idlc generate --lang julia`, + Endpoint-SDK) sync und async.

**Repo:** `crates/endpoint-e2e/tests/julia.rs` — `JULIA_APP` (baut die
generierten `Ping`/`Pong`-Typen als `module Gen` neben `zerodds.jl` ein, um
den Namenskonflikt `Endian`/`Writer`/`LE` zwischen generiertem Code und der
SDK zu vermeiden; Modus `sync`/`async` per CLI-Argument; ein `armed`-Flag
serialisiert den ersten `recvfrom`-Arm gegen den libuv-Datagram-Drop, bevor
der erste `send` läuft).

**Tests:** (Linux)
- `xrce_8_3_5_8_julia_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `ZeroDDS.Client`.
- `xrce_8_3_5_8_julia_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `ZeroDDS.start_reader`.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `Sender.submit!`/`pending_heartbeat!`/
`recv_acknack!`/`get_in_flight`; `Receiver.recv_data!`/`drain_in_order!`/
`pending_acknack`/`reset!`. Window 16, Receiver-Buffer 64, Heartbeat 500 ms,
Payload ≤ 65535, RFC-1982 16-bit Sequenznummern (`seq_lt`/`seq_gt`). Dazu der
async-entkoppelte `ReliableAsyncWriter`-Vorgang: der Producer enqueued in
einen `Channel`, ein dedizierter Drain-`Task` hält den `Sender`-State und
macht die gesamte I/O (senden, Heartbeat, ACKNACK-getriebenes Retransmit) —
der Producer geht nie in den Kernel.

**Repo:** `endpoints/julia/reliable.jl` (`module Reliable`) —
`reliable_write_frame`/`heartbeat_frame`/`acknack_frame`/`parse_heartbeat`/
`parse_acknack`; `Sender`, `Receiver`; `endpoints/julia/reliable_app.jl` —
`run_reliable` (Producer-Loop, `Channel` + `@async drain`-Task, leerer
Payload als Sentinel für „keine weiteren Samples"), `run_bench` (§6);
`endpoints/julia/example_reliable.jl` (lauffähige In-Process-Demo, kein
Socket).

**Tests:** (Linux)
- `endpoints/julia/reliable_test.jl` in der Spalte `julia` von `tools/endpoints-gate`
  (Job `integration-bindings-jvm-julia`) läuft
  `julia reliable_test.jl <golden_dir>` — 26 `check(...)`-Assertions:
  monotone seq (`endpointjulia_5_submit_assigns_monotonic_seq_0`/`_1`,
  `endpointjulia_5_submit_two_in_flight`), Payload-zu-groß
  (`endpointjulia_5_submit_rejects_payload_too_large`), Window-full
  (`endpointjulia_5_submit_rejects_when_window_full`), Heartbeat first/body/silence/nach-
  Periode/leer (`endpointjulia_5_heartbeat_fires_first_time`,
  `endpointjulia_5_heartbeat_body_first_last_stream`, `endpointjulia_5_heartbeat_silenced_before_period`,
  `endpointjulia_5_heartbeat_fires_after_period`, `endpointjulia_5_heartbeat_none_when_window_empty`),
  ACKNACK Teil-/Voll-Clear (`endpointjulia_5_acknack_clears_acked_keeps_missing`,
  `endpointjulia_5_acknack_full_clear_when_no_bits_set`), Receiver In-Order/Reorder/Dedup/
  Buffer-full (`endpointjulia_5_recv_data_buffers_in_order`, `endpointjulia_5_recv_data_reorder_blocks_on_gap`,
  `endpointjulia_5_recv_data_reorder_delivers`, `endpointjulia_5_recv_data_drops_duplicates`,
  `endpointjulia_5_recv_data_rejects_when_buffer_full`), Pending-ACKNACK-Bitmap
  (`endpointjulia_5_pending_acknack_marks_missing_slots`), Reset (`endpointjulia_5_reset_clears_state`),
  In-Process-End-to-End-Loss-Recovery (`endpointjulia_5_e2e_drain_blocks_on_lost_s1`,
  `endpointjulia_5_e2e_s1_retransmittable`, `endpointjulia_5_e2e_delivers_all_after_retransmit`),
  Byte-Golden gegen die per `zerodds-xrce` erzeugten Referenz-Goldens
  (`endpointjulia_5_byte_golden_heartbeat`, `endpointjulia_5_byte_golden_acknack`) sowie deren
  Parse-Rundreise (`endpointjulia_5_golden_heartbeat_parse`, `endpointjulia_5_golden_acknack_parse`).
  Erwartet `stdout` enthält `ALL OK`.
- `reliable_5_3_julia_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die
  App (`reliable_app.jl` `run_reliable`) retransmittiert auf ACKNACK; alle 12
  Samples lückenlos in Reihenfolge geliefert.

**Status:** done.

## §6 Latenz — Channel-Enqueue vs. inline `send`

**Spec:** §5.3 — der Producer-Pfad des `ReliableAsyncWriter`-Vorgangs
(Enqueue → `Channel`-Push) muss messbar unter dem inline `send`
(UDP-`sendto`)-Syscall liegen — der Beleg, dass Async-Write die
Syscall-Latenz aus dem Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/julia/reliable_app.jl::run_bench` — 20000 Iterationen
inline `send(sock, ...)` (UDP-`sendto`) vs. 20000 Iterationen `put!(chan,
sample)` auf einen Sink-`Task`, kein Live-Peer nötig (beliebiger
Loopback-Port, nur lokale Dispatch-Kosten unter Messung). Julias `Channel`
ist Lock+Condvar-basiert, kein wait-free SPSC-Ring — im Spec-Text als
bekannte Grenze dokumentiert (`reliable_app.jl` Kopfkommentar), das Enqueue
bleibt trotzdem deutlich unter dem inline-Syscall.

**Tests:** (Linux) `reliable_5_4_julia_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/julia_reliable.rs`) — Channel-Enqueue **80–106 ns**
vs. inline `send` **6.1–6.5 µs** (~60–75×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-julia-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-jvm-julia` (Job `integration-bindings-jvm-julia`) — 29 Tests grün.

29 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine. Bekannte Grenze (kein Spec-Verstoß): Julias `Channel`
ist Lock+Condvar-basiert, kein wait-free SPSC-Ring (§6).
