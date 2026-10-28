# ZeroDDS-TCP-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. Für TCP normiert OMG nur
den Locator-Kind (DDSI-RTPS 2.5 §9.4.5) und das Wire-Mapping (§9.5); das
Stream-Framing und der Connection-Bring-up sind vendor-spezifisch. Die
normativen Anforderungen dieses Dokuments stehen in den `##`-Sektionen §2
bis §6; jedes `###`-Item darunter trägt Spec-Zitat, Repo-Anker, Test und
Status.

Implementation: `crates/transport-tcp/` — Length-Prefix-Framing
(`framing.rs`), 16-Byte-Handshake (`handshake.rs`), Connection-Pool
(`tcp_transport.rs`).

Offene Punkte und Decision-Records: [`zerodds-tcp-transport-1.0.open.md`](zerodds-tcp-transport-1.0.open.md).

## §1 Scope und Spec-Status

### §1.1 Locator-Kind TCPv4 / TCPv6

**Spec:** DDSI-RTPS 2.5 §9.4.5 — `LOCATOR_KIND_TCPv4 = 4`,
`LOCATOR_KIND_TCPv6 = 8`. Der Transport MUSS genau diese Werte führen.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Tcpv4 = 0x01F0_0004`,
`LocatorKind::Tcpv6 = 0x01F0_0005`, `Locator::tcp_v4`, `Locator::tcp_v6`);
gebunden in `crates/transport-tcp/src/tcp_transport.rs`
(`locator_to_socket`, `TcpTransport::local_locator`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::locator_kinds_are_zerodds_vendor_range_values`
(prüft die beiden Zahlwerte `0x01F0_0004` und `0x01F0_0005` direkt),
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_to_socket_v4_maps_tcp`,
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_to_socket_v4_rejects_udp`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** partial — der Transport führt die Werte des Herstellerbereichs `0x01F0_0004` und `0x01F0_0005` (DDSI-RTPS 2.5 §9.3.2.4 reserviert 3 bis `0x01ffffff` für Hersteller); die Werte 4 und 8 der Spec-Zeile schreibt `set_tcp_kinds_fast_dds` (Fast DDS, DDS-TCP-PSM), gelesen werden beide. Rest: die Werte 4 und 8 sind nicht der Vorgabewert.

### §1.2 RTPS-Wire-Mapping über den Stream

**Spec:** DDSI-RTPS 2.5 §9.5 — der Frame-Body ist ein vollständiges
RTPS-Message (RTPS-Header + Submessages), byte-identisch zum UDP-PSM. Der
TCP-Transport MUSS ihn unverändert durchreichen und darf ihn nicht
umschreiben, zusammenfassen oder aufteilen.

**Repo:** `crates/transport-tcp/src/framing.rs` (`write_frame`,
`read_frame` — der Payload wird ohne Interpretation kopiert);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` reicht jeden
gelesenen Frame als `ReceivedDatagram` weiter).

**Tests:** `crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(zwei Frames über eine echte TCP-Verbindung, byte-gleich und in
Reihenfolge zurück),
`crates/transport-tcp/tests/loopback.rs::concurrent_senders_to_one_server`
(100 Frames aus zwei Prozess-Threads, kein Verlust),
`crates/transport-tcp/src/framing.rs::tests::roundtrip_small_frame`.

**Status:** done

### §1.3 Was OMG für TCP nicht normiert

**Spec:** DDSI-RTPS 2.5 kennt keinen TCP-Connection-Bring-up, kein
Logical-Port-Multiplexing und keine Connection-Pool-Policy. Cyclone DDS
(`ddsi_tcp`) sendet rohe RTPS-Frames ohne Handshake, FastDDS nutzt
BindConnectionRequest/Response als Submessages `0x71`/`0x72`, RTI einen
TLS-orientierten Handshake. Die Sektion hält den Scope fest und stellt
keine Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 Wire-Format

Jedes RTPS-Datagramm wird über TCP wie folgt gerahmt:

```text
+---------+---------+---------+---------+
| length (u32, big-endian)              |   Frame-Länge in Bytes (ohne Header)
+---------+---------+---------+---------+
| RTPS-Frame-Body (length Bytes)        |   wie beim UDP-PSM
+---------+---------+---------+---------+
```

