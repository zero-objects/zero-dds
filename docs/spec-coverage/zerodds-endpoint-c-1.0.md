# `zerodds-endpoint-c` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-c-1.0.md` -- Natives C-Endpoint-SDK-Spec
(XRCE-Framing, Sync/Async, Reliable Stream). Reliable-Stream-Kontraktdetails in
`docs/spec-coverage/reliable-endpoint-1.0.md`.

Deckt den C-**ENDPOINT**-Stack ab (getrennt von der C-**Codegen**-Spec
`docs/spec-coverage/zerodds-xcdr2-c-1.0.md`): XRCE-Framing, sync send/recv, async
Reader/Writer (inkl. Live-Ping-Pong-E2E-Pflicht) und den reliable Stream.

Implementation:

- `endpoints/c/include/zerodds_endpoint.h` / `src/zerodds_endpoint.c` — Frame-Hook,
  XRCE-Framing, serielles HDLC-Framing, reliable State-Machine (C89).
- `endpoints/c/include/zerodds_async.h` / `src/zerodds_async.c` — Async-Reactor
  (C11), `zdw_async_reader` / `zdw_async_writer`.
- `endpoints/c/include/zerodds_reliable_async.h` / `src/zerodds_reliable_async.c` —
  SPSC-Ring + pthread-Drain-Thread für den entkoppelten reliable Writer (C11).
- `crates/endpoint-e2e/tests/c_reliable.rs` — Live-UDP-E2E gegen den
  Rust-Referenz-Peer.

## §1 XRCE-Framing

### §1.1 WRITE_DATA-Frame (Session/Stream/Sequence + Sample-Body)

**Spec:** §1.1 (`zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 §8.3.2.3/§8.3.4) —
Message-Header (`session_id`, `stream_id`, `sequence_nr` LE) + WRITE_DATA-
Submessage-Header (id=7, flags, Länge LE) + XCDR-Sample-Body. Best-effort,
kein ClientKey (`session_id ≥ 128`).

**Repo:** `endpoints/c/include/zerodds_endpoint.h::zdw_xrce_write_frame` /
`zdw_xrce_read_frame`; `ZDW_XRCE_SESSION_NOKEY` / `ZDW_XRCE_STREAM_BEST_EFFORT`.

**Tests:** `endpoints/c/test/test_xrce_frame.c` — baut den Frame, vergleicht
byte-genau gegen `golden_xrce_le.bin` (echte `crates/xrce`-Message), läuft dann
durch den Frame-Hook (encode → Frame → Transport → receive → unwrap → decode).
Lokal nachgefahren: `XRCE WRITE_DATA frame 48 bytes byte-identical to crates/xrce`,
`frame-hook round-trip: unwrapped + decoded ok`, `ALL OK`.

**Status:** done

### §1.2 Empfangspfad (DATA-Message vom Agenten)

**Spec:** §1.2 (`zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 §8.3.4) —
DATA-Submessage (id=9), vom Agenten an den Client gepusht.

**Repo:** `zdw_xrce_read_frame` (gemeinsame Unwrap-Funktion für WRITE_DATA und
DATA).

**Tests:** `endpoints/c/test/test_receive.c` gegen `golden_data_le.bin` (reale
`zerodds-xrce`-DATA-Message). Lokal nachgefahren: `received DATA frame, 40-byte
sample body`, `agent DATA decoded: id=0xA1B2C3D4 label=bay-12`, `ALL OK`.

**Status:** done

### §1.3 Serielles HDLC-Framing (Annex C, RFC 1662)

