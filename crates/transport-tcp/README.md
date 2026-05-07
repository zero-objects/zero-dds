# zerodds-transport-tcp

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-tcp)](https://docs.rs/zerodds-transport-tcp)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-tcp)](https://crates.io/crates/zerodds-transport-tcp)

ZeroDDS-TCP-Transport: RTPS-over-TCP-Implementation. Layer 2 (Wire-Implementation).

`std`-only, `forbid(unsafe_code)`, Safety-Klasse **STANDARD**.

## Spec-Status

Dieser Transport ist **OMG-konform auf Wire-Mapping-Ebene**:

- **DDSI-RTPS 2.5 §9.4** — Locator-Kinds `TCPv4` (4) / `TCPv6` (8)
- **DDSI-RTPS 2.5 §9.5** — Wire-Bytes-Mapping (RTPS-Header + Submessages,
  identisch zum UDP-PSM)

OMG normiert **keinen** TCP-Connection-Bring-up-Handshake. Vendoren
haben jeweils eigene Formate (Cyclone: kein Handshake; FastDDS:
0x71/0x72 Submessages; RTI: TLS-orientiert).

ZeroDDS definiert seinen eigenen Handshake explizit als eigene Spec:
**ZeroDDS-TCP-Transport 1.0**, dokumentiert in
[`docs/spec-coverage/zerodds-tcp-transport-1.0.md`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md).

## Was liefert dieses Crate

- `TcpTransport` — `Transport`-Trait-Implementation mit Connection-Pool
- `TcpTransport::without_handshake` — Cyclone-`ddsi_tcp`-Compat-Mode
- `TcpTransportError` — typisierte Fehler
- `framing` — Length-Prefix-Frame-Encoder/Decoder (§2.1)
- `handshake` — BindConnection-Request/Response (§3.1+§3.2)

## Cross-Vendor-Interop

| Peer | Status |
|---|---|
| ZeroDDS ↔ ZeroDDS | ✅ voll (Handshake + RTPS-Frames) |
| ZeroDDS ↔ Cyclone | ✅ via `without_handshake` (raw RTPS-Frames) |
| ZeroDDS ↔ FastDDS | optionaler Erweiterungspunkt (vendor-spezifischer Handshake) |
| ZeroDDS ↔ RTI | optionaler Erweiterungspunkt (TLS-Handshake) |

Cross-Vendor-Erweiterungen sind in der ZeroDDS-TCP-Transport-1.0-Spec
§6 als optional dokumentiert — kein Spec-Gap, da OMG keinen
TCP-Handshake normiert.

## Tests

```bash
cargo test -p zerodds-transport-tcp
```

55 Tests grün (50 lib + 5 integration), abgedeckte Spec-Sektionen siehe
[`zerodds-tcp-transport-1.0.md §7`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md).

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
