# `zerodds-endpoint-c` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-c-1.0.md` — natives C-Endpoint-SDK
(XRCE-Framing, Sync/Async, Reliable Stream). Die Reliable-Stream-Kontraktdetails
stehen in `docs/spec-coverage/reliable-endpoint-1.0.md`.

Deckt den C-**ENDPOINT**-Stack ab (getrennt von der C-**Codegen**-Spec
`matrix/zerodds-c/`): XRCE-Framing, sync send/recv, async Reader/Writer
(inklusive Live-Ping-Pong-E2E-Pflicht) und den reliable Stream.

Implementation:

- `endpoints/c/include/zerodds_endpoint.h` / `endpoints/c/src/zerodds_endpoint.c`
  — Frame-Hook, XRCE-Framing, serielles Framing nach Annex C, reliable State-Machine (C89).
- `endpoints/c/include/zerodds_async.h` / `endpoints/c/src/zerodds_async.c` —
  Async-Reactor (C11).
- `endpoints/c/include/zerodds_reliable_async.h` /
  `endpoints/c/src/zerodds_reliable_async.c` — SPSC-Ring plus pthread-Drain-Thread
  für den entkoppelten reliable Writer (C11).
- `crates/endpoint-e2e/tests/c.rs` und `crates/endpoint-e2e/tests/c_reliable.rs`
  — Live-UDP-E2E gegen den Rust-Referenz-Peer.

## Wie die Belege laufen

Zwei getrennte Läufe, beide in diesem Audit gefahren:

1. **C-Seite.** Die Dateien unter `endpoints/c/test/` und
   `endpoints/c/examples/` sind C-Programme. Sie sind **nicht** Teil von
   `cargo test`; ein reiner Rust-Lauf führt null von ihnen aus. Gebaut und
   gefahren werden sie über `endpoints/c/Makefile`:
   `make -C endpoints/c test GOLDEN_DIR=<dir>` und
   `make -C endpoints/c examples`, wobei `<dir>` die Byte-Goldens aus
   `cargo run -p zerodds-endpoint-golden -- <dir>` enthält. In der Pipeline
   fährt der Job `integration-endpoints` (`ci/integration_scenarios.py`, Spalte `c` von
   `tools/endpoints-gate`) genau diese beiden Kommandos.
2. **Rust-Seite.** `crates/endpoint-e2e/tests/c.rs` und
   `crates/endpoint-e2e/tests/c_reliable.rs` sind in der Krate-Ebene
   `#[ignore]` (der Grund nennt den Job) und laufen im Job
   `integration-bindings` der Integrations-Säule
   (`ci/integration_scenarios.py`, `--run-ignored all`). Jeder Fall prüft
   zuerst, ob ein C-Compiler auf dem `PATH` liegt, und kehrt sonst mit einer
   `SKIP`-Zeile zurück; `ci/check_skips.py` lässt den Job an einer solchen
   Zeile scheitern, solange `ci/declared-skips.tsv` sie nicht deklariert.

Jedes Item nennt unten, welcher der beiden Läufe seinen Beleg trägt.

Offene Punkte: keine. Decision-Records: keine.

## §1 XRCE-Framing

### §1.1 WRITE_DATA-Frame (Session/Stream/Sequence + Sample-Body)

**Spec:** §1.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0
§8.3.2.3/§8.3.4) — Message-Header (`session_id`, `stream_id`, `sequence_nr`
little-endian) plus WRITE_DATA-Submessage-Header (id = 7, Flags, Länge
little-endian) plus XCDR-Sample-Body. Best-effort, kein ClientKey
(`session_id >= 128`).

**Repo:** `endpoints/c/include/zerodds_endpoint.h` /
`endpoints/c/src/zerodds_endpoint.c` (Funktionen `zdw_xrce_write_frame` und
`zdw_xrce_read_frame`, Konstanten `ZDW_XRCE_SESSION_NOKEY` und
`ZDW_XRCE_STREAM_BEST_EFFORT`).

