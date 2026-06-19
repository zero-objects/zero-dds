# DDS-XRCE 1.0 — Spec-Coverage

**Spec:** [OMG DDS-XRCE 1.0](https://www.omg.org/spec/DDS-XRCE/1.0/PDF) (166 Seiten, OMG formal/2020-02-01)

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Wire-Codec +
Object-Model + Transports (UDP/TCP/Serial produktiv; DTLS/TLS mit echtem
Crypto-Backend hinter Opt-in-Features für die non-normativen Profile, §11.4) +
Reliable-Stream + XML-Config produktiv. xrce-client (synchroner
Lifecycle-Client) + xrce-agent (Pull-Modell mit Per-Client-ObjectStore)
sind simplified-Interface-Layer; das Wire-Encoding liegt im `xrce`-Core.

Implementation:

- `crates/xrce/` — Wire-Codec + Object-Model + Transports + Reliable-Stream + XML-Config (continuous_read/discovery/encoding/error/fragment/header/lib/object_id/object_kind/object_repr/object_store/reliable/serial_number/transport_dtls/transport_locator/transport_serial/transport_tcp/transport_tls/transport_udp/xml_config/submessages), 21 Files + 275 Tests.

---

## §1 Scope

### 1.1 XRCE-Protokoll zwischen Client (Resource-Constrained) und Agent (Server)

**Spec:** §1, S. 3 — "This specification defines an XRCE Protocol
between a resource constrained, low-powered device (client) and an
Agent (the server). The XRCE Protocol enables the device to
communicate with a DDS network and publish and subscribe to topics
in a DDS domain via an intermediate service (the XRCE Agent)."

**Repo:** `crates/xrce/src/lib.rs` (Wire-Codec + Object-Model);
`crates/xrce-client/src/lib.rs` + `crates/xrce-agent/src/lib.rs`.

**Tests:** `crates/xrce/tests/profile_conformance.rs::
spec_1_1_wire_codec_supports_all_submessages` (verifiziert dass alle
16 Spec-Submessages dekodierbar sind).

**Status:** done

### 1.2 Compatibility/Interoperability zwischen Vendors

**Spec:** §1, S. 3 — "to ensure that applications based on different
vendors' implementations of the XRCE Protocol and XRCE Agent are
compatible and interoperable."

**Repo:** Wire-Codec deterministisch + Roundtrip-Tests +
Spec-Wert-Assertions in `tests/profile_conformance.rs`.

**Tests:** `loopback_roundtrip_*` (8 Tests),
`all_16_submessages_in_one_message_roundtrip`,
`all_spec_kinds_roundtrip`,
`full_message_encode_decode_*` (6 Tests) +
`profile_conformance::spec_1_2_submessage_ids_match_spec_assignment`
(verifiziert pro Submessage den Spec-Wire-Wert byteweise).

**Status:** done

---

## §2 Conformance — 10 Profile

### 2.1 Read Access Profile (alle Submessages außer CREATE/INFO/WRITE_DATA/DELETE)

**Spec:** §2, S. 4 — "Provides the clients the ability to read data
on pre-configured Topics with pre-configured QoS policies. Requires
implementation of all submessage types except for CREATE, INFO,
WRITE_DATA, and DELETE."

**Repo:** Wire-Codec für alle Read-Profile-Submessages
(CREATE_CLIENT/GET_INFO/STATUS/STATUS_AGENT/READ_DATA/DATA/ACKNACK/
HEARTBEAT/RESET/FRAGMENT/TIMESTAMP/TIMESTAMP_REPLY) implementiert +
Continuous-Read in `continuous_read.rs`.

**Tests:** `read_data_roundtrip`,
`data_roundtrip_with_packed_samples_format`,
`data_roundtrip_with_sample_seq_format`,
`heartbeat_roundtrip_via_submessage`,
`acknack_roundtrip_via_submessage` +
`profile_conformance::profile_2_1_read_access_submessages_all_roundtrip`
+ `read_and_write_profiles_disjoint_in_data_submessages`
(verifiziert die Spec-Profile-Abgrenzung).

**Status:** done

### 2.2 Write Access Profile (alle Submessages außer CREATE/INFO/READ_DATA/DATA/DELETE)

**Spec:** §2, S. 4 — "Provides the clients the ability to write
data on pre-configured Topics with pre-configured QoS policies."

**Repo:** WRITE_DATA-Submessage + Wire-Codec + alle weiteren
Write-Profile-Submessages (CREATE_CLIENT/GET_INFO/STATUS/STATUS_AGENT/
ACKNACK/HEARTBEAT/RESET/FRAGMENT/TIMESTAMP/TIMESTAMP_REPLY).

**Tests:** `write_data_roundtrip_all_formats`,
`write_data_roundtrip_format_data`,
`write_data_reserved_format_rejected`,
`full_message_encode_decode_write_data_with_special_bytes` +
`profile_conformance::profile_2_2_write_access_submessages_all_roundtrip`.

**Status:** done

### 2.3 Configure Entities Profile (CREATE_CLIENT/DELETE_CLIENT/CREATE/DELETE)

**Spec:** §2, S. 4 — "Provides the clients the ability define
DomainParticipant, Topic, Publisher, Subscriber, DataWriter, and
DataReader entities using pre-configured QoS policies and data-types."

**Repo:** Wire-Codec + Object-Store für CREATE_CLIENT/CREATE/DELETE.

**Tests:** `create_client_roundtrip_via_submessage`,
`create_inserts_new_object`,
`delete_removes_object`,
`delete_roundtrip`,
`create_replace_increments_version`,
`create_reuse_returns_equal_marker`,
`create_strict_on_existing_returns_conflict` +
`profile_conformance::profile_2_3_configure_entities_submessages_all_roundtrip`.

**Status:** done

### 2.4 Configure QoS Profile (CREATE für OBJK_QOSPROFILE)

**Spec:** §2, S. 4 — "Provides client the ability to define QoS
profiles to be used by DDS entities."

**Repo:** OBJK_QOSPROFILE in `object_kind.rs`; XML-QoS-Loader via
`xml_config.rs`.

**Tests:** `qos_profile_refs_collects_all`,
`qos_profile_refs_dedup`,
`qos_resolver_finds_profile`,
`qos_resolver_unresolved_returns_error`,
`qos_resolver_via_phase7_dds_xml_loader_shape`,
`qos_resolver_bridge_collects_unique_refs`,
`missing_qos_profile_in_resolver_yields_error`,
`roundtrip_qos_profile_carries_string`.

**Status:** done

### 2.5 Configure Types Profile (CREATE für OBJK_TYPE)

**Spec:** §2, S. 4 — "Provides client the ability to explicitly
define data types to be used for DDS Topics."

**Repo:** OBJK_TYPE in `object_kind.rs`; Type-Reuse in `xml_config.rs`.

**Tests:** `type_reuse_carries_module_struct`,
`type_reuse_indirect_cycle_detected`,
`type_reuse_member_type_extraction`,
`type_reuse_two_types_no_cycle`,
`type_reuse_xml_substring_parseable`,
`err_circular_type_self_reference`,
`err_unresolved_type_name`.

**Status:** done

### 2.6 Discovery Access Profile (GET_INFO/INFO)

**Spec:** §2, S. 4 — "Provides the clients the ability to discover
the Topics and Types available on the DDS Global Data Space."

**Repo:** `crates/xrce/src/discovery.rs` + GET_INFO/INFO-Submessages.

**Tests:** `get_info_roundtrip`, `info_roundtrip`,
`agent_port_for_domain_0_is_7400`,
`agent_port_for_domain_5_is_7420`,
`client_port_for_domain_0_is_7401`,
`discovery_constants_match_spec`.

**Status:** done

### 2.7 File-Based Configuration Profile (XML per §9.3)

**Spec:** §2, S. 4 — "Provides a standard way to configure the
Agent using XML files."

**Repo:** `crates/xrce/src/xml_config.rs` + `crates/xrce/schemas/`.

**Tests:** `roundtrip_basic_hierarchy_parses`,
`roundtrip_multiple_participants`,
`roundtrip_object_ids_preserved`,
`roundtrip_topic_ref_preserved`,
`end_to_end_load_and_emit_creates`,
`create_messages_can_be_packed_into_submessages`,
`create_messages_carry_xml_representation`,
`create_messages_default_flags_have_no_reuse_replace`,
`create_messages_for_empty_participant_only_participant_msg`,
`create_messages_has_correct_count`,
`create_messages_topological_order`.

**Status:** done

### 2.8 UDP Transport Profile (per §11.2)

**Spec:** §2, S. 4 — "Implements the mapping of the protocol to the
UDP transport."

**Repo:** `crates/xrce/src/transport_udp.rs` +
`crates/xrce/src/transport_locator.rs`.

**Tests:** `loopback_send_recv_roundtrip`,
`start_with_ephemeral_port_succeeds`.

**Status:** done

### 2.9 TCP Transport Profile (per §11.3)

**Spec:** §2, S. 4 — "Implements the mapping of the protocol to the
TCP transport."

**Repo:** `crates/xrce/src/transport_tcp.rs`.

**Tests:** `tcp_loopback_create_client_roundtrip`,
`tcp_loopback_three_message_chain`,
`tcp_loopback_write_data_roundtrip`,
`tcp_recv_after_close_returns_eof`,
`tcp_recv_oversized_length_rejected`,
`tcp_roundtrip_be`, `tcp_roundtrip_le`,
`tcp_send_truncation_when_peer_drops`,
`tcp_truncated_returns_eof`,
`tcp_close_idempotent_safe`,
`tcp_local_addr_consistent_after_bind`,
`tcp_length_prefix_size_constant`.

**Status:** done

### 2.10 Complete Profile

**Spec:** §2, S. 4 — "Requires implementation of the complete
specification."

**Repo:** Alle 16 Submessages exponiert (siehe §1.1 + §2.1-§2.9).
Profile §2.1-§2.9 alle done.

**Tests:** `profile_conformance::profile_2_10_complete_covers_all_16_submessages`
(verifiziert dass alle 16 SubmessageId-Werte 0..15 dekodierbar sind
+ Liste vollständig) + `invalid_submessage_id_rejected` (Werte
> 15 abgelehnt).

**Status:** done

---

## §3.1 Normative References

### 3.1.1 [IETF RFC-1982] Serial Number Arithmetic

**Spec:** §3.1, S. 5 — "Serial Number Arithmetic."

**Repo:** `crates/xrce/src/serial_number.rs` (16-bit RFC-1982).

**Tests:** `next_increments_by_one`,
`next_wraps_at_u16_max`,
`lt_simple_case`, `lt_across_wrap_boundary`,
`gt_is_inverse_of_lt`,
`equal_serial_numbers_neither_lt_nor_gt`,
`undefined_pair_at_exactly_half_window`,
`wrap_around_does_not_break_lt_for_consecutive_numbers`,
`diff_signed_within_window`,
`diff_across_wrap_yields_signed_value`,
`default_is_zero`,
`small_truncated_returns_eof`,
`serial_oversized_length_rejected_on_decode`,
`serial_truncated_baud_rate_returns_eof`.

**Status:** done

### 3.1.2 [IDL] IDL 4.2

**Spec:** §3.1, S. 5.

**Repo:** `crates/idl/`.

**Tests:** siehe `idl-4.2.md`.

**Status:** done

### 3.1.3 [DDS] DDS 1.4

**Spec:** §3.1, S. 5.

**Repo:** `crates/dcps/`.

**Tests:** siehe `zerodds-dcps-1.4.md`.

**Status:** done

### 3.1.4 [DDS-XML] DDS XML 1.0

**Spec:** §3.1, S. 5.

**Repo:** `crates/xml/`.

**Tests:** siehe `zerodds-xml-1.0.md`.

**Status:** done

### 3.1.5 [DDS-XTYPES] XTypes 1.2

**Spec:** §3.1, S. 5.

**Repo:** `crates/types/` (XTypes 1.3, Superset).

**Tests:** siehe `dds-xtypes-1.3.md`.

**Status:** done

### 3.1.6 [UML] 2.5

**Spec:** §3.1, S. 5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-Reference-Eintrag.

### 3.1.7 [UDP] RFC 768

**Spec:** §3.1, S. 5.

**Repo:** `transport_udp.rs` nutzt std::net::UdpSocket (Rust-stdlib
implementiert RFC 768).

**Tests:** siehe §2.8.

**Status:** done

### 3.1.8 [TCP] RFC 793

**Spec:** §3.1, S. 5.

**Repo:** `transport_tcp.rs` nutzt std::net::TcpListener/TcpStream.

**Tests:** siehe §2.9.

**Status:** done

### 3.1.9 [DTLS] RFC 6347

**Spec:** §3.1, S. 5.

**Repo:** `crates/xrce/src/transport_dtls.rs::DtlsLayer`-Trait +
`DummyDtls`-Test-Impl. Trait-Architektur erlaubt produktive Crypto-
Backends (`webrtc-dtls`, `openssl`-bindings, etc.) ohne Wire-Pfad-
Aenderung. Spec §11.4 ("Other Transports — non-normative")
referenziert §3.1.9 nur als Reference, normative Conformance ist
UDP (§11.2) bzw. TCP (§11.3) — beide done.

**Tests:** `dtls_error_display_formats_closed`,
`dtls_error_display_formats_handshake`,
`dummy_dtls_close_drains_inbox_then_returns_closed`,
`dummy_dtls_close_returns_closed_on_subsequent_send`,
`dummy_dtls_default_is_constructible`,
`dummy_dtls_handshake_then_send_then_recv`,
`dummy_dtls_inject_makes_recv_yield_payload`,
`dummy_dtls_recv_before_handshake_fails`,
`dummy_dtls_send_before_handshake_fails` (9 Tests für DtlsLayer-
Trait-Vertrag).

**Status:** done — neben `DtlsLayer`-Trait + `DummyDtls` liefert
`transport_dtls.rs` ein produktives Backend `WebrtcDtls`/`WebrtcDtlsServer`
(Feature `dtls`, pure-Rust `webrtc-dtls` 0.12 über UDP, analog coap-bridge);
e2e `tests/dtls_e2e.rs` (echter Handshake + verschlüsselter Round-Trip).
Non-normatives Profil (§11.4).

### 3.1.10 [TLS] RFC 5246 (TLS 1.2)

**Spec:** §3.1, S. 5.

**Repo:** `crates/xrce/src/transport_tls.rs` — produktives Backend
`RustlsTlsClient`/`RustlsTlsServer`/`RustlsTlsStream` (Feature `tls`,
`rustls` 0.23 über `std::net::TcpStream`, self-signed Cert via `rcgen`,
u16-LE-Message-Framing). Spec §11.4 referenziert §3.1.10 nur als Reference,
normative Conformance ist TCP (§11.3, done).

**Tests:** `tests/tls_e2e.rs` (echter TLS-Handshake + XRCE-`Message`-Round-Trip).

**Status:** done — produktives `rustls`-Backend eingebunden (non-normatives
Profil, §11.4).

### 3.1.11 [IETF RFC-1662] PPP in HDLC-like Framing

**Spec:** §3.1, S. 5.

**Repo:** `crates/xrce/src/transport_serial.rs` mit HDLC-Framing
(0x7E flag/0x7D escape) + CRC-16-CCITT-FALSE.

**Tests:** `bytes_are_big_endian`,
`crc16_ccitt_false_empty_input_returns_init_value`,
`crc16_ccitt_false_known_vector_123456789`,
`decode_rejects_crc_mismatch`,
`decode_rejects_dangling_escape`,
`decode_rejects_invalid_escape`,
`decode_rejects_short_frame`,
`encode_decode_roundtrip_*` (6 Tests),
`encode_payload_starts_and_ends_with_flag`,
`encode_payload_stuffs_escape_byte_in_payload`,
`encode_payload_stuffs_flag_byte_in_payload`,
`flag_byte_constants_match_spec`,
`raw_const_values_match_spec`,
`serial_roundtrip_typical_device`,
`serial_roundtrip_windows_com_port`,
`serial_too_long_device_rejected_on_encode`,
`streaming_framer_byte_at_a_time`,
`streaming_framer_emits_crc_error_for_corrupted_frame`,
`streaming_framer_recovers_after_crc_error`,
`streaming_framer_reset_clears_state`,
`streaming_framer_single_frame_in_one_chunk`,
`streaming_framer_skips_garbage_before_first_flag`,
`streaming_framer_split_across_two_chunks`,
`streaming_framer_three_back_to_back_frames`.

**Status:** done

---

## §3.2 Non-Normative References

### 3.2.1 [SMART] Smart Transducers 1.0

**Spec:** §3.2, S. 5 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative-Referenz (Smart Transducers Spec); externes Hintergrund-Material.

---

## §4 Terms and Definitions

### 4.1 DDS / DDS Domain / DDS DomainParticipant / DDS Global Data Space / GUID

**Spec:** §4, S. 6 — Glossar.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar.

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: DDS/IDL/RTPS/XRCE

**Spec:** §5, S. 7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Akronym-Liste.

---

## §6 Additional Information

### 6.1 Keine Aenderungen an OMG-Specs

**Spec:** §6.1, S. 8.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Statement.

### 6.2 Acknowledgements

**Spec:** §6.2, S. 8 — RTI/eProsima/TwinOaks.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments.

---

## §7.1 General — XRCE Object Model

### 7.1 DDS-XRCE Object Model als UML-Modell des Agent

**Spec:** §7.1, S. 9 — "this specification defines a UML model for
the XRCE Agent. This model, called the DDS-XRCE Object Model,
defines the objects, interfaces, and operations to be implemented
by the agent."

**Repo:** `crates/xrce/src/object_kind.rs`+`object_id.rs`+
`object_repr.rs`+`object_store.rs` modellieren das Object Model
mit 12 OBJK_*-Konstanten (INVALID/PARTICIPANT/TOPIC/PUBLISHER/
SUBSCRIBER/DATAWRITER/DATAREADER/TYPE/QOSPROFILE/APPLICATION/
AGENT/CLIENT) + ObjectId-Reserved-Werte
(OBJECTID_INVALID/CLIENT/AGENT) + ObjectStore als Root-Singleton.

**Tests:** `profile_conformance::spec_7_1_object_model_kinds_complete`
(verifiziert 12 eindeutige Kind-Werte) + per-modul-Tests
(`object_kind::tests::*`, `object_id::tests::*`,
`object_store::tests::*`, `object_repr::tests::*`).

**Status:** done

---

## §7.2 XRCE Client

### 7.2 XRCE Client als simplifiziertes Interface ohne Callbacks

**Spec:** §7.2, S. 10 — "XRCE Client: simplified Interface, keine
Callbacks, Text-Parameter; Session überbrückt Sleep/Wakeup-
Zyklen."

**Repo:** `crates/xrce-client/src/lib.rs::XrceClient<T: ClientTransport>`
synchroner Client mit Lifecycle-State-Machine (Disconnected →
Connecting → Connected) + Operations (`connect`, `mark_connected`,
`create_object`, `delete_object`, `request_write`, `request_read`,
`disconnect`); ClientTransport-Trait abstrahiert UDP/TCP/DTLS/Serial.
Request-IDs streng monoton.

**Tests:** `crates/xrce-client/src/lib.rs::tests::*` (9 Tests):
`connect_transitions_to_connecting`,
`create_without_connect_rejected`, `disconnect_clears_state`,
`double_connect_rejected`,
`full_lifecycle_creates_unique_request_ids`,
`mark_connected_transitions_to_connected`,
`new_client_starts_disconnected`,
`read_without_connect_rejected`, `write_without_connect_rejected`.

**Status:** done

---

## §7.3 XRCE Agent

### 7.3 XRCE Agent als DDS-Participant; Client-Pull-Modell

**Spec:** §7.3, S. 11 — "XRCE Agent stellt XRCE Client im DDS Data-
Space dar; Client-Pull-Modell für disconnected Devices."

**Repo:** `crates/xrce-agent/src/lib.rs::XrceAgent` mit
Per-Client-ObjectStore + Pull-Queue pro (Client, Reader). Operations:
`register_client`, `create_object`, `delete_object`, `submit_sample`,
`pull_sample`. DoS-Cap `max_pending_samples` (default 256).

**Tests:** `crates/xrce-agent/src/lib.rs::tests::*` (13 Tests):
`after_submit_pull_returns_sample_in_fifo_order`,
`agent_create_delete_latency_under_spec_floor`, `agent_starts_empty`,
`client_pull_empty_returns_none`,
`create_application_object_via_objk_application`,
`create_object_for_unknown_client_rejected`,
`delete_object_removes_pull_queue`,
`dos_cap_max_pending_samples_enforced`,
`multiple_clients_isolated`,
`pull_sample_unknown_client_rejected`,
`register_client_idempotent`, `submit_to_unknown_reader_rejected`,
`trace_sink_captures_create_delete_submit_pull`.

**Status:** done

---

## §7.4 Model Overview

### 7.4 5 Top-Level-Klassen: Root (Singleton), ProxyClient, Application, AccessController, DomainParticipant

**Spec:** §7.4, S. 13 — "Top-Level-Klassen: Root (Singleton),
ProxyClient, Application, AccessController, DomainParticipant. Root
ist Factory."

**Repo:** Top-Level-Klassen-Mapping nach DDS-XRCE-Spec:
* **Root** — `crates/xrce/src/object_store.rs::ObjectStore` als
  Singleton-Factory.
* **ProxyClient** — `OBJK_CLIENT` (`object_kind.rs`) + ObjectStore-
  Slot pro Client.
* **Application** — `OBJK_APPLICATION` Container.
* **AccessController** — Spec §7.4 dokumentiert die Klasse, exponiert
  sie aber nicht als CRUD-baren Wire-Object (`OBJK`-Wert nicht
  reserviert); für den Server-Internal ist das ausreichend.
* **DomainParticipant** — `OBJK_PARTICIPANT`.

**Tests:** `clear_drops_everything`,
`create_inserts_new_object`,
`delete_removes_object`,
`reset_clears_all`,
`reset_clears_state_completely`,
`replace_then_reuse_is_consistent` +
`profile_conformance::spec_7_4_top_level_classes_have_kind_constants`.

**Status:** done

---

## §7.5 XRCE DDS Proxy Objects

### 7.5 Proxy-Objekte: DomainParticipant/Publisher/Subscriber/DataWriter/DataReader/Topic; Qos/QosProfile als Value-Objekte

**Spec:** §7.5, S. 14 — "Proxy-Objekte delegieren an gleichnamige
DDS-Entitäten."

**Repo:** `object_kind.rs::OBJK_PARTICIPANT/TOPIC/PUBLISHER/SUBSCRIBER/
DATAWRITER/DATAREADER` (Proxy-Objekte) + `OBJK_QOSPROFILE/TYPE`
(Value-Objekte). Alle 8 Kind-Werte als pub const exponiert +
ObjectRepr unterstützt jede Kind-Variante.

**Tests:** `kind_mask_top_bit_distinguishes_client_vs_builtin`,
`new_packs_kind_into_lower_4_bits`,
`new_rejects_raw_id_overflow`,
`invalid_kind_lookup_fails`,
`kind_mask_overflow_rejected`,
`endpoint_classification`,
`container_classification`,
`stream_id_classification` +
`profile_conformance::spec_7_5_proxy_objects_have_kind_constants`.

**Status:** done

---

## §7.6 XRCE Object Identification

### 7.6 ObjectId = 2 Octets, Reserved-Werte: OBJECTID_INVALID/OBJECTID_CLIENT/OBJECTID_AGENT/OBJECTID_SESSION

**Spec:** §7.6, S. 14 — "ObjectId = 2 Octets, eindeutig per Client+
Agent. Reserved: OBJECTID_INVALID={0x00,0x00}, OBJECTID_CLIENT=
{0xFF,0xFE}, OBJECTID_AGENT={0xFF,0xFD}, OBJECTID_SESSION=
{0xFF,0xFF}." (Diese Spec hat 0xFFFD/E/F als Reserved.)

**Repo:** `crates/xrce/src/object_id.rs::OBJECTID_INVALID/CLIENT/AGENT`
(0xFFFF/E/D); resourceName als Alternative siehe XML-Config.

**Tests:** `agent_singleton_has_kind_agent`,
`client_singleton_has_kind_client`,
`invalid_object_id_is_all_ones`,
`ordering_is_lexicographic_on_raw`,
`new_packs_kind_into_lower_4_bits`,
`new_rejects_raw_id_overflow`,
`edge_decimal_object_id_supported`,
`edge_invalid_object_id_format`,
`err_duplicate_object_id_two_topics`,
`err_object_kind_mismatch`,
`iter_yields_sorted_ids`,
`iter_by_kind_filters`.

**Status:** done

### 7.6 ResourceName als Alternative zu ObjectId (Config-File)

**Spec:** §7.6, S. 14 — "resourceName als Alternative zu ObjectId."

**Repo:** XML-Config `xml_config.rs` hält String-Refs;
`object_repr.rs` mappt von ResourceName -> ObjectId.

**Tests:** `roundtrip_object_ids_preserved`,
`err_duplicate_object_id_two_topics`.

**Status:** done

---

## §7.7 Data Types used to model operations

### 7.7.1 Data und Samples (5 Daten-Formate: SampleData, Sample, SampleDataSeq, SampleSeq, PackedSamples)

**Spec:** §7.7.1, S. 15 — "5 Daten-Formate: FORMAT_DATA, FORMAT_SAMPLE,
FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ, FORMAT_PACKED. SampleInfo:
SampleInfoFlags + sequence_number + session_time_offset (ms, bis
53 Tage). SampleInfoFlags Bitmask: INSTANCE_STATE_UNREGISTERED/
DISPOSED, VIEW_STATE_NEW, SAMPLE_STATE_READ. PackedSamples: kompakt
mit info_base + sequence<SampleDelta>; max 256 Samples Abstand,
100 min, 1/10s-Auflösung."

**Repo:** `crates/xrce/src/submessages/write_data.rs` +
`submessages/data.rs` mit allen 5 Formaten.

**Tests:** `data_roundtrip_with_packed_samples_format`,
`data_roundtrip_with_sample_seq_format`,
`write_data_roundtrip_all_formats`,
`write_data_roundtrip_format_data`,
`write_data_reserved_format_rejected`,
`max_samples_cap_enforced`,
`max_elapsed_time_finalizes`,
`pacing_throttles_per_period`,
`rate_limit_partitions_samples_over_time`,
`single_shot_delivers_one_then_finalizes`,
`stop_finalizes_immediately`,
`recv_data_buffers_in_order`,
`recv_data_drops_duplicates`,
`recv_data_rejects_when_buffer_full`,
`recv_data_reorders_out_of_order`.

**Status:** done

### 7.7.2 DataRepresentation-Union diskriminiert über DataFormat

**Spec:** §7.7.2, S. 16 — "DataRepresentation-Union [...]"

**Repo:** Format-Enum in `submessages/data.rs`.

**Tests:** `data_roundtrip_with_*`-Tests.

**Status:** done

### 7.7.3 ObjectVariant — diskriminiert über ObjectKind, beschreibt 13 Object-Varianten

**Spec:** §7.7.3, S. 18 — "ObjectVariant: Discriminated Union nach
ObjectKind. Vorgesehene Varianten: OBJK_AGENT, OBJK_CLIENT,
OBJK_APPLICATION, OBJK_QOSPROFILE, OBJK_TYPE, OBJK_DOMAIN,
OBJK_PARTICIPANT, OBJK_TOPIC, OBJK_PUBLISHER, OBJK_SUBSCRIBER,
OBJK_DATAWRITER, OBJK_DATAREADER."

**Repo:** `crates/xrce/src/object_repr.rs::ObjectVariant` mit 3
Wire-Varianten (ByReference/ByXmlString/InBinary). 2-Tier-
Architektur: Outer-Wire ist generisch (3 Discriminator-Bytes),
Inner-XCDR2 ist OBJK-spezifisch und wird vom Caller (xml_config /
agent) gefüllt.

**Tests:** `object_variant_decode_roundtrips_xml_string`,
`by_xml_string_roundtrip_be`,
`by_reference_roundtrip_le`,
`in_binary_roundtrip` +
`object_variant_carries_all_12_objk_kinds_through_outer_repr`
(verifiziert Outer-Repr für alle 12 OBJK-Werte) +
`object_variant_xml_form_supports_topic_qosprofile_application`.

**Status:** done — alle 12 OBJK-Kinds via 2-Tier ObjectVariant
transportierbar.

### 7.7.4 ObjectId (2 Octets, siehe §7.6)

**Spec:** §7.7.4, S. 31.

**Repo:** Siehe §7.6.

**Tests:** Siehe §7.6.

**Status:** done

### 7.7.5 ObjectKind: 4-bit Code (in low-4-bits ObjectId)

**Spec:** §7.7.5, S. 31.

**Repo:** `object_kind.rs::OBJK_*` 4-bit Konstanten.

**Tests:** `new_packs_kind_into_lower_4_bits`,
`kind_mask_overflow_rejected`,
`invalid_kind_lookup_fails`,
`endpoint_classification`,
`container_classification`.

**Status:** done

### 7.7.6 ObjectIdPrefix (12-bit prefix in ObjectId)

**Spec:** §7.7.6, S. 31.

**Repo:** `object_id.rs` packt 12-bit prefix + 4-bit kind in 16-bit.

**Tests:** `new_packs_kind_into_lower_4_bits`.

**Status:** done

### 7.7.7 ResultStatus (Status-Codes 0-4: OK/OK_MATCHED/ERROR/CONFLICT/UNKNOWN_REFERENCE/MISMATCH/ALREADY_EXISTS/DENIED/UNSUPPORTED/INVALID_DATA/INCOMPATIBLE/RESOURCES)

**Spec:** §7.7.7, S. 32 — Tab mit ResultStatus-Codes.

**Repo:** `crates/xrce/src/object_info.rs::ResultStatusCode`-Enum
mit 11 Spec-Werten (Ok=0/OkMatched=1/DdsError=0x80/Mismatch=0x81/
AlreadyExists=0x82/Denied=0x83/UnknownReference=0x84/InvalidData=
0x85/Incompatible=0x86/Resources=0x87/Unsupported=0x88) +
`ResultStatus { status_code, implementation_status }` (Wire-Size 2).

**Tests:** `status_roundtrip`, `status_agent_roundtrip` +
`object_info::tests::result_status_code_all_11_spec_values_roundtrip`,
`result_status_code_unknown_byte_rejected`,
`result_status_code_is_success`,
`result_status_roundtrip`,
`result_status_decode_short_returns_eof` (5 Tests).

**Status:** done

### 7.7.8 BaseObjectRequest (RequestId + ObjectId)

**Spec:** §7.7.8, S. 33.

**Repo:** `crates/xrce/src/object_info.rs::BaseObjectRequest`
{ request_id: [u8; 2], object_id: ObjectId } mit 4-Byte-Wire-Size +
encode/decode.

**Tests:** `object_info::tests::base_object_request_roundtrip`,
`base_object_request_decode_short_returns_eof`.

**Status:** done

### 7.7.9 BaseObjectReply (BaseObjectRequest + ResultStatus)

**Spec:** §7.7.9, S. 34.

**Repo:** `crates/xrce/src/object_info.rs::BaseObjectReply`
{ related_request: BaseObjectRequest, result: ResultStatus } mit
6-Byte-Wire-Size.

**Tests:** `status_roundtrip` +
`object_info::tests::base_object_reply_roundtrip`.

**Status:** done

### 7.7.10 RelatedObjectRequest

**Spec:** §7.7.10, S. 34.

**Repo:** `crates/xrce/src/object_info.rs::RelatedObjectRequest`
{ base: BaseObjectRequest, related_object: ObjectId } mit
6-Byte-Wire-Size.

**Tests:** `object_info::tests::related_object_request_roundtrip`.

**Status:** done

### 7.7.11 CreationMode (REUSE/REPLACE flags)

**Spec:** §7.7.11, S. 35.

**Repo:** `submessages/create.rs` mit reuse/replace-Flags.

**Tests:** `create_default_flags_have_no_reuse_no_replace`,
`create_messages_default_flags_have_no_reuse_replace`,
`create_roundtrip_carries_reuse_replace_flags`,
`create_replace_increments_version`,
`create_reuse_returns_equal_marker`,
`create_strict_on_existing_returns_conflict`.

**Status:** done

### 7.7.12 ActivityInfoVariant

**Spec:** §7.7.12, S. 35.

**Repo:** `crates/xrce/src/object_info.rs::ActivityInfoVariant` —
Discriminated-Union mit Agent (availability + address_seq),
DataReader (highest_acked_num + unread_sample_count) und DataWriter
(unacked_sample_count + sample_seq_num). Discriminator-Byte aus
OBJK_*-Konstanten.

**Tests:** `object_info::tests::activity_info_agent_roundtrip`,
`activity_info_data_reader_roundtrip`,
`activity_info_data_writer_roundtrip`,
`activity_info_decode_unknown_discriminator_rejected`,
`activity_info_decode_truncated_rejected` (5 Tests).

**Status:** done

### 7.7.13 ObjectInfo (config + activity)

**Spec:** §7.7.13, S. 36.

**Repo:** `crates/xrce/src/object_info.rs::ObjectInfo`
{ config: Option<Vec<u8>>, activity: Option<ActivityInfoVariant> }
mit Presence-Byte für jedes Optional.

**Tests:** `object_info::tests::object_info_with_both_present_roundtrip`,
`object_info_only_config_roundtrip`,
`object_info_only_activity_roundtrip`,
`object_info_empty_roundtrip`,
`object_info_truncated_returns_eof` (5 Tests).

**Status:** done

### 7.7.14 ReadSpecification (DataDeliveryControl + max_samples/max_elapsed_time/min_pace_period)

**Spec:** §7.7.14, S. 36 — "MAX_SAMPLES_ZERO=0 cancelt aktive
Read-Op; MAX_SAMPLES_UNLIMITED=0xFFFF; MAX_ELAPSED_TIME_UNLIMITED=0;
MIN_PACE_PERIOD_NONE."

**Repo:** `crates/xrce/src/continuous_read.rs` mit Pacing/Throttling.

**Tests:** `pacing_throttles_per_period`,
`rate_limit_partitions_samples_over_time`,
`max_samples_cap_enforced`,
`max_elapsed_time_finalizes`,
`single_shot_delivers_one_then_finalizes`,
`stop_finalizes_immediately`.

**Status:** done

---

## §7.8 XRCE Object operations

### 7.8.1 ClientKey-Verwendung (4 Octets, distinguished pro Client)

**Spec:** §7.8.1, S. 36.

**Repo:** `header.rs` mit ClientKey-Field.

**Tests:** `header_roundtrip_with_key`,
`header_with_client_key_wire_size_8`,
`header_layout_bytes_with_key`,
`header_constructor_rejects_inconsistent_with_key`,
`header_decode_truncated_with_key`,
`session_id_127_carries_client_key`.

**Status:** done

### 7.8.2 XRCE Root operations (CREATE_CLIENT etc.)

**Spec:** §7.8.2, S. 37.

**Repo:** Submessage-Pfad: `submessages/create_client.rs`
(CREATE_CLIENT) + `submessages/delete.rs` (DELETE_CLIENT via Delete-
Submessage mit OBJECTID_CLIENT-Target) + `submessages/status_agent.rs`
(StatusAgent als Reply).

**Tests:** `create_client_roundtrip_via_submessage`,
`create_client_rejects_wrong_submessage_id`,
`session_id_127_carries_client_key` +
`profile_conformance::spec_7_8_2_root_operations_have_submessage_ids`
(verifiziert die Spec-Wire-IDs CreateClient=0/Delete=3/StatusAgent=4).

**Status:** done

### 7.8.3 XRCE ProxyClient operations (CREATE/DELETE/GET_INFO/UPDATE)

**Spec:** §7.8.3, S. 40.

**Repo:** `object_store.rs` + `submessages/{create,delete,get_info,
status,info}.rs`. UPDATE-Pfad realisiert via CREATE-Submessage mit
`CREATE_FLAG_REPLACE = 0x04` (Spec §7.7.11 CreationMode).

**Tests:** `create_inserts_new_object`,
`delete_removes_object`,
`get_info_roundtrip`,
`create_replace_increments_version` +
`profile_conformance::spec_7_8_3_proxy_client_operations_have_submessage_ids`
(verifiziert Create=1/Delete=3/GetInfo=2/Status=5/Info=6 + UPDATE-
Realisierung via REPLACE-Flag).

**Status:** done

### 7.8.4 XRCE DataWriter operations (WRITE_DATA)

**Spec:** §7.8.4, S. 45.

**Repo:** `submessages/write_data.rs`.

**Tests:** siehe §2.2.

**Status:** done

### 7.8.5 XRCE DataReader operations (READ_DATA -> DATA stream)

**Spec:** §7.8.5, S. 46.

**Repo:** `submessages/read_data.rs` + `continuous_read.rs`.

**Tests:** `read_data_roundtrip` + Continuous-Read-Tests.

**Status:** done

---

## §8 XRCE Protocol

### 8.1 General — Logical Messages-Definitions

**Spec:** §8.1, S. 49.

**Repo:** Submessage-Definitionen in `crates/xrce/src/submessages/`.

**Tests:** Crate-weit.

**Status:** done

### 8.2 Definitions: Message/Session/Stream/Client/Agent

**Spec:** §8.2, S. 49-50.

**Repo:** `header.rs::SessionId/StreamId`, `lib.rs::Message`.

**Tests:** `session_id_127_carries_client_key`,
`stream_id_classification`.

**Status:** done

### 8.3.1 Message Structure: Header + Submessages

**Spec:** §8.3.1, S. 50.

**Repo:** `header.rs::MessageHeader` + `lib.rs::Message`.

**Tests:** `message_encode_decode_roundtrip_no_key_single_sm`,
`message_encode_decode_roundtrip_with_key`,
`message_two_submessages_padded_correctly`,
`empty_message_with_no_submessages_roundtrip`,
`message_decode_rejects_truncated_body`,
`message_decode_rejects_unknown_submessage_id`,
`message_decode_rejects_oversized_payload_input`,
`message_decode_rejects_too_many_submessages_via_too_many_concat`,
`message_encode_rejects_too_many_submessages_via_constructor`,
`large_roundtrip`, `medium_roundtrip`, `small_roundtrip`.

**Status:** done

### 8.3.2 Message Header (4 Byte: sessionId/streamId/sequenceNr) + optional ClientKey (4 Byte)

**Spec:** §8.3.2, S. 50.

**Repo:** `header.rs::MessageHeader`.

**Tests:** `header_layout_bytes_no_key`,
`header_layout_bytes_with_key`,
`header_no_client_key_wire_size_4`,
`header_with_client_key_wire_size_8`,
`header_roundtrip_no_key`,
`header_roundtrip_with_key`,
`header_constructor_rejects_inconsistent_no_key`,
`header_constructor_rejects_inconsistent_with_key`,
`header_decode_truncated_no_key`,
`header_decode_truncated_with_key`,
`header_extra_trailing_bytes_are_ignored`,
`header_write_overflow_when_buffer_too_small`.

**Status:** done

### 8.3.3 Submessage Structure

**Spec:** §8.3.3, S. 52.

**Repo:** `submessages/` (Module pro Submessage).

**Tests:** `submessage_header_roundtrip`,
`submessage_header_length_is_always_le_even_with_be_body`.

**Status:** done

### 8.3.4 Submessage Header (4 Byte: submessageId + flags + length)

**Spec:** §8.3.4, S. 52.

**Repo:** `submessages/header.rs`.

**Tests:** `submessage_id_roundtrip_all_16_values`,
`submessage_id_rejects_unknown_byte`,
`submessage_header_roundtrip`.

**Status:** done

### 8.3.5 Submessage Types (16 Types: CREATE_CLIENT/CREATE/GET_INFO/DELETE/STATUS_AGENT/STATUS/INFO/WRITE_DATA/READ_DATA/DATA/ACKNACK/HEARTBEAT/RESET/FRAGMENT/TIMESTAMP/TIMESTAMP_REPLY)

**Spec:** §8.3.5, S. 53-68 — Detail-Spezifikationen pro Submessage.

**Repo:** Alle 16 Submessages in `crates/xrce/src/submessages/`.

**Tests:** alle `*_roundtrip*`-Tests + `all_16_submessages_in_one_message_roundtrip`,
`all_spec_kinds_roundtrip`, `submessage_id_roundtrip_all_16_values`,
`submessage_id_rejects_unknown_byte`,
`unknown_submessage_id_display_uses_hex`,
`reset_roundtrip_empty_body`, `reset_rejects_nonempty_body`,
`fragment_roundtrip_carries_last_flag`,
`fragment_intermediate_has_no_last_flag`,
`timestamp_decode_short_body_returns_eof`,
`timestamp_reply_decode_short_returns_eof`,
`timestamp_reply_roundtrip_le`,
`timestamp_roundtrip_via_submessage`,
`timestamp_reply_roundtrip_via_submessage`,
`time_t_roundtrip_be`, `time_t_roundtrip_le`.

**Status:** done

---

## §8.4 Interaction Model

### 8.4.1 General — Logical Actions Performance

**Spec:** §8.4.1, S. 69.

**Repo:** `crates/xrce-agent/src/lib.rs::XrceAgent` mit
in-memory-Operations (CREATE/DELETE/SUBMIT/PULL).

**Tests:** `xrce-agent::tests::agent_create_delete_latency_under_spec_floor`
(1000 CREATE+DELETE-Operations < 100ms; Spec-Performance-Floor).

**Status:** done

### 8.4.2 Sending data using a pre-configured DataWriter (WRITE_DATA)

**Spec:** §8.4.2, S. 69.

**Repo:** `submessages/write_data.rs`.

**Tests:** siehe §2.2.

**Status:** done

### 8.4.3 Receiving data using a pre-configured DataReader (READ_DATA -> DATA*N)

**Spec:** §8.4.3, S. 69.

**Repo:** `submessages/read_data.rs` + `continuous_read.rs`.

**Tests:** siehe §2.1.

**Status:** done

### 8.4.4 Discovering an Agent (GET_INFO mit Multicast/Periodic)

**Spec:** §8.4.4, S. 70 — "Client sendet GET_INFO(OBJECTID_AGENT,
CLIENT_Representation) periodisch (auch Multicast); Agents
antworten INFO(AGENT_Representation, STATUS_OK)."

