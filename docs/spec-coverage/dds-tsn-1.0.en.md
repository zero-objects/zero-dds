# DDS Extensions for Time Sensitive Networking 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-tsn-1.0-beta2.pdf` (OMG DDS-TSN 1.0,
ptc/2024-05-16; not tracked in the repo for IP/copyright reasons). Public spec
page: <https://www.omg.org/spec/DDS-TSN/>.

**Repo:** `crates/transport-tsn/` — PIM configuration model (§7.2) + XML/JSON/YANG PSM (§7.3) + DDSI-RTPS Ethernet PSM (Annex A), pure-Rust no_std+alloc. Live AF_PACKET transport under feature `live`. 93 lib tests green (default), 105 with `--features live` (+12 platform-neutral `live_frame` tests) + veth roundtrip integration test (CI, root).

**Context:** DDS-TSN brings DDS onto IEEE 802.1 Time-Sensitive Networking.
ZeroDDS implements the **PIM configuration model** (§7.2 wire-relevant
tables), all three **configuration PSMs** (§7.3: XML, JSON and the
normative **YANG group transformation** §7.3.3) and the **DDSI-RTPS
Ethernet PSM** (Annex A) as a pure-Rust no_std+alloc library — including
an optional live AF_PACKET transport (feature `live`, Linux). The TSN-UNI
wire protocol (transport of the YANG groups over the concrete UNI
protocol) + the 802.1AS PTP daemon + hardware acceleration (TX
timestamping via `SO_TIMESTAMPING`/PHC) are caller-layer.

**§3 normative references (IEEE 802.1Qbu frame preemption / 802.1CB
FRER):** the spec lists these in §3 as external IEEE references; §2
explicitly declares *no* independent conformance points for them. They
are therefore not standalone DDS-TSN mappings — 802.1CB surfaces as
`num_seamless_trees` (NetworkRequirements, Tab 7.18/7.25) and in the YANG
`interface-capabilities` (`cb-*-list`), 802.1Qbv as `TimeAware`
(Tab 7.17). No open gap.

---

## §1 Scope

**Spec:** §1, p. 1 — DDS over IEEE 802.1 TSN.

**Repo:** crate doc.

**Status:** done

---

## §2 Conformance

**Spec:** §2, p. 2 — "no independent conformance points".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec explicitly declares no own conformance points; conformance is measured via the DDS consumer specs.

---

## §3 Normative references

**Spec:** §3, p. 2-3 — IEEE 802.1AS/CB/Q/Qbv/Qcc + DDS specs + RFCs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external normative references (the IEEE 802.1 family + RFCs); fulfilled operationally in the respective consumer items §7.2.3/§8.3/Annex A.

---

## §4-§6 Terms + symbols + acknowledgments

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary/symbols/acks; without a code mapping.

---

## §7.2.3 TSN configuration

### Tab 7.15 TsnTalker + Tab 7.24 TsnListener + Figure 7.3 TsnConfiguration

**Repo:** `crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
StreamIdentifier, TsnConfiguration}`.

`TsnTalker` covers all Tab 7.15 fields: `name`, `stream_name`,
`traffic_specification`, `network_requirements` (0..1),
`data_frame_specification` (0..1, `Option`), `datawriter_ref`,
`time_aware` (0..1). `TsnListener` (Tab 7.24): `name`, `stream_name`,
`network_requirements` (0..1), `datareader_ref`. `TsnConfiguration`
(Figure 7.3) aggregates `tsn_talker`/`tsn_listener` (0..* each).

**Tests:** `stream::tests::matching_stream_ids_match`,
`different_vlan_streams_do_not_match`,
`time_aware_talker_can_be_time_critical`,
`talker_carries_optional_network_requirements_per_tab_7_15`,
`listener_carries_optional_network_requirements_per_tab_7_24`,
`tsn_configuration_aggregates_talkers_and_listeners`.

**Status:** done

### Tab 7.18/7.25 NetworkRequirements

**Spec:** §7.2.3.1.2 Tab 7.18 (Talker) + §7.2.3.2.1 Tab 7.25 (Listener)
— `num_seamless_trees` (UInt8, IEEE 802.1CB FRER redundancy) +
`max_latency` (Talker `UInt32`, Listener `String8`; both nanoseconds,
unified to `u32`).

**Repo:** `crates/transport-tsn/src/network_requirements.rs::NetworkRequirements`.

**Tests:** `network_requirements::tests::*` (4).

**Status:** done

### Tab 7.16 TrafficSpecification

