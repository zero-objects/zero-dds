# `zerodds-endpoint-lua` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-lua-1.0.md` — ZeroDDS Lua
Endpoint-SDK-Spec. Ergänzt die Codegen-Coverage `zerodds-xcdr2-lua`
(`docs/spec-coverage/zerodds-xcdr2-lua-1.0.md`) — dort das Marshalling, hier
der Transport.

Implementation:

- `endpoints/lua/zerodds.lua` — XRCE-Framing (`writeFrame`/`readFrame`),
  sync `Client`, coroutine-basierter `asyncReader`.
- `endpoints/lua/reliable.lua` — reliable Sender/Receiver-State-Machine +
  HEARTBEAT/ACKNACK-Wire-Codec + kooperativer `AsyncWriter`.
- `endpoints/lua/reliable_app.lua` — live UDP-Sender-App für das E2E
  (Loss-Recovery + Latenz-Bench).
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
das Framing wird über `lua_endpoint_sync`/`lua_endpoint_async` (§4) live
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
der `zerodds-xcdr2-lua`-Coverage); Live-E2E `lua_endpoint_sync` (§4).

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
Live-E2E `lua_endpoint_async` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine `lua5.4`-App tauscht mit dem geteilten Rust-XRCE-Peer
über ein echtes `luasocket`-UDP-Datagramm ein typisiertes Sample aus: voller
Stack (generierte `zerodds-idl-lua`-Typen + `endpoints/lua`) sowohl sync als
auch async.

**Repo:** `crates/endpoint-e2e/tests/lua.rs` — `LUA_MAIN` (generiertes
`gen.lua` mit `marshal_Ping`/`unmarshal_Pong` als Globals + `require("zerodds")`,
`udpTransport` über `{deliver, receive}`, Modus `sync`/`async` per
CLI-Argument, 50ms Socket-Timeout für nicht-blockierendes `receive()`).

**Tests (lokal nachvollzogen — `lua` 5.5 statt `lua5.4`, `string.pack`/
`coroutine`-Semantik identisch seit Lua 5.3; `example_sync.lua` und
`example_async.lua` liefen fehlerfrei mit `ALL OK`, siehe §6 — der Live-UDP-
Ping-Pong selbst braucht den Rust-Peer aus `zerodds-endpoint-e2e` und wurde
in dieser Session nicht gegen den Peer ausgeführt):

- `lua_endpoint_sync` — voller Stack über `zerodds.Client`.
- `lua_endpoint_async` — voller Stack über `zerodds.asyncReader`.

2/2 (Referenz: bisheriger CI/codepit-Lauf; siehe Audit-Status).

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
Socket); `endpoints/lua/reliable_app.lua` (live UDP-Sender-App für das E2E,
Modi `run`/`bench`).

**Tests:**

- `lua_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/lua_reliable.rs`)
  läuft `lua5.4 reliable_test.lua <golden_dir>` gegen die goldenen
  HEARTBEAT/ACKNACK-Bytes, die der Test selbst über `zerodds-xrce` erzeugt
  (dieselbe Bibliothek, die auch den Wire-Bruch anzeigen würde). Lokal
  nachvollzogen mit `lua` 5.5 statt `lua5.4`: **48 Checks**, alle `ok`
  (`ALL OK`) — 44 State-Machine-/Frame-Roundtrip-Checks (monotone `seq`,
  Payload-zu-groß, Window-full, Heartbeat first/silence/nach Periode,
  Heartbeat bei leerem Window, ACKNACK Teil-/Voll-Clear, Receiver
  Reorder/Dedup/Buffer-full, Pending-ACKNACK-Bitmap, Reset, End-to-End-
  Loss-Recovery im Speicher, `AsyncWriter` push/drain inkl. Window-Cap) plus
  4 Byte-Golden-Checks (`byte_golden_heartbeat`, `byte_golden_acknack`,
  `golden_heartbeat_parse`, `golden_acknack_parse`) — HEARTBEAT
  `80 00 01 00 0B 01 05 00 01 00 03 00 80` und ACKNACK
  `80 00 01 00 0A 01 05 00 01 00 00 00 80`, identisch zu den
  Referenz-Goldens der anderen SDKs.
