# DDS Extensions for Time Sensitive Networking 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-tsn-1.0-beta2.pdf` (OMG DDS-TSN 1.0,
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

## §1-§6 Scope + Conformance + Referenzen

### §1 Scope

**Spec:** §1, S. 1 — die Spec bildet DDS auf IEEE-802.1-Time-Sensitive-
Networking ab: ein DDS-DataWriter/DataReader-Paar wird als TSN-Talker
bzw. -Listener konfiguriert, und RTPS läuft über das TSN-Ethernet.

**Repo:** `crates/transport-tsn/src/lib.rs` (Crate-Doc);
`crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
TsnConfiguration}` (DataWriter/DataReader → Talker/Listener);
`crates/transport-tsn/src/ethernet_psm.rs::{EthernetFrameHeader,
ETHERTYPE_RTPS}` (RTPS im Ethernet-Frame).

**Tests:** DataWriter/DataReader als Talker/Listener:
`crates/transport-tsn/src/stream.rs::tests::tsn_configuration_aggregates_talkers_and_listeners`;
RTPS über TSN-Ethernet:
`crates/transport-tsn/src/ethernet_psm.rs::tests::round_trip_with_vlan`,
`crates/transport-tsn/src/ethernet_psm.rs::tests::rtps_ethertype_constant`;
Übergabe an das TSN-Netz:
`crates/transport-tsn/src/pim/yang.rs::tests::config_transform_assigns_sequential_datawriter_ids`.

**Status:** done

### §2 Conformance

**Spec:** §2, S. 2 — "no independent conformance points".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec deklariert explizit keine eigenen Conformance-Points; Konformität wird über DDS-Konsumenten-Specs gemessen.

### §3 Normative References

**Spec:** §3, S. 2-3 — IEEE 802.1AS/CB/Q/Qbv/Qcc + DDS-Specs + RFCs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenzen (IEEE-802.1-Familie + RFCs); werden in den jeweiligen Konsumenten-Items §7.2.3/§8.3/Annex A operativ erfüllt.

### §4-§6 Terms + Symbols + Acknowledgments

**Spec:** §4-§6, S. 3-5 — Terms and Definitions, Symbols,
Acknowledgments.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar/Symbole/Acks; ohne Code-Mapping.

---

## §7.2.3 TSN Configuration

### Tab 7.15 TsnTalker + Tab 7.24 TsnListener + Figure 7.3 TsnConfiguration

**Spec:** §7.2.3.1 Tab 7.15 (TsnTalker: `name`, `stream_name`,
`traffic_specification`, `network_requirements` 0..1,
`data_frame_specification` 0..1, `datawriter_ref`, `time_aware` 0..1),
§7.2.3.2 Tab 7.24 (TsnListener: `name`, `stream_name`,
`network_requirements` 0..1, `datareader_ref`) und Figure 7.3
(TsnConfiguration aggregiert `tsn_talker`/`tsn_listener`, je 0..*).

**Repo:** `crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
StreamIdentifier, TsnConfiguration}`.

`TsnTalker` deckt alle Tab-7.15-Felder ab: `name`, `stream_name`,
`traffic_specification`, `network_requirements` (0..1),
`data_frame_specification` (0..1, `Option`), `datawriter_ref`,
`time_aware` (0..1). `TsnListener` (Tab 7.24): `name`, `stream_name`,
`network_requirements` (0..1), `datareader_ref`. `TsnConfiguration`
(Figure 7.3) aggregiert `tsn_talker`/`tsn_listener` (je 0..*).

**Tests:** `crates/transport-tsn/src/stream.rs::tests::{matching_stream_ids_match,
different_vlan_streams_do_not_match, time_aware_talker_can_be_time_critical,
talker_carries_optional_network_requirements_per_tab_7_15,
listener_carries_optional_network_requirements_per_tab_7_24,
tsn_configuration_aggregates_talkers_and_listeners}` (6).

**Status:** done

