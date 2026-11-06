# `zerodds-endpoint-ada` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-ada-1.0.md` — ZeroDDS Ada-Endpoint-SDK-Spec
(DDS-XRCE 1.0 §8.3/§8.4.10/§8.4.11 + `reliable-endpoint-1.0`); ergänzt die
Codegen-Coverage `zerodds-xcdr2-ada`.

Implementation:

- `endpoints/ada/` — das Ada-Endpoint-SDK (Stage-1, ADR 0013: `Interfaces.C`-
  Bindings über den C89-Wire-Core `zdw`, plus die idiomatische `Deep_Reading`-
  Schicht): `src/zdw.ads` (Wire-Core-FFI), `src/deep_reading.{ads,adb}`
  (Codec, `Frame`/`Deframe`, `Mailbox`, `Reader_Task`), `src/reliable.{ads,adb}`
  (reliable State-Machine + async-entkoppelter Writer),
  `test/{example_sync,example_async,example_reliable}.adb`.
- `crates/endpoint-e2e/tests/ada.rs` — das Live-Ping-Pong-E2E gegen den geteilten
  Rust-XRCE-Peer.

## §1 XRCE-Framing

### §1 Endpoint-SDK (Wire-Core-Bindung)

**Spec:** §1 -- "`body` — das XCDR2-kodierte Sample, gebunden über
`endpoints/ada/src/zdw.ads` (Package `Zdw`, `Interfaces.C`-FFI über den
C89-Wire-Core), byte-identisch zur `zerodds-cdr`-Referenz."

**Repo:** `endpoints/ada/src/zdw.ads` (dünnes FFI über den C89-Wire-Core,
`Put_*`/`Get_*`), `deep_reading.{ads,adb}` (Codec, `Frame`/`Deframe`, `Mailbox`
protected object, `Reader_Task`).

**Tests:** `endpoints/ada/test/test_byte_identity.adb::Test_Byte_Identity` — die
Marken `"negative: a body past Max_Frame refused"`,
`"negative: the largest body still frames"` und `"ALL OK"` gegen
`golden_le.bin`/`golden_be.bin`; gefahren über
`crates/endpoint-e2e/tests/ada.rs::xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong`.

**Status:** done

### §1 WRITE_DATA-Framing (8-Byte-Header + Body)

**Spec:** §1 -- "Ein WRITE_DATA-Sample wird als 8-Byte-Header + Body gerahmt:
`[session][stream][seq_lo][seq_hi][0x07][flags][len_lo][len_hi][body...]`.
`session=0x80`, `stream=0x01` (best-effort) auf dem Sync/Async-Pfad,
`stream_id ≥ 128` auf dem reliable Pfad. `Deep_Reading.Frame`/`Deframe` MÜSSEN
diesen Rahmen für den best-effort Stream bauen bzw. parsen; `Reliable.Write_Frame`
für den reliable Stream."

**Repo:** `Deep_Reading.Frame` / `Deframe` (best-effort Stream `0x01`);
`Reliable.Write_Frame` (reliable Stream `0x80`) in `endpoints/ada/src/reliable.adb`.

**Tests:** `endpoints/ada/test/test_udp_loopback.adb` — Marke
`"UDP loopback round-trip ok"`;
`endpoints/ada/test/test_byte_identity.adb::Test_Byte_Identity` für die
Rahmen-Oktette; das Ping-Pong- und reliable-E2E aus §2/§3/§4.

**Status:** done

## §2 sync Client

### §2 `Deep_Reading.Mailbox` + `example_sync.adb`

**Spec:** §2 -- "Ein blockierender Empfangspfad über das idiomatische
Ada-Concurrency-Primitiv: das protected object `Deep_Reading.Mailbox`. MUSS:
`Mailbox.Deliver(Frame)`; `Mailbox.Receive(Frame)` / `Mailbox.Try_Receive(Frame,
Success)` — der Integrator besitzt den Poll-/Wait-Loop."

**Repo:** `Deep_Reading.Mailbox` (protected FIFO-Transport, `Deliver`/`Receive`);
Beispiel `example_sync.adb`.

**Tests:** (Linux) `xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/ada.rs`,
Ada-App via gprbuild über den C89-Wire-Core gebaut) — gemeinsam mit
`xrce_8_3_5_8_ada_async_app_writes_the_ping_and_reads_the_pong` (§3) 2/2 passed.