- `lua_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig
  (`bind_reliable_peer(Some(3))`); die App (`reliable_app.lua` Modus `run`)
  retransmittet auf ACKNACK; alle 12 Samples lückenlos in Reihenfolge
  geliefert (assertiert per Wert- und Reihenfolge-Check im Test).
- `lua_reliable_no_loss` — dieselbe App ohne Drop, lossless Baseline; 12/12.
- `lua_reliable_example` — `example_reliable.lua` läuft und meldet
  `RELIABLE OK: 12/12 delivered gap-free in 1 round(s), sequence 0..11
  verified in order`. Lokal reproduziert (`lua` 5.5): exakt diese Ausgabe.

4/4 dieser Gruppe (Latenz-Bench in §6).

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

**Tests:** `lua_reliable_producer_latency` — läuft `reliable_app.lua` Modus
`bench`, assertiert nur, dass `BENCH` in der Ausgabe steht (bewusst **keine**
`enqueue < inline`-Hartassertion — der Test selbst dokumentiert das als
"honest note", siehe `lua_reliable.rs`-Kommentar).

Lokal nachvollzogen in dieser Session (`lua` 5.5, nicht `lua5.4`, nicht
codepit — reine Plausibilitätsprüfung des Mechanismus, nicht der
Referenzwert): 4 Läufe, `enqueue_ns` 14–18, `inline_send_ns` 2829–3998 —
gleiche Größenordnung (Faktor ~180–260× in dieser Stichprobe), Tabellen-Insert
klar unter dem UDP-Syscall, wie erwartet für einen reinen Call-Kosten-
Unterschied auf demselben Thread. Der frühere codepit/CI-Referenzlauf nannte
enqueue ~29–32 ns / inline ~3780–4050 ns — diese Session hat den codepit-Lauf
nicht erneut ausgeführt, nur die lokale Größenordnung bestätigt.

**Status:** done (als ehrlicher Call-Kosten-Beleg; explizit nicht als
Nebenläufigkeitsbeleg deklariert — das ist hier Teil der Konformanz, nicht
ein offener Punkt).

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (Referenz, `cargo test -p zerodds-endpoint-e2e`, Gate `lua5.4` +
`luasocket`): `--test lua` 2/2 (`lua_endpoint_sync`, `lua_endpoint_async`);
`--test lua_reliable` 5/5 (`lua_reliable_unit_and_golden` — 48 Lua-Checks
inkl. Byte-Golden, `lua_reliable_loss_recovery`, `lua_reliable_no_loss`,
`lua_reliable_example`, `lua_reliable_producer_latency`).

Diese Session hat `reliable_test.lua`, `example_sync.lua`,
`example_async.lua` und `example_reliable.lua` sowie den `bench`-Modus von
`reliable_app.lua` lokal mit `lua` 5.5 (nicht `lua5.4`, kein Rust-Peer)
direkt ausgeführt und die oben zitierten Ausgaben (48/48 Checks, `ALL OK`
×2, `RELIABLE OK: 12/12 ... 1 round(s)`, `BENCH ...`) beobachtet. Der Live-
UDP-Ping-Pong sowie die reliable-Loss-Recovery gegen den Rust-Peer wurden in
dieser Session **nicht** erneut ausgeführt (kein `lua5.4`-Binary, kein
Rust-Peer-Setup verfügbar); deren Status stützt sich auf den bisherigen
CI/codepit-Referenzlauf.

Offene Punkte: keine funktionalen. `endpoints/lua`s Test-Harness bindet sich
strikt an den Binary-Namen `lua5.4` (kein Fallback auf andere Lua-5.4-
kompatible Binary-Namen) — siehe
`docs/specs/zerodds-endpoint-lua-1.0.md` §7.