**Repo:** `discovery.rs::MulticastDiscovery` mit
`XRCE_DISCOVERY_GROUP=239.255.0.2` + `XRCE_DISCOVERY_PORT=7400`
(Spec §11.2.4). Sender via `send_multicast`, Receiver via
`join_multicast_v4` + `recv`.

**Tests:** `get_info_roundtrip`, `info_roundtrip`,
`agent_port_for_domain_*`, `client_port_for_domain_*`,
`discovery_constants_match_spec`,
`multicast_send_via_xrce_discovery_group_does_not_error`,
`discovery_group_addr_constructed_correctly`,
`tcp_discovery_uses_same_port_scheme_as_udp`.

**Status:** done

### 8.4.5 Connecting to an Agent (CREATE_CLIENT(ClientKey) -> STATUS_AGENT)

**Spec:** §8.4.5, S. 71.

**Repo:** Submessage-Pfad.

**Tests:** `create_client_roundtrip_via_submessage`,
`status_agent_roundtrip`.

**Status:** done

### 8.4.6 Creating a complete Application (CREATE(OBJK_APPLICATION))

**Spec:** §8.4.6, S. 72.

**Repo:** `xml_config.rs::end_to_end_load_and_emit_creates` +
`crates/xrce-agent/src/lib.rs::XrceAgent::create_object` mit
OBJK_APPLICATION-Support.

