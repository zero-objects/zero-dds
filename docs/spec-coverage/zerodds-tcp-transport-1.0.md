# ZeroDDS-TCP-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. DDSI-RTPS 2.5 legt
keine Abbildung auf TCP fest: §9.3.2.1 nennt als Locator-Arten nur UDPv4
(1) und UDPv6 (2), §9.5 bildet RTPS-Nachrichten auf UDP/IP ab, und
§9.3.2.4 überlässt die Locator-Arten 3 bis `0x01ffffff` den Herstellern.
§8.3.3 hält fest, dass ein Stream-Transport wie TCP die Nachrichtenlänge
in der RTPS-HeaderExtension bräuchte. Locator-Art, Stream-Framing und
Verbindungsaufbau dieses Transports sind ZeroDDS-eigen. Die normativen
Anforderungen dieses Dokuments stehen in den `##`-Sektionen §1 bis §6;
jedes `###`-Item darunter trägt Spec-Zitat, Repo-Anker, Test und Status.

Implementation: `crates/transport-tcp/` — Length-Prefix-Framing
(`framing.rs`), 16-Byte-Handshake (`handshake.rs`), Stream-Framing über
die HeaderExtension (`rtps_stream.rs`), der Stream von Cyclone DDS
(`cyclone_stream.rs`), das TCP-Protokoll von Fast DDS (`rtcp.rs`),
Connection-Pool (`tcp_transport.rs`).

Offene Punkte: `zerodds-tcp-transport-1.0.open.md`.

## §1 Scope und Spec-Status

### §1.1 Locator-Kind TCPv4 / TCPv6

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — die Spec nennt keine Locator-Art für
TCP (§9.3.2.1: `LOCATOR_KIND_UDPv4 = 1`, `LOCATOR_KIND_UDPv6 = 2`) und
reserviert 3 bis `0x01ffffff` für Hersteller. Ein Locator des eigenen
Dialekts (§2, §3) MUSS `0x01F0_0004` (TCPv4) und `0x01F0_0005` (TCPv6)
tragen. Die Werte 4 und 8 MÜSSEN je Transport und je Runtime wählbar sein,
nicht prozessweit; Cyclone DDS schreibt sie (`ddsi_protocol.h`:
`DDSI_LOCATOR_KIND_TCPv4 = 4`, `DDSI_LOCATOR_KIND_TCPv6 = 8`), für Fast DDS
ist das nicht an dessen Quelltext geprüft. Ein Locator, der für den Dialekt
eines anderen Herstellers angekündigt wird (§4.1, §6.1, §6.2), MUSS dessen
Art und Port-Layout tragen. Gelesen werden beide Wertepaare; ein gelesener
Locator behält die Art, mit der er geschrieben wurde, und ein Ziel beider
Wertepaare wird über denselben Transport erreicht.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Tcpv4 = 0x01F0_0004`,
`LocatorKind::Tcpv6 = 0x01F0_0005`, `LocatorKind::Tcpv4Kind4 = 4`,
`LocatorKind::Tcpv6Kind8 = 8`, `TcpKindSet`, `LocatorKind::tcp_v4`,
`tcp_v6`, `is_tcp_v4`, `is_tcp_v6`, `tcp_kind_set`, `transport_kind`,
`Locator::tcp_v4`, `Locator::tcp_v6`, `Locator::with_tcp_kind_set`);
`crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_locator_kinds`, `locator_to_socket`,
`TcpTransport::local_locator`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::tcp_kind_set`, Schlüssel `transport.tcp_locator_kinds`,
`select_user_transport`, `announce_locator`, `is_routable_user_locator`);
`crates/dcps/src/layered_transport.rs` (Zuordnung eines Ziels zu einem
Transport über `LocatorKind::transport_kind`).

**Tests:** `crates/rtps/src/wire_types.rs::tests::tcp_1_1_the_wire_kind_follows_the_kind_set`,
`crates/rtps/src/wire_types.rs::tests::tcp_1_1_two_kind_sets_in_one_process_do_not_interfere`
(zwei Threads kodieren gleichzeitig Locatoren je eines Wertepaars),
`crates/rtps/src/wire_types.rs::tests::the_tcp_kinds_are_vendor_range_by_default_and_fast_dds_by_setting`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_a_transport_announces_the_kinds_of_its_set`
(zwei Transporte eines Prozesses, TCPv4 und TCPv6; die Quelle eines
empfangenen Frames trägt das Wertepaar des Empfängers),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_a_destination_of_either_kind_set_uses_one_connection`,
`crates/transport-tcp/src/tcp_transport.rs::tests::rtps_9_3_2_4_locator_kinds_are_zerodds_vendor_range_values`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_locator_to_socket_v4_maps_tcp`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_1_1_locator_to_socket_v4_rejects_udp`,
`crates/dcps/src/runtime.rs::tests::tcp_1_1_the_announced_tcp_kind_follows_the_runtime_setting`
(zwei Runtimes eines Prozesses mit TCP als Nutzdaten-Transport; die
SPDP-Nachricht jeder Runtime trägt den Locator mit der Art ihrer
Einstellung und einer anderen Adresse als `0.0.0.0`, und `SpdpReader`
liest sie so zurück),
`crates/dcps/src/runtime.rs::tests::tcp_1_1_both_kind_sets_are_one_transport_per_address_family`
(unter beiden Wertepaaren: ein Listener auf `::` wird mit `::1`
angekündigt, einer auf `0.0.0.0` mit einer Adresse des Hosts oder der
gewählten Schnittstelle, je mit seiner Art und seinem Port; ein Ziel
beider Wertepaare geht über denselben TCP-Transport),
`crates/dcps/tests/runtime_config_layering.rs::rtps_9_3_2_4_the_tcp_locator_kinds_are_a_setting`.

**Status:** partial — gebaut: die Art ist Eigenschaft des Locators, der prozessweite Schalter `set_tcp_kinds_fast_dds` ist entfernt; `TcpTransport::with_locator_kinds` und `transport.tcp_locator_kinds` wählen 4 und 8 je Transport und je Runtime; Lesen behält die geschriebene Art; ein Ziel mit 4 oder 8 wird über den TCP-Transport gesendet. Mit dem Dialekt `cyclone` (§4.1) kündigt eine Runtime 4 und 8 an und einen Listener-Port für Metatraffic und Nutzdaten, wie Cyclone DDS mit `TCP/Port` (`crates/dcps/tests/runtime_config_layering.rs::tcp_4_1_the_cyclone_dialect_is_a_setting`; mit einem laufenden Cyclone DDS: `crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`). Die zitierten Tests in `zerodds-rtps`, `zerodds-transport-tcp` und `zerodds-dcps` sind auf dem Linux-Testhost grün. Mit dem Dialekt `fastdds` (§6.1) kündigt eine Runtime 4 und 8 mit dem logischen Port in den oberen 16 Bit des Ports an, und ein Locator der Art 4 oder 8 mit einem Port über 65535 wird als einer von Fast DDS gelesen und erreicht (`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination`, `crates/dcps/tests/fastdds_live_tcp.rs`). Rest: Arten und Port-Layout des Dialekts von RTI Connext (§6.2) fehlen; `transport.tcp_locator_kinds: fast-dds` allein kündigt 4 und 8 ohne logischen Port an, einen Locator, den Fast DDS nicht adressiert (es braucht den Dialekt `fastdds`).

### §1.2 RTPS-Wire-Mapping über den Stream

**Spec:** §2 — der Frame-Body ist eine vollständige RTPS-Nachricht
(RTPS-Header + Submessages, DDSI-RTPS 2.5 §8.3.3), byte-gleich zu der
Nachricht, die die UDP-Abbildung (§9.5) in einem Datagramm trägt. Der
TCP-Transport MUSS ihn unverändert durchreichen und darf ihn nicht
umschreiben, zusammenfassen oder aufteilen.

**Repo:** `crates/transport-tcp/src/framing.rs` (`write_frame`,
`read_frame` — der Payload wird ohne Interpretation kopiert);
`crates/transport-tcp/src/tcp_transport.rs` (`accept_one` reicht jeden
gelesenen Frame als `ReceivedDatagram` weiter).

**Tests:** `crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(zwei Frames über eine echte TCP-Verbindung, byte-gleich und in
Reihenfolge zurück),
`crates/transport-tcp/tests/loopback.rs::tcp_1_2_concurrent_senders_to_one_server`
(100 Frames aus zwei Prozess-Threads, kein Verlust),
`crates/transport-tcp/src/framing.rs::tests::tcp_1_2_roundtrip_small_frame`.

**Status:** done

### §1.3 Was OMG für TCP nicht normiert

**Spec:** DDSI-RTPS 2.5 legt für TCP nichts fest: keine Locator-Art
(§9.3.2.1), keine Abbildung der Nachrichten (§9.5 gilt für UDP/IP),
keinen Verbindungsaufbau, kein Logical-Port-Multiplexing und keine
Connection-Pool-Policy. Zum Stream-Framing sagt §8.3.3, dass ein
Stream-Transport wie TCP die Länge in der RTPS-HeaderExtension bräuchte
(„would need to include the length in the RTPS HeaderExtension"); diese
Anforderung führt §2.5 als Dialekt `rtps`. Cyclone DDS (`ddsi_tcp`) kennt keinen Handshake
und beginnt jede Nachricht mit dem RTPS-Header und einer eigenen
Submessage `0x81`, deren Längenfeld die ganze Nachricht zählt
(`ddsi_receive.c`, `read_packet_from_stream`; §4.1). Fast DDS und RTI
Connext haben eigene Formate: Fast DDS das Protokoll RTCP mit eigenem
Header, Bind und logischen Ports (§6.1, am Quelltext von Fast DDS 3.6.1
gelesen), RTI Connext eines, das nicht an dessen Quelltext geprüft ist
(§6.2). Die Sektion hält den Scope fest und stellt keine
Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Abgrenzung des Geltungsbereichs; die Anforderungen stehen in den Items ab §1.1 und §2.

## §2 Wire-Format

Im Default-Dialekt (`zerodds`) wird jedes RTPS-Datagramm über TCP wie
folgt gerahmt; der Dialekt `rtps` (§2.5) grenzt eine Nachricht stattdessen
über die HeaderExtension ab, die sie trägt:

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

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_1_2_roundtrip_small_frame`
(prüft die vier Header-Bytes gegen `5u32.to_be_bytes()`),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_1_roundtrip_empty_frame`,
`crates/transport-tcp/src/framing.rs::tests::tcp_2_1_two_frames_back_to_back`.

**Status:** done

### §2.2 Reassembly über Write-Grenzen hinweg

**Spec:** §2 — TCP ist ein Stream: der Empfänger MUSS einen Frame
zusammensetzen, der beim Sender über mehrere `write`-Aufrufe verteilt
wurde, und darf ihn weder verkürzt noch als zwei Frames weiterreichen.
Das gilt auch, wenn schon das Längenfeld gesplittet ankommt.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_frame` — eine Schleife über `read` füllt die vier Oktette der
Länge; `read_exact_or_eof` — `read_exact` füllt den Body; beides
unabhängig davon, wie viele `read`-Aufrufe das kostet).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_2_reassembles_message_split_across_two_writes`
(Längen-Präfix plus erste Hälfte im ersten Write, Rest plus Folgeframe im
zweiten; der Reader liefert genau eine Nachricht und danach den
Folgeframe),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_2_reassembles_frame_split_across_reads`
(4096-Byte-Payload bei Chunk-Größen 1, 3, 4, 5 und 4095),
`crates/transport-tcp/tests/loopback.rs::tcp_2_2_frame_split_across_two_writes_is_reassembled`
(dieselbe Teilung über einen echten TCP-Socket, mit 80 ms Pause zwischen
den beiden Writes; der Server liefert genau einen Frame).

**Status:** done

### §2.3 Maximum-Frame-Size

