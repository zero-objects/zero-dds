<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-lua` v1.0 — Lua Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/lua/`.

Ergänzt die Codegen-Spec [`zerodds-xcdr2-lua`](zerodds-xcdr2-lua-1.0.md) (dort
das Marshalling per `string.pack`/`string.unpack`) und die Endpoint-SDKs der
anderen Sprachen (`endpoints/go`, `endpoints/zig`, `endpoints/nim`,
`endpoints/d`, `endpoints/c`, `endpoints/ada`, ...): das native
Lua-Endpoint-Modul `zerodds` über XRCE-Framing, den sync `Client`, den
coroutine-basierten `asyncReader` und den reliable Stream, so dass eine
`lua5.4`-App ein XCDR2-Sample byte-identisch zu `crates/xrce` +
`endpoints/c` mit dem geteilten Rust-Peer austauscht.

Anders als die C-, Go- oder Ada-Endpoints hat stock `lua5.4` **keine nativen
Betriebssystem-Threads**. Wo diese Spec "async" schreibt, ist damit
durchgängig **kooperative Nebenläufigkeit** gemeint (Coroutinen, ein
Producer/Consumer-Interleaving auf demselben OS-Thread) — nie parallele
Ausführung. §3 und §4 machen das je an der Stelle explizit, an der es für
die Konformanz zählt.

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`:

```
[0]=session [1]=stream [2..4)=seq u16 LE [4]=submessage id [5]=flags [6..8)=len u16 LE [8..]=body
```

`endpoints/lua/zerodds.lua` (Modul `zerodds`, geladen per `require("zerodds")`)
MUSS bereitstellen:

- `M.writeFrame(session, stream, seqNo, sample) -> string` — framet ein
  Sample; Flags fest `0x03` (E-Flag = LE plus `DataFormat::Sample`-Bits).
- `M.readFrame(frame) -> string|nil` — entrahmt; `nil` bei zu kurzem Frame
  (`#frame < 8`) oder falscher Submessage-ID (`frame:byte(5) ~= 0x07`).
- Konstanten `M.SESSION_NOKEY` (`0x80`, best-effort, ohne ClientKey) und
  `M.STREAM_BEST_EFFORT` (`0x01`).
- `M.LE` (`"<"`) / `M.BE` (`">"`) als `string.pack`/`string.unpack`-Endian-Präfixe,
  geteilt mit dem Marshalling aus `zerodds-xcdr2-lua`.

## §2 Sync `Client`

Ein gepollter, nicht-blockierender Client — Lua hat kein eingebautes
Async-I/O-Primitive, das die SDK selbst verwenden könnte, also überlässt der
`Client` das Deadline-Handling dem Aufrufer (vgl. §5 Punkt 1).

`endpoints/lua/zerodds.lua` MUSS bereitstellen:

- Einen `Transport`-Vertrag als einfache Lua-Tabelle
  `{ deliver = function(frame) ... end, receive = function() ... end }` —
  Lua kennt kein Interface-Konstrukt, der Vertrag ist rein strukturell
  (Duck-Typing); der Integrator implementiert ihn für seinen Link (z. B.
  UDP über `luasocket`). `receive()` liefert `nil`, wenn im Moment nichts
  anliegt; ob und wie lange der zugrunde liegende Aufruf blockiert (z. B.
  ein Socket-Timeout), legt der Integrator fest — die SDK selbst schläft
  nie.
- `M.Client.new(transport) -> Client` mit `session = M.SESSION_NOKEY`,
  `stream = M.STREAM_BEST_EFFORT`, `seq = 1` als Defaults.
- `Client:write(sample)` — framet + liefert synchron über den `Transport`;
  erhöht `seq` monoton modulo `0x10000` pro Aufruf.
- `Client:poll() -> body|nil` — genau ein nicht-blockierender
  Empfangsversuch: ruft `transport.receive()` einmal auf, entrahmt bei Erfolg.
  Es gibt **keine** eingebaute `Receive(timeout)`-Methode wie im Go-SDK; ein
  Aufrufer, der bis zu einer Deadline warten will, pollt selbst in einer
  Schleife (`repeat body = c:poll() until body ~= nil or <deadline erreicht>`,
  so in `crates/endpoint-e2e/tests/lua.rs`).