**Status:** done

## §3 async Reader/Writer

### §3 `Reader_Task` + `example_async.adb`

**Spec:** §3 -- "Ein Hintergrund-Reader als eigener Ada-Task (`Reader_Task`) —
kein Async-Runtime jenseits von `task`/`protected object`. MUSS: `Reader_Task`
drained den Transport, dekodiert jeden WRITE_DATA-Body, legt ihn per
`Mailbox.Deliver` ab; der aufrufende Task blockiert auf `Mailbox.Receive`."

**Repo:** `Reader_Task` (Hintergrund-Drain, legt dekodierte Bodies per
`Mailbox.Deliver` ab); Beispiel `example_async.adb` (Haupt-Task blockiert auf
`Inbox.Receive`).

**Tests:** (Linux) `xrce_8_3_5_8_ada_async_app_writes_the_ping_and_reads_the_pong` (`crates/endpoint-e2e/tests/ada.rs`,
Ada-App via gprbuild über den C89-Wire-Core gebaut) — gemeinsam mit
`xrce_8_3_5_8_ada_sync_app_writes_the_ping_and_reads_the_pong` (§2) 2/2 passed.

**Status:** done

## §4 reliable Stream

### §4 State-Machine (Sender + Receiver)

**Spec:** §4 -- "Ada implementiert den kanonischen Vertrag
(`reliable-endpoint-1.0` §3) in `endpoints/ada/src/reliable.{ads,adb}`: Sender —
`Submit`/`Pending_Heartbeat`/`Recv_Acknack`/`Get_In_Flight`; Receiver —
`Recv_Data`/`Drain_Next`/`Pending_Acknack`/`Reset`."

**Repo:** `endpoints/ada/src/reliable.{ads,adb}`.

**Tests:** (Linux) `test_reliable_unit` — monotone seq, window-full, heartbeat
first/silence/empty, acknack-clear (+full-clear), reorder, duplicate-drop,
buffer-full, pending-acknack-Bitmap, reset → ALL OK.

**Status:** done

### §4 Wire (byte-golden HEARTBEAT/ACKNACK)

**Spec:** §4 -- "Wire-Codec — `Heartbeat_Frame`/`Acknack_Frame` + die zugehörigen
`Parse_*`-Funktionen; byte-identisch zu `golden_heartbeat_le.bin`/
`golden_acknack_le.bin`."

**Repo:** `Reliable.Heartbeat_Frame` / `Acknack_Frame` / `Parse_*`.

**Tests:** (Linux) `test_reliable_unit` assertet `Heartbeat_Frame(1,3)` ==
`golden_heartbeat_le.bin` und `Acknack_Frame(1,0)` == `golden_acknack_le.bin` →
byte-identisch.

**Status:** done

### §4 Async-entkoppelter Writer + Loss-Recovery

**Spec:** §4 -- "`Reliable.Send_Ring` — ein protected Ring
(`Enqueue`/`Dequeue`/`Close`) als async-entkoppelter Writer: der Producer macht
nur ein wait-freies `Enqueue`, kein Syscall; ein dedizierter Drain-Task
(`example_reliable.adb`, `GNAT.Sockets`) besitzt Socket und Sender-State und
macht die gesamte I/O (Send, Heartbeat, ACKNACK-getriebenes Retransmit)."

**Repo:** `Reliable.Send_Ring` (protected `Enqueue`/`Dequeue`/`Close`) +
`example_reliable.adb` (Drain-Task via `GNAT.Sockets`).

**Tests:** (Linux)
- `reliable_5_3_ada_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Datagramm; App retransmittet
  auf ACKNACK; alle 12 Samples lückenlos in-order geliefert.
- `reliable_5_4_ada_the_producer_enqueue_costs_less_than_the_inline_send` — der
  Bench-Modus von `example_reliable.adb` (`bench`) misst Producer
  `Enqueue`→return gegen inline Frame+`sendto` und druckt beide Werte; der Test
  prüft, dass das Enqueue darunter liegt (Job `integration-bindings-gnat-beam`).

**Status:** done

---

## Audit-Status

7 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-ada-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-gnat-beam` (Job `integration-bindings-gnat-beam`) — 3 Tests grün.
* `python3 ci/run_integration.py integration-endpoints` (Job `integration-endpoints`) — 6 Tests grün.

9 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
