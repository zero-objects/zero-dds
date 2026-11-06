# `reliable-endpoint` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/reliable-endpoint-1.0.md` -- Reliable Delivery als
Endpoint-Fähigkeit (ZeroDDS Vendor-Spec, baut auf DDS-XRCE 1.0 §8.4.10/§8.4.11 auf).

Implementation:

- `crates/xrce/src/reliable.rs` -- Referenz-State-Machine (`ReliableStreamState`).
- `crates/xrce/src/submessages/{acknack,heartbeat,write_data}.rs` -- Wire-Codec.
- `endpoints/{ada,ada-83,ada-native,c,cpp,csharp,d,elixir,fsharp,go,java,julia,kotlin,
  lua,nim,node,ocaml,python,rust,swift,zig}/` --
  Sprach-SDKs (reliable-Modul + Beispiele + Unit-Tests je Sprache).
- `crates/endpoint-e2e/tests/*_reliable.rs` -- Live-E2E gegen den Rust-Referenz-Peer.

## §1 Entscheidung — Reliable Delivery liegt im Endpoint, nicht am Hub

### §1 Hub/Endpoint-Schnitt

**Spec:** §1 -- "Hub: Discovery (SPDP/SEDP), QoS-Matching, Routing. Endpoint (Pflicht):
reliable Delivery — ein stateful Writer und Reader. Ein begrenzter, komponierbarer
Baustein (XRCE Reliable Stream, `stream_id ≥ 128`), kein Voll-Stack."

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState` trägt den kompletten
reliable Writer-/Reader-State pro Endpoint (kein Hub-seitiges Pendant im Repo);
Discovery-Code (`crates/discovery/`) berührt reliable Delivery nicht.

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery`
(Sender und Receiver laufen als zwei `ReliableStreamState`-Instanzen ohne
Hub dazwischen; die Recovery entsteht allein aus HEARTBEAT/ACKNACK
zwischen den Endpoints), `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_constructor_panics_on_best_effort_stream`
(der Baustein greift nur für `stream_id >= 128`, die Grenze aus §1),
`crates/xrce/src/reliable.rs::tests::reliable_1_config_submessages_delivered_in_order_via_reliable_stream`
(derselbe Baustein trägt auch den Config-Kanal — komponierbar, kein
Voll-Stack).

**Status:** done

## §2 Zwei Achsen, ein Bauteil

### §2 drain-Task hält den reliable State

**Spec:** §2 -- "Async-Write und Reliability sind dasselbe Bauteil: der drain-Task des
Async-Writers hält den reliable State." Producer `enqueue` bleibt wait-free/kernel-frei;
der drain-Task ruft `submit`, batched `sendmmsg`, sendet HEARTBEAT, verarbeitet ACKNACK,
retransmit'et aus der History.

**Repo:** `crates/xrce/src/reliable.rs` als sprachneutrale Referenz; konkrete
drain-Task-Andockpunkte je Sprache: `endpoints/ada/src/reliable.adb` (protected object +
drain task), `endpoints/c/src/zerodds_reliable_async.c`
(`endpoints/c/include/zerodds_reliable_async.h`), `endpoints/rust/src/reliable.rs`,
`endpoints/cpp/include/zerodds_reliable.hpp`, `endpoints/go/reliable.go`,
`endpoints/d/reliable.d`, `endpoints/nim/reliable.nim`, `endpoints/zig/src/reliable.zig`.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send`
(fährt `endpoints/c/test/bench_reliable_async.c` und vergleicht inline gegen
async-entkoppelt),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_4_csharp_the_producer_enqueue_costs_less_than_the_inline_send`,
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send`; je Sprache ein eigener
Vergleich, siehe §5.2.

**Status:** done

## §3 Kanonischer State-Machine-Kontrakt

### §3.1 Konstanten

**Spec:** §3.1, Tabelle -- `HEARTBEAT_PERIOD` 500 ms, `SENDER_WINDOW` 16,
`RECEIVER_BUFFER` 64, `MAX_PAYLOAD` 65535, reliable stream id Bit 7 gesetzt.

**Repo:** `crates/xrce/src/reliable.rs`: `DEFAULT_HEARTBEAT_PERIOD` (500 ms),
`SENDER_WINDOW_CAP` (16), `RECEIVER_BUFFER_CAP` (64), `RELIABLE_MAX_PAYLOAD` (65535).

**Tests:** `crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full` (SENDER_WINDOW 16),
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full` (RECEIVER_BUFFER 64),
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_payload_too_large` (MAX_PAYLOAD 65535),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_pending_heartbeat_silenced_until_period_elapsed` (HEARTBEAT_PERIOD 500 ms),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_constructor_panics_on_best_effort_stream` (Bit 7 der stream_id).

**Status:** done

### §3.2 Sender-Kontrakt

**Spec:** §3.2 -- `submit(payload) -> seq` (Fehler `PayloadTooLarge`/`WindowFull`),
`pending_heartbeat(now) -> HEARTBEAT?`, `recv_acknack(payload)` (RFC-1982-Fenster-Logik),
`get_in_flight(seq)`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{submit, pending_heartbeat,
recv_acknack, get_in_flight}` -- Signaturen und Fehlerpfade identisch zur Spec-Notation.

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full`, `crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_payload_too_large`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_12_pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_pending_heartbeat_silenced_until_period_elapsed`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_pending_heartbeat_none_when_window_empty`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_2_recv_acknack_full_clear_when_no_bits_set`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_heartbeat_window_across_wrap_reports_rfc1982_order`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_12_heartbeat_window_end_with_hole_is_serial_max_not_count`.

