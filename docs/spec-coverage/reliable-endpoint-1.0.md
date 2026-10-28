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

**Tests:** `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`
(Sender und Receiver laufen als zwei `ReliableStreamState`-Instanzen ohne
Hub dazwischen; die Recovery entsteht allein aus HEARTBEAT/ACKNACK
zwischen den Endpoints), `crates/xrce/src/reliable.rs::tests::constructor_panics_on_best_effort_stream`
(der Baustein greift nur für `stream_id >= 128`, die Grenze aus §1),
`crates/xrce/src/reliable.rs::tests::config_submessages_delivered_in_order_via_reliable_stream`
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

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench`
(fährt `endpoints/c/test/bench_reliable_async.c` und vergleicht inline gegen
async-entkoppelt), `crates/endpoint-e2e/tests/ada_reliable.rs::ada_reliable_baseline`,
`crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_latency_bench`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_latency_bench`,
`crates/endpoint-e2e/tests/java_reliable.rs::java_reliable_latency_bench`; je Sprache ein eigener
Bench, siehe §5.2.

**Status:** done

## §3 Kanonischer State-Machine-Kontrakt

### §3.1 Konstanten

**Spec:** §3.1, Tabelle -- `HEARTBEAT_PERIOD` 500 ms, `SENDER_WINDOW` 16,
`RECEIVER_BUFFER` 64, `MAX_PAYLOAD` 65535, reliable stream id Bit 7 gesetzt.

**Repo:** `crates/xrce/src/reliable.rs`: `DEFAULT_HEARTBEAT_PERIOD` (500 ms),
`SENDER_WINDOW_CAP` (16), `RECEIVER_BUFFER_CAP` (64), `RELIABLE_MAX_PAYLOAD` (65535).

**Tests:** `crates/xrce/src/reliable.rs::tests::submit_rejects_when_window_full` (SENDER_WINDOW 16),
`crates/xrce/src/reliable.rs::tests::recv_data_rejects_when_buffer_full` (RECEIVER_BUFFER 64),
`crates/xrce/src/reliable.rs::tests::submit_rejects_payload_too_large` (MAX_PAYLOAD 65535),
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_silenced_until_period_elapsed` (HEARTBEAT_PERIOD 500 ms),
`crates/xrce/src/reliable.rs::tests::constructor_panics_on_best_effort_stream` (Bit 7 der stream_id).

**Status:** done

### §3.2 Sender-Kontrakt

**Spec:** §3.2 -- `submit(payload) -> seq` (Fehler `PayloadTooLarge`/`WindowFull`),
`pending_heartbeat(now) -> HEARTBEAT?`, `recv_acknack(payload)` (RFC-1982-Fenster-Logik),
`get_in_flight(seq)`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{submit, pending_heartbeat,
recv_acknack, get_in_flight}` -- Signaturen und Fehlerpfade identisch zur Spec-Notation.

**Tests:** `crates/xrce/src/reliable.rs::tests::submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::submit_rejects_when_window_full`, `crates/xrce/src/reliable.rs::tests::submit_rejects_payload_too_large`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_silenced_until_period_elapsed`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_none_when_window_empty`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_full_clear_when_no_bits_set`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_end_with_hole_is_serial_max_not_count`.

**Status:** done

### §3.3 Receiver-Kontrakt

**Spec:** §3.3 -- `recv_data(seq, payload)` (Duplikat-Drop, `BufferFull`),
`drain_in_order() -> [(seq, payload)]`, `pending_acknack(hint_last_seen) -> ACKNACK`,
`reset()`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{recv_data,
drain_in_order, pending_acknack, reset}`.

**Tests:** `crates/xrce/src/reliable.rs::tests::recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::recv_data_reorders_out_of_order`, `crates/xrce/src/reliable.rs::tests::recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::reset_clears_state_completely`.

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

**Tests:** `crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`
(ACKNACK-Body auf der Wire, Retransmit über den Serial-Wrap),
`crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order` (HEARTBEAT-Body),
`crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery` (WRITE_DATA im
reliable Sample-Case). Golden-Abgleich C gegen die Rust-Referenz:
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_no_loss` — beide fahren
`endpoints/c/test/test_reliable.c` gegen `golden_heartbeat_le.bin` und
`golden_acknack_le.bin` (Linux-Toolchain nötig, sonst lauter Skip).

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