**Tests:** C-Seite — `endpoints/c/test/test_xrce_frame.c`, gefahren über den
Makefile-Lauf (`make -C endpoints/c test`); baut den Frame und vergleicht ihn byte-genau
gegen `golden_xrce_le.bin`, dann Round-Trip durch den Frame-Hook (encode,
Frame, Transport, receive, unwrap, decode). Gemessene Ausgabe dieses Audits:
XRCE WRITE_DATA frame 48 bytes byte-identical to crates/xrce.
Rust-Seite — `crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_sync_app_writes_the_ping_and_reads_the_pong` (der
Rust-Peer entrahmt den vom C-Programm gesendeten WRITE_DATA-Frame live über
UDP und prüft die 23 Oktetts des `Ping`). Die Referenz, aus der das Golden
entsteht: `crates/xrce/src/submessages/write_data.rs::tests::xrce_8_2_3_write_data_flags_are_e_le_and_format_data`,
`crates/xrce/src/submessages/write_data.rs::tests::xrce_8_3_5_8_write_data_base_object_request_precedes_the_sample`,
`crates/xrce/src/submessages/write_data.rs::tests::xrce_7_7_1_write_data_matches_the_annex_b_example`.

**Status:** done

### §1.2 Empfangspfad (DATA-Message vom Agenten)

**Spec:** §1.2 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 §8.3.4)
— die DATA-Submessage (id = 9) ist die Richtung Agent → Client. Der Endpoint
MUSS sie annehmen; WRITE_DATA in dieser Richtung wäre falsch.

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (`zdw_xrce_read_frame`, die
gemeinsame Unwrap-Funktion für WRITE_DATA und DATA).

**Tests:** C-Seite — `endpoints/c/test/test_receive.c` gegen
`golden_data_le.bin` (Makefile-Lauf `make -C endpoints/c test`). Gemessene
Ausgabe dieses Audits: received DATA frame, 40-byte sample body / agent DATA
decoded. Rust-Seite — `crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_sync_app_writes_the_ping_and_reads_the_pong` und
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_async_app_writes_the_ping_and_reads_the_pong`: der Peer antwortet
ausschließlich mit id = 9, das C-Programm muss den Pong daraus lesen. Die
Referenz für das Golden: `crates/xrce/src/submessages/data.rs::tests::xrce_8_3_5_10_data_uses_id_9`,
`crates/xrce/src/submessages/data.rs::tests::xrce_7_7_1_data_base_object_request_precedes_the_sample`.

**Status:** done

### §1.3 Serielles Framing (Annex C)

**Spec:** §1.3 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 Annex C.1)
— `7E | sourceAddress | remoteAddress | payloadLength (LE) | payload |
frameCheck (LE)`; Stuffing von `0x7E` und `0x7D` nach dem Flag; `frameCheck`
die FCS-16 nach RFC 1662 über Adressen, Länge und Payload; Abweisung bei
falschem `frameCheck` und bei `0x7E` vor dem Rahmenende (C.1.3).

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (`zdw_fcs16`,
`zdw_serial_frame`, `zdw_serial_deframe`).

**Tests:** C-Seite — `endpoints/c/test/test_serial_frame.c` gegen
`golden_serial_le.bin` (`tools/endpoints-gate`, Lauf auf dem Linux-Testhost am
Stand `8bcb1d00161`). Gemessene Ausgabe: byte-stuffing ok / fcs16 0x906E,
check and flag refusals ok / XRCE serial frame 59 bytes byte-identical to
crates/xrce / serial round-trip ok. Für diese Klausel gibt es keinen Live-E2E-Fall:
der Rust-Peer spricht UDP, keinen seriellen Link. Die Referenz, gegen die das
Golden byte-genau steht:
`crates/xrce/src/transport_serial.rs::tests::xrce_annex_c_1_1_6_the_frame_check_is_the_fcs16_of_rfc_1662`,
`crates/xrce/src/transport_serial.rs::tests::xrce_annex_c_1_1_a_frame_carries_the_addresses_the_length_and_a_little_endian_check`,
`crates/xrce/src/transport_serial.rs::tests::xrce_annex_c_1_2_flag_and_escape_octets_are_stuffed_everywhere_after_the_flag`,
`crates/xrce/src/transport_serial.rs::tests::xrce_annex_c_1_3_a_frame_with_a_wrong_check_is_refused`,
`crates/xrce/src/transport_serial.rs::tests::xrce_annex_c_1_3_a_begin_flag_inside_a_frame_drops_it_and_begins_the_next`;
`endpoints/golden-gen/src/main.rs` erzeugt `golden_serial_le.bin` mit
`encode_frame(1, 0, …)` aus dieser Implementierung.

**Status:** done

## §2 sync send/recv

### §2.1 Frame-Hook-Vertrag (`zdw_transport`)

**Spec:** §2.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, ADR 0013 Invariante 5)
— der Endpoint ist transport-opak: er reicht ein vollständig gerahmtes,
encodiertes Message an den vom Integrator gefüllten Transport (`ctx` plus
Funktionspointer für Deliver und Receive) und erhält vollständige Frames
zurück. Der Endpoint öffnet selbst keinen Socket.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (Struktur `zdw_transport`,
`zdw_endpoint_send`, `zdw_endpoint_recv`).

**Tests:** C-Seite — `endpoints/c/test/test_endpoint_loopback.c` mit einem
In-Memory-Loopback-Transport (Encode, Send, Receive, Decode, Feldvergleich;
Makefile-Lauf `make -C endpoints/c test`). Gemessene Ausgabe dieses Audits:
frame-hook 40-byte frame delivered + received + decoded ok. Rust-Seite —
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_sync_app_writes_the_ping_and_reads_the_pong` steckt in denselben
Frame-Hook einen echten POSIX-UDP-Socket und belegt damit, dass der Vertrag
nicht nur gegen den Test-Stub trägt.