**Status:** done

### §3.3 Receiver-Kontrakt

**Spec:** §3.3 -- `recv_data(seq, payload)` (Duplikat-Drop, `BufferFull`),
`drain_in_order() -> [(seq, payload)]`, `pending_acknack(hint_last_seen) -> ACKNACK`,
`reset()`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{recv_data,
drain_in_order, pending_acknack, reset}`.

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_2_recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_reorders_out_of_order`, `crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_13_reset_clears_state_completely`.

**Status:** done

## §4 Wire-Format (byte-golden)

### §4 ACKNACK / HEARTBEAT / WRITE_DATA byte-golden

**Spec:** §4 -- `AckNack`-Body 5 Byte (`first_unacked_seq_num: i16`,
`nack_bitmap: [u8;2] LE`, `stream_id: u8`), Submessage-ID `0x0A`, Flags `0x01`;
`Heartbeat`-Body (`first_unacked_seq_nr`, `last_unacked_seq_nr`, `stream_id`),
Submessage-ID `0x0B`, Flags `0x01`; `WriteData` Submessage-ID `0x07`, Flags `0x03`
im reliable Sample-Case.

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`ACKNACK_BODY_SIZE = 5`,
`SubmessageId::AckNack = 10`), `crates/xrce/src/submessages/heartbeat.rs`
(`SubmessageId::Heartbeat = 11`), `crates/xrce/src/submessages/write_data.rs`
(`SubmessageId::WriteData = 7`, `DataFormat::Sample` + `FLAG_E_LITTLE_ENDIAN` ergeben
Flags `0x03`). Golden-Vektoren: `endpoints/c/test/test_reliable.c`
(`golden_heartbeat_le.bin`, `golden_acknack_le.bin`).

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_acknack_retransmit_of_lost_sample_across_wrap`
(ACKNACK-Body auf der Wire, Retransmit über den Serial-Wrap),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_heartbeat_window_across_wrap_reports_rfc1982_order` (HEARTBEAT-Body),
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery` (WRITE_DATA im
reliable Sample-Case). Golden-Abgleich C gegen die Rust-Referenz:
`endpoints/c/test/test_reliable.c` gegen `golden_heartbeat_le.bin` und
`golden_acknack_le.bin` in der Spalte `c` von `tools/endpoints-gate` (Job
`integration-endpoints`, `make test GOLDEN_DIR=…`).

**Status:** done — Rust-Referenz und C-Golden byte-identisch; die
Golden-Zusicherung je Sprache steht in §5.2.

## §5 Test- & Beleg-Pflicht (pro Sprache)

### §5.1 Referenz-State-Machine erfüllt die 5-Punkt-Matrix

**Spec:** §5 -- Unit (monotone seq/window-full/heartbeat/acknack-clear/reorder/
duplicate-drop/buffer-full/pending-acknack/reset), Byte-golden, E2E-Loss-Recovery,
Latenz-Bench, Example -- kein false-green, lauter Skip nur bei fehlender Toolchain.

**Repo:** `crates/xrce/src/reliable.rs` (Unit), `crates/xrce/src/submessages/{acknack,
heartbeat}.rs` (Byte-golden), `crates/endpoint-e2e/tests/*_reliable.rs` (Loss-Recovery
gegen den Rust-Referenz-Peer), `endpoints/c/test/bench_reliable_async.c` (Latenz),
`endpoints/*/example_reliable.*` (Example je Sprache).

**Tests:** Unit — `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_12_pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_reorders_out_of_order`, `crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_13_reset_clears_state_completely`. Byte-golden —
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_acknack_retransmit_of_lost_sample_across_wrap`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_heartbeat_window_across_wrap_reports_rfc1982_order`.
E2E-Loss-Recovery — `crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery`,
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams`. Latenz —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send`.

**Status:** done

## §5.2 Pro-Sprache-Rollout (Coverage-Evidenz)

Die 5-Punkt-Matrix aus §5.1 gilt laut Spec §5 für "jede Sprache". Jede Sprache ist ein
eigenes Item.

### §5.2 ada

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada.

**Repo:** `endpoints/ada/src/reliable.{ads,adb}` (protected object + drain task, Leitfall).

**Tests:** `endpoints/ada/test/test_reliable_unit.adb` (Punkte 1 und 2, Spalte `ada` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada_reliable.rs::reliable_5_3_ada_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada_reliable.rs::reliable_5_4_ada_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 c

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache c.

**Repo:** `endpoints/c/src/zerodds_reliable_async.c`, `endpoints/c/test/test_reliable.c`.

**Tests:** `endpoints/c/test/test_reliable_sm.c` (Punkte 1 und 2, Spalte `c` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 cpp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache cpp.

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp`,
`endpoints/cpp/test/test_reliable_cpp.cpp`.

