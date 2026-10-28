# `zerodds-endpoint-java` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-java-1.0.md` — the ZeroDDS Java
endpoint SDK spec. Reliable-stream contract details in
`docs/spec-coverage/reliable-endpoint-1.0.en.md`. This document covers transport; marshalling is
produced by the shared code generator (`crates/lang-rules`, the
`Language::Java` column) rather than by a Java-specific generator. The former
codegen coverage `zerodds-xcdr2-java` went with `crates/idl-java`
(commit `d045e43ea86`).

Implementation:

- `endpoints/java/ZdwEndpoint.java` (default package) — XRCE framing
  (`xrceWriteFrame`/`xrceReadBody`), serial framing after Annex C
  (`serialFrame`/`serialDeframe`/`fcs16`).
- `endpoints/java/Zdw.java` (default package) — wire core (`Writer`/`Reader`,
  XCDR2).
- `endpoints/java/org/zerodds/endpoint/ReliableWire.java` — reliable wire
  codec (HEARTBEAT/ACKNACK/WRITE_DATA) + RFC-1982 comparisons.
- `endpoints/java/org/zerodds/endpoint/ReliableSender.java` /
  `ReliableReceiver.java` — reliable sender/receiver state machine.
- `endpoints/java/org/zerodds/endpoint/AsyncReliableWriter.java` —
  async-decoupled reliable writer (`BlockingQueue` + drain `Thread`).
- `crates/endpoint-e2e/tests/java.rs` — ping-pong E2E; `crates/endpoint-e2e/tests/java_reliable.rs` —
  reliable-stream E2E + unit/golden + latency bench.
- `endpoints/java/EndpointTest.java` — byte-golden for WRITE_DATA/serial/DATA/heartbeat-read/ACKNACK
  (run manually against `cargo run -p zerodds-endpoint-golden`, not wired into `cargo test`).

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`; DATA receive path (id `0x09`) through the same unwrap;
serial framing after Annex C.1 (addresses, length, FCS-16 of RFC 1662).

**Repo:** `endpoints/java/ZdwEndpoint.java` — `xrceWriteFrame`/`xrceReadBody`,
constants `SESSION_NOKEY` (`0x80`)/`STREAM_BEST_EFFORT` (`0x01`),
`serialFrame`/`serialDeframe`/`fcs16`, `heartbeatRead`,
`acknackFrame`.

**Tests:** `crates/endpoint-e2e/tests/java.rs::java_endpoint_golden` runs
`endpoints/java/EndpointTest.java` against real Rust goldens generated with
`zerodds-endpoint-golden`. This is the Java counterpart to
`endpoints/c/Makefile`'s `make test GOLDEN_DIR=…`: the test generates the five
golden files (`golden_xrce_le.bin`, `golden_serial_le.bin`,
`golden_data_le.bin`, `golden_heartbeat_le.bin`, `golden_acknack_le.bin`),
compiles `EndpointTest` together with `Zdw` and `ZdwEndpoint`, and asserts on
`ALL OK` and the exit status. Unlike `ReliableSelfTest`, `EndpointTest` carries
no hardcoded fallback goldens, so a failure to generate them fails the test
rather than quietly skipping the comparison.

Run on the Linux test host (`tools/endpoints-gate`, at `2fa0800f55a`):

```
XRCE WRITE_DATA byte-identical (52 bytes)
serial byte-identical
DATA receive: body ok
serial deframe+crc round-trip ok
wrong frameCheck refused
0x7E inside the frame refused
HEARTBEAT parsed: first=1 last=3
ACKNACK byte-identical
appended submessage bounded out
over-long length rejected
truncated header rejected
writer >0xFFFF refused
writer maximal body framed
ALL OK
```

The framing is also exercised live via
`java_endpoint_sync`/`java_endpoint_async` (§4).

**Status:** done

## §2 Sync send/recv

**Spec:** §2 — transport-opaque, no `Client` object: `ZdwEndpoint` is
stateless; the integrator owns the run loop and calls
`xrceWriteFrame`/`xrceReadBody` directly against its own transport (a socket
or an in-memory queue).

**Repo:** `endpoints/java/ExampleSync.java` — in-memory `ArrayDeque` poll
loop, full field decode (`Reading{id,value,label}`); `sync` mode in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA_TAIL` — blocking
`DatagramSocket.receive()` against the real Rust peer.

**Tests:** `java_endpoint_sync` (§4).

**Status:** done.

## §3 Async Reader/Writer

**Spec:** §3 — a reader `Thread` drains the transport into a
`BlockingQueue`; the consumer blocks on `take()`. No dedicated
`AsyncReader`/`AsyncWriter` class for the plain path (unlike
`endpoints/go`) — the integrator composes it from the stateless
`ZdwEndpoint` framing methods.

**Repo:** `endpoints/java/ExampleAsync.java` — reader `Thread` +
`LinkedBlockingQueue<byte[]>` inbox, full field decode; `async` mode in
`crates/endpoint-e2e/tests/java.rs::MAIN_JAVA_TAIL` — a reader `Thread` reads
from the real `DatagramSocket`, the consumer blocks on `take()`.

**Tests:** `java_endpoint_async` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — a Java app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: the generated `Ping`/`Pong` codec + the
endpoint SDK (`ZdwEndpoint`), sync and async.

