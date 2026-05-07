# zerodds-transport-tsn

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-tsn)](https://docs.rs/zerodds-transport-tsn)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-tsn)](https://crates.io/crates/zerodds-transport-tsn)

OMG **DDS Extensions for Time Sensitive Networking 1.0** (formal/2024-05-16)
für ZeroDDS. Layer 2 (Wire-Implementation).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`, Safety-Klasse
**STANDARD**.

## Spec

- **OMG DDS-TSN 1.0** (formal/2024-05-16) — Time-Sensitive-Networking-
  Erweiterungen für DDS.
- Spec-Coverage:
  [`docs/spec-coverage/dds-tsn-1.0.md`](../../docs/spec-coverage/dds-tsn-1.0.md).

## Was liefert dieses Crate

**Configuration-Modell PIM** (Spec §7.2):

- `Ieee802VlanTag` (Tab 7.21) — TPID + PCP + DEI + VID per IEEE 802.1Q
- `MacAddress` (Tab 7.20) — 6-Byte MAC mit Multicast/Broadcast-Erkennung
- `TrafficSpecification` (Tab 7.16) — Interval/MaxFrameSize/MaxFramesPerInterval/TransmissionSelection per IEEE 802.1Qcc
- `TimeAware` (Tab 7.17) — earliest/latest_transmit_offset + jitter per IEEE 802.1Qbv
- `TsnTalker` + `TsnListener` (Tab 7.15+7.24) — Stream-Identifier
- `DataFrameSpecification` (Tab 7.19) — Frame-Header-Filter (MAC + VLAN + IPv4/v6-Tuple)
- `Dscp` (RFC 2474) — Differentiated Services Code Point

**DDSI-RTPS-Ethernet-PSM** (Spec Annex A):

- `EthernetFrameHeader` + `ETHERTYPE_RTPS`

**Configuration-PSM** (Spec §7.3):

- XML-Loader (`parse_xml_config`)
- JSON-Renderer (`render_json_config`)
- `TsnConfiguration` + `DeploymentLibrary` + `DomainLibrary` +
  `TsnQosLibrary`

## Bewusst nicht im Crate

| Bereich | Begründung |
|---|---|
| TSN-UNI-Wire-Protocol | proprietary, Bridge-Vendor-spezifisch (Cisco IE/Hirschmann/…) — Caller-Layer |
| YANG-PSM (§7.3) | separate `yang`-Crate, Caller-Layer |
| Hardware-TX-Timestamping | OS-spezifische API (`SO_TIMESTAMPING`/PHC) — Caller-Layer |
| gPTP / 802.1AS-Daemon | typischerweise extern (`linuxptp/ptp4l`) — Caller-Layer |

Das sind **Scope-Boundaries**, keine Deferrals — die Spec selbst sieht
diese Layer als externe Komponenten.

## Tests

```bash
cargo test -p zerodds-transport-tsn
```

69 Tests grün; clippy clean; baut auf `no_std + alloc`.

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