**Tests:** `crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_1_cpp_state_machine_passes_the_reference_cases_and_the_goldens` (Punkte 1, 2 und 4),
`crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_3_cpp_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_5_cpp_example_recovers_its_sequence_without_gaps` (Punkt 5).

**Status:** done

### §5.2 d

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache d.

**Repo:** `endpoints/d/reliable.d`, `endpoints/d/reliable_test.d`,
`endpoints/d/reliable_bench.d`.

**Tests:** `endpoints/d/reliable_test.d` (Punkte 1 und 2, Spalte `d` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/d_reliable.rs::reliable_5_3_d_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/d_reliable.rs::reliable_5_4_d_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/d/example_reliable.d` (Punkt 5, Spalte `d`).

**Status:** done

### §5.2 nim

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache nim.

**Repo:** `endpoints/nim/reliable.nim`, `endpoints/nim/reliable_test.nim`.

**Tests:** `endpoints/nim/reliable_test.nim` (Punkte 1 und 2, Spalte `nim` von `tools/endpoints-gate`, Job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/nim_reliable.rs::reliable_5_3_nim_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/nim_reliable.rs::reliable_5_4_nim_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 rust

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache rust.

**Repo:** `endpoints/rust/src/reliable.rs`.

**Tests:** `endpoints/rust/src/reliable.rs` (Punkte 1 und 2, die Tests der Krate `zerodds-endpoint-rust`),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_4_rust_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_the_committed_rust_ping_is_what_the_generator_writes` (der Lauf von Punkt 3 nutzt die Ausgabe des Generators).

**Status:** done

### §5.2 zig

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache zig.

**Repo:** `endpoints/zig/src/reliable.zig`.

**Tests:** `endpoints/zig/src/reliable.zig` (Punkte 1 und 2, Spalte `zig` von `tools/endpoints-gate`, Job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/zig_reliable.rs::reliable_5_3_zig_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/zig_reliable.rs::reliable_5_4_zig_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/zig/example_reliable.zig` (Punkt 5, Spalte `zig`).

**Status:** done

### §5.2 go

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache go.

**Repo:** `endpoints/go/reliable.go`, `endpoints/go/reliable_test.go`,
`endpoints/go/reliable_bench`.

**Tests:** `endpoints/go/reliable_test.go` (Punkte 1 und 2, Spalte `go` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_3_go_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_5_go_example_recovers_its_sequence_without_gaps` (Punkt 5).

**Status:** done

### §5.2 java

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache java.

**Repo:** `endpoints/java/org/zerodds/endpoint/{ReliableWire,ReliableSender,
ReliableReceiver,AsyncReliableWriter}.java`.

**Tests:** `endpoints/java/ReliableSelfTest.java` (Punkte 1 und 2, Spalte `java` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_3_java_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 csharp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache csharp.

**Repo:** `endpoints/csharp/Reliable.cs`, `endpoints/csharp/ReliableTests.cs`.

**Tests:** `endpoints/csharp/ReliableTests.csproj` (Punkte 1 und 2, Spalte `csharp` von `tools/endpoints-gate`, Job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_3_csharp_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_4_csharp_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_5_csharp_example_recovers_its_sequence_without_gaps` (Punkt 5).

**Status:** done

### §5.2 python

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache python.

**Repo:** `endpoints/python/zerodds_reliable.py`, `endpoints/python/reliable_test.py`.

**Tests:** `endpoints/python/reliable_test.py` (Punkte 1 und 2, Spalte `python` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/python_reliable.rs::reliable_5_3_python_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/python_reliable.rs::reliable_5_4_python_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 elixir

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache elixir.

**Repo:** `endpoints/elixir/lib/reliable.ex`, `endpoints/elixir/reliable_test.exs`.

**Tests:** `endpoints/elixir/reliable_test.exs` (Punkte 1 und 2, Spalte `elixir` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/elixir_reliable.rs::reliable_5_3_elixir_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/elixir_reliable.rs::reliable_5_4_elixir_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/elixir/example_reliable.exs` (Punkt 5, Spalte `elixir`).

**Status:** done

### §5.2 julia

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache julia.

**Repo:** `endpoints/julia/reliable.jl`, `endpoints/julia/reliable_test.jl`.

**Tests:** `endpoints/julia/reliable_test.jl` (Punkte 1 und 2, Spalte `julia` von `tools/endpoints-gate`, Job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/julia_reliable.rs::reliable_5_3_julia_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/julia_reliable.rs::reliable_5_4_julia_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 swift

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache swift.

**Repo:** `endpoints/swift/Reliable.swift`, `endpoints/swift/reliable_tests.swift`.

**Tests:** `endpoints/swift/reliable_tests.swift` (Punkte 1 und 2, Spalte `swift` von `tools/endpoints-gate`, Job `integration-bindings-swift`),
`crates/endpoint-e2e/tests/swift_reliable.rs::reliable_5_3_swift_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-swift`),
`crates/endpoint-e2e/tests/swift_reliable.rs::reliable_5_4_swift_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/swift/example_reliable.swift` (Punkt 5, Spalte `swift`).

**Status:** done

### §5.2 ocaml

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ocaml.

**Repo:** `endpoints/ocaml/reliable.ml` (eigenständiges Modul, Mutex/Condition-Mailbox +
Drain-Thread; kein `zerodds.ml`-Dependency), `endpoints/ocaml/reliable_test.ml`,
`reliable_bench.ml`.

**Tests:** `endpoints/ocaml/reliable_test.ml` (Punkte 1 und 2, Spalte `ocaml` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/ocaml_reliable.rs::reliable_5_3_ocaml_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ocaml_reliable.rs::reliable_5_4_ocaml_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/ocaml/example_reliable.ml` (Punkt 5, Spalte `ocaml`).

**Status:** done

### §5.2 lua

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache lua.

**Repo:** `endpoints/lua/reliable.lua` (eigenständiges Modul, byte-identischer
Frame-Codec zu `crates/xrce`), `endpoints/lua/reliable_test.lua`.

**Tests:** `endpoints/lua/reliable_test.lua` (Punkte 1 und 2, Spalte `lua` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/lua_reliable.rs::reliable_5_3_lua_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/lua_reliable.rs::reliable_5_4_lua_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`endpoints/lua/example_reliable.lua` (Punkt 5, Spalte `lua`).

**Status:** done

### §5.2 ada-83

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada-83.

**Repo:** `endpoints/ada-83/src/zerodds_ada83_reliable.{ads,adb}` (Ada-83-Dialekt ohne
protected objects — Polling-Drain), `endpoints/ada-83/src/zerodds_ada83_async_writer.{ads,adb}`
(entkoppelter Writer: ein Ada-83-Task als Drain über den SPSC-Ring
`endpoints/ada-83/src/zerodds_ada83_ring.{ads,adb}`), `endpoints/ada-83/test/test_ada83_reliable.adb`,
`endpoints/ada-83/test/example_ada83_reliable.adb`.

**Tests:** `endpoints/ada-83/test/test_ada83_reliable.adb` (Punkte 1 und 2, Spalte `ada-83` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada83_reliable.rs::reliable_5_3_ada83_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada83_reliable.rs::reliable_5_4_ada83_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 ada-native

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada-native.

**Repo:** `endpoints/ada-native/src/native_reliable.{ads,adb}` (reiner Ada-Codec ohne
`Interfaces.C`-Bindung an den C89-Wire-Core),
`endpoints/ada-native/test/test_native_reliable_unit.adb`,
`endpoints/ada-native/test/example_native_reliable.adb`.

**Tests:** `endpoints/ada-native/test/test_native_reliable_unit.adb` (Punkte 1 und 2, Spalte `ada-native` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada_native_reliable.rs::reliable_5_3_ada_native_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada_native_reliable.rs::reliable_5_4_ada_native_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 fsharp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache fsharp.

**Repo:** `endpoints/fsharp/reliable.fs` (Sender-/Receiver-State-Machine +
async-entkoppelter Writer), `endpoints/fsharp/ReliableTests.fs`,
`endpoints/fsharp/ExampleReliable.fs`.

**Tests:** `endpoints/fsharp/ReliableTests.fs` (Punkte 1 und 2, Spalte `fsharp` von `tools/endpoints-gate`, Job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_3_fsharp_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_4_fsharp_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_5_fsharp_example_recovers_its_sequence_without_gaps` (Punkt 5).

**Status:** done

### §5.2 kotlin

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache kotlin.

**Repo:** `endpoints/kotlin/src/Reliable.kt` (Sender-/Receiver-State-Machine +
Drain-Thread), `endpoints/kotlin/ReliableSelfTest.kt`,
`endpoints/kotlin/ReliableExample.kt`.

**Tests:** `endpoints/kotlin/ReliableSelfTest.kt` (Punkte 1 und 2, Spalte `kotlin` von `tools/endpoints-gate`, Job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/kotlin_reliable.rs::reliable_5_3_kotlin_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/kotlin_reliable.rs::reliable_5_4_kotlin_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

### §5.2 node

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache node.

**Repo:** `endpoints/node/reliable.js` (Sender-/Receiver-State-Machine +
Event-Loop-Drain), `endpoints/node/reliable.test.js`,
`endpoints/node/reliable_selftest.js`, `endpoints/node/example_reliable.js`.

**Tests:** `endpoints/node/reliable_selftest.js` (Punkte 1 und 2, Spalte `node` von `tools/endpoints-gate`, Job `integration-endpoints`),
`crates/endpoint-e2e/tests/node_reliable.rs::reliable_5_3_node_samples_arrive_in_order_despite_dropped_datagrams` (Punkt 3, Job `integration-bindings`),
`crates/endpoint-e2e/tests/node_reliable.rs::reliable_5_4_node_the_producer_enqueue_costs_less_than_the_inline_send` (Punkt 4).

**Status:** done

## §6 Außerhalb des Scopes (getrennte Runden)

### §6 Fragmentierung (FRAGMENT-Submessage, Payload > 64 KiB)

**Spec:** §6 — "Fragmentierung (FRAGMENT-Submessage, Payload > 64 KiB)." Die Spec führt sie als getrennte Runde.

**Repo:** —

**Tests:** —

**Status:** open — keine FRAGMENT-Submessage in `crates/xrce/src/reliable.rs` und in den Endpoints.

### §6 RESET-Handshake über die Live-Verbindung

**Spec:** §6 — "RESET-Handshake über die Live-Verbindung." Die Spec führt ihn als getrennte Runde.

**Repo:** —

**Tests:** —

**Status:** open — es gibt `reset()` für den lokalen Zustand (§3), aber keinen Austausch eines RESET zwischen Sender und Empfänger.

### §6 Kernel-Bypass-Drain (io_uring / AF_XDP)

**Spec:** §6 — "Kernel-Bypass-Drain (io_uring / AF_XDP)." Die Spec führt ihn als Optimierung nach der `sendmmsg`-Basis in einer getrennten Runde.

**Repo:** —

**Tests:** —

**Status:** open — kein Code mit `io_uring` oder `AF_XDP` in den Endpoints.

---

## Audit-Status

28 done / 0 partial / 3 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9101 von `main` (`python3 ci/coverage_results.py --document reliable-endpoint-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-xrce --all-targets` (Job `merge-rust-xrce`) — 20 Tests grün.
* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 18 Tests grün.
* `python3 ci/run_integration.py integration-bindings-dotnet` (Job `integration-bindings-dotnet`) — 6 Tests grün.
* `python3 ci/run_integration.py integration-bindings-gnat-beam` (Job `integration-bindings-gnat-beam`) — 12 Tests grün.
* `python3 ci/run_integration.py integration-bindings-jvm-julia` (Job `integration-bindings-jvm-julia`) — 4 Tests grün.
* `python3 ci/run_integration.py integration-bindings-swift` (Job `integration-bindings-swift`) — 2 Tests grün.
* `python3 ci/run_integration.py integration-bindings-zig-nim` (Job `integration-bindings-zig-nim`) — 4 Tests grün.

66 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `reliable-endpoint-1.0.open.md`.
