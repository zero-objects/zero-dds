# `zerodds-endpoint-cpp` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-cpp-1.0.md` (226 Zeilen) — die
normative C++17-Endpoint-SDK-Spec (XRCE-Framing, sync Client, async
Reader/Writer, reliable Stream). Implementiert in `endpoints/cpp/`: das
XRCE-Framing + der sync Client über den C89-Wire-Core `zdw`
(`endpoints/c`), async Reader/Writer als dünne C++17-Fassade
(`zerodds_async.hpp`), und der header-only reliable Stream
(`zerodds_reliable.hpp`). Ergänzt die Codegen-Coverage `zerodds-xcdr2-cpp`
(`zerodds-xcdr2-cpp-1.0.md`) — dort geht es um den generierten
`topic_type_support<T>`-Codec, hier um den Endpoint, der ihn transportiert.

## §1 XRCE-Framing

**Spec:** `zerodds-endpoint-cpp-1.0.md` §1 -- DDS-XRCE-1.0-Message-Header (8
Byte: session, stream, seq LE, submsg id, flags, len LE) + Body; best-effort
Stream `0x01`.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` — `zdw_xrce_write_frame`
/ `zdw_xrce_read_frame`, `ZDW_XRCE_SESSION_NOKEY`,
`ZDW_XRCE_STREAM_BEST_EFFORT`. Der C++-SDK re-implementiert das Framing
nicht, sondern bindet den C89-Wire-Core direkt ein.

**Tests:** `crates/endpoint-e2e/tests/cpp.rs::run_sync` / `run_async` framen
bzw. deframen jedes Ping/Pong über echtes UDP.

**Status:** done.

## §2 sync Client

**Spec:** `zerodds-endpoint-cpp-1.0.md` §2 -- Ein blockierender
Transport-Poll-Client: `zdw_endpoint_send` / `zdw_endpoint_recv` über einen
integrator-gestellten `zdw_transport` (`deliver`/`receive`-Callbacks,
ADR-0013-Frame-Hook), Marshalling per C++98-Fassade `zerodds::Writer` /
`zerodds::Reader`.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (`zdw_transport`,
`zdw_endpoint_send`/`recv`); `endpoints/cpp/include/zerodds_wire.hpp`
(`zerodds::Writer`/`Reader`, C++98-Fassade über den C89-Kern); C++-Aufrufstelle
`crates/endpoint-e2e/tests/cpp.rs::run_sync` (UDP-Callbacks `udp_deliver` /
`udp_receive`, 100-ms-Poll-Loop bis zu 100 Iterationen);
`endpoints/cpp/example_sync.cpp` (Sensor-Telemetrie-Deep-Example
`Reading{id,value,label}`, C++98-Poll-Loop über eine In-Memory-FIFO, voller
Feld-Decode).

**Tests (codepit, g++/gcc gated):** `cpp_endpoint_sync` — Live-Ping-Pong-E2E
gegen den geteilten Rust-XRCE-Peer über echtes UDP; sowie `cpp_raw_udp`
(bares XCDR2 ohne XRCE-Framing, direkter Codec-Aufruf über die generierten
Typen `topic_type_support<Ping>`/`<Pong>` von `idl-cpp`) als
Codec-Build-Integration derselben App-Kompilierung. 2/2 passed. Lauter Skip
nur bei fehlendem Compiler auf dem `PATH`.

**Status:** done.

## §3 async Reader/Writer

**Spec:** `zerodds-endpoint-cpp-1.0.md` §3 -- Ein event-driven,
nicht-blockierender Reader (Callback pro empfangenem Sample) und ein
Fire-and-forget-Writer, beide dünne C++17-Fassaden über den auditierten
C-Reaktor (`zerodds_async.c`) — additiv zur konservativen C++98-Wire-Fassade
(`zerodds_wire.hpp`).

**Repo:** `endpoints/cpp/include/zerodds_async.hpp` — `zerodds::AsyncReader`
(`poll()` / `run(max)`, RAII, `std::function`-Trampolin über
`zdw_async_reader_init`) und `zerodds::AsyncWriter` (`write()` über
`zdw_async_writer_init` / `zdw_async_write`); `endpoints/cpp/example_async.cpp`
(Sensor-Telemetrie-Deep-Example, C++17-Reactor).

**Tests (codepit, g++/gcc gated):** `crates/endpoint-e2e/tests/cpp.rs::run_async`
— `AsyncReader` dispatcht das empfangene Pong an eine Lambda, `AsyncWriter`
schickt das Ping; Live-Ping-Pong-E2E gegen den Rust-Peer über echtes UDP;
Testname `cpp_endpoint_async`. 1/1 passed. Lauter Skip nur bei fehlendem
Compiler auf dem `PATH`.

**Status:** done.

## §4 Reliable Stream — State-Machine, Async-Writer, Loss-Recovery, Byte-Golden, Latenz

**Spec:** `zerodds-endpoint-cpp-1.0.md` §4 (normativ: `reliable-endpoint-1.0.md`)
-- DDS-XRCE reliable Stream (`stream_id ≥ 128`, §8.4.10/§8.4.11), spiegelt die
Referenz `crates/xrce/src/reliable.rs`: Sender
`submit`/`pending_heartbeat`/`recv_acknack`/`get_in_flight`; Receiver
`recv_data`/`drain_in_order`/`pending_acknack`/`reset`. Der Async-Writer
entkoppelt den Producer vom I/O: `enqueue()` ist wait-free in einen
lock-free SPSC-Ring, ein dedizierter Drain-Thread besitzt den
`Sender`-State, batched `WRITE_DATA` via `sendmmsg`, feuert HEARTBEATs
periodisch und retransmittet auf ACKNACK. Der Producer darf im
async-entkoppelten Pfad nie in den Kernel eintreten — ein Messwert soll die
Entkopplung gegen den inline-`sendto`-Pfad zeigen (Design-Motiv aus
`reliable-endpoint-1.0.md` §2).

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp` (381 Zeilen,
header-only C++17) — `Sender`/`Receiver`-State-Machine, Wire-Codec
(`write_frame`/`unframe`/`acknack_frame`/`heartbeat_frame`/`parse_acknack`/
`parse_heartbeat`), `AsyncWriter` (SPSC-Ring mit `std::atomic<size_t>
head_`/`tail_`, `std::thread`-Drain, `enqueue`/`finish`/`stop`). Pure C++17,
header-only; kein Linking gegen Rust-Layer, cross-compile-fest.
`endpoints/cpp/example_reliable.cpp` (In-Process-Demo: Sender + Receiver,
jedes 3. Sample im ersten Round gedroppt, ACKNACK-getriebene Recovery-Runden).
`endpoints/cpp/test/test_reliable_cpp.cpp` (12 Unit-Checks + Byte-Golden +
Latenz-Bench).