**Spec:** §1.3 (`zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 Annex C) —
`7E [byte-stuffed(payload) byte-stuffed(crc16-BE)] 7E`; Stuffing von
`0x7E`/`0x7D`; CRC-16-CCITT-FALSE (init `0xFFFF`, Poly `0x1021`) über den
rohen Payload.

**Repo:** `zdw_crc16_ccitt_false`, `zdw_serial_frame`, `zdw_serial_deframe`.

**Tests:** `endpoints/c/test/test_serial_frame.c` gegen `golden_serial_le.bin`.
Lokal nachgefahren: `byte-stuffing: 0x7E->7D5E, 0x7D->7D5D ok`, `XRCE serial
frame 52 bytes byte-identical to crates/xrce`, `serial round-trip: deframe + crc
+ unwrap + decode ok`, `ALL OK`.

**Status:** done

## §2 sync send/recv

### §2.1 Frame-Hook-Vertrag (`zdw_transport`)

**Spec:** §2.1 (`zerodds-endpoint-c-1.0.md`, ADR 0013 Invariante 5) — der
Endpoint ist transport-opak: er reicht ein vollständig geframtes, encodiertes
Message an den vom Integrator gefüllten Transport (`ctx` + `deliver`/`receive`-
Funktionspointer) und erhält vollständige Frames zurück.

**Repo:** `endpoints/c/include/zerodds_endpoint.h::zdw_transport`,
`zdw_endpoint_send`, `zdw_endpoint_recv`.

**Tests:** `endpoints/c/test/test_endpoint_loopback.c` — In-Memory-Loopback-
Transport, Encode → Send → Receive → Decode, Feldvergleich. Lokal nachgefahren:
`frame-hook: 40-byte frame delivered + received + decoded ok`, `ALL OK`.

**Status:** done

### §2.2 Poll-Loop-Idiom (C89)

**Spec:** §2.2 (`zerodds-endpoint-c-1.0.md`) — sync ist die C89-Baseline-
Nutzung des Frame-Hooks: der Integrator besitzt den Run-Loop und ruft
`zdw_endpoint_recv` selbst.

**Repo:** `endpoints/c/examples/example_sync.c` (C89-Poll-Loop über den
Transport).

**Tests:** `make -C endpoints/c examples` (Ziel `example_sync`); druckt fünf
dekodierte `Reading`-Samples + `ALL OK`. CI-Job `endpoints-native`.

**Status:** done

## §3 async Reader/Writer

### §3.1 Event-driven Reactor (`zdw_async_reader` / `zdw_async_writer`)

**Spec:** §3.1 (`zerodds-endpoint-c-1.0.md`, ADR 0013) — additiv zum C89-Kern
eine Latenz-entkoppelte, callback-getriebene Alternative zum Poll-Loop (C11,
kein malloc).

**Repo:** `endpoints/c/include/zerodds_async.h` / `src/zerodds_async.c` —
`zdw_async_reader_init` bindet einen `on_sample`-Callback,
`zdw_async_run` drained den Transport und dispatcht jedes dekodierte Sample;
`zdw_async_writer_init` + `zdw_async_write`.

**Tests:** `endpoints/c/test/test_async_loopback.c` — In-Memory-FIFO-Transport,
5 Samples geschrieben, Reactor dispatcht alle 5 in Reihenfolge. Lokal
nachgefahren: `async loopback: 5 samples dispatched + decoded in order`,
`ALL OK`.

**Status:** done

### §3.2 Live-UDP-Reactor

**Spec:** §3.2 (`zerodds-endpoint-c-1.0.md`) — Nachweis, dass der Reactor
echte Datagramm-I/O treibt, nicht nur eine In-Memory-Queue.

**Repo:** dieselben Reactor-Funktionen über einen nicht-blockierenden POSIX-UDP-
Socket (`endpoints/c/test/test_async_udp.c`).

**Tests:** `endpoints/c/test/test_async_udp.c` — 5 Samples über UDP-Loopback.
Lokal nachgefahren: `async UDP: 5/5 samples received + decoded via reactor`,
`ALL OK`.

**Status:** done

### §3.3 Async-Deep-Example

**Spec:** §3.3 (`zerodds-endpoint-c-1.0.md`) — lauffähiges Beispiel, kein Stub.

**Repo:** `endpoints/c/examples/example_async.c` (C11-Reactor + Callback).

**Tests:** `make -C endpoints/c examples` (Ziel `example_async`); fünf dekodierte
Readings + `ALL OK`. CI-Job `endpoints-native`; codepit-verifiziert
(5/5 Feld-Decode, byte-identisch via `zdw`, siehe
`docs/spec-coverage/zerodds-xcdr2-c-1.0.md` §8).

**Status:** done

### §3.4 Test-Pflicht: Live-Ping-Pong-E2E

**Spec:** §3.4 (`zerodds-endpoint-c-1.0.md`) — zusätzlich zu den isolierten
Unit-/Loopback-Tests aus §1–§3 MUSS jede Sprache eine Live-E2E-Datei nach dem
Muster `crates/endpoint-e2e/tests/<lang>.rs` bereitstellen (Referenz: `cpp.rs`
für C++ — raw/sync/async-Modi gegen den `idl-cpp`-generierten `Ping`/`Pong`-
Codec), damit Framing (§1), sync (§2) und async (§3) im Zusammenspiel belegt
sind.

**Repo:** Für **C** existiert in `crates/endpoint-e2e/tests/` keine eigene
`c.rs`-Ping-Pong-Datei (anders als für `cpp`, `ada`, `d`, `nim`, `zig`, `csharp`,
`go`, `java`, `julia`, `lua`, `ocaml`, `python`, `swift` — siehe
`crates/endpoint-e2e/tests/`). Die C-Endpoint-Fähigkeiten (Framing §1, sync §2,
async §3) werden stattdessen über die C-SDK-eigene Testsuite (`endpoints/c/test/`)
und, für den reliable Stream, über `crates/endpoint-e2e/tests/c_reliable.rs`
belegt (§4). Eine dedizierte Ping-Pong-E2E-Datei für C fehlt.

**Tests:** —

**Status:** open — Ping-Pong-E2E (`c.rs` nach dem `cpp.rs`-Muster) ist nicht
angelegt; kein erfundener Test wird hier zitiert.

## §4 reliable Stream

### §4.1 C89-State-Machine (`zdw_reliable`)

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §3 — reliabler Sender +
Receiver, gespiegelt von `crates/xrce::ReliableStreamState`; feste Storage,
kein malloc: `ZDW_REL_WINDOW`=16, `ZDW_REL_RECV_BUF`=64, RFC-1982-16-bit-
Sequenznummern.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (Deklaration) /
`src/zerodds_endpoint.c` (Implementierung, C89) — `zdw_reliable_submit`,
`zdw_reliable_pending_heartbeat`, `zdw_reliable_recv_acknack`,
`zdw_reliable_get_in_flight` (Sender); `zdw_reliable_recv_data`,
`zdw_reliable_drain`, `zdw_reliable_pending_acknack` (Receiver);
`zdw_reliable_reset`.

**Tests:** `endpoints/c/test/test_reliable_sm.c` — 14 Prüf-Funktionen: 13
spiegeln die `crates/xrce::reliable`-Referenztests (monotone `seq`,
window-full, heartbeat first/silence/erneut, acknack-clear/full-clear,
in-order-drain, reorder, duplicate-drop, buffer-full, pending-acknack-Bitmap,
reset, End-to-End-Loss-Recovery in-process) plus 1 Byte-Golden-Check. Lokal
nachgefahren (`make build/test_reliable_sm && ./build/test_reliable_sm
golden_heartbeat_le.bin golden_acknack_le.bin`): `test_reliable_sm: ALL OK`.

**Status:** done

### §4.2 Byte-golden HEARTBEAT/ACKNACK

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §4 — `AckNack{first_unacked_seq_num
i16, nack_bitmap [u8;2] LE, stream_id u8}`, `Heartbeat{first_unacked_seq_nr i16,
last_unacked_seq_nr i16, stream_id u8}`.

**Repo:** `t_byte_golden` in `test_reliable_sm.c` parst `golden_heartbeat_le.bin`
/ `golden_acknack_le.bin`, baut neu, vergleicht byte-genau; zusätzlich
`endpoints/c/test/test_reliable.c` (reiner Wire-Test, ohne State-Machine).

**Tests:** dito §4.1 sowie `endpoints/c/test/test_reliable.c`. Lokal
nachgefahren: `HEARTBEAT parsed: first=1 last=3 stream=0x80`, `ACKNACK 13 bytes
byte-identical to crates/xrce`, `ALL OK`.

**Status:** done

### §4.3 pthread-SPSC-Ring async Writer (Latenz-Entkopplung)

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §2 — der Producer darf nie in
den Kernel eintreten; das Bauteil trägt zugleich den reliable Sender-State.

**Repo:** `endpoints/c/include/zerodds_reliable_async.h` /
`src/zerodds_reliable_async.c` (C11 + pthread) — `zdw_async_ring_start` startet
den Drain-Thread, `zdw_async_ring_enqueue` ist ein wait-freies Enqueue (kein
Syscall), der Drain-Thread hält `zdw_reliable rel` und übernimmt Framing +
`sendmmsg`-artige I/O.

**Tests:** `endpoints/c/test/bench_reliable_async.c` (Latenz-Vergleich, siehe
§4.6) und `endpoints/c/examples/example_reliable.c` (Loss-Recovery-Demo,
in-process, kein Netzwerk).

**Status:** done

### §4.4 Loss-Recovery-Example

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 5 — lauffähiges
Beispiel, kein Stub; N Samples, injizierter Verlust, lückenlose Zustellung
nach Retransmit.

**Repo:** `endpoints/c/examples/example_reliable.c` — 12 Samples submitted,
jedes 4. beim ersten Durchlauf verworfen, Recovery-Runden bis alle 12 in Folge
0..11 ankommen.

**Tests:** `make -C endpoints/c examples` (Ziel `example_reliable`). Lokal
nachgefahren: `reliable: 3/12 delivered before recovery (3 lost)`, `reliable:
delivered contiguous 0..11, expected=12`, `reliable: ALL 12 samples recovered
gap-free`.

**Status:** done

### §4.5 Live-E2E gegen den Rust-Referenz-Peer

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 3 — live gegen den
Rust reliable Peer (`zerodds-endpoint-e2e`), `stream_id ≥ 128`, mit
injiziertem Drop; Assert: alle Samples lückenlos in-order geliefert trotz Loss.

**Repo:** `endpoints/c/test/reliable_udp_app.c` — die C-App spielt den
reliablen **Sender** (submit + WRITE_DATA + HEARTBEAT + ACKNACK-getriebener
Retransmit) gegen den geteilten Rust `ReliablePeer`, der Loss injiziert.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`
(12 Samples, injizierter Drop, Assert lückenlos) und
`::c_reliable_no_loss` (12 Samples ohne Drop). Auf codepit (Linux) laut
vorheriger Verifikation: `cargo test --test c_reliable` → 3 passed. **Auf
diesem Host (macOS/Apple-Clang) lokal nachgefahren: Build schlägt fehl** —
`endpoints/c/test/reliable_udp_app.c` definiert `_POSIX_C_SOURCE 200809L`,
was unter Apples libc `INADDR_LOOPBACK` (eine BSD-Socket-Erweiterung, keine
POSIX-Konstante) ausblendet (`error: use of undeclared identifier
'INADDR_LOOPBACK'`); Linux/glibc kennt diese Restriktion nicht. Ein
Host-spezifischer Build-Unterschied, kein auf diesem Host verifizierter
Testlauf — die Datei wurde nicht verändert (außerhalb des Auftrags dieses
Dokuments).

**Status:** done (codepit/Linux, gemäß vorheriger Verifikation) — **auf
macOS lokal nicht baubar** (Fundstelle oben); nicht erneut selbst auf Linux
nachgefahren im Rahmen dieser Doku-Erstellung.

### §4.6 Latenz: Producer-Latenz inline send vs. entkoppeltes Ring-Enqueue

**Spec:** §4 (`zerodds-endpoint-c-1.0.md`) i.V.m.
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 4 — Producer
`write→return` im async-entkoppelten Pfad vs. inline Deliver; ein Messwert,
der die Entkopplung zeigt.

**Repo:** `endpoints/c/test/bench_reliable_async.c` — misst `inline_ns_per_op`
(Frame + `send()`-Syscall pro Sample, gegen einen gebundenen-aber-ungelesenen
Loopback-Socket) gegen `decoupled_enqueue_ns_per_op` (reines
`zdw_async_ring_enqueue`, kein Kernel).

**Tests:** `endpoints/c/test/bench_reliable_async.c` (Ziel `bench_reliable_async`)
und `crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench`. Laut
Aufgabenstellung/vorheriger Verifikation (codepit): inline ~3.5 µs vs.
entkoppeltes Ring-Enqueue ~5–7 ns (~600×). **Auf diesem Host (macOS)**: Build
schlägt mit derselben `INADDR_LOOPBACK`-Ursache wie §4.5 fehl (`bench_reliable_async.c`
nutzt `-std=c11 -pedantic` ohne `_DEFAULT_SOURCE`, anders als
`test_async_udp.c`, das `_DEFAULT_SOURCE 1` setzt) — kein lokaler Messwert
erhoben.

**Status:** done (codepit/Linux, gemäß vorheriger Verifikation) — **auf
macOS lokal nicht baubar**, Ursache identisch zu §4.5.

---

## Audit-Status

15 done / 0 partial / 1 open / 0 n/a.

Test-Lauf (dieser Host, macOS/Apple-Clang, `cc` = clang):
`make -C endpoints/c build/test_xrce_frame build/test_receive
build/test_endpoint_loopback build/test_reliable_sm build/test_reliable
build/example_reliable build/test_async_loopback build/test_async_udp
build/test_serial_frame GOLDEN_DIR=<goldens aus zerodds-endpoint-golden>` →
alle 9 Binaries bauen fehlerfrei (`-std=c89 -pedantic -Wall -Wextra` bzw.
`-std=c11 -pedantic -Wall -Wextra`, 0 Warnungen) und laufen `ALL OK`
(`example_reliable` druckt keine `ALL OK`-Zeile, meldet aber `ALL 12 samples
recovered gap-free` mit Exit 0). `endpoints/c/test/bench_reliable_async.c` und
`endpoints/c/test/reliable_udp_app.c` (letzteres nur via
`cargo test -p zerodds-endpoint-e2e --test c_reliable`) bauen auf diesem Host
**nicht** — `INADDR_LOOPBACK` unter Apples libc nicht sichtbar
(§4.5/§4.6-Fundstelle). Die für §4.5/§4.6 zitierten Ergebnisse (3 passed;
~3.5 µs vs. ~5–7 ns) stammen aus vorheriger codepit(Linux)-Verifikation, nicht
aus einem in dieser Sitzung selbst durchgeführten Linux-Lauf.

Offene Punkte:
- §3.4 Live-Ping-Pong-E2E für C (`crates/endpoint-e2e/tests/c.rs` nach dem
  `cpp.rs`-Muster) ist nicht angelegt.
- §4.5/§4.6 auf diesem Host (macOS) nicht baubar/nicht nachgefahren; nur
  codepit(Linux)-Altbeleg zitiert, keine erneute eigene Linux-Verifikation in
  dieser Sitzung.
