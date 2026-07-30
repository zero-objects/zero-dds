# `reliable-endpoint` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/reliable-endpoint-1.0.md` -- Reliable Delivery als
Endpoint-Fähigkeit (ZeroDDS Vendor-Spec, baut auf DDS-XRCE 1.0 §8.4.10/§8.4.11 auf).

Implementation:

- `crates/xrce/src/reliable.rs` -- Referenz-State-Machine (`ReliableStreamState`).
- `crates/xrce/src/submessages/{acknack,heartbeat,write_data}.rs` -- Wire-Codec.
- `endpoints/{ada,c,cpp,d,go,java,csharp,python,elixir,julia,nim,rust,swift,zig}/` --
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

**Tests:** --

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

**Tests:** `endpoints/c/test/bench_reliable_async.c` (Latenz-Bench inline vs.
async-entkoppelt); je Sprache eigener Bench/Test, siehe §5.2.

**Status:** done

## §3 Kanonischer State-Machine-Kontrakt

### §3.1 Konstanten

**Spec:** §3.1, Tabelle -- `HEARTBEAT_PERIOD` 500 ms, `SENDER_WINDOW` 16,
`RECEIVER_BUFFER` 64, `MAX_PAYLOAD` 65535, reliable stream id Bit 7 gesetzt.

**Repo:** `crates/xrce/src/reliable.rs`: `DEFAULT_HEARTBEAT_PERIOD` (500 ms),
`SENDER_WINDOW_CAP` (16), `RECEIVER_BUFFER_CAP` (64), `RELIABLE_MAX_PAYLOAD` (65535).

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` (Teil der 17 Tests, u. a.
Window-Full- und Buffer-Full-Assertions gegen exakt diese Konstanten).

**Status:** done

### §3.2 Sender-Kontrakt

**Spec:** §3.2 -- `submit(payload) -> seq` (Fehler `PayloadTooLarge`/`WindowFull`),
`pending_heartbeat(now) -> HEARTBEAT?`, `recv_acknack(payload)` (RFC-1982-Fenster-Logik),
`get_in_flight(seq)`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{submit, pending_heartbeat,
recv_acknack, get_in_flight}` -- Signaturen und Fehlerpfade identisch zur Spec-Notation.

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` deckt monotone `seq`,
`WindowFull`, `PayloadTooLarge`, Heartbeat first/silence/empty, ACKNACK-Clear ab.

**Status:** done

### §3.3 Receiver-Kontrakt

**Spec:** §3.3 -- `recv_data(seq, payload)` (Duplikat-Drop, `BufferFull`),
`drain_in_order() -> [(seq, payload)]`, `pending_acknack(hint_last_seen) -> ACKNACK`,
`reset()`.

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState::{recv_data,
drain_in_order, pending_acknack, reset}`.

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` deckt Reorder, Duplicate-Drop,
`BufferFull`, pending-acknack-Bitmap, `reset()` ab.

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

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` (Wire-Roundtrip); `endpoints/c`
`test_reliable` gegen `golden_heartbeat_le.bin` + `golden_acknack_le.bin` --
HEARTBEAT geparst, ACKNACK byte-identisch, ALL OK.

**Status:** done (Rust-Referenz + C-Golden byte-identisch); pro-Sprache-Golden-Assertion
in §5.2.

## §5 Test- & Beleg-Pflicht (pro Sprache)

### §5.1 Referenz-State-Machine erfüllt die 5-Punkt-Matrix

**Spec:** §5 -- Unit (monotone seq/window-full/heartbeat/acknack-clear/reorder/
duplicate-drop/buffer-full/pending-acknack/reset), Byte-golden, E2E-Loss-Recovery,
Latenz-Bench, Example -- kein false-green, lauter Skip nur bei fehlender Toolchain.

**Repo:** `crates/xrce/src/reliable.rs` (Unit), `crates/xrce/src/submessages/{acknack,
heartbeat}.rs` (Byte-golden), `crates/endpoint-e2e/tests/*_reliable.rs` (Loss-Recovery
gegen den Rust-Referenz-Peer), `endpoints/c/test/bench_reliable_async.c` (Latenz),
`endpoints/*/example_reliable.*` (Example je Sprache).

**Tests:** `cargo test -p zerodds-xrce --lib reliable::` → 17/17 passed;
`endpoints/c` `test_reliable` → ALL OK.

**Status:** done

### §5.2 Pro-Sprache-Rollout (Coverage-Evidenz)

Die 5-Punkt-Matrix aus §5.1 gilt laut Spec §5 für "jede Sprache". Jede Sprache ist ein
eigenes Item.

#### §5.2 ada

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ada.

**Repo:** `endpoints/ada/src/reliable.{ads,adb}` (protected object + drain task, Leitfall).

**Tests:** `crates/endpoint-e2e/tests/ada_reliable.rs`.

**Status:** done -- codepit-verifiziert (Unit+Golden ALL OK, Loss-Recovery 12/12, Latenz
87 ns vs. 7338 ns inline, ~84×).

#### §5.2 c

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache c.

**Repo:** `endpoints/c/src/zerodds_reliable_async.c`, `endpoints/c/test/test_reliable.c`.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs`; `endpoints/c` `test_reliable` gegen
die Goldens (§4).

**Status:** done -- codepit-verifiziert.

#### §5.2 cpp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache cpp.

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp`,
`endpoints/cpp/test/test_reliable_cpp.cpp`.

**Tests:** `crates/endpoint-e2e/tests/cpp_reliable.rs`.

**Status:** done -- codepit-verifiziert.

#### §5.2 d

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache d.