**Status:** done

### §2.2 Poll-Loop-Idiom (C89)

**Spec:** §2.2 (`docs/specs/zerodds-endpoint-c-1.0.md`) — sync ist die
C89-Baseline-Nutzung des Frame-Hooks: der Integrator besitzt den Run-Loop und
ruft `zdw_endpoint_recv` selbst. Es MUSS ein lauffähiges Beispiel geben, kein
Stub.

**Repo:** `endpoints/c/examples/example_sync.c` (C89-Poll-Loop über den
Transport, übersetzt mit `-std=c89 -pedantic -Wall -Wextra`).

**Tests:** C-Seite — der Beispiel-Lauf (`make -C endpoints/c examples`, Ziel `example_sync`).
Gemessene Ausgabe dieses Audits: fünf dekodierte `Reading`-Samples
(id 0x1000 bis 0x1004) und ALL OK, 0 Compiler-Warnungen. Rust-Seite —
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_sync_app_writes_the_ping_and_reads_the_pong` fährt dasselbe Idiom in
einem eigens gebauten Programm live gegen den Rust-Peer.

**Status:** done

## §3 async Reader/Writer

### §3.1 Event-driven Reactor (`zdw_async_reader` / `zdw_async_writer`)

**Spec:** §3.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, ADR 0013) — additiv
zum C89-Kern eine latenz-entkoppelte, callback-getriebene Alternative zum
Poll-Loop (C11, kein `malloc`).

**Repo:** `endpoints/c/include/zerodds_async.h` /
`endpoints/c/src/zerodds_async.c` (`zdw_async_reader_init` bindet einen
`on_sample`-Callback, `zdw_async_run` drained den Transport und dispatcht
jedes dekodierte Sample; dazu `zdw_async_writer_init`, `zdw_async_write` und `zdw_async_poll`).

**Tests:** C-Seite — `endpoints/c/test/test_async_loopback.c` über einen
In-Memory-FIFO-Transport (Makefile-Lauf `make -C endpoints/c test`). Gemessene
Ausgabe dieses Audits: async loopback 5 samples dispatched + decoded in
order. Rust-Seite — `crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_async_app_writes_the_ping_and_reads_the_pong`
(`zdw_async_write` rahmt und liefert, `zdw_async_poll` entpackt und ruft den
Callback, live über UDP).

**Status:** done

### §3.2 Live-UDP-Reactor

**Spec:** §3.2 (`docs/specs/zerodds-endpoint-c-1.0.md`) — Nachweis, dass der
Reactor echte Datagramm-I/O treibt und nicht nur eine In-Memory-Queue.

**Repo:** dieselben Reactor-Funktionen aus `endpoints/c/src/zerodds_async.c`,
angebunden an einen nicht-blockierenden POSIX-UDP-Socket in
`endpoints/c/test/test_async_udp.c`.

**Tests:** C-Seite — `endpoints/c/test/test_async_udp.c` (Makefile-Lauf `make -C endpoints/c test`). Gemessene Ausgabe dieses Audits: async UDP 5/5
samples received + decoded via reactor. Rust-Seite —
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_async_app_writes_the_ping_and_reads_the_pong` (derselbe Reactor gegen
einen fremden Prozess statt gegen einen zweiten Socket im selben Programm),
`crates/endpoint-e2e/tests/c.rs::endpoint_c_3_4_the_generated_codec_pings_without_the_xrce_frame` (die Gegenprobe ohne
XRCE-Hülle).

