# RC1 Review — `zerodds-transport-tcp`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.5 (Wire — TCP)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

ZeroDDS-TCP-Transport — RTPS-over-TCP per DDSI-RTPS 2.5 §9.4+§9.5,
Length-Prefix-Framing + ZeroDDS-eigener 16-Byte-Handshake +
Cyclone-Compat-Mode + Connection-Pool.

## 2 Public-Strategy

🌐 public — TCP-Transport für DDS-Anwendungen mit Stream-Wire-Bedarf
(NAT-Traversal, Reliable-Stream-Voraussetzung).

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs            # Crate-Header + Re-Exports
src/framing.rs        # Length-Prefix-Frame Encoder/Decoder (~196 LOC)
src/handshake.rs      # ZeroDDS-Handshake (BindConnection, ~691 LOC)
src/tcp_transport.rs  # TcpTransport + Connection-Pool (~998 LOC)
```

### 3.2 Public-API-Surface

25 Items: `TcpTransport`, `TcpTransportError`, `InvalidLocator`,
`MAX_PEERS`, `MAX_INBOUND_QUEUE`, `MAX_FRAME_SIZE`, `FRAME_HEADER_LEN`,
`FramingError`, `read_frame`, `write_frame`, `BindConnectionRequest`,
`BindConnectionResponse`, `ResponseStatus`, `RejectReason`,
`HandshakeError`, `client_handshake`, `server_handshake`,
`ACCEPTED_VERSION_DIFF`, `HANDSHAKE_*` Magic-Konstanten,
`HANDSHAKE_WIRE_SIZE`, `TCP_PSM_VERSION_{MAJOR,MINOR}`, `VENDOR_ID_ZERODDS`.

### 3.3 Tests

- `cargo test -p zerodds-transport-tcp`: ✅ 55 passed (50 lib + 5 integration).

### 3.4 Coherence-Audit (§1.5b)

| Public-Item-Familie | Spec-Anker | External Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `TcpTransport` | ZeroDDS-TCP-Transport 1.0 §2-§5 | 2 (`tools/bench-suite/benches/transports_e2e.rs::bench_tcp` + `tools/isolation-smoke`) + 1 loopback-Test | CONNECTED | — |
| `TcpTransportError` | Vendor-Error-Type | 0 ext direkt; Return-Type pub-Konstruktor | VENDOR-EXTENSION (Error-Contract) | — |
| `InvalidLocator` | Vendor-Error-Sub-Type | 0 ext; pub-Pattern für Error-Match | VENDOR-EXTENSION | — |
| `MAX_PEERS`, `MAX_INBOUND_QUEUE`, `MAX_FRAME_SIZE`, `FRAME_HEADER_LEN` | Library-Defaults / DoS-Caps | `MAX_FRAME_SIZE` 2 ext, andere 0 | VENDOR-EXTENSION (Public Default-Konstanten für End-User-Konfig-Inspekt) | — |
| `FramingError`, `read_frame`, `write_frame` | DDSI-RTPS 2.5 §9.5 (Wire-Bytes-Mapping) + Vendor-Length-Prefix | `read_frame` 4, `write_frame` 6, `FramingError` 1 | CONNECTED | — |
| `BindConnectionRequest`, `BindConnectionResponse`, `ResponseStatus`, `RejectReason`, `HandshakeError`, `client_handshake`, `server_handshake` | ZeroDDS-TCP-Transport 1.0 §3 (Handshake) | `HandshakeError` 2 ext (tcp_transport.rs intern); andere 0 ext direkt | VENDOR-EXTENSION (Public Handshake-API für End-User-Embedded-TCP-Server-Builds) | — |
| `HANDSHAKE_MAGIC_REQUEST/_ACCEPT/_REJECT`, `HANDSHAKE_WIRE_SIZE`, `TCP_PSM_VERSION_*`, `VENDOR_ID_ZERODDS`, `ACCEPTED_VERSION_DIFF` | ZeroDDS-TCP-Transport 1.0 §3.1-§3.3 (Wire-Konstanten) | 0 ext | VENDOR-EXTENSION (Wire-Format-Konstanten als Public-API für Inspect/Debug) | — |

**Zusammenfassung:** 25/25 Public-Items klassifiziert. 0 ❌-Klassen.
TCP-Crate exposed primär ZeroDDS-TCP-Transport-1.0-Spec-Wire-Konstanten
+ Handshake-API als Public-API — VENDOR-EXTENSION nach §1.5b
(spec-distinkt zur OMG-DDSI-RTPS, eigene Spec-Family).

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-rtps = { path = "../rtps" }
zerodds-transport = { path = "../transport" }
```

### 4.2 Dependents

- `tools/bench-suite/benches/transports_e2e.rs::bench_tcp` (Production-Bench).
- `tools/isolation-smoke/src/main.rs` (Production-Tool).
- `crates/transport-tcp/tests/loopback.rs` (Integration-Test).

DCPS-Default-Runtime spawnt keine TcpTransport (SPDP-Multicast erfordert
UDP per Spec; user-unicast-TCP wäre architektonisch ein paralleler Pfad
mit eigener Discovery — das ist Phase-2-Erweiterung). End-User können
TcpTransport via Library-API in eigenen Custom-DCPS-Builds nutzen, wie
in `bench-suite` demonstriert.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + TcpListener/TcpStream |
| `alloc` | ✅ (via std) | — |

## 5 Spec-Relevanz

- **OMG normativ:** DDSI-RTPS 2.5 §9.4 (Locator-Kind TCPv4/TCPv6) +
  §9.5 (Wire-Bytes-Mapping = identisch zum UDP-PSM).
- **ZeroDDS-eigene Spec:** ZeroDDS-TCP-Transport 1.0
  (`docs/spec-coverage/zerodds-tcp-transport-1.0.md`).

## 6 Cleanup-Findings

Keine offenen — siehe vorheriger Review-Pass für Phase-Marker-Cleanup.

## 7 Cleanup-Actions

Bereits abgeschlossen (Layer-2 Pass 1):
1. SPDX-Header in 4 src-Files.
2. Cargo.toml RC1-Metadata.
3. 10 Phase-X-Marker rewriting (siehe F-TCP-3 Layer-2 Pass 1).
4. ZeroDDS-TCP-Transport-1.0-Spec materialisiert.

## 8 Spec-Doc-Updates

`docs/spec-coverage/zerodds-tcp-transport-1.0.md` neu (§1-§8).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header
- [x] README + CHANGELOG

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-transport-tcp                                   # ✅ 55 passed
cargo clippy -p zerodds-transport-tcp --all-targets -- -D warnings    # ✅
cargo doc -p zerodds-transport-tcp --no-deps                          # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅
- F-DCPS-tcp-default ✅ resolved (CONNECTED via tools/bench-suite)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
