# `reliable-endpoint` 1.0 -- Spec-Coverage

**Source:** `docs/specs/reliable-endpoint-1.0.md` -- Reliable Delivery as an
Endpoint Capability (ZeroDDS Vendor-Spec, builds on DDS-XRCE 1.0 §8.4.10/§8.4.11).

Implementation:

- `crates/xrce/src/reliable.rs` -- reference state machine (`ReliableStreamState`).
- `crates/xrce/src/submessages/{acknack,heartbeat,write_data}.rs` -- wire codec.
- `endpoints/{ada,ada-83,ada-native,c,cpp,csharp,d,elixir,fsharp,go,java,julia,kotlin,
  lua,nim,node,ocaml,python,rust,swift,zig}/` --
  language SDKs (reliable module + examples + unit tests per language).
- `crates/endpoint-e2e/tests/*_reliable.rs` -- live E2E against the Rust reference peer.

## §1 Decision — reliable delivery lives in the endpoint, not the hub

### §1 Hub/endpoint cut

**Spec:** §1 -- "Hub: discovery (SPDP/SEDP), QoS matching, routing. Endpoint
(mandatory): reliable delivery — a stateful writer and reader. A bounded, composable
building block (XRCE reliable stream, `stream_id ≥ 128`), not a full stack."

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState` carries the complete
reliable writer/reader state per endpoint (no hub-side counterpart in the repo);
discovery code (`crates/discovery/`) does not touch reliable delivery.

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery`
(sender and receiver run as two `ReliableStreamState` instances with no hub
between them; the recovery comes from HEARTBEAT/ACKNACK between the
endpoints alone), `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_constructor_panics_on_best_effort_stream`
(the building block only applies to `stream_id >= 128`, the §1 boundary),
`crates/xrce/src/reliable.rs::tests::reliable_1_config_submessages_delivered_in_order_via_reliable_stream`
(the same block also carries the config channel — composable, not a full
stack).

**Status:** done

## §2 Two axes, one component

### §2 The drain task holds the reliable state

**Spec:** §2 -- "Async-write and reliability are the same component: the drain task of
the async writer holds the reliable state." The producer's `enqueue` stays wait-free/
kernel-free; the drain task calls `submit`, batches `sendmmsg`, sends HEARTBEAT,
processes ACKNACK, retransmits from history.

**Repo:** `crates/xrce/src/reliable.rs` as the language-neutral reference; concrete
drain-task dock points per language: `endpoints/ada/src/reliable.adb` (protected object
+ drain task), `endpoints/c/src/zerodds_reliable_async.c`
(`endpoints/c/include/zerodds_reliable_async.h`), `endpoints/rust/src/reliable.rs`,
`endpoints/cpp/include/zerodds_reliable.hpp`, `endpoints/go/reliable.go`,
`endpoints/d/reliable.d`, `endpoints/nim/reliable.nim`, `endpoints/zig/src/reliable.zig`.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send`
(runs `endpoints/c/test/bench_reliable_async.c` and compares inline against
async-decoupled),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send`,
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_4_csharp_the_producer_enqueue_costs_less_than_the_inline_send`,
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send`; each language has its
own comparison, see §5.2.

**Status:** done

## §3 Canonical state-machine contract

### §3.1 Constants

**Spec:** §3.1, table -- `HEARTBEAT_PERIOD` 500 ms, `SENDER_WINDOW` 16,
`RECEIVER_BUFFER` 64, `MAX_PAYLOAD` 65535, reliable stream id bit 7 set.

**Repo:** `crates/xrce/src/reliable.rs`: `DEFAULT_HEARTBEAT_PERIOD` (500 ms),
`SENDER_WINDOW_CAP` (16), `RECEIVER_BUFFER_CAP` (64), `RELIABLE_MAX_PAYLOAD` (65535).

**Tests:** `crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full` (SENDER_WINDOW 16),
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full` (RECEIVER_BUFFER 64),
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_payload_too_large` (MAX_PAYLOAD 65535),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_pending_heartbeat_silenced_until_period_elapsed` (HEARTBEAT_PERIOD 500 ms),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_constructor_panics_on_best_effort_stream` (bit 7 of the stream id).

**Status:** done

### §3.2 Sender contract