### §2.1 Length-Prefix-Frame (4 Byte big-endian)

**Spec:** §2 — der Sender MUSS jedem Frame eine 4-Byte-Länge in
Big-Endian voranstellen, die den Body ohne den Header selbst zählt. Ein
leerer Body ist zulässig (`length = 0`). Aufeinanderfolgende Frames
stehen ohne Trenner hintereinander.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`write_frame`, `read_frame`, `FRAME_HEADER_LEN = 4`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::roundtrip_small_frame`
(prüft die vier Header-Bytes gegen `5u32.to_be_bytes()`),
`crates/transport-tcp/src/framing.rs::tests::roundtrip_empty_frame`,
`crates/transport-tcp/src/framing.rs::tests::two_frames_back_to_back`.

**Status:** done

### §2.2 Reassembly über Write-Grenzen hinweg

**Spec:** §2 — TCP ist ein Stream: der Empfänger MUSS einen Frame
zusammensetzen, der beim Sender über mehrere `write`-Aufrufe verteilt
wurde, und darf ihn weder verkürzt noch als zwei Frames weiterreichen.
Das gilt auch, wenn schon das Längenfeld gesplittet ankommt.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_exact_or_eof` — `read_exact` füllt Header und Body vollständig,
unabhängig davon, wie viele `read`-Aufrufe das kostet).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::reassembles_message_split_across_two_writes`
(Längen-Präfix plus erste Hälfte im ersten Write, Rest plus Folgeframe im
zweiten; der Reader liefert genau eine Nachricht und danach den
Folgeframe),
`crates/transport-tcp/src/framing.rs::tests::reassembles_frame_split_across_reads`
(4096-Byte-Payload bei Chunk-Größen 1, 3, 4, 5 und 4095),
`crates/transport-tcp/tests/loopback.rs::frame_split_across_two_writes_is_reassembled`
(dieselbe Teilung über einen echten TCP-Socket, mit 80 ms Pause zwischen
den beiden Writes; der Server liefert genau einen Frame).

**Status:** done

### §2.3 Maximum-Frame-Size

**Spec:** §2 — der Empfänger MUSS eine angekündigte Länge oberhalb des
Limits ablehnen, statt den Puffer zu allokieren (DoS-Schutz). Limit: 16
MiB. Größere Samples werden auf RTPS-Ebene per `DATA_FRAG`
(DDSI-RTPS §8.3.7) zerlegt, nicht auf Transport-Ebene.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`MAX_FRAME_SIZE = 16 * 1024 * 1024`, `FramingError::FrameTooLarge`);
Sendepfad in `crates/transport-tcp/src/tcp_transport.rs`
(`SendError::PayloadTooLarge`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::rejects_oversized_announcement`
(`MAX_FRAME_SIZE + 1` wird abgelehnt, bevor gelesen wird),
`crates/transport-tcp/src/tcp_transport.rs::tests::send_oversized_payload_returns_payload_too_large`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_frame_too_large_errors`.

**Status:** done

### §2.4 Abbruch bei EOF mitten im Frame

**Spec:** §2 — schließt der Peer die Verbindung mitten in Header oder
Body, MUSS der Empfänger das als `UnexpectedEof` melden und darf keinen
verkürzten Frame nach oben geben. Ein EOF genau auf einer Frame-Grenze
ist dagegen der reguläre Verbindungsabbau.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`FramingError::UnexpectedEof`, `read_exact_or_eof`);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` beendet die
Frame-Pumpe bei `UnexpectedEof` mit `Ok(())`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::unexpected_eof_mid_header`,
`crates/transport-tcp/src/framing.rs::tests::unexpected_eof_mid_payload`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_reads_one_frame_then_eof`.

**Status:** done

## §3 Handshake (ZeroDDS-spezifisch)

```text
Client                                    Server
  |-- BindConnectionRequest  (16 B) ------->|   §3.1
  |<-- BindConnectionResponse (16 B) -------|   §3.2
  |-- Length-Prefix-Frame [RTPS] ---------->|   §2.1
```

