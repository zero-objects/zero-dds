# `zerodds-endpoint-nim` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-nim-1.0.md` — ZeroDDS Nim Endpoint-SDK-Spec
(XRCE-Framing, sync `Client`, async `AsyncReader`, Reliable Stream).

Implementation:

- `endpoints/nim/zerodds.nim` — `Writer`/`Reader` (XCDR2, Alignment cap 4),
  `writeFrame`/`readFrame` (XRCE best-effort Stream 0x01), `Client` (sync),
  `AsyncReader` (async).
- `endpoints/nim/reliable.nim` — Sender/Receiver-State-Machine + HEARTBEAT/ACKNACK-
  Frame-Codec (reliable Stream 0x80).
- `endpoints/nim/reliable_app.nim` — Channel-basierter Drain-Thread (Producer
  enqueued, Drain-Thread besitzt Socket + Sender-State).
- `endpoints/nim/example_reliable.nim` — In-Process-Demo der Loss-Recovery.
- `crates/endpoint-e2e/tests/nim.rs`, `crates/endpoint-e2e/tests/nim_reliable.rs` —
  Live-E2E gegen den Rust-XRCE-Peer.

## §1 XRCE-Framing

**Spec:** §1 — XRCE `WRITE_DATA`-Rahmen: 8-Byte-Header (Session, Stream, Seq LE,
Submessage-Id, Flags, Länge LE) + Payload, byte-identisch zu `crates/xrce` und
den übrigen Endpoint-SDKs.

**Repo:** `endpoints/nim/zerodds.nim::writeFrame`/`readFrame` (best-effort,
Session `0x80`, Stream `0x01`); `endpoints/nim/reliable.nim::reliableWriteFrame`
(reliable, Stream `0x80`) — identische Byte-Struktur, andere Stream-Id.

**Tests:** `endpoints/nim/test.nim` (Byte-Identität des Wire-Cores, CI-Job
`endpoints-nim`); Frame-Roundtrip live in §2/§3/§4.

**Status:** done.

## §2 Sync-Client

**Spec:** §2 — Ein blockierungsfreier Poll-Client: `write` framet+sendet, `poll`
liest non-blocking und liefert `Option[seq[byte]]`.

**Repo:** `endpoints/nim/zerodds.nim::Client`/`newClient`/`write`/`poll`
(monotone `seqNo`, wrap bei `0xffff`).

**Tests:** `nim_endpoint_sync` (`crates/endpoint-e2e/tests/nim.rs`) — Live-Poll-
Loop gegen den Rust-Peer: eine App aus generierten `idlc`-Typen (`gen.nim`) + dem
Endpoint-SDK tauscht über einen echten UDP-Socket ein typisiertes Sample mit dem
gemeinsamen Rust-XRCE-Peer aus (`crates/endpoint-e2e/tests/nim.rs::build_nim_endpoint`
kompiliert `gen.nim` + eine Kopie von `zerodds.nim` + `main.nim`, spawnt die App).

**Status:** done.

## §3 Async-Reader

**Spec:** §3 — Der idiomatische Nim-`async`/`await`-Pfad: `recv` liefert ein
`Future[seq[byte]]`, das erst mit dem entframten Sample resolved.

**Repo:** `endpoints/nim/zerodds.nim::AsyncReader`/`newAsyncReader`/`recv`
(`{.async.}`-Proc, pollt via `sleepAsync(1)` bis ein Frame ansteht).

**Tests:** `nim_endpoint_async` (`crates/endpoint-e2e/tests/nim.rs`) — `waitFor
fut.withTimeout(10000)` gegen dieselbe generierte App; `Client.write` sendet in
beiden Modi (das SDK hat nur einen sync Writer), der async-Pfad prüft allein das
`AsyncReader`-`Future`.

**Tests (codepit):** `nim_endpoint_sync`, `nim_endpoint_async` — beide grün.

**Status:** done.

## §4 Reliable Stream

**Spec:** §4 — XRCE Reliable Stream (`stream_id ≥ 128`, §8.4.10/§8.4.11),
gespiegelt aus der Referenz `crates/xrce/src/reliable.rs`: Sender `submit`/
`pendingHeartbeat`/`recvAcknack`/`getInFlight`; Receiver `recvData`/
`drainInOrder`/`pendingAcknack`/`reset`. Fenster 16, Receiver-Buffer 64,
Heartbeat 500 ms, Payload ≤ 65535, RFC-1982-16-Bit-Sequenznummern. Der
Async-Writer ist als Channel-Drain-Thread gebaut: der Producer enqueued
lock-basiert (Nim-stdlib `Channel`, kein wait-free Ring), ein dedizierter
Drain-Thread besitzt den Socket und den `Sender`-State (WRITE_DATA senden,
periodisches HEARTBEAT, ACKNACK-Empfang, Retransmit aus `inFlight`).

