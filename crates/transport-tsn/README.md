# zerodds-transport-tsn

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-tsn)](https://docs.rs/zerodds-transport-tsn)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-tsn)](https://crates.io/crates/zerodds-transport-tsn)

OMG **DDS Extensions for Time Sensitive Networking 1.0** (formal/2024-05-16)
for ZeroDDS. Layer 2 (wire implementation).

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)` (except for the optional
`live` feature, which needs libc `unsafe` for AF_PACKET), safety class
**STANDARD**.

## Spec

- **OMG DDS-TSN 1.0** (formal/2024-05-16) — Time-Sensitive-Networking
  extensions for DDS.
- Spec coverage:
  [`docs/spec-coverage/dds-tsn-1.0.md`](../../docs/spec-coverage/dds-tsn-1.0.md).

## What this crate provides

**Configuration model PIM** (Spec §7.2):

- `Ieee802VlanTag` (Tab 7.21) — TPID + PCP + DEI + VID per IEEE 802.1Q
- `MacAddress` (Tab 7.20) — 6-byte MAC with multicast/broadcast detection
- `TrafficSpecification` (Tab 7.16) — Interval/MaxFrameSize/MaxFramesPerInterval/TransmissionSelection per IEEE 802.1Qcc
- `TimeAware` (Tab 7.17) — earliest/latest_transmit_offset + jitter per IEEE 802.1Qbv
- `TsnTalker` + `TsnListener` + `TsnConfiguration` (Tab 7.15+7.24+Figure 7.3) — stream configuration model incl. `network_requirements` + `datawriter_ref`
- `NetworkRequirements` (Tab 7.18/7.25) — `num_seamless_trees` + `max_latency`
- `DataFrameSpecification` (Tab 7.19) — frame header filter (MAC + VLAN + IPv4/v6 tuple incl. `dscp` from Tab 7.22/7.23)
- `Dscp` (RFC 2474) — Differentiated Services Code Point

**DDSI-RTPS Ethernet PSM** (Spec Annex A):

- `EthernetFrameHeader` + `ETHERTYPE_RTPS`
- Live AF_PACKET transport `TsnTransport` (feature `live`, Linux) —
  RTPS directly in the Ethernet frame (EtherType 0x88B5)

**Configuration PSM** (Spec §7.3) — all three:

- §7.3.1 XML loader (`pim::xml::parse_dds_tsn_xml`)
- §7.3.2 JSON renderer (`pim::json::render_dds_tsn_json`)
- §7.3.3 YANG group transformation (`pim::yang`) — `group-talker`/
  `group-listener` for the UNI exchange CUC↔CNC, incl.
  `stream-id-type` + RFC-7951 YANG-JSON renderer

## Deliberately not in the crate

| Area | Rationale |
|---|---|
| TSN UNI wire protocol | proprietary, bridge-vendor-specific (Cisco IE/Hirschmann/…) — caller layer. We provide the YANG group representation (§7.3.3); its transport over the concrete UNI protocol is the caller layer |
| RFC-7950 `.yang` source parsing | reading foreign `.yang` modules — caller layer; the spec requires generating the YANG groups, not parsing them |
| Hardware TX timestamping | OS-specific API (`SO_TIMESTAMPING`/PHC) — caller layer |
| gPTP / 802.1AS daemon | typically external (`linuxptp/ptp4l`) — caller layer |

These are **scope boundaries**, not deferrals — the spec itself treats
these layers as external components.

## Tests

```bash
cargo test -p zerodds-transport-tsn                 # 93 tests
cargo test -p zerodds-transport-tsn --features live # 105 (+12 live_frame)
```

93 tests green (default), 105 with `--features live`; the veth RTPS
roundtrip (`tests/veth_loopback.rs`) runs as root in CI (see
`internal/ci/tsn-live.md`). clippy clean; builds on `no_std + alloc`.

## License

Apache-2.0 OR MIT — see the workspace root.
