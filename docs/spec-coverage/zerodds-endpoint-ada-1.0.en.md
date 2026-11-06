# `zerodds-endpoint-ada` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-ada-1.0.md` — ZeroDDS Ada endpoint SDK
spec (DDS-XRCE 1.0 §8.3/§8.4.10/§8.4.11 + `reliable-endpoint-1.0`); complements
the codegen coverage `zerodds-xcdr2-ada`.

Implementation:

- `endpoints/ada/` — the Ada endpoint SDK (Stage-1, ADR 0013: `Interfaces.C`
  bindings over the C89 wire core `zdw`, plus the idiomatic `Deep_Reading`
  layer): `src/zdw.ads` (wire-core FFI), `src/deep_reading.{ads,adb}` (codec,
  `Frame`/`Deframe`, `Mailbox`, `Reader_Task`), `src/reliable.{ads,adb}`
  (reliable state machine + async-decoupled writer),
  `test/{example_sync,example_async,example_reliable}.adb`.
- `crates/endpoint-e2e/tests/ada.rs` — the live ping-pong E2E against the shared
  Rust XRCE peer.

## §1 XRCE Framing

### §1 Endpoint SDK (wire-core binding)

**Spec:** §1 -- "`body` — the XCDR2-encoded sample, bound via
`endpoints/ada/src/zdw.ads` (package `Zdw`, `Interfaces.C` FFI over the C89 wire
core), byte-identical to the `zerodds-cdr` reference."

**Repo:** `endpoints/ada/src/zdw.ads` (thin FFI over the C89 wire core,
`Put_*`/`Get_*`), `deep_reading.{ads,adb}` (codec, `Frame`/`Deframe`, `Mailbox`
protected object, `Reader_Task`).

**Tests:** `endpoints/ada/test/test_byte_identity.adb::Test_Byte_Identity` — the marks `"negative: a body past Max_Frame refused"`,
`"negative: the largest body still frames"` and `"ALL OK"` against
`golden_le.bin`/`golden_be.bin`; driven by
`crates/endpoint-e2e/tests/ada.rs::xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong`.

**Status:** done

### §1 WRITE_DATA framing (8-byte header + body)

**Spec:** §1 -- "A WRITE_DATA sample is framed as an 8-byte header + body:
`[session][stream][seq_lo][seq_hi][0x07][flags][len_lo][len_hi][body...]`.
`session=0x80`, `stream=0x01` (best-effort) on the sync/async path,
`stream_id >= 128` on the reliable path. `Deep_Reading.Frame`/`Deframe` MUST
build resp. parse this frame for the best-effort stream;
`Reliable.Write_Frame` for the reliable stream."

**Repo:** `Deep_Reading.Frame` / `Deframe` (best-effort stream `0x01`);
`Reliable.Write_Frame` (reliable stream `0x80`) in `endpoints/ada/src/reliable.adb`.

**Tests:** `endpoints/ada/test/test_udp_loopback.adb` — mark
`"UDP loopback round-trip ok"`;
`endpoints/ada/test/test_byte_identity.adb::Test_Byte_Identity` for the frame octets; the ping-pong and reliable E2E from §2/§3/§4.

**Status:** done

## §2 sync Client

### §2 `Deep_Reading.Mailbox` + `example_sync.adb`

**Spec:** §2 -- "A blocking receive path over the idiomatic Ada concurrency
primitive: the protected object `Deep_Reading.Mailbox`. MUST:
`Mailbox.Deliver(Frame)`; `Mailbox.Receive(Frame)` / `Mailbox.Try_Receive(Frame,
Success)` — the integrator owns the poll/wait loop."

**Repo:** `Deep_Reading.Mailbox` (protected FIFO transport, `Deliver`/`Receive`);
example `example_sync.adb`.

**Tests:** (Linux) `xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/ada.rs`, Ada
app built via gprbuild over the C89 wire core) — together with
`xrce_8_3_5_8_ada_async_app_writes_the_ping_and_reads_the_pong` (§3) 2/2 passed.