**Status:** done

### §3.3 Async-Deep-Example

**Spec:** §3.3 (`docs/specs/zerodds-endpoint-c-1.0.md`) — lauffähiges
Beispiel für den Reactor, kein Stub.

**Repo:** `endpoints/c/examples/example_async.c` (C11-Reactor plus Callback,
übersetzt mit `-std=c11 -pedantic -Wall -Wextra`).

**Tests:** C-Seite — der Beispiel-Lauf (`make -C endpoints/c examples`, Ziel `example_async`).
Gemessene Ausgabe dieses Audits: fünf dekodierte Readings (id 0x2000 bis
0x2004) und ALL OK, 0 Compiler-Warnungen. Rust-Seite —
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_async_app_writes_the_ping_and_reads_the_pong` belegt denselben
Callback-Pfad gegen einen echten Peer.

**Status:** done

### §3.4 Test-Pflicht: Live-Ping-Pong-E2E

**Spec:** §3.4 (`docs/specs/zerodds-endpoint-c-1.0.md`) — zusätzlich zu den
isolierten Unit- und Loopback-Tests aus §1 bis §3 MUSS jede Sprache eine
Live-E2E-Datei nach dem Muster `crates/endpoint-e2e/tests/<lang>.rs`
bereitstellen (Referenz: `crates/endpoint-e2e/tests/cpp.rs`), damit Framing
(§1), sync (§2) und async (§3) im Zusammenspiel belegt sind und nicht nur
einzeln.

**Repo:** `crates/endpoint-e2e/tests/c.rs` — drei Fälle über die
C-Endpoint-Oberfläche: der generierte Codec direkt auf einem POSIX-UDP-Socket
ohne XRCE-Hülle; dieselbe Probe in einem XRCE-WRITE_DATA über
`zdw_endpoint_send` und `zdw_endpoint_recv`; die ereignisgetriebene
Oberfläche über `zdw_async_write` und `zdw_async_poll`. Das Wire-Runtime
`runtime/c` wird über sein eigenes Makefile zur statischen Bibliothek
gebaut, weil dessen `CFLAGS` Voraussetzung sind; der Include-Pfad führt
`runtime/c/include` vor `endpoints/c/include`, weil `runtime/c/include/zerodds_wire.h` und
`endpoints/c/include/zerodds_wire.h` denselben Include-Guard führen.

**Tests:** Rust-Seite, Job `integration-bindings` — `crates/endpoint-e2e/tests/c.rs::endpoint_c_3_4_the_generated_codec_pings_without_the_xrce_frame`,
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_sync_app_writes_the_ping_and_reads_the_pong`,
`crates/endpoint-e2e/tests/c.rs::xrce_8_3_5_8_c_async_app_writes_the_ping_and_reads_the_pong`.
Ohne C-Compiler auf dem `PATH` kehren die drei Fälle mit einer `SKIP`-Zeile
zurück, an der der Job scheitert. Der Rust-Peer prüft die
exakten 23 Oktetts des `Ping` und das Programm die Antwortzeile mit
`reply=pong:hello from app`.

**Status:** done

## §4 reliable Stream