### Tab 7.18/7.25 NetworkRequirements

**Spec:** §7.2.3.1.2 Tab 7.18 (Talker) + §7.2.3.2.1 Tab 7.25 (Listener)
— `num_seamless_trees` (UInt8, IEEE 802.1CB FRER-Redundanz) +
`max_latency` (Talker `UInt32`, Listener `String8`; beide Nanosekunden,
vereinheitlicht auf `u32`).

**Repo:** `crates/transport-tsn/src/network_requirements.rs::NetworkRequirements`.

**Tests:** `crates/transport-tsn/src/network_requirements.rs::tests::zero_trees_means_no_redundancy_per_spec_46_2_3_6_1`,
`crates/transport-tsn/src/network_requirements.rs::tests::one_or_more_trees_requests_frer`.

**Status:** done

### Tab 7.16 TrafficSpecification

**Spec:** §7.2.3.1.1 Tab 7.16 — `interval` (numerator/denominator),
`max_frames_per_interval`, `max_frame_size`, `transmission_selection`
mit den vier IEEE-802.1Q-Algorithmen (Strict Priority, Credit-Based
Shaper, Enhanced Transmission Selection, Asynchronous Traffic Shaping).

**Repo:** `crates/transport-tsn/src/traffic.rs::{TrafficSpecification,
TransmissionSelection}` mit allen 4 Algorithmen (StrictPriority/CBS/
ETS/ATS) + bytes_per_second-Berechnung.

**Tests:** `crates/transport-tsn/src/traffic.rs::tests::{transmission_selection_distinct_values,
bytes_per_second_calculation_for_1ms_interval_1500_byte_frame,
bytes_per_second_for_125us_interval_8x_64_byte_frame,
bytes_per_second_zero_interval_returns_max}` (4).

**Status:** done

### Tab 7.17 TimeAware

**Spec:** §7.2.3.1.3 Tab 7.17 — `earliest_transmit_offset`,
`latest_transmit_offset`, `jitter`; das Sendefenster ist
`latest - earliest`, und der Jitter muss darin Platz haben
(IEEE 802.1Qbv Time-Aware Shaper).

**Repo:** `crates/transport-tsn/src/time_aware.rs::TimeAware` mit
Window-Length-Berechnung + is_valid-Prädikat.

**Tests:** `crates/transport-tsn/src/time_aware.rs::tests::{valid_window_is_latest_minus_earliest,
invalid_when_earliest_above_latest, invalid_when_jitter_exceeds_window,
zero_jitter_is_valid_boundary}` (4).

**Status:** done

### Tab 7.19 DataFrameSpecification

**Spec:** §7.2.3.1.4 Tab 7.19 — die Frame-Spezifikation eines Streams
ist genau eine der drei Varianten: IEEE-802-MAC-Adressen (Tab 7.20),
IPv4-Tuple (Tab 7.22) oder IPv6-Tuple (Tab 7.23).