## §3 Async `Reader` (Coroutine)

Das idiomatische Lua-Async-Modell für die Leseseite: keine Goroutine/kein
Thread, sondern eine Coroutine, die bei jedem Resume genau einen
`transport.receive()`-Versuch macht und das entrahmte Sample-Body (oder
`nil`, wenn der Transport gerade leer ist) an den Aufrufer zurückgibt.

`endpoints/lua/zerodds.lua` MUSS bereitstellen:

- `M.asyncReader(transport) -> function` — liefert einen
  `coroutine.wrap`-Producer. Jeder Aufruf des zurückgegebenen Funktionswerts
  resumed die Coroutine um genau einen `transport.receive()`-Versuch;
  liefert entweder den entrahmten Body oder `nil`.

Es gibt in `zerodds.lua` **keinen separaten `AsyncWriter`**-Typ für den
Best-Effort-Pfad: Die Schreibseite ist bereits ein einzelner, nicht
blockierender Framing+Deliver-Aufruf (`Client:write`) — es gibt nichts, das
ein zweiter, entkoppelter Typ auf stock Lua gewinnen könnte, solange kein
Syscall-lastiger Producer-Loop involviert ist. Sync- und Async-Apps rufen
für den Schreibpfad identisch `Client:write` auf; der Unterschied liegt
allein in der Leseseite (`Client:poll` vs. `asyncReader`). Ein
schreibseitiges Async-Bauteil mit echtem Submit/Drain-Split existiert erst
für den reliable Stream (§4) — dort lohnt es sich, weil dort ein History-
Cache und ein periodisches HEARTBEAT dazukommen.

## §4 Reliable Stream

`endpoints/lua` implementiert den reliable Stream als Endpoint-Fähigkeit
gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie einen
`AsyncWriter` mit Submit/Drain-Split.

`endpoints/lua/reliable.lua` (eigenständiges Modul, `require("reliable")`,
`require`-t **nicht** `zerodds` — dessen `Writer`/`Reader` sind
Chunk-lokal und von außen nicht erreichbar; `reliable.lua` trägt daher
einen eigenen minimalen Frame-Codec, byte-identisch zu `crates/xrce` und den
übrigen Endpoint-SDKs) MUSS bereitstellen:

