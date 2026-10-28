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

## §1-§6 Scope + conformance + references

### §1 Scope

**Spec:** §1, p. 1 — the spec maps DDS onto IEEE 802.1 Time-Sensitive
Networking: a DDS DataWriter/DataReader pair is configured as a TSN talker
resp. listener, and RTPS runs over the TSN Ethernet.

**Repo:** `crates/transport-tsn/src/lib.rs` (crate doc);
`crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
TsnConfiguration}` (DataWriter/DataReader -> talker/listener);
`crates/transport-tsn/src/ethernet_psm.rs::{EthernetFrameHeader,
ETHERTYPE_RTPS}` (RTPS in the Ethernet frame).

**Tests:** DataWriter/DataReader as talker/listener:
`crates/transport-tsn/src/stream.rs::tests::tsn_configuration_aggregates_talkers_and_listeners`;
RTPS over TSN Ethernet:
`crates/transport-tsn/src/ethernet_psm.rs::tests::round_trip_with_vlan`,
`crates/transport-tsn/src/ethernet_psm.rs::tests::rtps_ethertype_constant`;
handover to the TSN network:
`crates/transport-tsn/src/pim/yang.rs::tests::config_transform_assigns_sequential_datawriter_ids`.

**Status:** done

### §2 Conformance

**Spec:** §2, p. 2 — "no independent conformance points".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec explicitly declares no own conformance points; conformance is measured via the DDS consumer specs.

### §3 Normative references

**Spec:** §3, p. 2-3 — IEEE 802.1AS/CB/Q/Qbv/Qcc + DDS specs + RFCs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external normative references (the IEEE 802.1 family + RFCs); fulfilled operationally in the respective consumer items §7.2.3/§8.3/Annex A.

### §4-§6 Terms + symbols + acknowledgments

**Spec:** §4-§6, p. 3-5 — terms and definitions, symbols,
acknowledgments.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary/symbols/acks; without a code mapping.

---

## §7.2.3 TSN configuration

### Tab 7.15 TsnTalker + Tab 7.24 TsnListener + Figure 7.3 TsnConfiguration

**Spec:** §7.2.3.1 Tab 7.15 (TsnTalker: `name`, `stream_name`,
`traffic_specification`, `network_requirements` 0..1,
`data_frame_specification` 0..1, `datawriter_ref`, `time_aware` 0..1),
§7.2.3.2 Tab 7.24 (TsnListener: `name`, `stream_name`,
`network_requirements` 0..1, `datareader_ref`) and Figure 7.3
(TsnConfiguration aggregates `tsn_talker`/`tsn_listener`, 0..* each).

**Repo:** `crates/transport-tsn/src/stream.rs::{TsnTalker, TsnListener,
StreamIdentifier, TsnConfiguration}`.

`TsnTalker` covers all Tab 7.15 fields: `name`, `stream_name`,
`traffic_specification`, `network_requirements` (0..1),
`data_frame_specification` (0..1, `Option`), `datawriter_ref`,
`time_aware` (0..1). `TsnListener` (Tab 7.24): `name`, `stream_name`,
`network_requirements` (0..1), `datareader_ref`. `TsnConfiguration`
(Figure 7.3) aggregates `tsn_talker`/`tsn_listener` (0..* each).

**Tests:** `crates/transport-tsn/src/stream.rs::tests::{matching_stream_ids_match,
different_vlan_streams_do_not_match, time_aware_talker_can_be_time_critical,
talker_carries_optional_network_requirements_per_tab_7_15,
listener_carries_optional_network_requirements_per_tab_7_24,
tsn_configuration_aggregates_talkers_and_listeners}` (6).

**Status:** done

### Tab 7.18/7.25 NetworkRequirements

**Spec:** §7.2.3.1.2 Tab 7.18 (Talker) + §7.2.3.2.1 Tab 7.25 (Listener)
— `num_seamless_trees` (UInt8, IEEE 802.1CB FRER redundancy) +
`max_latency` (Talker `UInt32`, Listener `String8`; both nanoseconds,
unified to `u32`).

**Repo:** `crates/transport-tsn/src/network_requirements.rs::NetworkRequirements`.

**Tests:** `crates/transport-tsn/src/network_requirements.rs::tests::zero_trees_means_no_redundancy_per_spec_46_2_3_6_1`,
`crates/transport-tsn/src/network_requirements.rs::tests::one_or_more_trees_requests_frer`.