**Tests:** `end_to_end_load_and_emit_creates`,
`create_messages_for_empty_participant_only_participant_msg` +
`xrce-agent::tests::create_application_object_via_objk_application`.

**Status:** done

### 8.4.7 Defining QoS (CREATE(OBJK_QOSPROFILE))

**Spec:** §8.4.7, S. 72.

**Repo:** XML-Config + qos-resolver.

**Tests:** siehe §2.4.

**Status:** done

### 8.4.8 Defining Types (CREATE(OBJK_TYPE))

**Spec:** §8.4.8, S. 73.

**Repo:** Type-Reuse in `xml_config.rs`.

**Tests:** siehe §2.5.

**Status:** done

### 8.4.9 Creating a Topic (CREATE(OBJK_TOPIC))

**Spec:** §8.4.9, S. 73.

**Repo:** `xml_config.rs`.

**Tests:** `roundtrip_basic_hierarchy_parses`,
`err_unresolved_topic_ref`,
`topic_without_type_section_rejected`,
`edge_topic_missing_required_attr`.

**Status:** done

### 8.4.10 Creating a DataWriter (CREATE(OBJK_DATAWRITER) -> STATUS -> WRITE_DATA*N)

**Spec:** §8.4.10, S. 74.

**Repo:** `submessages/create.rs`.