**Repo:** `endpoints/nim/reliable.nim` (State-Machine + Frame-Codec);
`endpoints/nim/reliable_app.nim::drain` (Drain-Thread über `gChan.tryRecv`);
`endpoints/nim/reliable_test.nim` (Unit + Byte-Golden); `endpoints/nim/example_reliable.nim`
(In-Process-Demo, kein Socket).

**Tests (codepit):**

- `nim_reliable_loss_recovery` (`crates/endpoint-e2e/tests/nim_reliable.rs`) —
  12 Samples, Peer droppt jedes 3. Datagramm; die App retransmittet auf ACKNACK;
  alle 12 lückenlos in Reihenfolge zugestellt.
- `nim_reliable_no_loss` — identischer Lauf ohne injizierten Loss (Baseline).
- `nim_reliable_unit_and_golden` — kompiliert und läuft `reliable_test.nim`;
  22 gespiegelte State-Machine-Checks (Sender: monotone Seq, Payload-zu-groß,
  Window-voll, Heartbeat first/silence/after-period/leer, ACKNACK-Clear
  partial/full; Receiver: in-order, Reorder-Block+Delivery, Duplikat-Drop,
  Buffer-voll, ACKNACK-Bitmap, Reset; plus 3 End-to-End-Loss-Recovery-Checks)
  + 4 Byte-Golden-Checks (`byte_golden_heartbeat`, `byte_golden_acknack` gegen
  `golden_heartbeat_le.bin`/`golden_acknack_le.bin`, plus Parse-Roundtrip beider
  Goldens) — `ALL OK`.
- `nim_reliable_producer_latency` — Bench-Modus von `reliable_app.nim`: Producer
  `enqueue`→Return via Channel vs. inline `sendto` im selben Thread (Messwert
  siehe §5).

**Status:** done.

**Ehrlich vermerkt:** Nims stdlib-`Channel` ist lock-basiert (Mutex+Copy), kein
wait-free SPSC-Ring; die Entkopplung vom Send-Syscall ist real, ein echter Ring
würde den Enqueue-Wert weiter senken (Kommentar in `reliable_app.nim`). Die
Referenz-Spec (§4) schreibt die konkrete Queue-Implementierung nicht vor.

## §5 Latenz

**Spec:** §4 — Ein Messwert, der die Producer-Entkopplung vom Socket-Syscall
zeigt (`reliable-endpoint` §5 Punkt 4, Latenz-Bench).

**Repo:** `endpoints/nim/reliable_app.nim::runBench` — 20000 Iterationen je
Pfad; `inline`: `sock.send` direkt im Producer-Thread; `decoupled`: `gChan.send`
in einen von einem Drain-Thread geleerten Channel.

**Tests (codepit):** `nim_reliable_producer_latency` — Enqueue **192 ns** vs.
inline `sendto` **3926 ns** (~20×).

**Status:** done (Messwert vorhanden); Weiterer Zuwachs via wait-free Ring bleibt
offen (siehe §4, ehrlich vermerkt — kein separater Tracking-Punkt, da kein
Spec-Erfordernis).

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a.

Test-Lauf (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test
nim` → `nim_endpoint_sync` + `nim_endpoint_async` 2/2 grün; `--test
nim_reliable` → `nim_reliable_loss_recovery`, `nim_reliable_no_loss`,
`nim_reliable_unit_and_golden`, `nim_reliable_producer_latency` 4/4 grün;
Latenz-Messwert 192 ns (enqueue) vs. 3926 ns (inline sendto), ~20×. Kein
GitLab-CI-Job führt `zerodds-endpoint-e2e` aus — die Nachweise sind
manuelle codepit-Läufe, nicht in CI verdrahtet (`endpoints-nim`-Job deckt nur
Wire-Core/Beispiele ab, kein reliable/E2E).

Offene Punkte: `zerodds-endpoint-e2e`-Tests (Ping-Pong + reliable) sind nicht
in die GitLab-CI verdrahtet — laufen nur manuell auf codepit. Ein echter
wait-free SPSC-Ring statt des lock-basierten `Channel` bleibt eine mögliche
weitere Latenzsenkung (kein offener Spec-Punkt, siehe §4).