**Spec:** §3.2 -- `submit(payload) -> seq` (errors `PayloadTooLarge`/`WindowFull`),
`pending_heartbeat(now) -> HEARTBEAT?`, `recv_acknack(payload)` (RFC-1982 window
logic), `get_in_flight(seq)`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{submit, pending_heartbeat,
recv_acknack, get_in_flight}` -- signatures and error paths identical to the spec
notation.

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

### §3.3 Receiver contract

**Spec:** §3.3 -- `recv_data(seq, payload)` (duplicate drop, `BufferFull`),
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

## §4 Wire format (byte-golden)

### §4 ACKNACK / HEARTBEAT / WRITE_DATA byte-golden

**Spec:** §4 -- `AckNack` body 5 bytes (`first_unacked_seq_num: i16`,
`nack_bitmap: [u8;2] LE`, `stream_id: u8`), submessage id `0x0A`, flags `0x01`;
`Heartbeat` body (`first_unacked_seq_nr`, `last_unacked_seq_nr`, `stream_id`),
submessage id `0x0B`, flags `0x01`; `WriteData` submessage id `0x07`, flags `0x03` in
the reliable sample case.

**Repo:** `crates/xrce/src/submessages/acknack.rs` (`ACKNACK_BODY_SIZE = 5`,
`SubmessageId::AckNack = 10`), `crates/xrce/src/submessages/heartbeat.rs`
(`SubmessageId::Heartbeat = 11`), `crates/xrce/src/submessages/write_data.rs`
(`SubmessageId::WriteData = 7`, `DataFormat::Sample` + `FLAG_E_LITTLE_ENDIAN` yield
flags `0x03`). Golden vectors: `endpoints/c/test/test_reliable.c`
(`golden_heartbeat_le.bin`, `golden_acknack_le.bin`).

**Tests:** `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_acknack_retransmit_of_lost_sample_across_wrap`
(the ACKNACK body on the wire, retransmit across the serial wrap),
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_heartbeat_window_across_wrap_reports_rfc1982_order` (the HEARTBEAT body),
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery` (WRITE_DATA in the
reliable sample case). The C golden against the Rust reference:
`endpoints/c/test/test_reliable.c` against `golden_heartbeat_le.bin` and
`golden_acknack_le.bin` in the `c` column of `tools/endpoints-gate` (job
`integration-endpoints`, `make test GOLDEN_DIR=…`).

**Status:** done — Rust reference and C golden byte-identical; the per-language
golden assertion is in §5.2.

## §5 Test & evidence obligation (per language)

### §5.1 Reference state machine satisfies the 5-point matrix

**Spec:** §5 -- unit (monotonic seq/window-full/heartbeat/acknack-clear/reorder/
duplicate-drop/buffer-full/pending-acknack/reset), byte-golden, E2E loss recovery,
latency bench, example -- no false-green, loud-skip only when the toolchain is absent.

**Repo:** `crates/xrce/src/reliable.rs` (unit), `crates/xrce/src/submessages/{acknack,
heartbeat}.rs` (byte-golden), `crates/endpoint-e2e/tests/*_reliable.rs` (loss recovery
against the Rust reference peer), `endpoints/c/test/bench_reliable_async.c` (latency),
`endpoints/*/example_reliable.*` (example per language).

**Tests:** unit — `crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_12_pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_reorders_out_of_order`, `crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_13_reset_clears_state_completely`. Byte-golden —
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_acknack_retransmit_of_lost_sample_across_wrap`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_heartbeat_window_across_wrap_reports_rfc1982_order`.
E2E loss recovery — `crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery`,
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams`. Latency —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send`.

**Status:** done

## §5.2 Per-language rollout (coverage evidence)

The 5-point matrix from §5.1 applies, per spec §5, to "every language". Each language
is its own item.

### §5.2 ada

**Spec:** §5 -- 5-point matrix for the ada language.

**Repo:** `endpoints/ada/src/reliable.{ads,adb}` (protected object + drain task, lead
case).

**Tests:** `endpoints/ada/test/test_reliable_unit.adb` (points 1 and 2, the `ada` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada_reliable.rs::reliable_5_3_ada_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada_reliable.rs::reliable_5_4_ada_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 c

**Spec:** §5 -- 5-point matrix for the c language.

**Repo:** `endpoints/c/src/zerodds_reliable_async.c`, `endpoints/c/test/test_reliable.c`.

**Tests:** `endpoints/c/test/test_reliable_sm.c` (points 1 and 2, the `c` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 cpp

**Spec:** §5 -- 5-point matrix for the cpp language.

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp`,
`endpoints/cpp/test/test_reliable_cpp.cpp`.