**Repo:** `crates/transport-tsn/src/data_frame.rs::DataFrameSpecification`
Variant Mac/IPv4/IPv6.

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::{mac_specification_carries_source_and_destination,
ipv4_tuple_carries_5_tuple_fields, ipv6_tuple_uses_16_byte_addresses}` (3).

**Status:** done

### Tab 7.20 IEEE802MacAddresses

**Spec:** §7.2.3.1.4 Tab 7.20 — `destination_mac_address` +
`source_mac_address` als IEEE-802-MAC-Adressen (6 Oktett); das
niederwertigste Bit des ersten Oktetts trennt Unicast von Multicast,
das zweitniederwertigste kennzeichnet lokal administrierte Adressen.

**Repo:** `crates/transport-tsn/src/mac.rs::MacAddress` mit
Multicast/Broadcast/Locally-Administered-Detection + Display-Format.

**Tests:** `crates/transport-tsn/src/mac.rs::tests::{broadcast_constant_is_all_ff,
unicast_lsb_zero_is_not_multicast, multicast_lsb_set_detected,
locally_administered_2nd_bit_set,
display_format_matches_canonical_colon_separated_hex}` (5).

**Status:** done

### Tab 7.21 IEEE802VlanTag

**Spec:** Tab 7.21 — IEEE 802.1Q VLAN Tag (TPID + PCP + DEI + VID,
4 Bytes Wire-Form).

**Repo:** `crates/transport-tsn/src/vlan_tag.rs::Ieee802VlanTag` mit
PCP-Limit (3-bit), VID-Limit (12-bit), reserved-VID-Detection,
to_wire/from_wire Round-Trip + Bit-Layout-Validation.

**Tests:** `crates/transport-tsn/src/vlan_tag.rs::tests::{well_known_tpid_constants,
pcp_values_0_through_7_accepted, pcp_above_7_rejected, vid_above_4095_rejected,
wire_round_trip_preserves_all_fields, wire_layout_matches_spec_bit_packing,
from_wire_too_short_fails, reserved_vids_detected}` (8) —
`wire_layout_matches_spec_bit_packing` prüft das Bit-Layout mit
konkreten Hex-Werten.

**Status:** done

### Tab 7.22 IPv4Tuple

**Spec:** §7.2.3 Tab 7.22 — `source_ip`, `destination_ip`, `dscp`
(RFC 2474, 64=ignore), `protocol`, `source_port`, `destination_port`
für IPv4.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv4Tuple` (inkl.
`dscp`-Feld).

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::ipv4_tuple_carries_5_tuple_fields`.

**Status:** done

### Tab 7.23 IPv6Tuple

**Spec:** §7.2.3 Tab 7.23 — wie Tab 7.22 für IPv6 (16-byte Adressen),
inkl. `dscp`.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv6Tuple` (inkl.
`dscp`-Feld).

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::ipv6_tuple_uses_16_byte_addresses`.

**Status:** done

---

## §7.2.1-§7.2.2 Application- + Deployment-Configuration

### §7.2.1 DDS Application Configuration

**Spec:** §7.2.1, Tab 7.1-7.7 (PDF) — Application-Library mit
QosLibrary, DomainLibrary, ApplicationLibrary, ApplicationFactory,
ApplicationFactoryRequester, ApplicationInstance,
ApplicationInstanceRequester. Tab 7.2 nennt die sechs
QoS-Kategorien eines QosProfile; ein DomainParticipant erbt optional
von einem anderen.

**Repo:** `crates/transport-tsn/src/pim/application.rs`
(Schema-Modelle für Application-Library + Factory).

**Tests:** `crates/transport-tsn/src/pim/application.rs::tests::{qos_profile_holds_six_qos_categories_per_spec_tab_72,
domain_carries_registered_types_and_topics,
domain_participant_inheritance_chain_is_optional,
application_holds_multiple_dp_refs}` (4).

**Status:** done

### §7.2.2 DDS Deployment Configuration

**Spec:** §7.2.2, Tab 7.8-7.14 (PDF) — Deployment-Library mit
DeploymentTalker, DeploymentListener, BridgeNode, etc. Ein Node trägt
seine MAC-Adresse und optionale IP-Adressen; ein Deployment hat
Multiplizität 1..* an Application-Instanzen.

**Repo:** `crates/transport-tsn/src/pim/deployment.rs`.

**Tests:** `crates/transport-tsn/src/pim/deployment.rs::tests::{node_holds_mac_and_optional_ips,
deployment_with_one_app_validates,
deployment_without_apps_is_invalid_per_spec_mult_1_star,
deployment_configuration_can_reference_tsn_block}` (4).

**Status:** done

---

## §7.3 Configuration Representation (PSM)

Alle drei normativen PSM sind abgedeckt.

### §7.3.1 XML PSM

**Spec:** §7.3.1, S. 21-22 — das XML-PSM bildet das
Configuration-Modell auf ein `<dds_tsn>`-Wurzelelement mit den sechs
Bibliotheken (Tab 7.1-7.14) ab.

**Repo:** `crates/transport-tsn/src/pim/xml.rs::parse_dds_tsn_xml` —
Loader für `<dds_tsn>` nach Spec-XSD (Tab 7.1-7.14).

**Tests:** `crates/transport-tsn/src/pim/xml.rs::tests::{parses_full_sample_into_six_libraries,
qos_profile_carries_named_qos_categories, domain_carries_registered_types_and_topics,
node_with_mac_and_ipv4_parses, deployment_with_tsn_configuration_ref,
missing_root_yields_unexpected_root_error, missing_required_attribute_is_diagnostic,
invalid_mac_is_diagnostic, parse_mac_accepts_dash_separator,
parse_mac_rejects_short_input}` (10).

**Status:** done

### §7.3.2 JSON PSM

**Spec:** §7.3.2, S. 22-23 — dasselbe Configuration-Modell im
JSON-Schema der Spec: sechs Top-Level-Arrays, QoS-Profile mit
benannten Kategorien, Node-MAC in kanonischer Schreibweise.

**Repo:** `crates/transport-tsn/src/pim/json.rs::render_dds_tsn_json` —
Renderer im Spec-JSON-Schema.

**Tests:** `crates/transport-tsn/src/pim/json.rs::tests::{render_includes_six_top_level_arrays,
render_qos_profile_emits_named_categories, render_node_emits_mac_in_canonical_form,
render_escapes_special_chars}` (4).

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

**Tests:** `crates/transport-tsn/src/pim/yang.rs::tests::{stream_id_yang_string_is_mac_plus_16bit_id,
stream_id_uses_node_mac_and_datawriter_id, talker_stream_rank_is_one_per_spec,
ethernet_data_frame_maps_mac_and_vlan_from_stream,
ipv4_data_frame_maps_tuple_including_dscp, absent_data_frame_yields_none,
interval_is_period_as_reduced_fraction_of_second, traffic_spec_maps_all_fields,
time_aware_unspecified_sets_zero_offsets, time_aware_present_maps_offsets,
user_to_network_requirements_present_when_talker_has_them,
user_to_network_requirements_absent_when_talker_has_none,
end_station_interface_carries_node_mac_and_ifname,
listener_maps_node_mac_and_requirements,
config_transform_assigns_sequential_datawriter_ids,
yang_json_contains_normative_keys, yang_json_groups_has_both_arrays}` (17).

**Status:** done

---

## §8 DDSI-RTPS Wire Protocol over TSN

### §8.1 DDSI-RTPS PIM unverändert

**Spec:** §8.1, S. 29 — "DDSI-RTPS PIM is unchanged from
DDSI-RTPS 2.5." TSN-spezifische Änderungen nur im PSM (§8.3+§8.4).

**Repo:** `crates/rtps/src/header.rs` + `crates/rtps/src/submessages.rs`
— unverändert gegenüber DDSI-RTPS 2.5, keine TSN-Sonderpfade (siehe
`ddsi-rtps-2.5.md`).

**Tests:** dass das PIM unverändert bleibt, belegen dieselben
RTPS-Header-Tests, die auch ohne TSN gelten:
`crates/rtps/src/header.rs::tests::{header_layout_first_four_bytes_are_rtps_magic,
header_protocol_version_at_bytes_4_5, header_total_size_is_20_bytes,
header_roundtrip}`; dass der TSN-Pfad denselben Header transportiert:
`crates/transport-tsn/src/ethernet_psm.rs::tests::round_trip_with_vlan`.

**Status:** done

### §8.2 DDSI-RTPS Conformance with TSN-Constraints

**Spec:** §8.2, S. 30-31 — Conformance-Regeln für RTPS-über-TSN
(Latency-Bounded-Pfad, Frame-Size-Limits).

**Repo:** `crates/rtps/src/fragment_assembler.rs::FragmentAssembler`
(Frame-Size-Grenze über den Fragment-Layer);
`crates/transport-tsn/src/traffic.rs::TrafficSpecification::max_frame_size`
(TSN-seitige Obergrenze).

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::{single_fragment_sample_completes_immediately,
two_fragments_complete_in_order, fragments_complete_out_of_order,
last_fragment_shorter_than_fragment_size, sample_too_large_drops_without_alloc}`;
`crates/transport-tsn/src/traffic.rs::tests::bytes_per_second_calculation_for_1ms_interval_1500_byte_frame`;
`crates/transport-tsn/src/ethernet_psm.rs::tests::truncated_input_is_error`.

