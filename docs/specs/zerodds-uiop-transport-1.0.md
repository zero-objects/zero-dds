# `zerodds-uiop-transport` v1.0 — UIOP: GIOP über Unix-Domain-Sockets

ZeroDDS Vendor-Spec. In `crates/corba-iiop` + `crates/corba-interop` implementiert.
Authored im Stil der OMG-CORBA-Spec (nummerierte Klauseln, RFC-2119-Keywords,
Konformitätsprofil). Konsistent mit der DDS-seitigen `zerodds-uds-transport-1.0`
und ADR-0001 (Vendor-Spec-Strategie).

## Motivation

GIOP/IIOP (OMG CORBA 3.4 §15.7) normiert ausschließlich TCP/IP als Transport.
Für **same-host-IPC** (Client und Server auf derselben Maschine) zahlt der
loopback-TCP-Pfad unnötigen Overhead: IP-Stack, Port-Allokation, Nagle/ACK-Logik,
Firewall-Hooks. omniORB und TAO bieten dafür proprietäre Unix-Domain-Socket-
Transporte (`giop:unix` bzw. TAO-`uiop`) an — beide Vendor-spezifisch und
untereinander nicht interoperabel.

ZeroDDS definiert **UIOP** als orthogonalen, klar abgegrenzten Vendor-Transport:
GIOP-1.0/1.1/1.2-Messages — byte-identisch zum IIOP-Wire — über einen
`AF_UNIX`-`SOCK_STREAM`. Nur der Transport-Unterbau wechselt; der GIOP-Codec,
CDR, POA-Dispatch und die Codeset-Negotiation bleiben unverändert.

## Ziele

- **GIOP-wire-identisch**: dieselben Request/Reply/LocateRequest/Fragment-Bytes
  wie über IIOP; nur der Stream ist ein `UnixStream` statt `TcpStream`.
- **Annoncierung im IOR**: ein Vendor-`TaggedComponent` `TAG_ZERODDS_UDS_TRANS`
  trägt den Socket-Pfad; der ProfileBody bleibt ein gültiges
  `TAG_INTERNET_IOP`-Profil (Host `localhost`, Port 0).
- **Graceful Ignorierung**: Fremd-ORBs (omniORB/TAO/JacORB) ignorieren die
  unbekannte Component und nutzen — falls vorhanden — das TCP-Profil; eine reine
  UIOP-IOR ist für sie nicht aufrufbar (per Design, same-host-only).
- **Connection-Pooling**: UIOP-Connections werden wie TCP/SSLIOP nach Pfad
  gepoolt (kein Connect pro Call).

## Nicht-Ziele

- **Cross-Vendor-Interop über Unix-Sockets** — das omniORB-/TAO-`unix`-IOR-Format
  ist Vendor-proprietär; UIOP zielt auf ZeroDDS↔ZeroDDS-same-host. Cross-Vendor
  bleibt IIOP/SSLIOP.
- **Abstract Unix-Sockets** (Linux `@`-Namespace) — v1.0 nutzt Filesystem-Pfade.
- **Multi-Host** — UIOP ist per Definition same-host.

## §1 Transport-Binding

### §1.1 Socket

Ein UIOP-Endpoint ist ein `AF_UNIX`/`SOCK_STREAM`-Socket, gebunden an einen
Filesystem-Pfad. Der Server `bind()`+`listen()`+`accept()`; der Client
`connect()`. Eine stale Socket-Datei aus einem früheren Lauf MUSS der Server vor
`bind()` entfernen.

### §1.2 GIOP-Framing

Der GIOP-Message-Stream über den `UnixStream` ist **byte-identisch** zum
IIOP-Framing (OMG §15.4): 12-Byte-Magic+Header (`GIOP`, Version, Flags,
Message-Type, Message-Size) gefolgt vom CDR-Body. Fragmentierung (§15.4.9),
Versions-Honorierung (§15.4.1) und Codeset-Negotiation (§13.10) gelten unverändert.

### §1.3 Reader/Writer-Split & Timeouts

Wie beim TCP-Transport werden Reader/Writer-Halves via `UnixStream::try_clone`
gebildet. `SO_RCVTIMEO`/`SO_SNDTIMEO` werden gesetzt; `TCP_NODELAY` ist auf
`AF_UNIX` bedeutungslos und MUSS als no-op behandelt werden (kein
`setsockopt(IPPROTO_TCP)`).

## §2 IOR-Annoncierung

### §2.1 `TAG_ZERODDS_UDS_TRANS`

| Feld | Wert |
|---|---|
| Tag-ID | `0x5A445544` (ASCII `"ZDUD"`) |
| Lage | `TaggedComponent` im `TAG_INTERNET_IOP`-ProfileBody (`components`, ab IIOP 1.1) |
| `component_data` | CDR-Encapsulation: Byte-Order-Octet + `string` (Socket-Pfad) |

### §2.2 ProfileBody

Eine UIOP-IOR trägt einen normalen `TAG_INTERNET_IOP`-ProfileBody mit Host
`localhost` und Port `0` (kein nutzbarer TCP-Endpoint), plus optional
`TAG_CODE_SETS` und das `TAG_ZERODDS_UDS_TRANS`. Ein ZeroDDS-Client, der die
Component findet, wählt den UIOP-Transport; andernfalls fällt er auf das
TCP-Profil zurück.

## §3 Konformität

Ein **UIOP-konformer** ZeroDDS-Endpoint:

1. publiziert UIOP-IORs gemäß §2 (`stringify_object_ref_uds`),
2. akzeptiert eingehende GIOP-Messages über `AF_UNIX` (`CorbaServer::serve_uds` /
   `Acceptor::start_uds`),
3. erkennt `TAG_ZERODDS_UDS_TRANS` in einer Ziel-IOR und ruft über den
   Socket-Pfad (`IiopCorbaConnection` → `Connector::connect_uds`),
4. behandelt `TCP_NODELAY` als no-op und entfernt stale Socket-Dateien vor `bind`,
5. poolt UIOP-Connections nach Socket-Pfad.

## §4 Implementierungs-Mapping

| Spec | Code |
|---|---|
| §1.1/§1.3 Transport | `corba-iiop/src/connection.rs` — `Transport::Uds`, `Connection::from_unix_stream` |
| §1 Server-Accept | `corba-iiop/src/acceptor.rs` — `Acceptor::start_uds` |
| §3.3/§3.5 Client-Pool | `corba-iiop/src/connector.rs` — `Connector::connect_uds` (`PoolKey::Uds`) |
| §2 IOR-Component | `corba-interop/src/runtime.rs` — `TAG_ZERODDS_UDS_TRANS`, `stringify_object_ref_uds`, `serve_uds` |

## §5 Tests

- Unit: `connector::connect_uds_reuses_pooled_connection` (Pooling), iiop-Transport-Roundtrip.
- E2E: `corba-interop/tests/uiop.rs` — `serve_uds` + IOR-Component + Client-Detection
  + 5 gepoolte Echo-Calls über UDS.

## Annex A — Plattform-Hinweise

UIOP ist `#[cfg(unix)]` (Linux/macOS). macOS hat ein striktes `sun_path`-Limit
(104 Byte); ein Server SOLLTE kurze Socket-Pfade wählen. Ein vom Peer
halb-geschlossener UDS-Socket kann `setsockopt(SO_RCVTIMEO)` mit `EINVAL`
ablehnen (macOS) — ein konformer Server hält akzeptierte Connections offen.