**Repo:** `endpoints/d/reliable.d`, `endpoints/d/reliable_test.d`,
`endpoints/d/reliable_bench.d`.

**Tests:** `crates/endpoint-e2e/tests/d_reliable.rs`.

**Status:** done -- codepit-verifiziert.

#### §5.2 nim

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache nim.

**Repo:** `endpoints/nim/reliable.nim`, `endpoints/nim/reliable_test.nim`.

**Tests:** `crates/endpoint-e2e/tests/nim_reliable.rs`.

**Status:** done -- codepit-verifiziert.

#### §5.2 rust

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache rust.

**Repo:** `endpoints/rust/src/reliable.rs`.

**Tests:** `crates/endpoint-e2e/tests/rust_reliable.rs`.

**Status:** done -- codepit-verifiziert.

#### §5.2 zig

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache zig.

**Repo:** `endpoints/zig/src/reliable.zig`.

**Tests:** `crates/endpoint-e2e/tests/zig_reliable.rs`.

**Status:** done -- codepit-verifiziert.

#### §5.2 go

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache go.

**Repo:** `endpoints/go/reliable.go`, `endpoints/go/reliable_test.go`,
`endpoints/go/reliable_bench`.

**Tests:** `crates/endpoint-e2e/tests/go_reliable.rs`.

**Status:** done -- codepit-verifiziert (5/5, seriell --test-threads=1).

#### §5.2 java

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache java.

**Repo:** `endpoints/java/org/zerodds/endpoint/{ReliableWire,ReliableSender,
ReliableReceiver,AsyncReliableWriter}.java`.

**Tests:** `crates/endpoint-e2e/tests/java_reliable.rs`.

**Status:** done -- codepit-verifiziert (4/4, seriell --test-threads=1).

#### §5.2 csharp

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache csharp.

**Repo:** `endpoints/csharp/Reliable.cs`, `endpoints/csharp/ReliableTests.cs`.

**Tests:** `crates/endpoint-e2e/tests/csharp_reliable.rs`.

**Status:** done -- codepit-verifiziert (5/5, seriell --test-threads=1).

#### §5.2 python

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache python.

**Repo:** `endpoints/python/zerodds_reliable.py`, `endpoints/python/reliable_test.py`.

**Tests:** `crates/endpoint-e2e/tests/python_reliable.rs`.

**Status:** done -- codepit-verifiziert (4/4, seriell --test-threads=1); GIL-Runtime --
Async-Entkopplung schwächer als bei OS-Thread-Drain.

#### §5.2 elixir

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache elixir.

**Repo:** `endpoints/elixir/lib/reliable.ex`, `endpoints/elixir/reliable_test.exs`.

**Tests:** `crates/endpoint-e2e/tests/elixir_reliable.rs`.

**Status:** done -- codepit-verifiziert (5/5, seriell --test-threads=1).

#### §5.2 julia

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache julia.

**Repo:** `endpoints/julia/reliable.jl`, `endpoints/julia/reliable_test.jl`.

**Tests:** `crates/endpoint-e2e/tests/julia_reliable.rs`.

**Status:** done -- codepit-verifiziert (4/4, seriell --test-threads=1); Coroutine-
Task-Drain -- Async-Entkopplung schwächer als bei OS-Thread-Drain.

#### §5.2 swift

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache swift.

**Repo:** `endpoints/swift/Reliable.swift`, `endpoints/swift/reliable_tests.swift`.

**Tests:** `crates/endpoint-e2e/tests/swift_reliable.rs`.

**Status:** done -- lokal verifiziert (macOS, 5/5); kein codepit-swiftc vorhanden, daher
nicht codepit-, sondern lokal-verifiziert.

#### §5.2 ocaml

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache ocaml.

**Repo:** `endpoints/ocaml/reliable.ml` (eigenständiges Modul, Mutex/Condition-Mailbox +
Drain-Thread; kein `zerodds.ml`-Dependency), `endpoints/ocaml/reliable_test.ml`,
`reliable_bench.ml`.

**Tests:** `crates/endpoint-e2e/tests/ocaml_reliable.rs`.

**Status:** done -- codepit-verifiziert (5/5, seriell --test-threads=1).

#### §5.2 lua

**Spec:** §5 -- 5-Punkt-Matrix für die Sprache lua.

**Repo:** `endpoints/lua/reliable.lua` (eigenständiges Modul, byte-identischer
Frame-Codec zu `crates/xrce`), `endpoints/lua/reliable_test.lua`.

**Tests:** `crates/endpoint-e2e/tests/lua_reliable.rs`.

**Status:** done -- codepit-verifiziert (5/5, seriell --test-threads=1).

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

23 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-xrce --lib reliable::` → 17/17 passed (State-Machine,
§3+§4); `endpoints/c` `test_reliable` gegen `golden_heartbeat_le.bin` +
`golden_acknack_le.bin` → HEARTBEAT parsed, ACKNACK byte-identisch, ALL OK. Wave-1
(ada, c, cpp, d, nim, rust, zig) + Wave-2 (go 5/5, java 4/4, csharp 5/5, python 4/4,
ocaml 5/5, elixir 5/5, lua 5/5, julia 4/4) auf codepit grün, jeweils seriell
(`--test-threads=1`); swift 5/5 nur lokal auf macOS (kein codepit-swiftc verfügbar).

Offene Punkte: keine mehr auf Sprach-Rollout-Ebene -- alle 16 Sprachen aus §5.2 sind
`done`. Verbleibende Einschränkung: swift ist lokal (macOS), nicht codepit-verifiziert,
mangels swiftc-Toolchain auf codepit.