**Tests:** `crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_1_cpp_state_machine_passes_the_reference_cases_and_the_goldens` (points 1, 2 and 4),
`crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_3_cpp_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/cpp_reliable.rs::reliable_5_5_cpp_example_recovers_its_sequence_without_gaps` (point 5).

**Status:** done

### §5.2 d

**Spec:** §5 -- 5-point matrix for the d language.

**Repo:** `endpoints/d/reliable.d`, `endpoints/d/reliable_test.d`,
`endpoints/d/reliable_bench.d`.

**Tests:** `endpoints/d/reliable_test.d` (points 1 and 2, the `d` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/d_reliable.rs::reliable_5_3_d_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/d_reliable.rs::reliable_5_4_d_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/d/example_reliable.d` (point 5, the `d` column).

**Status:** done

### §5.2 nim

**Spec:** §5 -- 5-point matrix for the nim language.

**Repo:** `endpoints/nim/reliable.nim`, `endpoints/nim/reliable_test.nim`.

**Tests:** `endpoints/nim/reliable_test.nim` (points 1 and 2, the `nim` column of `tools/endpoints-gate`, job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/nim_reliable.rs::reliable_5_3_nim_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/nim_reliable.rs::reliable_5_4_nim_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 rust

**Spec:** §5 -- 5-point matrix for the rust language.

**Repo:** `endpoints/rust/src/reliable.rs`.

**Tests:** `endpoints/rust/src/reliable.rs` (points 1 and 2, the tests of the crate `zerodds-endpoint-rust`),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_5_4_rust_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`crates/endpoint-e2e/tests/rust_reliable.rs::reliable_the_committed_rust_ping_is_what_the_generator_writes` (the run of point 3 uses the generator's output).

**Status:** done

### §5.2 zig

**Spec:** §5 -- 5-point matrix for the zig language.

**Repo:** `endpoints/zig/src/reliable.zig`.

**Tests:** `endpoints/zig/src/reliable.zig` (points 1 and 2, the `zig` column of `tools/endpoints-gate`, job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/zig_reliable.rs::reliable_5_3_zig_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-zig-nim`),
`crates/endpoint-e2e/tests/zig_reliable.rs::reliable_5_4_zig_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/zig/example_reliable.zig` (point 5, the `zig` column).

**Status:** done

### §5.2 go

**Spec:** §5 -- 5-point matrix for the go language.

**Repo:** `endpoints/go/reliable.go`, `endpoints/go/reliable_test.go`,
`endpoints/go/reliable_bench`.