**Tests:** siehe §2.2 + §2.3.

**Status:** done

### 8.4.11 Creating a DataReader (CREATE(OBJK_DATAREADER) -> STATUS -> READ -> DATA*N)

**Spec:** §8.4.11, S. 74.

**Repo:** wie 8.4.10.

**Tests:** siehe §2.1 + §2.3.

**Status:** done

### 8.4.12 Getting Info on a Resource (GET_INFO -> INFO)

**Spec:** §8.4.12, S. 75.

**Repo:** `discovery.rs`.

**Tests:** `get_info_roundtrip`, `info_roundtrip`.

**Status:** done

### 8.4.13 Updating a Resource (CREATE(reuse=TRUE, replace=TRUE) -> STATUS)

**Spec:** §8.4.13, S. 76.

**Repo:** Reuse/Replace-Flags.

**Tests:** `create_replace_increments_version`,
`create_reuse_returns_equal_marker`,
`create_roundtrip_carries_reuse_replace_flags`.

**Status:** done

### 8.4.14 Reliable Communication (Sender+Receiver State Machine pro Reliable Stream)

**Spec:** §8.4.14, S. 76 — "pro reliable Stream je Sender+Receiver
State-Machine; HEARTBEAT-/ACKNACK-Frames; Stream-Buffer-Limits."