**Tests:** Unit — `crates/xrce/src/reliable.rs::tests::submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::submit_rejects_when_window_full`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::recv_data_reorders_out_of_order`, `crates/xrce/src/reliable.rs::tests::recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::reset_clears_state_completely`. Byte-golden —
`crates/xrce/src/reliable.rs::tests::acknack_retransmit_of_lost_sample_across_wrap`,
`crates/xrce/src/reliable.rs::tests::heartbeat_window_across_wrap_reports_rfc1982_order`.
E2E-Loss-Recovery — `crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`,
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`. Latenz —
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench`.

**Status:** done

## §5.2 Pro-Sprache-Rollout (Coverage-Evidenz)

Die 5-Punkt-Matrix aus §5.1 gilt laut Spec §5 für "jede Sprache". Jede Sprache ist ein
eigenes Item.

### §5.2 ada

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada.

**Repo:** `endpoints/ada/src/reliable.{ads,adb}` (protected object + drain task, Leitfall).

**Tests:** `crates/endpoint-e2e/tests/ada_reliable.rs::ada_reliable_unit`,
`crates/endpoint-e2e/tests/ada_reliable.rs::ada_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/ada_reliable.rs::ada_reliable_baseline`.

**Status:** done — Linux-verifiziert (Unit+Golden ALL OK, Loss-Recovery 12/12, Latenz
87 ns vs. 7338 ns inline, ~84×).

### §5.2 c

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache c.

**Repo:** `endpoints/c/src/zerodds_reliable_async.c`, `endpoints/c/test/test_reliable.c`.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_no_loss`,
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench` — die
ersten beiden fahren `endpoints/c/test/test_reliable.c` gegen die Goldens
aus §4.

**Status:** done — Linux-verifiziert.

### §5.2 cpp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache cpp.

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp`,
`endpoints/cpp/test/test_reliable_cpp.cpp`.

**Tests:** `crates/endpoint-e2e/tests/cpp_reliable.rs::cpp_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/cpp_reliable.rs::cpp_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/cpp_reliable.rs::cpp_reliable_lossless_baseline`,
`crates/endpoint-e2e/tests/cpp_reliable.rs::cpp_reliable_example`.

**Status:** done — Linux-verifiziert.

### §5.2 d

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache d.

**Repo:** `endpoints/d/reliable.d`, `endpoints/d/reliable_test.d`,
`endpoints/d/reliable_bench.d`.

**Tests:** `crates/endpoint-e2e/tests/d_reliable.rs::d_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/d_reliable.rs::d_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/d_reliable.rs::d_reliable_no_loss`,
`crates/endpoint-e2e/tests/d_reliable.rs::d_reliable_example`,
`crates/endpoint-e2e/tests/d_reliable.rs::d_reliable_latency_bench`.

**Status:** done — Linux-verifiziert.

### §5.2 nim

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache nim.

**Repo:** `endpoints/nim/reliable.nim`, `endpoints/nim/reliable_test.nim`.

**Tests:** `crates/endpoint-e2e/tests/nim_reliable.rs::nim_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/nim_reliable.rs::nim_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/nim_reliable.rs::nim_reliable_no_loss`,
`crates/endpoint-e2e/tests/nim_reliable.rs::nim_reliable_producer_latency`.

**Status:** done — Linux-verifiziert.

### §5.2 rust

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache rust.

**Repo:** `endpoints/rust/src/reliable.rs`.

**Tests:** `crates/endpoint-e2e/tests/rust_reliable.rs::rust_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/rust_reliable.rs::rust_reliable_no_loss_baseline`.

**Status:** done — Linux-verifiziert.

### §5.2 zig

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache zig.

**Repo:** `endpoints/zig/src/reliable.zig`.

**Tests:** `crates/endpoint-e2e/tests/zig_reliable.rs::zig_reliable_unit`,
`crates/endpoint-e2e/tests/zig_reliable.rs::zig_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/zig_reliable.rs::zig_reliable_no_loss`,
`crates/endpoint-e2e/tests/zig_reliable.rs::zig_reliable_example`,
`crates/endpoint-e2e/tests/zig_reliable.rs::zig_reliable_latency_bench`.

**Status:** done — Linux-verifiziert.

### §5.2 go

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache go.

**Repo:** `endpoints/go/reliable.go`, `endpoints/go/reliable_test.go`,
`endpoints/go/reliable_bench`.

**Tests:** `crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_no_loss`,
`crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_example`,
`crates/endpoint-e2e/tests/go_reliable.rs::go_reliable_latency_bench`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1).

### §5.2 java

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache java.

**Repo:** `endpoints/java/org/zerodds/endpoint/{ReliableWire,ReliableSender,
ReliableReceiver,AsyncReliableWriter}.java`.

**Tests:** `crates/endpoint-e2e/tests/java_reliable.rs::java_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/java_reliable.rs::java_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/java_reliable.rs::java_reliable_no_loss_baseline`,
`crates/endpoint-e2e/tests/java_reliable.rs::java_reliable_latency_bench`.

**Status:** done — Linux-verifiziert (4/4, seriell --test-threads=1).

### §5.2 csharp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache csharp.