**Status:** done

### Tab 7.16 TrafficSpecification

**Spec:** §7.2.3.1.1 Tab 7.16 — `interval` (numerator/denominator),
`max_frames_per_interval`, `max_frame_size`, `transmission_selection`
with the four IEEE 802.1Q algorithms (strict priority, credit-based
shaper, enhanced transmission selection, asynchronous traffic shaping).

**Repo:** `crates/transport-tsn/src/traffic.rs::{TrafficSpecification,
TransmissionSelection}` with all 4 algorithms (StrictPriority/CBS/ETS/ATS)
+ a bytes_per_second computation.

**Tests:** `crates/transport-tsn/src/traffic.rs::tests::{transmission_selection_distinct_values,
bytes_per_second_calculation_for_1ms_interval_1500_byte_frame,
bytes_per_second_for_125us_interval_8x_64_byte_frame,
bytes_per_second_zero_interval_returns_max}` (4).

**Status:** done

### Tab 7.17 TimeAware

**Spec:** §7.2.3.1.3 Tab 7.17 — `earliest_transmit_offset`,
`latest_transmit_offset`, `jitter`; the transmit window is
`latest - earliest`, and the jitter has to fit inside it
(IEEE 802.1Qbv time-aware shaper).

**Repo:** `crates/transport-tsn/src/time_aware.rs::TimeAware` with a
window-length computation + an is_valid predicate.

**Tests:** `crates/transport-tsn/src/time_aware.rs::tests::{valid_window_is_latest_minus_earliest,
invalid_when_earliest_above_latest, invalid_when_jitter_exceeds_window,
zero_jitter_is_valid_boundary}` (4).

**Status:** done

### Tab 7.19 DataFrameSpecification

**Spec:** §7.2.3.1.4 Tab 7.19 — a stream's frame specification is exactly
one of three variants: IEEE 802 MAC addresses (Tab 7.20), an IPv4 tuple
(Tab 7.22) or an IPv6 tuple (Tab 7.23).