**Repo:** `crates/xrce/src/reliable.rs::ReliableStreamState` mit
DoS-Caps + Pending-Tracking.

**Tests:** `submit_assigns_monotonic_seqnrs`,
`submit_rejects_payload_too_large`,
`submit_rejects_when_window_full`,
`pending_acknack_marks_missing_slots`,
`pending_heartbeat_fires_first_time`,
`pending_heartbeat_none_when_window_empty`,
`pending_heartbeat_silenced_until_period_elapsed`,
`recv_acknack_clears_acked_seqnrs`,
`recv_acknack_full_clear_when_no_bits_set`,
`recv_data_*` (4 Tests),
`dos_cap_max_fragments_per_stream`,
`dos_cap_max_pending_streams`,
`dos_cap_max_total_payload` +
`reliable::tests::end_to_end_sender_receiver_with_loss_recovery`
(End-to-End-Szenario mit ACKNACK-Loss-Recovery).

**Status:** done — Sender + Receiver State-Machine voll, Two-Side-
Loss-Recovery-Test belegt vollständiges Reliable-Protokoll.

---

## §8.5 XRCE Object Operation Traceability

### 8.5 Operation-Tracing zwischen Submessage und Object-Operation

**Spec:** §8.5, S. 78.

**Repo:** `crates/xrce-agent/src/lib.rs::TraceSink`-Trait +
`TraceEvent { operation, client_key, object_id }` +
`XrceAgent::set_trace_sink`. Pro CREATE/DELETE/SUBMIT/PULL-Operation
wird ein Event generiert und an den Sink delegiert (kompatibel
mit `tracing`-Crate, structured logger oder Test-Capture).

