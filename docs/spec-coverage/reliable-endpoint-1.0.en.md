# `reliable-endpoint` 1.0 -- Spec-Coverage

**Source:** `docs/specs/reliable-endpoint-1.0.md` -- Reliable Delivery as an
Endpoint Capability (ZeroDDS Vendor-Spec, builds on DDS-XRCE 1.0 §8.4.10/§8.4.11).

Implementation:

- `crates/xrce/src/reliable.rs` -- reference state machine (`ReliableStreamState`).
- `crates/xrce/src/submessages/{acknack,heartbeat,write_data}.rs` -- wire codec.
- `endpoints/{ada,c,cpp,d,go,java,csharp,python,elixir,julia,nim,rust,swift,zig}/` --
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

**Tests:** --

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

**Tests:** `endpoints/c/test/bench_reliable_async.c` (latency bench inline vs.
async-decoupled); each language has its own bench/test, see §5.2.

**Status:** done

## §3 Canonical state-machine contract

### §3.1 Constants

**Spec:** §3.1, table -- `HEARTBEAT_PERIOD` 500 ms, `SENDER_WINDOW` 16,
`RECEIVER_BUFFER` 64, `MAX_PAYLOAD` 65535, reliable stream id bit 7 set.

**Repo:** `crates/xrce/src/reliable.rs`: `DEFAULT_HEARTBEAT_PERIOD` (500 ms),
`SENDER_WINDOW_CAP` (16), `RECEIVER_BUFFER_CAP` (64), `RELIABLE_MAX_PAYLOAD` (65535).

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` (part of the 17 tests,
including window-full and buffer-full assertions against exactly these constants).

**Status:** done

### §3.2 Sender contract

**Spec:** §3.2 -- `submit(payload) -> seq` (errors `PayloadTooLarge`/`WindowFull`),
`pending_heartbeat(now) -> HEARTBEAT?`, `recv_acknack(payload)` (RFC-1982 window
logic), `get_in_flight(seq)`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{submit, pending_heartbeat,
recv_acknack, get_in_flight}` -- signatures and error paths identical to the spec
notation.

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` covers monotonic `seq`,
`WindowFull`, `PayloadTooLarge`, heartbeat first/silence/empty, ACKNACK-clear.

**Status:** done

### §3.3 Receiver contract

**Spec:** §3.3 -- `recv_data(seq, payload)` (duplicate drop, `BufferFull`),
`drain_in_order() -> [(seq, payload)]`, `pending_acknack(hint_last_seen) -> ACKNACK`,
`reset()`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{recv_data,
drain_in_order, pending_acknack, reset}`.

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` covers reorder,
duplicate-drop, `BufferFull`, pending-acknack bitmap, `reset()`.

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

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` (wire roundtrip); `endpoints/c`
`test_reliable` against `golden_heartbeat_le.bin` + `golden_acknack_le.bin` --
HEARTBEAT parsed, ACKNACK byte-identical, ALL OK.

**Status:** done (Rust reference + C golden byte-identical); per-language golden
assertion in §5.2.

## §5 Test & evidence obligation (per language)

### §5.1 Reference state machine satisfies the 5-point matrix

**Spec:** §5 -- unit (monotonic seq/window-full/heartbeat/acknack-clear/reorder/
duplicate-drop/buffer-full/pending-acknack/reset), byte-golden, E2E loss recovery,
latency bench, example -- no false-green, loud-skip only when the toolchain is absent.

**Repo:** `crates/xrce/src/reliable.rs` (unit), `crates/xrce/src/submessages/{acknack,
heartbeat}.rs` (byte-golden), `crates/endpoint-e2e/tests/*_reliable.rs` (loss recovery
against the Rust reference peer), `endpoints/c/test/bench_reliable_async.c` (latency),
`endpoints/*/example_reliable.*` (example per language).

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` → 17/17 passed;
`endpoints/c` `test_reliable` → ALL OK.

**Status:** done

### §5.2 Per-language rollout (coverage evidence)

The 5-point matrix from §5.1 applies, per spec §5, to "every language". Each language
is its own item.

#### §5.2 ada

**Spec:** §5 -- 5-point matrix for the ada language.

**Repo:** `endpoints/ada/src/reliable.{ads,adb}` (protected object + drain task, lead
case).

**Tests:** `crates/endpoint-e2e/tests/ada_reliable.rs`.

**Status:** done -- codepit-verified (unit+golden ALL OK, loss recovery 12/12, latency
87 ns vs. 7338 ns inline, ~84×).

#### §5.2 c

**Spec:** §5 -- 5-point matrix for the c language.

**Repo:** `endpoints/c/src/zerodds_reliable_async.c`, `endpoints/c/test/test_reliable.c`.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs`; `endpoints/c` `test_reliable`
against the goldens (§4).

**Status:** done -- codepit-verified.

#### §5.2 cpp

