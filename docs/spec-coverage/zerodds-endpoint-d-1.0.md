# `zerodds-endpoint-d` 1.0 — Spec-Coverage

**Quelle:** [`docs/specs/zerodds-endpoint-d-1.0.md`](../specs/zerodds-endpoint-d-1.0.md)
— ZeroDDS D-Endpoint-SDK-Spec (§1 XRCE-Framing, §2 sync Client, §3 async
Reader/Writer, §4 reliable Stream via
[`reliable-endpoint-1.0`](../specs/reliable-endpoint-1.0.md)). Deckt das
D-Endpoint-SDK `endpoints/d/` ab: `zerodds.d` (XRCE-Framing, sync `Client`,
async `AsyncReader` via `std.concurrency`) und das selbstständige `reliable.d`
(State-Machine + Wire-Codec + lock-freier `SpscRing`), plus das Live-Ping-Pong-
E2E. Ergänzt die Codegen-Coverage `zerodds-xcdr2-d`.

**Implementation:** `endpoints/d/zerodds.d`, `endpoints/d/reliable.d`,
`endpoints/d/reliable_app.d`, `endpoints/d/example_reliable.d`,
`endpoints/d/reliable_test.d`, `endpoints/d/reliable_bench.d`;
E2E-Peer `crates/endpoint-e2e/tests/d.rs` + `d_reliable.rs`.

## §1 XRCE-Framing

**Spec:** `zerodds-endpoint-d-1.0` §1 — 8-Byte-WRITE_DATA-Header (session,
stream, seq LE, submsg id `0x07`, flags `0x03`, len LE) + Body; best-effort
Stream `0x01`.

**Repo:** `zerodds.d` — `writeFrame`/`readFrame` (Zeile 136–151),
Konstanten `SessionNoKey = 0x80`, `StreamBestEffort = 0x01` (Zeile 133–134).

**Tests:** indirekt über das Ping-Pong-E2E (§4) — `d_endpoint_sync` +
`d_endpoint_async` framen/deframen jedes Sample über diesen Codepfad.

**Status:** done.

## §2 Sync-Client

**Spec:** `zerodds-endpoint-d-1.0` §2 — Ein blockierend gepollter `Client`
über eine austauschbare `Transport` (deliver/receive-Delegates);
Sequenznummer wrapt modulo 0x10000.

**Repo:** `zerodds.d` — `class Client` (Zeile 162–182), `struct Transport`
(Zeile 155–158), `memTransport()` als In-Memory-FIFO für Tests/Beispiele.

**Tests (codepit):** `d_endpoint_sync` (Ping-Pong-E2E, §4).

**Status:** done.

## §3 Async Reader

**Spec:** `zerodds-endpoint-d-1.0` §3 — Ein Hintergrund-Actor via
`std.concurrency` (`spawn`/`send`/`receive`, Tid-Message-Passing) — die
idiomatische D-Concurrency-Primitive, analog zu Adas protected object/task
und Rusts Channel. Auf dieser Ebene existiert nur ein async **Reader**; ein
entkoppelter async **Writer** ist Teil des reliable Streams (`SpscRing`, §5)
— der Sync-`Client` sendet inline.

**Repo:** `zerodds.d` — `class AsyncReader` (Zeile 214–222), `readerLoop`
(Zeile 201–212).