**Spec:** §2 — der Empfänger MUSS eine angekündigte Länge oberhalb des
Limits ablehnen, statt den Puffer zu allokieren (DoS-Schutz). Limit: 16
MiB. Der Sender MUSS eine Nutzlast oberhalb des Limits mit
`PayloadTooLarge` ablehnen, bevor er eine Verbindung aufbaut; das Ergebnis
darf nicht vom Zustand der Verbindung abhängen. Größere Samples werden auf
RTPS-Ebene per `DATA_FRAG` (DDSI-RTPS §8.3.7) zerlegt, nicht auf
Transport-Ebene.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`MAX_FRAME_SIZE = 16 * 1024 * 1024`, `FramingError::FrameTooLarge`,
`read_frame` prüft die Länge vor der Allokation, `write_frame` vor dem
ersten Oktett); Sendepfad in `crates/transport-tcp/src/tcp_transport.rs`
(`Transport::send` vergleicht die Länge vor dem Zugriff auf den Pool,
`SendError::PayloadTooLarge`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_3_rejects_oversized_announcement`
(`MAX_FRAME_SIZE + 1` wird abgelehnt, bevor gelesen wird),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_3_write_frame_takes_the_limit_and_refuses_one_octet_more`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_send_oversized_payload_returns_payload_too_large`
(Ziel ohne Listener: ein Connect-Versuch hätte `Io` geliefert),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_send_oversized_payload_does_not_connect`
(ein Listener am Ziel sieht keinen Connect, der Pool bleibt leer),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_3_accept_one_frame_too_large_errors`.

**Status:** done

### §2.4 Abbruch bei EOF mitten im Frame

**Spec:** §2 — schließt der Peer die Verbindung mitten in Header oder
Body, MUSS der Empfänger das als `UnexpectedEof` melden und zählen und
darf keinen verkürzten Frame nach oben geben. Ein EOF genau auf einer
Frame-Grenze ist dagegen der reguläre Verbindungsabbau und kein Fehler.

**Repo:** `crates/transport-tcp/src/framing.rs`
(`read_frame`: `FramingError::Closed` für das Ende vor dem ersten Oktett
eines Frames, `FramingError::UnexpectedEof` nach 1 bis 3 Oktetten der
Länge oder bei unvollständigem Body);
`crates/transport-tcp/src/tcp_transport.rs` (`read_connection`: `Closed`
beendet die Frame-Pumpe mit `Ok(())`, `UnexpectedEof` mit
`TcpTransportError::PeerIo` der Art `UnexpectedEof` und dem Zähler
`truncated_frames`; `serve` zählt jede Verbindung, deren Lesen mit einem
Fehler endete, in `connection_errors`).

**Tests:** `crates/transport-tcp/src/framing.rs::tests::tcp_2_4_unexpected_eof_mid_header`
(1, 2 und 3 Oktette der Länge),
`crates/transport-tcp/src/framing.rs::tests::tcp_2_4_unexpected_eof_mid_payload`,
`crates/transport-tcp/src/framing.rs::tests::tcp_2_4_eof_on_a_frame_boundary_is_closed`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reads_one_frame_then_eof`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reports_eof_mid_body`
(10 Oktette angekündigt, 3 gesendet: Fehler, Queue leer, Zähler 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_accept_one_reports_eof_mid_header`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_4_serve_counts_a_connection_that_ended_inside_a_frame`.

**Status:** done

### §2.5 Stream-Framing über die HeaderExtension

**Spec:** DDSI-RTPS 2.5 §8.3.3 — ein Stream-Transport wie TCP bräuchte die
Länge der Nachricht in der RTPS-HeaderExtension, um die Grenze der
Nachricht zu erkennen. §9.4.5.2 legt die HeaderExtension fest
(`messageLength` als erstes Feld bei gesetztem L-Flag, Submessage direkt
hinter dem RTPS-Header, §8.3.7.3). Eine Betriebsart, die Nachrichten auf
dem Stream so abgrenzt, MUSS wählbar sein (Dialekt `rtps`,
`transport.tcp_dialect: rtps`); das Framing aus §2.1 mit dem Handshake aus
§3 bleibt der Default dieses Dokuments (Dialekt `zerodds`). Im Dialekt
`rtps` trägt eine Verbindung RTPS-Nachrichten ohne Trenner hintereinander,
ohne Handshake und ohne Längenfeld vor der Nachricht. Jede Nachricht
beginnt mit 28 Oktetten:

```text
 0  'R' 'T' 'P' 'S'                RTPS-Header (§9.4.4), 20 Oktette
 4  major minor                    Protokollversion, 2.5 oder höher
 6  vendorId (2)
 8  guidPrefix (12)
20  0x00                           submessageId RTPS_HE (§9.4.5.1.1)
21  flags                          E = 0x01, L = 0x02 gesetzt
22  octetsToNextHeader (u16)       in der Byte-Reihenfolge von E, mindestens 4
24  messageLength (u32)            in der Byte-Reihenfolge von E
```

`messageLength` zählt die ganze Nachricht ab dem ersten Oktett des
RTPS-Headers (§8.3.7.5). Der Empfänger MUSS die 28 Oktette lesen, danach
`messageLength − 28` weitere, und die Nachricht unverändert weitergeben,
die HeaderExtension eingeschlossen (§1.2). Er MUSS die Verbindung
schließen, wenn eine Nachricht nicht mit `RTPS` beginnt, ihre
Protokollversion unter 2.5 liegt, ihre erste Submessage keine
HeaderExtension mit L-Flag ist, `messageLength` kleiner ist als RTPS-Header
und HeaderExtension zusammen oder größer als das Limit aus §2.3; ein
Stream hat keinen zweiten Weg zur nächsten Nachrichtengrenze. §2.4 gilt
für Nachrichten wie für Frames. Der Transport fügt einer Nachricht nichts
hinzu: ein `send` MUSS eine Nachricht ablehnen, die ihre eigene Länge
nicht so trägt, bevor er eine Verbindung aufbaut. Die Runtime setzt die
HeaderExtension mit `messageLength` in jede Nachricht, die sie über eine
solche Verbindung sendet. Ohne Handshake gibt es keinen Request: die
Vendor-Liste aus §3.3 gilt für die Vendor-ID im RTPS-Header jeder
Nachricht, eine Verbindung über dem Peer-Limit (§3.7) wird ohne Antwort
geschlossen und gezählt, und es gibt keinen Claim (§3.6). Beide Enden
einer Verbindung haben denselben Dialekt; die Erkennung am ersten Oktett
führt §4.2.

Eine als Ganzes geschützte Nachricht (DDS-Security 1.2,
`rtps_protection_kind` außer NONE oder ein Pre-Shared Key) trägt die
HeaderExtension zwischen RTPS-Header und SRTPS_PREFIX (DDS-Security 1.2
§10.5.3.3.1 Tabelle 80: „the original RTPS Header and, if present, the
original HeaderExtension, followed by the SecureRTPSPrefixSubMsg"). Der
Empfänger MUSS das SRTPS_PREFIX hinter einer führenden HeaderExtension
finden. Zwei Fälle:

- Der Schutz authentisiert RTPS-Header und HeaderExtension (Flag A des
  SRTPS_PREFIX: Pre-Shared Key, Tabelle 89, oder die Form mit „Additional
  Authenticated Data", Tabelle 80, 2). Die HeaderExtension MUSS vor dem
  Schutz in der Nachricht stehen: `messageLength` zählt für den Tag als
  null und beschreibt danach die geschützte Nachricht (Tabelle 80, 3). Eine
  nach dem Schutz eingesetzte HeaderExtension ist nicht die authentisierte,
  und der Empfänger lehnt die Nachricht ab; die Strecke, die die Länge
  einsetzt, MUSS eine solche Nachricht ohne HeaderExtension abweisen.
- Der Schutz authentisiert den Header nicht (kein Flag A, die Vorgabe mit
  dem ausgetauschten Schlüssel). `encode_rtps_message` nimmt dann keine
  HeaderExtension an (Tabelle 80, 1); die Strecke setzt sie nach dem Schutz
  ein. Kein Tag deckt sie: der Empfänger MUSS sie nach dem Abgrenzen
  verwerfen und darf keines ihrer Felder weitergeben. Diese Form zeigt
  DDS-Security 1.2 nicht: §10.5.3.3.4.6.4 kennt ohne AAD keine
  HeaderExtension.

**Repo:** `crates/transport-tcp/src/rtps_stream.rs` (`STREAM_HEAD_LEN`,
`message_length`, `read_message`, `check_message`, `write_message`);
`crates/transport-tcp/src/framing.rs` (`FramingError::NotRtps`,
`NoMessageLength`, `LengthTooSmall`, `LengthMismatch`);
`crates/transport-tcp/src/tcp_transport.rs` (`TcpDialect`,
`TcpTransport::with_dialect`, `dialect`, `read_rtps_stream`,
`PeerConn::speaking`, Prüfung in `Transport::send`,
`TcpTransportError::StreamFraming`, `VendorNotAccepted`);
`crates/rtps/src/header_extension.rs`
(`HeaderExtension::with_message_length`);
`crates/dcps/src/stream_length_leg.rs` (`StreamLengthLeg`,
`authenticated_without_extension`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_dialect`, Schlüssel
`transport.tcp_dialect` mit `zerodds` und `rtps`, Umgebungsvariable
`ZERODDS_TCP_DIALECT`, `RuntimeConfig::tcp_leg`,
`RuntimeConfig::apply_tcp_settings`; geschützte Nachrichten:
`secure_outbound_bytes`, `stated_for_authenticated_stream`,
`protect_for_stream`, `secure_inbound_bytes`);
`crates/security-rtps/src/srtps.rs` (`srtps_prefix_offset`,
`is_secured_rtps_message`, `srtps_prefix_flags`, `srtps_psk_flag`,
`decode_secured_rtps_message`);
`crates/security-runtime/src/shared.rs` (`classify_inbound`,
`transform_inbound_from`, `transform_inbound`,
`SharedSecurityGate::is_message_protected`,
`authenticates_header_without_extension`),
`crates/security-runtime/src/gate.rs` (`decode_inbound_message`),
`crates/security-runtime/src/psk_gate.rs` (`is_psk_protected`);
`crates/security-crypto/src/plugin.rs` (`decode_rtps_message_of`: ohne
Flag A gehört die HeaderExtension nicht zur dekodierten Nachricht).