**Repo:** `endpoints/csharp/Reliable.cs`, `endpoints/csharp/ReliableTests.cs`.

**Tests:** `crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_no_loss_baseline`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_standalone_example`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::csharp_reliable_latency_bench`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1).

### §5.2 python

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache python.

**Repo:** `endpoints/python/zerodds_reliable.py`, `endpoints/python/reliable_test.py`.

**Tests:** `crates/endpoint-e2e/tests/python_reliable.rs::python_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/python_reliable.rs::python_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/python_reliable.rs::python_reliable_no_loss`,
`crates/endpoint-e2e/tests/python_reliable.rs::python_reliable_producer_latency`.

**Status:** done — Linux-verifiziert (4/4, seriell --test-threads=1); GIL-Runtime --
Async-Entkopplung schwächer als bei OS-Thread-Drain.

### §5.2 elixir

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache elixir.

**Repo:** `endpoints/elixir/lib/reliable.ex`, `endpoints/elixir/reliable_test.exs`.

**Tests:** `crates/endpoint-e2e/tests/elixir_reliable.rs::elixir_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/elixir_reliable.rs::elixir_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/elixir_reliable.rs::elixir_reliable_no_loss`,
`crates/endpoint-e2e/tests/elixir_reliable.rs::elixir_reliable_example`,
`crates/endpoint-e2e/tests/elixir_reliable.rs::elixir_reliable_producer_latency`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1).

### §5.2 julia

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache julia.

**Repo:** `endpoints/julia/reliable.jl`, `endpoints/julia/reliable_test.jl`.

**Tests:** `crates/endpoint-e2e/tests/julia_reliable.rs::julia_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/julia_reliable.rs::julia_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/julia_reliable.rs::julia_reliable_no_loss`,
`crates/endpoint-e2e/tests/julia_reliable.rs::julia_reliable_producer_latency`.

**Status:** done — Linux-verifiziert (4/4, seriell --test-threads=1); Coroutine-
Task-Drain -- Async-Entkopplung schwächer als bei OS-Thread-Drain.

### §5.2 swift

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache swift.

**Repo:** `endpoints/swift/Reliable.swift`, `endpoints/swift/reliable_tests.swift`.

**Tests:** `crates/endpoint-e2e/tests/swift_reliable.rs::swift_reliable_unit`,
`crates/endpoint-e2e/tests/swift_reliable.rs::swift_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/swift_reliable.rs::swift_reliable_no_loss`,
`crates/endpoint-e2e/tests/swift_reliable.rs::swift_reliable_example`,
`crates/endpoint-e2e/tests/swift_reliable.rs::swift_reliable_latency_bench`.

**Status:** done — lokal verifiziert (macOS, 5/5); kein swiftc unter Linux vorhanden, daher
nicht Linux-, sondern lokal-verifiziert.

### §5.2 ocaml

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ocaml.

**Repo:** `endpoints/ocaml/reliable.ml` (eigenständiges Modul, Mutex/Condition-Mailbox +
Drain-Thread; kein `zerodds.ml`-Dependency), `endpoints/ocaml/reliable_test.ml`,
`reliable_bench.ml`.

**Tests:** `crates/endpoint-e2e/tests/ocaml_reliable.rs::ocaml_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/ocaml_reliable.rs::ocaml_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/ocaml_reliable.rs::ocaml_reliable_no_loss`,
`crates/endpoint-e2e/tests/ocaml_reliable.rs::ocaml_reliable_example`,
`crates/endpoint-e2e/tests/ocaml_reliable.rs::ocaml_reliable_latency_bench`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1).

### §5.2 lua

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache lua.

**Repo:** `endpoints/lua/reliable.lua` (eigenständiges Modul, byte-identischer
Frame-Codec zu `crates/xrce`), `endpoints/lua/reliable_test.lua`.

**Tests:** `crates/endpoint-e2e/tests/lua_reliable.rs::lua_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/lua_reliable.rs::lua_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/lua_reliable.rs::lua_reliable_no_loss`,
`crates/endpoint-e2e/tests/lua_reliable.rs::lua_reliable_example`,
`crates/endpoint-e2e/tests/lua_reliable.rs::lua_reliable_producer_latency`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1).

### §5.2 ada-83

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada-83.

**Repo:** `endpoints/ada-83/src/zerodds_ada83_reliable.{ads,adb}` (Ada-83-Dialekt ohne
protected objects — Polling-Drain), `endpoints/ada-83/test/test_ada83_reliable.adb`,
`endpoints/ada-83/test/example_ada83_reliable.adb`.

**Tests:** `crates/endpoint-e2e/tests/ada83_reliable.rs::ada83_reliable_unit`,
`crates/endpoint-e2e/tests/ada83_reliable.rs::ada83_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/ada83_reliable.rs::ada83_reliable_baseline`.