### §3.1 BindConnectionRequest — Wire-Layout

**Spec:** §3 — 16 Byte big-endian: Magic `"ZDDS"` (4), `v_major` (1),
`v_minor` (1), `vendor_id` (2), `flags` (4, reserviert), `logical_port`
(4). ZeroDDS-Version ist `(1,0)`, die VendorId `[0x01, 0x0F]`. Ein
falsches Magic MUSS die Verbindung scheitern lassen.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionRequest`, `encode_request`, `decode_request`,
`HANDSHAKE_MAGIC_REQUEST`, `HANDSHAKE_WIRE_SIZE = 16`,
`VENDOR_ID_ZERODDS`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::request_wire_layout_is_sixteen_bytes_big_endian`
(vergleicht alle 16 Bytes gegen das Literal des Diagramms, inklusive
`logical_port = 7400` als `00 00 1C E8`),
`crates/transport-tcp/src/handshake.rs::tests::request_roundtrip`,
`crates/transport-tcp/src/handshake.rs::tests::request_rejects_wrong_magic`.

**Status:** done

### §3.2 BindConnectionResponse — Wire-Layout und Status-Byte

**Spec:** §3 — 16 Byte big-endian: Magic-Prefix `"ZDA"` plus
Status-Byte (`0x2B` = `'+'` Accept, `0x2D` = `'-'` Reject), `v_major`,
`v_minor`, `vendor_id`, `flags`, `reason_code` (4, big-endian; `0` bei
Accept). Ein anderes Magic MUSS als `BadResponse` scheitern.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionResponse`, `ResponseStatus`, `encode_response`,
`decode_response`, `HANDSHAKE_MAGIC_ACCEPT`, `HANDSHAKE_MAGIC_REJECT`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::response_wire_layout_is_sixteen_bytes_big_endian`
(alle 16 Bytes für Accept und für Reject mit `reason_code = 3`),
`crates/transport-tcp/src/handshake.rs::tests::response_roundtrip_accept`,
`crates/transport-tcp/src/handshake.rs::tests::response_roundtrip_reject`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_errors_on_bad_response_magic`.

**Status:** done

### §3.3 Reject-Reason-Codes

**Spec:** §3 — fünf stabile Wire-Codes: `0 Unknown`,
`1 VersionMismatch`, `2 ResourceLimit`, `3 LogicalPortConflict`,
`4 VendorNotAccepted`. Ein unbekannter Code MUSS auf `Unknown` abgebildet
werden, damit ein neuerer Peer den Reject trotzdem verarbeiten kann.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`RejectReason`, `RejectReason::from_code`, `RejectReason::as_code`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::reject_reason_roundtrip_known_codes`,
`crates/transport-tcp/src/handshake.rs::tests::reject_reason_unknown_code_maps_to_unknown`,
`crates/transport-tcp/src/handshake.rs::tests::server_rejects_version_mismatch`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_resource_limit`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_logical_port_conflict`,
`crates/transport-tcp/src/handshake.rs::tests::client_handshake_rejects_on_vendor_not_accepted`.

**Status:** partial — Codec und Client-Verarbeitung sind für alle fünf
Codes vorhanden und getestet. `server_handshake` erzeugt jedoch nur
`VersionMismatch`; `ResourceLimit`, `LogicalPortConflict` und
`VendorNotAccepted` kommen nie aus einem ZeroDDS-Server auf die Leitung.
Die drei fehlenden Sender-Pfade sind als §3.5, §3.6 und §3.7 getrennt
geführt.

### §3.4 Handshake-Sequenz und Pflicht vor dem ersten Frame