**Tests (codepit, g++12):**

- `cpp_reliable_unit_and_golden` — 12 Unit-Checks (Sender: monotone seq,
  payload-too-large, window-full, heartbeat first/silence/period; Receiver:
  acknack-clear, acknack-full-clear, in-order, reorder, duplicate-drop,
  buffer-full, pending-acknack-Bitmap, reset) + Byte-Golden
  (`acknack_frame(0x80, NONE, 1, 1, 0, 0, 0x80)` == `golden_acknack_le.bin`,
  `heartbeat_frame(0x80, NONE, 1, 1, 3, 0x80)` == `golden_heartbeat_le.bin`,
  je 13 Byte, byte-identisch zu den Rust-generierten Goldens) + Latenz-Bench
  (Median über K=20000 Iterationen: `AsyncWriter::enqueue`, No-op-Drain-Hooks,
  kein Syscall, vs. `write_frame` + inline `send` auf einen gebundenen
  Loopback-Sink) → `ALL OK`.
- `cpp_reliable_loss_recovery` — der Rust-Peer dropt jedes 3. Datagramm; App
  retransmittet auf ACKNACK; 12/12 Samples lückenlos in-order geliefert.
- `cpp_reliable_lossless_baseline` — lossless; 12/12.
- `cpp_reliable_example` — In-Process-Demo → `RELIABLE OK N=16`.

4/4 passed. Latenz gemessen: 31–40 ns (`enqueue`) vs. 3427 ns (inline
`sendto`), ~85–110× je nach Lauf (Lauf-zu-Lauf-Streuung, kein Einzelwert).

Fund während der Entwicklung: ein Drain-Deadlock ohne Responder — `finish()`
wartet, bis Fenster und Ring leer sind, was ohne ACKNACK-Antwort nie
eintritt. Gefixt durch `stop()` (unconditionaler Teardown, für
responder-lose Kontexte wie den Latenz-Bench).

**Status:** done — Sender- und Receiver-State-Machine unit-verifiziert;
das live-E2E gegen den Rust-Peer deckt die Sender-Rolle ab (App sendet,
Rust-Peer empfängt + ACKNACKt); die Receiver-Rolle ist in-process
(`example_reliable.cpp`) verifiziert, nicht live-network gegen einen
externen Sender; Latenz-Entkopplung gemessen.

## §5 Test- und Beleg-Pflicht

**Spec:** `zerodds-endpoint-cpp-1.0.md` §5 (spiegelt `reliable-endpoint-1.0.md`
§5) -- Unit, Byte-Golden, E2E-Loss-Recovery, Latenz-Bench, ein lauffähiges
`example_reliable_*`; kein false-green, lauter Skip nur bei fehlender
Toolchain.

**Repo:** Alle fünf Pflicht-Artefakte vorhanden, belegt in §4:
`test_reliable_cpp.cpp` (Unit + Byte-Golden + Latenz-Bench),
`crates/endpoint-e2e/tests/cpp_reliable.rs` (Loss-Recovery +
Lossless-Baseline gegen den Rust-Peer), `example_reliable.cpp` (lauffähiges
Example, kein Stub). Lauter Skip nur bei fehlendem `g++`/`gcc` auf dem
`PATH`.

**Tests:** siehe §4 (`cpp_reliable_unit_and_golden`,
`cpp_reliable_loss_recovery`, `cpp_reliable_lossless_baseline`,
`cpp_reliable_example`) sowie §2/§3 (`cpp_raw_udp`, `cpp_endpoint_sync`,
`cpp_endpoint_async`).

**Status:** done.

## Audit-Status

5 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-run (codepit, g++12, verifiziert): `cargo test -p zerodds-endpoint-e2e
--test cpp` → 3/3 (`cpp_raw_udp`, `cpp_endpoint_sync`, `cpp_endpoint_async`);
`--test cpp_reliable` → 4/4 (`cpp_reliable_unit_and_golden` inkl.
12-Checks+Byte-Golden+Bench, `cpp_reliable_loss_recovery`,
`cpp_reliable_lossless_baseline`, `cpp_reliable_example`); Latenz-Bench
enqueue 31–40 ns / inline sendto 3427 ns.

Offene Punkte: keine.
