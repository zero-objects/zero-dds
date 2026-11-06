# `zerodds-endpoint-lua` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-lua-1.0.md` — ZeroDDS Lua
Endpoint-SDK-Spec. Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Lua`; hier steht der Transport.

Implementation:

- `endpoints/lua/zerodds.lua` — XRCE-Framing (`writeFrame`/`readFrame`),
  sync `Client`, coroutine-basierter `asyncReader`.
- `endpoints/lua/reliable.lua` — reliable Sender/Receiver-State-Machine +
  HEARTBEAT/ACKNACK-Wire-Codec + kooperativer `AsyncWriter`.
- `endpoints/lua/reliable_app.lua` — UDP-Sender-App (Modus `run` gegen einen
  Peer, Modus `bench` für den Latenz-Test).
- `crates/endpoint-e2e/tests/lua.rs` — Ping-Pong-E2E;
  `crates/endpoint-e2e/tests/lua_reliable.rs` — reliable-Stream-E2E.
- Gate: `lua5.4` + `luasocket` (das `socket`-Modul) auf PATH; beides fehlend
  → lauter Skip, kein False-Green.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, Submsg-ID `0x07`
WRITE_DATA, flags `0x03`, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/lua/zerodds.lua` — `M.writeFrame`/`M.readFrame`,
Konstanten `M.SESSION_NOKEY` (`0x80`) und `M.STREAM_BEST_EFFORT` (`0x01`).

**Tests:** kein isolierter Framing-Unit-Test unabhängig vom vollen Stack;
das Framing wird über `xrce_8_3_5_8_lua_sync_app_writes_the_ping_and_reads_the_pong`/`xrce_8_3_5_8_lua_async_app_writes_the_ping_and_reads_the_pong` (§4) live
geübt (jedes Sample durchläuft `writeFrame`/`readFrame` auf dem Draht).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — gepollter, nicht-blockierender `Client`; `Write` framet +
liefert synchron, `Poll` ist ein einzelner nicht-blockierender
Empfangsversuch; kein eingebautes `Receive(timeout)` — Deadline-Looping ist
Aufrufer-Sache.

**Repo:** `endpoints/lua/zerodds.lua::Client` (`Client.new`/`Client:write`/
`Client:poll`, `session`/`stream`-Defaults `SESSION_NOKEY`/
`STREAM_BEST_EFFORT`, monoton wachsender `seq` ab `1`); Transport-Vertrag als
strukturelle Tabelle `{deliver, receive}` (kein Lua-Interface-Konstrukt).

**Tests:** `endpoints/lua/test.lua` (sync-Loopback über `memTransport`, Teil
der `zerodds-xcdr2-lua`-Coverage); Live-E2E `xrce_8_3_5_8_lua_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 Async `Reader` (Coroutine)

**Spec:** §3 — `asyncReader` ist ein `coroutine.wrap`-Producer; jedes Resume
macht genau einen `transport.receive()`-Versuch und liefert das entrahmte
Body oder `nil`. Kein separater `AsyncWriter` für den Best-Effort-Pfad —
Schreiben bleibt `Client:write`, identisch für sync und async Apps; ein
echter Submit/Drain-Split existiert erst beim reliable Stream (§5), wo er
einen History-Cache und ein periodisches HEARTBEAT trägt.

**Repo:** `endpoints/lua/zerodds.lua::M.asyncReader`.

**Tests:** `endpoints/lua/test.lua` (async-Loopback über `memTransport`);
Live-E2E `xrce_8_3_5_8_lua_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine `lua5.4`-App tauscht mit dem geteilten Rust-XRCE-Peer
über ein echtes `luasocket`-UDP-Datagramm ein typisiertes Sample aus: voller
Stack (generierte `zerodds-idl-lua`-Typen + `endpoints/lua`) sowohl sync als
auch async.

**Repo:** `crates/endpoint-e2e/tests/lua.rs` — `LUA_MAIN` (das von
`crates/endpoint-e2e/tests/lua.rs::gen_module` zur Testlaufzeit als gen.lua
geschriebene Modul mit `marshal_Ping`/`unmarshal_Pong` als Globals +
`require("zerodds")`,
`udpTransport` über `{deliver, receive}`, Modus `sync`/`async` per
CLI-Argument, 50ms Socket-Timeout für nicht-blockierendes `receive()`).

**Tests:** `crates/endpoint-e2e/tests/lua.rs` — beide Fälle laufen live
gegen den Rust-XRCE-Peer über einen echten UDP-Socket:

- `xrce_8_3_5_8_lua_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `zerodds.Client`.
- `xrce_8_3_5_8_lua_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `zerodds.asyncReader`.

Der Harness sucht den Interpreter der Reihe nach als `lua5.4`, `lua`,
`lua5.3` und überspringt hörbar (`SKIP lua endpoint …`), wenn keiner davon
oder `luasocket` fehlt; im Job `integration-bindings-gnat-beam` lässt eine
solche Zeile den Job scheitern.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, `AsyncWriter`

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `Sender:submit`/`pendingHeartbeat`/
`recvAcknack`/`getInFlight`; `Receiver:recvData`/`drainInOrder`/
`pendingAcknack`/`reset`. Window 16, Receiver-Buffer 64, Heartbeat 500 ms,
Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der `AsyncWriter`
(`push`/`drain`) — **kooperativ**, nicht Thread-entkoppelt (§4 der Spec,
Ehrlichkeits-Absatz; ausführlich in §6 dieser Coverage).