**Repo:** `crates/transport-tsn/src/data_frame.rs::DataFrameSpecification`
variant Mac/IPv4/IPv6.

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::{mac_specification_carries_source_and_destination,
ipv4_tuple_carries_5_tuple_fields, ipv6_tuple_uses_16_byte_addresses}` (3).

**Status:** done

### Tab 7.20 IEEE802MacAddresses

**Spec:** §7.2.3.1.4 Tab 7.20 — `destination_mac_address` +
`source_mac_address` as IEEE 802 MAC addresses (6 octets); the least
significant bit of the first octet separates unicast from multicast, the
second-least marks locally administered addresses.

**Repo:** `crates/transport-tsn/src/mac.rs::MacAddress` with
multicast/broadcast/locally-administered detection + a display format.

**Tests:** `crates/transport-tsn/src/mac.rs::tests::{broadcast_constant_is_all_ff,
unicast_lsb_zero_is_not_multicast, multicast_lsb_set_detected,
locally_administered_2nd_bit_set,
display_format_matches_canonical_colon_separated_hex}` (5).

**Status:** done

### Tab 7.21 IEEE802VlanTag

**Spec:** Tab 7.21 — the IEEE 802.1Q VLAN tag (TPID + PCP + DEI + VID,
4 bytes wire form).

**Repo:** `crates/transport-tsn/src/vlan_tag.rs::Ieee802VlanTag` with a
PCP limit (3-bit), VID limit (12-bit), reserved-VID detection,
to_wire/from_wire round-trip + bit-layout validation.

**Tests:** `crates/transport-tsn/src/vlan_tag.rs::tests::{well_known_tpid_constants,
pcp_values_0_through_7_accepted, pcp_above_7_rejected, vid_above_4095_rejected,
wire_round_trip_preserves_all_fields, wire_layout_matches_spec_bit_packing,
from_wire_too_short_fails, reserved_vids_detected}` (8) —
`wire_layout_matches_spec_bit_packing` checks the bit layout against
concrete hex values.

**Status:** done

### Tab 7.22 IPv4Tuple

**Spec:** §7.2.3 Tab 7.22 — `source_ip`, `destination_ip`, `dscp`
(RFC 2474, 64=ignore), `protocol`, `source_port`, `destination_port`
for IPv4.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv4Tuple` (incl.
the `dscp` field).

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::ipv4_tuple_carries_5_tuple_fields`.

**Status:** done

### Tab 7.23 IPv6Tuple

**Spec:** §7.2.3 Tab 7.23 — as Tab 7.22 for IPv6 (16-byte addresses),
incl. `dscp`.

**Repo:** `crates/transport-tsn/src/data_frame.rs::IPv6Tuple` (incl.
the `dscp` field).

**Tests:** `crates/transport-tsn/src/data_frame.rs::tests::ipv6_tuple_uses_16_byte_addresses`.

**Status:** done

---

## §7.2.1-§7.2.2 Application + deployment configuration

### §7.2.1 DDS Application Configuration

**Spec:** §7.2.1, Tab 7.1-7.7 (PDF) — the application library with
QosLibrary, DomainLibrary, ApplicationLibrary, ApplicationFactory,
ApplicationFactoryRequester, ApplicationInstance,
ApplicationInstanceRequester. Tab 7.2 names the six QoS categories of a
QosProfile; a DomainParticipant optionally inherits from another.

**Repo:** `crates/transport-tsn/src/pim/application.rs` (schema models for
the application library + factory).

**Tests:** `crates/transport-tsn/src/pim/application.rs::tests::{qos_profile_holds_six_qos_categories_per_spec_tab_72,
domain_carries_registered_types_and_topics,
domain_participant_inheritance_chain_is_optional,
application_holds_multiple_dp_refs}` (4).

**Status:** done

### §7.2.2 DDS Deployment Configuration

**Spec:** §7.2.2, Tab 7.8-7.14 (PDF) — the deployment library with
DeploymentTalker, DeploymentListener, BridgeNode, etc. A node carries its
MAC address and optional IP addresses; a deployment has multiplicity 1..*
of application instances.

**Repo:** `crates/transport-tsn/src/pim/deployment.rs`.

**Tests:** `crates/transport-tsn/src/pim/deployment.rs::tests::{node_holds_mac_and_optional_ips,
deployment_with_one_app_validates,
deployment_without_apps_is_invalid_per_spec_mult_1_star,
deployment_configuration_can_reference_tsn_block}` (4).

**Status:** done

---

## §7.3 Configuration Representation (PSM)

All three normative PSMs are covered.

### §7.3.1 XML PSM

**Spec:** §7.3.1, p. 21-22 — the XML PSM maps the configuration model onto
a `<dds_tsn>` root element with the six libraries (Tab 7.1-7.14).

**Repo:** `crates/transport-tsn/src/pim/xml.rs::parse_dds_tsn_xml` — loader
for `<dds_tsn>` per the spec XSD (Tab 7.1-7.14).

**Tests:** `crates/transport-tsn/src/pim/xml.rs::tests::{parses_full_sample_into_six_libraries,
qos_profile_carries_named_qos_categories, domain_carries_registered_types_and_topics,
node_with_mac_and_ipv4_parses, deployment_with_tsn_configuration_ref,
missing_root_yields_unexpected_root_error, missing_required_attribute_is_diagnostic,
invalid_mac_is_diagnostic, parse_mac_accepts_dash_separator,
parse_mac_rejects_short_input}` (10).

**Status:** done

### §7.3.2 JSON PSM

**Spec:** §7.3.2, p. 22-23 — the same configuration model in the spec's
JSON schema: six top-level arrays, QoS profiles with named categories,
node MACs in canonical spelling.

**Repo:** `crates/transport-tsn/src/pim/json.rs::render_dds_tsn_json` —
renderer in the spec JSON schema.

**Tests:** `crates/transport-tsn/src/pim/json.rs::tests::{render_includes_six_top_level_arrays,
render_qos_profile_emits_named_categories, render_node_emits_mac_in_canonical_form,
render_escapes_special_chars}` (4).

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

## §8 DDSI-RTPS wire protocol over TSN

### §8.1 DDSI-RTPS PIM unchanged

**Spec:** §8.1, p. 29 — "DDSI-RTPS PIM is unchanged from DDSI-RTPS 2.5."
TSN-specific changes only in the PSM (§8.3+§8.4).

**Repo:** `crates/rtps/src/header.rs` + `crates/rtps/src/submessages.rs` —
unchanged against DDSI-RTPS 2.5, no TSN-specific paths (see
`ddsi-rtps-2.5.md`).

**Tests:** that the PIM stays unchanged is shown by the same RTPS header
tests that hold without TSN:
`crates/rtps/src/header.rs::tests::{header_layout_first_four_bytes_are_rtps_magic,
header_protocol_version_at_bytes_4_5, header_total_size_is_20_bytes,
header_roundtrip}`; that the TSN path carries the same header:
`crates/transport-tsn/src/ethernet_psm.rs::tests::round_trip_with_vlan`.

**Status:** done

### §8.2 DDSI-RTPS conformance with TSN constraints

**Spec:** §8.2, p. 30-31 — conformance rules for RTPS-over-TSN
(latency-bounded path, frame-size limits).

**Repo:** `crates/rtps/src/fragment_assembler.rs::FragmentAssembler` (the
frame-size bound via the fragment layer);
`crates/transport-tsn/src/traffic.rs::TrafficSpecification::max_frame_size`
(the TSN-side upper bound).

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::{single_fragment_sample_completes_immediately,
two_fragments_complete_in_order, fragments_complete_out_of_order,
last_fragment_shorter_than_fragment_size, sample_too_large_drops_without_alloc}`;
`crates/transport-tsn/src/traffic.rs::tests::bytes_per_second_calculation_for_1ms_interval_1500_byte_frame`;
`crates/transport-tsn/src/ethernet_psm.rs::tests::truncated_input_is_error`.