**Repo:** `crates/transport-tsn/src/traffic.rs::{TrafficSpecification,
TransmissionSelection}` with all 4 algorithms (StrictPriority/CBS/ETS/ATS)
+ a bytes_per_second computation.

**Tests:** `traffic::tests::*` (3).

**Status:** done

### Tab 7.17 TimeAware

**Repo:** `crates/transport-tsn/src/time_aware.rs::TimeAware` with a
window-length computation + an is_valid predicate.

**Tests:** `time_aware::tests::*` (4).

**Status:** done

### Tab 7.19 DataFrameSpecification

**Repo:** `crates/transport-tsn/src/data_frame.rs::DataFrameSpecification`
variant Mac/IPv4/IPv6.

**Tests:** `data_frame::tests::*` (3).

**Status:** done

### Tab 7.20 IEEE802MacAddresses

**Repo:** `crates/transport-tsn/src/mac.rs::MacAddress` with
multicast/broadcast/locally-administered detection + a display format.

**Tests:** `mac::tests::*` (5).

**Status:** done

### Tab 7.21 IEEE802VlanTag

**Spec:** Tab 7.21 — the IEEE 802.1Q VLAN tag (TPID + PCP + DEI + VID,
4 bytes wire form).

**Repo:** `crates/transport-tsn/src/vlan_tag.rs::Ieee802VlanTag` with a
PCP limit (3-bit), VID limit (12-bit), reserved-VID detection,
to_wire/from_wire round-trip + bit-layout validation.

**Tests:** `vlan_tag::tests::*` (8 tests incl.
`wire_layout_matches_spec_bit_packing` with concrete hex values).

**Status:** done

### Tab 7.22 IPv4Tuple

**Spec:** §7.2.3 Tab 7.22 — `source_ip`, `destination_ip`, `dscp`
(RFC 2474, 64=ignore), `protocol`, `source_port`, `destination_port`
for IPv4.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv4Tuple` (incl.
the `dscp` field).

**Tests:** `data_frame::tests::ipv4_tuple_carries_5_tuple_fields`.

**Status:** done

### Tab 7.23 IPv6Tuple

**Spec:** §7.2.3 Tab 7.23 — as Tab 7.22 for IPv6 (16-byte addresses),
incl. `dscp`.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv6Tuple` (incl.
the `dscp` field).

**Tests:** `data_frame::tests::ipv6_tuple_uses_16_byte_addresses`.

**Status:** done

---

## §7.2.1 DDS Application Configuration

**Spec:** §7.2.1, Tab 7.1-7.7 (PDF) — the application library with
QosLibrary, DomainLibrary, ApplicationLibrary, ApplicationFactory,
ApplicationFactoryRequester, ApplicationInstance,
ApplicationInstanceRequester.

**Repo:** `crates/transport-tsn/src/pim/application.rs` (schema models for
the application library + factory).

**Tests:** inline.

**Status:** done

## §7.2.2 DDS Deployment Configuration

**Spec:** §7.2.2, Tab 7.8-7.14 (PDF) — the deployment library with
DeploymentTalker, DeploymentListener, BridgeNode, etc.

**Repo:** `crates/transport-tsn/src/pim/deployment.rs`.

**Tests:** inline.

**Status:** done

---

## §7.3 Configuration Representation (PSM)

All three normative PSMs are covered.

### §7.3.1 XML PSM

**Repo:** `crates/transport-tsn/src/pim/xml.rs::parse_dds_tsn_xml` — loader
for `<dds_tsn>` per the spec XSD (Tab 7.1-7.14).

**Tests:** `pim::xml::tests::*` (11).

**Status:** done

### §7.3.2 JSON PSM

**Repo:** `crates/transport-tsn/src/pim/json.rs::render_dds_tsn_json` —
renderer in the spec JSON schema.

**Tests:** `pim::json::tests::*` (4).

**Status:** done

### §7.3.3 YANG PSM

**Spec:** §7.3.3, p. 23-27 — transformation of the configuration model into
the YANG data-module definitions from IEEE 802.1Q §46.3 (Talker/Listener
groups for the UNI exchange CUC↔CNC). Normative and NOT covered by the JSON
PSM — it is a distinct mapping (stream-id-type, group-talker/group-listener
with the 802.1Q/802.1CB `interface-capabilities`).

**Repo:** `crates/transport-tsn/src/pim/yang.rs` — the §7.3.3 transformation
rules 1:1:

- `StreamId` (`stream-id-type`): 6-octet node MAC + 16-bit DataWriter ID,
  YANG string `AA-BB-CC-DD-EE-FF-NN-NN` (IEEE 802.1Qcc).
