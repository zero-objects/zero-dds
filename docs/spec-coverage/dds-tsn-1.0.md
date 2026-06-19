# DDS Extensions for Time Sensitive Networking 1.0 — Spec-Coverage

**Quelle:** `docs/standards/cache/omg/dds-tsn-1.0-beta2.pdf` (OMG DDS-TSN 1.0,
ptc/2024-05-16; aus IP-/Copyright-Gründen nicht im Repo getrackt). Öffentliche
Spec-Seite: <https://www.omg.org/spec/DDS-TSN/>.

**Repo:** `crates/transport-tsn/` — PIM-Configuration-Modell (§7.2) + XML/JSON/YANG-PSM (§7.3) + DDSI-RTPS-Ethernet-PSM (Annex A), pure-Rust no_std+alloc. Live-AF_PACKET-Transport unter Feature `live`. 93 lib-Tests grün (Default), 105 mit `--features live` (+12 plattformneutrale `live_frame`-Tests) + veth-Roundtrip-Integrationstest (CI, root).

**Kontext:** DDS-TSN bringt DDS auf IEEE-802.1-Time-Sensitive-
Networking. ZeroDDS implementiert das **PIM-Configuration-Modell**
(§7.2 wire-relevante Tables), alle drei **Configuration-PSM** (§7.3:
XML, JSON und die normative **YANG-Group-Transformation** §7.3.3) und
das **DDSI-RTPS-Ethernet-PSM** (Annex A) als pure-Rust no_std+alloc
Library — inkl. optionalem Live-AF_PACKET-Transport (Feature `live`,
Linux). TSN-UNI-Wire-Protocol (Transport der YANG-Groups über das
konkrete UNI-Protokoll) + 802.1AS-PTP-Daemon + Hardware-Acceleration
(TX-Timestamping per `SO_TIMESTAMPING`/PHC) sind Caller-Layer.

**§3 normative Referenzen (IEEE 802.1Qbu Frame-Preemption / 802.1CB
FRER):** Die Spec listet diese in §3 als externe IEEE-Referenzen; §2
deklariert explizit *keine* eigenen Conformance-Points dafür. Sie sind
also keine eigenständigen DDS-TSN-Mappings — 802.1CB taucht als
`num_seamless_trees` (NetworkRequirements, Tab 7.18/7.25) und in den
YANG-`interface-capabilities` (`cb-*-list`) auf, 802.1Qbv als
`TimeAware` (Tab 7.17). Kein offenes Loch.

---

## §1 Scope

**Spec:** §1, S. 1 — DDS over IEEE 802.1 TSN.

**Repo:** Crate-Doc.

**Status:** done

---

## §2 Conformance

**Spec:** §2, S. 2 — "no independent conformance points".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec deklariert explizit keine eigenen Conformance-Points; Konformität wird über DDS-Konsumenten-Specs gemessen.

---

## §3 Normative References

**Spec:** §3, S. 2-3 — IEEE 802.1AS/CB/Q/Qbv/Qcc + DDS-Specs + RFCs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenzen (IEEE-802.1-Familie + RFCs); werden in den jeweiligen Konsumenten-Items §7.2.3/§8.3/Annex A operativ erfüllt.

---

## §4-§6 Terms + Symbols + Acknowledgments

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar/Symbole/Acks; ohne Code-Mapping.

---

## §7.2.3 TSN Configuration

### Tab 7.15 TsnTalker + Tab 7.24 TsnListener + Figure 7.3 TsnConfiguration

**Repo:** `crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
StreamIdentifier, TsnConfiguration}`.

`TsnTalker` deckt alle Tab-7.15-Felder ab: `name`, `stream_name`,
`traffic_specification`, `network_requirements` (0..1),
`data_frame_specification` (0..1, `Option`), `datawriter_ref`,
`time_aware` (0..1). `TsnListener` (Tab 7.24): `name`, `stream_name`,
`network_requirements` (0..1), `datareader_ref`. `TsnConfiguration`
(Figure 7.3) aggregiert `tsn_talker`/`tsn_listener` (je 0..*).

**Tests:** `stream::tests::matching_stream_ids_match`,
`different_vlan_streams_do_not_match`,
`time_aware_talker_can_be_time_critical`,
`talker_carries_optional_network_requirements_per_tab_7_15`,
`listener_carries_optional_network_requirements_per_tab_7_24`,
`tsn_configuration_aggregates_talkers_and_listeners`.

**Status:** done

### Tab 7.18/7.25 NetworkRequirements

**Spec:** §7.2.3.1.2 Tab 7.18 (Talker) + §7.2.3.2.1 Tab 7.25 (Listener)
— `num_seamless_trees` (UInt8, IEEE 802.1CB FRER-Redundanz) +
`max_latency` (Talker `UInt32`, Listener `String8`; beide Nanosekunden,
vereinheitlicht auf `u32`).

**Repo:** `crates/transport-tsn/src/network_requirements.rs::NetworkRequirements`.

**Tests:** `network_requirements::tests::*` (4).

**Status:** done

### Tab 7.16 TrafficSpecification