**Repo:** `endpoints/lua/reliable.lua` — `writeDataFrame`/`parseWriteData`,
`heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAcknack`; `Sender`,
`Receiver`; `AsyncWriter` (`push`/`drain`/`pending`/`isEmpty`);
`endpoints/lua/example_reliable.lua` (lauffähige In-Process-Demo, kein
Socket); `endpoints/lua/reliable_app.lua` (UDP-Sender-App, Modi `run`/`bench`).

**Tests:**

- `endpoints/lua/reliable_test.lua` in der Spalte `lua` von `tools/endpoints-gate`
  (Job `integration-endpoints`)
  läuft `lua5.4 reliable_test.lua <golden_dir>` gegen die goldenen
  HEARTBEAT/ACKNACK-Bytes, die der Test selbst über `zerodds-xrce` erzeugt
  (dieselbe Bibliothek, die auch den Wire-Bruch anzeigen würde). Lokal
  nachvollzogen mit `lua` 5.5 statt `lua5.4`: **48 Checks**, alle `ok`
  (`ALL OK`) — 44 State-Machine-/Frame-Roundtrip-Checks (monotone `seq`,
  Payload-zu-groß, Window-full, Heartbeat first/silence/nach Periode,
  Heartbeat bei leerem Window, ACKNACK Teil-/Voll-Clear, Receiver
  Reorder/Dedup/Buffer-full, Pending-ACKNACK-Bitmap, Reset, End-to-End-
  Loss-Recovery im Speicher, `AsyncWriter` push/drain inkl. Window-Cap) plus
  4 Byte-Golden-Checks (`endpointlua_5_byte_golden_heartbeat`, `endpointlua_5_byte_golden_acknack`,
  `endpointlua_5_golden_heartbeat_parse`, `endpointlua_5_golden_acknack_parse`) — HEARTBEAT
  `80 00 01 00 0B 01 05 00 01 00 03 00 80` und ACKNACK
  `80 00 01 00 0A 01 05 00 01 00 00 00 80`, identisch zu den
  Referenz-Goldens der anderen SDKs.
- `reliable_5_3_lua_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig
  (`bind_reliable_peer(Some(3))`); die App, die der Test aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und der
  Schleife von `reliable_app.lua` baut, retransmittet auf ACKNACK; alle 12
  Samples lückenlos in Reihenfolge geliefert, der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`).
- `endpoints/lua/example_reliable.lua` in der Spalte `lua` von `tools/endpoints-gate`
  (Job `integration-endpoints`) — `example_reliable.lua` läuft und meldet
  `RELIABLE OK: 12/12 delivered gap-free in 1 round(s), sequence 0..11
  verified in order`. Lokal reproduziert (`lua` 5.5): exakt diese Ausgabe.

**Status:** done.

## §6 Producer-Latenz — ehrlich: kooperativ, nicht nebenläufig

**Spec:** §4 der Spec, Ehrlichkeits-Absatz — `AsyncWriter:push` (Tabellen-
Insert) vs. ein inline `sendto`, ausdrücklich **kein** Beleg für
Thread-Entkopplung: stock `lua5.4` hat keine nativen OS-Threads; `push` und
`drain` laufen im selben Call-Stack desselben OS-Threads (der Aufrufer-Loop
in `reliable_app.lua` ruft `drain()` selbst auf, siehe dessen Kommentar).
Der Messwert zeigt nur den Call-Kosten-Unterschied Tabellen-Insert vs.
UDP-Syscall — keine parallele Verarbeitung.

**Repo:** `endpoints/lua/reliable_app.lua` Funktion `runBench` — 20000
Iterationen inline `udp:send` vs. 20000 Iterationen `AsyncWriter`-Enqueue
(reiner Tabellen-Insert, kein Drain in der Messschleife); Ausgabe
`BENCH enqueue_ns=... inline_send_ns=... note=cooperative_single_os_thread_no_concurrent_drain`.

**Tests:** `reliable_5_4_lua_the_producer_enqueue_costs_less_than_the_inline_send` — läuft `reliable_app.lua` Modus
`bench` und prüft, dass `enqueue_ns` der `BENCH`-Zeile unter
`inline_send_ns` liegt: der Aufruf in die Tabelle des Writers gegen den
inline Send auf demselben OS-Thread, keine nebenläufige Entkopplung.

Gemessen in diesem Lauf (macOS, `lua` 5.5 über den Fallback):
`BENCH enqueue_ns=13 inline_send_ns=2731
note=cooperative_single_os_thread_no_concurrent_drain` — Faktor ~210×,
Tabellen-Insert klar unter dem UDP-Syscall, wie für einen reinen
Call-Kosten-Unterschied auf demselben Thread erwartet. Der frühere Linux/CI-
Referenzlauf nannte enqueue ~29–32 ns / inline ~3780–4050 ns; dieser Lauf
bestätigt die Größenordnung, nicht den Referenzwert.

**Status:** done — der Messwert belegt die Kosten des Aufrufs, nicht die
Nebenläufigkeit. Die Spec verlangt an dieser Stelle den Aufrufkostenbeleg;
damit ist das Item erfüllt.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-lua-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-gnat-beam` (Job `integration-bindings-gnat-beam`) — 4 Tests grün.
* `python3 ci/run_integration.py integration-endpoints` (Job `integration-endpoints`) — 4 Tests grün.

8 zitierte Tests grün, 0 nicht grün.

`crates/endpoint-e2e/tests/lua_reliable.rs` nimmt dieselbe Namensliste wie
`lua.rs` (`lua5.4`, `lua`, `lua5.3`) und meldet den Grund laut, wenn keiner
davon oder `luasocket` fehlt.

Offene Punkte: keine.