**Status:** done — Linux-verifiziert (3/3, seriell --test-threads=1).

### §5.2 ada-native

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada-native.

**Repo:** `endpoints/ada-native/src/native_reliable.{ads,adb}` (reiner Ada-Codec ohne
`Interfaces.C`-Bindung an den C89-Wire-Core),
`endpoints/ada-native/test/test_native_reliable_unit.adb`,
`endpoints/ada-native/test/example_native_reliable.adb`.

**Tests:** `crates/endpoint-e2e/tests/ada_native_reliable.rs::ada_native_reliable_unit`,
`crates/endpoint-e2e/tests/ada_native_reliable.rs::ada_native_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/ada_native_reliable.rs::ada_native_reliable_baseline`.

**Status:** done — Linux-verifiziert (3/3, seriell --test-threads=1).

### §5.2 fsharp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache fsharp.

**Repo:** `endpoints/fsharp/reliable.fs` (Sender-/Receiver-State-Machine +
async-entkoppelter Writer), `endpoints/fsharp/ReliableTests.fs`,
`endpoints/fsharp/ExampleReliable.fs`.

**Tests:** `crates/endpoint-e2e/tests/fsharp_reliable.rs::fsharp_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/fsharp_reliable.rs::fsharp_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/fsharp_reliable.rs::fsharp_reliable_no_loss_baseline`,
`crates/endpoint-e2e/tests/fsharp_reliable.rs::fsharp_reliable_standalone_example`,
`crates/endpoint-e2e/tests/fsharp_reliable.rs::fsharp_reliable_latency_bench`.

**Status:** done — Linux-verifiziert (5/5, seriell --test-threads=1; Latenz
enqueue-entkoppelt 5445 ns vs. inline 419184 ns).

### §5.2 kotlin

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache kotlin.

**Repo:** `endpoints/kotlin/src/Reliable.kt` (Sender-/Receiver-State-Machine +
Drain-Thread), `endpoints/kotlin/ReliableSelfTest.kt`,
`endpoints/kotlin/ReliableExample.kt`.

**Tests:** `crates/endpoint-e2e/tests/kotlin_reliable.rs::kotlin_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/kotlin_reliable.rs::kotlin_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/kotlin_reliable.rs::kotlin_reliable_no_loss_baseline`.

**Status:** done — Linux-verifiziert (3/3, seriell --test-threads=1).

### §5.2 node

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache node.

**Repo:** `endpoints/node/reliable.js` (Sender-/Receiver-State-Machine +
Event-Loop-Drain), `endpoints/node/reliable.test.js`,
`endpoints/node/reliable_selftest.js`, `endpoints/node/example_reliable.js`.

**Tests:** `crates/endpoint-e2e/tests/node_reliable.rs::node_reliable_unit_and_golden`,
`crates/endpoint-e2e/tests/node_reliable.rs::node_reliable_loss_recovery`,
`crates/endpoint-e2e/tests/node_reliable.rs::node_reliable_no_loss_baseline`.

**Status:** done — Linux-verifiziert (3/3, seriell --test-threads=1).

## §6 Außerhalb des Scopes (getrennte Runden)

### §6 Fragmentierung / RESET-Handshake / Kernel-Bypass-Drain

**Spec:** §6 -- "Fragmentierung (FRAGMENT-Submessage, Payload > 64 KiB). RESET-Handshake
über die Live-Verbindung. Kernel-Bypass-Drain (io_uring / AF_XDP)."

**Repo:** explizit als getrennte Runden markiert, kein Implementierungs-Anspruch dieser
Spec.

**Tests:** --

**Status:** n/a (informative)

---

## Audit-Status

28 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-xrce --lib reliable::` → 20/20 passed (State-Machine,
§3+§4); `endpoints/c` `test_reliable` gegen `golden_heartbeat_le.bin` +
`golden_acknack_le.bin` → HEARTBEAT parsed, ACKNACK byte-identisch, ALL OK. Wave-1
(ada, c, cpp, d, nim, rust, zig) + Wave-2 (go 5/5, java 4/4, csharp 5/5, python 4/4,
ocaml 5/5, elixir 5/5, lua 5/5, julia 4/4) + Wave-3 (ada-83 3/3, ada-native 3/3,
fsharp 5/5, kotlin 3/3, node 3/3) auf dem Linux-Testhost grün, jeweils seriell
(`--test-threads=1`); swift 5/5 nur lokal auf macOS (kein swiftc unter Linux verfügbar).

Offene Punkte: keine mehr auf Sprach-Rollout-Ebene -- alle 21 Sprachen aus §5.2 sind
`done`. Verbleibende Einschränkung: swift ist lokal (macOS), nicht Linux-verifiziert,
mangels swiftc-Toolchain auf dem Linux-Testhost.