### §4.1 C89-State-Machine (`zdw_reliable`)

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §3 — reliabler Sender und
Receiver, gespiegelt von `crates/xrce/src/reliable.rs`; fester Storage, kein
`malloc`: Fenster 16, Empfangspuffer 64, RFC-1982-16-Bit-Sequenznummern.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (Deklaration) und
`endpoints/c/src/zerodds_endpoint.c` (Implementierung in C89) — Sender-Seite
`zdw_reliable_submit`, `zdw_reliable_pending_heartbeat`,
`zdw_reliable_recv_acknack`, `zdw_reliable_get_in_flight`; Receiver-Seite
`zdw_reliable_recv_data`, `zdw_reliable_drain`,
`zdw_reliable_pending_acknack`; dazu `zdw_reliable_reset`.

**Tests:** C-Seite — `endpoints/c/test/test_reliable_sm.c`, (Makefile-Lauf `make -C endpoints/c test`): 14 Prüf-Funktionen, davon 13 als Spiegel der
Referenztests und eine als Byte-Golden-Check. Gemessene Ausgabe dieses
Audits: test_reliable_sm ALL OK. Die gespiegelte Referenz ist
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_submit_rejects_when_window_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_12_pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_1_pending_heartbeat_silenced_until_period_elapsed`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::reliable_3_2_recv_acknack_full_clear_when_no_bits_set`,
`crates/xrce/src/reliable.rs::tests::xrce_8_4_14_2_recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_reorders_out_of_order`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_2_3_recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::reliable_3_1_recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_13_reset_clears_state_completely`,
`crates/xrce/src/reliable.rs::tests::xrce_8_3_5_11_end_to_end_sender_receiver_with_loss_recovery`.

**Status:** done

### §4.2 Byte-golden HEARTBEAT und ACKNACK

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §4 — ACKNACK trägt
`first_unacked_seq_num` als `i16`, `nack_bitmap` als zwei Bytes
little-endian und `stream_id` als ein Byte; HEARTBEAT trägt
`first_unacked_seq_nr` und `last_unacked_seq_nr` als `i16` plus `stream_id`.

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (die Codec-Hälften von
`zdw_reliable_pending_heartbeat` und `zdw_reliable_pending_acknack`), geprüft
von der Byte-Golden-Funktion in `endpoints/c/test/test_reliable_sm.c` und vom
reinen Wire-Test `endpoints/c/test/test_reliable.c`.

**Tests:** C-Seite — `endpoints/c/test/test_reliable.c` und
`endpoints/c/test/test_reliable_sm.c` gegen `golden_heartbeat_le.bin` und
`golden_acknack_le.bin` (Makefile-Lauf `make -C endpoints/c test`). Gemessene
Ausgabe dieses Audits: HEARTBEAT parsed first=1 last=3 stream=0x80 / ACKNACK
13 bytes byte-identical to crates/xrce. Die Referenz, aus der beide Goldens
entstehen: `crates/xrce/src/submessages/heartbeat.rs::tests::xrce_8_3_5_12_heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::xrce_8_3_5_12_heartbeat_roundtrip_via_submessage`,
`crates/xrce/src/submessages/acknack.rs::tests::xrce_8_3_5_11_acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::xrce_8_3_5_11_acknack_roundtrip_via_submessage`.

**Status:** done

### §4.3 pthread-SPSC-Ring als async Writer (Latenz-Entkopplung)

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §2 — der Producer darf nie in
den Kernel eintreten; das Bauteil trägt zugleich den reliable Sender-State.

**Repo:** `endpoints/c/include/zerodds_reliable_async.h` /
`endpoints/c/src/zerodds_reliable_async.c` (C11 plus pthread):
`zdw_async_ring_start` startet den Drain-Thread, `zdw_async_ring_enqueue` ist
ein wait-freies Enqueue ohne Syscall, der Drain-Thread hält den
`zdw_reliable`-State und übernimmt Framing und I/O.

**Tests:** C-Seite — `endpoints/c/test/test_reliable_async_overflow.c`,
(Makefile-Lauf `make -C endpoints/c test`): prüft das Verhalten bei vollem
Sende-Fenster und dass der Drain-Thread sich nicht selbst bestätigt.
Gemessene Ausgabe dieses Audits: ALL OK, delivered 24/24, no self-ack, no
hang. Dazu `endpoints/c/test/bench_reliable_async.c` als Latenzvergleich
(siehe §4.6). Rust-Seite —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send` baut und
fährt denselben Ring im Job `integration-bindings`.