**Spec:** §5 -- 5-point matrix for the cpp language.

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp`,
`endpoints/cpp/test/test_reliable_cpp.cpp`.

**Tests:** `crates/endpoint-e2e/tests/cpp_reliable.rs`.

**Status:** done -- codepit-verified.

#### §5.2 d

**Spec:** §5 -- 5-point matrix for the d language.

**Repo:** `endpoints/d/reliable.d`, `endpoints/d/reliable_test.d`,
`endpoints/d/reliable_bench.d`.

**Tests:** `crates/endpoint-e2e/tests/d_reliable.rs`.

**Status:** done -- codepit-verified.

#### §5.2 nim

**Spec:** §5 -- 5-point matrix for the nim language.

**Repo:** `endpoints/nim/reliable.nim`, `endpoints/nim/reliable_test.nim`.

**Tests:** `crates/endpoint-e2e/tests/nim_reliable.rs`.

**Status:** done -- codepit-verified.

#### §5.2 rust

**Spec:** §5 -- 5-point matrix for the rust language.

**Repo:** `endpoints/rust/src/reliable.rs`.

**Tests:** `crates/endpoint-e2e/tests/rust_reliable.rs`.

**Status:** done -- codepit-verified.

#### §5.2 zig

**Spec:** §5 -- 5-point matrix for the zig language.

**Repo:** `endpoints/zig/src/reliable.zig`.

**Tests:** `crates/endpoint-e2e/tests/zig_reliable.rs`.

**Status:** done -- codepit-verified.

#### §5.2 go

**Spec:** §5 -- 5-point matrix for the go language.

**Repo:** `endpoints/go/reliable.go`, `endpoints/go/reliable_test.go`,
`endpoints/go/reliable_bench`.

**Tests:** `crates/endpoint-e2e/tests/go_reliable.rs`.

**Status:** done -- codepit-verified (5/5, serial --test-threads=1).

#### §5.2 java

**Spec:** §5 -- 5-point matrix for the java language.

**Repo:** `endpoints/java/org/zerodds/endpoint/{ReliableWire,ReliableSender,
ReliableReceiver,AsyncReliableWriter}.java`.

**Tests:** `crates/endpoint-e2e/tests/java_reliable.rs`.

**Status:** done -- codepit-verified (4/4, serial --test-threads=1).

#### §5.2 csharp

**Spec:** §5 -- 5-point matrix for the csharp language.

**Repo:** `endpoints/csharp/Reliable.cs`, `endpoints/csharp/ReliableTests.cs`.

**Tests:** `crates/endpoint-e2e/tests/csharp_reliable.rs`.

**Status:** done -- codepit-verified (5/5, serial --test-threads=1).

#### §5.2 python

**Spec:** §5 -- 5-point matrix for the python language.

**Repo:** `endpoints/python/zerodds_reliable.py`, `endpoints/python/reliable_test.py`.

**Tests:** `crates/endpoint-e2e/tests/python_reliable.rs`.

**Status:** done -- codepit-verified (4/4, serial --test-threads=1); GIL runtime --
async decoupling weaker than OS-thread drain.

#### §5.2 elixir

**Spec:** §5 -- 5-point matrix for the elixir language.

**Repo:** `endpoints/elixir/lib/reliable.ex`, `endpoints/elixir/reliable_test.exs`.

**Tests:** `crates/endpoint-e2e/tests/elixir_reliable.rs`.

**Status:** done -- codepit-verified (5/5, serial --test-threads=1).

#### §5.2 julia

**Spec:** §5 -- 5-point matrix for the julia language.

**Repo:** `endpoints/julia/reliable.jl`, `endpoints/julia/reliable_test.jl`.

**Tests:** `crates/endpoint-e2e/tests/julia_reliable.rs`.

**Status:** done -- codepit-verified (4/4, serial --test-threads=1); coroutine-task
drain -- async decoupling weaker than OS-thread drain.

#### §5.2 swift

**Spec:** §5 -- 5-point matrix for the swift language.

**Repo:** `endpoints/swift/Reliable.swift`, `endpoints/swift/reliable_tests.swift`.

**Tests:** `crates/endpoint-e2e/tests/swift_reliable.rs`.

**Status:** done -- verified locally (macOS, 5/5); no codepit swiftc available, so
this is a local, not a codepit, verification.

#### §5.2 ocaml

**Spec:** §5 -- 5-point matrix for the ocaml language.

**Repo:** `endpoints/ocaml/reliable.ml` (self-contained module, mutex/condition
mailbox + drain thread; no `zerodds.ml` dependency), `endpoints/ocaml/reliable_test.ml`,
`reliable_bench.ml`.

**Tests:** `crates/endpoint-e2e/tests/ocaml_reliable.rs`.

**Status:** done -- codepit-verified (5/5, serial --test-threads=1).

#### §5.2 lua

**Spec:** §5 -- 5-point matrix for the lua language.

**Repo:** `endpoints/lua/reliable.lua` (self-contained module, byte-identical frame
codec to `crates/xrce`), `endpoints/lua/reliable_test.lua`.

**Tests:** `crates/endpoint-e2e/tests/lua_reliable.rs`.

**Status:** done -- codepit-verified (5/5, serial --test-threads=1).

## §6 Out of scope (separate rounds)

### §6 Fragmentation / RESET handshake / kernel-bypass drain

**Spec:** §6 -- "Fragmentation (FRAGMENT submessage, payload > 64 KiB). RESET
handshake over the live link. Kernel-bypass drain (io_uring / AF_XDP)."

**Repo:** explicitly marked as separate rounds, no implementation claim of this spec.

**Tests:** --

**Status:** n/a (informative)

---

## Audit-Status

23 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-xrce --lib reliable::` → 17/17 passed (state machine,
§3+§4); `endpoints/c` `test_reliable` against `golden_heartbeat_le.bin` +
`golden_acknack_le.bin` → HEARTBEAT parsed, ACKNACK byte-identical, ALL OK. Wave-1
(ada, c, cpp, d, nim, rust, zig) + Wave-2 (go 5/5, java 4/4, csharp 5/5, python 4/4,
ocaml 5/5, elixir 5/5, lua 5/5, julia 4/4) green on codepit, each serial
(`--test-threads=1`); swift 5/5 local-only on macOS (no codepit swiftc available).

Open items: none remaining at the language-rollout level -- all 16 languages in §5.2
are `done`. Remaining caveat: swift is local (macOS), not codepit-verified, for lack of
a swiftc toolchain on codepit.
