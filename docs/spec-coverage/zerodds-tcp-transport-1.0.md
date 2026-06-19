# ZeroDDS-TCP-Transport 1.0 — Spec-Coverage

ZeroDDS-vendor-spezifische TCP-Transport-Spec. Analog zu Cyclone's
`ddsi_tcp`, FastDDS-TCPv4 und RTI-DDS-TCP. **Nicht OMG-normativ.**

Implementation:

- `crates/transport-tcp/` — TCP-Transport (Length-Prefix-Framing + 16-Byte-Handshake).

| Spec-Family | Status |
|---|---|
| **OMG-normativ** | DDSI-RTPS 2.5 §9.4 (Locator-Kind) + §9.5 (Wire-Mapping) — voll abgedeckt; siehe `ddsi-rtps-2.5.md` Sektion 9.4/9.5 |
| **ZeroDDS-eigene Spec** | Length-Prefix-Framing + 16-Byte-Handshake — [`zerodds-tcp-transport-1.0.md`](https://github.com/zero-objects/zero-dds/blob/main/docs/spec-coverage/zerodds-tcp-transport-1.0.md) |

## §1 Scope und Spec-Status

### §1.1 Was OMG normiert

DDSI-RTPS 2.5 normiert für TCP nur:

- **§9.4.5 LocatorKind**: `LOCATOR_KIND_TCPv4 = 4`, `LOCATOR_KIND_TCPv6 = 8`.
- **§9.5 Wire-Mapping**: RTPS-Message-Format (RTPS-Header + Submessages)
  ist identisch zum UDP-PSM. TCP fügt Stream-Framing hinzu, das die Spec
  nicht weiter normiert.

ZeroDDS-Implementation:

- `crates/rtps/src/wire_types.rs` — LocatorKind-Konstanten.
- `crates/transport-tcp/src/framing.rs` — RTPS-Frame-Mapping auf Stream.

### §1.2 Was OMG nicht normiert

- **Handshake / BindConnection**: Es existiert kein OMG-Standard für
  TCP-Connection-Bring-up. Vendoren haben eigene Formate:
  - **Cyclone DDS**: kein Handshake; raw RTPS-Frames mit
    Length-Prefix.
  - **FastDDS**: BindConnectionRequest/Response als RTPS-Submessages
    `0x71`/`0x72` (vendor-spezifisch).
  - **RTI DDS**: TLS-orientierter Handshake (vendor-spezifisch).
- **Logical-Port-Multiplexing**: vendor-spezifisch.
- **Connection-Pool-Reuse-Policies**: nicht spec-relevant.

### §1.3 ZeroDDS-Wahl

ZeroDDS-TCP-Transport definiert eine **eigene Spec** (diese Datei) für
den Bring-up-Handshake. Die Spec ist:

- **OMG-konform** auf Locator+Wire-Frame-Ebene (§9.4+§9.5).
- **Vendor-spezifisch** auf Handshake-Ebene (analog Cyclone/FastDDS/RTI).
- **Cross-Vendor-fähig** zu Cyclone via Handshake-Skip-Mode.

## §2 Wire-Format

### §2.1 Length-Prefix-Frame (DDSI-RTPS §9.5-konform)

Jedes RTPS-Datagramm wird über TCP wie folgt gerahmt:

```text
+---------+---------+---------+---------+
| length (u32, big-endian)              |   Frame-Length in Bytes (excl. Header selbst)
+---------+---------+---------+---------+
| RTPS-Frame-Body (length Bytes)        |   wie bei UDP-PSM
+---------+---------+---------+---------+
```

- **length-Field**: 4 Byte big-endian.
- **RTPS-Frame-Body**: vollständiges RTPS-Message (RTPS-Header +
  Submessages), identisch zum UDP-PSM (DDSI-RTPS §9.5).

Repo-Anker: `crates/transport-tcp/src/framing.rs::encode_frame`,
`decode_frame`. Tests: `crates/transport-tcp/src/framing.rs::tests`.

### §2.2 Maximum-Frame-Size

Default: 64 MiB. Empfänger MUSS Frames > Limit ablehnen (DoS-Schutz).
Repo-Anker: `MAX_FRAME_SIZE` in `framing.rs`.

## §3 Handshake (ZeroDDS-spezifisch)

### §3.1 BindConnectionRequest (16 Byte, big-endian)

```text
+---------+---------+---------+---------+
|  'Z'    |  'D'    |  'D'    |  'S'    |   magic "ZDDS"
+---------+---------+---------+---------+
| v_major | v_minor |  vendor_id (2 B)  |   protocol-version + vendor-id
+---------+---------+---------+---------+
|               flags (u32)             |   reserved, = 0
+---------+---------+---------+---------+
|            logical_port (u32)         |   DDS-Endpoint-Logical-Port
+---------+---------+---------+---------+
```

- **Magic** `"ZDDS"` — kollidiert nicht mit RTPS-Magic `"RTPS"`,
  HTTP-Magic `"HTTP"`, TLS-ClientHello — Sofort-Discriminator.
- **Version**: aktuell `(1,0)`.
- **VendorId**: ZeroDDS = `[0x01, 0x0F]`.
- **Flags**: reserviert, MUSS `0` sein.
- **LogicalPort**: u32; `0` = "kein Claim".

Repo-Anker: `handshake.rs::BindConnectionRequest`,
`encode_request`, `decode_request`.

### §3.2 BindConnectionResponse (16 Byte, big-endian)

```text
+---------+---------+---------+---------+
|  'Z'    |  'D'    |  'A'    | status  |   magic "ZDA" + accept/reject
+---------+---------+---------+---------+
| v_major | v_minor |  vendor_id (2 B)  |   server-version + vendor-id
+---------+---------+---------+---------+
|               flags (u32)             |   reserved, = 0
+---------+---------+---------+---------+
|            reason_code (u32)          |   0 bei Accept, sonst per §3.3
+---------+---------+---------+---------+
```

- **status-Byte**:
  - `0x2B` (`'+'`) — Accept
  - `0x2D` (`'-'`) — Reject

Repo-Anker: `handshake.rs::BindConnectionResponse`,
`encode_response`, `decode_response`.

### §3.3 Reject-Reason-Codes

| Code | Konstante | Bedeutung |
|---:|---|---|
| 0 | `Unknown` | nicht klassifizierter Reject |
| 1 | `VersionMismatch` | Peer-Version außerhalb des akzeptierten Fensters |
| 2 | `ResourceLimit` | Server hat MAX_PEERS erreicht |
| 3 | `LogicalPortConflict` | Port bereits gebunden |
| 4 | `VendorNotAccepted` | Vendor-Policy lehnt Vendor-Id ab |

Repo-Anker: `handshake.rs::RejectReason`.

### §3.4 Sequenz

```text
Client                                    Server
  |                                          |
  |-- BindConnectionRequest (16 B) --------->|   §3.1
  |                                          |   verifies version + magic
  |<--- BindConnectionResponse (16 B) -------|   §3.2
  |                                          |
  |-- Length-Prefix-Frame [RTPS] ----------->|   §2.1
  |<-- Length-Prefix-Frame [RTPS] -----------|
  |                            ...           |
```

Bei Reject droppen beide Seiten die Connection. Client-Side soll
Backoff anwenden (siehe `tcp_transport.rs` Connection-Pool).

## §4 Cyclone-`ddsi_tcp`-Compatibility

Cyclone DDS spricht raw RTPS-Frames über TCP **ohne Handshake**.
ZeroDDS unterstützt diesen Mode via Handshake-Skip:

- Client-Side: `TcpTransport::without_handshake()` skippt §3 und
  sendet direkt §2.1 Length-Prefix-Frames.
- Server-Side: detektiert Cyclone-Mode, wenn das erste Byte des
  Frame-Bodys nicht `'Z'` ist (kein ZeroDDS-Handshake-Magic), sondern
  `'R'` (RTPS-Magic-erstes-Byte). Falls Auto-Detect-Feature aktiviert.

Repo-Anker: `tcp_transport.rs::TcpTransport::without_handshake`.

## §5 Connection-Pool

Nicht spec-relevant; Implementation-Detail. Der Pool bietet
Connection-Reuse, Backoff und MAX_PEERS-Cap.

Repo-Anker: `tcp_transport.rs::TcpTransport`,
`tcp_transport.rs::ConnectionPool`.

## §6 Cross-Vendor-Erweiterungspunkte (optional)

Für künftige Releases als optionale Feature-Flags vorgesehen,
**nicht RC1-Anforderung** (kein normativer Spec):

- `feature = "fastdds-compat"`: BindConnectionRequest/Response als
  RTPS-Submessages `0x71`/`0x72` (FastDDS-Format).
- `feature = "rti-compat"`: TLS-orientierter Handshake (RTI-Format).

Erweiterung würde §3 erweitern, nicht ersetzen — der ZeroDDS-eigene
Handshake bleibt Default.

## §7 Test-Coverage

| Spec-Sektion | Tests |
|---|---|
| §2.1 Length-Prefix-Frame | `framing.rs::tests` (8 Tests) |
| §2.2 Max-Frame-Size | `framing.rs::tests::frame_size_limit` |
| §3.1 BindConnectionRequest | `handshake.rs::tests::request_*` |
| §3.2 BindConnectionResponse | `handshake.rs::tests::response_*` |
| §3.3 Reject-Reasons | `handshake.rs::tests::reject_reason_*` |
| §3.4 Handshake-Sequenz | `handshake.rs::tests::client_server_*` |
| §4 Cyclone-Compat | `tests/loopback.rs::cyclone_compat_*` |
| §5 Connection-Pool | `tcp_transport.rs::tests::pool_*` |

Total: 50 Tests (lib) + 5 Tests (integration). Alle grün, Stand
Audit-Datum.

## §8 Status

**Voll abgedeckt**. ZeroDDS-TCP-Transport ist eine vollständige,
in-sich-kohärente Spec; alle §-Sektionen sind implementiert und
getestet. Cross-Vendor-Erweiterungspunkte (§6) sind als optional
markiert.
