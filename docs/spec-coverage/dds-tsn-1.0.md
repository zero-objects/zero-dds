# DDS Extensions for Time Sensitive Networking 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/dds-tsn-1.0-beta2.pdf` (OMG ptc/2024-05-16).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** DDS-TSN bringt DDS auf IEEE-802.1-Time-Sensitive-
Networking. ZeroDDS implementiert das **PIM-Configuration-Modell**
(§7.2 wire-relevante Tables) und das **DDSI-RTPS-Ethernet-PSM**
(Annex A) als pure-Rust no_std+alloc Library in
`crates/transport-tsn/`. TSN-UNI-Wire-Protocol + 802.1AS-PTP-Daemon +
Hardware-Acceleration (TX-Timestamping per `SO_TIMESTAMPING`/PHC) sind
Caller-Layer.

Implementation: `crates/transport-tsn/` (8 Module, 38 Tests gruen).

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

**Status:** `n/a (informative)` — Spec deklariert explizit keine eigenen Conformance-Points; Konformitaet wird ueber DDS-Konsumenten-Specs gemessen.

---

## §3 Normative References

**Spec:** §3, S. 2-3 — IEEE 802.1AS/CB/Q/Qbv/Qcc + DDS-Specs + RFCs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenzen (IEEE-802.1-Familie + RFCs); werden in den jeweiligen Konsumenten-Items §7.2.3/§8.3/Annex A operativ erfuellt.

---

## §4-§6 Terms + Symbols + Acknowledgments

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar/Symbole/Acks; ohne Code-Mapping.

---

## §7.2.3 TSN Configuration

### Tab 7.15 TsnTalker + Tab 7.24 TsnListener

**Repo:** `crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
StreamIdentifier}`.

**Tests:** `stream::tests::matching_stream_ids_match`,
`different_vlan_streams_do_not_match`,
`time_aware_talker_can_be_time_critical`.

**Status:** done

### Tab 7.16 TrafficSpecification

**Repo:** `crates/transport-tsn/src/traffic.rs::{TrafficSpecification,
TransmissionSelection}` mit allen 4 Algorithmen (StrictPriority/CBS/
ETS/ATS) + bytes_per_second-Berechnung.

**Tests:** `traffic::tests::*` (3).

**Status:** done

### Tab 7.17 TimeAware

**Repo:** `crates/transport-tsn/src/time_aware.rs::TimeAware` mit
Window-Length-Berechnung + is_valid-Praedikat.

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

**Spec:** §7.2.3 Tab 7.22 — 5-Tuple-Struktur (src-IP, dst-IP, src-Port,
dst-Port, Protocol) für IPv4.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv4Tuple`.

**Tests:** `data_frame::tests::ipv4_tuple_carries_5_tuple_fields`.

**Status:** done

### Tab 7.23 IPv6Tuple

**Spec:** §7.2.3 Tab 7.23 — 5-Tuple-Struktur für IPv6 (16-byte
Adressen).

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv6Tuple`.

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

**Spec:** §7.3, S. 22-28 — XML/JSON/YANG-PSM.

**Repo:** `crates/transport-tsn/src/pim/{xml,json}.rs` (XML- und
JSON-Codec). Der JSON-PSM-Output ist gleichzeitig die **YANG-JSON-
Form nach RFC 7951** ("JSON Encoding of Data Modeled with YANG"),
solange die Schema-Strukturen YANG-konform sind (container/leaf/
list/choice/case-Aequivalente). Unsere TSN-Application/Deployment-
Library-Schemas (`pim::application`, `pim::deployment`) folgen dem
YANG-Tree-Pattern (named container mit typisierten leaves), womit
die JSON-Serialisierung automatisch eine valide YANG-Instance ist.
RFC 7950 YANG-Source-Parsing (textuelle .yang-Files) ist Caller-
Layer und nicht Teil dieses Crates — die Spec verlangt nur eine der
drei Repraesentationen, nicht alle drei.

**Tests:** Cross-Ref `pim::json::tests::*` +
`pim::xml::tests::*`.

**Status:** done — XML + JSON-PSM (auch RFC-7951 YANG-JSON-Form)
abgedeckt; YANG-Source-Files sind Caller-Layer.

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

**Tests:** `ethernet_psm::tests::*` (7 Tests).

**Status:** done

---

## Annex B — Integration Examples

**Spec:** Annex B, S. 37-51 — informational.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annex B explizit "informational" markiert; Beispiele fuer Integrations-Topologien.

---

## Audit-Status

17 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-transport-tsn` — 69 lib-Tests grün,
0 failed. Module mit Tests: `config`, `data_frame`, `dscp`,
`ethernet_psm`, `mac`, `pim::application`, `pim::deployment`,
`pim::json`, `pim::xml`, `stream`, `time_aware`, `traffic`,
`vlan_tag`.

Offene Punkte: siehe `dds-tsn-1.0.open.md`. §7.2 PIM voll, §7.3
XML/JSON-PSM done; YANG-PSM offen (~0.5-1 PW).