**Status:** done

### §4.4 Loss-Recovery-Example

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 5 — lauffähiges
Beispiel, kein Stub: N Samples, injizierter Verlust, lückenlose Zustellung
nach Retransmit.

**Repo:** `endpoints/c/examples/example_reliable.c` — 12 Samples submitted,
jedes vierte beim ersten Durchlauf verworfen, Recovery-Runden bis alle 12 in
der Folge 0 bis 11 angekommen sind.

**Tests:** C-Seite — der Beispiel-Lauf (`make -C endpoints/c examples`, Ziel
`example_reliable`; zusätzlich im Makefile-Ziel `test`). Gemessene Ausgabe dieses Audits: 3/12 delivered
before recovery, delivered contiguous 0..11 expected=12, ALL 12 samples
recovered gap-free. Rust-Seite —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams` belegt
denselben Ablauf live über UDP gegen einen fremden Prozess statt in-process.

**Status:** done

### §4.5 Live-E2E gegen den Rust-Referenz-Peer

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 3 — live gegen den
Rust reliable Peer, `stream_id >= 128`, mit injiziertem Drop; Assert: alle
Samples lückenlos und in Reihenfolge zugestellt trotz Verlust.

**Repo:** `endpoints/c/include/zerodds_reliable_async.h` (`zdw_async_ring`) —
der Test baut die C-App aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und dem entkoppelten Writer: der Producer
reiht die kodierten Samples ein, der Drain-Thread sendet WRITE_DATA und
HEARTBEAT und retransmittiert auf ACKNACK, gegen den geteilten Rust-Peer aus
`crates/endpoint-e2e/src/lib.rs`, der den Verlust injiziert.

**Tests:** Rust-Seite, Job `integration-bindings` —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams`
(12 Samples, injizierter Drop, Assert auf Lückenlosigkeit; der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`); die
Samples, die nicht verworfen werden, sind der Lauf ohne Verlust). Auch hier gilt die
Compiler-Vorprüfung aus dem Kopf dieses Dokuments.

**Status:** done

### §4.6 Latenz: inline Send gegen entkoppeltes Ring-Enqueue

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) in Verbindung mit
`docs/spec-coverage/reliable-endpoint-1.0.md` §5 Punkt 4 — die
Producer-Latenz von `write` bis `return` im entkoppelten Pfad MUSS gegen den
inline Deliver gemessen werden; ein Messwert, der die Entkopplung zeigt.

**Repo:** `endpoints/c/test/bench_reliable_async.c` — misst
`inline_ns_per_op` (Framing plus `send()`-Syscall pro Sample gegen einen
gebundenen, aber ungelesenen Loopback-Socket) gegen
`decoupled_enqueue_ns_per_op` (reines Enqueue in den Ring, kein Kernel). Die
Datei trug dieselbe `INADDR_LOOPBACK`-Ursache wie §4.5 und deklariert
deshalb ebenfalls `_DARWIN_C_SOURCE 1`.

**Tests:** Rust-Seite, Job `integration-bindings` —
`crates/endpoint-e2e/tests/c_reliable.rs::reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send` (baut das
Bench-Programm mit `gcc -O2 -std=c11`, fährt es und lässt den Fall
scheitern, wenn die Latenzzeile fehlt oder das entkoppelte Enqueue nicht
unter dem inline Send liegt). C-Seite — dasselbe Programm im Makefile-Ziel
(`test`). Gemessen in diesem Audit auf macOS 15 (Apple-Clang):
inline_ns_per_op=2585 gegen decoupled_enqueue_ns_per_op=13 über 5000
zugestellte Samples. Der absolute Wert für die inline-Zahl schwankt mit der
Host-Last; der Assert prüft die Ordnung, nicht den Betrag.

**Status:** done

---

## Audit-Status

15 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-c-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-xrce --all-targets` (Job `merge-rust-xrce`) — 27 Tests grün.
* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 5 Tests grün.

32 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