**Tests:** `crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_message_delimited_by_header_extension_length`
(die 28 Oktette von Hand nach §9.4.4 und §9.4.5.2 gebaut; Oktette einer
Folgenachricht bleiben im Stream),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_big_endian_message_length`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_reads_a_header_extension_with_further_fields`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_refuses_a_stream_message_without_length`
(keine HeaderExtension, kein L-Flag, kein Platz für das Feld, Version
2.4, der Request aus §3.1; dieselben auf der sendenden Seite, ohne dass
ein Oktett geschrieben wird),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_refuses_a_message_length_below_28`
(0, 1, 20, 27; 28 ist die Nachricht ohne weitere Submessage; 16 MiB + 1
wird vor dem Lesen abgelehnt),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_two_messages_back_to_back`
(bei Chunk-Größen 1, 3, 27, 28, 29 und 4096),
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_eof_inside_a_message_is_an_error`,
`crates/transport-tcp/src/rtps_stream.rs::tests::tcp_2_5_a_message_to_send_states_its_own_length`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_two_transports_exchange_messages_in_the_rtps_dialect`
(echte Sockets: ein roher Listener liest genau die Oktette der beiden
Nachrichten, beginnend mit `RTPS`, und nichts wird zurückgeschrieben;
zwischen zwei Transporten kommen die Nachrichten unverändert an),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_send_refuses_a_message_without_length`
(kein Connect, der Pool bleibt leer),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_that_is_no_rtps_stream_is_closed`
(auch: ein Listener des Dialekts `zerodds` weist einen RTPS-Stream mit
`BadMagic` ab),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_the_vendor_list_applies_to_the_rtps_header`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_beyond_the_limit_is_closed_without_an_answer`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_puts_the_length_behind_the_header`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_keeps_the_fields_of_an_extension`,
`crates/rtps/src/header_extension.rs::tests::rtps_8_3_3_with_message_length_refuses_what_is_no_rtps_2_5_message`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_a_message_sent_through_the_leg_states_its_length`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_refuses_what_cannot_carry_the_length`,
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_hands_everything_else_to_the_transport`,
`crates/dcps/src/runtime.rs::tests::tcp_2_5_the_tcp_leg_of_the_rtps_dialect_states_the_length_of_each_message`
(ein roher Listener liest von der TCP-Strecke einer Runtime zwei
Nachrichten zu je 40 Oktetten mit HeaderExtension und ohne Handshake),
`crates/dcps/tests/runtime_config_layering.rs::tcp_2_5_the_tcp_dialect_is_a_setting`,
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv4_rtps_dialect`
(zwei Prozesse, RELIABLE und KEEP_ALL: alle 200 Samples kommen einmal und
in Reihenfolge an, über eine TCP-Verbindung zwischen den Prozessen).
Geschützte Nachrichten:
`crates/security-rtps/src/srtps.rs::tests::security_10_5_3_3_1_the_srtps_prefix_is_found_behind_a_header_extension`
(SRTPS_PREFIX hinter einer HeaderExtension in beiden Byte-Reihenfolgen,
die Flags dahinter; Klartext und abgeschnittene Nachrichten nicht),
`crates/security-crypto/src/plugin.rs::tests::security_10_5_3_3_1_an_unauthenticated_header_extension_is_not_part_of_the_decoded_message`
(ohne Flag A: die dekodierte Nachricht ist die geschützte, ohne die
HeaderExtension; ein unterwegs eingesetzter Zeitstempel kommt nicht an),
`crates/security-runtime/src/shared.rs::tests::security_10_5_3_3_1_classify_inbound_finds_srtps_behind_a_header_extension`
(das Gate nimmt die Nachricht an; Klartext hinter einer HeaderExtension
bleibt auf einer geschützten Domain abgewiesen),
`crates/dcps/src/stream_length_leg.rs::tests::tcp_2_5_the_leg_and_a_protected_message`
(ohne Flag A setzt die Strecke die HeaderExtension vor das SRTPS_PREFIX;
mit Flag A und ohne HeaderExtension weist sie die Nachricht ab; mit
HeaderExtension setzt sie nur die Länge),
`crates/dcps/src/runtime.rs::tests::tcp_2_5_a_message_under_a_pre_shared_key_states_its_length_before_the_protection`
(Pre-Shared Key: `RTPS-Header | HeaderExtension | SRTPS_PREFIX` mit A und
P, `messageLength` gleich der Länge der geschützten Nachricht; der
Empfänger liest die Nachricht mit ihrer HeaderExtension zurück; eine nach
dem Schutz eingesetzte HeaderExtension wird abgelehnt),
`crates/dcps/src/runtime.rs::tests::tcp_2_5_a_protection_that_authenticates_the_header_gets_the_header_extension`
(die Entscheidung am Flag A der Ausgabe, mit nachgestellten
Schutzfunktionen),
`tests/integration/tests/security.rs::security_rtps_protection_over_the_tcp_rtps_dialect`
(zwei Prozesse, `rtps_protection_kind` ENCRYPT, Nutzdaten über TCP im
Dialekt `rtps`: alle 50 Samples kommen einmal und in Reihenfolge an; der
Kernel zeigt die TCP-Verbindung; jede RTPS-Nachricht, die `strace` auf
einem TCP-Socket des Schreibers aufgezeichnet hat, trägt die
HeaderExtension mit der Länge und ist `RTPS-Header | HeaderExtension |
SRTPS_PREFIX`, außer dem Verkehr, für den der Schutz der ganzen Nachricht
nicht gilt (RTPS Bootstrapping Messages nach DDS-Security 1.2 §7.5.7 und
§9.4.2.4 Tabelle 33, Attribut is_rtps_axk_protected: SPDP, Authentisierung,
Schlüsselaustausch; er läuft nach §4.3 ebenfalls über TCP); der Text der
Samples steht in keinem Send. Ohne die Suche hinter der HeaderExtension bleibt die
Bestätigung des Lesers aus und das Szenario scheitert nach 15 s).

**Status:** done — nicht gelaufen: die Form mit „Additional Authenticated Data" und ausgetauschtem Schlüssel Ende zu Ende (kein Schlüssel der Konfiguration schaltet sie am Gate ein; getestet sind die Entscheidung der Runtime mit nachgestellten Schutzfunktionen und das Plugin für sich); der Pre-Shared Key über zwei Prozesse (getestet in einer Runtime und zwischen zwei Runtimes eines Prozesses, ohne Socket); kein anderer Hersteller, weder mit noch ohne Schutz.

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
(4). ZeroDDS-Version ist `(1,0)`, die VendorId `[0x01, 0xF0]` (`VendorId::ZERODDS` aus `zerodds-rtps`). Ein
falsches Magic MUSS die Verbindung scheitern lassen.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionRequest`, `encode_request`, `decode_request`,
`HANDSHAKE_MAGIC_REQUEST`, `HANDSHAKE_WIRE_SIZE = 16`,
`VENDOR_ID_ZERODDS`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_wire_layout_is_sixteen_bytes_big_endian`
(vergleicht alle 16 Bytes gegen das Literal des Diagramms, inklusive
`logical_port = 7400` als `00 00 1C E8`),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_roundtrip`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_1_request_rejects_wrong_magic`.

**Status:** done

### §3.2 BindConnectionResponse — Wire-Layout und Status-Byte

**Spec:** §3 — 16 Byte big-endian: Magic-Prefix `"ZDA"` plus
Status-Byte (`0x2B` = `'+'` Accept, `0x2D` = `'-'` Reject), `v_major`,
`v_minor`, `vendor_id`, `flags`, `reason_code` (4, big-endian; `0` bei
Accept). Ein anderes Magic MUSS als `BadResponse` scheitern.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`BindConnectionResponse`, `ResponseStatus`, `encode_response`,
`decode_response`, `HANDSHAKE_MAGIC_ACCEPT`, `HANDSHAKE_MAGIC_REJECT`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_wire_layout_is_sixteen_bytes_big_endian`
(alle 16 Bytes für Accept und für Reject mit `reason_code = 3`),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_roundtrip_accept`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_response_roundtrip_reject`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_2_client_handshake_errors_on_bad_response_magic`.

**Status:** done

### §3.3 Reject-Reason-Codes

**Spec:** §3 — sechs stabile Wire-Codes: `0 Unknown`,
`1 VersionMismatch`, `2 ResourceLimit`, `3 LogicalPortConflict`,
`4 VendorNotAccepted`, `5 ReservedFlagsSet`. Ein unbekannter Code MUSS auf
`Unknown` abgebildet werden, damit ein neuerer Peer den Reject trotzdem
verarbeiten kann; ein Peer ohne Code 5 liest ihn so. Der Server MUSS einen
Request in fester Reihenfolge prüfen und mit dem Grund der ersten Prüfung
ablehnen, die scheitert:

1. Version (`VersionMismatch`): die Felder dahinter haben nur bei passender
   Version die Bedeutung dieses Dokuments.
2. Reservierte Flags (`ReservedFlagsSet`, §3.5).
3. Vendor-ID (`VendorNotAccepted`).
4. Peer-Limit (`ResourceLimit`, §3.7).
5. Logical-Port-Claim (`LogicalPortConflict`, §3.6). Er steht zuletzt, weil
   ein gewährter Claim gehalten wird: ein aus anderem Grund abgelehnter
   Request hat nichts beansprucht.

Welche Vendor-IDs ein Listener annimmt, MUSS einstellbar sein
(`transport.tcp_accepted_vendors`, Liste aus je vier Hex-Ziffern, `01f0`
für ZeroDDS); ohne Einstellung nimmt er jede an. Die Liste der Einstellung
hat mindestens einen Eintrag: die leere Liste wird als Wert des Schlüssels
abgelehnt. Am Transport (`TcpTransport::with_accepted_vendors`) nimmt eine
leere Liste keinen Hersteller an.

**Repo:** `crates/transport-tcp/src/handshake.rs`
(`RejectReason`, `RejectReason::from_code`, `RejectReason::as_code`,
`ServerPolicy`, `read_request`, `decide`, `write_response`,
`server_handshake_with`; `server_handshake` ist deren Folge mit der
Default-Policy); `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_accepted_vendors`, `accepted_vendors`,
`read_connection` und `answer_resource_limit` übergeben die Liste an
`decide`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::tcp_accepted_vendors`, Schlüssel
`transport.tcp_accepted_vendors`, Umgebungsvariable
`ZERODDS_TCP_ACCEPTED_VENDORS`, `parse_vendor_list`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_reject_reason_roundtrip_known_codes`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_reject_reason_unknown_code_maps_to_unknown`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_rejects_version_mismatch`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_rejects_vendor_not_accepted`
(die Response ist `ZDA-` mit `reason_code = 4`; eine leere Liste nimmt
keinen Hersteller an),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_server_accepts_a_listed_vendor`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_decide_checks_in_the_stated_order`
(ein Request, der an allen fünf Prüfungen scheitert; mit jeder
behobenen Prüfung nennt die nächste den Grund, und der Claim wird erst
angefragt, wenn alle anderen bestanden sind),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_resource_limit`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_logical_port_conflict`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_vendor_not_accepted`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_3_accept_one_rejects_an_unlisted_vendor_over_a_real_connection`
(über eine echte Verbindung: `client_handshake` liest
`VendorNotAccepted`, der Server schließt, ein Frame hinter dem
abgelehnten Request erreicht die Queue nicht, ein gelisteter Hersteller
wird gelesen),
`crates/dcps/src/runtime.rs::tests::tcp_3_3_the_tcp_transport_of_a_runtime_has_its_vendor_list_and_claim`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_3_3_the_accepted_vendors_are_a_setting`
(auch: die leere Liste und der leere Wert sind ein Konfigurationsfehler).

**Status:** done

### §3.4 Handshake-Sequenz und Pflicht vor dem ersten Frame

**Spec:** §3 — der Client MUSS den Request vor dem ersten
Length-Prefix-Frame senden, der Server MUSS die Response vor dem ersten
Frame beantworten. Ein Peer, dessen erste Oktette weder der Request noch
der Beginn eines anderen angenommenen Dialekts sind (§4.2), MUSS
abgewiesen werden; bei Reject droppen beide Seiten die Verbindung.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`client_handshake`,
`server_handshake`); `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected` ruft `client_handshake` direkt nach
`connect` und behält bei einem Fehler oder Reject keine Verbindung;
`read_connection` ruft `server_handshake_with` vor der Frame-Pumpe und schließt
die Verbindung nach einem Reject, ohne einen Frame zu lesen).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_4_full_roundtrip_accept_via_paired_cursors`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`
(HTTP-Probe wird mit `TcpTransportError::Handshake` abgewiesen),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_the_server_closes_after_a_reject`
(Request mit Version 2.0 über eine echte Verbindung: der Peer liest
`ZDA-` mit `reason_code = 1` und danach EOF; ein Frame hinter dem
abgelehnten Request erreicht die Queue nicht),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_the_client_keeps_no_connection_after_a_reject`
(der Listener antwortet `ZDA-`: der Client schließt ohne Frame, der Pool
hält keine Verbindung, der Backoff steht auf 50 ms),
`crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(Handshake und Frames über eine echte Verbindung).

**Status:** done

### §3.5 Reserved `flags` — Empfänger-Prüfung

**Spec:** §3 — das `flags`-Feld ist reserviert und MUSS `0` sein. Ein
Empfänger, der ein Feld mit gesetzten Bits akzeptiert, verbaut die
spätere Belegung dieser Bits: ein alter Peer würde ein neues Flag still
ignorieren statt die Verbindung abzulehnen. Der Server MUSS einen Request
mit einem gesetzten Bit mit `ReservedFlagsSet` (Code 5, §3.3) ablehnen.
Der Client MUSS eine Response mit einem gesetzten Bit verwerfen, gleich ob
sie Accept oder Reject trägt, und die Verbindung schließen; der nächste
Versuch wartet den Backoff aus §5.2 ab. Beide Seiten senden `flags = 0`.