**Status:** done

## §3 async Reader/Writer

### §3 `Reader_Task` + `example_async.adb`

**Spec:** §3 -- "A background reader as its own Ada task (`Reader_Task`) — no
async runtime beyond `task`/`protected object`. MUST: `Reader_Task` drains the
transport, decodes every WRITE_DATA body, deposits it via `Mailbox.Deliver`; the
calling task blocks on `Mailbox.Receive`."

**Repo:** `Reader_Task` (background drain, deposits decoded bodies via
`Mailbox.Deliver`); example `example_async.adb` (main task blocks on
`Inbox.Receive`).

**Tests:** (Linux) `xrce_8_3_5_8_ada_async_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/ada.rs`,
Ada app built via gprbuild over the C89 wire core) — together with
`xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong` (§2) 2/2 passed.

**Status:** done

## §4 reliable Stream

### §4 State machine (sender + receiver)

**Spec:** §4 -- "Ada implements the canonical contract (`reliable-endpoint-1.0`
§3) in `endpoints/ada/src/reliable.{ads,adb}`: sender —
`Submit`/`Pending_Heartbeat`/`Recv_Acknack`/`Get_In_Flight`; receiver —
`Recv_Data`/`Drain_Next`/`Pending_Acknack`/`Reset`."

**Repo:** `endpoints/ada/src/reliable.{ads,adb}`.

**Tests:** (Linux) `test_reliable_unit` — monotonic seq, window-full, heartbeat
first/silence/empty, acknack-clear (+full-clear), reorder, duplicate-drop,
buffer-full, pending-acknack bitmap, reset → ALL OK.

**Status:** done

### §4 Wire (byte-golden HEARTBEAT/ACKNACK)

**Spec:** §4 -- "Wire codec — `Heartbeat_Frame`/`Acknack_Frame` + the
corresponding `Parse_*` functions; byte-identical to
`golden_heartbeat_le.bin`/`golden_acknack_le.bin`."

**Repo:** `Reliable.Heartbeat_Frame` / `Acknack_Frame` / `Parse_*`.

**Tests:** (Linux) `test_reliable_unit` asserts `Heartbeat_Frame(1,3)` ==
`golden_heartbeat_le.bin` and `Acknack_Frame(1,0)` == `golden_acknack_le.bin` →
byte-identical.

**Status:** done

### §4 Async-decoupled writer + loss recovery

**Spec:** §4 -- "`Reliable.Send_Ring` — a protected ring
(`Enqueue`/`Dequeue`/`Close`) as the async-decoupled writer: the producer only
ever performs a wait-free `Enqueue`, no syscall; a dedicated drain task
(`example_reliable.adb`, `GNAT.Sockets`) owns the socket and the sender state
and performs all the I/O (send, heartbeat, ACKNACK-driven retransmit)."

**Repo:** `Reliable.Send_Ring` (protected `Enqueue`/`Dequeue`/`Close`) +
`example_reliable.adb` (drain task via `GNAT.Sockets`).

**Tests:** (Linux)
- `reliable_5_3_ada_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd datagram; app retransmits
  on ACKNACK; all 12 samples delivered gap-free in order.
- `reliable_5_4_ada_the_producer_enqueue_costs_less_than_the_inline_send` — the
  bench mode of `example_reliable.adb` (`bench`) measures producer
  `Enqueue`→return against an inline frame+`sendto` and prints both figures; the
  test checks that the enqueue is the lower one (job `integration-bindings-gnat-beam`).

**Status:** done

---

## Audit-Status

7 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-ada-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-gnat-beam` (job `integration-bindings-gnat-beam`) — 3 tests green.
* `python3 ci/run_integration.py integration-endpoints` (job `integration-endpoints`) — 6 tests green.

9 cited tests green, 0 not green.

Open items: none.