**Tests:** `endpoints/go/reliable_test.go` (points 1 and 2, the `go` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_3_go_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`crates/endpoint-e2e/tests/go_reliable.rs::reliable_5_5_go_example_recovers_its_sequence_without_gaps` (point 5).

**Status:** done

### §5.2 java

**Spec:** §5 -- 5-point matrix for the java language.

**Repo:** `endpoints/java/org/zerodds/endpoint/{ReliableWire,ReliableSender,
ReliableReceiver,AsyncReliableWriter}.java`.

**Tests:** `endpoints/java/ReliableSelfTest.java` (points 1 and 2, the `java` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_3_java_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/java_reliable.rs::reliable_5_4_java_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 csharp

**Spec:** §5 -- 5-point matrix for the csharp language.

**Repo:** `endpoints/csharp/Reliable.cs`, `endpoints/csharp/ReliableTests.cs`.

**Tests:** `endpoints/csharp/ReliableTests.csproj` (points 1 and 2, the `csharp` column of `tools/endpoints-gate`, job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_3_csharp_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_4_csharp_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`crates/endpoint-e2e/tests/csharp_reliable.rs::reliable_5_5_csharp_example_recovers_its_sequence_without_gaps` (point 5).

**Status:** done

### §5.2 python

**Spec:** §5 -- 5-point matrix for the python language.

**Repo:** `endpoints/python/zerodds_reliable.py`, `endpoints/python/reliable_test.py`.

**Tests:** `endpoints/python/reliable_test.py` (points 1 and 2, the `python` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/python_reliable.rs::reliable_5_3_python_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/python_reliable.rs::reliable_5_4_python_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 elixir

**Spec:** §5 -- 5-point matrix for the elixir language.

**Repo:** `endpoints/elixir/lib/reliable.ex`, `endpoints/elixir/reliable_test.exs`.

**Tests:** `endpoints/elixir/reliable_test.exs` (points 1 and 2, the `elixir` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/elixir_reliable.rs::reliable_5_3_elixir_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/elixir_reliable.rs::reliable_5_4_elixir_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/elixir/example_reliable.exs` (point 5, the `elixir` column).

**Status:** done

### §5.2 julia

**Spec:** §5 -- 5-point matrix for the julia language.

**Repo:** `endpoints/julia/reliable.jl`, `endpoints/julia/reliable_test.jl`.

**Tests:** `endpoints/julia/reliable_test.jl` (points 1 and 2, the `julia` column of `tools/endpoints-gate`, job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/julia_reliable.rs::reliable_5_3_julia_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/julia_reliable.rs::reliable_5_4_julia_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 swift

**Spec:** §5 -- 5-point matrix for the swift language.

**Repo:** `endpoints/swift/Reliable.swift`, `endpoints/swift/reliable_tests.swift`.

**Tests:** `endpoints/swift/reliable_tests.swift` (points 1 and 2, the `swift` column of `tools/endpoints-gate`, job `integration-bindings-swift`),
`crates/endpoint-e2e/tests/swift_reliable.rs::reliable_5_3_swift_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-swift`),
`crates/endpoint-e2e/tests/swift_reliable.rs::reliable_5_4_swift_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/swift/example_reliable.swift` (point 5, the `swift` column).

**Status:** done

### §5.2 ocaml

**Spec:** §5 -- 5-point matrix for the ocaml language.

**Repo:** `endpoints/ocaml/reliable.ml` (self-contained module, mutex/condition
mailbox + drain thread; no `zerodds.ml` dependency), `endpoints/ocaml/reliable_test.ml`,
`reliable_bench.ml`.

**Tests:** `endpoints/ocaml/reliable_test.ml` (points 1 and 2, the `ocaml` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/ocaml_reliable.rs::reliable_5_3_ocaml_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ocaml_reliable.rs::reliable_5_4_ocaml_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/ocaml/example_reliable.ml` (point 5, the `ocaml` column).

**Status:** done

### §5.2 lua

**Spec:** §5 -- 5-point matrix for the lua language.

**Repo:** `endpoints/lua/reliable.lua` (self-contained module, byte-identical frame
codec to `crates/xrce`), `endpoints/lua/reliable_test.lua`.

**Tests:** `endpoints/lua/reliable_test.lua` (points 1 and 2, the `lua` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/lua_reliable.rs::reliable_5_3_lua_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/lua_reliable.rs::reliable_5_4_lua_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`endpoints/lua/example_reliable.lua` (point 5, the `lua` column).

**Status:** done

### §5.2 ada-83

**Spec:** §5 -- 5-point matrix for the ada-83 language.

**Repo:** `endpoints/ada-83/src/zerodds_ada83_reliable.{ads,adb}` (the Ada 83 dialect
without protected objects — polling drain),
`endpoints/ada-83/src/zerodds_ada83_async_writer.{ads,adb}` (the decoupled writer:
an Ada 83 task drains the SPSC ring `endpoints/ada-83/src/zerodds_ada83_ring.{ads,adb}`),
`endpoints/ada-83/test/test_ada83_reliable.adb`,
`endpoints/ada-83/test/example_ada83_reliable.adb`.

**Tests:** `endpoints/ada-83/test/test_ada83_reliable.adb` (points 1 and 2, the `ada-83` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada83_reliable.rs::reliable_5_3_ada83_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada83_reliable.rs::reliable_5_4_ada83_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 ada-native

**Spec:** §5 -- 5-point matrix for the ada-native language.

**Repo:** `endpoints/ada-native/src/native_reliable.{ads,adb}` (a pure Ada codec without
the `Interfaces.C` binding to the C89 wire core),
`endpoints/ada-native/test/test_native_reliable_unit.adb`,
`endpoints/ada-native/test/example_native_reliable.adb`.

**Tests:** `endpoints/ada-native/test/test_native_reliable_unit.adb` (points 1 and 2, the `ada-native` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/ada_native_reliable.rs::reliable_5_3_ada_native_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-gnat-beam`),
`crates/endpoint-e2e/tests/ada_native_reliable.rs::reliable_5_4_ada_native_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 fsharp

**Spec:** §5 -- 5-point matrix for the fsharp language.

**Repo:** `endpoints/fsharp/reliable.fs` (sender/receiver state machine + the
async-decoupled writer), `endpoints/fsharp/ReliableTests.fs`,
`endpoints/fsharp/ExampleReliable.fs`.

**Tests:** `endpoints/fsharp/ReliableTests.fs` (points 1 and 2, the `fsharp` column of `tools/endpoints-gate`, job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_3_fsharp_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-dotnet`),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_4_fsharp_the_producer_enqueue_costs_less_than_the_inline_send` (point 4),
`crates/endpoint-e2e/tests/fsharp_reliable.rs::reliable_5_5_fsharp_example_recovers_its_sequence_without_gaps` (point 5).

**Status:** done

### §5.2 kotlin

**Spec:** §5 -- 5-point matrix for the kotlin language.

**Repo:** `endpoints/kotlin/src/Reliable.kt` (sender/receiver state machine + drain
thread), `endpoints/kotlin/ReliableSelfTest.kt`,
`endpoints/kotlin/ReliableExample.kt`.

**Tests:** `endpoints/kotlin/ReliableSelfTest.kt` (points 1 and 2, the `kotlin` column of `tools/endpoints-gate`, job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/kotlin_reliable.rs::reliable_5_3_kotlin_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings-jvm-julia`),
`crates/endpoint-e2e/tests/kotlin_reliable.rs::reliable_5_4_kotlin_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

### §5.2 node

**Spec:** §5 -- 5-point matrix for the node language.

**Repo:** `endpoints/node/reliable.js` (sender/receiver state machine + event-loop
drain), `endpoints/node/reliable.test.js`,
`endpoints/node/reliable_selftest.js`, `endpoints/node/example_reliable.js`.

**Tests:** `endpoints/node/reliable_selftest.js` (points 1 and 2, the `node` column of `tools/endpoints-gate`, job `integration-endpoints`),
`crates/endpoint-e2e/tests/node_reliable.rs::reliable_5_3_node_samples_arrive_in_order_despite_dropped_datagrams` (point 3, job `integration-bindings`),
`crates/endpoint-e2e/tests/node_reliable.rs::reliable_5_4_node_the_producer_enqueue_costs_less_than_the_inline_send` (point 4).

**Status:** done

## §6 Out of scope (separate rounds)

### §6 Fragmentation (FRAGMENT submessage, payload > 64 KiB)

**Spec:** §6 — "Fragmentation (FRAGMENT submessage, payload > 64 KiB)." The spec lists it as a separate round.

**Repo:** —

**Tests:** —

**Status:** open — no FRAGMENT submessage in `crates/xrce/src/reliable.rs` and in the endpoints.

### §6 RESET handshake over the live connection

**Spec:** §6 — "RESET handshake over the live connection." The spec lists it as a separate round.

**Repo:** —

**Tests:** —

**Status:** open — there is `reset()` for the local state (§3), but no exchange of a RESET between sender and receiver.

### §6 Kernel-bypass drain (io_uring / AF_XDP)

**Spec:** §6 — "Kernel-bypass drain (io_uring / AF_XDP)." The spec lists it as an optimisation after the `sendmmsg` baseline in a separate round.

**Repo:** —

**Tests:** —

**Status:** open — no code with `io_uring` or `AF_XDP` in the endpoints.

---

## Audit-Status

28 done / 0 partial / 3 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9101 of `main` (`python3 ci/coverage_results.py --document reliable-endpoint-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-xrce --all-targets` (job `merge-rust-xrce`) — 20 tests green.
* `python3 ci/run_integration.py integration-bindings` (job `integration-bindings`) — 18 tests green.
* `python3 ci/run_integration.py integration-bindings-dotnet` (job `integration-bindings-dotnet`) — 6 tests green.
* `python3 ci/run_integration.py integration-bindings-gnat-beam` (job `integration-bindings-gnat-beam`) — 12 tests green.
* `python3 ci/run_integration.py integration-bindings-jvm-julia` (job `integration-bindings-jvm-julia`) — 4 tests green.
* `python3 ci/run_integration.py integration-bindings-swift` (job `integration-bindings-swift`) — 2 tests green.
* `python3 ci/run_integration.py integration-bindings-zig-nim` (job `integration-bindings-zig-nim`) — 4 tests green.

66 cited tests green, 0 not green.

Open items: `reliable-endpoint-1.0.open.en.md`.