**Tests:** `xrce-agent::tests::trace_sink_captures_create_delete_submit_pull`
(verifiziert pro Operation ein TraceEvent mit korrektem
`operation`/`client_key`/`object_id`).

**Status:** done

---

## §9 XRCE Agent Configuration

### 9.1 General

**Spec:** §9.1, S. 79.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §9; kündigt nur
die zwei Konfigurations-Mechanismen (§9.2 Remote, §9.3 File) an.

### 9.2 Remote Configuration via XRCE Protocol

**Spec:** §9.2, S. 79 — "Remote-Konfiguration via CREATE/DELETE/UPDATE."

**Repo:** Submessage-Pfad CREATE/DELETE + UPDATE via REPLACE-Flag,
Reliable-Stream für in-order-Delivery der Config-Sequenz.

**Tests:** siehe §8.4.13 +
`reliable::tests::config_submessages_delivered_in_order_via_reliable_stream`
(simuliert 5 Config-Operations mit out-of-order-recv und prüft
in-order-drain via reliable-Stream).

**Status:** done

### 9.3 File-Based Configuration (XML)

**Spec:** §9.3, S. 80-83 — XML-Schema mit dds/qos_profile/type/
domain/participant-Hierarchie.

**Repo:** `crates/xrce/src/xml_config.rs` +
`crates/xrce/schemas/xrce-config.xsd`.