**Tests (codepit):** `d_endpoint_async` (Ping-Pong-E2E, §4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** kein eigener Abschnitt in `zerodds-endpoint-d-1.0` — repo-interner
Live-Beweis für §1–§3 zusammen: drei Live-UDP-Tests gegen den geteilten
Rust-XRCE-Peer: generierter Codegen über nacktes UDP (kein XRCE-Frame),
Voll-Stack über den sync `Client` (§2), Voll-Stack über den async
`AsyncReader` (§3).

**Repo:** `crates/endpoint-e2e/tests/d.rs`.

**Tests (codepit):** `d_raw_udp` + `d_endpoint_sync` + `d_endpoint_async` —
3/3 passed.

**Status:** done.

## §5 Reliable Stream — State-Machine + async-entkoppelter Writer

**Spec:** `zerodds-endpoint-d-1.0` §4, referenziert `reliable-endpoint-1.0`
§3.1/§3.2/§3.3 — XRCE reliable Stream (`stream_id >= 128`, §8.4.10/§8.4.11).
Sender `submit`/`pendingHeartbeat`/`recvAcknack`/`getInFlight`; Receiver
`recvData`/`drainInOrder`/`pendingAcknack`/`reset`. Window 16, Receiver-Puffer
64, Heartbeat 500 ms, Payload ≤ 65535, RFC-1982-16-Bit-Sequenznummern.
Selbstständiger Wire-Codec (kein `Endian`/`Writer`-Namenskonflikt mit
`zerodds.d`). Dazu `SpscRing` — ein wait-freier Single-Producer/
Single-Consumer-Ring (`CAP = 1024`) als async-entkoppelter Writer: der
Producer macht nur einen Slot-Store + einen Release-Store auf `head`, kein
Lock, kein Syscall; ein separater Drain-Thread besitzt Socket + Sender-State.

**Repo:** `endpoints/d/reliable.d` (Sender/Receiver/`SpscRing`/Wire-Codec),
`endpoints/d/reliable_app.d` (E2E-Sender-App: initialer Burst + Heartbeat/
ACKNACK-getriebene Retransmit-Loop), `endpoints/d/example_reliable.d`
(In-Prozess-Demo), `endpoints/d/reliable_test.d` (Unit- + Byte-Golden-Suite).

**Tests (codepit):**
- `d_reliable_unit_and_golden` — `reliable_test.d` prüft monotone Seq,
  Payload-zu-groß, Window-full, Heartbeat first/silenced-&lt;500ms/
  after-500ms/empty, ACKNACK clear/full-clear, in-order Drain, Reorder,
  Duplicate-Drop, Buffer-full, Pending-ACKNACK-Bitmap, Reset, plus 2
  hardcodierte Byte-Goldens (HEARTBEAT/ACKNACK) — und, wenn die Rust-Goldens
  generiert wurden, zusätzlich byte-identisch gegen
  `golden_heartbeat_le.bin`/`golden_acknack_le.bin`. Ausgabe „ALL OK".
- `d_reliable_loss_recovery` — Peer dropt jedes 3. Datagramm (12 Samples);
  `reliable_app.d` retransmittet auf ACKNACK; alle 12 lückenlos in-order
  geliefert.
- `d_reliable_no_loss` — dieselbe App, lossless Baseline, 12/12.
- `d_reliable_example` — `example_reliable.d`, In-Prozess-Sender/Receiver-
  Loss-Recovery-Demo, N=12, Ausgabe „RELIABLE OK".

4/4 passed (`d_reliable_latency_bench` separat in §6; alle 5 Tests aus
`d_reliable.rs` zusammen 5/5, siehe Audit-Status).

**Status:** done.

## §6 Latenz — Producer-Enqueue vs. inline sendto

**Spec:** `reliable-endpoint-1.0` §5 Punkt 4 (Latenz-Bench), referenziert von
`zerodds-endpoint-d-1.0` §4 — Micro-Bench vergleicht ein inline UDP-`sendto`
pro Sample gegen den wait-freien `SpscRing`-Enqueue — der Syscall, den der
entkoppelte Writer aus dem Hot-Path des Producers entfernt.

**Repo:** `endpoints/d/reliable_bench.d` — `ITERS = 20000`,
`MonoTime`-Zeitstempel um `sendto` bzw. `ring.enqueue`, sortiert, Median
genommen.

**Tests (codepit):** `d_reliable_latency_bench` — inline-sendto-Median
3600 ns; ring-enqueue-Median 0 ns.

**HONEST NOTE:** Der Ring-Enqueue liegt unterhalb der `MonoTime`-Auflösung
dieser Maschine — der Median liest sich als 0 ns als Bench-Granularitätslimit,
nicht als Behauptung „unendlich schnell". Messbar und real ist: die
3600-ns-Syscall wird durch die Entkopplung über den Ring aus dem
Producer-Pfad entfernt.

**Status:** done (Messung mit dem oben genannten Granularitätslimit; das
Limit selbst ist kein Defekt, sondern eine Eigenschaft der Bench-Auflösung).

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-run (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test d`
3/3 (`d_raw_udp`, `d_endpoint_sync`, `d_endpoint_async`); `--test d_reliable`
5/5 (`d_reliable_loss_recovery`, `d_reliable_no_loss`,
`d_reliable_unit_and_golden`, `d_reliable_example`,
`d_reliable_latency_bench`); Latenz-Bench inline-sendto 3600 ns /
ring-enqueue 0 ns (Bench-Granularitätslimit, siehe §6).

Offene Punkte: keine.