**Repo:** `crates/transport-tcp/src/handshake.rs` (`decide`: Prüfung nach
der Version; `client_handshake`: `HandshakeError::ReservedFlags`;
`RejectReason::ReservedFlagsSet`; `write_response` und `client_handshake`
schreiben `flags = 0`); `crates/transport-tcp/src/tcp_transport.rs`
(`read_connection` schließt nach dem Reject, `PeerConn::ensure_connected`
behält nach `ReservedFlags` keine Verbindung).

**Tests:** `crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_server_rejects_a_request_with_reserved_flags_set`
(`ZDA-` mit `reason_code = 5`; der Claim des Requests wird nicht
angefragt),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_client_drops_a_response_with_reserved_flags_set`
(Accept und Reject),
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_5_each_single_flag_bit_is_rejected`
(jedes der 32 Bits einzeln, in beiden Richtungen),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_5_accept_one_rejects_flags_over_a_real_connection`
(ein Frame hinter dem Request erreicht die Queue nicht),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_5_the_client_drops_a_connection_accepted_with_reserved_flags`
(der Listener liest nach seiner Response das Ende der Verbindung und
keinen Frame; der Pool hält keine Verbindung, Backoff 50 ms).

**Status:** done

### §3.6 Logical-Port-Claim

**Spec:** §3 — `logical_port` ist der Port, den der Client auf dieser
Verbindung beansprucht; `0` heißt „kein Claim". Der Client beansprucht den
Port seines eigenen Listeners, also den Port des Locators, den er
ankündigt. Ein Claim gilt für das Tripel aus der Adresse des Listeners, an
der die Verbindung ankam, der Adresse des Peers und dem Port, unter den
lebenden Verbindungen eines Listeners. Der Server MUSS einen Request,
dessen Tripel eine lebende Verbindung hält, mit `LogicalPortConflict`
ablehnen und die Verbindung sonst dem beanspruchten Port zuordnen: ihre
Frames tragen als Quelle die Adresse des Peers mit dem beanspruchten Port,
also den Locator des Listeners des Peers. Ohne Claim tragen sie den Port,
von dem aus der Peer verbunden hat. Ein Claim über 65535 nennt keinen
Port eines TCP-Listeners: der Server MUSS ihn mit `LogicalPortConflict`
ablehnen, ohne die Claim-Tabelle zu befragen, denn die Frames der
Verbindung trügen sonst eine Quelle, an die kein `send` senden kann. Der
Claim endet mit der Verbindung.
Angenommene Verbindungen MÜSSEN TCP-Keepalive tragen (10 s Leerlauf, 3
Proben im Abstand von 5 s, wo die Plattform Abstand und Anzahl einstellen
lässt), damit ein Peer, der ohne FIN verschwunden ist, seinen Claim nicht
unbegrenzt hält. Nach `LogicalPortConflict` wartet der Client den Backoff
aus §5.2 ab und versucht es erneut, denn der Server kann das Ende der
vorigen Verbindung noch nicht gelesen haben. Zwei Hosts hinter einer
übersetzten Adresse, deren Listener denselben Port haben, kollidieren; für
sie MUSS der Client den Claim abschalten können
(`transport.tcp_logical_port_claim: none`, der Request trägt dann `0`).