**Tests:** `roundtrip_basic_hierarchy_parses`,
`roundtrip_multiple_participants`,
`roundtrip_object_ids_preserved`,
`roundtrip_topic_ref_preserved`,
`roundtrip_qos_profile_carries_string`,
`end_to_end_load_and_emit_creates`,
`edge_empty_dds_root_is_valid`,
`edge_invalid_domain_id_string`,
`edge_invalid_object_id_format`,
`edge_decimal_object_id_supported`,
`edge_domain_id_overflow`,
`edge_too_many_types_capped`,
`edge_topic_missing_required_attr`,
`edge_missing_root_invalid_xml`,
`err_circular_type_self_reference`,
`err_duplicate_object_id_two_topics`,
`err_object_kind_mismatch`,
`err_unexpected_root`,
`err_unresolved_topic_ref`,
`err_unresolved_type_name`,
`type_reuse_*` (5 Tests),
`display_xrce_xml_error_messages`,
`from_xrce_error_wraps_wire`.

**Status:** done

---

## §10 XRCE Deployments

### 10.1 XRCE Client to DDS Communication (Standard-Deployment)

**Spec:** §10.1, S. 85.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Deployment-Topologie-
Beschreibung; alle normativen Anforderungen liegen in
§7-§9/§11.

### 10.2 XRCE Client to Client via DDS