**Repo:** `crates/transport-tsn/src/traffic.rs::{TrafficSpecification,
TransmissionSelection}` mit allen 4 Algorithmen (StrictPriority/CBS/
ETS/ATS) + bytes_per_second-Berechnung.

**Tests:** `traffic::tests::*` (3).

**Status:** done

### Tab 7.17 TimeAware

**Repo:** `crates/transport-tsn/src/time_aware.rs::TimeAware` mit
Window-Length-Berechnung + is_valid-Prädikat.

**Tests:** `time_aware::tests::*` (4).

**Status:** done

### Tab 7.19 DataFrameSpecification

**Repo:** `crates/transport-tsn/src/data_frame.rs::DataFrameSpecification`
Variant Mac/IPv4/IPv6.

**Tests:** `data_frame::tests::*` (3).

**Status:** done

### Tab 7.20 IEEE802MacAddresses

**Repo:** `crates/transport-tsn/src/mac.rs::MacAddress` mit
Multicast/Broadcast/Locally-Administered-Detection + Display-Format.

**Tests:** `mac::tests::*` (5).

**Status:** done

### Tab 7.21 IEEE802VlanTag

**Spec:** Tab 7.21 — IEEE 802.1Q VLAN Tag (TPID + PCP + DEI + VID,
4 Bytes Wire-Form).

**Repo:** `crates/transport-tsn/src/vlan_tag.rs::Ieee802VlanTag` mit
PCP-Limit (3-bit), VID-Limit (12-bit), reserved-VID-Detection,
to_wire/from_wire Round-Trip + Bit-Layout-Validation.

**Tests:** `vlan_tag::tests::*` (8 Tests inkl.
`wire_layout_matches_spec_bit_packing` mit konkreten Hex-Werten).

**Status:** done

### Tab 7.22 IPv4Tuple

**Spec:** §7.2.3 Tab 7.22 — `source_ip`, `destination_ip`, `dscp`
(RFC 2474, 64=ignore), `protocol`, `source_port`, `destination_port`
für IPv4.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv4Tuple` (inkl.
`dscp`-Feld).

**Tests:** `data_frame::tests::ipv4_tuple_carries_5_tuple_fields`.

**Status:** done

### Tab 7.23 IPv6Tuple

**Spec:** §7.2.3 Tab 7.23 — wie Tab 7.22 für IPv6 (16-byte Adressen),
inkl. `dscp`.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv6Tuple` (inkl.
`dscp`-Feld).

**Tests:** `data_frame::tests::ipv6_tuple_uses_16_byte_addresses`.

**Status:** done

---

## §7.2.1 DDS Application Configuration

**Spec:** §7.2.1, Tab 7.1-7.7 (PDF) — Application-Library mit
QosLibrary, DomainLibrary, ApplicationLibrary, ApplicationFactory,
ApplicationFactoryRequester, ApplicationInstance,
ApplicationInstanceRequester.

**Repo:** `crates/transport-tsn/src/pim/application.rs`
(Schema-Modelle für Application-Library + Factory).

**Tests:** Inline.

**Status:** done

## §7.2.2 DDS Deployment Configuration

**Spec:** §7.2.2, Tab 7.8-7.14 (PDF) — Deployment-Library mit
DeploymentTalker, DeploymentListener, BridgeNode, etc.

**Repo:** `crates/transport-tsn/src/pim/deployment.rs`.

**Tests:** Inline.

**Status:** done

---

## §7.3 Configuration Representation (PSM)

Alle drei normativen PSM sind abgedeckt.

### §7.3.1 XML PSM

**Repo:** `crates/transport-tsn/src/pim/xml.rs::parse_dds_tsn_xml` —
Loader für `<dds_tsn>` nach Spec-XSD (Tab 7.1-7.14).

**Tests:** `pim::xml::tests::*` (11).

**Status:** done

### §7.3.2 JSON PSM

**Repo:** `crates/transport-tsn/src/pim/json.rs::render_dds_tsn_json` —
Renderer im Spec-JSON-Schema.

**Tests:** `pim::json::tests::*` (4).

**Status:** done

### §7.3.3 YANG PSM

**Spec:** §7.3.3, S. 23-27 — Transformation des Konfigurationsmodells in
die YANG-Datenmodul-Definitionen aus IEEE 802.1Q §46.3
(Talker-/Listener-Groups für den UNI-Austausch CUC↔CNC). Normativ und
NICHT durch das JSON-PSM abgedeckt — es ist ein eigenes Mapping
(stream-id-type, group-talker/group-listener mit den 802.1Q/802.1CB-
`interface-capabilities`).

**Repo:** `crates/transport-tsn/src/pim/yang.rs` — die §7.3.3-
Transformregeln 1:1:

- `StreamId` (`stream-id-type`): 6-Oktett-Node-MAC + 16-bit-DataWriter-
  ID, YANG-String `AA-BB-CC-DD-EE-FF-NN-NN` (IEEE 802.1Qcc).