- Die Konstanten `SESSION_NOKEY=0x80`, `RELIABLE_STREAM_ID=0x80`,
  `HEARTBEAT_PERIOD_MS=500`, `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
  `MAX_PAYLOAD=65535`.
- Den Sender-Kontrakt `Sender:submit(payload) -> seq|nil, err`
  (`"too_large"`/`"window_full"`), `Sender:pendingHeartbeat(nowMs)`,
  `Sender:recvAcknack(firstUnacked, nackLo, nackHi)`,
  `Sender:getInFlight(seq)`, `Sender:reset()`.
- Den Receiver-Kontrakt `Receiver:recvData(seq, payload) -> bool`,
  `Receiver:drainInOrder() -> {seq, payload}[]`,
  `Receiver:pendingAcknack(hintLastSeen)`, `Receiver:reset()`.
- Den Wire-Codec `writeDataFrame`/`parseWriteData`,
  `heartbeatFrame`/`parseHeartbeat`, `acknackFrame`/`parseAcknack` — HEARTBEAT
  `0x0B`, ACKNACK `0x0A`, RFC-1982-16-bit-Sequenznummern, byte-identisch zu
  den Referenz-Goldens.
- Einen `AsyncWriter` mit `push(payload) -> bool` (wait-free Tabellen-Insert,
  gedeckelt auf `SENDER_WINDOW`) und `drain() -> n` (submitted + sendet alles
  Gepufferte über den mitgegebenen `sendFn`).

**Honestly (verbindlicher Teil dieser Spec, keine Fußnote):** Der
`AsyncWriter` ist ein **kooperatives** Submit/Drain-Split, keine
Thread-entkoppelte Queue wie beim Go- oder C-SDK. Auf stock `lua5.4` gibt es
keine nativen OS-Threads; `push` und `drain` laufen im selben Call-Stack
desselben OS-Threads (der Aufrufer-Loop in `reliable_app.lua` ruft
`writer:drain()` explizit selbst auf). `push` ist billiger als ein inline
`sendto`, weil es kein Syscall ist (reiner Tabellen-Insert) — aber es gibt
keine zweite, parallel laufende Instanz, die währenddessen drained; der
Producer treibt den Socket am Ende immer noch selbst, nur über einen
späteren, günstigeren Call auf seinem eigenen Pfad. Ein Latenz-Messwert
zwischen `push` und einem inline `sendto` (§5 Punkt 3, `reliable_app.lua`
Modus `bench`) ist entsprechend **kein Beleg für nebenläufige Entkopplung**,
sondern für den reinen Call-Kosten-Unterschied Tabellen-Insert vs.
UDP-Syscall auf demselben Thread.

## §5 Conformance

Eine Lua-Endpoint-Implementierung ist konform, wenn:

1. Der volle Stack (generierte Typen aus `zerodds-xcdr2-lua` + `zerodds.lua`)
   ein typisiertes Sample sowohl über den sync `Client` als auch über
   `asyncReader` mit dem geteilten Rust-XRCE-Peer über ein reales
   `luasocket`-UDP-Datagramm austauscht; die Deadline-Schleife liegt dabei
   beim Aufrufer (§2).
2. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens
   sind (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`) und der
   reliable Stream Datagramm-Verlust lückenlos in-order aufholt (§4,
   `reliable-endpoint` v1.0 §5).
3. Ein Latenz-Messwert `AsyncWriter:push` vs. ein inline `sendto` gezogen
   wird — und **ehrlich** als Call-Kosten-Differenz auf demselben OS-Thread
   ausgewiesen wird, nicht als Beleg für Nebenläufigkeit (§4).
4. Fehlt `luasocket` (das `socket`-Modul) auf dem `lua5.4`, überspringen die
   netzwerkgebundenen Tests **laut** (`SKIP ...` auf stderr) statt still
   grün zu erscheinen — kein False-Green.

## §6 Beispiele

- Sync: `endpoints/lua/example_sync.lua` — Poll-Loop über `Client:poll`,
  vollem Feld-Decode einer `Reading { id, value, label }`.
- Async: `endpoints/lua/example_async.lua` — dieselbe Telemetrie, aber über
  den `coroutine.wrap`-`asyncReader`.
- Reliable: `endpoints/lua/example_reliable.lua` — In-Process-Demo (kein
  Socket) der Loss-Recovery über den Wire-Codec (`writeDataFrame`/
  `parseWriteData`), nicht nur die reine State-Machine im Speicher.
- Quickstart: `endpoints/lua/QUICKSTART.md`.

## §7 Errata + Open-Questions

- Die Live-E2E-Tests (`crates/endpoint-e2e/tests/lua.rs`,
  `lua_reliable.rs`) rufen das Binary strikt unter dem Namen `lua5.4` auf
  (`Command::new("lua5.4")`). Ein anderweitig benanntes Lua-5.4-kompatibles
  Binary (z. B. nur `lua` oder `lua5.5` auf PATH) wird nicht gefunden und
  führt zum lauten Skip, nicht zum Fallback auf eine andere Binary — offen,
  ob ein PATH-Alias/Config-Override sinnvoll wäre.
- Sync und async sind vollständig implementiert und byte-verifiziert;
  reliable ebenso (siehe
  `docs/spec-coverage/zerodds-endpoint-lua-1.0.md`). Kein funktionales
  Defizit offen. Die kooperative (nicht parallele) Natur von `AsyncWriter`
  (§4) ist ein bewusster Designpunkt dieser Sprache, kein offener Punkt.
