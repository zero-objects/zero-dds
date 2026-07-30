# `zerodds-endpoint-c` v1.0 — Natives C-Endpoint-SDK (XRCE-Framing, Sync/Async, Reliable Stream)

ZeroDDS Vendor-Spec. Implementiert in `endpoints/c/`.

Baut auf [ADR 0013](../adr/0013-native-endpoint-sdks.md) (Native Endpoint-SDKs,
Frame-Hook-Vertrag) und DDS-XRCE 1.0 §8.3 (Framing) sowie §8.4.10/§8.4.11
(Reliable Streams) auf. Der Reliable-Delivery-Kontrakt selbst ist getrennt
normiert in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md).

## §1 XRCE-Framing

Der C-Endpoint MUSS DDS-XRCE-1.0-konforme **Framing-Struktur** — Message-Header
(§8.3.2) und Submessage-Header (§8.3.4) — sowohl für paketbasierte Transporte
(UDP) als auch für serielle Byte-Strom-Transporte (RS-485, UART) erzeugen und
parsen.

Der **Datensubmessage-Payload** des C-Endpoints ist derzeit das *ZeroDDS
Endpoint Profile* (§1.1): der WRITE_DATA-/DATA-Body ist der reine XCDR-Sample,
ohne den von DDS-XRCE §8.3.5.8/§7.7.8 geforderten `BaseObjectRequest`
(`request_id` + `object_id`) vor den Nutzdaten. Ein fremder XRCE-Agent kann
diesen Frame damit keinem DataWriter zuordnen — der Frame ist auf Header-Ebene
konform, auf Payload-Ebene noch nicht.

Die spec-konforme DDS-XRCE-Wire-Form der Datensubmessages (WRITE_DATA/READ_DATA/
DATA mit vorangestelltem `BaseObjectRequest`) ist im Rust-Ursprung `crates/xrce`
implementiert (`WriteDataPayload`/`ReadDataPayload`/`DataPayload`) und über einen
client↔agent-Roundtrip belegt (`crates/endpoint-e2e/src/lib.rs`,
`endpoints/xrce-agent-demo`). Die Portierung dieses C-Endpoints auf die
konforme Payload-Form ist **Phase 2**.

### §1.1 WRITE_DATA-Frame (ZeroDDS Endpoint Profile)

Message-Header (`session_id`, `stream_id`, `sequence_nr` LE — DDS-XRCE 1.0
§8.3.2.3) gefolgt vom WRITE_DATA-Submessage-Header (id=7, Flags, Länge LE —
§8.3.4) gefolgt vom XCDR-Sample-Body. Best-effort-Betrieb ohne ClientKey:
`session_id ≥ 128`. Dies ist die Profile-Form ohne `BaseObjectRequest`; die
konforme DDS-XRCE-Form ergänzt `request_id` + `object_id` vor dem Sample
(§8.3.5.8).

### §1.2 Empfangspfad (DATA-Message)

Der Endpoint MUSS DATA-Submessages (id=9, §8.3.4), die der Agent an den
Client pusht, über denselben Unwrap-Pfad wie WRITE_DATA parsen können — in der
Profile-Form ohne, in der konformen Form (Phase 2) mit vorangestelltem
`BaseObjectRequest`.

### §1.3 Serielles HDLC-Framing (Annex C)

Für serielle Byte-Strom-Transporte MUSS der Endpoint das DDS-XRCE-1.0-
Annex-C-Framing implementieren: `7E [byte-stuffed(payload)
byte-stuffed(crc16-BE)] 7E`, Byte-Stuffing von `0x7E`/`0x7D` (RFC 1662),
CRC-16-CCITT-FALSE (Init `0xFFFF`, Polynom `0x1021`) über den rohen Payload.

## §2 sync send/recv

### §2.1 Frame-Hook-Vertrag

Der Endpoint ist transport-opak (ADR 0013, Invariante 5): er reicht ein
vollständig geframtes, encodiertes Message an einen vom Integrator gefüllten
Transport (Context-Pointer + `deliver`/`receive`-Funktionspointer) und
erhält vollständige Frames zurück. Encode/Send und Receive/Decode laufen
symmetrisch über denselben Hook.

### §2.2 Poll-Loop-Idiom (C89)

Die Baseline-Nutzung ist synchron: der Integrator besitzt den Run-Loop und
ruft die Empfangs-Funktion selbst auf (Poll). Der C89-Kern darf keine
C99/C11-Syntax voraussetzen.

## §3 async Reader/Writer

### §3.1 Event-driven Reactor

Additiv zum C89-Kern MUSS eine callback-getriebene, latenz-entkoppelte
Alternative zum Poll-Loop existieren (C11, kein `malloc` im Reactor-Pfad):
ein Reader, der einen `on_sample`-Callback bindet und beim Drainen des
Transports jedes dekodierte Sample dispatcht; ein Writer mit äquivalentem
asynchronem Write-Pfad.

### §3.2 Live-Transport

Der Reactor MUSS nachweislich über echte Datagramm-I/O (POSIX-UDP,
nicht-blockierend) funktionieren, nicht nur über eine In-Memory-Queue.

### §3.3 Async-Deep-Example

Ein lauffähiges Beispiel (kein Stub) MUSS den Reactor end-to-end zeigen.

### §3.4 Test-Pflicht: Live-Ping-Pong-E2E

Zusätzlich zu den isolierten Unit-/Loopback-Tests aus §1–§3 MUSS jede
Sprache eine Live-E2E-Datei nach dem Muster
`crates/endpoint-e2e/tests/<lang>.rs` bereitstellen (Referenz: `cpp.rs`):
raw/sync/async-Modi über einen echten POSIX-Socket gegen den generierten
`Ping`/`Pong`-Codec, damit Framing (§1), sync (§2) und async (§3) im
Zusammenspiel — nicht nur isoliert — belegt sind.

## §4 reliable Stream

Der reliable Stream ist ein eigenständiges Bauteil und normativ vollständig
in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md) spezifiziert
(State-Machine-Kontrakt §3, Wire-Format §4, Test- & Beleg-Pflicht §5). Der
C-Endpoint MUSS diesen Kontrakt implementieren:

- eine C89-State-Machine (fester Storage, kein `malloc`) für Sender und
  Receiver gemäß §3, inklusive der Konstanten aus §3.1;
- byte-identische HEARTBEAT-/ACKNACK-Wire-Codecs gemäß §4;
- einen latenz-entkoppelten async Writer, dessen Drain-Task den reliable
  Sender-State trägt (§2) — realisiert über einen wait-freien Ring plus
  dedizierten Drain-Thread (hier: SPSC-Ring + pthread, C11);
- die vollständige Test- und Beleg-Pflicht aus §5: Unit-Tests, Byte-Golden,
  E2E-Loss-Recovery gegen den Rust-Referenz-Peer, ein Latenz-Bench-Messwert
  (entkoppeltes Ring-Enqueue vs. inline `send()`), und ein lauffähiges
  `example_reliable_*`.