**Status:** done

### §8.3 DDSI-RTPS UDP/IP PSM over TSN — DSCP + VLAN tag

**Spec:** §8.3, p. 31-32 — RTPS over UDP/IP inside a TSN network marks its
packets with a DSCP codepoint (RFC 2474, the top 6 bits of the ToS octet)
and an IEEE 802.1Q VLAN tag carrying the PCP priority.

**Repo:** `crates/transport-tsn/src/dscp.rs::Dscp` (RFC 2474 with
DEFAULT/EF/AF11/AF21/AF31/AF41 constants + ToS-octet round-trip); VLAN-tag
insertion in `vlan_tag.rs`.

**Tests:** `crates/transport-tsn/src/dscp.rs::tests::{dscp_well_known_constants_match_rfc,
dscp_above_63_rejected, tos_octet_round_trip_preserves_dscp,
from_tos_ignores_ecn_bits}` (4);
`crates/transport-tsn/src/vlan_tag.rs::tests::{wire_layout_matches_spec_bit_packing,
wire_round_trip_preserves_all_fields, pcp_values_0_through_7_accepted,
pcp_above_7_rejected}`.

**Status:** done

### §8.4 DDSI-RTPS Ethernet PSM (refers to Annex A)

**Spec:** §8.4, p. 32 — for the Ethernet PSM §8.4 refers to Annex A: RTPS
messages sit directly in the Ethernet frame payload under their own
EtherType, without an IP and UDP layer.

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs`.

**Tests:** `crates/transport-tsn/src/ethernet_psm.rs::tests::{rtps_ethertype_constant,
header_without_vlan_is_14_bytes, header_with_vlan_is_18_bytes,
round_trip_without_vlan, round_trip_with_vlan, truncated_input_is_error,
ipv4_ethertype_parsed_without_vlan_branch}` (7).

**Status:** done

---

## Annexes

### Annex A — DDSI-RTPS Ethernet PSM

**Spec:** Annex A, p. 33-36 — RTPS directly in the Ethernet frame payload.

**Repo:** `crates/transport-tsn/src/ethernet_psm.rs::{EthernetFrameHeader,
ETHERTYPE_RTPS}`. Header with/without VLAN tag (14 vs 18 bytes),
round-trip, truncation detection, IPv4-EtherType non-VLAN branch.

**Live transport (feature `live`, Linux):**
`crates/transport-tsn/src/socket.rs::TsnTransport` sends/receives RTPS
directly in the Ethernet frame over an `AF_PACKET`/`SOCK_RAW` socket; the
platform-neutral frame logic (VLAN selection, frame build + min-frame
padding, sysfs MAC parsing) lives in `live_frame.rs`.

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
parse_mac_sysfs_non_hex}` (12, platform-neutral, feature `live`);
`crates/transport-tsn/tests/veth_loopback.rs` — a real RTPS roundtrip over a
veth pair (root, CI `tsn-live` job, see `internal/ci/tsn-live.md`).

**Status:** done

---

### Annex B — Integration Examples

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