**Spec:** §3 — der Client MUSS den Request vor dem ersten
Length-Prefix-Frame senden, der Server MUSS die Response vor dem ersten
Frame beantworten. Ein Peer ohne Handshake MUSS abgewiesen werden; bei
Reject droppen beide Seiten die Verbindung.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`client_handshake`,
`server_handshake`); `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected` ruft `client_handshake` direkt nach
`connect`; `accept_one` ruft `server_handshake` vor der Frame-Pumpe).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::full_roundtrip_accept_via_paired_cursors`,
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_rejects_peer_without_handshake`
(HTTP-Probe wird mit `TcpTransportError::Handshake` abgewiesen),
`crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(Handshake und Frames über eine echte Verbindung).

**Status:** done

### §3.5 Reserved `flags` — Empfänger-Prüfung

**Spec:** §3 — das `flags`-Feld ist reserviert und MUSS `0` sein. Ein
Empfänger, der ein Feld mit gesetzten Bits akzeptiert, verbaut die
spätere Belegung dieser Bits: ein alter Peer würde ein neues Flag still
ignorieren statt die Verbindung abzulehnen.

**Repo:** —

**Tests:** —

**Status:** open

### §3.6 Logical-Port-Claim

**Spec:** §3 — `logical_port` ist der DDS-Endpoint-Port, den der Client
auf dieser Verbindung beansprucht; `0` heißt „kein Claim". Der Server
MUSS einen bereits gebundenen Port mit `LogicalPortConflict` ablehnen und
die Verbindung sonst dem beanspruchten Port zuordnen.

**Repo:** —

**Tests:** —

**Status:** open

### §3.7 ResourceLimit-Reject bei erreichtem Peer-Limit

**Spec:** §3 — hat der Server sein Peer-Limit erreicht, MUSS er den
Handshake mit `ResourceLimit` ablehnen, statt die Verbindung anzunehmen
und danach zu verwerfen. Nur so kann der Client den Backoff vom
Verbindungsfehler unterscheiden.

**Repo:** —

**Tests:** —

**Status:** open

### §3.8 Zeitbudget im Accept-Pfad

**Spec:** §3 — der Accept-Pfad MUSS ein Gesamtbudget für den Handshake
führen, nicht nur ein Timeout pro Syscall. Ein byte-weise trödelnder Peer
würde sonst `16 × slice` Sekunden des Accept-Threads binden.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs` (`accept_one`:
`handshake_total_budget` = 2 s, `handshake_syscall_slice` = 200 ms,
Deadline-Prüfung nach `server_handshake`; nach Accept werden die Timeouts
wieder aufgehoben).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_gives_up_on_a_stalling_handshake_peer`
(Peer sendet ein Handshake-Byte und schweigt, hält die Verbindung aber
offen; `accept_one` kehrt mit `Handshake(_)` in unter 3 s zurück),
`crates/transport-tcp/src/tcp_transport.rs::tests::accept_one_rejects_peer_without_handshake`.

**Status:** done

## §4 Cyclone-`ddsi_tcp`-Kompatibilität

### §4.1 Handshake-Skip-Mode

**Spec:** §4 — Cyclone DDS spricht rohe RTPS-Frames mit Length-Prefix
über TCP, ohne Handshake. Für Interop MUSS eine Client-Variante existieren,
die §3 überspringt und direkt §2.1-Frames sendet, und eine Server-Seite,
die solche Verbindungen annimmt.

**Repo:** —

**Tests:** —

**Status:** open

### §4.2 Auto-Detect am ersten Frame-Byte

**Spec:** §4 — die Server-Seite MUSS den Cyclone-Modus am ersten Byte
erkennen: `'Z'` ist der ZeroDDS-Handshake, `'R'` das erste Byte des
RTPS-Magic. Ohne Erkennung müsste ein Deployment die Betriebsart pro Port
vorab konfigurieren.

**Repo:** —

**Tests:** —

**Status:** open

## §5 Connection-Pool

### §5.1 Connection-Reuse und Lazy Connect

**Spec:** §5 — pro Ziel-Socket-Adresse hält der Transport genau eine
ausgehende Verbindung, die beim ersten `send` aufgebaut und danach
wiederverwendet wird. Der Pool-Lock darf nicht über die blockierende I/O
gehalten werden.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected`, `TcpTransport::send` — Schritt 1 nimmt den
Pool-Lock nur für Lookup/Insert, Schritt 2 den Peer-Lock für die I/O).