- `GroupTalker::from_talker`: `stream-rank`=1, `end-station-interfaces`,
  optionale `data-frame-specification` (Ethernet-MAC+VLAN / IPv4 / IPv6
  inkl. `dscp`), `traffic-specification` (Intervall als
  numerator/denominator, `max-frames-per-interval`, `max-frame-size`,
  `transmission-selection`, `time-aware` mit Null bei unspezifiziert),
  optionale `user-to-network-requirements`, `interface-capabilities`.
- `GroupListener::from_listener`: `end-station-interfaces`,
  `user-to-network-requirements`, `interface-capabilities`.
- `talker_listener_groups`: ganze `TsnConfiguration` → Groups, mit
  sequentiellen DataWriter-IDs (§7.3.3.1).
- RFC-7951-YANG-JSON-Renderer (`to_yang_json`).

RFC 7950 YANG-Source-Parsing (textuelle `.yang`-Module einlesen) ist
Caller-Layer; die Spec verlangt die Erzeugung der YANG-Group-
Repräsentation, nicht das Parsen fremder `.yang`-Dateien.

**Tests:** `pim::yang::tests::*` (17, inkl. stream-id-Bildung,
data-frame-Branches, interval-Bruch, time-aware-Null-Regel,
UNR present/absent, YANG-JSON-Wohlgeformtheit).

**Status:** done

---

## §8 DDSI-RTPS Wire Protocol over TSN

### §8.1 DDSI-RTPS PIM unverändert

**Spec:** §8.1, S. 29 — "DDSI-RTPS PIM is unchanged from
DDSI-RTPS 2.5." TSN-spezifische Änderungen nur im PSM (§8.3+§8.4).

**Repo:** Cross-Ref `crates/rtps/` + `crates/discovery/` (siehe
`ddsi-rtps-2.5.md`).

**Tests:** Cross-Ref `ddsi-rtps-2.5.md`.

**Status:** done

### §8.2 DDSI-RTPS Conformance with TSN-Constraints

**Spec:** §8.2, S. 30-31 — Conformance-Regeln für RTPS-über-TSN
(Latency-Bounded-Pfad, Frame-Size-Limits).

**Repo:** Cross-Ref `crates/rtps/` (Frame-Size via Fragment-Layer
in `rtps/src/data_frag.rs`).

**Tests:** Cross-Ref `ddsi-rtps-2.5.md` §8 Fragmentation.

**Status:** done

### §8.3 DDSI-RTPS UDP/IP PSM over TSN — DSCP + VLAN-Tag

**Repo:** `crates/transport-tsn/src/dscp.rs::Dscp` (RFC 2474 mit
DEFAULT/EF/AF11/AF21/AF31/AF41-Konstanten + ToS-Octet-Round-Trip);
VLAN-Tag-Insertion in `vlan_tag.rs`.

**Tests:** `dscp::tests::*` (4), `vlan_tag::tests::*` (8).

**Status:** done

### §8.4 DDSI-RTPS Ethernet PSM (verweist Annex A)

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs`.

**Tests:** `ethernet_psm::tests::*` (7).

**Status:** done

---

## Annex A — DDSI-RTPS Ethernet PSM

**Spec:** Annex A, S. 33-36 — RTPS direkt im Ethernet-Frame-Payload.

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs::{
EthernetFrameHeader, ETHERTYPE_RTPS}`. Header mit/ohne VLAN-Tag
(14 vs 18 bytes), Round-Trip, Truncation-Detection,
IPv4-EtherType-Non-VLAN-Branch.

**Live-Transport (Feature `live`, Linux):**
`crates/transport-tsn/src/socket.rs::TsnTransport` sendet/empfängt RTPS
direkt im Ethernet-Frame über einen `AF_PACKET`/`SOCK_RAW`-Socket; die
plattformneutrale Frame-Logik (VLAN-Wahl, Frame-Bau + Min-Frame-Padding,
sysfs-MAC-Parsing) liegt in `live_frame.rs`.

**Tests:** `ethernet_psm::tests::*` (7); `live_frame::tests::*` (12,
plattformneutral); `tests/veth_loopback.rs` — echter RTPS-Roundtrip
über ein veth-Paar (root, CI-`tsn-live`-Job, siehe
`docs/ci/tsn-live.md`).

**Status:** done

---

## Annex B — Integration Examples

**Spec:** Annex B, S. 37-51 — informational.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annex B explizit "informational" markiert; Beispiele für Integrations-Topologien.

---

## Audit-Status

20 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-transport-tsn` — 93 lib-Tests grün,
0 failed; mit `--features live` 105 (+12 `live_frame`). Module mit
Tests: `config`, `data_frame`, `dscp`, `ethernet_psm`, `live_frame`
(live), `mac`, `network_requirements`, `pim::application`,
`pim::deployment`, `pim::json`, `pim::xml`, `pim::yang`, `stream`,
`time_aware`, `traffic`, `vlan_tag`. Plus `tests/veth_loopback.rs`
(live, root, CI).

Keine offenen Punkte: §7.2 PIM voll, §7.3 alle drei PSM (XML/JSON/YANG)
done, Annex A inkl. Live-Transport.