**Status:** done

### §8.3 DDSI-RTPS UDP/IP PSM over TSN — DSCP + VLAN-Tag

**Spec:** §8.3, S. 31-32 — RTPS über UDP/IP im TSN-Netz markiert die
Pakete mit einem DSCP-Codepoint (RFC 2474, oberste 6 Bit des
ToS-Oktetts) und einem IEEE-802.1Q-VLAN-Tag mit PCP-Priorität.

**Repo:** `crates/transport-tsn/src/dscp.rs::Dscp` (RFC 2474 mit
DEFAULT/EF/AF11/AF21/AF31/AF41-Konstanten + ToS-Octet-Round-Trip);
VLAN-Tag-Insertion in `crates/transport-tsn/src/vlan_tag.rs`.

**Tests:** `crates/transport-tsn/src/dscp.rs::tests::{dscp_well_known_constants_match_rfc,
dscp_above_63_rejected, tos_octet_round_trip_preserves_dscp,
from_tos_ignores_ecn_bits}` (4);
`crates/transport-tsn/src/vlan_tag.rs::tests::{wire_layout_matches_spec_bit_packing,
wire_round_trip_preserves_all_fields, pcp_values_0_through_7_accepted,
pcp_above_7_rejected}`.

**Status:** done

### §8.4 DDSI-RTPS Ethernet PSM (verweist Annex A)