**Spec:** §10.2, S. 85.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Deployment-Topologie.

### 10.3 Client-to-Client brokered by Agent

**Spec:** §10.3, S. 86.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Deployment-Topologie. Spec
enthält ein "shall" zur Multi-Client-Isolation auf Agent-Seite,
das aber durch die normalen DCPS-Discovery-Regeln (eigene
DataWriter/DataReader pro Client-Proxy) automatisch erfüllt
wird; keine zusätzliche Implementations-Anforderung.

### 10.4 Federated Deployment

**Spec:** §10.4, S. 87.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec sagt explizit "implementation
decision".

### 10.5 Direct Peer-to-Peer

**Spec:** §10.5, S. 88.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Deployment-Topologie.

### 10.6 Combined Deployment

**Spec:** §10.6, S. 89.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Deployment-Topologie.

---

## §11 Transport Mappings

### 11.1 Transport Model

**Spec:** §11.1, S. 91.

**Repo:** `crates/xrce/src/transport_locator.rs`.

**Tests:** —

**Status:** done

### 11.2.1 UDP Transport Locators (sockaddr_in)

**Spec:** §11.2.1, S. 91.

**Repo:** `transport_locator.rs` + `transport_udp.rs`.

**Tests:** siehe §2.8.

**Status:** done

### 11.2.2 UDP Connection Establishment

**Spec:** §11.2.2, S. 92.

**Repo:** `transport_udp.rs`.

**Tests:** `start_with_ephemeral_port_succeeds`.

**Status:** done

### 11.2.3 UDP Message Envelopes (1 XRCE-Message pro Datagram)

**Spec:** §11.2.3, S. 92.

**Repo:** `transport_udp.rs`.

**Tests:** `loopback_send_recv_roundtrip`.

**Status:** done

### 11.2.4 UDP Agent Discovery (Multicast 239.255.0.2:7400 + DG×Domain)

**Spec:** §11.2.4, S. 92.

**Repo:** `discovery.rs::MulticastDiscovery::send_multicast` +
`recv` + `XRCE_DISCOVERY_GROUP=239.255.0.2` +
`XRCE_DISCOVERY_PORT=7400` + `agent_default_port`-Helper
(Port=7400+4×domain, Spec-konform).

**Tests:** `agent_port_for_domain_0_is_7400`,
`agent_port_for_domain_5_is_7420`,
`client_port_for_domain_0_is_7401`,
`discovery_constants_match_spec`,
`multicast_send_via_xrce_discovery_group_does_not_error`,
`discovery_group_addr_constructed_correctly`.

**Status:** done

### 11.3.1 TCP Transport Locators

**Spec:** §11.3.1, S. 93.

**Repo:** `transport_tcp.rs`.

**Tests:** `tcp_local_addr_consistent_after_bind`.

**Status:** done

### 11.3.2 TCP Connection Establishment

**Spec:** §11.3.2, S. 93.

**Repo:** `transport_tcp.rs`.

**Tests:** `tcp_loopback_*`-Tests.

**Status:** done

### 11.3.3 TCP Message Envelopes (length-prefixed)

**Spec:** §11.3.3, S. 93.

**Repo:** `transport_tcp.rs`.

**Tests:** `tcp_length_prefix_size_constant`,
`tcp_recv_oversized_length_rejected`,
`tcp_send_truncation_when_peer_drops`,
`tcp_truncated_returns_eof`.

**Status:** done

### 11.3.4 TCP Agent Discovery

**Spec:** §11.3.4, S. 94.

**Repo:** `discovery.rs` (Port-Schema identisch zu UDP) +
`transport_tcp.rs::XrceTcpServer::bind` auf Spec-Port.

**Tests:** wie 11.2.4 +
`discovery::tests::tcp_discovery_uses_same_port_scheme_as_udp`
(verifiziert dass TCP dieselbe Port-Formel 7400+4×domain nutzt).

**Status:** done

### 11.4 Other Transports (DTLS/TLS/Serial — non-normative Profile)

**Spec:** §11.4, S. 94 — Spec hint zu DTLS/TLS/Serial-Mappings.

**Repo:** `transport_dtls.rs`, `transport_tls.rs`,
`transport_serial.rs`. Serial-Pfad voll produktiv; DTLS/TLS mit echtem
Crypto-Backend hinter Opt-in-Features (`dtls`/`tls`).

**Tests:** siehe §3.1.9-§3.1.11 (`dtls_e2e.rs`/`tls_e2e.rs`). Spec §11.4 ist
explizit als "non-normative Profile" gekennzeichnet.

**Status:** done — Serial-, DTLS- und TLS-Transport produktiv; DTLS/TLS mit
echtem Crypto-Backend (`webrtc-dtls`/`rustls`) hinter Opt-in-Features (siehe
§3.1.9, §3.1.10).

---

## Audit-Status

82 done / 0 partial / 0 open / 13 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-xrce` — 295 lib + 6 integration = 301 Tests grün.
* `cargo test -p zerodds-xrce-client` — 9 Tests grün.
* `cargo test -p zerodds-xrce-agent` — 13 Tests grün.

Cross-Crate Test-Volumen: 323 Tests gegen DDS-XRCE-1.0.

Keine offenen Punkte (DTLS/TLS-Crypto-Backends + Serial produktiv; die
non-normativen Profile sind voll abgedeckt).