**Repo:** `crates/transport-tcp/src/handshake.rs` (`ServerPolicy::claim`,
`decide`); `crates/transport-tcp/src/tcp_transport.rs` (`ClaimTable`,
`ClaimGuard` — gibt den Claim bei jedem Ende von `read_connection` frei —,
`read_connection`: Claim, Quelle der Frames, `set_keepalive` mit
`KEEPALIVE_IDLE`, `KEEPALIVE_INTERVAL`, `KEEPALIVE_PROBES`;
`TcpTransport::claimed_logical_ports`; verbindende Seite:
`TcpTransport::with_logical_port_claim`, `claims_logical_port`,
`outgoing_claim`, `PeerConn::claiming`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::tcp_logical_port_claim`, Schlüssel
`transport.tcp_logical_port_claim` mit `listener` und `none`,
Umgebungsvariable `ZERODDS_TCP_LOGICAL_PORT_CLAIM`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_a_second_connection_claiming_a_bound_port_is_rejected`
(echte Verbindungen: die zweite liest `ZDA-` mit `reason_code = 3` und
das Ende der Verbindung, ihr Frame kommt nicht an, die erste bleibt),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_claim_is_released_when_the_connection_ends`
(nach dem Ende des Halters wird derselbe Port wieder gewährt; ein wegen
der Version abgelehnter Request hält nichts),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_zero_is_no_claim`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_6_a_claim_above_65535_is_refused`
(65536 und `u32::MAX`: die Response ist `ZDA-` mit `reason_code = 3`, die
Policy wird nicht befragt; 65535 wird gewährt),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_a_claim_above_65535_is_rejected_over_a_real_connection`
(der Peer liest `ZDA-` mit `reason_code = 3`, kein Claim wird gehalten,
kein Frame kommt an; mit 65535 tragen die Frames diesen Port als Quelle),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_frames_of_a_claimed_connection_carry_the_claimed_port_as_source`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_client_claims_its_listener_port`
(Oktette 12 bis 15 des Requests sind der Port des eigenen Listeners, mit
abgeschaltetem Claim `0`; zwischen zwei Transporten ist die Quelle eines
Frames der Locator des Senders, und ein an sie gesendeter Frame kommt
dort an),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_same_port_from_another_address_is_accepted`
(die Claim-Tabelle: anderer Peer, andere Listener-Adresse und anderer
Port werden gewährt, dasselbe Tripel nicht),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_two_peer_addresses_hold_the_same_port_at_once`
(nur Linux: Verbindungen von 127.0.0.1 und 127.0.0.2),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_an_accepted_connection_has_keepalive_on`
(liest die Optionen am Socket; unter Linux 10 s, 5 s, 3),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_6_the_client_retries_after_a_logical_port_conflict`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_3_6_the_logical_port_claim_is_a_setting`,
`crates/dcps/src/runtime.rs::tests::tcp_3_3_the_tcp_transport_of_a_runtime_has_its_vendor_list_and_claim`.

**Status:** done — nicht gemessen: dass der Kernel eine Verbindung ohne Gegenstelle nach den Keepalive-Proben aufgibt; der Test liest die gesetzten Optionen.

### §3.7 ResourceLimit-Reject bei erreichtem Peer-Limit

**Spec:** §3 — hat der Server sein Peer-Limit erreicht, MUSS er den
Handshake mit `ResourceLimit` ablehnen, statt die Verbindung anzunehmen
und danach zu verwerfen. Nur so kann der Client den Backoff vom
Verbindungsfehler unterscheiden. Das Limit zählt die Verbindungen, die der
Listener gerade liest (256, am Transport einstellbar). Der Server liest
den Request einer Verbindung über dem Limit, bevor er antwortet und
schließt: ein Schließen mit ungelesenen Daten setzt die Verbindung zurück
und kann den Peer die Response kosten. Scheitert der Request an einer
Prüfung, die in §3.3 vor dem Limit steht, trägt der Reject deren Grund.
Die annehmende Schleife darf kein Oktett einer abgelehnten Verbindung
lesen. Das Ablehnen ist selbst begrenzt: Request und Response haben
zusammen 250 ms, und höchstens 16 abgelehnte Verbindungen warten zugleich
auf ihre Antwort. Ein Thread beantwortet die wartenden Verbindungen der
Reihe nach, und jede stumme davor kostet 250 ms. Der Reject MUSS
geschrieben sein, solange der Client auf ihn wartet (2 s, §3.8): eine
abgelehnte Verbindung wird bis 1,5 s nach ihrer Annahme beantwortet, und
ihr Austausch endet spätestens zu diesem Zeitpunkt. Eine weitere über den
16, eine, deren Request in der Zeit nicht vollständig ist, und eine, die
später an die Reihe kommt, wird ohne Antwort geschlossen und zusätzlich
gezählt; gezählt wird vor dem Schließen. Eine Verbindung, für die kein
Stream-Klon oder kein lesender Thread entsteht, wird wie eine über dem
Limit abgelehnt. Der Client MUSS einen Reject von einem Verbindungsfehler
unterscheiden: er zählt ihn nach Grund, der Fehler des `send` nennt den
Grund, und nach `ResourceLimit` wartet der nächste Versuch mindestens 1 s
statt der 50 ms aus §5.2.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::serve`, `refuse_beyond_limit`, `refuse_unstarted`,
`answer_resource_limit` im Thread `zdds-tcp-refuse`, `REFUSE_BUDGET`,
`REFUSE_QUEUE`, `REFUSE_DEADLINE`,
`TcpTransport::with_accept_limit`, `accept_limit`, `refused_connections`,
`refused_unanswered`; verbindende Seite: `PeerConn::ensure_connected`,
`PeerConn::last_reject`, `RESOURCE_LIMIT_BACKOFF_MS`, `reject_message`,
`TcpTransport::connect_rejections`, `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_serve_answers_resource_limit_beyond_the_peer_limit`
(echte Verbindungen, Limit 2: die dritte liest `ZDA-` mit
`reason_code = 2` und das Ende der Verbindung, `refused_connections` 1,
`refused_unanswered` 0, die zwei gelesenen bleiben; ein Request der
Version 2.0 über dem Limit liest `reason_code = 1`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_silent_refused_peer_does_not_hold_the_accept_loop`
(acht stumme Verbindungen über dem Limit: eine Verbindung, die inzwischen
einen Platz findet, ist gelesen, bevor alle acht aufgegeben sind; jede
stumme liest das Ende der Verbindung ohne Response),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_beyond_the_refuse_queue_a_connection_is_closed_unanswered`
(die 17. wartende Verbindung wird geschlossen und gezählt, die 16 davor
bleiben offen),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_client_behind_silent_refused_connections_is_answered_in_time`
(drei stumme Verbindungen vor dem Client: sein `send` nennt
`tcp peer rejected: resource limit` in unter 2 s, Backoff 1000 ms; die
drei stummen sind als unbeantwortet gezählt, seine Verbindung nicht),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_refused_connection_whose_turn_comes_too_late_is_closed_unanswered`
(mit vollständigem Request: vor der Frist `ZDA-` mit `reason_code = 2`, an
der Frist kein Oktett einer Response und `refused_unanswered` 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_connection_without_a_reading_thread_is_refused`
(`refuse_unstarted`, der Weg von `serve` nach einem gescheiterten
Thread-Start: der Platz ist wieder frei, der Peer liest `ZDA-` mit
`reason_code = 2`; ohne registrierten Stream wird die Verbindung als
abgelehnt und unbeantwortet gezählt),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_the_client_counts_the_resource_limit_reject_and_backs_off_one_second`
(`SendError::Io` mit `tcp peer rejected: resource limit`,
`connect_rejections` 1, Backoff 1000 ms; der gedrosselte `send` baut
keine zweite Verbindung auf; ein Ziel ohne Listener liefert
`tcp write failed` mit 50 ms),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_a_connection_ended_frees_a_place`,
`crates/transport-tcp/src/handshake.rs::tests::tcp_3_3_client_handshake_rejects_on_resource_limit`.

**Status:** done — kein Test lässt `try_clone` oder den Start des lesenden Threads selbst scheitern; getestet ist die Ablehnung, die `serve` danach aufruft (`refuse_beyond_limit`, `refuse_unstarted`).

### §3.8 Zeitbudget im Accept-Pfad

**Spec:** §3 — der Accept-Pfad MUSS ein Gesamtbudget für den Handshake
führen, nicht nur ein Timeout pro Syscall. Ein byte-weise trödelnder Peer
würde sonst `16 × slice` Sekunden des Accept-Threads binden. Das Budget
beträgt 2 s für Request und Response zusammen; läuft es ab, bevor der
Request vollständig ist, wird keine Response geschrieben. Dasselbe Budget
gilt auf der verbindenden Seite.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs` (`DeadlineStream`:
setzt vor jedem Lese- und Schreib-Syscall das Socket-Timeout auf das
Minimum aus Slice und Restbudget und liefert nach Ablauf `TimedOut`;
`read_connection` mit `SERVER_HANDSHAKE_BUDGET` = 2 s und
`SERVER_HANDSHAKE_SLICE` = 200 ms; `PeerConn::ensure_connected` mit
`CLIENT_HANDSHAKE_BUDGET` = 2 s; nach Accept werden die Timeouts wieder
aufgehoben).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_accept_one_gives_up_on_a_peer_sending_octet_by_octet`
(ein Oktett je 150 ms, jedes innerhalb der Slice, der Peer braucht also
mindestens 2,25 s: `accept_one` kehrt nach 1,9 s oder später mit
`Handshake(Io(TimedOut))` zurück, der Peer hat kein Oktett einer Response
gelesen; ein Budget, das länger ist, als der Peer braucht, oder nur ein
Timeout je Read ließe die Request vollständig ankommen, und der Test
scheitert),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_the_client_gives_up_on_a_listener_answering_octet_by_octet`
(der Send scheitert nach 1,9 s oder später, bevor der Listener
vollständig geantwortet hat, der Pool hält keine Verbindung),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_8_accept_one_gives_up_on_a_stalling_handshake_peer`
(Peer sendet ein Handshake-Byte und schweigt, hält die Verbindung aber
offen; `accept_one` kehrt nach einer Slice, in unter 1 s, zurück),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`.

**Status:** done

## §4 Cyclone-`ddsi_tcp`-Kompatibilität

### §4.1 Handshake-Skip-Mode

**Spec:** §4 — Cyclone DDS (`ddsi_tcp`) kennt über TCP keinen Handshake
und kein Längenfeld vor der Nachricht. Eine Verbindung trägt
RTPS-Nachrichten ohne Trenner hintereinander, und die erste Submessage
jeder Nachricht ist `0x81` (`DDSI_RTPS_SMID_ADLINK_MSG_LEN`), deren
32-Bit-Feld die ganze Nachricht zählt, Header und diese Submessage
eingeschlossen (`ddsi_xmsg.c`: `xp->msg_len`; `ddsi_receive.c`:
`read_packet_from_stream` weist eine Nachricht ab, deren erste Submessage
nicht `0x81` ist). Diese Betriebsart MUSS wählbar sein (Dialekt `cyclone`,
`transport.tcp_dialect: cyclone`), für die Verbindungen, die der Transport
öffnet; die er annimmt, erkennt er nach §4.2. Jede Nachricht beginnt auf
dem Stream mit 28 Oktetten:

```text
 0  'R' 'T' 'P' 'S'                RTPS-Header, 20 Oktette
 4  major minor                    Protokollversion
 6  vendorId (2)
 8  guidPrefix (12)
20  0x81                           submessageId der Längen-Submessage
21  flags                          E = 0x01
22  octetsToNextHeader (u16)       in der Byte-Reihenfolge von E; 4
24  length (u32)                   in der Byte-Reihenfolge von E
```

Die Längen-Submessage ist Framing und kein Teil der RTPS-Nachricht: mit
RTPS-Schutz setzt Cyclone DDS sie hinter den Header der geschützten
Nachricht und entfernt sie vor dem Entschlüsseln (`ddsi_security_omg.c`).
Der Sender MUSS die acht Oktette hinter den RTPS-Header der Nachricht
setzen, die er erhält, und der Empfänger MUSS sie entfernen: über dem
Transport steht die Nachricht, die ein UDP-Datagramm trägt (§1.2). Der
Empfänger liest die 28 Oktette und danach `length − 28` weitere. Er MUSS
die Verbindung schließen, wenn eine Nachricht nicht mit `RTPS` beginnt,
ihre erste Submessage nicht `0x81` ist, deren `octetsToNextHeader` das
Feld nicht fasst, `length` kleiner ist als Header und Längen-Submessage
zusammen oder größer als das Limit aus §2.3; §2.4 gilt für Nachrichten wie
für Frames. Cyclone DDS verwirft eine Verbindung bei einer Nachricht über
65536 Oktetten (`ddsi_receive.c`, `do_packet`). Der Sender MUSS eine
Nachricht, die mit der Längen-Submessage länger wäre (über 65528 Oktette),
mit `PayloadTooLarge` ablehnen, bevor er eine Verbindung aufbaut, ebenso
eine ohne RTPS-Header; ein Transport dieses Dialekts nennt 65528 als
größte Nachricht. Vendor-Liste und Peer-Limit gelten wie in §2.5, einen
Claim gibt es nicht.

Verbindungen werden in beide Richtungen genutzt. Cyclone DDS ohne
`TCP/Port` hat keinen Listener, kündigt keinen Unicast-Locator an und
erwartet die Antworten auf der Verbindung, die es geöffnet hat
(`ddsi_init.c`: `publish_uc_locators`; `ddsi_tcp.c`: angenommene und
aufgebaute Verbindungen in einer Tabelle). Die Quelle der Nachrichten einer
angenommenen Verbindung ohne Claim ist die Adresse, von der aus der Peer
verbunden hat; dort hört niemand. Ein `send` an diese Adresse MUSS auf der
angenommenen Verbindung schreiben, in deren Dialekt, statt zu verbinden,
solange sie lebt. Was der Peer auf einer Verbindung schreibt, die der
Transport geöffnet hat, MUSS gelesen und mit der Adresse des Peers als
Quelle weitergegeben werden. Beides gilt für jeden Dialekt; im Dialekt
`zerodds` bleibt die Quelle einer Verbindung mit Claim der Listener des
Peers (§3.6), und ein `send` dorthin verbindet. Eine Nachricht, die der
Dialekt der angenommenen Verbindung nicht tragen kann, wird abgelehnt,
bevor ein Oktett geschrieben ist; ein gescheitertes Schreiben schließt die
Verbindung.

Die HeaderExtension mit `messageLength` des Dialekts `rtps` (§2.5) setzt
die Runtime je Ziel ein: in jede Nachricht an eine Verbindung dieses
Dialekts, auch an eine angenommene, wenn ihr eigener Dialekt ein anderer
ist, und in keine Nachricht an eine Verbindung eines anderen Dialekts.
Eine Nachricht, deren Schutz den Header authentisiert (Flag A: Pre-Shared
Key oder die Form mit „Additional Authenticated Data"), wird einmal für
alle ihre Ziele geschützt, und die Strecke kann die HeaderExtension hinter
diesem Schutz nicht einsetzen (§2.5). Die Runtime setzt sie vor dem Schutz
ein, in ihrem eigenen Dialekt `rtps` und immer dann, wenn ihr TCP-Listener
Verbindungen des Dialekts `rtps` annimmt (`transport.tcp_accept_dialects`,
ohne Angabe jeder Dialekt), und zwar in jede so geschützte Nachricht, über
welchen Transport sie auch geht. Eine Runtime eines anderen Dialekts
antwortet so auch unter einem solchen Schutz auf einer angenommenen
Verbindung des Dialekts `rtps`. Eine solche Nachricht an eine Verbindung
des Dialekts `cyclone` behält ihre HeaderExtension.

**Repo:** `crates/transport-tcp/src/cyclone_stream.rs` (`STREAM_HEAD_LEN`,
`MSG_LEN_SUBMESSAGE_ID`, `MAX_STREAM_MESSAGE_SIZE`, `MAX_MESSAGE_SIZE`,
`stream_head`, `read_body`, `read_message`, `check_message`,
`write_message`); `crates/transport-tcp/src/tcp_transport.rs`
(`TcpDialect::Cyclone`, `TcpDialect::max_message_size`, `Transport::send`
mit der Prüfung vor dem Verbindungsaufbau, `Transport::max_message_size`;
beide Richtungen: `AnswerTable`, `TcpTransport::answer_on`,
`send_on_accepted`, `answered_on_accepted`, `PeerConn::reading_into`,
`start_reading`, `closed_by_peer`, `Inbound::pump`, `Drop` für
`PeerConn`; der Dialekt je Ziel: `TcpTransport::dialect_towards`);
`crates/dcps/src/stream_length_leg.rs` (`StreamLengthLeg::for_tcp`,
`states_length_towards`, `for_destination`); `crates/dcps/src/runtime.rs`
(Wert `cyclone` des Schlüssels `transport.tcp_dialect`,
`parse_tcp_dialect`, `RuntimeConfig::tcp_kind_set` — der Dialekt `cyclone`
bringt die Locator-Arten 4 und 8 —, `RuntimeConfig::apply_tcp_settings`,
`RuntimeConfig::tcp_leg`; die HeaderExtension vor dem Schutz:
`RuntimeConfig::may_write_tcp_dialect_rtps`, `RuntimeConfig::uses_tcp`,
`stated_for_authenticated_stream`, `secure_outbound_bytes`).

**Tests:** `crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_msg_len_submessage_layout`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reads_what_cyclone_dds_wrote`
(die erste Nachricht, die ein Teilnehmer von Cyclone DDS 0.10.5 auf seine
TCP-Verbindung geschrieben hat, ohne und mit `TCP/Port`, aufgezeichnet auf
dem Linux-Testhost: 328 und 384 Oktette; erneut geschrieben ergibt die
gelesene Nachricht die Aufzeichnung),
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_the_message_handed_up_has_no_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reads_a_big_endian_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_a_first_submessage_other_than_msg_len`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_a_length_below_28_and_above_the_limit`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_takes_out_a_longer_length_submessage`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_reassembles_across_reads`
(bei Chunk-Größen 1, 3, 27, 28, 29 und 4096),
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_eof_inside_a_message_is_an_error`,
`crates/transport-tcp/src/cyclone_stream.rs::tests::tcp_4_1_refuses_to_write_what_cyclone_dds_does_not_read`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_two_transports_exchange_messages_in_the_cyclone_dialect`
(echte Sockets: ein roher Listener liest Header, `81 01 04 00`, Länge und
Rest je Nachricht; zwischen zwei Transporten kommen die Nachrichten ohne
Längen-Submessage an),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_listener_reads_what_cyclone_dds_wrote`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_send_to_the_source_of_an_accepted_connection_uses_that_connection`
(in den Dialekten `cyclone`, `rtps` und `zerodds` ohne Claim: der rohe
Peer liest die Antwort auf seiner Verbindung, der Pool bleibt leer, und
`dialect_towards` nennt für ihre Quelle den Dialekt der Verbindung und für
ein anderes Ziel den eigenen des Transports; nach dem Ende der Verbindung
nennt `dialect_towards` den eigenen Dialekt, und der `send` scheitert),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_a_claimed_connection_is_answered_at_the_claimed_listener`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_what_the_peer_writes_on_an_outgoing_connection_is_read`
(ein roher Listener antwortet auf der angenommenen Verbindung; zwischen
zwei Transporten in jedem Dialekt trägt eine Verbindung Hin- und
Rückweg),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_an_outgoing_connection_ends_with_its_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_1_send_refuses_what_cyclone_dds_does_not_read`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_1_the_cyclone_dialect_is_a_setting`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_runtime_answers_an_accepted_connection_in_its_dialect`
(eine Runtime in jedem der drei Dialekte und ein TCP-Peer ohne Listener,
der die aufgezeichnete SPDP-Nachricht schreibt: ein Peer des Dialekts
`rtps` liest die Antworten der Runtime mit einer HeaderExtension am Anfang
jeder Nachricht, auch von einer Runtime des Dialekts `zerodds` oder
`cyclone`; ein Peer des Dialekts `cyclone` liest sie ohne, auch von einer
Runtime des Dialekts `rtps`; kein Metatraffic-Send scheitert),
`crates/dcps/src/runtime.rs::tests::tcp_4_1_a_runtime_taking_the_rtps_dialect_states_the_length_before_a_pre_shared_key`
(eine Runtime des Dialekts `zerodds` mit einer TCPv4-Strecke, die jeden
Dialekt annimmt: eine Nachricht unter einem Pre-Shared Key ist `RTPS
header | HeaderExtension | SRTPS_PREFIX` mit A und P, messageLength ist
die Länge der geschützten Nachricht, und die Strecke findet nichts zu
ändern; mit einem Listener, der den Dialekt `rtps` nicht annimmt, wird die
Nachricht geschützt, wie sie ist; die Bedingung für jede Kombination aus
Dialekt, Transporten und angenommenen Dialekten).
Mit einem laufenden Cyclone DDS 0.10.5 (`ddsperf -T OU`,
`General/Transport` = `tcp`, also ohne UDP-Socket; der Teilnehmer von
ZeroDDS mit `transport.tcp_dialect: cyclone`, ohne SPDP-Multicast und ohne
UDP-Metatraffic-Locator):
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`
(Cyclone ohne `TCP/Port` verbindet zum Listener von ZeroDDS und wird auf
seiner Verbindung beantwortet; fünf Samples mit steigender Sequenznummer
kommen beim Reader an),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_with_a_listener_reaches_a_zerodds_reader`
(Cyclone mit `TCP/Port`; ZeroDDS verbindet über `discovery.peers`),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`
(`ddsperf sub` zählt die Samples des Writers von ZeroDDS).

**Status:** done — die drei Tests mit Cyclone DDS laufen im wöchentlichen Job `vendor-live-dcps` und sind auf dem Linux-Testhost von Hand grün (Cyclone DDS 0.10.5). Nicht gelaufen: TCPv6 mit Cyclone DDS; eine geschützte Verbindung (DDS-Security) mit Cyclone DDS über TCP, also auch, wie Cyclone DDS die HeaderExtension einer geschützten Nachricht einer Runtime aufnimmt, die den Dialekt `rtps` spricht oder annimmt; eine Runtime eines anderen Dialekts, die unter einem Pre-Shared Key auf einer angenommenen Verbindung des Dialekts `rtps` antwortet, über zwei Prozesse (geprüft ist die geschützte Nachricht, die die Runtime bildet); OpenSplice über TCP.

### §4.2 Auto-Detect am ersten Frame-Byte

**Spec:** §4 — die Server-Seite MUSS den Dialekt einer angenommenen
Verbindung an deren ersten Oktetten erkennen; ohne Erkennung müsste ein
Deployment die Betriebsart pro Port vorab konfigurieren. Ein Oktett
entscheidet es nicht: der Stream von Cyclone DDS (§4.1) und der Dialekt
`rtps` (§2.5) beginnen beide mit `RTPS`. Der Listener liest die ersten
vier Oktette unter der Frist des Handshakes (§3.8):

1. `ZDDS` ist der Request aus §3.1: der Rest des Handshakes läuft unter
   derselben Frist, danach folgen Frames nach §2.1.
2. `RTPS` ist die erste Nachricht eines Streams. Ihre erste Submessage
   (Oktett 20) nennt den Dialekt: `0x81` ist die Längen-Submessage von
   Cyclone DDS, `0x00` die HeaderExtension des Dialekts `rtps`. Jede
   weitere Nachricht der Verbindung MUSS denselben Dialekt haben; eine
   andere erste Submessage schließt die Verbindung.
3. `RTCP` ist der Header des Dialekts `fastdds` (§6.1): der
   BindConnectionRequest läuft unter derselben Frist, danach die
   Nachrichten der Verbindung.
4. Alles andere schließt die Verbindung.

§3.4 gilt für Verbindungen, die mit `ZDDS` beginnen; ein Peer, dessen erste
Oktette keinen angenommenen Dialekt beginnen, MUSS abgewiesen werden.
Welche Dialekte ein Listener annimmt, MUSS einstellbar sein
(`transport.tcp_accept_dialects`, Liste aus `zerodds`, `rtps`, `cyclone`,
`fastdds`);
ohne Einstellung nimmt er alle an. Eine Verbindung in einem nicht
angenommenen Dialekt wird geschlossen und gezählt. Die Stream-Dialekte
haben keinen Request und keine Response: die Vendor-Liste (§3.3) gilt für
die Vendor-ID im RTPS-Header jeder Nachricht, und eine Verbindung über dem
Peer-Limit (§3.7) wird ohne Antwort geschlossen und gezählt; nimmt der
Listener den Dialekt `zerodds` an, wartet er dafür höchstens 250 ms auf
einen Request, den er mit Reject beantworten kann. Der Dialekt, in dem
eine Verbindung erkannt wurde, ist der, in dem der Transport auf ihr
antwortet (§4.1).

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::with_accepted_dialects`, `accepted_dialects`,
`read_connection` — die vier Oktette unter `SERVER_HANDSHAKE_BUDGET` und
`SERVER_HANDSHAKE_SLICE` —, `read_zerodds_connection` mit `Prefixed`,
`read_message_stream`, `TcpTransportError::DialectNotAccepted`,
`refuse_beyond_limit`, `TcpDialect::ALL`, `name`, `from_name`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_accept_dialects`,
Schlüssel `transport.tcp_accept_dialects`, Umgebungsvariable
`ZERODDS_TCP_ACCEPT_DIALECTS`, `parse_tcp_dialect_list`,
`RuntimeConfig::apply_tcp_settings`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_one_listener_reads_a_peer_of_each_dialect`
(drei Verbindungen zugleich an einen Listener mit den
Default-Einstellungen, je eine der Dialekte `zerodds`, `rtps` und
`cyclone`: alle drei Nachrichten sind in der Queue),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_unknown_first_octets_are_refused`
(HTTP-Probe: `BadMagic` mit `GET `; eine RTPS-Nachricht, wie UDP sie
trägt: `NoMessageLength`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_detection_is_under_the_handshake_budget`
(der Peer sendet `R` und schweigt: `accept_one` kehrt in unter 1 s mit
`TimedOut` zurück),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_restricted_listener_refuses_the_other_dialects`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_an_unlisted_vendor_in_the_rtps_header_is_refused`
(die Aufzeichnung von Cyclone DDS, Vendor-ID `01 10`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_connection_keeps_the_dialect_of_its_first_message`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_4_2_a_stream_peer_beyond_the_limit_is_closed_unanswered`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_4_accept_one_rejects_peer_without_handshake`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_2_5_a_connection_that_is_no_rtps_stream_is_closed`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_2_the_accepted_dialects_are_a_setting`
(auch: die leere Liste und ein Name, der kein Dialekt ist, sind
Konfigurationsfehler),
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`
(der Listener von ZeroDDS erkennt die Verbindungen eines laufenden
Cyclone DDS).

**Status:** done

### §4.3 Metatraffic über die TCP-Strecke

**Spec:** §4 — ein Teilnehmer, der nur TCP hat (Cyclone DDS mit
`General/Transport` = `tcp`; ein Teilnehmer hinter einem Netz, das kein
UDP trägt), MUSS über TCP entdeckt werden und selbst entdecken können.
DDSI-RTPS 2.5 legt dafür nichts fest (§1.3). SPDP, SEDP, das
Liveliness-Protokoll, TypeLookup und die Builtin-Endpoints von
DDS-Security laufen dann über dieselben TCP-Verbindungen wie die
Nutzdaten:

1. Der Port der TCP-Listener einer Runtime MUSS einstellbar sein
   (`transport.tcp_port`; `0`, der Default, lässt das System wählen),
   damit ein Peer ihn nennen kann. TCPv4- und TCPv6-Listener einer Runtime
   halten denselben Port; ein belegter Port lässt den Start scheitern.
2. `discovery.peers` MUSS den TCP-Listener eines Peers nehmen:
   `tcp://ip:port` und `tcp://[ipv6]:port`, der Port ist Pflicht. Die
   SPDP-Nachricht geht in jeder Ankündigungsperiode über die TCP-Strecke
   dorthin, solange kein entdeckter Teilnehmer diesen Locator trägt.
3. Die Runtime MUSS ihre TCP-Listener als Metatraffic-Unicast-Locator
   ankündigen (`PID_METATRAFFIC_UNICAST_LOCATOR`), neben dem
   UDP-Metatraffic-Socket. `discovery.unicast_transports` (Liste aus `udp`
   und `tcp`, Default beide) wählt, was angekündigt wird; mit `tcp` allein
   hat ein Peer nur TCP als Ziel, und eine Runtime ohne TCP-Transport
   startet damit nicht.
4. Metatraffic an einen TCP-Locator MUSS über die TCP-Strecke gesendet
   werden; ein Send, der scheitert, wird gezählt.
5. Eine Nachricht, die über die TCP-Strecke ankommt und eine
   Builtin-Entität adressiert (EntityId mit den Builtin-Bits, DDSI-RTPS
   2.5 §9.3.1.2) oder geschützte Submessages trägt, MUSS den Weg einer
   Nachricht des Metatraffic-Sockets nehmen, der auch die Nutzer-Entitäten
   bedient. Das gilt unabhängig von `discovery.unicast_transports`. Eine
   Nachricht mit Nutzdaten allein behält ihren Weg.
6. Ein Teilnehmer, dessen SPDP-Nachricht über eine TCP-Verbindung kam und
   der keinen Unicast-Locator ankündigt, MUSS unter der Quelle dieser
   Verbindung geführt werden, je Liste (Default und Metatraffic) für sich;
   die Antworten gehen dann auf seine Verbindung (§4.1). So verfährt
   Cyclone DDS (`ddsi_discovery_spdp.c`, `get_locators`). Eine Quelle, die
   kein TCP-Locator ist, ändert nichts.
7. Im Dialekt `cyclone` tragen die angekündigten TCP-Locatoren die Arten 4
   und 8 (§1.1); andere liest Cyclone DDS nicht.

Zwischen zwei Teilnehmern, die UDP und TCP ankündigen, geht Metatraffic
über beide Wege; das zuverlässige Protokoll verwirft die Doppel.

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_port`,
Schlüssel `transport.tcp_port`, Umgebungsvariable `ZERODDS_TCP_PORT`,
`select_user_transport`, `select_tcp_transport`, `select_tcp_pair` — beide
TCP-Listener auf einem Port, auch einem vom System gewählten —;
`parse_tcp_peer`, `expand_initial_peer`,
Schlüssel `discovery.peers`; `RuntimeConfig::metatraffic_over_udp`,
`metatraffic_over_tcp`, Schlüssel `discovery.unicast_transports`,
Umgebungsvariable `ZERODDS_DISCOVERY_UNICAST_TRANSPORTS`,
`parse_unicast_transports`; `DcpsRuntime::start` — die angekündigten
Metatraffic-Locatoren —, `DcpsRuntime::send_metatraffic`,
`send_spdp_unicast`, `metatraffic_tcp_send_errors`;
`recv_user_data_loop`, `may_address_builtin_entity`,
`dispatch_metatraffic_datagram`; `adopt_connection_source`);
`crates/transport-tcp/src/tcp_transport.rs`
(`TcpTransport::bind_v6_only`).

**Tests:** `crates/dcps/src/runtime.rs::tests::tcp_4_3_two_runtimes_discover_each_other_over_tcp_alone`
(zwei Runtimes ohne SPDP-Multicast und ohne UDP-Metatraffic-Locator, eine
kennt den TCP-Listener der anderen als Initial Peer: jede führt die
andere nur mit TCP-Locatoren, und die SEDP-Writer haben den Reader der
anderen als Proxy),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_cyclone_participant_without_a_listener_is_answered_on_its_connection`
(ein TCP-Peer schreibt die aufgezeichnete SPDP-Nachricht von Cyclone DDS
0.10.5 auf eine Verbindung zum Listener: die Runtime führt den Teilnehmer
unter der Quelle der Verbindung und schreibt ihre eigene SPDP-Nachricht im
Stream-Framing von Cyclone DDS auf dieselbe Verbindung),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_the_spdp_announcement_goes_to_a_tcp_initial_peer`
(ein roher Listener liest die SPDP-Nachricht; sie trägt den Listener der
Runtime mit Art 4 und dem Port aus `transport.tcp_port` als Default- und
als Metatraffic-Unicast-Locator),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_the_metatraffic_locators_follow_the_unicast_transports`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_message_for_a_builtin_entity_is_told_from_user_traffic`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_an_empty_locator_list_takes_the_source_of_the_connection`,
`crates/dcps/src/runtime.rs::tests::tcp_4_3_a_tcp_peer_entry_expands_to_a_tcp_locator`,
`crates/dcps/src/runtime.rs::tests::tcp_4_1_the_cyclone_dialect_and_the_listener_settings_reach_the_transport`
(TCPv4- und TCPv6-Listener auf demselben gesetzten Port),
`crates/dcps/src/runtime.rs::tests::tcp_4_3_both_tcp_listeners_hold_one_port_of_the_systems_choosing`
(mit `transport.tcp_port` 0 und beiden Reihenfolgen in
`transport.user_transports`: TCPv4- und TCPv6-Listener haben denselben
Port, und jeder nimmt auf ihm eine Verbindung seiner Adressfamilie an),
`crates/dcps/src/runtime.rs::tests::tcp_6_1_both_tcp_listeners_of_the_fastdds_dialect_hold_one_port`
(dasselbe im Dialekt `fastdds`, dessen Locator-Port den logischen Port
über dem Port des Listeners trägt),
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_the_listener_port_is_a_setting`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_discovery_peers_take_tcp_locators`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_4_3_the_unicast_transports_are_a_setting`,
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv4_without_udp_discovery`
(zwei Prozesse ohne UDP-Weg zueinander, RELIABLE und KEEP_ALL: der Writer
findet den Reader über dessen TCP-Listener, alle 200 Samples kommen
einmal und in Reihenfolge an; der Kernel zeigt den Listener auf dem Port
der Einstellung und die TCP-Verbindung),
`tests/integration/tests/dcps_transports.rs::dcps_over_tcpv6_without_udp_discovery`
(dasselbe über TCPv6, der Peer-Eintrag `tcp://[::1]:port`),
`tests/integration/tests/security.rs::security_over_tcp_alone`
(zwei Prozesse mit DDS-Security, `rtps_protection_kind` ENCRYPT, nur über
TCP im Dialekt `rtps`: Authentisierung, Schlüsselaustausch, geschütztes
SEDP und alle 50 Samples gehen über die TCP-Verbindung; `strace` zeigt
jede Nachricht des Schreibers auf dem Stream als ganz geschützt oder als
RTPS Bootstrapping Message (DDS-Security 1.2 §7.5.7 und §9.4.2.4
Tabelle 33, Attribut is_rtps_axk_protected) und keine RTPS-Nachricht auf einem
UDP-Socket außer den Weck-Nachrichten an die eigenen Sockets),
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_without_a_listener_reaches_a_zerodds_reader`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_cyclone_publisher_with_a_listener_reaches_a_zerodds_reader`,
`crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`
(Discovery und Samples mit einem Cyclone DDS, das keinen UDP-Socket hat).

**Status:** done — die Tests mit Cyclone DDS laufen im wöchentlichen Job `vendor-live-dcps` und sind auf dem Linux-Testhost von Hand grün (Cyclone DDS 0.10.5). Nicht gelaufen: Metatraffic über TCP mit einem anderen Hersteller als Cyclone DDS; DDS-Security über TCP mit einem anderen Hersteller.

## §5 Connection-Pool

### §5.1 Connection-Reuse und Lazy Connect

**Spec:** §5 — pro Ziel-Socket-Adresse hält der Transport genau eine
ausgehende Verbindung, die beim ersten `send` aufgebaut und danach
wiederverwendet wird. Der Pool-Lock darf nicht über die blockierende I/O
gehalten werden. Hat der Peer die Verbindung geschlossen und ist das vor
dem `send` angekommen, MUSS der `send` im selben Aufruf neu verbinden,
statt in die geschlossene Verbindung zu schreiben; das Schließen zählt
nicht als Verbindungsfehler und löst keinen Backoff aus. Ein Schließen,
das erst nach der Prüfung eintrifft, erkennt der folgende `send`. `Ok`
von `send` heißt, dass der Frame dem TCP-Stack übergeben ist, nicht, dass
der Peer ihn erhalten hat: ein Frame, der in die Verbindung geschrieben
wurde, bevor ihr Schließen auf diesem Host eingetroffen ist, kann verloren
sein, ohne dass sein `send` einen Fehler gemeldet hat, und er wird nicht
erneut gesendet. Das ist zulässig, weil RTPS vom Transport keine
Zustellung verlangt (DDSI-RTPS 2.5 §7.6: „the transport need not guarantee
each packet will reach its destination"); die Wiederholung liegt beim
zuverlässigen RTPS-Protokoll über dem Transport. Der Verlust darf nicht
unsichtbar bleiben: der Transport MUSS jede ausgehende Verbindung zählen,
die ein `send` vom Peer geschlossen vorfindet und ersetzt. Wie viele
Frames einer gezählten Verbindung verloren sind, ist auf der sendenden
Seite nicht bekannt; eine Verbindung, nach deren Schließen kein `send`
mehr folgt, wird nicht gezählt.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::ensure_connected`, `PeerConn::send` mit `peer_closed` — ein
nicht blockierender Blick auf die Empfangsseite vor dem Schreiben, unter
Unix ein Syscall (`recv` mit `MSG_PEEK | MSG_DONTWAIT`) —,
`TcpTransport::peer_closed_connections`, `Transport::send`: Schritt 1
nimmt den Pool-Lock nur für Lookup/Insert, Schritt 2 den Peer-Lock für
die I/O).

**Tests:** `crates/transport-tcp/tests/loopback.rs::tcp_1_2_loopback_single_frame_end_to_end`
(zwei Sends über dieselbe wiederverwendete Verbindung),
`crates/transport-tcp/tests/loopback.rs::tcp_5_1_reconnect_after_peer_restart`
(der Peer endet und ein zweiter bindet denselben Port; ein nach dem
Neustart gesendeter Frame kommt innerhalb von 2 s beim zweiten an),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_send_after_peer_close_reconnects_instead_of_writing_into_the_closed_connection`
(der Frame kommt auf der zweiten Verbindung an, der Pool hält einen
Eintrag, Backoff 0, der Zähler steht auf 1),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_a_frame_written_before_the_close_arrived_is_lost_and_the_connection_counted`
(der Peer schließt, ohne die zwei geschriebenen Frames zu lesen: deren
`send` hat `Ok` geliefert und nichts gezählt, der folgende `send` zählt
die Verbindung, und die zweite Verbindung beginnt mit dem Frame dieses
`send`),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_the_look_for_a_close_takes_no_octet_and_keeps_the_socket_blocking`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_a_stalled_peer_does_not_hold_sends_to_another_peer`
(Peer A nimmt die Verbindung an und liest nie, ein Send an ihn blockiert;
ein Send an Peer B kehrt in unter 1 s zurück),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_1_send_unsupported_locator_does_not_pollute_pool`.

**Status:** done

### §5.2 Exponentieller Backoff nach Verbindungsfehler

**Spec:** §5 — nach einem fehlgeschlagenen Connect MUSS der nächste
Versuch gedrosselt werden (50 ms, verdoppelnd, Deckel 5 s), damit ein
toter Peer keine Connect-Schleife auslöst. Ein erfolgreicher Connect
setzt den Backoff zurück. Nach einem Reject mit `ResourceLimit` beträgt
die Wartezeit mindestens 1 s (§3.7).

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`PeerConn::bump_backoff`, `INITIAL_BACKOFF_MS = 50`,
`MAX_BACKOFF_MS = 5_000`, `RESOURCE_LIMIT_BACKOFF_MS = 1_000`,
Rücksetzung in `ensure_connected`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_peer_conn_bump_backoff_sequence`
(50 → 100 → … → 5000, dann konstant),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_peer_conn_backoff_throttles_connect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_2_backoff_resets_after_successful_connect`
(der Listener bleibt über den ganzen Test gebunden: die erste Verbindung
schließt er ohne Antwort, der Backoff steht auf 50 ms; die zweite
beantwortet er, danach steht der Backoff auf 0 und der Frame ist
angekommen),
`crates/transport-tcp/tests/loopback.rs::tcp_5_2_backoff_prevents_tight_loop_on_dead_peer`
(zweiter Send an einen toten Port kehrt in unter 20 ms zurück),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_3_7_the_client_counts_the_resource_limit_reject_and_backs_off_one_second`
(1000 ms nach `ResourceLimit`).

**Status:** done

### §5.3 Peer-Cap mit deterministischer Eviction

**Spec:** §5 — der Pool ist auf `MAX_PEERS = 256` gedeckelt. Ist er voll
und kommt ein neues Ziel dazu, wird deterministisch der kleinste
Schlüssel verdrängt; ein bereits enthaltenes Ziel löst keine Eviction aus.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_PEERS`, Eviction-Zweig in `Transport::send`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_3_max_peers_eviction_removes_first_key_on_new_peer`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_3_max_peers_no_eviction_for_existing_peer`.

**Status:** done

### §5.4 Begrenzte Inbound-Queue mit gezähltem Drop

**Spec:** §5 — die Empfangs-Queue ist auf `MAX_INBOUND_QUEUE = 1024`
gedeckelt. Läuft sie über, wird der älteste Frame verworfen **und
gezählt**; ein stiller Verlust ist nicht zulässig. `recv` wartet per
Condvar und wacht beim Push auf, ohne Poll-Schleife. Kommt eine Sekunde
lang kein Frame, kehrt `recv` mit `RecvError::Timeout` zurück, damit ein
Thread, der auf `recv` schleift, sein Stopp-Signal sieht; nach `close`
kehrt `recv` sofort mit `RecvError::Io` zurück.

**Repo:** `crates/transport-tcp/src/tcp_transport.rs`
(`MAX_INBOUND_QUEUE`, `push_inbound`, `dropped_frames`,
`Transport::recv` mit `Condvar::wait_timeout` von 1 s, `close`).

**Tests:** `crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_inbound_overflow_drops_oldest_and_counts`
(1029 nummerierte Frames: der Zähler steht auf 5, die Queue liefert die
Nummern 5 bis 1028 in Reihenfolge),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_dropped_frames_zero_on_fresh_transport`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_recv_wakes_on_push_from_other_thread`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_push_preserves_fifo_order`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_an_idle_recv_times_out`
(`Timeout` nach 0,95 bis 3 s),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_5_4_a_closed_transport_neither_sends_nor_waits`.

**Status:** done

## §6 Cross-Vendor-Erweiterungspunkte

### §6.1 Fast-DDS-TCP (RTCP)

**Spec:** §6 — Fast DDS spricht über TCP ein eigenes Protokoll, das es
RTCP nennt (Fast DDS 3.6.1, `src/cpp/rtps/transport/tcp/RTCPHeader.h`,
`RTCPMessageManager.cpp`, `TCPControlMessage.cpp`,
`TCPTransportInterface.cpp`, `TCPChannelResource.cpp`,
`src/cpp/utils/IPLocator.cpp`). Es trägt Request und Response nicht als
RTPS-Submessages `0x71`/`0x72`, wie dieser Text früher sagte, sondern als
Kontrollnachrichten mit eigenem Header. Für Interop mit dem TCP-Transport
von Fast DDS MUSS dieses Protokoll als Dialekt wählbar sein
(`transport.tcp_dialect: fastdds`); der Handshake aus §3 bleibt Default.

Vor jeder Nachricht, RTPS oder Kontrolle, steht ein Header von 14
Oktetten:

```text
 0  'R' 'T' 'C' 'P'
 4  length (u32)         Header und Rest
 8  crc (u32)            Summe der Oktette hinter dem Header, Übertrag
                         zurückaddiert (Einerkomplement); 0 = nicht berechnet
12  logical_port (u16)   0 = Kontrollnachricht
```

Fast DDS schreibt und liest die drei Felder in der Byte-Reihenfolge
seines Hosts, ohne Kennzeichen; ZeroDDS schreibt und liest sie
little-endian, die Reihenfolge von x86-64 und AArch64. Eine
Kontrollnachricht hat einen Header von 16 Oktetten:

```text
 0  kind (u8)            D1..D6 Requests, E1..E4 Responses
 1  flags (u8)           0x02 little-endian, 0x04 Payload, 0x08 Antwort erwartet
 2  length (u16)         Kontroll-Header und Rest
 4  transaction_id (12)
16  [code (u32)]         nur Responses: 0 Ok, 1 BadRequest,
                         2 ExistingConnection, 3 InvalidPort,
                         4 UnknownLocator, 5 IncompatibleVersion,
                         6 ServerError
    [payload]            encapsulation (u16), length (u32), CDR-Daten mit
                         eigenem Encapsulation-Header (00 01 00 00)
```

Die Arten: `0xD1`/`0xE1` BindConnection (Payload des Requests:
Protokollversion 1.0, Vendor-ID, Locator des Listeners des Senders; der
Response: Locator der Gegenseite), `0xD2`/`0xE2` OpenLogicalPort (ein
logischer Port), `0xD3`/`0xE3` CheckLogicalPorts (Liste logischer Ports),
`0xD4`/`0xE4` KeepAlive (Locator), `0xD5` LogicalPortIsClosed (ein
logischer Port), `0xD6` UnbindConnection (ohne Payload). Ein Locator ist
`kind` (i32), `port` (u32) und 16 Oktette Adresse; die Arten sind 4
(TCPv4) und 8 (TCPv6). `port` trägt den physischen Port (TCP-Port des
Listeners) in den unteren und den logischen Port in den oberen 16 Bit;
in einem TCPv4-Locator sind die Oktette 8 bis 11 der Adresse die
WAN-Adresse, 12 bis 15 die Adresse im eigenen Netz.

Ablauf: die verbindende Seite sendet als erste Nachricht den
BindConnectionRequest und wartet unter der Frist aus §3.8 auf die
Response; die annehmende Seite MUSS als erste Nachricht diesen Request
erwarten, eine andere Protokollversion mit `IncompatibleVersion`
abweisen und mit dem Locator ihres Listeners antworten. Vor der ersten
RTPS-Nachricht an einen logischen Port fragt die sendende Seite mit
OpenLogicalPortRequest; die Gegenseite antwortet `Ok`, wenn der Port bei
ihr offen ist, sonst `InvalidPort`. Beide Seiten senden und fragen auf
derselben Verbindung. Eine RTPS-Nachricht an einen Port, der nicht offen
ist, MUSS gezählt und mit LogicalPortIsClosedRequest beantwortet werden
(Fast DDS verwirft sie mit einer Logzeile). Kontrollnachrichten werden
beantwortet wie Fast DDS sie beantwortet: CheckLogicalPorts mit den
offenen Ports der Liste, KeepAlive mit `Ok`, wenn der logische Port des
Locators 0 ist, sonst `UnknownLocator`, eine unbekannte Art und eine
falsche Länge mit derselben Art und `BadRequest`; LogicalPortIsClosed
nimmt den Port aus den geöffneten, und der nächste Versand fragt neu;
UnbindConnection beendet die Verbindung regulär, und ein Transport, der
schließt, sendet sie auf jeder Verbindung. Eine Prüfsumme, die weder 0
noch die Summe ist, eine Nachricht ohne `RTCP` und eine Länge unter 14
schließen die Verbindung. Fast DDS liest eine RTPS-Nachricht über seiner
`maxMessageSize` (65500) nicht; ein Versand darüber MUSS mit
`PayloadTooLarge` abgewiesen werden.

Ein Locator der Art 4 oder 8 mit einem Port über 65535 ist einer von Fast
DDS und MUSS in diesem Dialekt erreicht werden, auch von einem Transport
eines anderen Dialekts; im Dialekt `fastdds` ist ein Ziel ohne logischen
Port ein Fehler. Eine Adresse des eigenen Hosts gilt als Loopback, wie
Fast DDS alle Adressen seines Hosts als einen nimmt und eine zweite
Verbindung zwischen zwei Teilnehmern mit `ServerError` abweist. Die
Adressen stammen aus den Locatoren, die Peers ankündigen; der Transport
behält seine Antwort für höchstens so viele Adressen, wie sein Pool Peers
hat (256), und fragt für eine vergessene erneut. Ist die
WAN-Adresse gesetzt und nimmt die Adresse im Netz keine Verbindung an,
wird die WAN-Adresse versucht. Die eigenen Locatoren im Dialekt
`fastdds` haben die Arten 4 und 8 und als logischen Port den
Metatraffic-Unicast-Port des Teilnehmers (DDSI-RTPS 2.5 §9.6.1.4.1);
offen sind dieser und der Nutzdaten-Unicast-Port. Ein Eintrag
`tcp://ip:port` in `discovery.peers` wird im Dialekt `fastdds` an den
Metatraffic-Unicast-Ports der ersten Teilnehmer-IDs gefragt, wie Fast DDS
einen Initial Peer ohne logischen Port fragt
(`configureInitialPeerLocator`); `tcp://ip:port/logischer-port` nennt
den Port. Die Vendor-Liste (§3.3) gilt für die Vendor-ID des
BindConnectionRequest; ein nicht genannter Vendor wird mit `ServerError`
abgewiesen.

**Repo:** `crates/transport-tcp/src/rtcp.rs` (`TcpHeader`, `crc`,
`ControlMessage`, `ResponseCode`, `kind`, `WireLocator`,
`ConnectionRequest`, `encode_connection_request`, `encode_locator`,
`encode_port`, `encode_ports` und die Gegenstücke, `read_message`,
`data_message`, `MAX_MESSAGE_SIZE`, `Link::connect`, `Link::accept`,
`Link::run`, `Link::send`, `Link::unbind`, `RtcpError`);
`crates/transport-tcp/src/tcp_transport.rs` (`TcpDialect::FastDds`,
`destination`, `TcpTransport::host_alias`, `with_logical_port`,
`set_logical_ports`, `logical_ports`, `listener_port`, `with_vendor_id`, `local_locator`,
`read_rtcp_connection`, `answer_on_link`, `send_on_accepted`,
`PeerConn::start_reading_link`, `rtcp_source`, `rtcp_closed_port_messages`,
`rtcp_refused_ports`, `rtcp_unmatched_responses`, `rtcp_bad_requests`,
`close`); `crates/dcps/src/runtime.rs` (`transport.tcp_dialect: fastdds`,
`RuntimeConfig::tcp_kind_set`, `fastdds_initial_peers`,
`parse_tcp_peer` mit `/logischer-port`, die logischen Ports der
TCP-Listener beim Start); `tests/interop/ci/fastdds/fastdds_shapes.cpp`
(Option `tcp`: der TCPv4-Transport von Fast DDS allein, Listener-Port,
Initial Peers).

**Tests:** `crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_rtcp_header_layout`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_bind_connection_request_layout`
(beide gegen den BindConnectionRequest, den Fast DDS 3.6.1 auf dem
Linux-Testhost schrieb, `tests/data/fastdds_3_6_1_rtcp_bind_request.hex`:
der Encoder schreibt dieselben Oktette),
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_listener_answers_of_fast_dds_decode`
(BindConnectionResponse und zwei OpenLogicalPortResponses eines Listeners
von Fast DDS 3.6.1, `Ok` und `InvalidPort`, Oktett für Oktett),
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_crc_adds_the_carry_back_in`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bad_crc_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_stream_without_rtcp_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_control_message_of_a_wrong_length_is_answered_bad_request`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_locator_port_splits_into_physical_and_logical`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_cdr_payloads_round_trip`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_open_logical_port_exchange`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_the_messages_reach_the_reader_with_their_port`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_closed_logical_port_is_reopened`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_keep_alive_and_check_logical_ports_are_answered`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bind_of_another_version_is_refused`,
`crates/transport-tcp/src/rtcp.rs::tests::tcp_6_1_a_bind_without_an_answer_times_out`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_locator_of_the_dialect_holds_the_logical_port`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_an_address_of_this_host_is_reached_at_loopback`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_answers_on_host_addresses_are_bounded`
(762 Adressen der Dokumentationsbereiche lassen 256 Antworten übrig, die
zuletzt erfragten; eine Adresse dieses Hosts, deren Antwort vergessen ist,
wird erneut erfragt und über Loopback erreicht),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_two_transports_exchange_messages_in_the_fastdds_dialect`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_the_dialect_is_chosen_by_the_locator`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_listener_answers_what_fast_dds_wrote`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_listener_refuses_a_dialect_or_vendor_it_does_not_take`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_whose_address_refuses_is_reached_at_its_wan_address`
(die Adresse im Netz weist ab, die WAN-Adresse nimmt an),
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_an_entry_of_another_dialect_in_use_is_not_taken_for_a_fast_dds_destination`,
`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_logical_port_on_a_connection_of_another_dialect_is_refused`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_the_fastdds_dialect_reaches_the_transport_with_the_logical_ports`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_a_tcp_peer_of_the_fastdds_dialect_is_asked_at_the_metatraffic_ports`,
`crates/dcps/src/runtime.rs::tests::tcp_6_1_both_tcp_listeners_of_the_fastdds_dialect_hold_one_port`,
`crates/dcps/tests/runtime_config_layering.rs::tcp_6_1_the_fastdds_dialect_is_a_setting`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_fastdds_tcp_publisher_reaches_a_zerodds_subscriber`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_a_fastdds_client_without_a_listening_port_is_answered_on_its_connection`,
`crates/dcps/tests/fastdds_live_tcp.rs::tcp_6_1_zerodds_publisher_reaches_a_fastdds_tcp_subscriber`
(gegen ein laufendes Fast DDS 3.6.1, `fastdds_shapes` mit der Option
`tcp`, ohne UDP; Job `vendor-live-dcps`).

**Status:** done — die zitierten Tests in `zerodds-transport-tcp` und `zerodds-dcps` sind auf dem Linux-Testhost grün; `fastdds_live_tcp` 3 von 3 gegen Fast DDS 3.6.1, aus dem Quelltext des CI-Image-Tags gebaut. Nicht geprüft: ein Fast-DDS-Host mit Big-Endian-Byte-Reihenfolge, TCPv6 mit Fast DDS, die WAN-Adresse über NAT (der Rückgriff auf die WAN-Adresse ist auf einem Host geprüft: die Adresse im Netz weist ab, die WAN-Adresse ist eine Loopback-Adresse), die TLS-Schicht von Fast DDS.

### §6.2 RTI-TLS-orientierter Handshake

**Spec:** §6 — RTI DDS TCP nutzt einen TLS-orientierten Bring-up. Für
Interop mit RTI MUSS dieser Dialekt als zusätzliche Betriebsart wählbar
sein; der ZeroDDS-eigene Handshake aus §3 bleibt Default.

**Repo:** —

**Tests:** —

**Status:** open — keine Betriebsart mit dem Protokoll von RTI Connext. Es fehlen: das TCP-Protokoll von RTI (eigene Kontrollnachrichten, Kontroll- und Datenverbindungen getrennt, die Klassen-IDs `tcpv4_lan`/`tcpv4_wan`/`tlsv4_lan`/`tlsv4_wan` in der Locator-Art), die TLS-Schicht darunter und ein Test gegen RTI Connext. Der Text oben ist nicht an RTI geprüft; RTI veröffentlicht keine Spezifikation des Protokolls. Auf dem Linux-Testhost dieser Einheit ist kein RTI Connext installiert, gegen das eine Umsetzung geprüft werden könnte.

---

## Audit-Status

22 done / 1 partial / 1 open / 0 blocked / 1 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-tcp-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (Job `merge-rust-rtps`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-transport-tcp --all-targets` (Job `merge-rust-transport-tcp`) — 45 Tests grün.

46 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-tcp-transport-1.0.open.md`.