**Spec:** §8.4, S. 32 — für das Ethernet-PSM verweist §8.4 auf Annex A:
RTPS-Nachrichten liegen unmittelbar im Ethernet-Frame-Payload unter
einem eigenen EtherType, ohne IP- und UDP-Schicht.

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs`.

**Tests:** `crates/transport-tsn/src/ethernet_psm.rs::tests::{rtps_ethertype_constant,
header_without_vlan_is_14_bytes, header_with_vlan_is_18_bytes,
round_trip_without_vlan, round_trip_with_vlan, truncated_input_is_error,
ipv4_ethertype_parsed_without_vlan_branch}` (7).

**Status:** done

---

## Annexe

### Annex A — DDSI-RTPS Ethernet PSM

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

**Tests:** `crates/transport-tsn/src/ethernet_psm.rs::tests::{rtps_ethertype_constant,
header_without_vlan_is_14_bytes, header_with_vlan_is_18_bytes,
round_trip_without_vlan, round_trip_with_vlan, truncated_input_is_error,
ipv4_ethertype_parsed_without_vlan_branch}` (7);
`crates/transport-tsn/src/live_frame.rs::tests::{target_vid_overrides_default,
zero_target_falls_back_to_default_vid, zero_default_and_target_is_untagged,
vid_out_of_range_is_error, pcp_out_of_range_is_error,
build_frame_untagged_has_14_byte_header, build_frame_tagged_has_18_byte_header,
build_frame_large_payload_not_padded, parse_mac_sysfs_standard_format,
parse_mac_sysfs_tolerates_trailing_newline, parse_mac_sysfs_too_few_octets,
parse_mac_sysfs_non_hex}` (12, plattformneutral, Feature `live`);
`crates/transport-tsn/tests/veth_loopback.rs` — echter RTPS-Roundtrip
über ein veth-Paar (root, CI-`tsn-live`-Job, siehe
`internal/ci/tsn-live.md`).

**Status:** done

---

### Annex B — Integration Examples

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