**Tests:** `crates/transport-tcp/tests/loopback.rs::loopback_single_frame_end_to_end`
(zwei Sends über dieselbe wiederverwendete Verbindung),
`crates/transport-tcp/tests/loopback.rs::reconnect_after_peer_restart`,
`crates/transport-tcp/src/tcp_transport.rs::tests::send_unsupported_locator_does_not_pollute_pool`.

**Status:** done

### §5.2 Exponentieller Backoff nach Verbindungsfehler

**Spec:** §5 — nach einem fehlgeschlagenen Connect MUSS der nächste
Versuch gedrosselt werden (50 ms, verdoppelnd, Deckel 5 s), damit ein
toter Peer keine Connect-Schleife auslöst. Ein erfolgreicher Connect
setzt den Backoff zurück.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::bump_backoff`, `INITIAL_BACKOFF_MS = 50`,
`MAX_BACKOFF_MS = 5_000`, Rücksetzung in `ensure_connected`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::peer_conn_bump_backoff_sequence`
(50 → 100 → … → 5000, dann konstant),
`crates/transport-tcp/src/tcp_transport.rs::tests::peer_conn_backoff_throttles_connect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::backoff_resets_after_successful_connect`,
`crates/transport-tcp/tests/loopback.rs::backoff_prevents_tight_loop_on_dead_peer`
(zweiter Send an einen toten Port kehrt in unter 20 ms zurück).

**Status:** done

### §5.3 Peer-Cap mit deterministischer Eviction

**Spec:** §5 — der Pool ist auf `MAX_PEERS = 256` gedeckelt. Ist er voll
und kommt ein neues Ziel dazu, wird deterministisch der kleinste
Schlüssel verdrängt; ein bereits enthaltenes Ziel löst keine Eviction aus.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_PEERS`, Eviction-Zweig in `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::max_peers_eviction_removes_first_key_on_new_peer`,
`crates/transport-tcp/src/tcp_transport.rs::tests::max_peers_no_eviction_for_existing_peer`.

**Status:** done

### §5.4 Begrenzte Inbound-Queue mit gezähltem Drop

**Spec:** §5 — die Empfangs-Queue ist auf `MAX_INBOUND_QUEUE = 1024`
gedeckelt. Läuft sie über, wird der älteste Frame verworfen **und
gezählt**; ein stiller Verlust ist nicht zulässig. `recv` blockiert per
Condvar und wacht beim Push auf, ohne Poll-Schleife.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_INBOUND_QUEUE`, `push_inbound`, `dropped_frames`,
`Transport::recv` mit `Condvar::wait`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::inbound_overflow_drops_oldest_and_counts`,
`crates/transport-tcp/src/tcp_transport.rs::tests::dropped_frames_zero_on_fresh_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::recv_wakes_on_push_from_other_thread`,
`crates/transport-tcp/src/tcp_transport.rs::tests::push_preserves_fifo_order`.

**Status:** done

## §6 Cross-Vendor-Erweiterungspunkte

### §6.1 FastDDS-BindConnection (Submessages 0x71/0x72)

**Spec:** §6 — FastDDS trägt Request und Response als
RTPS-Submessages `0x71` und `0x72`. Für Interop mit FastDDS-TCPv4 MUSS
dieses Format als zusätzlicher Handshake-Dialekt wählbar sein; der
ZeroDDS-eigene Handshake aus §3 bleibt Default.

**Repo:** —

**Tests:** —

**Status:** open

### §6.2 RTI-TLS-orientierter Handshake

**Spec:** §6 — RTI DDS TCP nutzt einen TLS-orientierten Bring-up. Für
Interop mit RTI MUSS dieser Dialekt als zusätzliche Betriebsart wählbar
sein; der ZeroDDS-eigene Handshake aus §3 bleibt Default.

**Repo:** —

**Tests:** —

**Status:** open

---

## Audit-Status

13 done / 2 partial / 7 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-transport-tcp` — 58 Tests (lib) + 6
Tests (`tests/loopback.rs`) grün, 0 failed. Gemessen auf macOS 15
(Apple-Clang-Toolchain, Rust-Host `aarch64-apple-darwin`).

Offene Punkte und Decision-Records: [`zerodds-tcp-transport-1.0.open.md`](zerodds-tcp-transport-1.0.open.md).