**Repo:** `crates/endpoint-e2e/tests/java.rs` — `MAIN_JAVA_HEAD` (imports and
class header) and `MAIN_JAVA_TAIL` (the run loop, mode `sync`/`async` via CLI
argument) wrap the members of the generated `ZdCodec`; `split_generated` lifts
them out of that class, because the generator emits them `private static` and
nothing outside the class could otherwise call them. Compiled against
`runtime/java` (`org.zerodds.wire.*`) — the former runtime
`crates/java-omgdds` + `crates/idl-java/runtime` went with the IDL transplant.

**Tests:** (this environment, OpenJDK 21, `cargo test -p zerodds-endpoint-e2e --test java`)
- `java_endpoint_sync` — full stack via blocking `DatagramSocket.receive()`.
- `java_endpoint_async` — full stack via reader `Thread`/`LinkedBlockingQueue`.
- `java_endpoint_golden` — `EndpointTest.java` against the Rust goldens (§1).

3 passed, 0 failed (verified locally).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight`; `ReliableReceiver.recvData`/`drainInOrder`/
`pendingAcknack`/`reset`. Window 16, receiver buffer 64, heartbeat 500 ms,
payload ≤ 65535, RFC-1982 16-bit sequence numbers. Alongside it, the
async-decoupled `AsyncReliableWriter`: the producer enqueues wait-free onto
a buffered `BlockingQueue` (`submit`/`offer`), a dedicated drain `Thread`
holds the `ReliableSender` state and does all the I/O (send, heartbeat,
ACKNACK-driven retransmit) — the producer never enters the kernel.

**Repo:** `endpoints/java/org/zerodds/endpoint/ReliableWire.java` —
`writeFrame`, `heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAckNack`,
`seqLt`/`seqGt`; `ReliableSender`, `ReliableReceiver`; `AsyncReliableWriter`
(`ArrayBlockingQueue<byte[]>` capacity 4096, drain `Thread`
`zdw-reliable-drain`, `submit`/`offer`/`finish`/`delivered`); example app
`endpoints/java/ExampleReliable.java` (a real UDP peer, no in-process stub).

**Tests:** (this environment, `cargo test -p zerodds-endpoint-e2e --test java_reliable`)
- `java_reliable_unit_and_golden` — compiles and runs
  `endpoints/java/ReliableSelfTest.java`: 33 `check()` assertions across 17
  test scenarios (sender: monotonic seq, in-flight count,
  payload-too-large, window-full, heartbeat first/silence/after-500ms,
  no-heartbeat-when-empty, acknack partial/full clear; receiver: in-order
  drain, reorder, duplicate dropped, buffer-full, pending-acknack bitmap,
  reset; in-process end-to-end loss recovery; byte-golden for
  HEARTBEAT/ACKNACK — hardcoded **and**, when
  `cargo run -p zerodds-endpoint-golden` is available, additionally checked
  against the generated `golden_heartbeat_le.bin`/`golden_acknack_le.bin`).
  Byte-golden: `HeartbeatFrame(1,3)` ==
  `80 00 01 00 0b 01 05 00 01 00 03 00 80`, `AckNackFrame(1,0)` ==
  `80 00 01 00 0a 01 05 00 01 00 00 00 80` — identical to the reference
  goldens (same bytes as `endpoints/go`). Output: `ALL OK`.
- `java_reliable_loss_recovery` — the peer drops every 3rd sample once; the
  app (`ExampleReliable`, `AsyncReliableWriter`) retransmits on ACKNACK; all
  12 samples delivered gap-free in order.
- `java_reliable_no_loss_baseline` — lossless baseline; 12/12.

3/3 passed (verified locally; latency bench in §6).

**Status:** done.

## §6 Latency — `BlockingQueue` enqueue vs. inline `DatagramSocket.send`

**Spec:** §5.3 — the producer path of `AsyncReliableWriter` (`offer` →
`BlockingQueue` push) must be measurably below the inline
`DatagramSocket.send` syscall — the evidence that async write removes
syscall latency from the producer path, not that it waits for ACKNACK.

**Repo:** `endpoints/java/ReliableBench.java` — 20000 iterations of inline
`DatagramSocket.send` (a real, bound loopback destination socket, never
read) vs. 20000 iterations of `BlockingQueue.offer` (a drain `Thread` keeps
the queue empty, the way `AsyncReliableWriter`'s real drain thread would),
no live peer needed.

**Tests:** (this environment, `java_reliable_latency_bench`)

```
producer latency: queue-enqueue median = 84 ns, inline-send median = 5917 ns (70x)
```

Median over 20000 iterations per path, measured locally (macOS, not a
Linux run) — the order of magnitude matches `endpoints/go`'s
Linux-verified 20–25 ns / 4360 ns (~175–220×); the absolute value is
machine-/load-dependent, the ratio (enqueue well below the syscall) is the
evidence.

**Status:** done.

---

## Audit status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (this environment — macOS, OpenJDK 21, `javac`/`java` on PATH —
**not** Linux-verified): `cargo test -p zerodds-endpoint-e2e --test java`
→ 3 passed, 0 failed (`java_endpoint_sync`, `java_endpoint_async`,
`java_endpoint_golden`); `--test java_reliable` → 4 passed, 0 failed
(`java_reliable_loss_recovery`, `java_reliable_no_loss_baseline`,
`java_reliable_unit_and_golden` — 33 Java unit assertions across 17 scenarios
including byte goldens, `java_reliable_latency_bench` — queue enqueue 83 ns
against inline `send` 6542 ns, ~79x).

Open items: none. Decision records: none.