- `GroupTalker::from_talker`: `stream-rank`=1, `end-station-interfaces`,
  optional `data-frame-specification` (Ethernet MAC+VLAN / IPv4 / IPv6 incl.
  `dscp`), `traffic-specification` (interval as numerator/denominator,
  `max-frames-per-interval`, `max-frame-size`, `transmission-selection`,
  `time-aware` with zero when unspecified), optional
  `user-to-network-requirements`, `interface-capabilities`.
- `GroupListener::from_listener`: `end-station-interfaces`,
  `user-to-network-requirements`, `interface-capabilities`.
- `talker_listener_groups`: a whole `TsnConfiguration` → groups, with
  sequential DataWriter IDs (§7.3.3.1).
- RFC 7951 YANG-JSON renderer (`to_yang_json`).

RFC 7950 YANG source parsing (reading textual `.yang` modules) is
caller-layer; the spec requires producing the YANG group representation,
not parsing foreign `.yang` files.

**Tests:** `pim::yang::tests::*` (17, incl. stream-id construction,
data-frame branches, interval fraction, time-aware zero rule, UNR
present/absent, YANG-JSON well-formedness).

**Status:** done

---

## §8 DDSI-RTPS wire protocol over TSN

### §8.1 DDSI-RTPS PIM unchanged

**Spec:** §8.1, p. 29 — "DDSI-RTPS PIM is unchanged from DDSI-RTPS 2.5."
TSN-specific changes only in the PSM (§8.3+§8.4).

**Repo:** cross-ref `crates/rtps/` + `crates/discovery/` (see
`ddsi-rtps-2.5.md`).

**Tests:** cross-ref `ddsi-rtps-2.5.md`.

**Status:** done

### §8.2 DDSI-RTPS conformance with TSN constraints

**Spec:** §8.2, p. 30-31 — conformance rules for RTPS-over-TSN
(latency-bounded path, frame-size limits).

**Repo:** cross-ref `crates/rtps/` (frame size via the fragment layer in
`rtps/src/data_frag.rs`).

**Tests:** cross-ref `ddsi-rtps-2.5.md` §8 fragmentation.

**Status:** done

### §8.3 DDSI-RTPS UDP/IP PSM over TSN — DSCP + VLAN tag

**Repo:** `crates/transport-tsn/src/dscp.rs::Dscp` (RFC 2474 with
DEFAULT/EF/AF11/AF21/AF31/AF41 constants + ToS-octet round-trip); VLAN-tag
insertion in `vlan_tag.rs`.

**Tests:** `dscp::tests::*` (4), `vlan_tag::tests::*` (8).

**Status:** done

### §8.4 DDSI-RTPS Ethernet PSM (refers to Annex A)

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs`.

**Tests:** `ethernet_psm::tests::*` (7).

**Status:** done

---

## Annex A — DDSI-RTPS Ethernet PSM

**Spec:** Annex A, p. 33-36 — RTPS directly in the Ethernet frame payload.

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs::{EthernetFrameHeader,
ETHERTYPE_RTPS}`. Header with/without VLAN tag (14 vs 18 bytes),
round-trip, truncation detection, IPv4-EtherType non-VLAN branch.

**Live transport (feature `live`, Linux):**
`crates/transport-tsn/src/socket.rs::TsnTransport` sends/receives RTPS
directly in the Ethernet frame over an `AF_PACKET`/`SOCK_RAW` socket; the
platform-neutral frame logic (VLAN selection, frame build + min-frame
padding, sysfs MAC parsing) lives in `live_frame.rs`.

**Tests:** `ethernet_psm::tests::*` (7); `live_frame::tests::*` (12,
platform-neutral); `tests/veth_loopback.rs` — real RTPS roundtrip over a
veth pair (root, CI `tsn-live` job, see `internal/ci/tsn-live.md`).

**Status:** done

---

## Annex B — Integration Examples

**Spec:** Annex B, p. 37-51 — informational.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annex B explicitly marked "informational"; examples for integration topologies.

---

## Audit status

20 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-transport-tsn` — 93 lib tests green,
0 failed; with `--features live` 105 (+12 `live_frame`). Modules with
tests: `config`, `data_frame`, `dscp`, `ethernet_psm`, `live_frame`
(live), `mac`, `network_requirements`, `pim::application`,
`pim::deployment`, `pim::json`, `pim::xml`, `pim::yang`, `stream`,
`time_aware`, `traffic`, `vlan_tag`. Plus `tests/veth_loopback.rs`
(live, root, CI).

No open items: §7.2 PIM full, §7.3 all three PSMs (XML/JSON/YANG) done,
Annex A incl. live transport.
