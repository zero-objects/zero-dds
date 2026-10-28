# DDSI-RTPS 2.5 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/ddsi-rtps-2.5.pdf` (206 Seiten, OMG formal/2022-04-01) —
[OMG DDSI-RTPS 2.5](https://www.omg.org/spec/DDSI-RTPS/2.5/PDF)

Item-für-Item-Audit gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 6). Jede Zeile
einer normativen Tabelle ist ein eigenes Item. `done` steht nur, wo Code und
Test die Anforderung abdecken; ein Test-Zitat nennt die Testfunktion als
`datei::modul::name`.

**Kontext:** Der DDSI-RTPS-2.5-Stack ist verteilt über:

- `crates/rtps/` — Header, Submessages, HeaderExtension, ParameterList,
  Inline-QoS, Receiver-Zustand, Writer- und Reader-Zustandsmaschinen mit
  ihren Proxies, HistoryCache, Fragmentierung, Discovery-Datentypen.
- `crates/discovery/` — SPDP, SEDP, TypeLookup, die sicheren Builtin-Endpoints.
- `crates/transport/`, `crates/transport-udp/` — Transport-Schnittstelle und
  der UDP/IP-PSM (§9.5).
- `crates/dcps/` — die DDS-Seite (§8.2.10), Writer Liveliness Protocol,
  KeyHash, Encapsulation-Header (§10).
- `crates/qos/` — die QoS-Policies und ihr Wire-Format.

**Offene Punkte:** siehe [`ddsi-rtps-2.5.open.md`](ddsi-rtps-2.5.open.md).

---

## §1 Scope

### §1 Scope

**Spec:** §1, S. 7 — "This specification defines an interoperability wire
protocol for DDS. Its purpose and scope are to ensure that applications based
on different vendors' implementations of DDS can interoperate."

**Repo:** `crates/rtps/src/lib.rs`

**Tests:** —

**Status:** `n/a (informative)` — Zweckbestimmung der Spec; die Anforderungen
stehen in §7.5–§10 und sind dort einzeln geführt.

---

## §2 Conformance

### §2 Conformance

**Spec:** §2, S. 7 — "Implementations of this specification must comply with
the conformance statements listed in 8.4.2 of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis ohne eigene Anforderung; die
Konformitätsaussagen von §8.4.2 sind unter §8.4.2.1–§8.4.2.3 einzeln geführt.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, S. 7 — Liste der normativen Dokumente [1] DDS 1.4, [2] IDL 4.2,
[3] DDS-XTypes 1.2, [4] NTP Version 3 (RFC 1305), [5] MD5 (RFC 1321),
[6] SCTP-CRC32c (RFC 4960 Anhang B), [7] AUTOSAR-CRC-Routinen R20-11.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Querverweisliste. Was aus den Referenzen
folgt, ist an der verwendenden Stelle geführt: Time_t im NTP-Format unter
§9.3.2.2, MD5 unter §9.4.2.15 und §9.6.4.8, CRC32c und CRC64 unter §9.4.2.15.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, S. 7 — "For the purposes of this specification, the terms and
definitions given in the normative references apply."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsverweis ohne Anforderung.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, S. 7 — Abkürzungen CDR, DDS, EDP, GUID, PDP, PIM, PSM, RTPS,
SEDP.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abkürzungsliste.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 9 — "This specification does not change any adopted OMG
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — redaktionelle Angabe.

### §6.2 How to Read this Specification

**Spec:** §6.2, S. 9 — Lesehinweise zu PIM, PSM und den Modulen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Lesehinweis.

### §6.3 Acknowledgments

**Spec:** §6.3, S. 9–11 — Liste der beitragenden Firmen und Personen je
Revision.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

### §6.4 Statement of Maturity

**Spec:** §6.4, S. 11 — Reifegrad und bekannte Implementierungen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrundangabe.

---

## §7 Overview

### §7.1 Introduction

**Spec:** §7.1, S. 13 — Motivation eines DDS-Wire-Protokolls.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einführung ohne Anforderung.

### §7.2 Requirements for a DDS Wire-protocol

**Spec:** §7.2, S. 13–14 — Anforderungen an ein DDS-Wire-Protokoll als
Entwurfsziele (kein zentraler Namensdienst, keine Single Points of Failure,
Erweiterbarkeit).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Entwurfsziele; die daraus folgenden
Mechanismen (SPDP/SEDP ohne Server, Versionierung) sind unter §8.5 und §8.6
geführt.

### §7.3 The RTPS Wire-protocol

**Spec:** §7.3, S. 14 — Herkunft und Eigenschaften von RTPS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung.

### §7.4 The RTPS Platform Independent Model (PIM)

**Spec:** §7.4–§7.4.4, S. 14–17 — Gliederung des PIM in die Module
Structure, Messages, Behavior und Discovery.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Module sind unter §8.2–§8.5
geführt.

### §7.5 UDP/IP-PSM ist Pflicht

**Spec:** §7.5, S. 17 — "Multiple PSMs may be supported, but all
implementations of DDS must at least implement the PSM on top of UDP/IP,
which is presented in Clause 9."

**Repo:** `crates/transport-udp/src/udp_transport.rs` (`UdpTransport`);
`crates/dcps/src/runtime.rs` (SPDP/SEDP und Nutzdaten über UDP).

**Tests:** `crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`,
`crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_multiple_in_order`,
`crates/transport-udp/src/udp_transport.rs::tests::send_recv_v6_loopback`

**Status:** `done`

### §7.6 Transport: Unicast-Adresse in 16 Oktetten

**Spec:** §7.6, S. 18 — "The transport has a generalized notion of a unicast
address (shall fit within 16 bytes)."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::address: [u8; 16]`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v6_constructor_keeps_full_address`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`

**Status:** `done`

### §7.6 Transport: Port in 4 Oktetten

**Spec:** §7.6, S. 18 — "The transport has a generalized notion of a port
(shall fit within 4 bytes)".

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::port: u32`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_roundtrip_le`

**Status:** `done`

### §7.6 Transport: Datagramm an Adresse/Port senden

**Spec:** §7.6, S. 18 — "The transport can send a datagram (uninterpreted
sequence of octets) to a specific address/port."

**Repo:** `crates/transport/src/lib.rs` (`Transport::send`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransport::send`)

**Tests:** `crates/transport-udp/src/udp_transport.rs::tests::loopback_send_and_recv_delivers_datagram`,
`crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`

**Status:** `done`

### §7.6 Transport: Datagramm an Adresse/Port empfangen

**Spec:** §7.6, S. 18 — "The transport can receive a datagram at a specific
address/port."

**Repo:** `crates/transport/src/lib.rs` (`Transport::recv`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransport::recv`,
`UdpTransport::bind_v4`)

**Tests:** `crates/transport-udp/src/udp_transport.rs::tests::bind_v4_returns_local_locator_with_assigned_port`,
`crates/transport-udp/src/udp_transport.rs::tests::loopback_multiple_datagrams_in_order`

**Status:** `done`

### §7.6 Transport verwirft unvollständige oder verfälschte Nachrichten

**Spec:** §7.6, S. 18 — "The transport will drop messages if incomplete or
corrupted during transfer (i.e., RTPS assumes messages are complete and not
corrupted)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Annahme über den Transport, keine
Anforderung an die RTPS-Implementierung. Für UDP prüft die Prüfsumme des
UDP-Kopfs im Betriebssystem; die optionale Nachrichtenprüfsumme der
HeaderExtension ist unter §8.3.3.2 und §9.4.2.15 geführt.

---

## §8.1 Introduction

### §8.1 PIM als virtuelle Maschine

**Spec:** §8.1, S. 19 — "This description is not intended to constrain the
internal implementation in any way. The only criteria for a compliant
implementation is that the externally-observable behavior satisfies the
requirements for interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — legt den Maßstab für die PIM-Items fest:
geprüft wird das von außen beobachtbare Verhalten, nicht die Klassenstruktur.

---

## §8.2 Structure Module

### §8.2.1 Overview

**Spec:** §8.2.1, S. 19 — jede RTPS-Entity entspricht einer DDS-Entity; der
HistoryCache ist die Schnittstelle zwischen beiden.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Beziehung ist unter §8.2.10
je Übergang geführt.

### §8.2.1.1 Table 8.1 Klassen der virtuellen Maschine

**Spec:** §8.2.1.1, S. 20–21, Table 8.1 — Zusammenfassung der Klassen
Entity, Endpoint, Participant, Writer, Reader, HistoryCache, CacheChange,
Data.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zusammenfassung; jede Klasse ist mit ihren
Attributen unter §8.2.2–§8.2.9 geführt.

### §8.2.1.2 Table 8.2 GUID_t

**Spec:** §8.2.1.2, S. 21, Table 8.2 — "GUID_t … Must be possible to
represent using 16 octets. The following values are reserved by the
protocol: GUID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`, `Guid::WIRE_SIZE`,
`Guid::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/rtps/src/wire_types.rs::tests::guid_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done` — `Guid::UNKNOWN` ist aus den beiden geprüften
UNKNOWN-Teilen zusammengesetzt.

### §8.2.1.2 Table 8.2 GuidPrefix_t

**Spec:** §8.2.1.2, S. 21, Table 8.2 — "GuidPrefix_t … Must be possible to
represent using 12 octets. The following values are reserved by the
protocol: GUIDPREFIX_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix`, `GuidPrefix::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`

**Status:** `done`

### §8.2.1.2 Table 8.2 EntityId_t

**Spec:** §8.2.1.2, S. 21, Table 8.2 — "EntityId_t … Must be possible to
represent using 4 octets. The following values are reserved by the
protocol: ENTITYID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId`, `EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §8.2.1.2 Table 8.2 SequenceNumber_t

**Spec:** §8.2.1.2, S. 21, Table 8.2 — "SequenceNumber_t … Must be possible
to represent using 64 bits. The following values are reserved by the
protocol: SEQUENCENUMBER_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber(i64)`,
`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §8.2.1.2 Table 8.2 Locator_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "It must be possible to represent
the discriminator and port number using 4 octets each, the address using 16
octets." Reserviert: LOCATOR_INVALID, LOCATOR_KIND_INVALID,
LOCATOR_KIND_RESERVED, LOCATOR_KIND_UDPv4, LOCATOR_KIND_UDPv6,
LOCATOR_ADDRESS_INVALID, LOCATOR_PORT_INVALID.

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator`, `LocatorKind::{Invalid,
Reserved, UdpV4, UdpV6}`, `Locator::INVALID`, `Locator::ADDRESS_INVALID`,
`Locator::PORT_INVALID`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_invalid_constant_matches_spec`,
`crates/rtps/src/wire_types.rs::tests::locator_reserved_constant_kind_is_zero`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v4_any_kind_is_one`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v6_any_kind_is_two`,
`crates/rtps/src/wire_types.rs::tests::locator_roundtrip_le`

**Status:** `done` — die Werte der Konstanten sind unter §9.3.2.4 geführt.

### §8.2.1.2 Table 8.2 TopicKind_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "Enumeration used to distinguish
whether a Topic has defined some fields within to be used as the 'key' …
NO_KEY, WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs` (`TopicKind::{NoKey, WithKey}`,
`EntityKind::topic_kind`, `TopicKind::writer_entity_kind`,
`TopicKind::reader_entity_kind`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::topic_kind_entity_kind_pairs_round_trip`

**Status:** `done`

### §8.2.1.2 Table 8.2 ChangeKind_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "It can take the values: ALIVE,
ALIVE_FILTERED, NOT_ALIVE_DISPOSED, NOT_ALIVE_UNREGISTERED".

**Repo:** `crates/rtps/src/history_cache.rs` (`ChangeKind::{Alive,
AliveFiltered, NotAliveDisposed, NotAliveUnregistered,
NotAliveDisposedUnregistered}`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::change_kind_distinct_variants`,
`crates/rtps/src/history_cache.rs::tests::change_kind_alive_filtered_is_alive_but_not_relevant`,
`crates/rtps/src/history_cache.rs::tests::change_kind_not_alive_kinds_are_not_alive`

**Status:** `done` — `NotAliveDisposedUnregistered` ist eine zusätzliche
Kombination für PID_STATUS_INFO mit beiden Bits.

### §8.2.1.2 Table 8.2 ChangeCount_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "Type used to hold a counter
representing the number of HistoryCache changes that belong to a certain
category."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`, `Option<u64>`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — der Zähler ist 64 Bit breit (high, low; acht Oktette, §9.3.2.11).

### §8.2.1.2 Table 8.2 ReliabilityKind_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "Enumeration used to indicate the
level of the reliability used for communications. It can take the values:
BEST_EFFORT, RELIABLE."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind::{BestEffort,
Reliable}`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::try_from_u32_is_strict`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`

**Status:** `done`

### §8.2.1.2 Table 8.2 InstanceHandle_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "Type used to represent the identity
of a data-object whose changes in value are communicated by the RTPS
protocol."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`);
`crates/dcps/src/instance_handle.rs` (`InstanceHandle`)

**Tests:** `crates/dcps/src/instance_handle.rs::tests::from_guid_distinguishes_keys`,
`crates/rtps/src/history_cache.rs::tests::keep_last_per_instance_keeps_the_newest_of_each_instance`

**Status:** `done` — auf RTPS-Ebene trägt der KeyHash (§9.6.4.8) die
Instanz-Identität.

### §8.2.1.2 Table 8.2 ProtocolVersion_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — reserviert PROTOCOLVERSION,
PROTOCOLVERSION_1_0, _1_1, _2_0, _2_1, _2_2, _2_4; "PROTOCOLVERSION is an
alias for the most recent version, in this case PROTOCOLVERSION_2_4".

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion::{V1_0, V1_1,
V2_0, V2_1, V2_2, V2_3, V2_4, V2_5, CURRENT}`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_aliases_match_spec`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_all_aliases_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_default_is_2_5`

**Status:** `done` — `CURRENT` ist 2.5 nach §8.3.3.1 („protocol version
2.5“); die Angabe „in this case PROTOCOLVERSION_2_4“ in Table 8.2
widerspricht §8.3.3.1.

### §8.2.1.2 Table 8.2 VendorId_t

**Spec:** §8.2.1.2, S. 22, Table 8.2 — "The possible values for the vendorId
are assigned by the OMG. The following values are reserved by the protocol:
VENDORID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId`, `VendorId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_unknown_is_two_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`

**Status:** `done` — der von ZeroDDS gesendete Wert ist unter §8.3.3.1
(vendorId) geführt.

### §8.2.1.3 Configuration attributes of the RTPS Entities

**Spec:** §8.2.1.3, S. 23, Figure 8.2 — Konfigurationsattribute der
Entities; Writer- und Reader-Attribute folgen in §8.4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Attribute sind in
Table 8.8, Table 8.9 und §8.4.7/§8.4.10 je einzeln geführt.

### §8.2.2 HistoryCache: Rolle auf Writer- und Reader-Seite

**Spec:** §8.2.2, S. 23 — Writer-Seite: "the partial history of changes …
needed to service existing and future matched RTPS Reader endpoints";
Reader-Seite: "the partial superposition of changes to data-objects made by
all the matched RTPS Writer endpoints".

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter`, Feld `cache`);
`crates/rtps/src/reliable_reader.rs` (`ReliableReader`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`,
`crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`

**Status:** `done`

### §8.2.2 Table 8.3 HistoryCache::changes

**Spec:** §8.2.2, S. 24, Table 8.3 — "changes: CacheChange[*] — The list of
CacheChanges contained in the HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache`, `get`,
`iter_range`, `len`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::insert_and_get`,
`crates/rtps/src/history_cache.rs::tests::iter_range_is_ordered`

**Status:** `done`

### §8.2.2.1 HistoryCache new

**Spec:** §8.2.2.1, S. 25 — "This operation creates a new RTPS HistoryCache.
The newly-created history cache is initialized with an empty list of
changes."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::new`,
`HistoryCache::new_with_kind`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::new_cache_is_empty`

**Status:** `done`

### §8.2.2.2 HistoryCache add_change

**Spec:** §8.2.2.2, S. 25 — "This operation inserts the CacheChange a_change
into the HistoryCache. This operation will only fail if there are not enough
resources to add the change to the HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::insert`,
`HistoryCache::insert_returning_evicted`, `CacheError`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::insert_and_get`,
`crates/rtps/src/history_cache.rs::tests::insert_at_capacity_is_err`,
`crates/rtps/src/history_cache.rs::tests::insert_duplicate_is_err`

**Status:** `done` — `insert` weist zusätzlich eine bereits belegte
Sequenznummer ab; nach außen nicht sichtbar, da Writer jede Nummer nur einmal
vergeben.

### §8.2.2.2 RESOURCE_LIMITS und Fehlerweitergabe an den DDS-Nutzer

**Spec:** §8.2.2.2, S. 25 — "It is the responsibility of the DDS service
implementation to configure the HistoryCache in a manner consistent with the
DDS Entity RESOURCE_LIMITS QoS and to propagate any errors to the DDS-user in
the manner specified by the DDS specification."

**Repo:** `crates/dcps/src/runtime.rs` (`await_writer_room`);
`crates/rtps/src/history_cache.rs` (`HistoryKind`,
`HistoryCache::set_per_instance_depth`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::a_writer_refuses_an_instance_beyond_its_own_max_instances`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_rejects_when_reliable`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** `done`

### §8.2.2.3 HistoryCache remove_change

**Spec:** §8.2.2.3, S. 25 — "This operation indicates that a
previously-added CacheChange should be removed from the HistoryCache … The
determination of which changes should be removed from the cache is made based
on the QoS associated with the related DDS entity and on the acknowledgment
status of the CacheChange."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::remove_up_to`,
`remove_up_to_with`, `remove_highest`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::remove_samples_up_to`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::remove_up_to_inclusive`,
`crates/rtps/src/history_cache.rs::tests::remove_up_to_with_no_matches_is_noop`,
`crates/rtps/src/reliable_writer.rs::tests::the_unacked_window_fills_and_acknowledgements_free_it`

**Status:** `done`

### §8.2.2.4 HistoryCache get_seq_num_min

**Spec:** §8.2.2.4, S. 25 — "This operation retrieves the smallest value of
the CacheChange::sequenceNumber attribute among the CacheChange stored in the
HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::min_sn`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::min_max_sn_reflect_content`

**Status:** `done`

### §8.2.2.5 HistoryCache get_seq_num_max

**Spec:** §8.2.2.5, S. 25 — "This operation retrieves the largest value of
the CacheChange::sequenceNumber attribute among the CacheChange stored in the
HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::max_sn`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::min_max_sn_reflect_content`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::kind

**Spec:** §8.2.3, S. 26, Table 8.5 — "kind: ChangeKind_t — Identifies the
kind of change."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::kind`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::kind`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::cache_change_alive_constructor`,
`crates/rtps/src/reliable_reader.rs::tests::dispose_data_yields_not_alive_disposed`,
`crates/rtps/src/reliable_reader.rs::tests::unregister_data_yields_not_alive_unregistered`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::writerGuid

**Spec:** §8.2.3, S. 26, Table 8.5 — "writerGuid: GUID_t — The GUID_t that
identifies the RTPS Writer that made the change".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DeliveredSample::writer_guid`);
Writer-Seite implizit, je Writer ein eigener `HistoryCache`.

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::instanceHandle

**Spec:** §8.2.3, S. 26, Table 8.5 — "instanceHandle: InstanceHandle_t —
Identifies the instance of the data-object to which the change applies."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::key_hash`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::the_instance_index_follows_inserts_evictions_and_removals`,
`crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::sequenceNumber

**Spec:** §8.2.3, S. 26, Table 8.5 — "sequenceNumber: SequenceNumber_t —
Sequence number assigned by the RTPS Writer to uniquely identify the change."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::sequence_number`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/writer.rs::tests::writer_increments_sequence_number_per_write`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::data_value

**Spec:** §8.2.3, S. 26, Table 8.5 — "data_value: Data — The data value
associated with the change. Depending on the kind of CacheChange, there may
be no associated data."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::payload`,
`CacheChange::lifecycle`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::cache_change_alive_constructor`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_without_a_key_clears_k_and_sends_no_payload`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::inlineQos

**Spec:** §8.2.3, S. 26, Table 8.5 — "inlineQos: ParameterList — Contains QoS
that may affect the interpretation of the CacheChange::data_value."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::inline_qos`,
`CacheChange::with_inline_qos`); Reader-Seite:
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample` mit den
ausgewerteten Parametern `key_hash`, `coherent_set_sn`,
`related_sample_identity`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`,
`crates/rtps/src/reliable_reader.rs::tests::delivered_sample_carries_the_coherent_set_marker`

**Status:** `done`

### §8.2.4 Table 8.6 Entity::guid

**Spec:** §8.2.4, S. 26, Table 8.6 — "guid: GUID_t — Globally and uniquely
identifies the RTPS Entity within the DDS domain. Maps to the value of the
DDS BuiltinTopicKey_t".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`);
`crates/dcps/src/builtin_topics.rs` (Schlüssel der Builtin-Topic-Daten)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/dcps/src/instance_handle.rs::tests::from_guid_distinguishes_keys`

**Status:** `done`

### §8.2.4.1 Table 8.7 GUID = <prefix, entityId>

**Spec:** §8.2.4.1, S. 26–27, Table 8.7 — "The GUID is built as a tuple
<prefix, entityId> combining a GuidPrefix_t prefix and an EntityId_t
entityId."

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid::new`, `Guid::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`

**Status:** `done`

### §8.2.4.2 Participant-GUID <prefix, ENTITYID_PARTICIPANT>

**Spec:** §8.2.4.2, S. 27 — "Every Participant has GUID <prefix,
ENTITYID_PARTICIPANT>".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::PARTICIPANT`);
`crates/dcps/src/participant.rs` (`random_guid_prefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`,
`crates/dcps/src/participant.rs::tests::random_guid_prefixes_share_host_id_within_process`

**Status:** `done`

### §8.2.4.2 Prefix eindeutig in der Domain

**Spec:** §8.2.4.2, S. 27 — "The implementation is free to choose the
prefix, as long as every Participant in the Domain has a unique GUID."

**Repo:** `crates/dcps/src/participant.rs` (`next_process_unique`, `namespaced_pid`, `process_unique`); `crates/dcps/src/runtime.rs` (`DcpsRuntime::prefix_collisions`)

**Tests:** `crates/dcps/src/participant.rs::tests::the_prefixes_of_one_process_are_pairwise_different`,
`crates/dcps/src/participant.rs::tests::two_pid_namespaces_with_the_same_pid_give_different_prefixes`,
`crates/dcps/src/runtime.rs::tests::a_beacon_with_the_own_prefix_from_another_participant_is_counted`

**Status:** `done` — die Prefixe der Participants eines Prozesses sind paarweise verschieden (Zähler des Prozesses, 65536 je Prozess, auch für die C-API und die Brücken); über Prozesse und Hosts hinweg verhindert die Konstruktion die Kollision (Item „Eindeutigkeit des Prefix über Hosts und Prozesse“).

### §8.2.4.2 Eindeutigkeit des Prefix über Hosts und Prozesse

**Spec:** §8.2.4.2, S. 27 — "every Participant in the Domain has a unique GUID".

**Repo:** `crates/dcps/src/participant.rs` (`namespaced_pid`, `pid_namespace_inode`, `next_process_unique`, `process_unique`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::prefix_collisions`, `same_process`)

**Tests:** `crates/dcps/src/participant.rs::tests::two_pid_namespaces_with_the_same_pid_give_different_prefixes`,
`crates/dcps/src/participant.rs::tests::the_prefixes_of_one_process_share_ten_octets_and_differ_in_the_tail`,
`crates/dcps/src/participant.rs::tests::random_guid_prefixes_share_host_id_within_process`,
`crates/dcps/src/runtime.rs::tests::a_beacon_with_the_own_prefix_from_another_participant_is_counted`

**Status:** `done` — der Prefix besteht aus der VendorId, dem Host-Kennwert (4 Oktette), der Prozess-ID und einem Prozessanteil (Startwert aus der Zufallsquelle des Betriebssystems plus Zähler des Prozesses). Die Prozess-ID trägt die Marke des PID-Namensraums (`namespaced_pid`: XOR mit einer Maske aus dem Inode von `/proc/self/ns/pid`, im Ausgangs-Namensraum null, innerhalb eines Namensraums umkehrbar eindeutig). Container mit dem Netz des Hosts (gleicher Host-Kennwert) und eigenem PID-Namensraum, deren erster Prozess die Nummer 1 hat, tragen dadurch verschiedene Prefixe. `same_process` (erste zehn Oktette) und `same_host` (Host-Kennwert) lesen unverändert. Verbleibende Kollision: zwei verschiedene Hostnamen mit gleichem 32-Bit-Hash, gleicher markierter Prozess-ID und gleichem Prozessanteil; `DcpsRuntime::prefix_collisions` zählt einen SPDP-Beacon mit dem eigenen Prefix und fremden Locatoren.

### §8.2.4.3 GUID einer Endpoint-Gruppe

**Spec:** §8.2.4.3, S. 27 — "The endpoint Groups contained by a Participant
with GUID <participantPrefix, ENTITYID_PARTICIPANT> have the GUID
<participantPrefix, entityId>. The entityId is the unique identification of
the Group relative to the Participant."

**Repo:** `crates/dcps/src/publisher.rs` (`PublisherInner::entity_id`),
`crates/dcps/src/subscriber.rs` (`SubscriberInner::entity_id`),
`crates/dcps/src/runtime.rs` (`build_publication_data`,
`build_subscription_data`: `group_guid`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`,
`crates/rtps/src/wire_types.rs::tests::every_kind_table_9_1_names_decodes_to_a_named_kind`

**Status:** `done`

### §8.2.4.4 GUID der Endpoints eines Participants

**Spec:** §8.2.4.4, S. 27 — "The Endpoints contained by a Participant with
GUID <participantPrefix, ENTITYID_PARTICIPANT> have the GUID
<participantPrefix, entityId>. The entityId is the unique identification of
the Endpoint relative to the Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`next_entity_key`, `entity_key_in_use`, `held_entity_keys`, `release_entity_key`);
`crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs` (`Drop` der Inner-Typen)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_the_key_of_a_live_writer`,
`crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_reserved_and_group_keys_until_they_are_given_back`

**Status:** `done` — der 3-Oktett-Schlüssel kommt aus einem Zähler; nach dem Umlauf der 24 Bit wird ein Schlüssel übersprungen, den ein registrierter Writer, Reader, ein angekündigtes Topic, eine reservierte, noch nicht registrierte Id oder eine Gruppen-Id (Publisher, Subscriber) hält; ein zurückgegebener Schlüssel ist wieder frei.

### §8.2.5 Table 8.8 Participant::defaultUnicastLocatorList

**Spec:** §8.2.5, S. 28, Table 8.8 — "Default list of unicast locators …
These are the unicast locators that will be used in case the Endpoint does
not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::default_unicast_locators`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done` — der Rückgriff des Endpoints auf die Default-Locatoren
ist unter §8.5.5.1 geführt.

### §8.2.5 Table 8.8 Participant::defaultMulticastLocatorList

**Spec:** §8.2.5, S. 28, Table 8.8 — "Default list of multicast locators …
used in case the Endpoint does not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::default_multicast_locators`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.5 Table 8.8 Participant::protocolVersion

**Spec:** §8.2.5, S. 28, Table 8.8 — "protocolVersion: ProtocolVersion_t —
Identifies the version of the RTPS protocol that the Participant uses to
communicate."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::protocol_version`);
`crates/rtps/src/header.rs` (`RtpsHeader::new`)

**Tests:** `crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.5 Table 8.8 Participant::vendorId

**Spec:** §8.2.5, S. 28, Table 8.8 — "vendorId: VendorId_t — Identifies the
vendor of the RTPS middleware that contains the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::vendor_id`); `crates/rtps/src/header.rs`

**Tests:** `crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.6 RTPS Group: Publisher enthält Writer, Subscriber enthält Reader

**Spec:** §8.2.6, S. 28–29 — "The RTPS Publisher contains RTPS Writer
endpoints. The RTPS Publisher maps to a DDS Publisher. The RTPS Subscriber
contains RTPS Reader endpoints. The RTPS Subscriber maps to a DDS
Subscriber."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_writer_of_another_publisher_is_left_as_it_was`,
`crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`,
`crates/dcps/tests/entity_deletion.rs::publisher_delete_contained_entities_deletes_every_writer`

**Status:** `done` — die GUID der Gruppe ist unter §8.2.4.3 geführt.

### §8.2.7 Table 8.9 Endpoint::unicastLocatorList

**Spec:** §8.2.7, S. 29, Table 8.9 — "List of unicast locators … that can be
used to send messages to the Endpoint. The list may be empty."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::unicast_locators`);
`crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::unicast_locators`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_locators_roundtrip_le`

**Status:** `done` — die Reader-Seite nutzt dieselben Hilfsfunktionen
(`encode_locator_params`, `collect_locator_params`); ihr Test steht unter
§8.5.4.4.

### §8.2.7 Table 8.9 Endpoint::multicastLocatorList

**Spec:** §8.2.7, S. 29, Table 8.9 — "List of multicast locators … that can
be used to send messages to the Endpoint. The list may be empty."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::multicast_locators`);
`crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::multicast_locators`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_locators_roundtrip_le`

**Status:** `done` — die Reader-Seite nutzt dieselben Hilfsfunktionen; ihr
Test steht unter §8.5.4.4.

### §8.2.7 Table 8.9 Endpoint::reliabilityLevel

**Spec:** §8.2.7, S. 29, Table 8.9 — "reliabilityLevel: ReliabilityKind_t —
The levels of reliability supported by the Endpoint. Maps to the RELIABILITY
QoS 'kind.'"

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::reliability`); `crates/rtps/src/writer.rs`
(`BestEffortWriter`); `crates/rtps/src/reliable_writer.rs` (`ReliableWriter`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::roundtrip_le`,
`crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`

**Status:** `done`

### §8.2.7 Table 8.9 Endpoint::topicKind

**Spec:** §8.2.7, S. 29, Table 8.9 — "topicKind: TopicKind_t — Used to
indicate whether the Endpoint supports instance lifecycle management
operations (see 8.7.4)."

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::topic_kind`);
`crates/dcps/src/publisher.rs` (Wahl der `entityKind` aus `DdsType::HAS_KEY`)

**Tests:** `crates/dcps/src/publisher.rs::tests::live_datawriter_entity_kind_is_nokey_for_keyless_type`,
`crates/rtps/src/wire_types.rs::tests::topic_kind_of_the_predefined_entity_ids`

**Status:** `done`

### §8.2.7 Table 8.9 Endpoint::endpointGroup

**Spec:** §8.2.7, S. 29, Table 8.9 — "endpointGroup: EntityId_t — Used to
identify the RTPS Group (Publisher or Subscriber) to which the Endpoint
belongs."

**Repo:** `crates/dcps/src/runtime.rs` (`UserWriterConfig::group_entity_id`,
`UserReaderConfig::group_entity_id`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §8.2.8 RTPS Writer gehört zu einem Publisher

**Spec:** §8.2.8, S. 29–30 — "Its role is to transfer all CacheChange
changes in its HistoryCache to the HistoryCache of the matching remote RTPS
Readers. An RTPS Writer belongs to an RTPS Group of type Publisher."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`,
`crates/rtps/tests/reliable_e2e.rs::e2e_zero_loss_delivers_all_in_order`

**Status:** `done`

### §8.2.9 RTPS Reader gehört zu einem Subscriber

**Spec:** §8.2.9, S. 30 — "RTPS Reader specializes RTPS Endpoint and
represents the actor that receives CacheChange messages from the matched
RTPS Writer endpoints. An RTPS Reader belongs to an RTPS Group of type
Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`);
`crates/rtps/src/reliable_reader.rs` (`ReliableReader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`,
`crates/rtps/tests/reliable_e2e.rs::e2e_10_percent_loss_delivers_all_in_order`

**Status:** `done`

### §8.2.10 Relation to DDS Entities

**Spec:** §8.2.10, S. 30 — "How exactly a DDS Entity interacts with the
HistoryCache however, is implementation specific and not formally modeled by
the RTPS protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — erläutert die Übergänge; diese sind
unter §8.2.10.1 und §8.2.10.2 einzeln geführt.

### §8.2.10.1 DataWriter: alle Changes für jeden Reader relevant

**Spec:** §8.2.10.1, S. 30–31 — "By default, all changes in the HistoryCache
are considered relevant for each matching remote RTPS Reader. … The Reader
itself will always filter."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`,
Auslieferung an alle `ReaderProxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done` — der Reader-seitige Filter ist unter §8.7.3 geführt.

### §8.2.10.1 Table 8.11 DataWriter T1 (new DDS DataWriter)

**Spec:** §8.2.10.1, S. 32, Table 8.11 T1 — "the_rtps_writer = new
RTPS::Writer; the_dds_writer.related_rtps_writer := the_rtps_writer;"

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/dcps/src/runtime.rs` (`register_user_writer_as`)

**Tests:** `crates/dcps/src/publisher.rs::tests::live_datawriter_entity_kind_is_nokey_for_keyless_type`,
`crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T2 (write)

**Spec:** §8.2.10.1, S. 32, Table 8.11 T2 — "a_change :=
the_rtps_writer.new_change(ALIVE, data, inlineQos, handle);
the_rtps_writer.writer_cache.add_change(a_change);" Nachbedingung
"the_rtps_writer.writer_cache.get_seq_num_max() == a_change.sequenceNumber".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T3 (dispose, WITH_KEY)

**Spec:** §8.2.10.1, S. 32 — "if (the_rtps_writer.topicKind == WITH_KEY) {
a_change := the_rtps_writer.new_change(NOT_ALIVE_DISPOSED, <nil>, inlineQos,
handle); the_rtps_writer.writer_cache.add_change(a_change); }"

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose_w_timestamp`);
`crates/dcps/src/runtime.rs` (`write_user_lifecycle`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write_lifecycle`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_with_a_key_sets_k_and_carries_it`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T4 (unregister, WITH_KEY)

**Spec:** §8.2.10.1, S. 33 — "if (the_rtps_writer.topicKind == WITH_KEY) {
a_change := the_rtps_writer.new_change(NOT_ALIVE_UNREGISTERED, <nil>,
inlineQos, handle); the_rtps_writer.writer_cache.add_change(a_change); }"

**Repo:** `crates/dcps/src/publisher.rs`
(`DataWriter::unregister_instance_w_timestamp`);
`crates/dcps/src/runtime.rs` (`write_user_lifecycle`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`,
`crates/rtps/src/reliable_reader.rs::tests::unregister_data_yields_not_alive_unregistered`

**Status:** `done`

### §8.2.10.1 T3/T4 ohne Wirkung bei NO_KEY

**Spec:** §8.2.10.1, S. 32–33 — dispose und unregister: "This operation has
no effect if the topicKind==NO_KEY."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle_with`, `keyless_lifecycle_not_sent`); `writer.keyless_lifecycle_on_wire`

**Tests:** `crates/dcps/src/runtime.rs::tests::dispose_and_unregister_of_a_no_key_writer_send_no_marker_unless_asked_for`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`,
`crates/dcps/tests/keyless_instance.rs::the_writer_disposes_the_one_instance_of_a_keyless_topic`

**Status:** `done` — dispose und unregister eines Writers ohne Schlüssel erzeugen keinen Change und senden keinen Marker (gezählt in `DcpsRuntime::keyless_lifecycle_not_sent`); der Instanzzustand des Writers selbst ändert sich (DDS 1.4 §2.2.1.2.2: eine Instanz). `writer.keyless_lifecycle_on_wire` (Umgebung `ZERODDS_WRITER_KEYLESS_LIFECYCLE_ON_WIRE`) sendet den Marker (nur Status-Bits) für einen Peer, der ihn erwartet. Erwartet bei den Herstellern: Reader von Topics ohne Schlüssel sehen kein NOT_ALIVE_DISPOSED der ZeroDDS-Writer mehr; ob Cyclone DDS, Fast DDS oder OpenDDS selbst einen Marker für NO_KEY senden, ist nicht gemessen.

### §8.2.10.1 Table 8.11 DataWriter T5 (delete DDS DataWriter)

**Spec:** §8.2.10.1, S. 33 — "delete the_dds_writer.related_rtps_writer;"

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`

**Status:** `done`

### §8.2.10.2 DataReader: read und take greifen auf den HistoryCache zu

**Spec:** §8.2.10.2, S. 33 — "The changes returned to the user are those in
the HistoryCache that pass all Reader specific filters, if any."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`,
`DataReader::take`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T1 (new DDS DataReader)

**Spec:** §8.2.10.2, S. 34, Table 8.12 T1 — "the_rtps_reader = new
RTPS::Reader; the_dds_reader.related_rtps_reader := the_rtps_reader;"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`);
`crates/dcps/src/runtime.rs` (`register_user_reader`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T2 (read)

**Spec:** §8.2.10.2, S. 34 — "Changes returned to the application remain in
the RTPS Reader's HistoryCache such that subsequent read or take operations
can find them again."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`,
`DataReader::read_with_info`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/tests/reader_history_resource_limits.rs::plain_read_keep_last_1_keeps_only_the_newest_per_instance_and_is_idempotent`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T3 (take)

**Spec:** §8.2.10.2, S. 34–35 — "Changes returned to the application are
removed from the RTPS Reader's HistoryCache such that subsequent read or take
operations do not find the same change."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take`,
`DataReader::take_with_info`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T4 (delete DDS DataReader)

**Spec:** §8.2.10.2, S. 35 — "delete the_dds_reader.related_rtps_reader;"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`,
`crates/dcps/tests/entity_deletion.rs::lookup_datareader_hands_out_a_handle_to_the_same_reader`

**Status:** `done`

---

## §8.3 Messages Module

### §8.3.1 Overview

**Spec:** §8.3.1, S. 35 — Gliederung des Moduls; "The number of Submessages
that can be sent in a single RTPS Message is only limited by the maximum
message size the underlying transport can support."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Anforderungen folgen in
§8.3.2–§8.3.8.

### §8.3.2 Table 8.13 ProtocolId_t (PROTOCOL_RTPS)

**Spec:** §8.3.2, S. 35, Table 8.13 — "ProtocolId_t: Enumeration used to
identify the protocol. The following values are reserved by the protocol:
PROTOCOL_RTPS".

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`,
`crates/rtps/src/header.rs::tests::header_decode_rejects_invalid_magic`

**Status:** `done`

### §8.3.2 Table 8.13 SubmessageFlag

**Spec:** §8.3.2, S. 35, Table 8.13 — "A Submessage flag takes a boolean value
and affects the parsing of the Submessage by the receiver."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::flags`,
`FLAG_E_LITTLE_ENDIAN`); Flag-Konstanten in `crates/rtps/src/submessages.rs`

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_is_little_endian_flag`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §8.3.2 Table 8.13 SubmessageKind

**Spec:** §8.3.2, S. 36, Table 8.13 — reserviert "RTPS_HE, DATA, GAP,
HEARTBEAT, ACKNACK, PAD, INFO_TS, INFO_REPLY, INFO_DST, INFO_SRC, DATA_FRAG,
NACK_FRAG, HEARTBEAT_FRAG".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId`);
`crates/rtps/src/header_extension.rs` (`SUBMESSAGE_ID_HEADER_EXTENSION`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`,
`crates/rtps/tests/cyclone_he_must_understand.rs::local_encoder_produces_must_understand_bit`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.2 Table 8.13 Time_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold a timestamp. Should
have at least nano-second resolution. The following values are reserved by
the protocol: TIME_ZERO, TIME_INVALID, TIME_INFINITE".

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`, `TIME_INVALID`, `TIME_INFINITE`); `crates/dcps/src/time.rs`

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::time_t_seconds_are_unsigned`,
`crates/dcps/src/time.rs::tests::the_reserved_time_t_values_cross_the_wire_as_the_spec_gives_them`

**Status:** `done` — `seconds` ist vorzeichenlos (`u32`, §9.3.2.1), die drei reservierten Werte TIME_ZERO, TIME_INVALID und TIME_INFINITE sind Konstanten und gehen mit den Werten der Spec auf die Leitung. Das DCPS-`Time` zählt Sekunden als `long`; die Oktette sind dieselben.

### §8.3.2 Table 8.13 Count_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold a count that is
incremented monotonically, used to identify message duplicates."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::count`,
`AckNackSubmessage::count`, `HeartbeatFragSubmessage::count`,
`NackFragSubmessage::count`, jeweils `i32`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_wraps_around_at_i32_max_per_spec_8_4_15_7`

**Status:** `done` — die Auswertung des Zählers beim Empfänger ist unter
§8.3.8.1, §8.3.8.6 und §8.4.15.7 geführt.

### §8.3.2 Table 8.13 Checksum_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold a checksum. Used to
detect RTPS message corruption by the underlying transport. The following
values are reserved by the protocol: CHECKSUM_INVALID."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::{None,
Crc32c, Crc64, Md5}`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_value_kind_matches`,
`crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`

**Status:** `done` — `ChecksumValue::None` steht für CHECKSUM_INVALID.

### §8.3.2 Table 8.13 MessageLength_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold the length of an
RTPS Message. The following values are reserved by the protocol:
MESSAGE_LENGTH_INVALID".

**Repo:** `crates/rtps/src/header_extension.rs`
(`HeaderExtension::message_length: Option<u32>`); `crates/rtps/src/receiver_state.rs`
(`ReceiverState::message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_message_length_only_le`,
`crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done` — `None` steht für MESSAGE_LENGTH_INVALID.

### §8.3.2 Table 8.13 ParameterId_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to uniquely identify a
parameter in a parameter list. … A range of values is reserved for
protocol-defined parameters, while another range can be used for
vendor-defined parameters".

**Repo:** `crates/rtps/src/parameter_list.rs` (`Parameter::id: u16`,
`VENDOR_SPECIFIC_BIT`, `MUST_UNDERSTAND_BIT`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`,
`crates/rtps/src/parameter_list.rs::tests::must_understand_unknown_optional_pid_skips`

**Status:** `done` — die Bereichsaufteilung ist unter §8.3.5.9 und
§9.4.2.11 geführt.

### §8.3.2 Table 8.13 FragmentNumber_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold fragment numbers.
Must be possible to represent using 32 bits."

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_wire_size_is_four`,
`crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`

**Status:** `done`

### §8.3.2 Table 8.13 GroupDigest_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold a digest value that
uniquely identifies a group of Entities belonging to the same Participant."

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §8.3.2 Table 8.13 UExtension4_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold an undefined 4-byte
value. It is intended to be used in future revisions of the specification."

**Repo:** `crates/rtps/src/wire_types.rs` (`UExtension4`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::uextension4_roundtrip`

**Status:** `done`

### §8.3.2 Table 8.13 WExtension8_t

**Spec:** §8.3.2, S. 36, Table 8.13 — "Type used to hold an undefined 8-byte
value. It is intended to be used in future revisions of the specification."

**Repo:** `crates/rtps/src/wire_types.rs` (`WExtension8`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::wextension8_roundtrip`

**Status:** `done`

### §8.3.3 Nachricht = Header + Submessages

**Spec:** §8.3.3, S. 36 — "The overall structure of an RTPS Message consists
of a fixed-size leading RTPS Header followed by a variable number of RTPS
Submessage parts. Each Submessage in turn consists of a SubmessageHeader and
a variable number of SubmessageElements."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`);
`crates/rtps/src/message_builder.rs` (`MessageBuilder`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/tests/vendor_frames.rs::cyclone_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.3 Nachrichtenlänge über Transport oder HeaderExtension

**Spec:** §8.3.3, S. 37 — "The length may also be sent by the underlying
transport that carries the RTPS message. In this case it may be omitted from
the HeaderExtension. … a stream-oriented transport (like TCP) would need to
include the length in the RTPS HeaderExtension".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: Länge aus dem
UDP-Datagramm)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`

**Status:** `done` — für UDP (§9) trägt der Transport die Länge; die
TCP-Abbildung gehört nicht zum UDP/IP-PSM.

### §8.3.3.1 Header am Anfang jeder Nachricht

**Spec:** §8.3.3.1, S. 37 — "The RTPS Header must appear at the beginning of
every message."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::to_bytes`,
`RtpsHeader::from_bytes`); `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_invalid_magic`,
`crates/rtps/src/writer.rs::tests::writer_produces_decodable_datagram`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.protocol = PROTOCOL_RTPS

**Spec:** §8.3.3.1, S. 38 — "The protocol identifies the message as an RTPS
message. This value is set to PROTOCOL_RTPS."

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`, `RtpsHeader::to_bytes`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.version = 2.5

**Spec:** §8.3.3.1, S. 38 — "Implementations following this version of the
document implement protocol version 2.5 (major = 2, minor = 5) and have this
field set to PROTOCOLVERSION."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::new` setzt
`ProtocolVersion::V2_5`)

**Tests:** `crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.vendorId

**Spec:** §8.3.3.1, S. 38 — "The vendorId identifies the vendor of the
middleware that implemented the RTPS protocol … The possible values for the
vendorId are assigned by the OMG."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::vendor_id`);
`crates/rtps/src/wire_types.rs` (`VendorId::ZERODDS = [0x01, 0xF0]`)

**Tests:** `crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_zerodds_constant`

**Status:** `partial` — das Feld wird gesendet und gelesen; ZeroDDS sendet
`0x01F0`, laut Kommentar an `VendorId::ZERODDS` ein Zwischenwert und kein von
der OMG zugewiesener Wert (eigenes `open`-Item unter §8.3.5.2). Blocker: ein harter externer, die Zuweisung einer VendorId durch die OMG; der Antrag läuft seit dem 22.06. Bis dahin sendet ZeroDDS `0x01F0`, einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

### §8.3.3.1 Table 8.14 Header.guidPrefix

**Spec:** §8.3.3.1, S. 38 — "The guidPrefix defines a default prefix that can
be used to reconstruct the Globally Unique Identifiers (GUIDs) that appear
within the Submessages contained in the message."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::guid_prefix`);
`crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`)

**Tests:** `crates/rtps/src/header.rs::tests::header_guid_prefix_at_bytes_8_to_19`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.3.1 Header-Struktur in Version 2 unveränderlich

**Spec:** §8.3.3.1, S. 38 — "The structure of the RTPS Header cannot be
changed in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::WIRE_SIZE = 20`)

**Tests:** `crates/rtps/src/header.rs::tests::header_total_size_is_20_bytes`,
`crates/rtps/src/header.rs::tests::header_roundtrip`

**Status:** `done`

### §8.3.3.2 HeaderExtension direkt nach dem Header

**Spec:** §8.3.3.2, S. 38 — "The HeaderExtension may optionally appear
immediately following the Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: HE nur als erste
Submessage)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`

**Status:** `done`

### §8.3.3.2 HeaderExtension für Versionen bis 2.4 überspringbar

**Spec:** §8.3.3.2, S. 39 — "versions of the protocol that do not understand
the HeaderExtension will treat it as 'unknown messge kind', skip it, and
continue processing the submessages that follow it".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension`

**Status:** `n/a (informative)` — beschreibt das Verhalten älterer
Implementierungen; für ZeroDDS selbst gilt §8.3.4.1.

### §8.3.3.2 Table 8.15 HE EndiannessFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_E`,
`HeaderExtension::little_endian`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::empty_he_be_roundtrip`,
`crates/rtps/src/header_extension.rs::tests::wire_layout_be_message_length_correct`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE LengthFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "LengthFlag … Indicates the
messageLength field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_L`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::flag_byte_with_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE TimestampFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "TimestampFlag … Indicates the
rtpsSendTimestamp field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_T`, Bit 2)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_timestamp_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE UExtensionFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "UExtensionFlag … Indicates the
uExtension4 field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_U`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE WExtensionFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "WExtensionFlag … Indicates the
wExtension8 field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_W`, Bit 4)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE ChecksumFlags

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "ChecksumFlags (2 flags) … Indicates
the presence and format of the messageChecksum field."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_C0`, `HE_FLAG_C1`,
`ChecksumKind::from_flags`, `ChecksumKind::to_flag_bits`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE ParametersFlag

**Spec:** §8.3.3.2, S. 39, Table 8.15 — "ParametersFlag … Indicates the
parameters field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_P`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE messageLength

**Spec:** §8.3.3.2, S. 39–40 — "The messageLength indicates the length of the
complete RTPS Message, starting from the beginning of the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, HeaderExtension); `crates/rtps/src/header_extension.rs` (`validate_message_length`, `restate_for_rebuilt_message`); `crates/dcps/src/runtime.rs` (`unprotect_volatile_datagram`, `unprotect_user_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_too_small_for_header_and_extension_is_invalid`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_counts_from_the_start_of_the_rtps_header`,
`crates/rtps/src/datagram.rs::tests::a_rebuilt_message_takes_the_header_extension_with_its_own_length`

**Status:** `done` — der Decoder weist ein `messageLength` ab, das kleiner ist als Header und HeaderExtension, und eines, das von der Länge der empfangenen Nachricht ab dem RTPS-Header abweicht. Eine Nachricht, die der Empfänger nach dem Öffnen geschützter Submessages neu zusammensetzt, wird wie empfangen geprüft und bekommt dann die HeaderExtension mit ihrer eigenen Länge und ohne Prüfsumme (`restate_for_rebuilt_message`); unverändert wurde jede geöffnete Nachricht von CoreDX 6.14.1 abgewiesen, das ein `messageLength` sendet, ihre Krypto-Tokens eingeschlossen (OMG dds-security, Linux-Testhost).

### §8.3.3.2 Table 8.15 HE rtpsSendTimestamp

**Spec:** §8.3.3.2, S. 40 — "The rtpsSendTimestamp indicates the time when
the RTPS message was sent by the originating Participant. … shall collected
as close as possible to the moment when the RTPS Message is sent … The time
origin … shall be the same as the one used in the Timestamp Submessage
Element".

**Repo:** `crates/rtps/src/header_extension.rs`, `crates/rtps/src/receiver_state.rs` (`apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_timestamp_is_no_source_timestamp`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`

**Status:** `done` — Feld kodiert und gelesen; beim Empfang setzt es `Receiver.rtpsSendTimestamp` (§8.3.7.4), nicht den Quellzeitstempel. ZeroDDS sendet keine HeaderExtension (sie ist optional, §8.3.3.2); die Vorgabe für den Erfassungszeitpunkt gilt für einen Sender, der sie schreibt.

### §8.3.3.2 Table 8.15 HE uExtension4

**Spec:** §8.3.3.2, S. 40 — "The uExtension4 is undefined in this version of
the protocol."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE wExtension8

**Spec:** §8.3.3.2, S. 40 — "The wExtension8 is undefined in this version of
the protocol."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE messageChecksum

**Spec:** §8.3.3.2, S. 40 — "The messageChecksum provides a checksum computed
over the complete of the RTPS Message, which includes the RTPS Header, the
HeaderExtension, and all Submessages that may follow."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`,
`crates/rtps/src/datagram.rs::tests::a_message_with_a_wrong_checksum_is_refused_and_counted`

**Status:** `done` — die Prüfsumme der ganzen Nachricht mit genulltem Feld wird berechnet (`seal_message`) und beim Empfang geprüft; ein Fehler verwirft die Nachricht und wird gezählt (`refused_messages`). Einen Sendepfad, der eine HeaderExtension mit Prüfsumme schreibt, gibt es in der Runtime nicht.

### §8.3.3.2 Table 8.15 HE parameters

**Spec:** §8.3.3.2, S. 40 — "The parameters provide an extensibility
mechanism. It enables revisions of the protocol to add information to the
HeaderExtension without breaking interoperability".

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::parameters`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_malformed_parameter_list_when_p_flag_set`,
`crates/rtps/src/header_extension.rs::tests::decode_takes_a_parameter_list_without_sentinel`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.submessageId

**Spec:** §8.3.3.3, S. 41, Table 8.16 — "submessageId: SubmessageKind —
Identifies the kind of Submessage."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::submessage_id`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_le`,
`crates/rtps/src/submessage_header.rs::tests::submessage_id_roundtrip_for_known_ids`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.flags (Bit 0 = Endianness)

**Spec:** §8.3.3.3, S. 41–42 — "The first flag, the EndiannessFlag, is present
and located in the same position in all Submessages … If the EndiannessFlag is
set to FALSE, the Submessage is encoded in big-endian format, EndiannessFlag
set to TRUE means little-endian."

**Repo:** `crates/rtps/src/submessage_header.rs` (`FLAG_E_LITTLE_ENDIAN`,
`SubmessageHeader::is_little_endian`); `crates/rtps/src/datagram.rs`
(`SubmessageFrame::little_endian`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_be`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_le`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.submessageLength

**Spec:** §8.3.3.3, S. 42 — "In case submessageLength > 0, it is either: The
length from the start of the contents of the Submessage until the start of the
header of the next Submessage … Or else it is the remaining Message length".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end =
body_start + octets`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`

**Status:** `done`

### §8.3.3.3 submessageLength == 0: letzte Submessage

**Spec:** §8.3.3.3, S. 42 — "In case submessageLength==0, the Submessage is
the last Submessage in the Message and extends up to the end of the
Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`SubmessageFrame::octets_elided`, `encode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`,
`crates/rtps/src/datagram.rs::tests::an_info_ts_without_a_body_is_not_the_last_submessage`,
`crates/rtps/tests/vendor_frames.rs::opendds_datagrams_re_encode_byte_for_byte`

**Status:** `done` — die Ausnahme für PAD und INFO_TS steht in §9.4.5.1.3.

### §8.3.3.3 Submessage-Struktur in Version 2 unveränderlich

**Spec:** §8.3.3.3, S. 41 — "The structure of the RTPS Submessage cannot be
changed in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::WIRE_SIZE = 4`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_be`

**Status:** `done`

### §8.3.4 Receiver-Zustand je Nachricht zurückgesetzt

**Spec:** §8.3.4, S. 42 — "This state is modeled as the state of an RTPS
Receiver that is reset each time a new message is processed and provides
context for the interpretation of each Submessage."

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`
legt je Datagramm einen neuen `ReceiverState` an)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert sourceVersion

**Spec:** §8.3.4, S. 43, Table 8.17 — "sourceVersion: PROTOCOLVERSION".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_version`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — Anfangswert PROTOCOLVERSION (2.5).

### §8.3.4 Table 8.17 Anfangswert sourceVendorId

**Spec:** §8.3.4, S. 43, Table 8.17 — "sourceVendorId: VENDORID_UNKNOWN".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_vendor_id`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — Anfangswert VENDORID_UNKNOWN.

### §8.3.4 Table 8.17 Anfangswert sourceGuidPrefix

**Spec:** §8.3.4, S. 43, Table 8.17 — "sourceGuidPrefix: GUIDPREFIX_UNKNOWN".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_guid_prefix`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — Anfangswert GUIDPREFIX_UNKNOWN.

### §8.3.4 Table 8.17 Anfangswert destGuidPrefix

**Spec:** §8.3.4, S. 43, Table 8.17 — "destGuidPrefix: GUID prefix of the
participant receiving the message".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`);
`crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert unicastReplyLocatorList

**Spec:** §8.3.4, S. 43, Table 8.17 — "The list is initialized to contain a
single Locator_t with the LocatorKind, Address, and Port fields specified
below: The LocatorKind is set to the kind that identifies the transport that
received the message … The Address is set to the Address of the source of the
message … The port is set to LOCATOR_PORT_INVALID."

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::set_initial_reply_locators`);
`crates/rtps/src/datagram.rs` (`ReceiveContext`, `ParsedDatagram::receiver_walk_in`);
`crates/dcps/src/runtime.rs` (`receive_context`, `receiver_walk_of`, `reply_targets_of`);
`crates/discovery/src/spdp.rs` (`samples_in`), `crates/discovery/src/sedp/stack.rs` (`handle_datagram_in`);
`crates/dcps/src/wlp.rs`, `crates/dcps/src/type_lookup_transport.rs`

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_from_starts_the_reply_lists_at_the_transport_values`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_in_applies_the_source_and_the_clock_of_the_context`,
`crates/dcps/src/runtime.rs::tests::a_reader_proxy_without_locators_takes_the_info_reply_of_its_acknack`

**Status:** `done` — Anfangswert nach Table 8.17 (Transportart, Quelladresse, `LOCATOR_PORT_INVALID`). Jeder Empfangspfad geht über `receiver_walk_in` mit der Quelle des Datagramms und der Empfangsuhr: Nutzer-Daten, SPDP, SEDP, WLP, TypeLookup und die Security-Builtins. Die Antwortseite benutzt die Liste: ein Reader-Proxy ohne eigenen Locator übernimmt die Einträge der Unicast-Liste mit Port (die ein InfoReply liefert; die Anfangswerte haben `LOCATOR_PORT_INVALID`) und behält sie für folgende Antworten.

### §8.3.4 Table 8.17 Anfangswert multicastReplyLocatorList

**Spec:** §8.3.4, S. 43, Table 8.17 — "The list is initialized to contain a
single Locator_t … The LocatorKind is set to the kind that identifies the
transport that received the message … The address is set to
LOCATOR_ADDRESS_INVALID. The port is set to LOCATOR_PORT_INVALID."

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::set_initial_reply_locators`);
`crates/rtps/src/datagram.rs` (`ReceiveContext`, `ParsedDatagram::receiver_walk_in`);
`crates/dcps/src/runtime.rs` (`receive_context`, `receiver_walk_of`, `reply_targets_of`);
`crates/discovery/src/spdp.rs` (`samples_in`), `crates/discovery/src/sedp/stack.rs` (`handle_datagram_in`);
`crates/dcps/src/wlp.rs`, `crates/dcps/src/type_lookup_transport.rs`

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_from_starts_the_reply_lists_at_the_transport_values`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_in_applies_the_source_and_the_clock_of_the_context`,
`crates/dcps/src/runtime.rs::tests::a_reader_proxy_without_locators_takes_the_info_reply_of_its_acknack`

**Status:** `done` — Anfangswert nach Table 8.17 (Transportart, `LOCATOR_ADDRESS_INVALID`, `LOCATOR_PORT_INVALID`). Jeder Empfangspfad geht über `receiver_walk_in` mit der Quelle des Datagramms und der Empfangsuhr: Nutzer-Daten, SPDP, SEDP, WLP, TypeLookup und die Security-Builtins. Die Antwortseite benutzt die Liste: ein Reader-Proxy ohne eigenen Locator übernimmt die Einträge der Unicast-Liste mit Port (die ein InfoReply liefert; die Anfangswerte haben `LOCATOR_PORT_INVALID`) und behält sie für folgende Antworten.

### §8.3.4 Table 8.17 Anfangswert haveTimestamp

**Spec:** §8.3.4, S. 43, Table 8.17 — "haveTimestamp: FALSE".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert timestamp

**Spec:** §8.3.4, S. 43, Table 8.17 — "timestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`);
`crates/rtps/src/header_extension.rs` (`HeTimestamp::TIME_INVALID`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`

**Status:** `done` — {0xffffffff, 0xffffffff}; auch nach InfoSource und InfoTimestamp mit I-Flag.

### §8.3.4 Table 8.17 Anfangswert messageLength

**Spec:** §8.3.4, S. 43, Table 8.17 — "messageLength: MESSAGE_LENGTH_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`:
`message_length: None`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert messageChecksum

**Spec:** §8.3.4, S. 43, Table 8.17 — "messageChecksum: CHECKSUM_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`:
`ChecksumValue::None`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert rtpsSendTimestamp

**Spec:** §8.3.4, S. 43, Table 8.17 — "rtpsSendTimestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::rtps_send_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`,
`crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert rtpsReceptionTimestamp

**Spec:** §8.3.4, S. 43, Table 8.17 — "rtpsReceptionTimestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`rtps_reception_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`

**Status:** `done` — das Feld beginnt als TIME_INVALID (`None`).

### §8.3.4 Table 8.17 Anfangswert clockSkewDetected

**Spec:** §8.3.4, S. 43, Table 8.17 — "clockSkewDetected: FALSE".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 Anfangswert parameters

**Spec:** §8.3.4, S. 43, Table 8.17 — "parameters: The list is initialized as
an empty list."

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `parameters`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — Anfangswert eine leere Liste.

### §8.3.4.1 Regel 1: unlesbarer Submessage-Header

**Spec:** §8.3.4.1, S. 43 — "If the full Submessage header cannot be read,
the rest of the Message is considered invalid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `UnexpectedEof`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_truncated_submessage_header_invalidates_the_rest`

**Status:** `done` — der Rest der Nachricht wird verworfen, der Fehler steht in
`ParsedDatagram::invalid_rest`; die Submessages davor bleiben.

### §8.3.4.1 Regel 2: ungültiges submessageLength

**Spec:** §8.3.4.1, S. 43 — "The submessageLength field defines where the
next Submessage starts … If this field is invalid, the rest of the Message is
invalid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end >
bytes.len()` ergibt `UnexpectedEof`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`,
`crates/rtps/tests/fuzz_smoke.rs::fuzz_decode_datagram_does_not_panic`

**Status:** `done` — wie Regel 1: der Rest wird verworfen, die vorangehenden
Submessages bleiben.

### §8.3.4.1 Regel 3: unbekannte SubmessageId ignorieren

**Spec:** §8.3.4.1, S. 43 — "A Submessage with an unknown SubmessageId must be
ignored and parsing must continue with the next Submessage. … SubmessageIds in
the vendor-specific range coming from a vendorId that is unknown must also be
ignored".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_unknown_submessage_without_must_understand`,
`crates/rtps/src/datagram.rs::tests::decode_marks_unknown_submessage_id_without_failing`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done`

### §8.3.4.1 Regel 3: kein Must-Understand auf Submessage-Ebene

**Spec:** §8.3.4.1, S. 43–44 — Regeln 3 und 4: unbekannte Submessages und
unbekannte Flags werden ignoriert; RTPS 2.5 kennt ein Must-Understand nur je
ParameterId (Table 9.6, S. 165).

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — eine unbekannte Submessage wird übersprungen, gleich welche
Flags sie trägt.

### §8.3.4.1 Regel 4: unbekannte Flags ignorieren

**Spec:** §8.3.4.1, S. 44 — "The receiver of a Submessage should ignore unknown
flags. An implementation of RTPS 2.5 should skip all flags that are marked as
'X' (unused) in the protocol."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::unknown_flags_of_a_known_submessage_are_ignored`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — bei bekannten Submessages ändern die oberen drei Flag-Bits, die keine Submessage definiert, nichts am Gelesenen; eine unbekannte Submessage wird übersprungen, gleich welche Flags sie trägt.

### §8.3.4.1 Regel 5: submessageLength auch bei bekannten Kennungen

**Spec:** §8.3.4.1, S. 44 — "A valid submessageLength field must always be used
to find the next Submessage, even for Submessages with known IDs."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `pos = body_end`
für jede Submessage)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`,
`crates/rtps/tests/vendor_frames.rs::rti_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.4.1 Regel 6: bekannte, ungültige Submessage

**Spec:** §8.3.4.1, S. 44 — "A known but invalid Submessage invalidates the
rest of the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — eine Submessage, die ihr Decoder nicht lesen kann oder deren „Validity“-Bedingung (§8.3.8) verletzt ist, macht den Rest der Nachricht ungültig; die Submessages davor bleiben (`invalid_rest`, gezählt). Die eine Bedingung, die der Definition von currentGSN widerspricht, steht unter §8.3.8.6 „Heartbeat ungültig: Group-Felder“.

### §8.3.4.1 Gültige Submessages vor einer ungültigen

**Spec:** §8.3.4.1, S. 43–44 — Regeln 1, 2 und 6 erklären nur „the rest of
the Message“ für ungültig.

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — `decode_datagram` liefert die gültig gelesenen Submessages
vor einer ungültigen und vermerkt den Fehler in `invalid_rest`; die
Empfangspfade verarbeiten sie.

### §8.3.4.1 Regel 6 bei inhaltlich ungültigen Submessages

**Spec:** §8.3.4.1, S. 44 — "A known but invalid Submessage invalidates the
rest of the Message." Die Gültigkeitsbedingungen je Submessage stehen in
§8.3.8 unter „Validity“.

**Repo:** `crates/rtps/src/submessages.rs` (Decoder je Submessage); `crates/rtps/src/datagram.rs`
(`ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`,
`crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/dcps/src/runtime.rs::tests::the_empty_acknack_setting_is_off_by_default`

**Status:** `done` — die Bedingungen der „Validity“-Abschnitte von §8.3.8 prüfen die
Decoder der Submessages; ein Verstoß ist ein Fehler des Decoders und macht den Rest
der Nachricht ungültig, die Submessages davor bleiben.
Gezählt, nicht still verworfen: `zerodds_rtps::datagram::invalid_message_rests()`,
`refused_messages()` und `skipped_must_understand_submessages()` (prozessweit),
am Runtime als `DcpsRuntime::invalid_message_rests`, `refused_messages`,
`skipped_must_understand_submessages`.

### §8.3.4.1 Zustandsändernde Submessages

**Spec:** §8.3.4.1, S. 44 — "In this version of the protocol, only the Header
and the Submessages HeaderExtension, InfoSource, InfoReply, InfoDestination,
and InfoTimestamp change the state of the Receiver."

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_yields_every_submessage_including_info_ones`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_ip4_sets_reply_locators`

**Status:** `done` — zusätzlich ändert InfoReplyIp4 (PSM §9.4.5.15) den
Zustand wie InfoReply.

### §8.3.5 SubmessageElements

**Spec:** §8.3.5, S. 44 — "Each RTPS Submessage in turn is built from a set of
predefined atomic building blocks called SubmessageElements."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Elemente sind unter
§8.3.5.1–§8.3.5.19 einzeln geführt.

### §8.3.5.1 Table 8.18 GuidPrefix-Element

**Spec:** §8.3.5.1, S. 44, Table 8.18 — "value: GuidPrefix_t — Identifies the
GuidPrefix_t part of the GUID_t of the Entity that is the source or target of
the message."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::guid_prefix`);
`crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`,
`crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`

**Status:** `done`

### §8.3.5.1 Table 8.19 EntityId-Element

**Spec:** §8.3.5.1, S. 45, Table 8.19 — "value: EntityId_t — Identifies the
EntityId_t part of the GUID_t of the Entity that is the source or target of
the message."

**Repo:** `crates/rtps/src/submessages.rs` (Felder `reader_id`, `writer_id`
aller Entity-Submessages)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §8.3.5.2 Table 8.20 VendorId-Element

**Spec:** §8.3.5.2, S. 45, Table 8.20 — "value: VendorId_t — Identifies the
vendor of the middleware that implements the protocol."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::vendor_id`);
`crates/rtps/src/header.rs` (`RtpsHeader::vendor_id`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`

**Status:** `done`

### §8.3.5.2 VendorId von der OMG zugewiesen

**Spec:** §8.3.5.2, S. 45 — "The following values are reserved by the
protocol: VENDORID_UNKNOWN. Other values must be assigned by the OMG."

**Repo:** —

**Tests:** —

**Status:** `open` — ZeroDDS sendet `VendorId::ZERODDS = 0x01F0`
(`crates/rtps/src/wire_types.rs`); der Kommentar dort nennt den Wert einen
Zwischenwert, bis eine VendorId beantragt ist. Blocker: ein harter externer, die Zuweisung einer VendorId durch die OMG; der Antrag läuft seit dem 22.06. Bis dahin sendet ZeroDDS `0x01F0`, einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

### §8.3.5.3 Table 8.21 ProtocolVersion-Element

**Spec:** §8.3.5.3, S. 45, Table 8.21 — "value: ProtocolVersion_t — Identifies
the major and minor version of the RTPS protocol." Sonderwerte
PROTOCOLVERSION_1_0 … PROTOCOLVERSION_2_4.

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion`);
`crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::protocol_version`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_all_aliases_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`

**Status:** `done`

### §8.3.5.4 Table 8.22 SequenceNumber: 64 Bit mit Vorzeichen

**Spec:** §8.3.5.4, S. 45 — "A sequence number is a 64-bit signed integer,
that can take values in the range: -2^63 <= N <= 2^63-1."

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber(i64)`,
`SequenceNumber::split`, `SequenceNumber::from_high_low`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_split_high_low`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §8.3.5.4 Sequenznummern beginnen bei 1

**Spec:** §8.3.5.4, S. 45 — "Sequence numbers begin at 1."

**Repo:** `crates/rtps/src/writer.rs` (`BestEffortWriter`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/writer.rs::tests::writer_starts_at_sequence_number_one`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::new_change_assigns_monotonic_sn`

**Status:** `done`

### §8.3.5.4 SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.5.4, S. 46 — "The protocol reserves the following value:
SEQUENCENUMBER_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`

**Status:** `done`

### §8.3.5.5 Table 8.23 SequenceNumberSet base/set

**Spec:** §8.3.5.5, S. 46, Table 8.23 — "base: SequenceNumber_t — Identifies
the first sequence number in the set. set: SequenceNumber_t[*] — A set of
sequence numbers, each verifying that: base <= element(set) <= base+255".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet`,
`from_missing`, `iter_set`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_iter_matches_missing`,
`crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_roundtrip_le`

**Status:** `done`

### §8.3.5.5 Gültigkeit SequenceNumberSet

**Spec:** §8.3.5.5, S. 46 — "a valid SequenceNumberSet must verify that:
maximum(SequenceNumberSet) - minimum(SequenceNumberSet) < 256,
minimum(SequenceNumberSet) >= 1".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`:
`numBits <= 256`, `bitmapBase >= 1`; `from_missing` kappt das Fenster bei 256)

**Tests:** `crates/rtps/tests/fuzz_smoke.rs::fuzz_sequence_number_set_read_from_does_not_panic`,
`crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_wire_size_matches_encoded`,
`crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/tests/vendor_frames.rs::the_empty_acknack_of_opendds_and_rti_is_invalid_by_default`,
`crates/rtps/tests/vendor_frames.rs::opendds_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`,
`crates/rtps/src/submessages.rs::validity_tests::the_empty_set_with_base_zero_is_accepted_only_on_request`

**Status:** `done` — die Spannweite ist auf 256 begrenzt, `minimum >= 1` prüft der Decoder.
Vendor-Form: OpenDDS 3.34.0 und RTI Connext 7.7.0 senden als ACKNACK eines Readers
ohne Samples `readerSNState` `0/0:` (bitmapBase 0, numBits 0; gemessen in
`tools/wire-capture/rtps-sessions`: 40 und 8 ACKNACK). Standard: ungültig. Die
Einstellung `wire.accept_empty_acknack_base_zero` (Env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, Standard `false`) nimmt genau diese
leere Menge in einem ACKNACK an, prozessweit.

### §8.3.5.6 Table 8.24 FragmentNumber

**Spec:** §8.3.5.6, S. 46 — "A fragment number is a 32-bit unsigned integer …
Fragment numbers start at 1."

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`);
`crates/rtps/src/reliable_writer.rs` (Fragmentierung ab Nummer 1)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`

**Status:** `done`

### §8.3.5.7 Table 8.25 FragmentNumberSet

**Spec:** §8.3.5.7, S. 46, Table 8.25 — "base: FragmentNumber_t … set:
FragmentNumber_t[*] — A set of fragment numbers, each verifying that: base <=
element(set) <= base+255". Gültig nur mit "maximum - minimum < 256,
minimum >= 1".

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet`,
`from_missing`, `read_from`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::frag_set_roundtrip_le`,
`crates/rtps/tests/fuzz_smoke.rs::fuzz_fragment_number_set_read_from_does_not_panic`,
`crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`

**Status:** `done` — Spannweite auf 256 begrenzt, `bitmapBase >= 1` wird geprüft.

### §8.3.5.8 Table 8.26 Timestamp

**Spec:** §8.3.5.8, S. 47 — "Timestamp is used to represent time. The
representation should be capable of having a resolution of nano-seconds or
better." Sonderwerte TIME_ZERO, TIME_INVALID, TIME_INFINITE.

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`);
`crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`); `crates/dcps/src/time.rs`

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::validity_tests::time_t_seconds_are_unsigned`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done` — Auflösung 2⁻³² s, vorzeichenlose Sekunden, TIME_INVALID und TIME_INFINITE als Konstanten.

### §8.3.5.9 Table 8.27/8.28 ParameterList und Parameter

**Spec:** §8.3.5.9, S. 47, Table 8.27 und 8.28 — "parameter: Parameter[*]";
je Parameter "parameterId: ParameterId_t", "length: short", "value:
octet[length]".

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList`, `Parameter`,
`to_bytes`, `from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::single_parameter_encodes_id_length_value`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_multiple_parameters`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_be_endianness`

**Status:** `done`

### §8.3.5.9 Höchstens 2^16 ParameterIds

**Spec:** §8.3.5.9, S. 47 — "There shall be no more than 2^16 possible values
of the ParameterId_t parameterId."

**Repo:** `crates/rtps/src/parameter_list.rs` (`Parameter::id: u16`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::single_parameter_encodes_id_length_value`

**Status:** `done`

### §8.3.5.9 2^15 Werte für protokolldefinierte Parameter

**Spec:** §8.3.5.9, S. 47 — "A range of 2^15 values is reserved for
protocol-defined parameters. All the parameter_id values defined by the 2.5
version of the protocol and all future revisions of the same major version
must use values in this range."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid`, `is_standard_pid`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`,
`crates/rtps/src/parameter_list.rs::tests::is_standard_pid_unknown_pid_returns_false`

**Status:** `done`

### §8.3.5.9 2^15 Werte für herstellerdefinierte Parameter

**Spec:** §8.3.5.9, S. 47 — "A range of 2^15 values is reserved for
vendor-defined parameters."

**Repo:** `crates/rtps/src/parameter_list.rs` (`VENDOR_SPECIFIC_BIT`,
`pid::SHM_LOCATOR`, `pid::ZERODDS_TYPE_ID`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendors_0x8002_is_not_read_as_a_type_identifier`,
`crates/rtps/src/publication_data.rs::tests::a_zerodds_0x8002_is_read_and_the_announcement_states_the_vendor`

**Status:** `done`

### §8.3.5.9 Parameterlänge höchstens 2^16 Oktette

**Spec:** §8.3.5.9, S. 47 — "The maximum length of any parameter is limited to
2^16 octets."

**Repo:** `crates/rtps/src/parameter_list.rs` (`length` als `u16`)

**Tests:** `crates/rtps/tests/user_data_propagation.rs::user_data_large_payload_32kib`

**Status:** `done`

### §8.3.5.10 Table 8.29 Count

**Spec:** §8.3.5.10, S. 47–48 — "Count is used by several Submessages and
enables a receiver to detect duplicates of the same Submessage."

**Repo:** `crates/rtps/src/submessages.rs` (`count`-Felder)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`

**Status:** `done` — das Element ist vorhanden; die Auswertung beim Empfänger
ist unter §8.3.8.1 und §8.3.8.6 geführt.

### §8.3.5.11 Table 8.30 ChangeCount

**Spec:** §8.3.5.11, S. 48 — "ChangeCount is used in the Gap Submessage and
enables the sender to indicate the number of changes within the Gap that
belong to a certain category."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — in der GAP-Submessage 64 Bit (acht Oktette, high und low).

### §8.3.5.12 Table 8.31 Checksum

**Spec:** §8.3.5.12, S. 48, Table 8.31 — "value: Checksum32_t, or
Checksum64_t, or Checksum128_t — A checksum of the RTPS Message, including the
RTPS Header."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue`,
`ChecksumKind`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_crc32c_le`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_crc64_be`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_md5_le`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`

**Status:** `done` — die drei Formate, CRC in Big-Endian.

### §8.3.5.12 Prüfsumme je Transport-PSM

**Spec:** §8.3.5.12, S. 48 — "the Checksum may be omitted from the
HeaderExtension. The specific behavior shall be defined for each Transport
PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Auftrag an die PSMs; der UDP/IP-PSM regelt
die Prüfsumme in §9.4.2.15.

### §8.3.5.13 Table 8.32 MessageLength

**Spec:** §8.3.5.13, S. 48 — "MessageLength is used in the HeaderExtension
Submessage and enables the sender to indicate the length of the RTPS
message."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.14 Table 8.33 UExtension4

**Spec:** §8.3.5.14, S. 48–49 — "UExtension4 is used in the HeaderExtension
Submessage and enables the sender to add 4 octets of information to the
header."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.15 Table 8.34 WExtension8

**Spec:** §8.3.5.15, S. 49 — "WExtension8 is used in the HeaderExtension
Submessage and enables the sender to add 8 octets of information to the
header."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.16 Table 8.35 LocatorList

**Spec:** §8.3.5.16, S. 49 — "LocatorList is used to specify a list of
locators."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::unicast_locators`,
`multicast_locators`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_reply_with_multicast_via_datagram`

**Status:** `done`

### §8.3.5.17 Table 8.36 SerializedData

**Spec:** §8.3.5.17, S. 49 — "The RTPS protocol does not interpret the
serialized data-stream. Therefore, it is represented as opaque data."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::serialized_payload`);
`crates/rtps/src/payload.rs` (`Payload`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_shared_decode_gives_ranges_of_the_datagram`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_roundtrip_bit_identical`

**Status:** `done`

### §8.3.5.18 Table 8.37 SerializedDataFragment

**Spec:** §8.3.5.18, S. 49 — "SerializedDataFragment contains the serialized
representation of a data-object that has been fragmented. … the RTPS protocol
does not interpret the fragmented serialized data-stream."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::serialized_payload`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_frag_preserves_flags_and_payload`

**Status:** `done`

### §8.3.5.19 Table 8.38 GroupDigest

**Spec:** §8.3.5.19, S. 49–50 — "GroupDigest is used to communicate a set of
EntityId_t in a compact manner."

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §8.3.6.1 Header: Purpose

**Spec:** §8.3.6.1, S. 50 — "The Header is used to identify the message as
belonging to the RTPS protocol, to identify the version of the RTPS protocol
used, and to provide context information".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe; die Felder sind unter
§8.3.3.1 geführt.

### §8.3.6.2 Header: Content

**Spec:** §8.3.6.2, S. 50 — "The structure of the Header can only be changed
if the major version of the protocol is also changed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — wiederholt §8.3.3.1; dort als Item
„Header-Struktur in Version 2 unveränderlich“ geführt.

### §8.3.6.3 Header ungültig: zu wenige Oktette

**Spec:** §8.3.6.3, S. 50 — "The Message has less than the required number
of octets to contain a full Header. The number required is defined by the
PSM."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::from_bytes`:
`UnexpectedEof` unter 20 Oktetten)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_datagram_shorter_than_the_header`,
`crates/rtps/src/header.rs::tests::header_decode_rejects_truncated_input`

**Status:** `done`

### §8.3.6.3 Header ungültig: protocol ≠ PROTOCOL_RTPS

**Spec:** §8.3.6.3, S. 50 — "Its protocol value does not match the value of
PROTOCOL_RTPS."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::from_bytes`:
`InvalidMagic`)

**Tests:** `crates/rtps/src/header.rs::tests::header_decode_rejects_invalid_magic`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_invalid_magic`

**Status:** `done`

### §8.3.6.3 Header ungültig: Major-Version zu hoch

**Spec:** §8.3.6.3, S. 50 — "The major protocol version is larger than the
major protocol version supported by the implementation."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram_into`: Vergleich mit
`ProtocolVersion::CURRENT.major`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_major_version_above_supported`,
`crates/rtps/src/datagram.rs::tests::decode_accepts_minor_version_above_supported`

**Status:** `done`

### §8.3.6.4 Receiver.sourceGuidPrefix = Header.guidPrefix

**Spec:** §8.3.6.4, S. 50 — "Receiver.sourceGuidPrefix = Header.guidPrefix".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::init_from_header_overrides_source_fields`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.6.4 Receiver.sourceVersion = Header.version

**Spec:** §8.3.6.4, S. 50 — "Receiver.sourceVersion = Header.version".

**Repo:** `crates/rtps/src/receiver_state.rs` (`init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_header_sets_the_source_and_clears_have_timestamp`

**Status:** `done` — der Header setzt `sourceVersion`, `sourceVendorId` und `sourceGuidPrefix`.

### §8.3.6.4 Receiver.sourceVendorId = Header.vendorId

**Spec:** §8.3.6.4, S. 50 — "Receiver.sourceVendorId = Header.vendorId".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::init_from_header_overrides_source_fields`

**Status:** `done`

### §8.3.6.4 Receiver.haveTimestamp = false

**Spec:** §8.3.6.4, S. 50 — "Receiver.haveTimestamp = false".

**Repo:** `crates/rtps/src/receiver_state.rs` (`init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_header_sets_the_source_and_clears_have_timestamp`

**Status:** `done` — der Header setzt `haveTimestamp = false`.

### §8.3.6.5 Header: Logical Interpretation

**Spec:** §8.3.6.5, S. 50 — "None".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §8.3.7.1 HeaderExtension unmittelbar nach dem Header

**Spec:** §8.3.7.1, S. 50 — "This submessage, if present, shall appear
immediately after the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.2 HeaderExtension: Content

**Spec:** §8.3.7.2, S. 50 — "The elements that form the structure of the
HeaderExtension were described in 8.3.3.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; Items unter §8.3.3.2.

### §8.3.7.3 HE ungültig: nicht direkt nach dem Header

**Spec:** §8.3.7.3, S. 51 — "The submessage is present but does not
immediately follow the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`

**Status:** `done` — eine HeaderExtension hinter einer anderen Submessage ist ungültig und mit ihr der Rest der Nachricht; die Submessages davor bleiben.

### §8.3.7.3 HE ungültig: submessageLength zu klein für die Felder

**Spec:** §8.3.7.3, S. 51 — "The submessageLength in the Submessage header is
too small to fit the fields that are present as indicated by the submessage
flags."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_uextension4_body`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_md5_checksum_body`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_body_before_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.3 HE ungültig: messageLength zu klein

**Spec:** §8.3.7.3, S. 51 — "The messageLength is too small to fit the RTPS
Header and the HeaderExtension."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_too_small_for_header_and_extension_is_invalid`

**Status:** `done` — ein `messageLength` unter Header und HeaderExtension (28 Oktette) macht die Nachricht ungültig.

### §8.3.7.3 HE ungültig: parameters fehlerhaft

**Spec:** §8.3.7.3, S. 51 — "The parameters are malformed."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`
über `ParameterList::from_bytes`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_rejects_malformed_parameter_list_when_p_flag_set`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 LengthFlag: Receiver.messageLength

**Spec:** §8.3.7.4, S. 51 — "IF ( LengthFlag ) { Receiver.messageLength =
HeaderExtension.messageLength }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 TimestampFlag: rtpsSendTimestamp, rtpsReceptionTimestamp, clockSkewDetected

**Spec:** §8.3.7.4, S. 51 — "IF ( TimestampFlag ) { Receiver.rtpsSendTimestamp
= HeaderExtension.rtpsSendTimestamp; Receiver.rtpsReceptionTimestamp =
GetCurrentTime(); RECEIVER.clockSkewDetected =
CLOCK_SKEW_DETECTED(receptionTime, Receiver.rtpsSendTimestamp) }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::note_reception`, `note_clock_skew`,
`apply_info_timestamp`); `crates/rtps/src/datagram.rs` (`ReceptionClock`, `ReceiveContext`);
`crates/dcps/src/runtime.rs` (`receive_context`, `RuntimeConfig::clock_skew_threshold_s`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_reception_time_is_kept_and_checked_against_the_send_time`,
`crates/rtps/src/receiver_state.rs::tests::an_info_timestamp_is_corrected_by_the_clock_skew`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`,
`crates/dcps/tests/runtime_config_layering.rs::the_clock_skew_threshold_is_a_setting_and_off_by_default`

**Status:** `done` — jeder Empfangspfad setzt bei einer HeaderExtension mit Zeitstempel `rtpsReceptionTimestamp` (Wanduhr). Die Spec nennt für `CLOCK_SKEW_DETECTED` keine Schwelle; sie ist die Einstellung `wire.clock_skew_threshold_s` (Sekunden). Vorgabe 0 = aus: dann bleibt `clockSkewDetected` FALSE. Es gibt keine Vorgabeschwelle.

### §8.3.7.4 uExtensionFlag: Receiver.uExtension4

**Spec:** §8.3.7.4, S. 51 — "IF ( uExtensionFlag ) { Receiver.uExtension4 =
HeaderExtension.uExtension4 }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.7.4 wExtensionFlag: Receiver.wExtension8

**Spec:** §8.3.7.4, S. 51 — "IF ( wExtensionFlag ) { Receiver.wExtension8 =
HeaderExtension.wExtension8 }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.7.4 ChecksumFlags: Receiver.messageChecksum

**Spec:** §8.3.7.4, S. 51 — "IF ( ChecksumFlags != 00 ) {
Receiver.messageChecksum = HeaderExtension.messageChecksum }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 ParametersFlag: Receiver.parameters

**Spec:** §8.3.7.4, S. 51 — "IF ( ParamatersFlag) { Receiver.parameters =
HeaderExtension.parameters }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_with_parameters_sets_pl`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.5 messageLength ab Beginn des RTPS-Headers

**Spec:** §8.3.7.5, S. 51 — "The value of the messageLength shall indicate the
length of the entire RTPS Message starting from the beginning of the RTPS
Header."

**Repo:** `crates/rtps/src/datagram.rs`, `crates/rtps/src/header_extension.rs` (`validate_message_length`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_counts_from_the_start_of_the_rtps_header`

**Status:** `done` — die Länge wird ab dem ersten Oktett des RTPS-Headers gezählt und mit der empfangenen Nachricht verglichen.

### §8.3.7.5 rtpsSendTimestamp und rtpsReceptionTimestamp

**Spec:** §8.3.7.5, S. 51 — "The rtpsSendTimestamp represents the time the
RTPS message was sent. It is measured using the clock of the sending
Participant. The rtpsReceptionTimestamp represents the time when the RTPS
message was received."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung; die Zuweisungen sind
unter §8.3.7.4 TimestampFlag geführt.

### §8.3.7.5 CLOCK_SKEW_DETECTED

**Spec:** §8.3.7.5, S. 51–52 — "The function CLOCK_SKEW_DETECTED() represents
an implementation-specific function used to determine whether the sending and
receiving participant clocks are not synchronized or have too large a skew."

**Repo:** `crates/rtps/src/receiver_state.rs` (`note_reception`, `note_clock_skew`); `crates/dcps/src/runtime.rs` (`receive_context`, `wire.clock_skew_threshold_s`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_reception_time_is_kept_and_checked_against_the_send_time`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`,
`crates/dcps/tests/runtime_config_layering.rs::the_clock_skew_threshold_is_a_setting_and_off_by_default`

**Status:** `done` — die Spec nennt die Funktion implementierungsspezifisch; ZeroDDS vergleicht Empfangs- und Sendezeit gegen die Schwelle `wire.clock_skew_threshold_s` (Vorgabe 0 = aus, keine Vorgabeschwelle).

### §8.3.7.5 messageChecksum mit genulltem Prüfsummenfeld

**Spec:** §8.3.7.5, S. 52 — "It shall be computed over the entire RTPS
Message. For the purpose of computing the checksum, the value of the
messageChecksum field in the RTPS HeaderExtension shall be set to zero."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`,
`crates/rtps/src/datagram.rs::tests::a_message_with_a_wrong_checksum_is_refused_and_counted`

**Status:** `done` — `message_checksum` rechnet über die ganze Nachricht mit genulltem Feld.

### §8.3.7.5 uExtension4, wExtension8, parameters ohne Bedeutung

**Spec:** §8.3.7.5, S. 52 — "The uExtension4 and wExtension8 may be useful to
communicate additional data on the header in a future version … This version
of the specification does not define any parameters in the HeaderExtension."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung an diese Version.

### §8.3.8 Entity- und Interpreter-Submessages

**Spec:** §8.3.8, S. 52–53, Table 8.39 — Einteilung in Entity-Submessages
(Data, DataFrag, Heartbeat, HeartbeatFrag, Gap, AckNack, NackFrag) und
Interpreter-Submessages (HeaderExtension, InfoSource, InfoDestination,
InfoReply, InfoTimestamp, Pad); Gliederung jeder Beschreibung.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Submessages sind unter
§8.3.8.1–§8.3.8.13 einzeln geführt.

### §8.3.8.1 AckNack: Purpose

**Spec:** §8.3.8.1, S. 54 — "This Submessage is used to communicate the state
of a Reader to a Writer. … This Submessage can be used to do both positive and
negative acknowledgments."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe; die Inhalte folgen.

### §8.3.8.1 Table 8.40 AckNack EndiannessFlag

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::write_body`,
`AckNackSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/tests/vendor_frames.rs::fastdds_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack FinalFlag

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "FinalFlag … Indicates to the Writer
whether a response is mandatory."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::final_flag`,
`ACKNACK_FLAG_FINAL`); `crates/rtps/src/reliable_reader.rs`
(`build_acknack_datagram`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_with_final_flag`,
`crates/rtps/src/datagram.rs::tests::decode_acknack_preserves_final_flag`

**Status:** `done` — die Antwortpflicht des Writers ist unter §9.4.5.3
geführt.

### §8.3.8.1 Table 8.40 AckNack readerId

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "readerId: EntityId — Identifies the
Reader entity that acknowledges receipt of certain sequence numbers and/or
requests to receive certain sequence numbers."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack writerId

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "writerId: EntityId — Identifies the
Writer entity that is the target of the AckNack message."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::writer_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: `rt.writer_slot(ack.writer_id)`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack readerSNState

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "All sequence numbers up to the one
prior to readerSNState.base are confirmed as received by the reader. The
sequence numbers that appear in the set indicate missing sequence numbers on
the reader side."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::reader_sn_state`);
`crates/rtps/src/reliable_writer.rs` (`handle_acknack_set_at`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_updates_proxy_state`,
`crates/rtps/src/reliable_writer.rs::tests::handle_acknack_with_lower_base_leaves_requested`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack count

**Spec:** §8.3.8.1, S. 54, Table 8.40 — "count: Count — A counter that is
incremented each time a new AckNack message is sent."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`build_acknack_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::the_counts_of_acknack_and_nack_frag_rise_with_each_message`

**Status:** `done` — aufeinanderfolgende ACKNACKs tragen die Zähler 1, 2, 3.

### §8.3.8.1 AckNack ungültig: submessageLength zu klein

**Spec:** §8.3.8.1, S. 54 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.1 AckNack ungültig: readerSNState ungültig

**Spec:** §8.3.8.1, S. 54 — "readerSNState is invalid (as defined in Section
8.3.5.5)."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::tests::seqnumset_rejects_num_bits_above_256`,
`crates/rtps/tests/vendor_frames.rs::the_empty_acknack_of_opendds_and_rti_is_invalid_by_default`,
`crates/rtps/tests/vendor_frames.rs::opendds_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`,
`crates/rtps/src/submessages.rs::validity_tests::the_empty_set_with_base_zero_is_accepted_only_on_request`

**Status:** `done` — `numBits > 256` und `bitmapBase < 1` werden abgewiesen, mit dem Rest
der Nachricht.
Vendor-Form: OpenDDS 3.34.0 und RTI Connext 7.7.0 senden als ACKNACK eines Readers
ohne Samples `readerSNState` `0/0:` (bitmapBase 0, numBits 0; gemessen in
`tools/wire-capture/rtps-sessions`: 40 und 8 ACKNACK). Standard: ungültig. Die
Einstellung `wire.accept_empty_acknack_base_zero` (Env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, Standard `false`) nimmt genau diese
leere Menge in einem ACKNACK an, prozessweit.

### §8.3.8.1 AckNack: writerGUID = {destGuidPrefix, writerId}

**Spec:** §8.3.8.1, S. 54 — "writerGUID = { Receiver.destGuidPrefix,
AckNack.writerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
Zuordnung über `ack.writer_id`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.1 AckNack: readerGUID = {sourceGuidPrefix, readerId}

**Spec:** §8.3.8.1, S. 55 — "readerGUID = { Receiver.sourceGuidPrefix,
AckNack.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`Guid::new(src_prefix, ack.reader_id)`); `crates/rtps/src/reliable_writer.rs`
(`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_unknown_source_counts_but_noops`,
`crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.3.8.1 AckNack: positive Bestätigung bis base − 1

**Spec:** §8.3.8.1, S. 55 — "The Submessage acknowledges all sequence numbers
up to and including the one just before the lowest sequence number in the
SequenceNumberSet (that is readerSNState.base -1)."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`:
`acked_changes_set(base)`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::acked_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`,
`crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_follows_the_slowest_reader_proxy`

**Status:** `done`

### §8.3.8.1 AckNack: negative Bestätigung der Menge

**Spec:** §8.3.8.1, S. 55 — "The Submessage negatively-acknowledges (requests)
the sequence numbers that appear explicitly in the set."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`:
`requested_changes_set`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.3.8.2 Data: Purpose

**Spec:** §8.3.8.2, S. 55 — "The Submessage notifies the RTPS Reader of a
change to a data-object belonging to the RTPS Writer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.2 Table 8.41 Data EndiannessFlag

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::write_body`,
`DataSubmessage::read_body_with_flags`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_be_with_empty_payload`,
`crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data InlineQosFlag

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "InlineQosFlag … Indicates to the
Reader the presence of a ParameterList containing QoS parameters that should
be used to interpret the message."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_INLINE_QOS`,
`DataSubmessage::inline_qos`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/src/submessages.rs::tests::a_pid_ignore_in_the_inline_qos_does_not_move_the_payload`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data DataFlag

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "DataFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_DATA`,
`DataSubmessage::data_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/src/reliable_reader.rs::tests::alive_data_yields_alive_changekind`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data KeyFlag

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "KeyFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
key of the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_KEY`,
`DataSubmessage::key_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_key_flag_roundtrip`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_with_a_key_sets_k_and_carries_it`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data NonStandardPayloadFlag

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "NonStandardPayloadFlag … Indicates to
the Reader that the serializedPayload submessage element is not formatted
according to Section 10."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_NON_STANDARD`,
`DataSubmessage::non_standard_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data readerId

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "readerId: EntityId — Identifies the
RTPS Reader entity that is being informed of the change to the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::reader_id`)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_drops_data_addressed_to_other_reader`,
`crates/rtps/src/reader.rs::tests::reader_delivers_data_addressed_to_us`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data writerId

**Spec:** §8.3.8.2, S. 55, Table 8.41 — "writerId: EntityId — Identifies the
RTPS Writer entity that made the change to the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::writer_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::mismatched_writer_id_is_counted`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data writerSN

**Spec:** §8.3.8.2, S. 56, Table 8.41 — "writerSN: SequenceNumber — Uniquely
identifies the change and the relative order for all changes made by the
RTPS Writer … Each change gets a consecutive sequence number."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::writer_sn`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/writer.rs::tests::writer_three_writes_have_increasing_sn_in_decoded`,
`crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data inlineQos

**Spec:** §8.3.8.2, S. 56, Table 8.41 — "inlineQos: ParameterList — Present
only if the InlineQosFlag is set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`:
ParameterList nur bei Q)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_survives_data_submessage_roundtrip`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data serializedPayload

**Spec:** §8.3.8.2, S. 56, Table 8.41 — "serializedPayload: SerializedPayload
— Present only if either the DataFlag or the KeyFlag are set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::serialized_payload`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::lifecycle_without_a_key_clears_k_and_sends_no_payload`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_empty_payload_decodes`

**Status:** `done`

### §8.3.8.2 Data ungültig: submessageLength zu klein

**Spec:** §8.3.8.2, S. 56 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.2 Data ungültig: writerSN ≤ 0 oder SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.8.2, S. 56 — "writerSN.value is not strictly positive (1, 2,
...) or is SEQUENCENUMBER_UNKNOWN."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — `writerSN` unter 1 und SEQUENCENUMBER_UNKNOWN ({-1, 0}) werden
abgewiesen; die Submessage ist ungültig und mit ihr der Rest der Nachricht.

### §8.3.8.2 Data ungültig: inlineQos ungültig

**Spec:** §8.3.8.2, S. 56 — "inlineQos is invalid."

**Repo:** `crates/rtps/src/parameter_list.rs`
(`ParameterList::from_bytes_with_consumed`); `crates/rtps/src/datagram.rs`
(`validate_must_understand_in_data_pipeline`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::decode_rejects_truncated_value`,
`crates/rtps/src/datagram.rs::tests::a_data_with_an_unknown_must_understand_pid_is_skipped`,
`crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`

**Status:** `done` — eine fehlerhafte ParameterList macht die DATA ungültig und
verwirft den Rest der Nachricht, die Submessages davor bleiben (§8.3.4.1 Regel
6); eine unbekannte Must-Understand-PID überspringt nur die DATA (Table 9.6).

### §8.3.8.2 Data: DataFlag ohne KeyFlag ist der Wert

**Spec:** §8.3.8.2, S. 56 — "If the DataFlag is set and the KeyFlag is not
set, the serializedPayload element is interpreted as the value of the
data-object."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`classify_change_kind`,
`handle_data_into`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::alive_data_yields_alive_changekind`,
`crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`

**Status:** `done`

### §8.3.8.2 Data: KeyFlag ohne DataFlag ist der Schlüssel

**Spec:** §8.3.8.2, S. 56 — "If the KeyFlag is set and the DataFlag is not
set, the serializedPayload element is interpreted as the value of the key
that identifies the registered instance of the data-object."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`:
schlüsselinhaltige DATA ohne Lebenszyklus als Registrierung)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::key_only_alive_registration_is_acked_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::dispose_data_yields_not_alive_disposed`

**Status:** `done`

### §8.3.8.2 Data: Inline-QoS überschreibt die Writer-QoS

**Spec:** §8.3.8.2, S. 56 — "If the InlineQosFlag is set, the inlineQos
element contains QoS values that override those of the RTPS Writer and should
be used to process the update. For a complete list of possible in-line QoS
parameters, see Table 8.85."

**Repo:** `crates/rtps/src/inline_qos.rs` (`InlineQosOverride`); `crates/rtps/src/reliable_reader.rs` (`DeliveredSample::inline_qos_override`);
`crates/dcps/src/runtime.rs` (`sample_strength`, `sample_lifespan_nanos`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_data_hands_on_the_in_line_strength_and_lifespan`,
`crates/dcps/src/runtime.rs::tests::the_in_line_strength_and_lifespan_of_a_sample_override_the_writers`

**Status:** `done` — OWNERSHIP_STRENGTH und LIFESPAN, die ein DATA in-line trägt, gelten für diese Probe statt der angekündigten Werte des Writers. Die übrigen Policies von Table 8.85 wirken nicht auf eine einzelne Probe (sie gehen in den Abgleich beim Match ein: DURABILITY, RELIABILITY, PRESENTATION, PARTITION, DEADLINE, LATENCY_BUDGET, DESTINATION_ORDER, LIVELINESS), TRANSPORT_PRIORITY ist Sache des Senders.

### §8.3.8.2 Data: NonStandardPayloadFlag nur informativ

**Spec:** §8.3.8.2, S. 56 — "This flag is informational. It indicates that
the SerializedPayload has been transformed as described in another
specification. For example, this flag should be set when the
SerializedPayload is transformed as described in the DDS-Security
specification."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::non_standard_flag`
wird gelesen, nicht interpretiert)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done` — ob ZeroDDS das Flag bei DDS-Security-transformierten
Nutzdaten setzt, ist nicht geprüft.

### §8.3.8.2 Data: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.2, S. 56 — "writerGUID = { Receiver.sourceGuidPrefix,
Data.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`proxy_index_by_writer`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: `src_prefix` aus
`receiver_walk`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`,
`crates/discovery/tests/opensplice_infosrc_regression.rs::opensplice_bundled_infosrc_datagrams_attribute_to_the_real_source`

**Status:** `done`

### §8.3.8.2 Data: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.2, S. 56 — "readerGUID = { Receiver.destGuidPrefix,
Data.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`rt.reader_slot(d.reader_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.2 Data: readerId ENTITYID_UNKNOWN

**Spec:** §8.3.8.2, S. 56 — "The Data.readerId can be ENTITYID_UNKNOWN, in
which case the Data applies to all Readers of that writerGUID within the
Participant identified by the GuidPrefix_t Receiver.destGuidPrefix."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: bei
`EntityId::UNKNOWN` an alle Reader-Slots, Filter über den Writer-Proxy)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_accepts_unknown_wildcard_target`

**Status:** `done` — die Einschränkung auf den Participant von
`destGuidPrefix` hängt am Item unter §8.3.8.8.

### §8.3.8.3 DataFrag: Purpose

**Spec:** §8.3.8.3, S. 57 — "The DataFrag Submessage extends the Data
Submessage by enabling the serializedData to be fragmented and sent as
multiple DataFrag Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.3 Table 8.42 DataFrag EndiannessFlag

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::write_body`,
`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag InlineQosFlag

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "InlineQosFlag … Indicates to the
Reader the presence of a ParameterList containing QoS parameters".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_INLINE_QOS`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`,
`crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag NonStandardPayloadFlag

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "NonStandardPayloadFlag … Indicates to
the Reader that the serializedPayload submessage element is not formatted
according to Section 10."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_NON_STANDARD`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag KeyFlag

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "KeyFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
key of the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_KEY`,
`DataFragSubmessage::key_flag`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_frag_preserves_flags_and_payload`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag readerId

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "readerId: EntityId — Identifies the
RTPS Reader entity that is being informed of the change".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag writerId

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "writerId: EntityId — Identifies the
RTPS Writer entity that made the change".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::writer_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag writerSN

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "writerSN: SequenceNumber — Uniquely
identifies the change and the relative order for all changes made by the
RTPS Writer".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::writer_sn`);
`crates/rtps/src/fragment_assembler.rs` (Puffer je `writer_sn`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::incomplete_sns_enumerates_in_order`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentStartingNum

**Spec:** §8.3.8.3, S. 57, Table 8.42 — "fragmentStartingNum: FragmentNumber —
Indicates the starting fragment for the series of fragments in serializedData.
Fragment numbering starts with number 1."

**Repo:** `crates/rtps/src/submessages.rs`
(`DataFragSubmessage::fragment_starting_num`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragment_index_zero_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::two_fragments_complete_in_order`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentsInSubmessage

**Spec:** §8.3.8.3, S. 58, Table 8.42 — "fragmentsInSubmessage: ushort — The
number of consecutive fragments contained in this Submessage, starting at
fragmentStartingNum."

**Repo:** `crates/rtps/src/submessages.rs`
(`DataFragSubmessage::fragments_in_submessage`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_all_full`,
`crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_including_last_with_tail`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag dataSize

**Spec:** §8.3.8.3, S. 58, Table 8.42 — "dataSize: ulong — The total size in
bytes of the original data before fragmentation."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::sample_size`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::inconsistent_sample_size_drops_fragment`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentSize

**Spec:** §8.3.8.3, S. 58, Table 8.42 — "fragmentSize: ushort — The size of an
individual fragment in bytes. The maximum fragment size equals 64K."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::fragment_size`,
Typ `u16`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragment_size_zero_dropped`,
`crates/rtps/src/submessages.rs::tests::data_frag_last_fragment_shorter_than_fragment_size`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag inlineQos

**Spec:** §8.3.8.3, S. 58, Table 8.42 — "inlineQos: ParameterList — Present
only if the InlineQosFlag is set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::inline_qos`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/rtps/src/submessages.rs::tests::data_frag_ignored_parameter_does_not_become_payload`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag serializedPayload

**Spec:** §8.3.8.3, S. 58, Table 8.42 — "A consecutive series of fragments,
starting at fragmentStartingNum for a total of fragmentsInSubmessage. …
If the KeyFlag is set, then it contains a consecutive set of fragments of the
key".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::serialized_payload`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::last_fragment_shorter_than_fragment_size`

**Status:** `done`

### §8.3.8.3 DataFrag ungültig: submessageLength zu klein

**Spec:** §8.3.8.3, S. 58 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.3 DataFrag ungültig: writerSN ≤ 0 oder SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.8.3, S. 58 — "writerSN.value is not strictly positive (1, 2,
...) or is SEQUENCENUMBER_UNKNOWN."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`

**Status:** `done` — wie bei DATA.

### §8.3.8.3 DataFrag ungültig: fragmentStartingNum außerhalb

**Spec:** §8.3.8.3, S. 58 — "fragmentStartingNum.value is not strictly
positive (1, 2, ...) or exceeds the total number of fragments".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`FragmentIndexZero`, `FragmentIndexOutOfRange`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/fragment_assembler.rs::tests::fragment_index_zero_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::fragment_index_out_of_range_dropped`

**Status:** `done` — `fragmentStartingNum` unter 1 oder über der Zahl der Fragmente
(`dataSize / fragmentSize`, aufgerundet) macht die Submessage und den Rest der
Nachricht ungültig.

### §8.3.8.3 DataFrag ungültig: fragmentSize > dataSize

**Spec:** §8.3.8.3, S. 58 — "fragmentSize exceeds dataSize."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`

**Status:** `done` — `fragmentSize` über `dataSize` und `fragmentSize` 0 werden abgewiesen.

### §8.3.8.3 DataFrag ungültig: serializedData zu groß

**Spec:** §8.3.8.3, S. 58 — "The size of serializedData exceeds
fragmentsInSubmessage * fragmentSize."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`PayloadSizeMismatch`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/fragment_assembler.rs::tests::payload_size_mismatch_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_payload_size_mismatch_rejected`

**Status:** `done` — mehr Oktette als `fragmentsInSubmessage * fragmentSize` machen die
Submessage ungültig. Das letzte Fragment des Samples ist so lang, wie das Sample
es lässt, auf 4 Oktette aufgerundet: Cyclone DDS und Fast DDS füllen es auf die
Ausrichtung der Submessage auf (72 Oktette für 71, gemessen).

### §8.3.8.3 DataFrag ungültig: inlineQos ungültig

**Spec:** §8.3.8.3, S. 58 — "inlineQos is invalid."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_with_an_invalid_inline_qos_is_invalid`

**Status:** `done` — eine Parameterliste, die über den Rumpf hinausläuft oder keinen Sentinel hat, macht die Submessage ungültig.

### §8.3.8.3 DataFrag: Zusammensetzen, Offset und Fragmentanzahl

**Spec:** §8.3.8.3, S. 58–59 — Offset "(fragmentStartingNum - 1) *
fragmentSize"; "The data is fully re-assembled when all fragments have been
received. The total number of fragments to expect equals: (dataSize /
fragmentSize) + ((dataSize % fragmentSize) ? 1 : 0)".

**Repo:** `crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`byte_start`, `total_fragments = sample_size.div_ceil(fragment_size)`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::missing_fragments_enumerates_gaps`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_10_percent_loss`

**Status:** `done`

### §8.3.8.3 DataFrag: zusammengesetzt wie Data zu interpretieren

**Spec:** §8.3.8.3, S. 58 — "Once the serializedData is re-assembled by the
RTPS Reader, the interpretation of the DataFrag Submessages is identical to
that of the Data Submessage."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_fragmented_dispose_is_delivered_as_a_dispose`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.3.8.3 DataFrag: Fragmentgröße nach kleinstem Transport

**Spec:** §8.3.8.3, S. 59 — "An RTPS Writer will select fragmentSize based on
the smallest message size supported across all underlying transports. …
the RTPS Writer can pack multiple fragments into a single DataFrag Submessage
or may even send a regular Data Submessage if fragmentation is no longer
required."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`fragment_size_for_message_size`, `fragments_per_message`, `ReliableWriterConfig::fragment_size`);
`crates/transport/src/lib.rs` (`Transport::max_message_size`); `crates/dcps/src/runtime.rs` (Writer-Anlage)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_under_fragment_size_produces_single_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`,
`crates/dcps/src/runtime.rs::tests::a_writers_fragment_size_follows_the_message_size_of_its_transport`

**Status:** `done` — die Fragmentgröße eines Writers folgt der kleinsten Nachrichtengröße seiner Transporte (`Transport::max_message_size`, Item unter §8.4.14.1); mehrere Fragmente gehen in ein DATA_FRAG, wo die Nachricht sie trägt; ein Pfad, der ohne Fragmentierung auskommt, erhält ein DATA.

### §8.3.8.3 DataFrag: Inline-QoS nur im ersten Fragment nötig

**Spec:** §8.3.8.3, S. 59 — "When sending inlineQos with DataFrag
Submessages, it is only required to send the inlineQos with the first
DataFrag Submessage for a given Writer sequence number."

**Repo:** `crates/rtps/src/reliable_writer.rs` (Inline-QoS mit PID_KEY_HASH an
den DATA_FRAG eines Samples)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`

**Status:** `done` — die Inline-QoS eines Coherent-Set-Mitglieds auf dem
ersten DATA_FRAG ist unter §8.7.6 geführt.

### §8.3.8.4 Gap: Purpose und gefilterte Changes

**Spec:** §8.3.8.4, S. 59 — "indicates to the RTPS Reader that a set of
changes in the RTPS Writer HistoryCache are not available to one or more RTPS
Reader endpoints and will not be sent to them. … Filtered changes should not
be treated as samples lost by the RTPS Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`:
`irrelevant_change_set`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::gap_marks_irrelevant`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_counts_what_was_neither_received_nor_irrelevant`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap EndiannessFlag

**Spec:** §8.3.8.4, S. 59, Table 8.43 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::write_body`,
`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap GroupInfoFlag

**Spec:** §8.3.8.4, S. 59, Table 8.43 — "GroupInfoFlag … Indicates the
presence of additional information about the group of writers (Writer Group)
the sender belongs to."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap FilteredCountFlag

**Spec:** §8.3.8.4, S. 59, Table 8.43 — "FilteredCountFlag … Indicates the
presence of the filteredCount submessage element."

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_FILTERED_COUNT`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap readerId

**Spec:** §8.3.8.4, S. 59, Table 8.43 — "The readerId may be set to
ENTITYID_UNKNOWN to indicate the Gap message applies to all Readers within
the receiving Participant that are matched with the Writer specified by the
writerId."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::reader_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: Verteilung bei
`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap writerId

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "writerId: EntityId — Identifies the
Writer Entity to which the not-available changes apply."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::writer_id`);
`crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapStart

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "gapStart: SequenceNumber — Identifies
the first sequence number in the interval of sequence numbers corresponding
to the set of not-available changes."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::gap_start`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_before_any_heartbeat_lets_the_next_data_through`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapList

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "gapList: SequenceNumberSet — …
(1) Identifies the last sequence number in the interval … (2) Identifies an
additional list of sequence numbers corresponding to not-available changes."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::gap_list`);
`crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapStartGSN

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "gapStartGSN: SequenceNumber —
Present only if the GroupInfoFlag is set in the header."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapEndGSN

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "gapEndGSN: SequenceNumber — Present
only if the GroupInfoFlag is set in the header. … It shall be greater than or
equal to the group sequence number corresponding to the sample identified by
gapList.bitmapBase."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap filteredCount

**Spec:** §8.3.8.4, S. 60, Table 8.43 — "filteredCount: ChangeCount — Present
only if the FilteredCountFlag is set in the header. Indicates the number of
changes corresponding to the set of sequence numbers in the Gap that are still
present in the Writer cache but are being filtered for the RTPS Reader."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Gap ungültig: submessageLength zu klein

**Spec:** §8.3.8.4, S. 60 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.4 Gap ungültig: gapStart ≤ 0

**Spec:** §8.3.8.4, S. 60 — "gapStart is zero or negative."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`

**Status:** `done` — `gapStart` unter 1 wird abgewiesen.

### §8.3.8.4 Gap ungültig: gapList ungültig

**Spec:** §8.3.8.4, S. 60 — "gapList is invalid (as defined in 8.3.5.5)."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::tests::seqnumset_rejects_num_bits_above_256`

**Status:** `done` — `numBits > 256` und `bitmapBase < 1` werden abgewiesen.

### §8.3.8.4 Gap ungültig: Group-Felder

**Spec:** §8.3.8.4, S. 60 — "If GroupInfoFlag is set and: gapStartGSN.value is
zero or negative; gapEndGSN.value is zero or negative; gapEndGSN.value <
gapStartGSN.value-1".

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_truncated_group_info`

**Status:** `done`

### §8.3.8.4 Gap::sequence_number_set

**Spec:** §8.3.8.4, S. 60–61 — "1. All sequence numbers in the range gapStart
<= sequence_number <= gapList.base -1; 2. All the sequence numbers that appear
explicitly listed in the gapList."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`

**Status:** `done`

### §8.3.8.4 Gap: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.4, S. 61 — "writerGUID = { Receiver.sourceGuidPrefix,
Gap.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`:
`proxy_index_by_writer(Guid::new(source_prefix, gap.writer_id))`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`

**Status:** `done`

### §8.3.8.4 Gap: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.4, S. 61 — "readerGUID = { Receiver.destGuidPrefix,
Gap.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.4 Gap: GroupInfoFlag-Inhalt

**Spec:** §8.3.8.4, S. 61 — "The Writer sets the GroupInfoFlag to indicate the
presence of the gapStartGSN and gapEndGSN elements. … The gapEndGSN can extend
past the Group Sequence Number that corresponds to gapList.bitmapBase".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`gap_group_info`, `note_gsn`, `GapGroupInfo`); `crates/rtps/src/submessages.rs`; `crates/dcps/src/runtime.rs` (`note_writer_group_progress`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_gap_of_a_group_writer_carries_the_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/dcps/src/group_order.rs::tests::a_gap_rules_the_number_out_for_its_writer`

**Status:** `done` — der Writer eines GROUP-Publishers sendet GAPs mit gesetztem GroupInfoFlag: `gapStartGSN` ist die Group Sequence Number der weggenommenen Änderung, `gapEndGSN` dieselbe (Bereich aus einer Nummer; gültig nach §8.3.8.4); die Nummer stammt aus dem Cache oder aus einer begrenzten Merkliste (4096) für Änderungen, die den Cache verlassen haben. Ein Writer ohne Gruppe sendet kein Flag. Der Reader rechnet beide Nummern in die Commit-Regel ein (`DcpsRuntime::note_writer_group_progress`).

### §8.3.8.4 Gap: Inhalt von filteredCount

**Spec:** §8.3.8.4, S. 61 — "The filteredCount, if present, shall include:
Changes filtered due to the writer-side application of a content-filtered-topic
… a time filter … Changes that are written in ways that intend to exclude
delivery to the Reader. … The filteredCount shall not include changes that are
no longer in the RTPS Writer HistoryCache".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — bedingte Regel für einen Writer, der
`filteredCount` sendet; ZeroDDS filtert nicht schreiberseitig und sendet das
Feld nicht (`append_gap` setzt `filtered_count: None`).

### §8.3.8.5 HeaderExtension

**Spec:** §8.3.8.5, S. 61 — "This submessage is logically part of the header
and it is defined in 8.3.7."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; Items unter §8.3.7.

### §8.3.8.6 Heartbeat: Purpose

**Spec:** §8.3.8.6, S. 61 — "This message is sent from an RTPS Writer to an
RTPS Reader to communicate the sequence numbers of changes that the Writer has
available."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.6 Table 8.44 Heartbeat EndiannessFlag

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::write_body`,
`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_heartbeat_decodes`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat FinalFlag

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "FinalFlag … Indicates whether the
Reader is required to respond to the Heartbeat or if it is just an advisory
heartbeat."

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_FINAL`,
`HeartbeatSubmessage::final_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_no_final_flag_when_disabled`,
`crates/rtps/src/reliable_writer.rs::tests::periodic_heartbeat_has_final_flag_unset`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat LivelinessFlag

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "LivelinessFlag … Indicates that the
DDS DataWriter associated with the RTPS Writer of the message has manually
asserted its LIVELINESS."

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_LIVELINESS`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::liveliness_heartbeat`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`crates/rtps/src/reliable_writer.rs::tests::liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat GroupInfoFlag

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "GroupInfoFlag … Indicates the
presence of additional information about the group of writers (Writer Group)
the sender belongs to."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat readerId

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "readerId … Can be set to
ENTITYID_UNKNOWN to indicate all readers for the writer that sent the
message."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::reader_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: Verteilung bei
`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::final_heartbeat_from_unknown_to_data_writer_requests_the_missing_sn`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat writerId

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "writerId: EntityId — Identifies the
Writer Entity to which the range of sequence numbers applies."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::writer_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::unknown_writer_id_in_heartbeat_counts_not_crashes`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat firstSN

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "If samples are available in the
Writer, identifies the first (lowest) sequence number that is available in the
Writer. If no samples are available in the Writer, identifies the lowest
sequence number that is yet to be written by the Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`,
`crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat lastSN

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "lastSN: SequenceNumber — Identifies
the last (highest) sequence number that the Writer has ever written."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`last_written_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat count

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "count: Count — A counter that is
incremented each time a new Heartbeat message is sent."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`heartbeat_count.wrapping_add(1)`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat currentGSN

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "currentGSN … Identifies the last
(highest) group sequence number written by any DataWriter in the Writer's
Group at the time that the HeartBeat was sent."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat firstGSN

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "firstGSN … Identifies the group
sequence number corresponding to the sample identified by sequence number
firstSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat lastGSN

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "lastGSN … Identifies the group
sequence number corresponding to the sample identified by sequence number
lastSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat writerSet

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "writerSet: GroupDigest … Identifies
the subset of Writers that belong to the Writer's Group at the time the sample
with currentGSN was written."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatGroupInfo`, `HeartbeatSubmessage::write_body`, `read_body`); `crates/rtps/src/dialect.rs` (`HeartbeatGroupDigests`); `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`); `crates/dcps/src/group_order.rs`; `wire.omit_heartbeat_group_digests`

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`

**Status:** `done` — mit G=1 sendet der Writer nach lastGSN writerSet als GroupDigest (vier Oktette, §9.4.2.19; die ersten vier Oktette des MD5 über das EntityIdSet_t, sortiert, §9.3.2.5; leere Gruppe: Null). Der Reader liest beide, wenn die submessageLength Platz lässt, und nimmt ihr Fehlen an (Bild §9.4.5.7); die Commit-Regel §8.7.5 vergleicht sie mit den entdeckten Writern. `wire.omit_heartbeat_group_digests` lässt sie beim Senden weg. Widerspruch PIM/PSM: siehe „Widersprüche innerhalb der Spec“.

### §8.3.8.6 Table 8.44 Heartbeat secureWriterSet

**Spec:** §8.3.8.6, S. 62, Table 8.44 — "secureWriterSet: GroupDigest …
Reserved for use by the DDS-Security Specification."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatGroupInfo`, `HeartbeatSubmessage::write_body`, `read_body`); `crates/rtps/src/dialect.rs` (`HeartbeatGroupDigests`); `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`); `crates/dcps/src/group_order.rs`; `wire.omit_heartbeat_group_digests`

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`

**Status:** `done` — mit G=1 sendet der Writer nach lastGSN secureWriterSet als GroupDigest (vier Oktette, §9.4.2.19; die ersten vier Oktette des MD5 über das EntityIdSet_t, sortiert, §9.3.2.5; leere Gruppe: Null; secureWriterSet ist für DDS-Security reserviert und wird als Null gesendet). Der Reader liest beide, wenn die submessageLength Platz lässt, und nimmt ihr Fehlen an (Bild §9.4.5.7); die Commit-Regel §8.7.5 vergleicht sie mit den entdeckten Writern. `wire.omit_heartbeat_group_digests` lässt sie beim Senden weg. Widerspruch PIM/PSM: siehe „Widersprüche innerhalb der Spec“.

### §8.3.8.6 Heartbeat Beispiel 1: nie geschrieben

**Spec:** §8.3.8.6, S. 63 — "A Writer that has never written any samples
before sending a Heartbeat will send a Heartbeat with firstSN.value = 1,
lastSN.value = 0."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_empty_writer_heartbeats_a_reader_until_it_answers`

**Status:** `done`

### §8.3.8.6 Heartbeat Beispiel 2: ein Sample im Cache

**Spec:** §8.3.8.6, S. 63 — "A Writer that has only one sample in its cache
with sequence number SN will send a Heartbeat with firstSN.value =
lastSN.value = SN."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done`

### §8.3.8.6 Heartbeat Beispiel 3: Teil der Historie im Cache

**Spec:** §8.3.8.6, S. 63 — "A Writer that has written 10 samples and still
has the last 5 samples in its cache will send a Heartbeat with firstSN.value
= 6, lastSN.value = 10."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`

**Status:** `done`

### §8.3.8.6 Heartbeat Beispiel 4: Cache leer nach Schreiben

**Spec:** §8.3.8.6, S. 63 — "A Writer that has written 10 samples before
sending a Heartbeat but does not have any samples available at the time of the
Heartbeat will send a Heartbeat with firstSN.value = 11, lastSN.value = 10."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`first_sn.min(last + 1)`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Heartbeat ungültig: submessageLength zu klein

**Spec:** §8.3.8.6, S. 63 — "submessageLength in the Submessage header is too
small".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.6 Heartbeat ungültig: firstSN ≤ 0

**Spec:** §8.3.8.6, S. 63 — "firstSN.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — `firstSN` unter 1 wird abgewiesen.

### §8.3.8.6 Heartbeat ungültig: lastSN < 0

**Spec:** §8.3.8.6, S. 63 — "lastSN.value is negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — ein negatives `lastSN` wird abgewiesen.

### §8.3.8.6 Heartbeat ungültig: lastSN < firstSN − 1

**Spec:** §8.3.8.6, S. 63 — "lastSN.value < firstSN.value - 1".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — `lastSN < firstSN - 1` wird abgewiesen; `lastSN = firstSN - 1` (leere
Historie) ist gültig.

### §8.3.8.6 Heartbeat ungültig: Group-Felder

**Spec:** §8.3.8.6, S. 63 — sechs Bedingungen für currentGSN, firstGSN und
lastGSN bei gesetztem GroupInfoFlag.

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_current_gsn_past_the_writers_last_is_accepted`,
`crates/rtps/src/submessages.rs::tests::heartbeat_decode_rejects_truncated_group_info`

**Status:** `done` — fünf der sechs Bedingungen werden geprüft. Die sechste, `currentGSN > lastGSN`, widerspricht der Definition von currentGSN (S. 62–63: die Nummer des zuletzt geschriebenen Samples, das bei lastGSN noch nicht eingerechnet sein kann) und wird nicht angewandt; der Test `a_current_gsn_past_the_writers_last_is_accepted` hält das fest. Die gewählte Lesart steht unter „Widersprüche innerhalb der Spec“. Cyclone DDS, Fast DDS und OpenDDS senden keine Group-HEARTBEATs; erwartet: keine Wirkung.

### §8.3.8.6 Heartbeat: Verfügbarkeit melden

**Spec:** §8.3.8.6, S. 63 — "It informs the Reader of the sequence numbers
that are available in the writer's HistoryCache so that the Reader may
request (using an AckNack) any that it has missed."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`:
`update_from_heartbeat`); `crates/rtps/src/writer_proxy.rs`

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::heartbeat_sets_available_range`,
`crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`

**Status:** `done`

### §8.3.8.6 Heartbeat ohne FinalFlag verlangt AckNack

**Spec:** §8.3.8.6, S. 63 — "if the FinalFlag is not set, then the Reader must
send an AckNack message indicating which CacheChange changes it has received,
even if the AckNack indicates it has received all CacheChange changes in the
writer's HistoryCache."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_without_the_final_flag_is_answered_when_everything_is_received`

**Status:** `done` — ein HEARTBEAT ohne FinalFlag löst einen ACKNACK aus, auch wenn alles empfangen ist (`bitmapBase` hinter der letzten Nummer, nichts fehlt), mit und ohne LivelinessFlag.

### §8.3.8.6 Heartbeat LivelinessFlag erneuert die manuelle Lease

**Spec:** §8.3.8.6, S. 63 — "The RTPS Reader should therefore renew the manual
liveliness lease of the corresponding remote DDS DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`liveliness.writer_asserted` bei `h.liveliness_flag`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`

**Status:** `done`

### §8.3.8.6 Heartbeat: GroupInfoFlag-Inhalt

**Spec:** §8.3.8.6, S. 64 — "The Writer sets the GroupInfoFlag to indicate the
presence of the currentGSN, firstGSN, lastGSN, writerSet, and secureWriterSet
elements."

**Repo:** `crates/rtps/src/submessages.rs`; `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done` — currentGSN, firstGSN, lastGSN, writerSet und secureWriterSet werden gesendet und gelesen (siehe die Items writerSet und secureWriterSet).

### §8.3.8.6 Heartbeat: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.6, S. 64 — "writerGUID = { Receiver.sourceGuidPrefix,
Heartbeat.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.3.8.6 Heartbeat: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.6, S. 64 — "readerGUID = { Receiver.destGuidPrefix,
Heartbeat.readerId }. The Heartbeat.readerId can be ENTITYID_UNKNOWN, in which
case the Heartbeat applies to all Readers of that writerGUID within the
Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.7 HeartbeatFrag: Purpose

**Spec:** §8.3.8.7, S. 64 — "When fragmenting data and until all fragments are
available, the HeartbeatFrag Submessage is sent from an RTPS Writer to an RTPS
Reader to communicate which fragments the Writer has available."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe; ZeroDDS sendet ein Sample
erst, wenn alle Fragmente vorliegen.

### §8.3.8.7 Table 8.45 HeartbeatFrag EndiannessFlag

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::write_body`,
`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_be`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag readerId

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "readerId … Can be set to
ENTITYID_UNKNOWN to indicate all readers for the writer that sent the
message."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag writerId

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "writerId: EntityId — Identifies the
Writer Entity that sent the Submessage."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::writer_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag writerSN

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "writerSN: SequenceNumber — Identifies
the sequence number of the data change for which fragments are available."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::writer_sn`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag lastFragmentNum

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "lastFragmentNum: FragmentNumber — All
fragments up to and including this last (highest) fragment are available on
the Writer for the change identified by writerSN."

**Repo:** `crates/rtps/src/submessages.rs`
(`HeartbeatFragSubmessage::last_fragment_num`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag count

**Spec:** §8.3.8.7, S. 64, Table 8.45 — "count: Count — A counter that is
incremented each time a new HeartbeatFrag message is sent."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done` — ZeroDDS sendet keine HEARTBEAT_FRAG; das Feld wird
kodiert und gelesen.

### §8.3.8.7 HeartbeatFrag ungültig: submessageLength zu klein

**Spec:** §8.3.8.7, S. 64 — "submessageLength in the Submessage header is too
small".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.7 HeartbeatFrag ungültig: writerSN ≤ 0

**Spec:** §8.3.8.7, S. 64 — "writerSN.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — `writerSN` unter 1 wird abgewiesen.

### §8.3.8.7 HeartbeatFrag ungültig: lastFragmentNum ≤ 0

**Spec:** §8.3.8.7, S. 64 — "lastFragmentNum.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — `lastFragmentNum` unter 1 wird abgewiesen.

### §8.3.8.7 HeartbeatFrag: Reader antwortet mit NackFrag

**Spec:** §8.3.8.7, S. 65 — "The RTPS Reader will respond by sending a
NackFrag message, but only if it is missing any of the available fragments."

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`ReliableReader::handle_heartbeat_frag`), `crates/dcps/src/runtime.rs`
(`handle_user_datagram_from`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done`

### §8.3.8.7 HeartbeatFrag: writerGUID/readerGUID

**Spec:** §8.3.8.7, S. 65 — "writerGUID = { Receiver.sourceGuidPrefix,
HeartbeatFrag.writerId } … readerGUID = { Receiver.destGuidPrefix,
HeartbeatFrag.readerId } … can be ENTITYID_UNKNOWN".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: readerId
oder ENTITYID_UNKNOWN), `crates/rtps/src/reliable_reader.rs`
(`proxy_index_by_writer`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done`

### §8.3.8.8 InfoDestination: Purpose

**Spec:** §8.3.8.8, S. 65 — "This message is sent from an RTPS Writer to an
RTPS Reader to modify the GuidPrefix used to interpret the Reader entityIds
appearing in the Submessages that follow it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.8 Table 8.46 InfoDestination EndiannessFlag

**Spec:** §8.3.8.8, S. 65, Table 8.46 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`
für `InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_decodes_to_typed_destination_prefix`

**Status:** `done`

### §8.3.8.8 Table 8.46 InfoDestination guidPrefix

**Spec:** §8.3.8.8, S. 65, Table 8.46 — "guidPrefix: GuidPrefix — Provides the
GuidPrefix that should be used to reconstruct the GUIDs of all the RTPS Reader
entities whose EntityIds appears in the Submessages that follow."

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`);
`crates/rtps/src/reliable_reader.rs` (`wrap_to_writer` sendet INFO_DST vor
ACKNACK/NACK_FRAG)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_carries_info_dst`,
`crates/rtps/src/reliable_writer.rs::tests::every_proxy_directed_datagram_carries_info_dst_not_just_the_heartbeat_path`

**Status:** `done`

### §8.3.8.8 InfoDestination ungültig: submessageLength zu klein

**Spec:** §8.3.8.8, S. 65 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, InfoDestination)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.8 InfoDestination: destGuidPrefix = guidPrefix

**Spec:** §8.3.8.8, S. 65 — "if (InfoDestination.guidPrefix !=
GUIDPREFIX_UNKNOWN) { Receiver.destGuidPrefix = InfoDestination.guidPrefix }".

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_info_source_does_not_reset_dest_guid_prefix`

**Status:** `done`

### §8.3.8.8 InfoDestination: GUIDPREFIX_UNKNOWN setzt den eigenen Prefix

**Spec:** §8.3.8.8, S. 65 — "else { Receiver.destGuidPrefix = <GuidPrefix_t of
the Participant receiving the_message> }".

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`,
`InfoDestination`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.8 destGuidPrefix im Nutzdatenpfad

**Spec:** §8.3.8.2, S. 56 — "readerGUID = { Receiver.destGuidPrefix,
Data.readerId }"; ebenso §8.3.8.4, S. 61 (Gap), §8.3.8.6, S. 64 (Heartbeat),
§8.3.8.7, S. 65 (HeartbeatFrag); §8.3.8.3, S. 58 — "the interpretation of the
DataFrag Submessages is identical to that of the Data Submessage"; §8.3.8.1,
S. 54 — "writerGUID = { Receiver.destGuidPrefix, AckNack.writerId }"; ebenso
§8.3.8.12, S. 69 (NackFrag).

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.9 InfoReply: Purpose

**Spec:** §8.3.8.9, S. 66 — "It contains explicit information on where to send
a reply to the Submessages that follow it within the same message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.9 Table 8.47 InfoReply EndiannessFlag

**Spec:** §8.3.8.9, S. 66, Table 8.47 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::write_body`,
`InfoReplySubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`

**Status:** `done`

### §8.3.8.9 Table 8.47 InfoReply MulticastFlag

**Spec:** §8.3.8.9, S. 66, Table 8.47 — "MulticastFlag … Indicates whether the
Submessage also contains a multicast address."

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_REPLY_FLAG_MULTICAST`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`

**Status:** `done`

### §8.3.8.9 Table 8.47 InfoReply unicastLocatorList

**Spec:** §8.3.8.9, S. 66, Table 8.47 — "unicastLocatorList: LocatorList —
Indicates an alternative set of unicast addresses that the Writer should use
to reach the Readers when replying to the Submessages that follow."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::unicast_locators`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_empty_unicast_list_is_valid`,
`crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`

**Status:** `done` — ob ein Writer die Liste beim Antworten benutzt, ist
unter §8.4 geführt.

### §8.3.8.9 Table 8.47 InfoReply multicastLocatorList

**Spec:** §8.3.8.9, S. 66, Table 8.47 — "multicastLocatorList … Only present
when the MulticastFlag is set."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::multicast_locators`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`

**Status:** `done`

### §8.3.8.9 InfoReply ungültig: submessageLength zu klein

**Spec:** §8.3.8.9, S. 66 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`,
`crates/rtps/src/submessages.rs::validity_tests::an_info_reply_locator_of_an_unknown_kind_is_left_out_and_counted`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben. Ein Locator unbekannter Art in der Liste macht die Submessage nicht ungültig (§9.3.2.4: Herstellerarten): er wird ausgelassen und gezählt (`skipped_unknown_locators`), die übrigen bleiben.

### §8.3.8.9 InfoReply: Receiver.unicastReplyLocatorList

**Spec:** §8.3.8.9, S. 66 — "Receiver.unicastReplyLocatorList =
InfoReply.unicastLocatorList".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_reply`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_reply_sets_locators`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_sets_reply_locators`

**Status:** `done`

### §8.3.8.9 InfoReply: Receiver.multicastReplyLocatorList

**Spec:** §8.3.8.9, S. 66 — "if ( MulticastFlag ) {
Receiver.multicastReplyLocatorList = InfoReply.multicastLocatorList } else {
Receiver.multicastReplyLocatorList = <empty> }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_reply`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_reply_sets_locators`,
`crates/rtps/src/receiver_state.rs::tests::an_info_reply_without_the_multicast_flag_empties_the_multicast_list`

**Status:** `done` — ohne MulticastFlag wird die Liste geleert.

### §8.3.8.10 InfoSource: Purpose

**Spec:** §8.3.8.10, S. 66 — "This message modifies the logical source of the
Submessages that follow."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.10 Table 8.48 InfoSource EndiannessFlag

**Spec:** §8.3.8.10, S. 67, Table 8.48 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::write_body`,
`InfoSourceSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_be`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource protocolVersion

**Spec:** §8.3.8.10, S. 67, Table 8.48 — "protocolVersion: ProtocolVersion —
Indicates the protocol used for subsequent Submessages."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::protocol_version`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_le`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource vendorId

**Spec:** §8.3.8.10, S. 67, Table 8.48 — "vendorId: VendorId — Indicates the
VendorId of the vendor that originated the subsequent Submessages."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::vendor_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_le`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource guidPrefix

**Spec:** §8.3.8.10, S. 67, Table 8.48 — "guidPrefix: GuidPrefix — Identifies
the Participant that is the container of the RTPS Writer entities that are the
source of the Submessages that follow."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::guid_prefix`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`

**Status:** `done`

### §8.3.8.10 InfoSource ungültig: submessageLength zu klein

**Spec:** §8.3.8.10, S. 67 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.10 InfoSource: sourceGuidPrefix, sourceVersion, sourceVendorId

**Spec:** §8.3.8.10, S. 67 — "Receiver.sourceGuidPrefix =
InfoSource.guidPrefix; Receiver.sourceVersion = InfoSource.protocolVersion;
Receiver.sourceVendorId = InfoSource.vendorId".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`,
`crates/discovery/tests/opensplice_infosrc_regression.rs::opensplice_bundled_infosrc_datagrams_attribute_to_the_real_source`

**Status:** `done`

### §8.3.8.10 InfoSource: Reply-Listen = {LOCATOR_INVALID}

**Spec:** §8.3.8.10, S. 67 — "Receiver.unicastReplyLocatorList = {
LOCATOR_INVALID }; Receiver.multicastReplyLocatorList = { LOCATOR_INVALID }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`

**Status:** `done` — beide Listen werden auf `{ LOCATOR_INVALID }` gesetzt.

### §8.3.8.10 InfoSource: haveTimestamp = false

**Spec:** §8.3.8.10, S. 67 — "haveTimestamp = false".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`

**Status:** `done`

### §8.3.8.11 InfoTimestamp: Purpose

**Spec:** §8.3.8.11, S. 67 — "This Submessage is used to send a timestamp
which applies to the Submessages that follow within the same message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.11 Table 8.49 InfoTimestamp EndiannessFlag

**Spec:** §8.3.8.11, S. 67, Table 8.49 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::write_body`,
`InfoTimestampSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`

**Status:** `done`

### §8.3.8.11 Table 8.49 InfoTimestamp InvalidateFlag

**Spec:** §8.3.8.11, S. 67, Table 8.49 — "InvalidateFlag … Indicates whether
subsequent Submessages should be considered as having a timestamp or not."

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_TIMESTAMP_FLAG_INVALIDATE`,
`InfoTimestampSubmessage::invalidate`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_invalidate_flag_yields_empty_body`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_decode_with_invalidate_ignores_body`

**Status:** `done`

### §8.3.8.11 Table 8.49 InfoTimestamp timestamp

**Spec:** §8.3.8.11, S. 67, Table 8.49 — "timestamp: Timestamp — Present only
if the InvalidateFlag is not set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::timestamp`,
`InfoTimestampSubmessage::timestamp_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`

**Status:** `done`

### §8.3.8.11 InfoTimestamp ungültig: submessageLength zu klein

**Spec:** §8.3.8.11, S. 68 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.11 InfoTimestamp: haveTimestamp und timestamp setzen

**Spec:** §8.3.8.11, S. 68 — "if ( !InfoTimestamp.InvalidateFlag ) {
Receiver.haveTimestamp = true; Receiver.timestamp = InfoTimestamp.timestamp
… } else { Receiver.haveTimestamp = false }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_timestamp_sets_value`,
`crates/rtps/src/receiver_state.rs::tests::apply_info_timestamp_with_invalidate_clears`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`

**Status:** `done`

### §8.3.8.11 InfoTimestamp: Korrektur bei clockSkewDetected

**Spec:** §8.3.8.11, S. 68 — "if ( Receiver.clockSkewDetected ) {
Receiver.timestamp += Receiver.rtpsReceptionTime - Receiver.rtpsSendTimestamp
}".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::note_reception`, `note_clock_skew`,
`apply_info_timestamp`); `crates/rtps/src/datagram.rs` (`ReceptionClock`, `ReceiveContext`);
`crates/dcps/src/runtime.rs` (`receive_context`, `RuntimeConfig::clock_skew_threshold_s`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_reception_time_is_kept_and_checked_against_the_send_time`,
`crates/rtps/src/receiver_state.rs::tests::an_info_timestamp_is_corrected_by_the_clock_skew`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`,
`crates/dcps/tests/runtime_config_layering.rs::the_clock_skew_threshold_is_a_setting_and_off_by_default`

**Status:** `done` — `apply_info_timestamp` addiert bei `clockSkewDetected` Empfangs- minus Sendezeit; in jedem Empfangspfad wirkt das, sobald `wire.clock_skew_threshold_s` gesetzt ist (Vorgabe aus, siehe §8.3.7.4 TimestampFlag).

### §8.3.8.11 InfoTimestamp: Quellzeitstempel der folgenden Data/DataFrag

**Spec:** §8.3.8.11, S. 68 — "The reception of the InfoTimestamp submessage
sets the source timestamp associated with any subsequent Data and DataFrag
submessages."

**Repo:** `crates/rtps/src/receiver_state.rs`, `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_timestamp_is_no_source_timestamp`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done` — ein INFO_TS setzt den Quellzeitstempel der folgenden Data und DataFrag, der I-Flag-Fall löscht ihn; der Zeitstempel einer HeaderExtension ist `rtpsSendTimestamp` und kein Quellzeitstempel.

### §8.3.8.12 NackFrag: Purpose

**Spec:** §8.3.8.12, S. 68 — "This Submessage can only contain negative
acknowledgements. … Fragments can be negatively acknowledged in any order."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.12 Table 8.50 NackFrag EndiannessFlag

**Spec:** §8.3.8.12, S. 68, Table 8.50 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::write_body`,
`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_be`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag readerId

**Spec:** §8.3.8.12, S. 68, Table 8.50 — "readerId: EntityId — Identifies the
Reader entity that requests to receive certain fragments."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nackfrag_dispatches_only_to_matching_proxy`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag writerId

**Spec:** §8.3.8.12, S. 69, Table 8.50 — "writerId: EntityId — Identifies the
Writer entity that is the target of the NackFrag message."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::writer_id`);
`crates/rtps/src/reliable_writer.rs` (`handle_nackfrag`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_queues_fragment_resends`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag writerSN

**Spec:** §8.3.8.12, S. 69, Table 8.50 — "writerSN: SequenceNumber — The
sequence number for which some fragments are missing."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::writer_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_fragments`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag fragmentNumberState

**Spec:** §8.3.8.12, S. 69, Table 8.50 — "fragmentNumberState:
FragmentNumberSet — … The fragment numbers that appear in the set indicate
missing fragments on the reader side."

**Repo:** `crates/rtps/src/submessages.rs`
(`NackFragSubmessage::fragment_number_state`); `crates/rtps/src/reader_proxy.rs`
(`requested_fragments_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::requested_fragments_set_above_ack_only`,
`crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_queues_fragment_resends`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag count

**Spec:** §8.3.8.12, S. 69, Table 8.50 — "count: Count — A counter that is
incremented each time a new NackFrag message is sent."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`build_nackfrag_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::the_counts_of_acknack_and_nack_frag_rise_with_each_message`

**Status:** `done` — aufeinanderfolgende NACK_FRAGs tragen die Zähler 1, 2, 3.

### §8.3.8.12 NackFrag ungültig: submessageLength zu klein

**Spec:** §8.3.8.12, S. 69 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — die Kürzung wird erkannt; die Submessage ist ungültig und mit ihr der Rest der Nachricht, die Submessages davor bleiben (`ParsedDatagram::invalid_rest`, gezählt).

### §8.3.8.12 NackFrag ungültig: writerSN ≤ 0

**Spec:** §8.3.8.12, S. 69 — "writerSN.value is zero or negative."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`

**Status:** `done` — `writerSN` unter 1 wird abgewiesen.

### §8.3.8.12 NackFrag ungültig: fragmentNumberState ungültig

**Spec:** §8.3.8.12, S. 69 — "fragmentNumberState is invalid (as defined in
8.3.5.7)."

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done` — `numBits > 256` und `bitmapBase < 1` werden abgewiesen.

### §8.3.8.12 NackFrag: writerGUID = {destGuidPrefix, writerId}

**Spec:** §8.3.8.12, S. 69 — "writerGUID = { Receiver.destGuidPrefix,
NackFrag.writerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`rt.writer_slot(nf.writer_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.12 NackFrag: readerGUID = {sourceGuidPrefix, readerId}

**Spec:** §8.3.8.12, S. 69 — "readerGUID = { Receiver.sourceGuidPrefix,
NackFrag.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`Guid::new(src_prefix, nf.reader_id)`); `crates/rtps/src/reliable_writer.rs`
(`handle_nackfrag`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_unknown_source_counts`

**Status:** `done`

### §8.3.8.13 Pad: Purpose

**Spec:** §8.3.8.13, S. 69 — "The purpose of this Submessage is to allow the
introduction of any padding necessary to meet any desired memory- alignment
requirements. It has no other meaning."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zweckangabe.

### §8.3.8.13 Pad: Länge aus submessageLength, immer gültig

**Spec:** §8.3.8.13, S. 69 — "This Submessage has no contents. … The amount of
padding is determined by the value of submessageLength. … This Submessage is
always valid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: PAD als
`ParsedSubmessage::Unknown` übersprungen, Länge 0 als Länge)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

---

## §8.4 Behavior Module

### §8.4.1 Overview

**Spec:** §8.4.1, S. 70 — Gliederung des Moduls; §8.4.2 enthält die
Pflichten, §8.4.7–§8.4.12 zwei Referenzimplementierungen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung.

### §8.4.1 DataWriter entfernt Changes nur nach Kommunikationsstand und QoS

**Spec:** §8.4.1, S. 70 — "It is the responsibility of the DDS DataWriter to
only remove those CacheChange changes that can be removed based on the
communication status and the DDS DataWriter's QoS. For example, the HISTORY
QoS setting of KEEP_LAST with a depth of 1 allows a DataWriter to remove a
CacheChange if a more recent change replaces the value of the same
data-object."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_acknowledged`,
KEEP_LAST-Verdrängung über `HistoryCache::insert_returning_evicted`);
`crates/dcps/src/runtime.rs` (`await_writer_room` für KEEP_ALL)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`,
`crates/rtps/src/history_cache.rs::tests::keep_last_per_instance_keeps_the_newest_of_each_instance`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** `done`

### §8.4.1.1 Example Behavior

**Spec:** §8.4.1.1, S. 70–72 — "The contents of this sub clause are not part
of the formal specification of the protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — ausdrücklich nicht Teil der formalen Spec.

### §8.4.2 Behavior Required for Interoperability: Geltungsbereich

**Spec:** §8.4.2, S. 72–73 — die Pflichten gelten für den Austausch zwischen
Implementierungen verschiedener Hersteller.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Geltungsbereich; die Pflichten folgen
einzeln.

### §8.4.2.1 Nur RTPS-Nachrichten

**Spec:** §8.4.2.1, S. 73 — "No other messages can be used than the RTPS
Messages defined in 8.3. … Vendors may extend Messages for vendor specific
needs using the extension mechanisms provided by the protocol (see 8.6)."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`);
`crates/rtps/src/parameter_list.rs` (Hersteller-PIDs `SHM_LOCATOR`,
`ZERODDS_TYPE_ID`)

**Tests:** `crates/rtps/tests/vendor_frames.rs::cyclone_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`

**Status:** `done` — die Abweichung bei der HeaderExtension-Kennung steht
unter §9.4.5.1.1.

### §8.4.2.1 Message Receiver umgesetzt

**Spec:** §8.4.2.1, S. 73 — "Implementations must implement the rules
followed by the RTPS Message Receiver, as introduced in 8.3.4 … This
requirement also includes proper Message formatting by preceding Entity
Submessages with Interpreter Submessages when required".

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`);
`crates/rtps/src/reliable_writer.rs` (INFO_TS und INFO_DST vor DATA,
HEARTBEAT); `crates/rtps/src/reliable_reader.rs` (`wrap_to_writer`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`,
`crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_carries_info_dst`

**Status:** `done` — der Receiver ist umgesetzt; die Items zu §8.3.4, §8.3.4.1 und §8.3.8, in denen er von der Spec abwich („Gültige Submessages vor einer ungültigen“, „destGuidPrefix im Nutzdatenpfad“ u. a.), stehen auf `done`.

### §8.4.2.1 Zeitverhalten einstellbar

**Spec:** §8.4.2.1, S. 73 — "where the requirements on the protocol behavior
allow delayed responses or specify periodic events, implementations must
allow the end-user to tune those timing characteristics."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::nack_response_delay`,
`nack_suppression_duration`, `writer_heartbeat_period`, `reader_heartbeat_response_delay`,
`reader_heartbeat_suppression_duration`, `spdp_period`, `wlp_period`)

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`,
`crates/rtps/src/reliable_writer.rs::tests::a_nack_inside_the_suppression_duration_is_ignored`

**Status:** `done` — nackResponseDelay, nackSuppressionDuration, heartbeatPeriod, heartbeatResponseDelay, heartbeatSuppressionDuration, SPDP- und WLP-Periode sind einstellbar (`writer.*_ms`, `reader.*_ms`, `discovery.*_ms`, Man-Seite und Beispielkonfiguration).

### §8.4.2.1 HEARTBEAT-Zeiten der Nutzer-Endpoints einstellbar

**Spec:** §8.4.2.1, S. 73 — wie vorheriges Item; §8.4.7.1 Table 8.53
`heartbeatPeriod`, §8.4.10.1 Table 8.67 `heartbeatResponseDelay`,
`heartbeatSuppressionDuration`.

**Repo:** `crates/dcps/src/runtime.rs` (`writer_heartbeat_period`, `reader_heartbeat_response_delay`,
`reader_heartbeat_suppression_duration`)

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `writer.heartbeat_period_ms`, `reader.heartbeat_response_delay_ms`, `reader.heartbeat_suppression_duration_ms`.

### §8.4.2.1 SPDP und SEDP umgesetzt

**Spec:** §8.4.2.1, S. 73 — "For the purpose of interoperability,
implementations must implement at least the Simple Participant Discovery
Protocol and Simple Endpoint Discovery Protocol (see 8.5.1)."

**Repo:** `crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`,
`crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`

**Status:** `done` — die Einzelpflichten stehen unter §8.5.

### §8.4.2.2 Writer senden nicht außer der Reihe

**Spec:** §8.4.2.2, S. 73 — "A Writer must send out data samples in the order
they were added to its HistoryCache."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`:
Senden nur an einen Proxy, der im Schritt ist; Reparaturen aufsteigend über
`next_requested_change`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_zero_loss_delivers_all_in_order`,
`crates/rtps/src/reliable_writer.rs::tests::repairs_in_order_bring_a_lagging_proxy_into_step`

**Status:** `done`

### §8.4.2.2 Writer senden Inline-QoS auf Wunsch des Readers

**Spec:** §8.4.2.2, S. 73 — "A Writer must honor a Reader's request to receive
data messages with in-line QoS."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_inline_writer_qos`,
`sample_inline_qos`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::expects_inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`

**Status:** `done`

### §8.4.2.2 Periodische HEARTBEATs (nur reliable)

**Spec:** §8.4.2.2, S. 73–74 — "A Writer must periodically inform each
matching reliable Reader of the availability of a data sample by sending a
periodic HEARTBEAT Message … For strict reliable communication, the Writer
must continue to send HEARTBEAT Messages to a Reader until the Reader has
either acknowledged receiving all available samples or has disappeared."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `announcing`,
`heartbeat_due`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_emits_heartbeat_after_period`,
`crates/rtps/src/reliable_writer.rs::tests::an_acknowledged_reader_is_heartbeated_until_it_answers_one`,
`crates/rtps/src/reliable_writer.rs::tests::a_reader_that_acknowledged_the_cache_gets_no_periodic_heartbeat`

**Status:** `done`

### §8.4.2.2 Antwort auf eine negative Bestätigung (nur reliable)

**Spec:** §8.4.2.2, S. 74 — "When receiving an ACKNACK Message indicating a
Reader is missing some data samples, the Writer must respond by either sending
the missing data samples, sending a GAP message when the sample is not
relevant, or sending a HEARTBEAT message when the sample is no longer
available."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: DATA aus dem Cache,
GAP über `append_gap` für nicht mehr vorhandene Changes)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.4.2.2 HEARTBEAT und GAP mit Writer-Group-Information

**Spec:** §8.4.2.2, S. 74 — "A Writer belonging to a Group shall send
HEARTBEAT or GAP Submessages to its matched Readers even if the Reader has
acknowledged all of that Writer's samples."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_writer_heartbeats_an_acknowledged_reader_when_the_group_moved_on`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writer_with_an_empty_cache_is_due_for_a_heartbeat`

**Status:** `done`

### §8.4.2.3 Best-Effort-Reader sind passiv

**Spec:** §8.4.2.3, S. 74 — "A best-effort Reader is completely passive as it
only receives data and does not send messages itself."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_sends_no_acknack`

**Status:** `done` — ein Reader mit `set_best_effort(true)` sendet weder ACKNACK
noch NACK_FRAG, weder beim Match noch auf HEARTBEATs.

### §8.4.2.3 Antwort auf HEARTBEAT ohne FinalFlag

**Spec:** §8.4.2.3, S. 74 — "Upon receiving a HEARTBEAT Message with final
flag not set, the Reader must respond with an ACKNACK Message. … The response
may be delayed to avoid message storms."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`,
`tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.2.3 Antwort auf HEARTBEAT mit fehlenden Samples

**Spec:** §8.4.2.3, S. 74 — "a Reader that is missing some data samples must
respond with an ACKNACK Message indicating which data samples are missing.
This requirement only applies if the Reader can accommodate these missing
samples in its cache … The response is not required when a liveliness
HEARTBEAT has both liveliness and final flags set".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`,
`tick_outbound`, `full_for_the_consumer`, `build_acknack_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`,
`crates/rtps/src/reliable_reader.rs::tests::a_cache_full_for_the_consumer_holds_the_acknack_until_it_takes`,
`crates/rtps/src/reliable_reader.rs::tests::a_cache_full_behind_a_missing_change_still_names_it`

**Status:** `done`

### §8.4.2.3 Einmal bestätigt, immer bestätigt (Reader)

**Spec:** §8.4.2.3, S. 74 — "Once a Reader has positively acknowledged
receiving a sample using an ACKNACK Message, it can no longer negatively
acknowledge that same sample at a later point."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`acknack_base` monoton;
`unreceive_change_set` nur oberhalb der Basis)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::acknack_base_walks_up`,
`crates/rtps/src/reliable_reader.rs::tests::sample_rejected_by_a_full_cache_stays_missing_for_the_acknack`

**Status:** `done`

### §8.4.2.3 Writer sendet erneut bestätigte, wieder angeforderte Samples

**Spec:** §8.4.2.3, S. 74 — "if a Writer receives a negative acknowledgement
to a previously positively acknowledged sample, and the Writer can still
service the request, the Writer should send the sample."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes_set`);
`crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`, `tick`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`,
`crates/rtps/src/reliable_writer.rs::tests::an_acknowledged_change_asked_for_again_is_sent_again`

**Status:** `done` — eine erneute Anforderung eines bestätigten Samples bleibt
stehen; der Writer sendet es, solange es im Cache ist, sonst einen GAP.

### §8.4.2.3 ACKNACK nur als Antwort auf HEARTBEAT

**Spec:** §8.4.2.3, S. 74 — "In steady state, an ACKNACK Message can only be
sent as a response to a HEARTBEAT Message from a Writer. ACKNACK Messages can
be sent from a Reader when it first discovers a Writer as an optimization."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`, `handle_heartbeat`,
`handle_data_frag_opening`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::acknack_and_nack_frag_wait_for_a_heartbeat`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`

**Status:** `done` — ein ACKNACK entsteht aus einem HEARTBEAT (Table 8.77 T2) und beim
ersten Match eines Writers; weder das erneute `add_writer_proxy` eines bekannten Writers
noch ein DATA_FRAG mit Lücken setzen ihn.

### §8.4.3 Implementing the RTPS Protocol

**Spec:** §8.4.3, S. 75 — "a compliant implementation of the protocol need
only satisfy the requirements presented in 8.4.2. … Actual implementations
need not necessarily follow the reference implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — stellt die Referenzimplementierungen frei.
Die Übergänge unter §8.4.8–§8.4.12 werden gegen die ZeroDDS-Typen geprüft,
die eine Referenzimplementierung abbilden: `ReliableWriter` (Stateful
Writer, best effort und reliable, Nutzdatenpfad), `ReliableReader` (Stateful
Reader), `BestEffortWriter`, `ReliableStatelessWriter`, `BestEffortReader`
(nur in Tests verwendet) und der SPDP-Beacon.

### §8.4.4 Zulässige Kombinationen von reliabilityLevel

**Spec:** §8.4.4, S. 75 — "An RTPS Writer cannot be matched to an RTPS Reader
unless either the RTPS Writer has the reliabilityLevel set to RELIABLE, or
else both the RTPS Writer and RTPS Reader have the reliabilityLevel set to
BEST_EFFORT."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind`-Ordnung);
QoS-Abgleich beim Match in `crates/dcps/src/runtime.rs`; der ReaderProxy ist
zuverlässig nur bei zuverlässigem Writer und Reader (`ReaderProxy::new` im
SEDP-Match-Pfad von `crates/dcps/src/runtime.rs`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`,
`crates/qos/src/policies/reliability.rs::tests::ordering_reliable_higher_than_besteffort`,
`crates/dcps/tests/transient_local_durability.rs::linux::transient_local_writer_delivers_history_to_late_joiner`

**Status:** `done` — der Abgleich der angebotenen und verlangten QoS ist im
DCPS-Audit geführt.

### §8.4.4 Stateful und Stateless verständigen sich

**Spec:** §8.4.4, S. 75 — "a Stateful Writer is able to communicate with a
Stateless Reader and vice versa."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Folgerung aus §8.4.2; keine eigene
Anforderung.

### §8.4.5 Notational Conventions

**Spec:** §8.4.5, S. 75–76, Table 8.51 — Abkürzungen DW, DR, W, RP, RL, R, WP,
WHC, RHC.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Schreibweise.

### §8.4.6 Table 8.52 Duration_t

**Spec:** §8.4.6, S. 76, Table 8.52 — "Duration_t: Type used to hold time
differences. Should have at least nano-second resolution."

**Repo:** `crates/qos/src/duration.rs` (`Duration {seconds, fraction}`,
2⁻³² s)

**Tests:** `crates/dcps/src/time.rs::tests::duration_zero_and_infinite_sentinels`,
`crates/rtps/src/participant_data.rs::tests::duration_roundtrip_le`

**Status:** `done`

### §8.4.6 Table 8.52 ChangeForReaderStatusKind

**Spec:** §8.4.6, S. 76, Table 8.52 — "UNSENT, UNACKNOWLEDGED, REQUESTED,
ACKNOWLEDGED, UNDERWAY".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn`,
`highest_acked_sn`, `requested_changes` statt eines Status je Change)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`,
`crates/rtps/src/reader_proxy.rs::tests::acking_also_prunes_requested_changes`

**Status:** `done` — als Zählerstände und Mengen abgebildet (§8.4.15.1).

### §8.4.6 Table 8.52 ChangeFromWriterStatusKind

**Spec:** §8.4.6, S. 76, Table 8.52 — "NOT_AVAILABLE, MISSING, RECEIVED,
UNKNOWN. There are three sub-kinds of NOT_AVAILABLE: NA_FILTERED,
NA_REMOVED, NA_UNSPECIFIED".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`ChangeFromWriterStatus`, `WriterProxy::change_status`,
`NotAvailableKind`, `not_available_change_set`, `lost_changes_update`);
`crates/rtps/src/reliable_reader.rs` (`ReliableReader::change_status`, `handle_gap`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::change_status_is_kept_per_change_with_the_sub_kind`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_sets_na_removed_per_change`,
`crates/rtps/src/writer_proxy.rs::tests::not_available_runs_are_joined_and_bounded`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`

**Status:** `done` — UNKNOWN, MISSING, RECEIVED und NOT_AVAILABLE mit den Unterarten NA_FILTERED, NA_REMOVED und NA_UNSPECIFIED werden je Änderung geführt (`WriterProxy::change_status`). Die NOT_AVAILABLE-Änderungen stehen als Läufe gleicher Unterart (höchstens 4096; älter heißt `None`). `lost_changes_update` trägt NA_REMOVED ein und zählt es; eine Änderung mit FilteredFlag trägt NA_FILTERED ein.

### §8.4.6 Table 8.52 InstanceHandle_t

**Spec:** §8.4.6, S. 76, Table 8.52 — "Type used to represent the identity of
a data-object whose changes in value are communicated by the RTPS protocol."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::the_instance_index_follows_inserts_evictions_and_removals`

**Status:** `done` — wie Table 8.2 (§8.2.1.2).

### §8.4.6 Table 8.52 ParticipantMessageData

**Spec:** §8.4.6, S. 76, Table 8.52 — "Type used to hold data exchanged
between Participants. The most notable use of this type is for the Writer
Liveliness Protocol."

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`

**Status:** `done` — Inhalt und Wire-Format unter §8.4.13.4 und §9.6.3.1.

### §8.4.7.1 Table 8.53 Writer pushMode

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "If pushMode==true, then the Writer
will push changes to the reader. If pushMode==false, changes will only be
announced via heartbeats and only be sent as response to the request of a
reader."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_push_mode`, `push_mode`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::push_mode_false_announces_by_heartbeat_and_sends_only_on_request`,
`crates/rtps/src/reliable_writer.rs::tests::push_mode_false_sends_no_data_on_the_piggyback_path`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer heartbeatPeriod

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "heartbeatPeriod: Protocol tuning
parameter that allows the RTPS Writer to repeatedly announce the
availability of data by sending a Heartbeat Message."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`ReliableWriterConfig::heartbeat_period`, `DEFAULT_HEARTBEAT_PERIOD`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_due_follows_the_last_heartbeat`,
`crates/rtps/src/reliable_writer.rs::tests::tick_emits_heartbeat_after_period`

**Status:** `done` — die Einstellbarkeit für den Anwender ist unter §8.4.2.1
geführt.

### §8.4.7.1 Table 8.53 Writer nackResponseDelay

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "nackResponseDelay: Protocol tuning
parameter that allows the RTPS Writer to delay the response to a request for
data from a negative acknowledgment."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_nack_response_delay`);
`crates/rtps/src/reader_proxy.rs` (`repair_due`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`,
`crates/rtps/src/reliable_writer.rs::tests::a_zero_nack_response_delay_repairs_on_the_next_tick`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer nackSuppressionDuration

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "nackSuppressionDuration: Protocol
tuning parameter that allows the RTPS Writer to ignore requests for data from
negative acknowledgments that arrive 'too soon' after the corresponding change
is sent."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`set_nack_suppression_duration`, `too_soon`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_nack_inside_the_suppression_duration_is_ignored`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer lastChangeSequenceNumber

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "Internal counter used to assign
increasing sequence number to each change made by the Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`next_sn`, `last_written_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer writer_cache

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "writer_cache: HistoryCache — Contains
the history of CacheChange changes for this Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::cache`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_at_cache_capacity_is_error`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer dataMaxSizeSerialized

**Spec:** §8.4.7.1, S. 78, Table 8.53 — "Optional attribute that indicates the
maximum size of any SerializedPayload that may be sent by the Writer."

**Repo:** `crates/dcps/src/dds_type.rs` (`MAX_SERIALIZED_SIZE`);
`crates/dcps/src/publisher.rs` (`WriterRegistration`); `crates/dcps/src/runtime.rs` (`UserWriterConfig::data_max_size_serialized`,
`build_publication_data`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`

**Status:** `done` — das Attribut ist optional. Der Writer kündigt `dataMaxSizeSerialized` als `PID_TYPE_MAX_SIZE_SERIALIZED` an, wenn es gesetzt ist: aus `DdsType::MAX_SERIALIZED_SIZE` oder aus `UserWriterConfig::data_max_size_serialized` (der Wert steht ohne Encapsulation-Header, angekündigt wird er mit den 4 Oktetten des Headers). Ohne Wert kündigt er nichts an.

### §8.4.7.1 Table 8.54 Writer new

**Spec:** §8.4.7.1, S. 79 — "this.lastChangeSequenceNumber := 0;
this.writer_cache := new HistoryCache;" und Übernahme der Attribute aus dem
Konstruktor.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::new`:
`next_sn: 0`, neuer `HistoryCache`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`

**Status:** `done`

### §8.4.7.1 Table 8.54 Writer new_change

**Spec:** §8.4.7.1, S. 79 — "The sequence number of the CacheChange is
automatically set to be the sequenceNumber of the previous change plus one.
… ++this.lastChangeSequenceNumber; a_change := new CacheChange(kind,
this.guid, this.lastChangeSequenceNumber, data, inlineQos, handle);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`,
`write_lifecycle`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/writer.rs::tests::writer_sn_overflow_is_error`

**Status:** `done`

### §8.4.7.1 Default nackResponseDelay = 200 ms

**Spec:** §8.4.7.1, S. 79 — "nackResponseDelay.sec = 0;
nackResponseDelay.nanosec = 200 * 1000 * 1000; //200 milliseconds".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_RESPONSE_DELAY`, `SPEC_NACK_RESPONSE_DELAY`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::nack_response_delay`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_NACK_RESPONSE_DELAY` und `RuntimeConfig::nack_response_delay` sind 200 ms; ein NACK wird erst nach dieser Zeit mit der Reparatur beantwortet (Test). `writer.nack_response_delay_ms: 0` antwortet im nächsten Tick.

### §8.4.7.1 Default nackResponseDelay

**Spec:** §8.4.7.1, S. 79 — "The following timing-related values are used as
the defaults in order to facilitate 'out-of-the-box' interoperability between
implementations."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_RESPONSE_DELAY`, `DEFAULT_NACK_SUPPRESSION_DURATION`);
`crates/dcps/src/runtime.rs`; `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`

**Status:** `done` — die Werte der Spec sind die Vorgaben: nackResponseDelay 200 ms, nackSuppressionDuration 0 (Laufzeit, YAML-Vorgabe, Man-Seite, Beispielkonfiguration). Die früheren 0 ms bleiben über `writer.nack_response_delay_ms` erreichbar; die Tests mit Millisekunden-Uhr setzen 0 ausdrücklich.

### §8.4.7.1 Default nackSuppressionDuration = 0

**Spec:** §8.4.7.1, S. 79 — "nackSuppressionDuration.sec = 0;
nackSuppressionDuration.nanosec = 0;".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_SUPPRESSION_DURATION`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`

**Status:** `done`

### §8.4.7.2 Table 8.55 StatelessWriter reader_locators

**Spec:** §8.4.7.2, S. 80, Table 8.55 — "The StatelessWriter maintains the
list of locators to which it sends the CacheChanges. This list may include
both unicast and multicast locators."

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::reader_locators`,
`ReaderLocator`); `crates/discovery/src/spdp.rs` (`SpdpBeacon`, Ziel-Locatoren)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — die Liste besteht aus `ReaderLocator`-Objekten mit eigenem Zustand (`highest_sent_seq_num`, angeforderte Changes, `expects_inline_qos`).

### §8.4.7.2 Table 8.56 StatelessWriter new

**Spec:** §8.4.7.2, S. 80 — "this.reader_locators := <empty>;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs`
(`ReliableStatelessWriter::new`: `reader_locators` leer)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::shutdown_clears_the_cache_and_the_requests`

**Status:** `done`

### §8.4.7.2 Table 8.56 StatelessWriter reader_locator_add

**Spec:** §8.4.7.2, S. 80 — "ADD a_locator TO {this.reader_locators};"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)
`reader_locator_add`

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — ein Locator wird als ReaderLocator mit `expectsInlineQos` hinzugefügt, einmal je Locator. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.7.2 Table 8.56 StatelessWriter reader_locator_remove

**Spec:** §8.4.7.3, S. 81 — "REMOVE a_locator FROM {this.reader_locators};"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::reader_locator_remove`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — ein ReaderLocator wird entfernt; ein zweites Entfernen meldet `false`.

### §8.4.7.2 Table 8.56 StatelessWriter unsent_changes_reset

**Spec:** §8.4.7.2, S. 81 — "This operation resets the 'highestSentChangeSN'
for all the ReaderLocators … useful when called periodically to cause the
StatelessWriter to keep re-sending all available changes".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::unsent_changes_reset`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_reset_makes_every_change_unsent_again`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — `unsent_changes_reset` setzt `highestSentChangeSN` aller ReaderLocatoren auf 0; der nächste Tick sendet alle Changes des Caches erneut an jeden Locator. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.7.3 Table 8.57 ReaderLocator-Attribute

**Spec:** §8.4.7.3, S. 81, Table 8.57 — highestSentChangeSN,
requestedChanges, locator, expectsInlineQos.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — `ReaderLocator` führt `locator`, `expects_inline_qos`, `highest_sent_seq_num` und die angeforderten Changes. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.7.3 Table 8.58 ReaderLocator-Operationen

**Spec:** §8.4.7.3, S. 81–82, Table 8.58 — new, next_requested_change,
next_unsent_change, requested_changes, requested_changes_set,
unsent_changes.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — `requested_changes`, `requested_changes_set`, `next_requested_change`, `unsent_changes` sind Methoden des `ReaderLocator`; `next_unsent_change` steckt in der Push-Maschine (`next_cached_above`). `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.7.4 Table 8.59 StatefulWriter matched_readers

**Spec:** §8.4.7.4, S. 83, Table 8.59 — "The StatefulWriter keeps track of all
the RTPS Readers matched with it. Each matched reader is represented by an
instance of the ReaderProxy class."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`reader_proxies`,
`reader_proxies()`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter new

**Spec:** §8.4.7.4, S. 83 — "this.matched_readers := <empty>;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::new` mit
`ReliableWriterConfig::reader_proxies`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_without_proxies_is_true`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter is_acked_by_all

**Spec:** §8.4.7.4, S. 83 — "return true IF and only IF FOREACH proxy IN
this.matched_readers a_change_seq_num IN proxy.acknowledged_changes".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`is_acked_by_all`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_follows_the_slowest_reader_proxy`,
`crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_without_proxies_is_true`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_add

**Spec:** §8.4.7.4, S. 83–84 — "ADD a_reader_proxy TO
{this.matched_readers};"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::add_reader_proxy_is_idempotent_on_same_guid`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_after_add_reader_proxy_is_non_final`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_remove

**Spec:** §8.4.7.4, S. 84 — "REMOVE a_reader_proxy FROM
{this.matched_readers}; delete proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_lookup

**Spec:** §8.4.7.4, S. 84 — "FIND proxy IN this.matched_readers SUCH-THAT
(proxy.remoteReaderGuid == a_reader_guid); return proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`,
`handle_nackfrag`: Suche nach `remote_reader_guid`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy remoteReaderGuid

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "remoteReaderGuid: GUID_t — Identifies
the remote matched RTPS Reader that is represented by the ReaderProxy."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::remote_reader_guid`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::add_reader_proxy_is_idempotent_on_same_guid`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy remoteGroupEntityId

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "remoteGroupEntityId: EntityId_t —
Identifies the group to which the matched Reader belongs".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::remote_group_entity_id`, `with_remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (Match-Pfad, `PID_GROUP_GUID` der Subscription)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — der Proxy führt `remoteGroupEntityId`; der Match-Pfad füllt ihn aus `PID_GROUP_GUID` der SEDP-Ankündigung des Readers, `ENTITYID_UNKNOWN` ohne Gruppe.

### §8.4.7.5 Table 8.61 ReaderProxy unicastLocatorList

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "List of unicast locators … that can be
used to send messages to the matched RTPS Reader. The list may be empty."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::unicast_locators`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_every_unicast_locator_of_a_proxy`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy multicastLocatorList

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "List of multicast locators … that can
be used to send messages to the matched RTPS Reader. The list may be empty."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::multicast_locators`);
`crates/rtps/src/reliable_writer.rs` (`proxy_targets_of`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_proxy_with_multicast_locators_is_sent_to_at_the_multicast_locator`

**Status:** `done` — ein Proxy mit Multicast-Locatoren wird dort angesprochen (DATA und HEARTBEAT, ein Datagramm für die Gruppe), einer ohne an seinen Unicast-Locatoren. Der SEDP-Match-Pfad übergibt eine leere Liste („The list may be empty“); die Unicast-Locatoren des Readers gelten dann.

### §8.4.7.5 Table 8.61 ReaderProxy highestSentChangeSN

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "The highest sequence number of the
changes that have been sent to the matched RTPS Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy requestedChanges

**Spec:** §8.4.7.5, S. 84, Table 8.61 — "A list of sequence numbers
representing changes that were requested by the matched RTPS Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy acknowledgedChanges

**Spec:** §8.4.7.5, S. 84–85, Table 8.61 — "A list of sequence numbers
representing changes that have been acknowledged by the matched RTPS
Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_acked_sn`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`

**Status:** `done` — als höchste bestätigte Nummer geführt.

### §8.4.7.5 Table 8.61 ReaderProxy expectsInlineQos

**Spec:** §8.4.7.5, S. 85, Table 8.61 — "Specifies whether the remote matched
RTPS Reader expects in-line QoS to be sent along with any data."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`expects_inline_qos`,
`set_expects_inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy isActive

**Spec:** §8.4.7.5, S. 85, Table 8.61 — "isActive: bool — Specifies whether the
remote Reader is responsive to the Writer."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`note_activity`, `is_inactive`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::proxy_becomes_inactive_after_threshold_elapses`,
`crates/rtps/src/reader_proxy.rs::tests::proxy_is_active_after_note_activity`

**Status:** `done`

### §8.4.7.5 Matching folgt dem DDS-Match

**Spec:** §8.4.7.5, S. 85 — "The matching is a consequence of the match of the
corresponding DDS entities. That is, the DDS DataWriter matches a DDS
DataReader by Topic, has compatible QoS, and is not being explicitly ignored
by the application that uses DDS."

**Repo:** `crates/dcps/src/runtime.rs` (SEDP-Match, `add_reader_proxy`);
`crates/dcps/src/participant.rs` (`ignore_subscription`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`,
`crates/dcps/tests/xtypes_aware_match.rs::final_vs_appendable_mismatch_rejected_at_match_time`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy new

**Spec:** §8.4.7.5, S. 85 — "this.requested_changes := <empty>;
this.acknowledged_changes := <empty>; this.highest_sent_seq_num := 0;"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::new`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::fresh_proxy_has_zero_state`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy acked_changes_set

**Spec:** §8.4.7.5, S. 86 — "FOR_EACH seq_num <= committed_seq_num DO ADD
seq_num TO this.acknowledged_changes".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`acked_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`,
`crates/rtps/src/reader_proxy.rs::tests::acking_also_prunes_requested_changes`

**Status:** `done` — der Parameter ist `readerSNState.base`, die bestätigte
Obergrenze `base − 1`.

### §8.4.7.5 Table 8.62 ReaderProxy next_requested_change

**Spec:** §8.4.7.5, S. 86 — "return MIN( this.requested_changes() );"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`next_requested_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_requested_change_pulls_smallest_first`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy next_unsent_change

**Spec:** §8.4.7.5, S. 86 — "unsent_changes := { changes SUCH-THAT
change.sequenceNumber > this.higuest_sent_seq_num } … ELSE return MIN {
unsent_changes.sequenceNumber }".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`next_unsent_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`,
`crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_picks_up_after_cache_grows`

**Status:** `done` — liefert `highest_sent + 1` bis zum Cache-Maximum; nicht
mehr vorhandene Nummern beantwortet `tick` mit GAP.

### §8.4.7.5 Table 8.62 ReaderProxy requested_changes

**Spec:** §8.4.7.5, S. 86 — "return this.requested_changes".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`pending_requested_count`,
`next_requested_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy requested_changes_set

**Spec:** §8.4.7.5, S. 86 — "FOR_EACH seq_num IN req_seq_num_set DO ADD
seq_num TO this.requested_changes;"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done` — übernimmt jede angeforderte Nummer ab der Basis des
ACKNACK, auch eine schon bestätigte (§8.4.2.3).

### §8.4.7.5 Table 8.62 ReaderProxy unsent_changes

**Spec:** §8.4.7.5, S. 86 — "return ( this.next_unsent_change() !=
SEQUENCE_NUMBER_INVALID )".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn` gegen
Cache-Maximum)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_picks_up_after_cache_grows`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy unacked_changes

**Spec:** §8.4.7.5, S. 86 — "return ( highest_available_seq_num >
highest_acked_seq_num )".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`unacked_changes`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::unacked_changes_detects_gap`

**Status:** `done`

### §8.4.8.1 Table 8.63 Best-Effort-StatelessWriter T1

**Spec:** §8.4.8.1, S. 87–88, Table 8.63 T1 — "a_locator := new
ReaderLocator( locator, expectsInlineQos ); the_rtps_writer.reader_locator_add(
a_locator );"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::initial_peers`,
SPDP-Multicast-Locator)

**Tests:** `crates/dcps/src/runtime.rs::tests::initial_peer_specs_expand_per_the_configured_participant_limit`,
`crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done` — der SPDP-Beacon ist ZeroDDS' Best-Effort-StatelessWriter;
seine Ziel-Locatoren stammen aus der Konfiguration (§8.5.3.1).

### §8.4.8.1 Table 8.63 Best-Effort-StatelessWriter T2

**Spec:** §8.4.8.1, S. 88 — "triggered by the guard condition
[RL::unsent_changes() == true] … The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion; beobachtbar
ist T4.

### §8.4.8.1 Table 8.63 Best-Effort-StatelessWriter T3

**Spec:** §8.4.8.1, S. 88 — "[RL::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.8.1 Table 8.63 Best-Effort-StatelessWriter T4

**Spec:** §8.4.8.1, S. 88 — nächster ungesendeter Change; GAP für übersprungene
Nummern; "DATA = new DATA(a_change); IF (the_reader_locator.expectsInlineQos)
{ DATA.inlineQos := … } DATA.readerId := ENTITYID_UNKNOWN; sendto
the_reader_locator.locator, DATA;"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`: je
Ankündigung eine DATA mit fortlaufender Nummer)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done` — die DATA trägt `readerId =
ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR` statt UNKNOWN (§8.3.8.2 lässt
beides zu); da jede Nummer gesendet wird, entsteht kein GAP.

### §8.4.8.1 Table 8.63 Best-Effort-StatelessWriter T5

**Spec:** §8.4.8.1, S. 88–89 — "the_rtps_writer.reader_locator_remove(
the_reader_locator); delete the_reader_locator;"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — für SPDP sind die Locatoren fest
konfiguriert (§8.5.3.1); ein Entfernen durch die Discovery tritt nicht auf.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T1

**Spec:** §8.4.8.2, S. 90, Table 8.64 T1 — "a_locator := new ReaderLocator(
locator, expectsInlineQos ); the_rtps_writer.reader_locator_add( a_locator );"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — `reader_locator_add(locator, expects_inline_qos)`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T2

**Spec:** §8.4.8.2, S. 90 — "[RL::unsent_changes() == true] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — ungesendete Changes setzen den Push-Zustand `Pushing`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T3

**Spec:** §8.4.8.2, S. 90 — "[RL::unsent_changes == false] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — ohne ungesendete Changes geht die Push-Maschine nach `Announcing`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T4

**Spec:** §8.4.8.2, S. 91 — nächsten ungesendeten Change als DATA mit
`readerId := ENTITYID_UNKNOWN` senden, GAP für übersprungene Nummern;
Nachbedingung `higuest_sent_seq_num == a_change_seq_num`.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::a_gap_precedes_the_data_after_a_hole_in_the_cache`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_related_qos_goes_out_with_the_data_only_where_it_is_expected`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_resend_per_tick_is_bounded`

**Status:** `done` — der nächste ungesendete Change geht als DATA mit `readerId = ENTITYID_UNKNOWN` an den Locator, davor ein GAP mit `filteredCount = 0` über die fehlenden Nummern; Inline-QoS des zugehörigen DDS-Writers nur bei `expectsInlineQos`; `RL::can_send()` ist die Grenze `max_per_tick`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T5

**Spec:** §8.4.8.2, S. 91 — periodisch "HEARTBEAT := new
HEARTBEAT(the_rtps_writer.writerGuid, seq_num_min, seq_num_max);
HEARTBEAT.FinalFlag := SET; HEARTBEAT.readerId := ENTITYID_UNKNOWN;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_announcing_locator_gets_a_final_heartbeat_each_period`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::a_heartbeat_of_an_empty_cache_has_first_after_last`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_heartbeat_count_wraps_at_u32_max`

**Status:** `done` — HEARTBEAT mit FinalFlag gesetzt und `readerId = ENTITYID_UNKNOWN` je `heartbeatPeriod` an einen ankündigenden Locator; ein leerer Cache meldet `firstSN = lastSN + 1`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T6

**Spec:** §8.4.8.2, S. 91–92 — "FOREACH reply_locator_t IN {
Receiver.unicastReplyLocatorList, Receiver.multicastReplyLocatorList }
reader_locator := the_rtps_writer.reader_locator_lookup(reply_locator_t);
reader_locator.requested_changes_set(ACKNACK.readerSNState.set);"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_without_a_usable_reply_locator_is_counted`

**Status:** `done` — ein ACKNACK setzt für jeden Reply-Locator des Receiver-Zustands, der einen Port nennt und ReaderLocator ist, `requested_changes_set(readerSNState)`; ein ACKNACK ohne brauchbaren Reply-Locator (kein InfoReply) wird gezählt (`unmatched_acknacks`). `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T7

**Spec:** §8.4.8.2, S. 92 — "[RL::requested_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — angeforderte Changes setzen den Repair-Zustand `MustRepair`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T8

**Spec:** §8.4.8.2, S. 92 — "The transition performs the same logical actions
performed by Transition T6".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — ein weiterer ACKNACK in `MustRepair` verfährt wie T6. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T9

**Spec:** §8.4.8.2, S. 92 — "triggered by the firing of a timer indicating
that the duration of W::nackResponseDelay has elapsed since the state
must_repair was entered."

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — nach `nackResponseDelay` (`set_nack_response_delay`) geht die Maschine nach `Repairing`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T10 (DATA)

**Spec:** §8.4.8.2, S. 92 — "a_change_seq_num :=
the_reader_locator.next_requested_change(); … IF ( a_change != <nil> ) { DATA
= new DATA(a_change); … DATA.readerId := ENTITYID_UNKNOWN; sendto
the_reader_locator.locator, DATA; }".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_related_qos_goes_out_with_the_data_only_where_it_is_expected`

**Status:** `done` — `next_requested_change` geht als DATA mit `readerId = ENTITYID_UNKNOWN` an den Locator; Inline-QoS des DDS-Writers und des Changes bei `expectsInlineQos`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T10 (GAP)

**Spec:** §8.4.8.2, S. 92 — "ELSE { GAP = new GAP(a_change.sequenceNumber);
GAP.readerId := ENTITYID_UNKNOWN; GAP.filteredCount := 0; sendto
the_reader_locator.locator, GAP; }".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::a_requested_change_that_left_the_cache_is_answered_by_a_gap`

**Status:** `done` — ein angeforderter Change, der nicht mehr im Cache ist, wird mit einem GAP (`filteredCount = 0`) beantwortet. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T11

**Spec:** §8.4.8.2, S. 93 — "[RL::requested_changes() == <empty>] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — ohne angeforderte Changes geht die Maschine nach `Waiting`. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.8.2 Table 8.64 Reliable-StatelessWriter T12

**Spec:** §8.4.8.2, S. 93 — "the_rtps_writer.reader_locator_remove(
the_reader_locator); delete the_reader_locator;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — `reader_locator_remove` löscht den ReaderLocator. `ReliableStatelessWriter` wird von keinem Produktionspfad verwendet; kein Hersteller ist gemessen.

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T1

**Spec:** §8.4.9.1, S. 94, Table 8.65 T1 — "a_reader_proxy := new ReaderProxy(
remoteReaderGuid, remoteGroupEntityId, expectsInlineQos, unicastLocatorList,
multicastLocatorList); the_rtps_writer.matched_reader_add(a_reader_proxy);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy` mit
`ReaderProxy::new(.., is_reliable: false)`); `crates/rtps/src/reader_proxy.rs` (`remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (Match-Pfad)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_gets_the_next_write`,
`crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_reader_add` nimmt `remoteGroupEntityId`, `expectsInlineQos` und beide Locator-Listen.

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T2

**Spec:** §8.4.9.1, S. 94 — "[RP::unsent_changes() == true] … Note that for a
Best-Effort Writer, W::pushMode == true … The transition performs no logical
actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T3

**Spec:** §8.4.9.1, S. 94 — "[RP::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T4

**Spec:** §8.4.9.1, S. 94–95 — nächster ungesendeter Change; GAP für
übersprungene Nummern mit filteredCount 0; DATA bei bestandenem DDS_FILTER,
sonst GAP mit filteredCount 1; Inline-QoS bei expectsInlineQos;
Nachbedingung `higuest_sent_seq_num == a_change_seq_num`.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`,
`tick`, `append_change`, `is_relevant`, `passes_dds_filter`, `gap_filtered_count`);
`crates/rtps/src/reader_proxy.rs` (`ChangeFilter`, `with_filter`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_gets_the_next_write`,
`crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds`,
`crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_behind_is_pushed_gaps_with_the_filtered_count`,
`crates/rtps/src/reliable_writer.rs::tests::a_change_that_fails_the_readers_filter_is_a_gap_with_filtered_count_one`,
`crates/rtps/src/reliable_writer.rs::tests::a_gap_for_a_number_the_cache_no_longer_holds_has_filtered_count_zero`,
`crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done` — ein Proxy im Schritt erhält jeden neuen Change als DATA; ein zurückliegender erhält in `tick` jeden noch nicht gesendeten Change als DATA, eine Nummer, die der Cache nicht mehr hat, als GAP mit `filteredCount` 0. Besteht ein Change `DDS_FILTER(the_reader_proxy, a_change)` nicht (`ReaderProxy::with_filter`), geht ein GAP mit `filteredCount` 1 statt des DATA. Die Laufzeit setzt keinen Filter (`DDS_FILTER` darf eine leere Operation sein, §8.2.10.1); das Verhalten gegenüber Cyclone DDS, Fast DDS und OpenDDS ist nicht gemessen.

### §8.4.9.1 Best-Effort-StatefulWriter: zurückliegender Proxy

**Spec:** §8.4.9.1, S. 95 — T4 sendet `next_unsent_change()` an jeden Proxy,
"if ( a_change_seq_num > the_reader_proxy.higuest_sent_seq_num +1 ) { GAP = …
}".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `next_unsent_change` je
Best-Effort-Proxy, `append_change`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds`

**Status:** `done` — ein zurückliegender Best-Effort-Proxy erhält die noch nicht
gesendeten Changes der Reihe nach, ohne Anforderung; eine Nummer, die der Cache
nicht mehr hat, als GAP.

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T5

**Spec:** §8.4.9.1, S. 95–96 — "ADD a_change TO the_rtps_writer.writer_cache;"
Nachbedingung "FOREACH proxy IN the_rtps_writer.matched_readers
proxy.unsent_changes() == true".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.9.1 Table 8.65 Best-Effort-StatefulWriter T6

**Spec:** §8.4.9.1, S. 96 — "the_rtps_writer.matched_reader_remove(
the_reader_proxy); delete the_reader_proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T1

**Spec:** §8.4.9.2, S. 98, Table 8.66 T1 — "a_reader_proxy := new ReaderProxy(
remoteReaderGuid, remoteGroupEntityId, expectsInlineQos, unicastLocatorList,
multicastLocatorList); the_rtps_writer.matched_reader_add(a_reader_proxy);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy`);
`crates/rtps/src/reader_proxy.rs` (`remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (Match-Pfad, `skip_samples_up_to` für VOLATILE)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_after_add_reader_proxy_is_non_final`,
`crates/rtps/src/reliable_writer.rs::tests::a_reader_matched_inside_an_open_coherent_set_gets_the_whole_set`,
`crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_reader_add` nimmt `remoteGroupEntityId`, `expectsInlineQos` und beide Locator-Listen.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T2

**Spec:** §8.4.9.2, S. 98–99 — "[RP::unsent_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T3

**Spec:** §8.4.9.2, S. 99 — "[RP::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T4

**Spec:** §8.4.9.2, S. 99–100 — nächster ungesendeter Change als DATA (oder
GAP bei DDS_FILTER = false, filteredCount 1), GAP für aus dem Cache entfernte
Nummern (filteredCount 0); "DATA.readerId := ENTITYID_UNKNOWN; // or
ReaderProxy.entityId".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`,
`write_with_heartbeat_stamped`; `tick` für zurückliegende Proxies)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/reliable_writer.rs::tests::repairs_in_order_bring_a_lagging_proxy_into_step`,
`crates/rtps/src/reliable_writer.rs::tests::a_reliable_proxy_behind_the_cache_is_pushed_into_step`,
`crates/rtps/src/reliable_writer.rs::tests::a_repair_out_of_order_is_followed_by_the_changes_before_it`,
`crates/rtps/src/reliable_writer.rs::tests::a_reader_asking_for_a_replaced_change_gets_the_current_one`

**Status:** `done` — `write` sendet einen Change sofort an Proxies, die im
Schritt sind; einem zurückliegenden Proxy (Late Joiner, oder einer, dessen
Anfrage ein GAP beantwortet hat) schickt `tick` unter pushMode die
ungesendeten Changes der Reihe nach, DATA für die im Cache, GAP für die
entfernten. Ein in derselben Runde außer der Reihe reparierter Change geht
nicht zweimal hinaus.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T5

**Spec:** §8.4.9.2, S. 100 — "[RP::unacked_changes() == false] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: kein periodischer
HEARTBEAT ohne unbestätigte Changes)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_acknowledged_the_cache_gets_no_periodic_heartbeat`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T6

**Spec:** §8.4.9.2, S. 100 — "[RP::unacked_changes() == true] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `announcing`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_heartbeat_sent_with_a_change_is_answered_by_the_next_acknack`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T7

**Spec:** §8.4.9.2, S. 100 — "HEARTBEAT := new
HEARTBEAT(the_rtps_writer.writerGuid, seq_num_min, seq_num_max);
HEARTBEAT.FinalFlag := NOT_SET; HEARTBEAT.readerId := ENTITYID_UNKNOWN; send
HEARTBEAT;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`, `append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::periodic_heartbeat_has_final_flag_unset`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done` — der HEARTBEAT geht je Proxy mit dessen `readerId`;
`firstSN` ist `max(seq_num_min, highest_acked + 1)` des Proxys.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T8

**Spec:** §8.4.9.2, S. 100 — "the_rtps_writer.acked_changes_set(
ACKNACK.readerSNState.base - 1); the_reader_proxy.requested_changes_set(
ACKNACK.readerSNState.set);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_updates_proxy_state`,
`crates/rtps/src/reliable_writer.rs::tests::handle_acknack_with_lower_base_leaves_requested`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T9

**Spec:** §8.4.9.2, S. 100 — "[RP::requested_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T10

**Spec:** §8.4.9.2, S. 101 — "The transition performs the same logical actions
as Transition T8".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_full_nack_window_gets_a_heartbeat_with_its_repair`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T11

**Spec:** §8.4.9.2, S. 101 — "triggered by the firing of a timer indicating
that the duration of W::nackResponseDelay has elapsed since the state
must_repair was entered."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`repair_due`, `note_nack`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T12

**Spec:** §8.4.9.2, S. 101–102 — angeforderten Change aus dem Cache als DATA
(oder GAP mit filteredCount 1 bei DDS_FILTER = false), sonst "GAP = new
GAP(a_change.sequenceNumber, the_reader_proxy.remoteReaderGuid);
GAP.filteredCount := 0;"; Nachbedingung: der Change ist nicht mehr
angefordert.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `append_data`,
`append_lifecycle`, `append_gap`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_repair_reproduces_the_key`

**Status:** `done` — der GAP enthält kein filteredCount-Feld; das entspricht
einem nicht gezählten Change.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T13

**Spec:** §8.4.9.2, S. 102 — "[RP::requested_changes() == <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T14

**Spec:** §8.4.9.2, S. 102 — "ADD a_change TO the_rtps_writer.writer_cache;"
Nachbedingung: jeder Proxy hat ungesendete Changes.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T15

**Spec:** §8.4.9.2, S. 102 — "triggered by the removal of a CacheChange
'a_change' from the HistoryCache of the RTPS Writer".

**Repo:** `crates/rtps/src/reliable_writer.rs` (KEEP_LAST-Verdrängung in
`stage_sample`, `remove_acknowledged`, `remove_samples_up_to`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_stalled_reader_does_not_block_fresh_writes`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.4.9.2 Table 8.66 Reliable-StatefulWriter T16

**Spec:** §8.4.9.2, S. 102 — "the_rtps_writer.matched_reader_remove(
the_reader_proxy); delete the_reader_proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`,
`crates/rtps/src/reliable_writer.rs::tests::the_unacked_window_fills_and_acknowledgements_free_it`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader heartbeatResponseDelay

**Spec:** §8.4.10.1, S. 103, Table 8.67 — "heartbeatResponseDelay: Protocol
tuning parameter that allows the RTPS Reader to delay the sending of a
positive or negative acknowledgment".

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`ReliableReaderConfig::heartbeat_response_delay`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`

**Status:** `done` — die Einstellbarkeit für den Anwender ist unter §8.4.2.1
geführt.

### §8.4.10.1 Table 8.67 Reader heartbeatSuppressionDuration

**Spec:** §8.4.10.1, S. 103, Table 8.67 — "heartbeatSuppressionDuration:
Protocol tuning parameter that allows the RTPS Reader to ignore HEARTBEATs
that arrive 'too soon' after a previous HEARTBEAT was received."

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`set_heartbeat_suppression_duration`, `handle_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_suppression_duration_ignores_a_heartbeat_that_arrives_too_soon`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader reader_cache

**Spec:** §8.4.10.1, S. 103, Table 8.67 — "reader_cache: History Cache —
Contains the history of CacheChange changes for this RTPS Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`WriterProxyState::received_cache`);
`crates/dcps/src/subscriber.rs` (Sample-Speicher des DataReader)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`,
`crates/rtps/src/reliable_reader.rs::tests::a_full_cache_takes_the_change_in_front_of_what_it_holds`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader expectsInlineQos

**Spec:** §8.4.10.1, S. 104, Table 8.67 — "expectsInlineQos: bool — Specifies
whether the RTPS Reader expects in-line QoS to be sent along with any data."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::expects_inline_qos`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §8.4.10.1 Default heartbeatResponseDelay = 500 ms

**Spec:** §8.4.10.1, S. 104 — "heartbeatResponseDelay.sec = 0;
heartbeatResponseDelay.nanosec = 500 * 1000 * 1000; // 500 milliseconds".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DEFAULT_HEARTBEAT_RESPONSE_DELAY`, `SPEC_HEARTBEAT_RESPONSE_DELAY`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::reader_heartbeat_response_delay`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_HEARTBEAT_RESPONSE_DELAY` und `RuntimeConfig::reader_heartbeat_response_delay` sind 500 ms; `reader.heartbeat_response_delay_ms: 0` antwortet im nächsten Tick.

### §8.4.10.1 Default heartbeatResponseDelay

**Spec:** §8.4.10.1, S. 104 — "The following timing-related values are used as
the defaults in order to facilitate 'out-of-the-box' interoperability between
implementations."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DEFAULT_HEARTBEAT_RESPONSE_DELAY`, `DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`);
`crates/dcps/src/runtime.rs`; `crates/dcps/src/type_lookup_transport.rs`; `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`

**Status:** `done` — die Werte der Spec sind die Vorgaben: heartbeatResponseDelay 500 ms, heartbeatSuppressionDuration 0 (Laufzeit, Man-Seite, Beispielkonfiguration). Die früheren 0 ms bleiben über `reader.heartbeat_response_delay_ms` erreichbar. §8.4.7.1 nennt keine heartbeatPeriod; sie bleibt 100 ms (`writer.heartbeat_period_ms`). Die eingebauten Reader von SEDP, Secure-Volatile und TypeLookup nutzen dieselbe Vorgabe.

### §8.4.10.1 Default heartbeatSuppressionDuration = 0

**Spec:** §8.4.10.1, S. 104 — "heartbeatSuppressionDuration.sec = 0;
heartbeatSuppressionDuration.nanosec = 0;".

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`,
`crates/rtps/src/reliable_reader.rs::tests::heartbeat_storm_from_a_writer_that_never_fulfils_a_nack_cannot_arise`,
`crates/discovery/src/sedp/reader.rs::tests::sedp_reader_does_not_storm_against_a_writer_that_never_delivers`

**Status:** `done` — alle Reader laufen mit 0, auch die eingebauten von SEDP
und VolatileSecure. Ein Writer, der dieselbe Spanne in kurzen Abständen neu
ankündigt (HDDS 1.4.8: ein HEARTBEAT `[1,1]` etwa alle 1,5 ms), bekommt eine
Antwort je heartbeatResponseDelay: die Verzögerung läuft ab dem ersten
HEARTBEAT, der eine Antwort verlangt.

### §8.4.10.1 Table 8.68 Reader new

**Spec:** §8.4.10.1, S. 104 — Übernahme von guid, Locator-Listen,
reliabilityLevel, topicKind, expectsInlineQos, heartbeatResponseDelay;
"this.reader_cache := new HistoryCache;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::new`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`

**Status:** `done`

### §8.4.10.2 Table 8.69 StatelessReader new

**Spec:** §8.4.10.2, S. 104–105 — "The RTPS StatelessReader has no knowledge
of the number of matched writers, nor does it maintain any state for each
matched RTPS Writer."

**Repo:** `crates/rtps/src/reader.rs` (`BestEffortReader::new`);
`crates/discovery/src/spdp.rs` (`SpdpReader::new`)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_delivers_data_addressed_to_us`,
`crates/discovery/src/spdp.rs::tests::reader_rejects_non_spdp_datagram`

**Status:** `done`

### §8.4.10.3 Table 8.70 StatefulReader matched_writers

**Spec:** §8.4.10.3, S. 105, Table 8.70 — "matched_writers: WriteProxy[*] —
Used to maintain state on the remote Writers matched up with the Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`writer_proxies`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader new

**Spec:** §8.4.10.3, S. 105–106 — "this.matched_writers := <empty>;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::new`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::no_pre_emptive_acknack_without_proxy`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_add

**Spec:** §8.4.10.3, S. 106 — "ADD a_writer_proxy TO {this.matched_writers};"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`,
`crates/rtps/src/reliable_reader.rs::tests::re_adding_known_writer_preserves_reliability_state`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_remove

**Spec:** §8.4.10.3, S. 106 — "REMOVE a_writer_proxy FROM
{this.matched_writers}; delete a_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`,
`depart_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_departed_writer_leaves_its_held_samples_with_the_reader`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_lookup

**Spec:** §8.4.10.3, S. 106 — "FIND proxy IN this.matched_writers SUCH-THAT
(proxy.remoteWriterGuid == a_writer_guid); return proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`proxy_index_by_writer`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.4.10.4 WriterProxy folgt dem DDS-Match

**Spec:** §8.4.10.4, S. 106 — "The association is a consequence of the
matching of the corresponding DDS Entities … the DDS DataReader matching a DDS
DataWriter by Topic, having compatible QoS, belonging to a common partition,
and not being explicitly ignored by the application that uses DDS."

**Repo:** `crates/dcps/src/runtime.rs` (Match-Pfad, `add_writer_proxy`);
`crates/dcps/src/participant.rs` (`ignore_publication`)

**Tests:** `crates/dcps/src/runtime.rs::tests::partition_mismatch_blocks_match_without_any_incompatible_qos_event`,
`crates/dcps/src/participant.rs::tests::ignore_publication_records_handle`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy remoteWriterGuid

**Spec:** §8.4.10.4, S. 106, Table 8.72 — "remoteWriterGuid: GUID_t —
Identifies the matched Writer."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::remote_writer_guid`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy unicastLocatorList

**Spec:** §8.4.10.4, S. 106, Table 8.72 — "List of unicast (address, port)
combinations that can be used to send messages to the matched Writer or
Writers. The list may be empty."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::unicast_locators`);
`crates/rtps/src/reliable_reader.rs` (`tick_outbound`: ACKNACK an diese
Locatoren)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::tick_emits_one_acknack_per_writer_with_missing`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy multicastLocatorList

**Spec:** §8.4.10.4, S. 106, Table 8.72 — "List of multicast (address, port)
combinations that can be used to send messages to the matched Writer or
Writers. The list may be empty."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::multicast_locators`,
`refresh_locators`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::re_adding_known_writer_preserves_reliability_state`

**Status:** `done` — die Liste wird geführt; ACKNACKs gehen an die
Unicast-Locatoren.

### §8.4.10.4 Table 8.72 WriterProxy dataMaxSizeSerialized

**Spec:** §8.4.10.4, S. 106, Table 8.72 — "Optional attribute that indicates
the maximum size of any SerializedPayload that may be sent by the matched
Writer."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`set_writer_data_max_size`, `oversize_payload_count`);
`crates/dcps/src/runtime.rs` (Verdrahtung beim Anlegen des Writer-Proxys, `reader.honor_data_max_size_serialized`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_payload_beyond_the_announced_maximum_is_refused_and_counted`,
`crates/dcps/tests/runtime_config_layering.rs::the_data_max_size_setting_is_on_by_default_and_read_from_yaml`

**Status:** `done` — der Reader übernimmt das Attribut aus der Ankündigung und weist ein SerializedPayload darüber ab (nach dem Öffnen eines geschützten Payloads; die Nummer gilt als nicht verfügbar, der Zähler `oversize_payload_count` steigt). Ein DATA_FRAG-Sample wird nach dem Zusammensetzen geprüft; `AssemblerCaps` begrenzt weiter die Größe zusammengesetzter Samples. Die benannte Einstellung `reader.honor_data_max_size_serialized=false` nimmt den Payload trotzdem an, für einen Writer, der seine eigene Ankündigung überschreitet (Fast DDS 3.6.1: 160 Oktette angekündigt, 100 036 gesendet).

### §8.4.10.4 Table 8.72 WriterProxy changes_from_writer

**Spec:** §8.4.10.4, S. 106, Table 8.72 — "List of CacheChange changes
received or expected from the matched RTPS Writer."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`known_up_to`, `received`,
`irrelevant`, `first_available_sn`, `last_available_sn`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::the_known_run_answers_as_the_sets_of_every_sn_did`

**Status:** `done` — als Zählerstände und Mengen abgebildet (§8.4.15.1).

### §8.4.10.4 Table 8.72 WriterProxy remoteGroupEntityId

**Spec:** §8.4.10.4, S. 107, Table 8.72 — "remoteGroupEntityId: EntityId_t —
Identifies the group to which the matched Reader belongs".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::remote_group_entity_id`, `with_remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (Match-Pfad, `PID_GROUP_GUID` der Publication)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — der Proxy führt `remoteGroupEntityId`; der Match-Pfad füllt ihn aus `PID_GROUP_GUID` der SEDP-Ankündigung des Writers, `ENTITYID_UNKNOWN` ohne Gruppe.

### §8.4.10.4 Table 8.73 WriterProxy new

**Spec:** §8.4.10.4, S. 107 — "this.changes_from_writer := <all past and future
samples from the writer>; … initialized to UNKNOWN".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::new`:
`first_available_sn = 1`, `last_available_sn = 0`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::fresh_proxy_has_no_missing`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy available_changes_max

**Spec:** §8.4.10.4, S. 108 — "seq_num := MAX { change.sequenceNumber
SUCH-THAT ( change IN this.changes_from_writer AND ( change.status ==
RECEIVED OR change.status == NOT_AVAILABLE) ) }" unter der Bedingung, dass
keine kleinere Nummer MISSING oder UNKNOWN ist.

**Repo:** `crates/rtps/src/writer_proxy.rs` (`available_changes_max`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::available_changes_max_stops_at_the_first_hole`,
`crates/rtps/src/writer_proxy.rs::tests::available_changes_max_passes_over_an_irrelevant_change`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy not_available_change_set

**Spec:** §8.4.10.4, S. 108 — "IF ( filteredCount == COUNT(this.
a_seq_num_set) ) THEN change.status := NA_FILTERED; ELSE IF ( filteredCount ==
0) change.status := NA_REMOVED; ELSE change.status := NA_UNSPECIFIED;"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`NotAvailableKind`, `WriterProxy::not_available_change_set`,
`not_available_count`, `change_status`); `crates/rtps/src/reliable_reader.rs` (`handle_gap`, `not_available_count`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`,
`crates/rtps/src/writer_proxy.rs::tests::change_status_is_kept_per_change_with_the_sub_kind`

**Status:** `done` — `filteredCount` gleich der Größe der Menge ergibt NA_FILTERED, 0 ergibt NA_REMOVED, dazwischen NA_UNSPECIFIED; der Status steht je Änderung. Ein GAP ohne FilteredCountFlag zählt als 0 (Annahme: das Feld fehlt bei Peers vor 2.5, deren GAP nur entfernte Änderungen meldet; nicht gegen einen Hersteller gemessen). Eine bereits bekannte Nummer wird nicht erneut gezählt oder umgesetzt.

### §8.4.10.4 Table 8.73 WriterProxy lost_changes_update

**Spec:** §8.4.10.4, S. 108 — "FOREACH change IN this.changes_from_writer
SUCH-THAT ( change.status == UNKNOWN OR change.status == MISSING AND seq_num
< first_available_seq_num ) DO { change.status := NA_REMOVED; }".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`lost_changes_update`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_counts_what_was_neither_received_nor_irrelevant`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_retires_everything_below_the_new_base`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy missing_changes

**Spec:** §8.4.10.4, S. 109 — "return { change IN this.changes_from_writer
SUCH-THAT change.status == MISSING};"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`missing_changes`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::missing_changes_respects_max_count`,
`crates/rtps/src/writer_proxy.rs::tests::received_removes_from_missing`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy missing_changes_update

**Spec:** §8.4.10.4, S. 109 — "FOREACH change IN this.changes_from_writer
SUCH-THAT ( change.status == UNKNOWN AND seq_num <= last_available_seq_num )
DO { change.status := MISSING; }".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`update_from_heartbeat`:
`last_available_sn`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::heartbeat_sets_available_range`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy received_change_set

**Spec:** §8.4.10.4, S. 109 — "FIND change FROM this.changes_from_writer
SUCH-THAT change.sequenceNumber == a_seq_num; change.status := RECEIVED".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`received_change_set`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::received_removes_from_missing`,
`crates/rtps/src/writer_proxy.rs::tests::received_before_first_is_ignored`

**Status:** `done`

### §8.4.10.5 Table 8.74 ChangeFromWriter status

**Spec:** §8.4.10.5, S. 109–110, Table 8.74 — "status: ChangeFromWriter
StatusKind — Indicates the status of a CacheChange relative to the RTPS
Writer represented by the WriterProxy." Werte UNKNOWN, MISSING, RECEIVED,
NOT_AVAILABLE mit den Unterarten NA_FILTERED, NA_REMOVED, NA_UNSPECIFIED.

**Repo:** `crates/rtps/src/writer_proxy.rs` (`ChangeFromWriterStatus`, `change_status`, `is_known`, `missing_changes`,
`not_available_count`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::change_status_is_kept_per_change_with_the_sub_kind`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_sets_na_removed_per_change`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`

**Status:** `done` — die vier Hauptwerte und die drei Unterarten von NOT_AVAILABLE werden je Änderung geführt (Item unter §8.4.6 Table 8.52 ChangeFromWriterStatusKind).

### §8.4.10.5 Table 8.74 ChangeFromWriter is_relevant

**Spec:** §8.4.10.5, S. 109, Table 8.74 — "is_relevant: bool — Indicates
whether the change is relevant to the RTPS Reader. The determination of
relevant changes is affected by DDS DataReader TIME_BASED_FILTER QoS and also
by the use of DDS ContentFilteredTopics."

**Repo:** `crates/rtps/src/history_cache.rs` (`ChangeKind::is_relevant`,
`ChangeKind::AliveFiltered`); `crates/dcps/src/runtime.rs`
(`tbf_should_deliver`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::change_kind_alive_filtered_is_alive_but_not_relevant`

**Status:** `done`

### §8.4.11.1 Table 8.75 Best-Effort-StatelessReader T1

**Spec:** §8.4.11.1, S. 110–111, Table 8.75 T1 — "triggered by the creation of
an RTPS StatelessReader … The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.11.1 Table 8.75 Best-Effort-StatelessReader T2

**Spec:** §8.4.11.1, S. 111 — "a_change := new CacheChange(DATA);
the_rtps_reader.reader_cache.add_change(a_change);"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpReader::samples`);
`crates/rtps/src/reader.rs` (`BestEffortReader::recv_datagram`)

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_reads_every_participant_a_datagram_bundles`,
`crates/rtps/src/reader.rs::tests::reader_handles_multiple_data_in_one_datagram`

**Status:** `done`

### §8.4.11.1 Table 8.75 Best-Effort-StatelessReader T3

**Spec:** §8.4.11.1, S. 111 — "triggered by the destruction of an RTPS Reader
… The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.11.2 Reliable StatelessReader nicht unterstützt

**Spec:** §8.4.11.2, S. 111 — "This combination is not supported by the RTPS
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung; ZeroDDS hat keinen
reliable StatelessReader.

### §8.4.12.1 Table 8.76 Best-Effort-StatefulReader T1

**Spec:** §8.4.12.1, S. 112, Table 8.76 T1 — "a_writer_proxy := new
WriterProxy(remoteWriterGuid, remoteGroupEntityId, unicastLocatorList,
multicastLocatorList); the_rtps_reader.matched_writer_add(a_writer_proxy);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`,
`set_best_effort`); `crates/rtps/src/writer_proxy.rs` (`remote_group_entity_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_writer_add` nimmt `remoteGroupEntityId` und beide Locator-Listen.

### §8.4.12.1 Table 8.76 Best-Effort-StatefulReader T2

**Spec:** §8.4.12.1, S. 113 — "expected_seq_num :=
writer_proxy.available_changes_max() + 1; if ( a_change.sequenceNumber >=
expected_seq_num ) { … add_change(a_change); received_change_set(…); if (
a_change.sequenceNumber > expected_seq_num ) { lost_changes_update(…) } }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`:
`gap_skipped`, `lost_changes_update` bei `best_effort`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/reliable_reader.rs::tests::duplicate_data_is_rejected`,
`crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`

**Status:** `done`

### §8.4.12.1 Table 8.76 Best-Effort-StatefulReader T3

**Spec:** §8.4.12.1, S. 113 — "the_rtps_reader.matched_writer_remove(
the_writer_proxy); delete the_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`

**Status:** `done`

### §8.4.12.1 Table 8.76 Best-Effort-StatefulReader T4

**Spec:** §8.4.12.1, S. 113 — "the_writer_proxy.not_available_change_set(GAP.
sequence_number_set, GAP.filteredCount);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — die Nummern des GAP werden mit der Unterart nach `filteredCount` als nicht verfügbar markiert und je Änderung gespeichert.

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T1

**Spec:** §8.4.12.2, S. 114–115, Table 8.77 T1 — "a_writer_proxy := new
WriterProxy(remoteWriterGuid, remoteGroupEntityId, unicastLocatorList,
multicastLocatorList); the_rtps_reader.matched_writer_add(a_writer_proxy);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`); `crates/rtps/src/writer_proxy.rs` (`remote_group_entity_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`,
`crates/rtps/src/reliable_reader.rs::tests::changes_a_writer_sent_before_the_match_are_requested_at_once`,
`crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_writer_add` nimmt `remoteGroupEntityId` und beide Locator-Listen.

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T2

**Spec:** §8.4.12.2, S. 114 — HEARTBEAT empfangen: "if (HB.FinalFlag ==
NOT_SET) then must_send_ack else if (HB.LivelinessFlag == NOT_SET) then
may_send_ack else waiting".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T3

**Spec:** §8.4.12.2, S. 115 — "[W::missing_changes() == <empty>] … The
transition performs no logical actions."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`: keine
ACKNACK bei F = 1 ohne Fehlendes)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_without_missing_and_final_schedules_no_acknack`

**Status:** `done`

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T4

**Spec:** §8.4.12.2, S. 115 — "[W::missing_changes() != <empty>] … The
transition performs no logical actions."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`:
`has_missing` setzt `pending_acknack_since`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::final_heartbeat_from_unknown_to_data_writer_requests_the_missing_sn`

**Status:** `done`

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T5

**Spec:** §8.4.12.2, S. 115–116 — nach heartbeatResponseDelay
"missing_seq_num_set.base := the_writer_proxy.available_changes_max() + 1; …
send ACKNACK(missing_seq_num_set);" — bei zu vielen fehlenden Nummern "the
subset with smallest value of the sequence number".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`tick_outbound`,
`build_acknack_datagram`); `crates/rtps/src/submessages.rs`
(`SequenceNumberSet::from_missing`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`,
`crates/rtps/src/submessages.rs::tests::snset_from_missing_spread_wide_stays_within_256_bits`,
`crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done` — Basis und kleinste Teilmenge stimmen; die Menge enthält nur Nummern, von denen kein Fragment vorliegt (die übrigen fordert NACK_FRAG an, §8.4.14.1).

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T6

**Spec:** §8.4.12.2, S. 116 — "Similar to T1 … The transition performs no
logical actions in the virtual machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übergang ohne logische Aktion.

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T7

**Spec:** §8.4.12.2, S. 116 — "the_writer_proxy.missing_changes_update(
HEARTBEAT.lastSN); the_writer_proxy.lost_changes_update(HEARTBEAT.firstSN,
true);"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`update_from_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T8

**Spec:** §8.4.12.2, S. 116 — "a_change := new CacheChange(DATA);
the_reader.reader_cache.add_change(a_change);
the_writer_proxy.received_change_set(a_change.sequenceNumber);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`,
`store_received`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`,
`crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`

**Status:** `done`

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T9

**Spec:** §8.4.12.2, S. 116 — "the_writer_proxy.not_available_change_set(GAP.
sequence_number_set, GAP.filteredCount);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — `filteredCount` wird ausgewertet (Item unter §8.4.10.4 Table 8.73 WriterProxy not_available_change_set); die Nummern stehen je Änderung als NOT_AVAILABLE.

### §8.4.12.2 Table 8.77 Reliable-StatefulReader T10

**Spec:** §8.4.12.2, S. 116 — "the_rtps_reader.matched_writer_remove(
the_writer_proxy); delete the_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`,
`depart_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_departed_writer_counts_what_is_missing_in_front_of_its_held_samples`

**Status:** `done`

### §8.4.12.3 ChangeFromWriter illustrated

**Spec:** §8.4.12.3, S. 117–118, Figure 8.25 — Zustände Unknown, Missing,
NotRequested, Requested, Received, NotAvailable (Filtered, Removed,
Unspecified) und die auslösenden Ereignisse.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Veranschaulichung der Übergänge aus
§8.4.10.4 und §8.4.12.2 ("It is illustrative"); keine eigene Anforderung.

### §8.4.13 Writer Liveliness Protocol: Unterstützung verpflichtend

**Spec:** §8.4.13, S. 118 — "All implementations must support the Writer
Liveliness Protocol in order to be interoperable."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::wlp`)

**Tests:** `crates/dcps/tests/wlp_integration.rs::linux_only::two_participants_exchange_automatic_wlp_heartbeats`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_exchange_wlp_heartbeat`,
`crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — Writer und Reader des Protokolls laufen in jedem Teilnehmer; der Writer hat die QoS aus §8.4.13.3 und behandelt den Reader nach `builtinEndpointQos`. Die Wire-Änderung des Writers ist gegen Cyclone, Fast DDS, OpenDDS und RTI nicht gemessen.

### §8.4.13.1 Assoziation mit den Builtin-Endpunkten eines entdeckten Teilnehmers

**Spec:** §8.4.13.1, S. 118 — "once a Participant knows of the presence of
another Participant, it can assume the presence of the built-in Endpoints
made available by the remote Participant and establish the association with
the locally matching built-in Endpoints."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::add_peer_reader`);
`crates/dcps/src/runtime.rs` (`spdp_announced`, `wlp_unicast_targets`,
`answer_wlp_heartbeats`)

**Tests:** `crates/dcps/src/runtime.rs::tests::wlp_unicast_targets_prefers_metatraffic_then_default`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`,
`crates/dcps/src/wlp.rs::tests::only_a_participant_with_a_message_reader_gets_a_proxy`

**Status:** `done` — aus jeder SPDP-Ankündigung entsteht (unverändert: bleibt) ein Reader-Proxy
für den BuiltinParticipantMessageReader des Teilnehmers, wenn dessen
`availableBuiltinEndpoints` ihn nennen; Ziel sind seine Metatraffic-Locatoren.

### §8.4.13.1 Dasselbe Protokoll wie für Anwendungs-Endpunkte

**Spec:** §8.4.13.1, S. 118 — "The protocol used to communicate between
built-in Endpoints is the same as used for application-defined Endpoints."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`,
`crates/dcps/src/wlp.rs::tests::the_writer_heartbeats_until_the_reader_has_acknowledged`

**Status:** `done` — der WLP-Writer ist ein `ReliableWriter` mit einem Reader-Proxy je
Teilnehmer: DATA mit Piggyback-HEARTBEAT, periodische HEARTBEATs bis zur
Bestätigung, Wiederholung nur des Angeforderten, GAP für überholte Nummern. Der
Reader beantwortet HEARTBEATs (`heartbeat_responses`) und führt je Writer die
höchste empfangene Sequenznummer.

### §8.4.13.2 Builtin-Endpunkte BuiltinParticipantMessageWriter und -Reader

**Spec:** §8.4.13.2, S. 118 — "The RTPS Protocol reserves the following values
of the EntityId_t for these built-in Endpoints:
ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER
ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER`,
`EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER`); `crates/dcps/src/wlp.rs`
(`WlpEndpoint::handle_datagram`)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`,
`crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_ignores_non_wlp_traffic`

**Status:** `done` — die Werte sind unter §9.3.1.3 geführt.

### §8.4.13.3 Writer durability.kind = TRANSIENT_LOCAL

**Spec:** §8.4.13.3, S. 118 — "both the BuiltinParticipantMessageWriter and
BuiltinParticipantMessageReader shall use the following QoS values:
durability.kind = TRANSIENT_LOCAL_DURABILITY".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`

**Status:** `done` — ein später hinzukommender Reader erhält die gehaltenen Proben mit dem ersten Tick, ohne sie anzufordern (die Historie hält eine Probe je Art, eine Anforderung träfe sonst eine neuere Probe), und ein HEARTBEAT sagt, was die Historie hält; fordert er sie erneut an, erhält er sie noch einmal.

### §8.4.13.3 Writer history KEEP_LAST, depth 1

**Spec:** §8.4.13.3, S. 118–119 — "history.kind = KEEP_LAST_HISTORY_QOS",
"history.depth = 1".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_history_holds_the_last_sample_of_each_kind`

**Status:** `done` — die Historie hält die letzte Probe je Instanz (participantGuidPrefix,
kind; `PID_KEY_HASH` in der In-line-QoS); überholte Proben werden nicht wiederholt.

### §8.4.13.3 Reader durability.kind = TRANSIENT_LOCAL

**Spec:** §8.4.13.3, S. 118 — "both the BuiltinParticipantMessageWriter and
BuiltinParticipantMessageReader shall use the following QoS values:
durability.kind = TRANSIENT_LOCAL_DURABILITY".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::heartbeat_responses`: die
Anforderung beginnt bei firstSN des HEARTBEAT, nicht beim Match)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`,
`crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done`

### §8.4.13.3 Reader history KEEP_LAST, depth 1

**Spec:** §8.4.13.3, S. 118–119 — "history.kind = KEEP_LAST_HISTORY_QOS",
"history.depth = 1".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::peers`: je Teilnehmer der
zuletzt empfangene Stand)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`

**Status:** `done` — der Reader hält keine Proben, sondern übergibt jede
empfangene Probe sofort an die Liveliness-Verwaltung der DataReader und
merkt sich je Teilnehmer `last_seen` und `last_kind`.

### §8.4.13.3 Writer reliability.kind = RELIABLE

**Spec:** §8.4.13.3, S. 119 — "The BuiltinParticipantMessageWriter shall use
reliability.kind = RELIABLE_RELIABILITY_QOS."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_writer_heartbeats_until_the_reader_has_acknowledged`,
`crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`

**Status:** `done` — der Writer führt Zustand je Reader (Reader-Proxy), sendet HEARTBEATs bis
zur Bestätigung und wiederholt auf ACKNACK (`handle_reader_messages`).

### §8.4.13.3 Reader RELIABLE oder BEST_EFFORT

**Spec:** §8.4.13.3, S. 119 — "The BuiltinParticipantMessageReader may be
configured to use either RELIABLE_RELIABILITY_QOS or
BEST_EFFORT_RELIABILITY_QOS. If the BuiltinParticipantMessageReader is
configured to use BEST_EFFORT_RELIABILITY_QOS then the
BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER flag in
ParticipantProxy::builtinEndpointQos shall be set."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::heartbeat_responses`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`,
`crates/dcps/src/wlp.rs::tests::once_the_pulse_arrived_a_final_heartbeat_gets_no_answer_and_a_plain_one_the_ack`

**Status:** `done` — der Reader beantwortet HEARTBEATs wie ein RELIABLE-Reader;
das Flag entfällt, ZeroDDS sendet kein PID_BUILTIN_ENDPOINT_QOS.

### §8.4.13.3 Writer behandelt den Reader nach builtinEndpointQos

**Spec:** §8.4.13.3, S. 119 — "If the ParticipantProxy::builtinEndpointQos is
included in the SPDPdiscoveredParticipantData, then the
BuiltinParticipantMessageWriter shall treat the
BuiltinParticipantMessageReader as indicated by the flags. If the
ParticipantProxy::builtinEndpointQos is not included then the
BuiltinParticipantMessageWriter shall treat the
BuiltinParticipantMessageReader as if it is configured with
RELIABLE_RELIABILITY_QOS."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::add_peer_reader`, `sync_proxy`);
`crates/dcps/src/runtime.rs` (`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — mit `builtinEndpointQos` und dem Flag ist der Reader ein Best-Effort-Proxy (keine HEARTBEATs), sonst RELIABLE.

### §8.4.13.4 Figure 8.26 ParticipantMessageData

**Spec:** §8.4.13.4, S. 119, Figure 8.26 — "Figure 8.26 defines the
ParticipantMessageData datatype associated with the RTPS built-in Endpoint for
the DCPSParticipantMessage Topic." Attribute participantGuidPrefix, kind,
data.

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`, `to_cdr`, `from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_with_topic_token`

**Status:** `done`

### §8.4.13.5 AUTOMATIC: Probe schneller als die kürzeste Lease

**Spec:** §8.4.13.5, S. 119 — "If the Participant contains one or more Writers
with a liveliness of AUTOMATIC_LIVELINESS_QOS, then one sample is written at a
rate faster than the smallest lease duration among the Writers sharing this
QoS."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_wlp_period`: ein Drittel der
kürzesten endlichen Lease eines AUTOMATIC-Writers, höchstens ein Drittel der
Teilnehmer-Lease); `crates/dcps/src/wlp.rs` (`WlpEndpoint::tick`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`,
`crates/dcps/src/wlp.rs::tests::wlp_tick_emits_again_after_period`,
`crates/dcps/src/runtime.rs::tests::wlp_period_default_is_lease_over_three`

**Status:** `done` — die AUTOMATIC-Probe geht auch ohne AUTOMATIC-Writer aus.

### §8.4.13.5 MANUAL_BY_PARTICIPANT: eigene Probe

**Spec:** §8.4.13.5, S. 119 — "Similarly, a separate sample is written if the
Participant contains one or more Writers with a liveliness of
MANUAL_BY_PARTICIPANT_LIVELINESS_QOS at a rate faster than the smallest lease
duration among these Writers."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_liveliness`,
`refresh_manual_by_participant`); `crates/dcps/src/wlp.rs`
(`WlpEndpoint::assert_participant`, `refresh_participant`)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_assert_participant_emits_manual_pulse`,
`crates/dcps/src/wlp.rs::tests::refreshes_before_the_tick_share_one_manual_by_participant_sample`,
`crates/dcps/tests/wlp_integration.rs::linux_only::manual_by_participant_pulse_arrives_at_peer`,
`crates/dcps/src/runtime.rs::tests::a_manual_by_participant_writer_refreshes_the_participant_liveliness`

**Status:** `done` — die Probe wird bei `DomainParticipant::assert_liveliness` und bei
`write`, `assert_liveliness`, `dispose` und `unregister_instance` eines
MANUAL_BY_PARTICIPANT-Writers geschrieben; Aufrufe vor dem Tick teilen sich eine Probe.

### §8.4.13.5 Zwei Instanzen, Schlüssel participantGuidPrefix und kind

**Spec:** §8.4.13.5, S. 119 — "The instances are distinguished using their DDS
key, which is comprised of the participantGuidPrefix and kind fields. Each of
the two types of liveliness QoS handled through this protocol will result in a
unique kind field and therefore form two distinct instances in the
HistoryCache."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::write_pulse`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_history_holds_the_last_sample_of_each_kind`,
`crates/dcps/src/wlp.rs::tests::wlp_assert_participant_emits_manual_pulse`,
`crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`

**Status:** `done` — die Instanz ist (participantGuidPrefix, kind); der KeyHash (12 Oktette
Prefix, 4 Oktette kind, Big-Endian) steht als `PID_KEY_HASH` in der In-line-QoS jeder
DATA, die Historie hält eine Probe je Instanz.

### §8.4.13.5 participantGuidPrefix des schreibenden Teilnehmers

**Spec:** §8.4.13.5, S. 119 — "In both liveliness cases the
participantGuidPrefix field contains the GuidPrefix_t of the Participant that
is writing the data (and therefore asserting the liveliness of its Writers)."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::encode_pulse`:
`ParticipantMessageData::automatic(self.own_prefix)`)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_automatic_default_data_empty`

**Status:** `done`

### §8.4.13.5 MANUAL_BY_TOPIC nicht über den BuiltinParticipantMessageWriter

**Spec:** §8.4.13.5, S. 119 — "The DDS liveliness kind
MANUAL_BY_TOPIC_LIVELINESS_QOS is not implemented using the
BuiltinParticipantMessageWriter and BuiltinParticipantMessageReader. It is
discussed in 8.7.2.2.3."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::assert_liveliness` →
`DcpsRuntime::assert_writer_liveliness_eid`: HEARTBEAT mit LivelinessFlag)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/rtps/src/reliable_writer.rs::tests::liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag`

**Status:** `done` — `DcpsRuntime::assert_writer_liveliness` schreibt
zusätzlich eine Probe mit der Herstellerart 0x80000001; die DCPS-API ruft diese
Funktion nicht auf.

### §8.4.14 Optionales Verhalten

**Spec:** §8.4.14, S. 119 — "Optional features may not be supported by all
RTPS implementations. An optional feature does not affect basic
interoperability, but is only available if all implementations involved
support it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung der folgenden Unterabschnitte,
keine Anforderung.

### §8.4.14.1 Large Data: Transportgrenzen und Fragment-Submessages

**Spec:** §8.4.14.1, S. 120 — "a transport may impose its own limitations.
For example, it may limit the maximum packet size (e.g., 64K for UDP) …
8.3.8 introduces the following Submessages to enable fragmenting large data:
DataFrag, HeartbeatFrag, NackFrag".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung, keine Anforderung; die
Submessages sind unter §8.3.8 geführt.

### §8.4.14.1 Fragmentgröße aus den Transporten des Writers

**Spec:** §8.4.14.1, S. 120 — "All transports available to the Writer must be
able to accommodate DataFrag Submessages containing at least one fragment.
This means the transport with the smallest maximum message size determines
the fragment size." "Note the fragment size is determined by all transports
available to the Writer, not simply the subset of transports required to reach
all currently known Readers."

**Repo:** `crates/transport/src/lib.rs` (`Transport::max_message_size`, `DEFAULT_MAX_MESSAGE_SIZE`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransportConfig::max_message_size`);
`crates/transport-shm`, `transport-uds`, `transport-tcp`, `transport-tsn` (je `max_message_size`);
`crates/dcps/src/layered_transport.rs` (Minimum der Legs); `crates/rtps/src/reliable_writer.rs`
(`fragment_size_for_message_size`); `crates/dcps/src/runtime.rs` (Writer-Anlage)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writers_fragment_size_follows_the_message_size_of_its_transport`,
`crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_is_bounded_by_what_the_field_holds`,
`crates/transport-udp/src/udp_transport.rs::tests::max_message_size_is_the_configured_one_up_to_the_datagram_limit`,
`crates/dcps/tests/runtime_config_layering.rs::the_udp_message_size_is_a_setting`,
`crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`

**Status:** `done` — jeder Transport nennt seine größte Nachricht (`Transport::max_message_size`; UDP einstellbar mit `transport.udp.max_message_size`, Vorgabe 1472; SHM und UDS ihr Datagramm-Limit, TCP die Rahmengrenze, TSN 1500; der Schichten-Transport das Minimum seiner Legs). Die Fragmentgröße eines Writers ist die Nachricht des kleinsten Transports abzüglich 128 Oktette, fest für das Leben des Writers: 1344 bei der Vorgabe.

### §8.4.14.1 Fragmentgröße fest je Writer

**Spec:** §8.4.14.1, S. 120 — "The fragment size must be fixed for a given
Writer and is identical for all remote Readers. By fixing the fragment size,
the data a fragment number refers to does not depend on a particular remote
Reader."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::fragment_size_for`,
`needs_fragmentation`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::message_limit`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`,
`crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_does_not_depend_on_the_reader_expecting_inline_qos`

**Status:** `done` — die Fragmentgröße wird nach dem Anlegen des Writers nicht mehr
geändert (`set_fragmentation` entfällt) und ist für alle Reader dieselbe; die
In-line-QoS, die ein Fragment kürzt, wird mit ihrer größten Länge für irgendeinen
Reader gezählt, sodass auch `expectsInlineQos` eines Readers sie nicht ändert.

### §8.4.14.1 Fragmentgröße ≤ 65536

**Spec:** §8.4.14.1, S. 120 — "The fragment size must satisfy: fragment size
<= 65536 bytes."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`MAX_FRAGMENT_SIZE`, `ReliableWriter::try_new`, `ReliableWriter::new`,
`fragment_size_for`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_is_bounded_by_what_the_field_holds`

**Status:** `done` — `fragmentSize` des DATA_FRAG ist ein `unsigned short`; 65536 hat keine Kodierung, die Grenze ist 65535. `try_new` weist größere Werte mit einem Fehler ab (`new` bricht ab). Ein DATA_FRAG hält seine 32 festen Oktette, die Inline-QoS und das Fragment in den 65535 Oktetten von `octetsToNextHeader`; die Fragmentgröße liegt entsprechend höchstens bei 65503.

### §8.4.14.1 DataFrag in aufsteigender Fragmentnummer

**Spec:** §8.4.14.1, S. 120 — "DataFrag Submessages are sent in order, where
ordering is defined by increasing fragment numbers."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`push_sample_datagrams`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`

**Status:** `done`

### §8.4.14.1 Fragmentieren nur, wenn nötig (je Sequenznummer)

**Spec:** §8.4.14.1, S. 120 — "Data must only be fragmented if required. …
Likewise, for variable size data types, a regular Data Submessage must be used
if fragmentation is not required for a particular sequence number."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`needs_fragmentation`,
`push_sample_datagrams`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_under_fragment_size_produces_single_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_mixed_data_and_data_frag`

**Status:** `done`

### §8.4.14.1 Reguläres Data auf Transporten ohne Fragmentierungsbedarf

**Spec:** §8.4.14.1, S. 120 — "If multiple transports are available to the
Writer and some transports do not require fragmentation, a regular Data
Submessage must be sent on those transports instead."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::set_message_limit`);
`crates/rtps/src/reliable_writer.rs` (`needs_fragmentation`,
`unfragmented_room`); `crates/dcps/src/runtime.rs` (Match eines Readers auf
demselben Host)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`

**Status:** `done` — ein Reader auf demselben Host (Loopback, Nachrichtengröße
`LOOPBACK_MTU`) erhält ein Sample, das in eine Nachricht passt, als eine DATA;
ein entfernter Reader desselben Writers erhält dasselbe Sample als DATA_FRAGs
der Writer-Größe.

### §8.4.14.1 Inline-QoS mit dem ersten DataFrag

**Spec:** §8.4.14.1, S. 120 — "For a given sequence number, if in-line QoS
parameters are used, they must be included with the first DataFrag Submessage
(containing the fragment with fragment number equal to 1)."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`build_data_frag_submessage_datagram`: `sample_inline_qos` an jedem DATA_FRAG)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_oversized_coherent_set_member_goes_out_as_marked_fragments`,
`crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.4.14.1 Empfehlung: mehrere Fragmente je DataFrag

**Spec:** §8.4.14.1, S. 120 — "If a transport can accommodate multiple
fragments of the given fragment size, it is recommended that implementations
concatenate as many fragments as possible into a single DataFrag message."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`fragments_per_message`, `push_sample_datagrams`, Antwort auf NACK_FRAG in `tick`);
`crates/rtps/src/reader_proxy.rs` (`next_requested_fragment_run_after`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_fragments_of_a_sample_fill_the_messages_and_no_more`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/src/reader_proxy.rs::tests::next_requested_fragment_run_after_pulls_neighbours_of_one_sn`

**Status:** `done` — ein DATA_FRAG enthält so viele Fragmente, wie die Nachricht an den Reader trägt (`fragmentsInSubmessage`); das gilt für den ersten Versand und für die Antwort auf ein NACK_FRAG (benachbarte angeforderte Fragmente). Bei der Vorgabe von 1472 Oktetten passt ein Fragment in eine Nachricht.

### §8.4.14.1 Flusssteuerung

**Spec:** §8.4.14.1, S. 121 — "When sending multiple DataFrag messages, flow
control may be required to avoid flooding the network. … This is not part of
the RTPS specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — laut Text nicht Teil der Spezifikation.

### §8.4.14.1 Zusammensetzen: danach Verhalten wie Data

**Spec:** §8.4.14.1, S. 121 — "Once all fragments have been received, the same
protocol behavior applies as for a regular Data Submessage."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_frag`:
`received_change_set`, `store_received`, `collect_in_order_for`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_many_samples_3_fragments_each_10_percent_loss`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_key_is_a_dispose_or_a_registration_as_a_data_is`

**Status:** `done` — Bestätigung, Auslieferung in Reihenfolge und Auswertung der Inline-QoS des ersten Fragments (Status-Info mit D, U und F, Key-Registrierung, gerichtetes Schreiben, Coherent Set, Gruppe) wie bei Data.

### §8.4.14.1 Fragmente in beliebiger Reihenfolge

**Spec:** §8.4.14.1, S. 121 — "Note that implementations must be able to handle
out-of-order arrival of DataFrag submessages."

**Repo:** `crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::out_of_order_fragments_preserve_known_zero_key`

**Status:** `done`

### §8.4.14.1 Heartbeat nur für vollständig verfügbare Nummern

**Spec:** §8.4.14.1, S. 121 — "A Heartbeat message must only include those
sequence numbers for which all fragments are available."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`:
`stage_sample` legt das ganze Sample in den HistoryCache, bevor ein Fragment
gesendet wird; `lastSN` des HEARTBEAT ist `last_written_sn`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`

**Status:** `done`

### §8.4.14.1 Positive Bestätigung erst nach allen Fragmenten

**Spec:** §8.4.14.1, S. 121 — "an AckNack message must only positively
acknowledge a sequence number when all fragments were received for that
sequence number."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_frag`:
`received_change_set` erst, wenn `FragmentAssembler::insert` das Sample
liefert)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_many_samples_3_fragments_each_10_percent_loss`

**Status:** `done`

### §8.4.14.1 AckNack nur für vollständig fehlende Nummern

**Spec:** §8.4.14.1, S. 121 — "Likewise, a sequence number must be negatively
acknowledged only when all fragments are missing."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done`

### §8.4.14.1 NackFrag für einen Teil der Fragmente

**Spec:** §8.4.14.1, S. 121 — "In order to negatively acknowledge a subset of
fragments for a given sequence number, a NackFrag Submessage must be used.
When data is fragmented, a Heartbeat may trigger both AckNack and NackFrag
Submessages."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`tick_outbound`: ein NACK_FRAG
je unvollständiger Sequenznummer, dann das ACKNACK; `build_nackfrag_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done`

### §8.4.14.1 HeartbeatFrag für teilweise verfügbare Nummern

**Spec:** §8.4.14.1, S. 121 — "If a Writer wants to inform a Reader on the
partial availability of fragments for a given sequence number, a
HeartbeatFrag Submessage can be used instead."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung; die Antwort des Readers
auf HeartbeatFrag ist unter §8.3.8.7 geführt.

### §8.4.14.1 NackFrag nur als Antwort auf Heartbeat oder HeartbeatFrag

**Spec:** §8.4.14.1, S. 121 — "A NackFrag Submessage can only be sent in
response to a Heartbeat or HeartbeatFrag submessage."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`,
`handle_heartbeat_frag`, `handle_data_frag_opening`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::acknack_and_nack_frag_wait_for_a_heartbeat`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — NACK_FRAG geht nur im Tick nach einem HEARTBEAT oder HEARTBEAT_FRAG
aus; ein DATA_FRAG, das Fragmente offen lässt, löst keinen aus. Ein verlorenes
Fragment wird mit dem nächsten HEARTBEAT des Writers angefordert.

### §8.4.15 Implementation Guidelines

**Spec:** §8.4.15, S. 121 — "The contents of this sub clause are not part of
the formal specification of the protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — laut Text nicht Teil der Spezifikation.

### §8.4.15.1 Implementierung von ReaderProxy und WriterProxy

**Spec:** §8.4.15.1, S. 121–122 — "In practice, what is required is that the
ReaderProxy is able to implement the operations used by the protocol and this
does not require the use of explicit associations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.2 Gap und AckNack effizient nutzen

**Spec:** §8.4.15.2, S. 122 — "implementations that combine multiple Gap and
AckNack Submessages and take advantage of the ability of these Submessages to
contain a set of sequence number will be more efficient".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.3 Mehrere Data-Submessages zusammenfassen

**Spec:** §8.4.15.3, S. 122 — "implementations that combine Submessages into a
single RTPS message will in general make better utilization of CPU and
bandwidth."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.4 HeartBeat an Data anhängen

**Spec:** §8.4.15.4, S. 122 — "A particularly useful case is the piggybacking
of HeartBeat Submessages following Data Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.5 Senden an unbekannte readerId

**Spec:** §8.4.15.5, S. 122 — "Implementations are encouraged to use this
feature to minimize bandwidth usage."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.6 Ressourcen von nicht antwortenden Readern zurückgewinnen

**Spec:** §8.4.15.6, S. 123 — "the Writer should be allowed to deem the Reader
as 'Inactive' and proceed in updating its queue."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15.

### §8.4.15.7 Count in Heartbeat, HeartbeatFrag, AckNack und NackFrag

**Spec:** §8.4.15.7, S. 123 — "an implementation should ensure that same
logical HEARTBEATs are tagged with the same Count." "The Count element should
be incremented and compared according to modular arithmetic rules in order to
accommodate the integer overflow."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Leitfaden nach §8.4.15; die Pflichten zu
Count stehen unter §8.3.8.

## §8.5 Discovery Module

### §8.5 Entdeckung entfernter Entitäten und Builtin-Topics

**Spec:** §8.5, S. 123 — "DDS implementations must automatically discover the
presence of remote entities, both when they join and leave the network. This
discovery information is made accessible to the user through DDS built-in
topics."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber`);
`crates/dcps/src/runtime.rs` (`handle_spdp_datagram`, SEDP-Ereignisse)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::spdp_receive_populates_dcps_participant_reader`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`,
`crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`

**Status:** `done`

### §8.5.1 SPDP und SEDP als Mindestumfang

**Spec:** §8.5.1, S. 124 — "For the purpose of interoperability, all RTPS
implementations must provide at least the following discovery protocols:
1. Simple Participant Discovery Protocol (SPDP) 2. Simple Endpoint Discovery
Protocol (SEDP)". Ebenso §8.5.3, S. 125, und §8.5.4, S. 130.

**Repo:** `crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`
(`SedpStack`)

**Tests:** `crates/discovery/tests/spdp_loopback_e2e.rs::two_zerodds_participants_discover_each_other`,
`crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`

**Status:** `done`

### §8.5.1 Weitere PDP und EDP

**Spec:** §8.5.1, S. 124 — "Implementations may choose to support multiple
PDPs and EDPs, possibly vendor-specific." "How this information is used by a
Participant to configure its local Endpoints depends on the actual
implementation of the RTPS protocol and is not part of the discovery protocol
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §8.5.2 Builtin-Topics und ihre RTPS-Endpunkte

**Spec:** §8.5.2, S. 124 — "There are four pre-defined built-in Topics:
“DCPSParticipant,” “DCPSSubscription,” “DCPSPublication,” and “DCPSTopic.”"
"The built-in DataReaders act as regular DDS DataReaders and can also be
accessed by the user through the DDS API." "RTPS maps each built-in DDS
DataWriter or DataReader to an associated built-in RTPS Endpoint."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (vier Builtin-DataReader);
`crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/src/builtin_subscriber.rs::tests::sinks_push_topic_lands_in_reader`

**Status:** `done`

### §8.5.3.1 SPDP-Builtin-Endpunkte

**Spec:** §8.5.3.1, S. 125 — "For each Participant, the SPDP creates two RTPS
built-in Endpoints: the SPDPbuiltinParticipantWriter and the
SPDPbuiltinParticipantReader."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`, `SpdpReader`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::spdp_beacon`)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`

**Status:** `done`

### §8.5.3.1 SPDPbuiltinParticipantWriter: Best-Effort-StatelessWriter

**Spec:** §8.5.3.1, S. 125 — "The SPDPbuiltinParticipantWriter is an RTPS
Best-Effort StatelessWriter."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`: DATA ohne
HEARTBEAT, ohne Zustand je Empfänger)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_uses_spdp_builtin_writer_id`,
`crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done`

### §8.5.3.1 Ein Datenobjekt im HistoryCache, bei Änderung ersetzt

**Spec:** §8.5.3.1, S. 125 — "The HistoryCache of the
SPDPbuiltinParticipantWriter contains a single data-object of type
SPDPdiscoveredParticipantData. The value of this data-object is set from the
attributes in the Participant. If the attributes change, the data-object is
replaced."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::data`);
`crates/dcps/src/runtime.rs` (Beacon-Aktualisierung bei geänderter USER_DATA)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** `done`

### §8.5.3.1 Periodisches Senden derselben Änderung

**Spec:** §8.5.3.1, S. 125 — "This is achieved by periodically calling
StatelessWriter::unsent_changes_reset, which causes the StatelessWriter to
resend all changes present in its HistoryCache to all locators."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`, `set_new_sn_each_period`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::spdp_new_sequence_per_period`)

**Tests:** `crates/discovery/src/spdp.rs::tests::the_resent_beacon_keeps_its_sequence_number_until_the_data_changes`,
`crates/discovery/src/spdp.rs::tests::beacon_increments_sequence_number`

**Status:** `done` — die periodische Ankündigung wiederholt die eine Änderung mit ihrer Sequenznummer; eine Änderung der Teilnehmerdaten ist eine neue Änderung. Die Form von Cyclone DDS und OpenDDS (neue Nummer je Periode, gemessen in `tools/wire-capture/rtps-sessions`) ist die Einstellung `discovery.spdp_new_sequence_per_period`. Nicht gemessen: ob ein Cyclone- oder OpenDDS-Reader den Teilnehmer bei wiederholter Nummer am Leben hält.

### §8.5.3.1 Periode: Vorgabe nach PSM, kleiner als leaseDuration

**Spec:** §8.5.3.1, S. 125 — "The periodic rate at which the
SPDPbuiltinParticipantWriter sends out the SPDPdiscoveredParticipantData
defaults to a PSM specified value. This period should be smaller than the
leaseDuration specified in the SPDPdiscoveredParticipantData".

**Repo:** `crates/dcps/src/runtime.rs` (`default_spdp_period_for`, `DEFAULT_SPDP_PERIOD`, Schlüssel `discovery.spdp_period_ms`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — die Vorgabe ist die der PSM, 30 s; bei einer Lease unter 90 s höchstens ein Drittel der Lease, damit die Periode kleiner als `leaseDuration` bleibt.

### §8.5.3.1 Vorkonfigurierte Unicast- und Multicast-Locatoren

**Spec:** §8.5.3.1, S. 125 — "The pre-configured list of locators may include
both unicast and multicast locators. Port numbers are defined by each PSM."

**Repo:** `crates/dcps/src/runtime.rs` (`send_spdp_multicast`,
`send_spdp_unicast`, `expand_initial_peer`)

**Tests:** `crates/dcps/src/runtime.rs::tests::multicast_free_discovery_via_initial_peers`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done`

### §8.5.3.1 SPDPbuiltinParticipantReader und Wahl des EDP

**Spec:** §8.5.3.1, S. 125 — "The contained information includes what Endpoint
Discovery Protocols the remote Participant supports. The proper Endpoint
Discovery Protocol is then used for exchanging Endpoint information with the
remote Participant."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`:
nach `builtin_endpoint_set`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.5.3.1 Optional: Ankündigung an einen bisher unbekannten Teilnehmer

**Spec:** §8.5.3.1, S. 125 — "Implementations can minimize any start-up delays
by sending an additional SPDPdiscoveredParticipantData in response to
receiving this data-object from a previously unknown Participant, but this
behavior is optional."

**Repo:** `crates/dcps/src/runtime.rs` (`directed_spdp_targets`,
`SpdpResponseTargets`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_spdp_response_axis_decides_the_targets`,
`crates/rtps/src/dialect.rs::tests::the_spdp_response_default_answers_the_beacon_source`

**Status:** `done`

### §8.5.3.1 Optional: Locatorliste um entdeckte Teilnehmer erweitern

**Spec:** §8.5.3.1, S. 125 — "Implementations may also enable the user to
choose whether to automatically extend the pre-configured list of locators
with new locators from newly discovered Participants. … These last two
features are optional and not required for the purpose of interoperability."

**Repo:** `crates/dcps/src/runtime.rs` (`send_spdp_unicast`: periodische
Ankündigung auch an die Metatraffic-Locatoren entdeckter Teilnehmer)

**Tests:** `crates/dcps/src/runtime.rs::tests::an_initial_peer_a_discovered_participant_covers_is_left_to_the_fan_out`

**Status:** `done` — die Erweiterung ist nicht abschaltbar.

### §8.5.3.2 Figure 8.27 SPDPdiscoveredParticipantData

**Spec:** §8.5.3.2, S. 125 — "the SPDPdiscoveredParticipantData specializes
the ParticipantProxy and therefore includes all the information necessary to
configure a discovered Participant. The SPDPdiscoveredParticipantData also
specializes the DDS-defined DDS::ParticipantBuiltinTopicData".

**Repo:** `crates/rtps/src/participant_data.rs` (`ParticipantBuiltinTopicData`:
Attribute des ParticipantProxy, `user_data`, `lease_duration`, `manual_liveliness_count`,
`builtin_endpoint_qos`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`

**Status:** `done`

### §8.5.3.2 Table 8.78 domainId

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "domainId: DomainId_t — Identifies the
DDS domainId of the associated DDS DomainParticipant."

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_id`, PID_DOMAIN_ID);
`crates/dcps/src/runtime.rs` (Beacon mit `domain_id: Some(domain_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 domainTag

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "domainTag: string — Identifies the
DDS domainTag of the associated DDS DomainParticipant."

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_tag`, PID_DOMAIN_TAG)

**Tests:** `crates/rtps/src/participant_data.rs::tests::domain_tag_goes_out_when_set_and_reads_back`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** `done`

### §8.5.3.2 Table 8.78 protocolVersion

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "protocolVersion: ProtocolVersion_t —
Identifies the RTPS protocol version used by the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`protocol_version`,
PID_PROTOCOL_VERSION)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 guidPrefix

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "guidPrefix: GuidPrefix_t — The
common GuidPrefix_t of the Participant and all the Endpoints contained within
the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`guid`, PID_PARTICIPANT_GUID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 vendorId

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "vendorId: VendorId_t — Identifies the
vendor of the DDS middleware that contains the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`vendor_id`, PID_VENDOR_ID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `partial` — ZeroDDS sendet `0x01F0`, keinen von der OMG
zugewiesenen Wert (`open`-Item unter §8.3.5.2). Blocker: ein harter externer, die Zuweisung einer VendorId durch die OMG; der Antrag läuft seit dem 22.06. Bis dahin sendet ZeroDDS `0x01F0`, einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

### §8.5.3.2 Table 8.78 expectsInlineQos

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "expectsInlineQos: bool — Describes
whether the Readers within the Participant expect that the QoS values that
apply to each data modification are encapsulated included with each Data."

**Repo:** `crates/rtps/src/participant_data.rs` (`expects_inline_qos`,
PID_EXPECTS_INLINE_QOS)

**Tests:** `crates/rtps/src/participant_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §8.5.3.2 Table 8.78 metatrafficUnicastLocatorList

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "List of unicast locators (transport,
address, port combinations) that can be used to send messages to the built-in
Endpoints contained in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`metatraffic_unicast_locators`, PID_METATRAFFIC_UNICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`,
`crates/dcps/src/runtime.rs::tests::wlp_unicast_targets_prefers_metatraffic_then_default`

**Status:** `done`

### §8.5.3.2 Table 8.78 metatrafficMulticastLocatorList

**Spec:** §8.5.3.2, S. 126, Table 8.78 — "List of multicast locators
(transport, address, port combinations) that can be used to send messages to
the built-in Endpoints contained in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`metatraffic_multicast_locators`, PID_METATRAFFIC_MULTICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/participant_data.rs::tests::decode_zero_locators_yields_empty_lists`

**Status:** `done`

### §8.5.3.2 Table 8.78 defaultUnicastLocatorList

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "defaultUnicastLocatorList:
Locator_t[1..*] … These are the unicast locators that will be used in case the
Endpoint does not specify its own set of Locators, so at least one Locator
must be present."

**Repo:** `crates/rtps/src/participant_data.rs` (`default_unicast_locators`);
`crates/dcps/src/runtime.rs` (`announce_locators`: nie leer)

**Tests:** `crates/dcps/src/runtime.rs::tests::announce_locators_automatic_is_non_empty_and_includes_probe`,
`crates/rtps/src/participant_data.rs::tests::encode_dedups_exact_duplicate_locators`

**Status:** `done`

### §8.5.3.2 Table 8.78 defaultMulticastLocatorList

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "Default list of multicast locators …
These are the multicast locators that will be used in case the Endpoint does
not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs` (`default_multicast_locators`,
PID_DEFAULT_MULTICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done` — ZeroDDS kündigt keinen Default-Multicast-Locator an.

### §8.5.3.2 Table 8.78 availableBuiltinEndpoints

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "All Participants must support the
SEDP. This attribute identifies the kinds of built-in SEDP Endpoints that are
available in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_set`,
`endpoint_flag`); `crates/dcps/src/runtime.rs` (`ALL_STANDARD` plus
TOPICS_ANNOUNCER/TOPICS_DETECTOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_standard_bits_by_default`

**Status:** `done`

### §8.5.3.2 Table 8.78 leaseDuration

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "How long a Participant should be
considered alive every time an announcement is received from the Participant.
If a Participant fails to send another announcement within this time period,
the Participant can be considered gone."

**Repo:** `crates/rtps/src/participant_data.rs` (`lease_duration`,
PID_PARTICIPANT_LEASE_DURATION); `crates/discovery/src/spdp.rs`
(`DiscoveredParticipantsCache::drain_expired`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §8.5.3.2 Table 8.78 manualLivelinessCount

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "manualLivelinessCount: Count_t —
Used to implement MANUAL_BY_PARTICIPANT liveliness QoS. When liveliness is
asserted, the manualLivelinessCount is incremented and a new
SPDPdiscoveredParticipantData is sent."

**Repo:** `crates/rtps/src/participant_data.rs` (`manual_liveliness_count`);
`crates/dcps/src/runtime.rs` (`bump_manual_liveliness_count`, `spdp_announced`);
`crates/dcps/src/wlp.rs` (`take_manual_participant_written`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/dcps/src/runtime.rs::tests::the_manual_liveliness_count_of_the_announcement_counts_up`,
`crates/dcps/src/runtime.rs::tests::a_changed_manual_liveliness_count_renews_the_manual_writers_of_that_participant`,
`crates/dcps/src/wlp.rs::tests::a_written_manual_sample_is_reported_once`

**Status:** `done` — wird eine MANUAL_BY_PARTICIPANT-Probe geschrieben, steigt der Zähler (höchstens einmal je Tick) und eine neue Ankündigung geht hinaus; ein Empfänger, bei dem sich der Zähler eines bekannten Teilnehmers ändert, wertet das als manuelle Liveliness-Zusicherung dieses Teilnehmers (Test: n, n + 1 erneuert die MANUAL_BY_PARTICIPANT-Writer des Teilnehmers am Reader).

### §8.5.3.2 Table 8.78 builtinEndpointQos

**Spec:** §8.5.3.2, S. 127, Table 8.78 — "builtinEndpointQos:
BuiltinEndpointQos_t — Provides additional information on the QoS of the
built-in Endpoints supported by the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_qos`);
`crates/dcps/src/wlp.rs` (`WlpEndpoint::add_peer_reader`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — das Feld wird gelesen und geschrieben; der WLP-Writer behandelt den Reader danach (§8.4.13.3). ZeroDDS selbst kündigt keines an: sein Reader ist RELIABLE.

### §8.5.3.2 Weitere EDP über die Erweiterungsmechanismen

**Spec:** §8.5.3.2, S. 127 — "There are currently no other Endpoint Discovery
Protocols defined by the RTPS specification. In order to extend
SPDPdiscoveredParticipantData to include additional EDPs, the standard RTPS
extension mechanisms can be used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §8.5.3.2.1 availableBuiltinEndpoints: ankündbare Endpunkte

**Spec:** §8.5.3.2, S. 127 — "The builtin endpoints that may be announced in
the availableBuiltinEndpoints include: PUBLICATIONS_DETECTOR,
PUBLICATIONS_ANNOUNCER, SUBSCRIPTIONS_DETECTOR, SUBSCRIPTIONS_ANNOUNCER,
TOPICS_DETECTOR, TOPICS_ANNOUNCER, PARTICIPANT_MESSAGE_READER, and
PARTICIPANT_MESSAGE_WRITER. The availableBuiltinEndpoints may also announce
builtin endpoints defined in other DDS specifications."

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag`,
`ALL_STANDARD`, `ALL_SECURE`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_roundtrip_through_pl_cdr`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`,
`crates/dcps/tests/type_lookup_e2e.rs::announces_type_lookup_endpoints_in_spdp`

**Status:** `done`

### §8.5.3.3 EntityIds der SPDP-Endpunkte

**Spec:** §8.5.3.3, S. 128 — "The Protocol reserves the following values of the
EntityId_t for the SPDP built-in Endpoints:
ENTITYID_SPDP_BUILTIN_PARTICIPANT_WRITER
ENTITYID_SPDP_BUILTIN_PARTICIPANT_READER"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`,
`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`,
`crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP-Writer unicastLocatorList

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "unicastLocatorList: <auto-detected>
Transport-kinds and addresses are either auto-detected or configured by the
application. Ports are a parameter to the SPDP initialization or else are set
to a PSM-specified value that depends on the domainId."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_locators`,
`spdp_unicast_port`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`,
`crates/dcps/src/runtime.rs::tests::announce_locators_pins_to_single_interface`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP-Writer multicastLocatorList

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "multicastLocatorList: <parameter to
the SPDP initialization> Defaults to a PSM-specified value."

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`, Gruppe
239.255.0.1); `crates/dcps/src/runtime.rs` (`spdp_multicast_group`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_default_multicast_is_239_255_0_1`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_0_is_7400`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP-Writer reliabilityLevel

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "reliabilityLevel: BEST_EFFORT".

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP-Writer topicKind

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "topicKind: WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`: `BuiltinWriterWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP-Writer resendPeriod

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "resendPeriod: <parameter to the SPDP
initialization> Defaults to a PSM-specified value."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::spdp_period`, `ZERODDS_SPDP_PERIOD_MS`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — einstellbar (`discovery.spdp_period_ms`); die Vorgabe ist 30 s, die PSM-Vorgabe.

### §8.5.3.3 Table 8.79 SPDP-Writer readerLocators

**Spec:** §8.5.3.3, S. 128, Table 8.79 — "readerLocators: <parameter to the
SPDP initialization>".

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::initial_peers`,
`expand_initial_peer`, Multicast-Ziel)

**Tests:** `crates/dcps/src/runtime.rs::tests::initial_peer_specs_reach_the_runtime_without_touching_the_environment`,
`crates/dcps/src/runtime.rs::tests::initial_peer_specs_expand_per_the_configured_participant_limit`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP-Reader unicastLocatorList

**Spec:** §8.5.3.3, S. 129, Table 8.80 — "unicastLocatorList: <auto-detected>
… Ports are a parameter to the SPDP initialization or else are set to a
PSM-specified value that depends on the domainId."

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_unicast`-Socket auf
`spdp_unicast_port`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP-Reader multicastLocatorList

**Spec:** §8.5.3.3, S. 129, Table 8.80 — "multicastLocatorList: <parameter to
the SPDP initialization>. Defaults to a PSM-specified value."

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_multicast_rx`,
`spdp_multicast_group`)

**Tests:** `crates/dcps/src/runtime.rs::tests::start_refuses_spdp_group_outside_allowlist`,
`crates/discovery/tests/spdp_loopback_e2e.rs::loopback_multicast_self_discovery`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP-Reader reliabilityLevel

**Spec:** §8.5.3.3, S. 129, Table 8.80 — "reliabilityLevel: BEST_EFFORT".

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpReader`: zustandslos, ohne
ACKNACK)

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_reads_every_participant_a_datagram_bundles`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP-Reader topicKind

**Spec:** §8.5.3.3, S. 129, Table 8.80 — "topicKind: WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`: `BuiltinReaderWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §8.5.3.3 SPDP-Reader: HistoryCache nach Teilnehmer-GUID

**Spec:** §8.5.3.3, S. 129 — "The HistoryCache of the
SPDPbuiltinParticipantReader contains information on all active discovered
participants; the key used to identify each data-object corresponds to the
Participant GUID." "If an entry with a matching key is not there, a new entry
is added keyed by the GUID of the Participant."

**Repo:** `crates/discovery/src/spdp.rs` (`DiscoveredParticipantsCache`:
Schlüssel `data.guid.prefix`)

**Tests:** `crates/discovery/src/spdp.rs::tests::cache_insert_returns_true_for_new_participant`,
`crates/discovery/src/spdp.rs::tests::cache_get_returns_inserted_participant`

**Status:** `done`

### §8.5.3.3 SPDP-Reader: veraltete Einträge entfernen

**Spec:** §8.5.3.3, S. 129 — "Periodically, the SPDP examines the
SPDPbuiltinParticipantReader HistoryCache looking for stale entries defined as
those that have not been refreshed for a period longer than their specified
leaseDuration. Stale entries are removed."

**Repo:** `crates/discovery/src/spdp.rs`
(`DiscoveredParticipantsCache::drain_expired`); `crates/dcps/src/runtime.rs`
(Aufruf je Tick)

**Tests:** `crates/discovery/src/spdp.rs::tests::participant_lease_lifecycle_observation_and_infinite`,
`crates/dcps/src/runtime.rs::tests::participant_lease_lifecycle_hard_death_and_same_prefix`

**Status:** `done` — `drain_expired` entfernt schon bei Gleichheit von
verstrichener Zeit und leaseDuration.

### §8.5.3.4 Table 8.81 SPDP_WELL_KNOWN_UNICAST_PORT

**Spec:** §8.5.3.4, S. 129, Table 8.81 — "SPDP_WELL_KNOWN_UNICAST_PORT —
entries in SPDPbuiltinParticipantReader.unicastLocatorList, unicast entries in
SPDPbuiltinParticipantWriter.readerLocators".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_unicast_port`);
`crates/dcps/src/runtime.rs` (`expand_initial_peer`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done`

### §8.5.3.4 Table 8.81 SPDP_WELL_KNOWN_MULTICAST_PORT

**Spec:** §8.5.3.4, S. 129, Table 8.81 — "SPDP_WELL_KNOWN_MULTICAST_PORT —
entries in SPDPbuiltinParticipantReader.multicastLocatorList, multicast entries
in SPDPbuiltinParticipantWriter.readerLocators".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_1_is_7650`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_5_is_8650`

**Status:** `done`

### §8.5.4.1 Assoziation mit den SEDP-Endpunkten eines entdeckten Teilnehmers

**Spec:** §8.5.4.1, S. 130 — "once a Participant knows of the presence of
another Participant, it can assume the presence of the built-in Endpoints
made available by the remote participant and establish the association with
the locally-matching built-in Endpoints."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::self_discovery_is_ignored`

**Status:** `done`

### §8.5.4.1 SEDP: dasselbe Protokoll wie für Anwendungs-Endpunkte

**Spec:** §8.5.4.1, S. 130 — "The protocol used to communicate between
built-in Endpoints is the same as used for application-defined Endpoints."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`:
`ReliableWriter`); `crates/discovery/src/sedp/reader.rs` (`ReliableReader`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`,
`crates/discovery/src/sedp/stack.rs::tests::a_heartbeat_past_a_hole_releases_the_buffered_subscription`

**Status:** `done`

### §8.5.4.2 SEDP-Endpunkte zuverlässig

**Spec:** §8.5.4.2, S. 130 — "The SEDP therefore maps each corresponding
built-in DDS DataWriter or DataReader into corresponding reliable RTPS Writer
and Reader Endpoints." "it is sufficient that an implementation provides the
required built-in Endpoints and reliable communication that satisfies the
general requirements listed in 8.4.2."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`);
`crates/discovery/src/sedp/reader.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::a_heartbeat_past_a_hole_releases_the_buffered_subscription`,
`crates/discovery/tests/cyclone_sedp_replay.rs::cyclone_sedp_publication_fixture_flows_through_stack`

**Status:** `done` — die Abweichungen der zuverlässigen Writer und Reader von
§8.4.2 sind dort geführt.

### §8.5.4.2 EntityIds der SEDP-Endpunkte

**Spec:** §8.5.4.2, S. 130–131 — "The RTPS Protocol reserves the following
values of the EntityId_t for the built-in Endpoints:
ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER … ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`,
`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER` und die übrigen)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_publications_writer_layout`,
`crates/rtps/src/wire_types.rs::tests::sedp_subscriptions_reader_layout`

**Status:** `done` — die Werte sind unter §9.3.1.3 geführt.

### §8.5.4.3 Topics-Endpunkte optional

**Spec:** §8.5.4.3, S. 131 — "it is not required to implement the
SEDPbuiltinTopicsReader and SEDPbuiltinTopicsWriter built-in Endpoints and for
the purpose of interoperability, implementations should not rely on their
presence in remote Participants."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`:
Topics-Proxies nur bei gesetzten Bits)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.5.4.3 Nur die benötigten Builtin-Endpunkte

**Spec:** §8.5.4.3, S. 131 — "a Participant is only required to provide the
built-in Endpoints required for matching up local and remote Endpoints."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung; ZeroDDS stellt alle
SEDP-Endpunkte bereit.

### §8.5.4.4 Figure 8.30 DiscoveredWriterData

**Spec:** §8.5.4.4, S. 131 — "DiscoveredWriterData extends
DDS::PublicationBuiltinTopicData" "the “Discovered” DataTypes also include all
the information that may be needed by an implementation of the protocol to
configure the RTPS Endpoints. This information is contained in the RTPS
ReaderProxy and WriterProxy."

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData`:
QoS, `key`, `unicast_locators`, `multicast_locators`, `group_guid`, `data_max_size_serialized`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`,
`crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`,
`crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done` — die Attribute des WriterProxy stehen in der Ankündigung: Writer-GUID, beide Locator-Listen, `dataMaxSizeSerialized` (`PID_TYPE_MAX_SIZE_SERIALIZED`) und die Gruppe (`PID_GROUP_GUID`, daraus `remoteGroupEntityId`).

### §8.5.4.4 Figure 8.30 DiscoveredReaderData

**Spec:** §8.5.4.4, S. 131 — "DiscoveredReaderData extends the DDS-defined
DDS::SubscriptionBuiltinTopicData".

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData`: QoS, `key`, `unicast_locators`,
`multicast_locators`, `expects_inline_qos`, `content_filter`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §8.5.4.4 Figure 8.30 DiscoveredTopicData

**Spec:** §8.5.4.4, S. 131 — "DiscoveredTopicData extends
DDS::TopicBuiltinTopicData."

**Repo:** `crates/rtps/src/topic_data.rs` (`TopicBuiltinTopicData`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`,
`crates/discovery/src/sedp/stack.rs::tests::a_cyclone_topic_and_its_dispose_are_read_by_cyclones_own_key`

**Status:** `done`

### §8.5.4.4 Fehlende Angaben: Vorgaben der PSM

**Spec:** §8.5.4.4, S. 132 — "An implementation of the protocol need not
necessarily send all information contained in the DataTypes. If any
information is not present, the implementation can assume the default
values, as defined by the PSM."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (Vorgabe je fehlendem PID)

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::participant_key_fallback_when_pid_missing`

**Status:** `done` — die Vorgabe je PID ist unter §9.6.3.2 geführt.

### §8.5.4.4 Table 8.82 Publications: DataType

**Spec:** §8.5.4.4, S. 133, Table 8.82 — "DataType: DiscoveredWriterData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpPublicationsWriter::announce`:
`PublicationBuiltinTopicData::to_pl_cdr_le`)

**Tests:** `crates/discovery/tests/sedp_loopback.rs::publication_announce_then_reader_fills_cache`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: Cardinality

**Spec:** §8.5.4.4, S. 133, Table 8.82 — "Cardinality: The number of
DataWriters contained by the DomainParticipant. There is a one-to-one
correspondence between each DataWriter in the participant and a data-object
that describes the DataWriter stored in the WriterHistoryCache for the
SEDPbuiltinPublicationsWriter."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`: eine
Änderung je Instanz, `SEDP_MAX_CHANGES`; `SedpPublicationsWriter::announce` mit
der Endpunkt-GUID als Schlüssel)

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: Einfügen

**Spec:** §8.5.4.4, S. 133, Table 8.82 — "Data-Object insertion: Each time a
DataWriter is created in the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_publication` beim Anlegen
des Writers)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: Ändern

**Spec:** §8.5.4.4, S. 133, Table 8.82 — "Data-Object modification: Each time
the QoS of an existing DataWriter is modified."

**Repo:** `crates/dcps/src/runtime.rs` (`reannounce_publication`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: Löschen

**Spec:** §8.5.4.4, S. 133, Table 8.82 — "Data-Object deletion: Each time an
existing DataWriter belonging to the DomainParticipant is deleted."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpPublicationsWriter::dispose`);
`crates/dcps/src/runtime.rs` (`dispose_publication` beim Löschen)

**Tests:** `crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: DataType

**Spec:** §8.5.4.4, S. 134, Table 8.83 — "DataType: DiscoveredReaderData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpSubscriptionsWriter::announce`)

**Tests:** `crates/discovery/tests/sedp_loopback.rs::subscription_announce_then_reader_fills_cache`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: Cardinality

**Spec:** §8.5.4.4, S. 134, Table 8.83 — "Cardinality: The number of
DataReaders contained by the DomainParticipant. There is a one-to-one
correspondence between each DataReaders in the Participant and a data-object
that describes the DataReaders stored in the WriterHistoryCache for the
SEDPbuiltinSubscriptionsWriter."

**Repo:** —

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done` — derselbe Writer-Aufbau (`make_sedp_writer`) wie unter Table
8.82 Cardinality, Ankündigung mit der GUID als Schlüssel.

### §8.5.4.4 Table 8.83 Subscriptions: Einfügen

**Spec:** §8.5.4.4, S. 134, Table 8.83 — "Data-Object insertion: Each time a
DataReader is created in the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_subscription` beim Anlegen
des Readers)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: Ändern

**Spec:** §8.5.4.4, S. 134, Table 8.83 — "Data-Object modification: Each time
the QoS of an existing DataReader is modified."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_subscription` nach
`set_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: Löschen

**Spec:** §8.5.4.4, S. 134, Table 8.83 — "Data-Object deletion: Each time an
existing DataReader belonging to the DomainParticipant is deleted."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpSubscriptionsWriter::dispose`)

**Tests:** `crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_reader_unmatches_the_remote_writer`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: DataType

**Spec:** §8.5.4.4, S. 134, Table 8.84 — "DataType: DiscoveredTopicData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpTopicsWriter::announce`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: Cardinality

**Spec:** §8.5.4.4, S. 134, Table 8.84 — "Cardinality: The number of Topics
created by the DomainParticipant. There is a one-to-one correspondence between
each Topic created by the DomainParticipant and a data-object that describes
the Topic stored in the WriterHistoryCache for the builtinTopicsWriter."

**Repo:** —

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done` — derselbe Writer-Aufbau (`make_sedp_writer`) wie unter Table
8.82 Cardinality, Ankündigung mit der GUID als Schlüssel.

### §8.5.4.4 Table 8.84 Topics: Einfügen

**Spec:** §8.5.4.4, S. 134, Table 8.84 — "Data-Object insertion: Each time a
Topic is created in the DomainParticipant."

**Repo:** `crates/dcps/src/participant.rs` (`announce_topic` beim Anlegen bzw.
Aktivieren)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: Ändern

**Spec:** §8.5.4.4, S. 134, Table 8.84 — "Data-Object modification: Each time
the QoS of an existing Topic is modified."

**Repo:** `crates/dcps/src/topic.rs` (`set_qos` → `announce_topic`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done` — der Test prüft den SEDP-Stack, nicht den Aufruf aus
`Topic::set_qos`.

### §8.5.4.4 Table 8.84 Topics: Löschen

**Spec:** §8.5.4.4, S. 134, Table 8.84 — "Data-Object deletion: Each time an
existing Topic belonging to the DomainParticipant is deleted."

**Repo:** `crates/dcps/src/runtime.rs` (`dispose_local_topic`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::delete_topic_disposes_the_announced_topic_and_a_writer_holds_it`

**Status:** `done`

### §8.5.5 Zusammenspiel mit der virtuellen Maschine

**Spec:** §8.5.5, S. 134 — "To further illustrate the SPDP and SEDP, this
specification describes how the information provided by the SPDP can be used
to configure the SEDP built-in Endpoints in the RTPS virtual machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §8.5.5.1 und §8.5.5.2.

### §8.5.5.1 Neuer Teilnehmer: domainId vergleichen

**Spec:** §8.5.5.1, S. 134–135 — "IF ( participant_data.domainId !=
local_participant.domainId ) THEN RETURN;"

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_announced`: PID_DOMAIN_ID)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/dcps/src/runtime.rs::tests::a_beacon_of_another_domain_is_named_once_per_sender`

**Status:** `done` — ein fehlendes PID_DOMAIN_ID gilt als eigene Domain.

### §8.5.5.1 Neuer Teilnehmer: domainTag vergleichen

**Spec:** §8.5.5.1, S. 135 — "IF ( !STRING_EQUAL(participant_data.domainTag,
local_participant.domainTag) ) THEN RETURN;"

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_announced`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** `done`

### §8.5.5.1 Neuer Teilnehmer: Proxies je angekündigtem Endpunkt

**Spec:** §8.5.5.1, S. 135–136 — "IF ( PUBLICATIONS_DETECTOR IS_IN
participant_data.availableEndpoints ) THEN … writer.matched_reader_add(proxy);"
und entsprechend für PUBLICATIONS_ANNOUNCER, SUBSCRIPTIONS_DETECTOR,
SUBSCRIPTIONS_ANNOUNCER, TOPICS_DETECTOR, TOPICS_ANNOUNCER.

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`,
`crates/discovery/tests/opensplice_spdp_bundle.rs::the_participant_with_the_detectors_gets_the_local_sedp_writers_proxies`

**Status:** `done` — die Proxies erhalten die Metatraffic-Unicast-Locatoren
(ersatzweise die Default-Unicast-Locatoren), keine Multicast-Locatoren.

### §8.5.5.2 Teilnehmer entfernt: Proxies der SEDP-Endpunkte entfernen

**Spec:** §8.5.5.2, S. 136 — "Based on the remote Participant’s leaseDuration,
a local Participant ‘local_participant’ concludes that a previously discovered
Participant with GUID_t participant_guid is no longer present. The Participant
‘local_participant’ must reconfigure any local Endpoints that were
communicating with Endpoints in the Participant".

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_lost`);
`crates/dcps/src/runtime.rs` (Lease-Ablauf und Abmeldung)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::on_participant_lost_clears_proxies_and_cache`,
`crates/discovery/tests/cyclone_sedp_replay.rs::replay_after_participant_lost_stops_accepting_publications`,
`crates/dcps/src/runtime.rs::tests::participant_lease_lifecycle_hard_death_and_same_prefix`

**Status:** `done`

### §8.5.6 Andere Entdeckungsprotokolle

**Spec:** §8.5.6, S. 136 — "Remote Participants using a different vendor’s
RTPS implementation must be contacted using at least the SPDP to ensure
interoperability. There is no such requirement when the remote Participant
uses the same RTPS implementation."

**Repo:** `crates/dcps/src/runtime.rs` (SPDP läuft in jedem Teilnehmer;
Abkürzung im selben Prozess über `inproc_announce_publication`)

**Tests:** `crates/dcps/src/runtime.rs::tests::handle_spdp_registers_real_fastdds_participant`,
`crates/discovery/tests/fastdds_spdp_repro.rs::zerodds_parses_real_fastdds_spdp`

**Status:** `done`

### §8.5.6 Gemeinsames EDP

**Spec:** §8.5.6, S. 137 — "All Participants must support at least the SEDP, so
they always have at least one EDP in common. However, if two Participants both
support another EDP, this alternative protocol can be used instead."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Pflicht zu SEDP ist unter §8.5.1
geführt; der Rest ist eine Erlaubnis.

## §8.6 Versioning and Extensibility

### §8.6 Höhere Minor-Versionen verarbeiten

**Spec:** §8.6, S. 137 — "Implementations of this version of the RTPS protocol
should be able to process RTPS Messages not only with the same major version
but possibly higher minor versions."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: nur eine höhere
Major-Version wird abgewiesen)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_accepts_minor_version_above_supported`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_major_version_above_supported`

**Status:** `done`

### §8.6.1 Unbekannte Submessages überspringen

**Spec:** §8.6.1, S. 137 — "Additional Submessages with other submessageIds can
be introduced and used anywhere in an RTPS Message. An implementation should
skip over unknown Submessages using the submessageLength field in the
SubmessageHeader."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_unknown_submessage_without_must_understand`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — eine Submessage mit unbekannter SubmessageId wird nach `submessageLength` übersprungen, auch mit gesetztem Flag-Bit 7 (§8.3.4.1 Regel 3 kennt kein Must-Understand auf Submessage-Ebene).

### §8.6.1 Zusätzliche Felder am Ende einer Submessage überspringen

**Spec:** §8.6.1, S. 137 — "Additional fields can be added to the end of a
Submessage that was already defined in the current minor version. An
implementation should skip over additional fields using the submessageLength
field in the SubmessageHeader."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_submessage`); die `read_body` der Submessages in
`crates/rtps/src/submessages.rs`

**Tests:** `crates/rtps/src/datagram.rs::tests::additional_fields_at_the_end_of_a_known_submessage_are_skipped`

**Status:** `done` — HEARTBEAT, ACKNACK, GAP, HEARTBEAT_FRAG, NACK_FRAG, INFO_TS, INFO_SRC, INFO_REPLY, INFO_REPLY_IP4 und INFO_DST ergeben mit acht Oktetten mehr am Ende des Körpers denselben Wert wie ohne. Bei DATA und DATA_FRAG endet der Körper mit den Nutzdaten; neue Felder davor überspringt `octetsToInlineQos`.

### §8.6.1 Unbekannte Builtin-Endpunkte ignorieren

**Spec:** §8.6.1, S. 137 — "Additional built-in Endpoints with new IDs can be
added. An implementation should ignore any unknown built-in Endpoints."

**Repo:** `crates/discovery/src/sedp/stack.rs` (Zuordnung nur bekannter
EntityIds); `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`:
nur bekannte Bits)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::routing_unknown_with_wrong_writer_does_not_match`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.6.1 Unbekannte Parameter ignorieren

**Spec:** §8.6.1, S. 137 — "Additional parameters with new parameterIds can be
added. An implementation should ignore any unknown parameters."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::from_bytes`);
`crates/rtps/src/publication_data.rs` (Decoder übergehen unbekannte PIDs)

**Tests:** `crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`

**Status:** `done` — unbekannte PIDs mit Must-Understand-Bit sind unter
§9.4.2.11 geführt.

### §8.6.1 Erweiterungen erhöhen die Minor-Version

**Spec:** §8.6.1, S. 137 — "All such changes require an increase of the minor
version number."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anforderung an künftige Fassungen der
Spezifikation.

### §8.6.2 Unveränderliches innerhalb der Major-Version

**Spec:** §8.6.2, S. 137 — "A Submessage cannot be deleted. A Submessage cannot
be modified except as described in 8.6.1. The meaning of submessageIds cannot
be modified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anforderung an künftige Fassungen der
Spezifikation.

## §8.7 Implementing DDS QoS and advanced DDS features using RTPS

### §8.7 DDS-QoS und erweiterte Funktionen über RTPS

**Spec:** §8.7, S. 137 — "This sub clause defines how to use RTPS to implement
the DDS QoS parameters." "This sub clause forms a normative part of the
specification for the purpose of interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Anforderungen stehen in
§8.7.1 bis §8.7.10.

### §8.7.1 Inline-QoS des Writers auf Wunsch des Readers

**Spec:** §8.7.1, S. 138 — "A stateless Reader’s need for receiving in-line QoS
to get information on remote Writers is the justification for requiring a
Writer to send in-line QoS if the Reader requests them (8.4.2.2.2)." "For
immutable QoS, all RxO QoS are sent in-line … Mutable QoS relevant to the
Reader are sent in-line so they may take effect immediately".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`sample_inline_qos`:
`inline_writer_qos` für Reader mit `expectsInlineQos`);
`crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`,
`crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.1 TOPIC_NAME in der Inline-QoS

**Spec:** §8.7.1, S. 138 — "If in-line QoS parameters are expected,
implementations must also include the topic name as an in-line parameter."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::TOPIC_NAME` zuerst)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.1 Coherent-Set- und Filter-Parameter

**Spec:** §8.7.1, S. 138 — "Independent of whether Readers expect in-line QoS
parameters, a Data Submessage may also contain in-line parameters related to
coherent sets and content-filtered topics."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §8.7.3 und §8.7.6.

### §8.7.1 Zustandsbehaftete Reader dürfen Inline-QoS übergehen

**Spec:** §8.7.1, S. 138 — "For improved performance, stateful implementations
may ignore in-line QoS and instead rely solely on cached values obtained
through Discovery."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung; ZeroDDS-Reader
übernehmen die QoS des Writers aus SEDP.

### §8.7.2 Table 8.85 USER_DATA

**Spec:** §8.7.2, S. 138, Table 8.85 — "USER_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TOPIC_DATA

**Spec:** §8.7.2, S. 138, Table 8.85 — "TOPIC_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 GROUP_DATA

**Spec:** §8.7.2, S. 138, Table 8.85 — "GROUP_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DURABILITY

**Spec:** §8.7.2, S. 138, Table 8.85 — "DURABILITY — See 8.7.2.2.1 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DURABILITY`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DURABILITY_SERVICE

**Spec:** §8.7.2, S. 138, Table 8.85 — "DURABILITY_SERVICE — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 PRESENTATION

**Spec:** §8.7.2, S. 138, Table 8.85 — "PRESENTATION — See 8.7.2.2.2 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::PRESENTATION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DEADLINE

**Spec:** §8.7.2, S. 138, Table 8.85 — "DEADLINE — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DEADLINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 LATENCY_BUDGET

**Spec:** §8.7.2, S. 138, Table 8.85 — "LATENCY_BUDGET — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LATENCY_BUDGET`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 OWNERSHIP

**Spec:** §8.7.2, S. 138, Table 8.85 — "OWNERSHIP — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::OWNERSHIP`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 OWNERSHIP_STRENGTH

**Spec:** §8.7.2, S. 138, Table 8.85 — "OWNERSHIP_STRENGTH — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::OWNERSHIP_STRENGTH`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 LIVELINESS

**Spec:** §8.7.2, S. 138, Table 8.85 — "LIVELINESS — See 8.7.2.2.3 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LIVELINESS`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TIME_BASED_FILTER

**Spec:** §8.7.2, S. 139, Table 8.85 — "TIME_BASED_FILTER — See 8.7.2.2.4 —
No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 PARTITION

**Spec:** §8.7.2, S. 139, Table 8.85 — "PARTITION — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::PARTITION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 RELIABILITY

**Spec:** §8.7.2, S. 139, Table 8.85 — "RELIABILITY — See 8.7.2.2.5 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::RELIABILITY`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TRANSPORT_PRIORITY

**Spec:** §8.7.2, S. 139, Table 8.85 — "TRANSPORT_PRIORITY — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::TRANSPORT_PRIORITY`, wenn
nicht 0)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §8.7.2 Table 8.85 LIFESPAN

**Spec:** §8.7.2, S. 139, Table 8.85 — "LIFESPAN — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LIFESPAN`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DESTINATION_ORDER

**Spec:** §8.7.2, S. 139, Table 8.85 — "DESTINATION_ORDER — See 8.7.2.2.6 —
Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DESTINATION_ORDER`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 HISTORY

**Spec:** §8.7.2, S. 139, Table 8.85 — "HISTORY — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 RESOURCE_LIMITS

**Spec:** §8.7.2, S. 139, Table 8.85 — "RESOURCE_LIMITS — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 ENTITY_FACTORY

**Spec:** §8.7.2, S. 139, Table 8.85 — "ENTITY_FACTORY — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 WRITER_DATA_LIFECYCLE

**Spec:** §8.7.2, S. 139, Table 8.85 — "WRITER_DATA_LIFECYCLE — See 8.7.2.2.7 —
No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 READER_DATA_LIFECYCLE

**Spec:** §8.7.2, S. 139, Table 8.85 — "READER_DATA_LIFECYCLE — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: nicht in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2.1 Fehlende Inline-QoS: Vorgabe nach DDS

**Spec:** §8.7.2.1, S. 139 — "If a Reader expects to receive in-line QoS
parameters and any of these QoS parameters are missing, it will assume the
default value for that QoS parameter, where the default is defined by DDS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Bedingung tritt nicht ein:
ZeroDDS-Reader kündigen `expectsInlineQos` = FALSE an und übernehmen die QoS
des Writers aus SEDP.

### §8.7.2.1 TOPIC_NAME für selbstbeschreibende Nachrichten

**Spec:** §8.7.2.1, S. 139 — "In order to achieve self-describing messages, not
only the parameters defined in Table 8.85 have to be sent with the submessage,
but also a parameter TOPIC_NAME."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2.2.1 DURABILITY

**Spec:** §8.7.2.2.1, S. 139 — "While volatile and transient-local durability
do not affect the RTPS protocol, support for transient and persistent
durability may. This is not covered in the current version of the
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §8.7.2.2.2 PRESENTATION

**Spec:** §8.7.2.2.2, S. 139 — "Sub clause 8.7.5 defines how to implement the
GROUP ordered access policy of the PRESENTATION QoS. Sub clause 8.7.6 defines
how to implement the coherent access policy of the PRESENTATION QoS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §8.7.5 und §8.7.6.

### §8.7.2.2.3 LIVELINESS AUTOMATIC

**Spec:** §8.7.2.2.3, S. 139 — "DDS_AUTOMATIC_LIVELINESS_QOS : liveliness is
maintained through the BuiltinParticipantMessageWriter. … implementations must
refresh the Participant’s liveliness (i.e., send the ParticipantMessageData,
see (8.4.13.5) at a rate faster than the smallest lease duration among the
Writers."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_wlp_period`);
`crates/dcps/src/wlp.rs` (`WlpEndpoint::tick`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`

**Status:** `done`

### §8.7.2.2.3 LIVELINESS MANUAL_BY_PARTICIPANT

**Spec:** §8.7.2.2.3, S. 139 — "If the Participant has any
MANUAL_BY_PARTICIPANT Writers, implementations must check periodically to see
if write(), assert_liveliness(), dispose(), or unregister_instance() was called
for any of them. The period for this check equals the smallest lease duration
among the Writers. If any of the operations were called, implementations must
refresh the Participant’s liveliness (i.e., send the ParticipantMessageData,
see 8.4.13.5)."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_manual_by_participant`, Aufrufe in
`write_user_sample_*`, `write_user_lifecycle_with`,
`assert_writer_liveliness_eid`, `bump_manual_liveliness_count`); `crates/dcps/src/wlp.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_by_participant_writer_refreshes_the_participant_liveliness`,
`crates/dcps/src/wlp.rs::tests::refreshes_before_the_tick_share_one_manual_by_participant_sample`,
`crates/dcps/src/runtime.rs::tests::the_manual_liveliness_count_of_the_announcement_counts_up`

**Status:** `done` — `write`, `assert_liveliness`, `dispose` und `unregister_instance` eines MANUAL_BY_PARTICIPANT-Writers veranlassen eine ParticipantMessageData mit MANUAL_LIVELINESS_UPDATE beim nächsten Tick (statt einer Prüfung im Takt der kleinsten Lease) und erhöhen den `manualLivelinessCount` der SPDP-Ankündigung.

### §8.7.2.2.3 LIVELINESS MANUAL_BY_TOPIC

**Spec:** §8.7.2.2.3, S. 140 — "The standard RTPS Messages that result from
calling write(), dispose(), or unregister_instance() on a Writer Entity suffice
to assert the liveliness of a Writer with LIVELINESS QoS set to
MANUAL_BY_TOPIC. When assert_liveliness() is called, the Writer must send a
Heartbeat Message with final flag and liveliness flag set."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`liveliness_heartbeat`);
`crates/dcps/src/runtime.rs` (`assert_writer_liveliness_eid`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag`,
`crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`,
`crates/dcps/src/runtime.rs::tests::a_manual_writer_goes_not_alive_when_silent_and_back_when_it_writes`

**Status:** `done`

### §8.7.2.2.4 TIME_BASED_FILTER auf der Writer-Seite

**Spec:** §8.7.2.2.4, S. 140 — "Implementations may optimize bandwidth usage by
applying a time-based filter on the Writer side. … When one or more data
updates are filtered out on the Writer side, implementations must send a Gap
Submessage instead, indicating which samples were filtered out. This
Submessage must be sent before the next update".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Bedingung tritt nicht ein: ZeroDDS
wendet TIME_BASED_FILTER nur auf der Reader-Seite an (`tbf_should_deliver`).

### §8.7.2.2.5 RELIABILITY

**Spec:** §8.7.2.2.5, S. 140 — "Implementations must meet the reliable RTPS
protocol requirements for interoperability, defined in 8.4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; die Anforderungen sind unter §8.4.2
geführt.

### §8.7.2.2.6 DESTINATION_ORDER BY_SOURCE_TIMESTAMP

**Spec:** §8.7.2.2.6, S. 140 — "In order to implement the
DDS_BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS policy, implementations must
include an InfoTimestamp Submessage with every update from a Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_info_ts` vor jeder
DATA und jedem DATA_FRAG); `crates/dcps/src/runtime.rs` (jeder `write` erhält
einen Quellzeitstempel)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_lifecycle_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_writer.rs::tests::resend_carries_original_info_ts`

**Status:** `done`

### §8.7.2.2.7 WRITER_DATA_LIFECYCLE autodispose

**Spec:** §8.7.2.2.7, S. 140 — "If autodispose_unregistered_instances is
enabled, Data Messages that unregister an instance must also dispose it."

**Repo:** `crates/dcps/src/publisher.rs` (`unregister_instance`: DISPOSED mit
UNREGISTERED bei `autodispose_unregistered_instances`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** `done`

### §8.7.3 Filterung auf der Reader-Seite

**Spec:** §8.7.3, S. 140 — "When filtering on the Reader side only, samples
which do not pass the filter are simply dropped by the middleware. In this
case, no further extensions to RTPS are needed."

**Repo:** `crates/dcps/src/subscriber.rs` (Filter des ContentFilteredTopic)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filter.rs::filter_drops_samples_that_return_false`

**Status:** `done`

### §8.7.3 Filterung auf der Writer-Seite

**Spec:** §8.7.3, S. 140 — "In order to support Writer side filtering, standard
RTPS extension mechanisms are used to: Include Reader filter information during
the Endpoint discovery phase. Include filter results with each data sample."
§8.7.3.4, S. 142: "Writer side filtering constitutes an optimization and is
optional".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optional; ZeroDDS filtert nicht auf der
Writer-Seite. Die Filterangaben des Readers in SEDP sind unter §8.7.3.1
geführt.

### §8.7.3 Hinweise des Writers: ALIVE_FILTERED, ContentFilterInfo, ausgelassene Daten

**Spec:** §8.7.3, S. 140 — "The Writer may indicate to a Reader that a Sample
has been filtered … by sending a directed Data message that includes only the
key information (DataFlag=0), indicating in the Inline Qos that the instance
state is ALIVE_FILTERED." "Alternatively, the Writer may not send a Data
message at all. This is only allowed if the previous sample for that Instance
was already filtered for that Reader".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt.

### §8.7.3.1 ContentFilterProperty_t in DiscoveredReaderData

**Spec:** §8.7.3.1, S. 141 — "RTPS adds an additional ContentFilterProperty_t
field to DiscoveredReaderData, defined in Table 8.86."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::content_filter`, `ContentFilterProperty`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::subscription_with_content_filter_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.1 Table 8.86 contentFilteredTopicName

**Spec:** §8.7.3.1, S. 141, Table 8.86 — "contentFilteredTopicName: string —
Name of the Content-filtered Topic associated with the Reader. Must have
non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::content_filtered_topic_name`);
`crates/dcps/src/participant.rs` (`create_contentfilteredtopic` lehnt einen
leeren Namen ab)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_name`

**Status:** `done`

### §8.7.3.1 Table 8.86 relatedTopicName

**Spec:** §8.7.3.1, S. 141, Table 8.86 — "relatedTopicName: string — Name of
the Topic related to the Content-filtered Topic. Must have non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::related_topic_name`); `crates/dcps/src/topic.rs`
(`ContentFilteredTopic::property`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`

**Status:** `done`

### §8.7.3.1 Table 8.86 filterClassName

**Spec:** §8.7.3.1, S. 141, Table 8.86 — "filterClassName: string … Must have
non-zero length. RTPS predefines the following values: “DDSSQL” Default filter
class name if none specified."

**Repo:** `crates/rtps/src/subscription_data.rs` (`filter_class::DDSSQL`);
`crates/dcps/src/topic.rs` (`ContentFilteredTopic::content_filter_property`: `DDSSQL` oder die
registrierte Klasse)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`,
`crates/dcps/tests/custom_content_filter.rs::a_registered_filter_class_is_named_announced_and_applied`

**Status:** `done`

### §8.7.3.1 Table 8.86 filterExpression

**Spec:** §8.7.3.1, S. 141, Table 8.86 — "filterExpression: string — The actual
filter expression. Must be a valid expression for the filter class specified
using filterClassName. Must have non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::filter_expression`); `crates/dcps/src/participant.rs`
(`create_contentfilteredtopic` lehnt einen leeren Ausdruck ab)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_expression`

**Status:** `done`

### §8.7.3.1 Table 8.86 expressionParameters

**Spec:** §8.7.3.1, S. 141, Table 8.86 — "expressionParameters: stringSequence
— Defines the value for each parameter in the filter expression. Can have zero
length if the filter expression contains no parameters."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::expression_parameters`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.1 Writer ohne passende Filterklasse

**Spec:** §8.7.3.1, S. 141 — "In order for the Writer to apply the filter, it
must have been configured to handle filters of the specified filter class. If
not, the Writer will simply ignore the filter information and not filter any
data samples."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt.

### §8.7.3.1 Geänderte Filterparameter über die Endpunkt-Entdeckung

**Spec:** §8.7.3.1, S. 141 — "Each time the parameters are modified, the
updated information is exchanged using the Endpoint discovery protocol."

**Repo:** `crates/dcps/src/runtime.rs` (`update_user_reader_content_filter`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.2 Gefiltertes Sample anzeigen

**Spec:** §8.7.3.2, S. 141 — "When this happens, the Writer can use a Data
submessage that does not contain a Data payload (DataFlag=0) and sets
FilteredFlag=1, see 8.3.8.2.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt. Die Data-Submessage hat kein FilteredFlag; das
FilteredFlag gehört zu StatusInfo_t (§9.6.4.9).

### §8.7.3.3 GAP statt DATA und Filterergebnisse in der Inline-QoS

**Spec:** §8.7.3.3, S. 142 — "In that case, the Data submessage is replaced by
a Gap submessage." "The sample passes some or all of the filters. In that
case, the sample must still be sent and the writer must include information
with the Data submessage on what filters were applied and the according
result."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt.

### §8.7.3.3 Table 8.87 filterResult

**Spec:** §8.7.3.3, S. 142, Table 8.87 — "filterResult: FilterResult_t — For
each filter signature, the results indicate whether the sample passed the
filter."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO wird nur bei
Filterung auf der Writer-Seite gesendet; ZeroDDS-Reader filtern stets selbst.

### §8.7.3.3 Table 8.87 filterSignatures

**Spec:** §8.7.3.3, S. 142, Table 8.87 — "filterSignatures:
FilterSignature_t[] — A list of filters that were applied to the sample."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO wird nur bei
Filterung auf der Writer-Seite gesendet; ZeroDDS-Reader filtern stets selbst.

### §8.7.3.4 Reader filtern, wenn der Writer nicht filtert

**Spec:** §8.7.3.4, S. 142 — "Samples will always be filtered on the Reader
side if: The Writer side did not apply any filtering. The Writer side did not
apply the filter expected by the Reader. … The Reader side does not support
Writer side filtering".

**Repo:** `crates/dcps/src/subscriber.rs` (Filter je empfangenem Sample)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filter.rs::filter_applies_also_to_read_peek`

**Status:** `done`

### §8.7.3.4 Filterklasse DDSSQL

**Spec:** §8.7.3.4, S. 143 — "If an implementation supports Content-filtered
Topics, it must at least recognize the “DDSSQL” filter class, as mandated by
the DDS specification."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic`, SQL-Filter);
`crates/rtps/src/subscription_data.rs` (`filter_class::DDSSQL`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filtered_reader.rs::a_field_the_type_does_not_answer_delivers_nothing`

**Status:** `done`

### §8.7.3.4 Eigene Filterklassen registrierbar

**Spec:** §8.7.3.4, S. 143 — "For all other filter classes, both
implementations must allow the user to register the same custom filter class."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilter`, `ContentFilteredTopic`);
`crates/dcps/src/participant.rs` (`DomainParticipant::register_contentfilter`,
`lookup_contentfilter`, `unregister_contentfilter`, `create_contentfilteredtopic_with_class`)

**Tests:** `crates/dcps/tests/custom_content_filter.rs::a_registered_filter_class_is_named_announced_and_applied`,
`crates/dcps/tests/custom_content_filter.rs::the_registry_refuses_what_it_cannot_serve`

**Status:** `done` — die Anwendung registriert eine Filterklasse je Teilnehmer und Typ (`register_contentfilter::<T>(name, Arc<dyn ContentFilter<T>>)`; `DDSSQL` ist die eigene Klasse der Implementierung und nicht registrierbar; Namen bis 256 Oktette). `create_contentfilteredtopic_with_class` nennt die Klasse; die Klasse prüft Ausdruck und Parameter (`compile`, beim Anlegen und bei `set_filter_parameters`) und filtert die Samples eines DataReaders auf dem Topic (`evaluate`). Der Klassenname geht als `filterClassName` in `PID_CONTENT_FILTER_PROPERTY` des Readers. Eine Klasse, die ein Peer nicht registriert hat, ignoriert er nach §8.7.3.4, und die Samples werden Reader-seitig gefiltert; ZeroDDS filtert auf der Writer-Seite nicht.

### §8.7.3.4 Angaben für ALIVE_FILTERED trotz Filterung beim Writer

**Spec:** §8.7.3.4, S. 143 — "Even if the Writer is performing writer-side
filtering, the Writer must provide enough information for the Reader to
correctly transition the instance state to ALIVE_FILTERED."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt; jedes Sample geht an jeden gematchten Reader.

### §8.7.4 Zustandswechsel der Instanz über Data und Inline-QoS

**Spec:** §8.7.4, S. 143 — "RTPS uses regular Data Submessages and the in-line
QoS parameter extension mechanism to communicate instance state changes. The
serialized information within the inline QoS contains the new InstanceState".

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`);
`crates/rtps/src/reliable_writer.rs` (`write_lifecycle`: PID_STATUS_INFO)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §8.7.4 Nur der Schlüssel im SerializedPayload

**Spec:** §8.7.4, S. 143 — "When RTPS sends a Data Submessage to communicate
instance state changes it may include only the Key of the Data-Object within
the SerializedPayload submessage element".

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`:
`lifecycle_serialized_key`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §8.7.4 Registrierung erst mit dem ersten Wert

**Spec:** §8.7.4, S. 143 — "An implementation of RTPS is not required to
propagate registration changes until the DDS DataWriter writes the first value
for that Data-Object instance."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §8.7.4 Wert, der den Filter nicht passiert

**Spec:** §8.7.4, S. 143 — "If the previous update to the instance passed the
filter, then the Writer must send a Data Submessage that either includes the
data value, or else indicates the InstanceState is ALIVE_FILTERED." "If the
previous update to the instance did not pass the filter, then the Writer may
omit sending the Data Submessage to the Reader."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gilt nur bei Filterung auf der Writer-Seite;
ZeroDDS sendet jeden Wert.

### §8.7.4 Dispose- und Unregister-Nachricht

**Spec:** §8.7.4, S. 143–144 — "This so called “dispose/unregister message”
shall be sent if any of the following conditions is met: 1. The Reader does
not have a Content Filter. 2. The Writer has previously sent a Data message to
the Reader for that same instance. 3. The Reader has OWNERSHIP QosPolicy kind
EXCLUSIVE …"

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`: an jeden
gematchten Reader)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done` — ZeroDDS sendet die Nachricht in jedem Fall.

### §8.7.5 PID_GROUP_SEQ_NUM und PID_WRITER_GROUP_INFO

**Spec:** §8.7.5, S. 144 — "In order to support group ordered access, RTPS uses
the in-line QoS parameter extension mechanism to include additional
information with each CacheChange. … PID_GROUP_SEQ_NUM to contain the group
sequence number. PID_WRITER_GROUP_INFO to contain the WriterGroupInfo_t".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/reliable_writer.rs::tests::a_writer_outside_a_group_sends_no_group_parameter`,
`crates/rtps/src/inline_qos.rs::tests::group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064`,
`crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`

**Status:** `done`

### §8.7.5 Table 8.88 WriterGroupInfo_t writerSet

**Spec:** §8.7.5, S. 144, Table 8.88 — "writerSet: GroupDigest_t — Identifies
the set of Writer EntityIds that are announced in the DiscoveredWriterData
that belonged to the Publisher at the time the sample was written."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/writer_group.rs::tests::the_writer_set_follows_the_writers`,
`crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`

**Status:** `done`

### §8.7.5 Gruppen-Sequenznummer auf jedem Data und dem ersten DataFrag

**Spec:** §8.7.5, S. 144 — "When a Publisher is configured with access scope
GROUP, all Data submessages and the first DataFrag submessage from any Writer
within the Publisher are accompanied with a GROUP sequence number sent as part
of the in-line QoS. The GROUP sequence number is a strictly monotonically
increasing sequence number originating from the Publisher."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/reliable_writer.rs::tests::a_dispose_carries_the_set_and_group_parameters_of_its_change`,
`crates/rtps/src/writer_group.rs::tests::group_sequence_numbers_rise_by_one_per_change_from_one`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.7.5 Subscriber: Übergabe nach Gruppen-Sequenznummer

**Spec:** §8.7.5, S. 144–145 — "A sample with GROUP sequence number GSN can be
committed to the DDS DataReader from the Subscriber’s history cache if any of
the following conditions apply: GSN-1 has been already been committed. It has
been determined that none of the remote DataWriters that match reliable
DataReaders have GSN-1."

**Repo:** `crates/dcps/src/group_order.rs` (`GroupOrder`),
`crates/dcps/src/subscriber.rs` (`SubscriberInner::release_group`)

**Tests:** `crates/dcps/src/group_order.rs::tests::samples_of_two_writers_go_out_in_group_sequence_order`,
`crates/dcps/src/group_order.rs::tests::a_number_no_counted_writer_has_is_passed_once_a_heartbeat_announced_it`,
`crates/dcps/src/group_order.rs::tests::a_counted_writer_whose_heartbeat_range_holds_the_number_is_waited_for`,
`crates/dcps/src/group_order.rs::tests::a_gap_rules_the_number_out_for_its_writer`,
`crates/dcps/src/group_order.rs::tests::a_number_waits_for_the_copy_of_every_reader_of_its_writer`,
`crates/dcps/tests/group_access_e2e.rs::a_reader_of_one_topic_gets_its_samples_without_the_others_numbers_over_udp`,
`crates/dcps/tests/group_access_e2e.rs::two_readers_of_one_topic_each_get_every_sample_over_udp`

**Status:** `done`

### §8.7.5 Nur Writer mit Liveliness

**Spec:** §8.7.5, S. 145 — "The above rules should only take into consideration
DataWriters that have not lost their liveliness, see 8.7.2.2.3."

**Repo:** `crates/dcps/src/runtime.rs`
(`DcpsRuntime::user_reader_counted_writers`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_that_lost_its_liveliness_is_not_counted_for_the_group_order`

**Status:** `done`

### §8.7.6 Publisher- und Subscriber-Coherent-Sets

**Spec:** §8.7.6, S. 145 — "A “Publisher coherent set” is defined as the set of
all CacheChanges performed by all DataWriters in the Publisher delimited by the
operations begin_coherent_changes() and end_coherent_changes()." "What
constitutes a “Subscriber coherent set” depends on the PRESENTATION
access_scope of the Subscriber".

**Repo:** —

**Tests:** `crates/dcps/tests/group_access_e2e.rs::a_group_coherent_set_across_two_topics_appears_after_its_end_over_udp`,
`crates/dcps/tests/group_access_e2e.rs::a_group_coherent_set_across_two_topics_appears_after_its_end_same_runtime`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`

**Status:** `done`

### §8.7.6 Unvollständige Coherent-Sets verwerfen

**Spec:** §8.7.6, S. 145 — "Incomplete coherent sets shall not be added to the
history of the RTPS DataReaders and the corresponding CacheChanges shall be
discarded by the Subscriber."

**Repo:** —

**Tests:** `crates/dcps/src/group_order.rs::tests::a_set_short_of_one_member_is_discarded_and_counted`,
`crates/rtps/src/reliable_reader.rs::tests::a_set_short_of_a_change_ends_incomplete`,
`crates/rtps/src/reliable_reader.rs::tests::a_set_ended_by_a_sample_is_reported_in_front_of_it_when_incomplete`,
`crates/dcps/src/subscriber.rs::tests::an_incomplete_set_is_discarded`,
`crates/dcps/tests/group_access_e2e.rs::a_reader_made_inside_a_set_does_not_get_its_tail_same_runtime`

**Status:** `done`

### §8.7.6 TOPIC-Scope: PID_COHERENT_SET mit der ersten Sequenznummer

**Spec:** §8.7.6, S. 146 — "a coherent set is uniquely identified by the
sequence number of the first sample update belonging to the coherent set. All
sample updates belonging to the same coherent set contain an in-line QoS
parameter with this same sequence number." "the same approach is followed for
DataFrag Submessages."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::coherent_set_member_data_carries_pid_0x0056_with_set_first_sn_octets`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`

**Status:** `done`

### §8.7.6 Ende eines Coherent-Sets des Writers

**Spec:** §8.7.6, S. 146 — "The end of a Writer’s coherent set is defined by the
arrival of one of the following: A Data Submessage from this Writer that
belongs to a new coherent set. A Data Submessage from this Writer that does not
contain a coherent set in-line QoS parameter or alternatively, contains a
coherent set in-line QoS parameter with value SEQUENCENUMBER_UNKNOWN."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::end_coherent_set_emits_ecs_data_with_d_and_k_clear`,
`crates/rtps/src/reliable_reader.rs::tests::end_of_coherent_set_data_is_delivered_as_a_terminator`,
`crates/dcps/src/subscriber.rs::tests::every_end_of_coherent_set_form_releases_the_writers_set`,
`crates/dcps/src/subscriber.rs::tests::a_sample_outside_any_set_releases_the_set_ahead_of_itself`,
`crates/dcps/src/subscriber.rs::tests::a_member_of_a_new_set_releases_the_earlier_set`,
`crates/rtps/src/inline_qos.rs::tests::find_coherent_set_reads_sequencenumber_unknown_as_no_set`

**Status:** `done`

### §8.7.6 GROUP-Scope: drei Inline-QoS-Parameter

**Spec:** §8.7.6, S. 146 — "All Data submessages and the first DataFrag
submessage belonging to the same group coherent set shall have three in-line
QoS parameters: The PID_GROUP_SEQ_NUM … The PID_COHERENT_SET … The
PID_GROUP_COHERENT_SET".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`

**Status:** `done`

### §8.7.6 ECS-Data-Submessage von allen Writern

**Spec:** §8.7.6, S. 146 — "A group’s coherent set is marked as being finished
by sending an End Coherent Set (ECS) Data submessage from all Writers within
the Publisher." Eigenschaften: ohne serializedPayload, Gruppen-Sequenznummer
der letzten Probe + 1, nicht gefiltert, zählt nicht gegen Resource Limits,
Inline-QoS PID_GROUP_SEQ_NUM, PID_GROUP_COHERENT_SET, PID_WRITER_GROUP_INFO.

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/dcps/src/group_order.rs::tests::a_set_waits_for_the_end_of_every_counted_writer`

**Status:** `done`

### §8.7.6 Keine Writer hinzufügen oder entfernen während eines Coherent-Sets

**Spec:** §8.7.6, S. 146 — "Note that Writers are not allowed to be added or
removed from a Publisher from the time that a coherent set begins until after
it ends."

**Repo:** —

**Tests:** `crates/dcps/tests/group_access_e2e.rs::a_writer_is_not_created_or_deleted_while_a_group_coherent_set_is_open`

**Status:** `done`

### §8.7.6 DataReader: Gruppen-Coherent-Set übergeben

**Spec:** §8.7.6, S. 147 — "The group coherent set becomes ready to be
committed to the DDS DataReader once an ECS sample is committed to the
Subscriber and the ECS sample meets the criteria for being committed to the DDS
DataReader. Once the group coherent set becomes ready to be committed the
Subscriber shall determine if the subscriber-relevant coherent set is complete
and if so, make it available to the application."

**Repo:** —

**Tests:** `crates/dcps/src/group_order.rs::tests::the_group_coherent_set_of_table_9_23_goes_out_whole_after_its_end`,
`crates/dcps/src/group_order.rs::tests::a_set_is_complete_per_reader_of_each_writer`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_in_a_group_coherent_set_waits_for_its_end_over_udp`

**Status:** `done`

### §8.7.6 Inline-QoS eines Coherent-Set-Mitglieds auf dem ersten DataFrag

**Spec:** §8.7.6, S. 146 — "The remainder of the discussion only refers to Data
Submessages, but the same approach is followed for DataFrag Submessages."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_oversized_coherent_set_member_goes_out_as_marked_fragments`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.7.7 Directed Write: PID_DIRECTED_WRITE

**Spec:** §8.7.7, S. 147 — "RTPS supports directed writes by using the in-line
QoS parameter extension mechanism. The serialized information denotes the
GUIDs of the targeted reader(s)."

**Repo:** `crates/rtps/src/inline_qos.rs` (`directed_write_param`,
`find_directed_write`, `directed_write_accepts`); `crates/rtps/src/parameter_list.rs`
(`pid::DIRECTED_WRITE`); `crates/rtps/src/reliable_reader.rs` (`addressed_elsewhere`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::directed_write_param_carries_target_guid`,
`crates/rtps/src/inline_qos.rs::tests::find_directed_write_returns_target_when_present`,
`crates/rtps/src/reliable_reader.rs::tests::a_directed_sample_is_absorbed_by_a_reader_it_does_not_name`

**Status:** `done` — Kodierung und Lesen des Parameters; der Empfangspfad liest
jeden PID_DIRECTED_WRITE einer DATA, nicht nur den ersten. Kein Writer von
ZeroDDS sendet einen gerichteten Write; die Spec bietet ihn dem Writer an, sie
verlangt ihn nicht.

### §8.7.7 Directed Write: nur der adressierte Reader übernimmt

**Spec:** §8.7.7, S. 147 — "When a writer sends a directed sample, only
recipients with a matching GUID accept the sample; all other recipients
acknowledge but absorb the sample, as if it were a GAP message."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`addressed_elsewhere`,
`irrelevant_change_set`); `crates/rtps/src/inline_qos.rs` (`directed_write_accepts`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_directed_sample_is_absorbed_by_a_reader_it_does_not_name`,
`crates/rtps/src/inline_qos.rs::tests::directed_write_matches_reader_returns_false_for_other_target`

**Status:** `done` — ein Reader, den kein PID_DIRECTED_WRITE der DATA nennt,
bestätigt die Probe und verwirft sie wie nach einem GAP; ein genannter nimmt
sie an. Gemessen: grün auf dem Linux-Testhost.

### §8.7.8 Property Lists

**Spec:** §8.7.8, S. 147 — "The RTPS protocol supports Property Lists as
in-line parameters. Properties can then be propagated during Discovery or as
in-line QoS."

**Repo:** `crates/rtps/src/property_list.rs` (`WirePropertyList`);
`crates/rtps/src/participant_data.rs` (`properties`, PID_PROPERTY_LIST)

**Tests:** `crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — ZeroDDS überträgt Property Lists in SPDP; in SEDP und als
Inline-QoS nicht.

### §8.7.9 Original Writer Info: Weiterleitung kennzeichnen

**Spec:** §8.7.9, S. 147 — "This service that forwards messages needs to
indicate that the forwarded message belongs to the message-stream of another
writer, such that if the reader receives the same messages from another source
… it can treat them as duplicates."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_sample_on_behalf`,
`write_user_lifecycle_on_behalf`, `write_user_sample_relayed`);
`crates/durability-service/src/lib.rs` (`write_replay_sample`);
`crates/routing-service/src/forwarding.rs` (Relay-Modus)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_sample_written_on_behalf_of_another_writer_is_that_writers`,
`crates/dcps/src/runtime.rs::tests::a_dispose_written_on_behalf_of_another_writer_is_that_writers`,
`crates/durability-service/src/lib.rs::tests::a_replayed_sample_goes_out_on_behalf_of_its_writer`,
`crates/routing-service/tests/relay_route.rs::a_relay_route_forwards_on_behalf_of_the_writer`,
`crates/rtps/src/reliable_writer.rs::tests::a_lifecycle_marker_keeps_its_own_inline_qos_on_every_send`

**Status:** `done` — ein Writer sendet eine Probe, ein Dispose oder ein Unregister im
Namen eines anderen mit PID_ORIGINAL_WRITER_INFO, beim ersten Senden und bei
jeder Reparatur. Der Durability-Service sendet gespeicherte Proben so, der
Routing-Service im Relay-Modus (`Route::relay`) jede Probe und jede
Lebenszyklus-Marke. Gemessen: grün auf dem Linux-Testhost.

### §8.7.9 Original Writer Info: Übergabe an den Reader des Originals

**Spec:** §8.7.9, S. 147 — "When a RTPS Reader receives this information, it
will treat it as a normal CacheChange, but once the CacheChange is ready to be
committed to the DDS DataReader, it will not commit it. Instead, it will hand
if off to the HistoryCache of the RTPS Reader that is communicating with the
RTPS Writer indicated in the ORIGINAL_WRITER_INFO in-line QoS".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`original_writer_of`,
`commit_released`, `commit_forwarded`, `set_forwarder_trusted`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_forwarded_change_is_the_originals_and_its_direct_copy_a_duplicate`,
`crates/rtps/src/reliable_reader.rs::tests::a_change_from_the_original_is_not_handed_out_again_when_forwarded`,
`crates/rtps/src/reliable_reader.rs::tests::a_forwarded_change_waiting_in_the_originals_cache_is_a_duplicate`,
`crates/rtps/src/reliable_reader.rs::tests::a_forwarded_change_the_original_gapped_is_handed_out`,
`crates/rtps/src/reliable_reader.rs::tests::changes_forwarded_before_the_original_matched_are_not_delivered_again`,
`crates/rtps/src/reliable_reader.rs::tests::an_untrusted_forwarder_delivers_as_itself`,
`crates/rtps/src/reliable_reader.rs::tests::a_writer_that_comes_back_starts_a_new_stream`

**Status:** `done` — eine Änderung mit PID_ORIGINAL_WRITER_INFO geht als die des
Original-Writers hinaus, mit dessen Sequenznummer, auch wenn der Reader zum
Original keinen Proxy hat; eine, die der Reader vom Original oder einem
anderen Weiterleiter schon hat, ist ein Duplikat. Ein Writer, dem der Reader
nicht als Weiterleiter traut, liefert als er selbst (unter DDS-Security nur
mit `<relay>` im Grant, `dds-security-1.2` §7.1.1.4). Gemessen: grün auf dem
Linux-Testhost.

### §8.7.9 Table 8.89 originalWriterGUID

**Spec:** §8.7.9, S. 147, Table 8.89 — "originalWriterGUID: GUID_t — The GUID
of the RTPS Writer that first generated the message."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_be`

**Status:** `done` — kodiert, gesendet und beim Empfang ausgewertet (Items unter
§8.7.9).

### §8.7.9 Table 8.89 originalWriterSN

**Spec:** §8.7.9, S. 147, Table 8.89 — "originalWriterSN: SequenceNumber_t —
The Sequence Number of the CacheChange as sent from the original writer."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`

**Status:** `done` — kodiert, gesendet und beim Empfang ausgewertet (Items unter
§8.7.9).

### §8.7.10 KeyHash aus den Daten berechnen

**Spec:** §8.7.10, S. 148 — "When the key hash is not received by a DataReader,
it should be computed from the data itself. If there is no data in the
submessage, then a default zero-valued key hash should be used by the
DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`reader_key_hash_context`);
`crates/dcps/src/key_hash.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`,
`crates/dcps/src/runtime.rs::tests::lifecycle_marker_without_key_hash_gets_the_alive_instances_hash_and_holder`

**Status:** `done`

### §8.7.10 KeyHash nach §9.6.4.8 berechnen

**Spec:** §8.7.10, S. 148 — "A Key Hash, if present, shall be computed as
described in 9.6.4.3."

**Repo:** `crates/dcps/src/key_hash.rs`

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_holder_over_sixteen_is_hashed`,
`crates/dcps/src/key_hash.rs::tests::a_u32_goes_out_big_endian`

**Status:** `done` — der Verweis auf §9.6.4.3 ist in der Spec falsch; die
Berechnung steht in §9.6.4.8.

## §9 Platform Specific Model (PSM): UDP/IP

### §9.1 PSM UDP/IP: Einleitung

**Spec:** §9.1, S. 149 — "This clause defines the Platform Specific Model (PSM)
that maps the Protocol PIM to UDP/IP."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Wahl von UDP/IP, keine
Anforderung.

### §9.2.1 Namensraum

**Spec:** §9.2.1, S. 149 — "All the definitions in this document are part of
the “RTPS” name-space."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konvention des Dokuments.

### §9.2.2 IDL-Darstellung und CDR-Leitungsform

**Spec:** §9.2.2, S. 149 — "These definitions use the OMG IDL … When these
structures are sent on the wire, they are encoded using the corresponding CDR
representation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konvention; die einzelnen Abbildungen sind
unter §9.3 und §9.4 geführt.

### §9.2.3 Darstellung von Bits und Bytes

**Spec:** §9.2.3, S. 149–150 — "the leftmost bit (bit 7) is the most
significant bit"; Bytes in Zeilen zu vier.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notation des Dokuments.

### §9.3.1 GUID aus GuidPrefix und EntityId

**Spec:** §9.3.1, S. 150 — "The PIM defines the GUID as composed of a
GuidPrefix_t prefix capable of holding 12 bytes, and an EntityId_t entityId
capable of holding 4 bytes."

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`, `Guid::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/rtps/src/wire_types.rs::tests::guid_roundtrip`

**Status:** `done`

### §9.3.1.1 GuidPrefix_t

**Spec:** §9.3.1.1, S. 150 — "typedef octet GuidPrefix_t[12];"

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`

**Status:** `done`

### §9.3.1.1 GUIDPREFIX_UNKNOWN

**Spec:** §9.3.1.1, S. 150 — "#define GUIDPREFIX_UNKNOWN
{0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00}".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`

**Status:** `done`

### §9.3.1.2 EntityId_t

**Spec:** §9.3.1.2, S. 150 — "struct EntityId_t { OctetArray3 entityKey; octet
entityKind; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::entity_id_user_writer_with_key_layout`

**Status:** `done`

### §9.3.1.2 ENTITYID_UNKNOWN

**Spec:** §9.3.1.2, S. 150 — "#define ENTITYID_UNKNOWN {{0x00, 0x00, 0x00},
0x00}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done`

### §9.3.1.2 entityKey eindeutig im Participant

**Spec:** §9.3.1.2, S. 150 — "When not pre-defined (see below), the entityKey
field within the EntityId_t can be chosen arbitrarily by the middleware
implementation as long as the resulting EntityId_t is unique within the
Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::next_entity_key`, `entity_key_in_use`,
`pinned_entity_keys`, `new_group_entity_id`, `reserve_user_writer_id`, `reserve_user_reader_id`)

**Tests:** `crates/dcps/src/runtime.rs::tests::entity_keys_are_unique_across_groups_reserved_and_registered_endpoints`,
`crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_the_key_of_a_live_writer`

**Status:** `done` — der Zähler (24 Bit) vergibt jeden Schlüssel einmal; nach dem Umlauf überspringt er die Schlüssel registrierter Writer, Reader und angekündigter Topics sowie die der Publisher, Subscriber und reservierten, noch nicht registrierten Endpunkte. Ein Schlüssel eines gelöschten Endpunkts wird wieder vergeben; ein reservierter, nie registrierter Schlüssel bleibt belegt.

### §9.3.1.2 entityKind: Art der Entität in den oberen zwei Bits

**Spec:** §9.3.1.2, S. 150 — "The information on whether the object is a
built-in entity, a vendor-specific entity, or a user-defined entity is encoded
in the two most-significant bits of the entityKind." "a vendor-specific
Entity … can therefore be ignored by another vendor’s implementation".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::from_byte`,
`EntityKind::Other`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unnamed_kind_byte_is_kept`

**Status:** `done`

### §9.3.1.2 Table 9.1 unknown

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "unknown — 0x00 — 0xc0".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Unknown`,
`EntityKind::Other(0xC0)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.2 Table 9.1 Participant

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Participant — N/A — 0xc1".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Participant`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer (with Key)

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Writer (with Key) — 0x02 — 0xc2".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserWriterWithKey`,
`BuiltinWriterWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer (no Key)

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Writer (no Key) — 0x03 — 0xc3".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserWriterNoKey`,
`BuiltinWriterNoKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader (no Key)

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Reader (no Key) — 0x04 — 0xc4".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserReaderNoKey`,
`BuiltinReaderNoKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader (with Key)

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Reader (with Key) — 0x07 — 0xc7".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserReaderWithKey`,
`BuiltinReaderWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer Group

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Writer Group — 0x08 — 0xc8".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Other`, Oktett
unverändert)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader Group

**Spec:** §9.3.1.2, S. 151, Table 9.1 — "Reader Group — 0x09 — 0xc9".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Other`, Oktett
unverändert)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.3 Bedeutung der vordefinierten EntityIds

**Spec:** §9.3.1.3, S. 151 — "The meaning of these Entity IDs cannot change in
this major version (2) of the protocol, but future minor versions may add
additional reserved Entity IDs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anforderung an künftige Fassungen der
Spezifikation.

### §9.3.1.3 Table 9.2 ENTITYID_PARTICIPANT

**Spec:** §9.3.1.3, S. 151, Table 9.2 — "ENTITYID_PARTICIPANT = {{00,00,01},c1}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::PARTICIPANT`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER

**Spec:** §9.3.1.3, S. 151, Table 9.2 — "ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER =
{{00,00,02},c2}".

**Repo:** `EntityId::SEDP_BUILTIN_TOPICS_WRITER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — die Konstante trägt {{00,00,02},c2}; der Test prüft alle Werte der Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR

**Spec:** §9.3.1.3, S. 151, Table 9.2 — "ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR =
{{00,00,02},c7}".

**Repo:** `EntityId::SEDP_BUILTIN_TOPICS_READER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — die Konstante trägt {{00,00,02},c7}; der Test prüft alle Werte der Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER = {{00,00,03},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_publications_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_PUBLICATIONS_DETECTOR

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_PUBLICATIONS_DETECTOR = {{00,00,03},c7}".

**Repo:** `EntityId::SEDP_BUILTIN_PUBLICATIONS_READER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — die Konstante trägt {{00,00,03},c7}; der Test prüft alle Werte der Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_ANNOUNCER

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_ANNOUNCER = {{00,00,04},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER`)

**Tests:** `crates/discovery/tests/vendor_discovery_frames.rs::fastdds_announcements_read_and_write_back`

**Status:** `done` — der Test ordnet mitgeschnittene Ankündigungen nach dieser
EntityId zu.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_DETECTOR

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_DETECTOR = {{00,00,04},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_subscriptions_reader_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SPDP_BUILTIN_PARTICIPANT_ANNOUNCER

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SPDP_BUILTIN_PARTICIPANT_ANNOUNCER = {{00,01,00},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR = {{00,01,00},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER = {{00,02,00},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER`)

**Tests:** `crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done` — der Test läuft nur mit `--ignored` und einem
Cyclone-`ddsperf`.

### §9.3.1.3 Table 9.2 ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER

**Spec:** §9.3.1.3, S. 151, Table 9.2 —
"ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER = {{00,02,00},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER`)

**Tests:** `crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done` — der Test läuft nur mit `--ignored` und einem
Cyclone-`ddsperf`.

### §9.3.1.3 Table 9.3 DDS-Security: Bereich {ff,00,00}–{ff,ff,ff}

**Spec:** §9.3.1.3, S. 152, Table 9.3 — "EntityIds that have both an entityKey
in the range {ff, 00, 00} – {ff, ff, ff} and an entityKind in the range
0xc0-0xff (inclusive)."

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER` und die übrigen sicheren
Builtin-EntityIds)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_12_secure_entityids_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.3 DDS-Security: {{00,02,01},c3} und {{00,02,01},c4}

**Spec:** §9.3.1.3, S. 152, Table 9.3 — "DDS-Security 1.1 — {{00, 02, 01},
c3} and {{00, 02, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`,
`EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER`)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/stateless.rs::tests::reader_has_expected_entity_id`

**Status:** `done`

### §9.3.1.3 Table 9.3 DDS-XTypes: TypeLookup-EntityIds

**Spec:** §9.3.1.3, S. 152, Table 9.3 — "DDS-XTypes 1.2 — {{00, 03, 00}, c3},
{{00, 03, 00}, c4}, {{00, 03, 01}, c3}, {{00, 03, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::TL_SVC_REQ_WRITER`,
`TL_SVC_REQ_READER`, `TL_SVC_REPLY_WRITER`, `TL_SVC_REPLY_READER`)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** `done`

### §9.3.1.4 Table 9.4 Client (0x05)

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "Client — 0x05"; "These EntityIds
should not be used by future versions of the protocol unless they are used with
the same meaning as in versions prior to 2.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 Server (0x06)

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "Server — 0x06".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 writerApplications

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "writerApplications — {{00,00,01},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 readerApplications

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "readerApplications — {{00,00,01},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 writerClients

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "writerClients — {{00,00,05},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 readerClients

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "readerClients — {{00,00,05},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 writerServices

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "writerServices — {{00,00,06},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 readerServices

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "readerServices — {{00,00,06},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 writerManagers

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "writerManagers — {{00,00,07},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 readerManagers

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "readerManagers — {{00,00,07},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.4 Table 9.4 writerApplicationsSelf

**Spec:** §9.3.1.4, S. 152, Table 9.4 — "writerApplicationsSelf —
{{00,00,08},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veraltete Kennung; ZeroDDS verwendet sie
nicht.

### §9.3.1.5 GUID_t

**Spec:** §9.3.1.5, S. 152 — "struct GUID_t { GuidPrefix_t guidPrefix;
EntityId_t entityId; } ;"

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`

**Status:** `done`

### §9.3.1.5 Erste zwei Oktette des guidPrefix = vendorId

**Spec:** §9.3.1.5, S. 152–153 — "implementations of the RTPS protocol shall
set the first two bytes of the guidPrefix to match their assigned vendorId …
guidPrefix[0] = vendorId[0] guidPrefix[1] = vendorId[1]".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::zerodds`);
`crates/dcps/src/participant.rs` (`random_guid_prefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_host_id_follows_the_vendor_id`

**Status:** `partial` — die ersten zwei Oktette sind `01 F0`, kein von der OMG
zugewiesener Wert (`open`-Item unter §8.3.5.2). `DcpsRuntime::start` nimmt
auch einen frei gewählten Prefix an; unter DDS-Security ersetzt der aus der
Identität abgeleitete Prefix den gebildeten. Dessen erste 48 Bit legt
DDS-Security 1.2 §10.3.3 Table 59 (`validate_local_identity`) fest: erstes Bit 1,
dann 47 Bit aus dem SHA-256 des SubjectName; die VendorId steht dort nach der
spezielleren Spec nicht (`crates/security-pki/src/guid_adjust.rs`, Test
`adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`). Blocker: ein harter externer, die Zuweisung einer VendorId durch die OMG; der Antrag läuft seit dem 22.06. Bis dahin sendet ZeroDDS `0x01F0`, einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

### §9.3.1.5 GUID_UNKNOWN

**Spec:** §9.3.1.5, S. 153 — "#define GUID_UNKNOWN{ GUIDPREFIX_UNKNOWN,
ENTITYID_UNKNOWN }".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done`

### §9.3.2.1 DomainId_t

**Spec:** §9.3.2.1, S. 153 — "typedef unsigned long DomainId_t;"

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_id: Option<u32>`,
PID_DOMAIN_ID mit 4 Oktetten)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.3.2.1 Time_t-Struktur

**Spec:** §9.3.2.1, S. 153 — "struct Time_t { unsigned long seconds; // time in
seconds unsigned long fraction; // time in sec/2^32 };"

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`);
`crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`

**Status:** `done` — `seconds` und `fraction` sind je ein `unsigned long` (`HeTimestamp::seconds: u32`), 4 + 4 Oktette in der Byte-Reihenfolge der Submessage; Sekundenwerte ab 2^31 bleiben positiv (der Test `roundtrip_all_fields_set_be` nutzt 0xFFFF_FFFD). §9.4.2.9 zeigt `long seconds`; es gilt die Struktur aus §9.3.2.1.

### §9.3.2.1 Time_t: reservierte Werte und vorzeichenlose Sekunden

**Spec:** §9.3.2.1, S. 153 — "TIME_ZERO: seconds = 0, fraction = 0;
TIME_INVALID: seconds = 0xffffffff, fraction = 0xffffffff; TIME_INFINITE:
seconds = 0xffffffff, fraction = 0xfffffffe".

**Repo:** `crates/dcps/src/time.rs` (`time_to_he_timestamp`, `he_timestamp_to_time`); `crates/rtps/src/header_extension.rs` (`HeTimestamp`, `TIME_INVALID`)

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_time_t_values_cross_the_wire_as_the_spec_gives_them`,
`crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`

**Status:** `done` — `Time::INVALID` geht als {0xffffffff, 0xffffffff} und `Time::INFINITE` als {0xffffffff, 0xfffffffe} auf die Leitung und wird so zurückgelesen; `HeTimestamp::seconds` ist `u32`, `TIME_INVALID` hat Sekunden 0xFFFFFFFF. Die Aufrufer in dcps, xrce-agent und opcua-bridge sind angepasst (`cargo check --tests` und clippy grün). Ob ein anderer Hersteller die Sonderwerte als Quellzeitstempel sendet, ist nicht geprüft.

### §9.3.2.1 Duration_t, DURATION_ZERO, DURATION_INFINITE

**Spec:** §9.3.2.1, S. 153 — "DURATION_ZERO: seconds = 0, fraction = 0;
DURATION_INFINITE: seconds = 0x7fffffff, fraction = 0xffffffff; struct
Duration_t { long seconds; unsigned long fraction; };"

**Repo:** `crates/qos/src/duration.rs` (`Duration::INFINITE`, `Duration::ZERO`)

**Tests:** `crates/qos/src/duration.rs::tests::infinite_constant_matches_spec`,
`crates/qos/src/duration.rs::tests::zero_is_default_and_zero`

**Status:** `done`

### §9.3.2.1 VendorId_t und VENDORID_UNKNOWN

**Spec:** §9.3.2.1, S. 153 — "VENDORID_UNKNOWN: VendorId_t[0] = 0,
VendorId_t[1] = 0; typedef octet VendorId_t[2];"

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId`, `VendorId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_unknown_is_two_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`

**Status:** `done`

### §9.3.2.1 SEQUENCENUMBER_UNKNOWN

**Spec:** §9.3.2.1, S. 153 — "SEQUENCENUMBER_UNKNOWN: high = -1, low = 0".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`

**Status:** `done`

### §9.3.2.1 FragmentNumber_t

**Spec:** §9.3.2.1, S. 153 — "typedef unsigned long FragmentNumber_t;"

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`,
`crates/rtps/src/wire_types.rs::tests::fragment_number_wire_size_is_four`

**Status:** `done`

### §9.3.2.1 LOCATOR_KIND_INVALID, _RESERVED, _UDPv4, _UDPv6

**Spec:** §9.3.2.1, S. 153–154 — "const long LOCATOR_KIND_INVALID = -1; const
long LOCATOR_KIND_RESERVED = 0; const long LOCATOR_KIND_UDPv4 = 1; const long
LOCATOR_KIND_UDPv6 = 2;"

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Invalid`, `Reserved`,
`UdpV4`, `UdpV6`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_reserved_constant_kind_is_zero`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v4_any_kind_is_one`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v6_any_kind_is_two`

**Status:** `done`

### §9.3.2.1 LOCATOR_PORT_INVALID, LOCATOR_ADDRESS_INVALID, LOCATOR_INVALID

**Spec:** §9.3.2.1, S. 154 — "const unsigned long LOCATOR_PORT_INVALID = 0;
LOCATOR_ADDRESS_INVALID: {0,…,0}; LOCATOR_INVALID: kind = LOCATOR_KIND_INVALID,
port = LOCATOR_PORT_INVALID, address = LOCATOR_ADDRESS_INVALID".

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::INVALID`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_invalid_constant_matches_spec`,
`crates/rtps/src/wire_types.rs::tests::locator_invalid_kind_decoded`

**Status:** `done`

### §9.3.2.1 Locator_t

**Spec:** §9.3.2.1, S. 154 — "struct Locator_t { long kind; unsigned long
port; octet address[16]; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator`, `Locator::to_bytes_le`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`,
`crates/rtps/src/wire_types.rs::tests::locator_roundtrip_le`

**Status:** `done`

### §9.3.2.1 BEST_EFFORT = 1, RELIABLE = 2

**Spec:** §9.3.2.1, S. 154 — "The values of the following constants as defined
in the DDS Specification should be mapped to the below values before being sent
on the wire. const long BEST_EFFORT = 1; const long RELIABLE = 2;"

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind::BestEffort
= 1`, `Reliable = 2`); `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::reliability_try_from_u32_rejects_unknown`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`

**Status:** `done`

### §9.3.2.1 Count_t

**Spec:** §9.3.2.1, S. 154 — "typedef long Count_t;"

**Repo:** `crates/rtps/src/submessages.rs` (`count: i32` in ACKNACK,
HEARTBEAT, HEARTBEAT_FRAG, NACK_FRAG)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`

**Status:** `done`

### §9.3.2.1 ProtocolVersion_t

**Spec:** §9.3.2.1, S. 154 — "struct ProtocolVersion_t { octet major; octet
minor; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_aliases_match_spec`

**Status:** `done`

### §9.3.2.1 KeyHash_t, StatusInfo_t, ParameterId_t

**Spec:** §9.3.2.1, S. 154 — "typedef octet KeyHash_t[16]; typedef octet
StatusInfo_t[4]; typedef short ParameterId_t;"

**Repo:** `crates/rtps/src/inline_qos.rs` (PID_KEY_HASH mit 16 Oktetten,
PID_STATUS_INFO mit 4 Oktetten); `crates/rtps/src/parameter_list.rs`
(`Parameter::id: u16`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done` — ParameterId_t ist als `u16` geführt; die Bits 0x8000 und
0x4000 werden als Bitfelder ausgewertet (§9.4.2.11.2).

### §9.3.2.1 ContentFilterProperty_t

**Spec:** §9.3.2.1, S. 154 — "struct ContentFilterProperty_t { string<256>
contentFilteredTopicName; string<256> relatedTopicName; string<256>
filterClassName; string filterExpression; sequence<string>
expressionParameters; };"

**Repo:** `crates/rtps/src/subscription_data.rs` (`ContentFilterProperty`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/rtps/src/subscription_data.rs::tests::content_filter_names_are_bounded_by_string_256`

**Status:** `done` — Reihenfolge, Kodierung und die Grenze von 256 Oktetten für die drei Namen.

### §9.3.2.1 ContentFilterProperty_t: string<256>

**Spec:** §9.3.2.1, S. 154 — "string<256> contentFilteredTopicName;
string<256> relatedTopicName; string<256> filterClassName;"

**Repo:** `crates/rtps/src/subscription_data.rs` (`encode_content_filter_property_le`,
`decode_content_filter_property`); `crates/dcps/src/participant.rs` (`create_contentfilteredtopic`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_names_are_bounded_by_string_256`

**Status:** `done` — Senden und Lesen begrenzen die drei Namen auf 256 Oktette; `create_contentfilteredtopic` lehnt einen längeren Namen ab. Ein ContentFilterProperty mit zu langem oder sonst unlesbarem Namen gilt als nicht vorhanden: der Reader wird ohne Filter angenommen (er filtert selbst), gezählt in `DcpsRuntime::unreadable_content_filters` und als `sedp.content_filter.unreadable` protokolliert.

### §9.3.2.1 ContentFilterInfo_t, FilterResult_t, FilterSignature_t

**Spec:** §9.3.2.1, S. 154 — "typedef sequence<long> FilterResult_t; typedef
long FilterSignature_t[4]; … struct ContentFilterInfo_t { FilterResult_t
filterResult; FilterSignatureSequence filterSignatures; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO gehört zur Filterung
auf der Writer-Seite, die ZeroDDS nicht ausführt (§8.7.3.4).

### §9.3.2.1 Property_t und EntityName_t

**Spec:** §9.3.2.1, S. 154 — "struct Property_t { string name; string value;
}; typedef string EntityName_t;"

**Repo:** `crates/rtps/src/property_list.rs` (`WirePropertyList`)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`,
`crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`

**Status:** `done` — EntityName_t (PID_ENTITY_NAME) ist unter §9.6.3.2
geführt.

### §9.3.2.1 OriginalWriterInfo_t

**Spec:** §9.3.2.1, S. 154 — "struct OriginalWriterInfo_t { GUID_t
originalWriterGUID; SequenceNumber_t originalWriterSN; ParameterList
originalWriterQos; };"

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`, `find_original_writer_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — das Leitungsbild des Parameters sind `originalWriterGUID` und `originalWriterSN`, 24 Oktette (§9.6.4.7, Table 8.89 nennt dieselben zwei Attribute); `originalWriterQos` aus der IDL-Struktur hat in Leitungsbild und Table 8.89 keine Entsprechung. Sende- und Empfangspfad sind unter §8.7.9 `done`.

### §9.3.2.1 GroupDigest_t

**Spec:** §9.3.2.1, S. 155 — "typedef octet GroupDigest_t[4];"

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §9.3.2.1 BuiltinEndpointSet_t (IDL)

**Spec:** §9.3.2.1, S. 155 — "bitmask BuiltinEndpointSet_t { @position(0)
DISC_BUILTIN_ENDPOINT_PARTICIPANT_ANNOUNCER, … @position(29)
DISC_BUILTIN_ENDPOINT_TOPICS_DETECTOR };"

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done` — die Positionen sind unter §9.3.2.12 einzeln geführt.

### §9.3.2.1 BuiltinEndpointQos_t

**Spec:** §9.3.2.1, S. 155 — "bitmask BuiltinEndpointQos_t { @position(0)
BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER };"

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_qos::BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — Bitmaske mit `@position(0)` BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER.

### §9.3.2.1 ProtocolId_t (PROTOCOL_RTPS)

**Spec:** §9.3.2.1, S. 155 — "PROTOCOL_RTPS: ProtocolId_t[0] = 'R' …
ProtocolId_t[3] = 'S'; typedef octet ProtocolId_t[4];"

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`

**Status:** `done`

### §9.3.2.1 MessageLength_t und MESSAGE_LENGTH_INVALID

**Spec:** §9.3.2.1, S. 155 — "typedef unsigned long MessageLength_t; const
MessageLength_t MESSAGE_LENGTH_INVALID = 0;"

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length: Option<u32>`, `MESSAGE_LENGTH_INVALID`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_message_length_only_le`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_announcing_message_length_invalid_refuses_the_message`

**Status:** `done` — `u32`, 32 Bit ohne Vorzeichen; `MESSAGE_LENGTH_INVALID` ist 0 und der Anfangswert des Receivers (`None`). Eine HeaderExtension mit diesem Wert ist ungültig (messageLength kleiner als RTPS Header und HeaderExtension, §8.3.7.3), die ganze Nachricht wird abgewiesen.

### §9.3.2.1 UExtension4_t, WExtension8_t, Checksum32_t, Checksum64_t, Checksum128_t

**Spec:** §9.3.2.1, S. 155 — "typedef octet UExtension4_t[4]; typedef octet
WExtension8_t[8]; typedef octet Checksum32_t[4]; typedef octet
Checksum64_t[8]; typedef octet Checksum128_t[16];"

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4: [u8; 4]`,
`wextension8: [u8; 8]`, `ChecksumValue`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_le`,
`crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`

**Status:** `done` — die Byte-Reihenfolge der Prüfsummen ist unter §9.4.2.15
geführt.

### §9.3.2.2 Time_t im NTP-Format

**Spec:** §9.3.2.2, S. 156 — "time = seconds + (fraction / 2^(32))".

**Repo:** `crates/dcps/src/time.rs` (`time_to_he_timestamp`,
`he_timestamp_to_time`); `crates/qos/src/duration.rs`
(`fraction_of_nanosec`)

**Tests:** `crates/qos/src/duration.rs::tests::a_whole_nanosecond_survives_the_way_there_and_back`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done`

### §9.3.2.3 Duration_t im NTP-Format

**Spec:** §9.3.2.3, S. 156 — "time = seconds + (fraction / 2^(32))". "Versions
of the RTPS specification previous to version 2.4 did not specify the
representation of Duration_t, therefore implementations should take into
account the vendor and protocol version when interpreting these fields."

**Repo:** `crates/qos/src/duration.rs` (`Duration`, `to_bytes_le`,
`from_bytes_le`)

**Tests:** `crates/qos/src/duration.rs::tests::from_millis_splits_correctly`,
`crates/qos/src/duration.rs::tests::to_from_bytes_le_roundtrip`

**Status:** `done` — eine Unterscheidung nach Hersteller und Protokollversion
vor 2.4 gibt es nicht.

### §9.3.2.4 Locator_t: IPv4-Adresse

**Spec:** §9.3.2.4, S. 156 — "If the Locator_t kind is LOCATOR_KIND_UPDv4, the
address contains an IPv4 address. In this case, the leading 12 octets of the
address must be zero."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::udp_v4`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`

**Status:** `done`

### §9.3.2.4 Locator_t: IPv6-Adresse

**Spec:** §9.3.2.4, S. 156 — "If the Locator_t kind is LOCATOR_KIND_UPDv6, the
address contains an IPv6 address."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::udp_v6`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v6_constructor_keeps_full_address`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v6_roundtrip_le`

**Status:** `done`

### §9.3.2.4 Bereiche der Locator-Arten

**Spec:** §9.3.2.4, S. 156 — "0x00000003 - 0x01ffffff (inclusive) are reserved
for vendor-specific Locator_t kinds … 0x02000000 - 0x02ffffff (inclusive) are
reserved for future use by the RTPS specification … 0x03000000 and greater are
reserved for Locator_t kinds that identify a transport developed by a
third-party".

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind`, `LocatorKind::from_i32`, `set_tcp_kinds_fast_dds`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_locator_kinds_fast_dds`, Schlüssel `transport.tcp_locator_kinds`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_zerodds_locator_kinds_are_in_the_vendor_specific_range`,
`crates/rtps/src/wire_types.rs::tests::the_tcp_kinds_are_vendor_range_by_default_and_fast_dds_by_setting`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_rejects_unknown`,
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_kinds_are_zerodds_vendor_range_values`,
`crates/dcps/tests/runtime_config_layering.rs::the_tcp_locator_kinds_are_a_setting`,
`crates/transport-shm/src/posix.rs::tests::locator_kind_is_the_vendor_reserved_shm_value`,
`crates/transport-uds/src/lib.rs::tests::locator_kind_is_the_vendor_reserved_uds_value`,
`crates/rtps/src/submessages.rs::tests::info_reply_leaves_out_a_locator_of_a_kind_it_does_not_know`

**Status:** `done` — alle Arten außer UDPv4 (1) und UDPv6 (2) liegen im Herstellerbereich 3 bis 0x01ffffff: `Shm`, `Uds`, `Tsn` (0x01F0_0001 bis 0x01F0_0003) und jetzt auch `Tcpv4` (0x01F0_0004) und `Tcpv6` (0x01F0_0005). Die Werte 4 und 8 von Fast DDS (und der DDS-TCP-PSM) sendet ZeroDDS nur mit der benannten Einstellung `transport.tcp_locator_kinds: fast-dds` (Umgebung `ZERODDS_TCP_LOCATOR_KINDS`, Man-Seite, Beispielkonfiguration); gelesen werden beide Werte. Eine Art, die der Empfänger nicht kennt, macht eine INFO_REPLY nicht ungültig, der Locator entfällt. Erwartetes Verhalten von Fast DDS: mit `fast-dds` erkennt es die TCP-Locatoren von ZeroDDS (Art 4 und 8); ohne die Einstellung ignoriert es die Arten im Herstellerbereich und baut keine TCP-Verbindung zu ZeroDDS auf. Nicht gemessen.

### §9.3.2.5 GroupDigest_t

**Spec:** §9.3.2.5, S. 156 — "The GroupDigest_t is computed from an
EntityIdSet_t by first computing a 128 bit MD5 Digest (IETF RFC 1321) applied
to the CDR Big-Endian serialization of the structure EntityIdSet_t. The
GroupDigest_t is the leading 4 octets of the MD5 Digest. The empty group is
represented by a zero value of the GroupDigest_t."

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §9.3.2.6 Checksum32_t, Checksum64_t, Checksum128_t

**Spec:** §9.3.2.6, S. 157 — "Checksum32_t represents a 32-bit checksum.
Checksum64_t, and Checksum128_t represent a 64-bit, and 128-bit checksum,
respectively."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::Crc32c`,
`Crc64`, `Md5`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`,
`crates/rtps/src/header_extension.rs::tests::checksum_value_kind_matches`

**Status:** `done`

### §9.3.2.7 MessageLength_t

**Spec:** §9.3.2.7, S. 157 — "This type is used to represent the length of an
RTPS message. The representation uses a 32-bit unsigned integer."

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length: Option<u32>`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`,
`crates/rtps/src/header_extension.rs::tests::wire_layout_be_message_length_correct`

**Status:** `done`

### §9.3.2.8 UExtension4_t

**Spec:** §9.3.2.8, S. 157 — "This type is used to represent an undefined
4-byte value."

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4: [u8; 4]`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`

**Status:** `done`

### §9.3.2.9 WExtension8_t

**Spec:** §9.3.2.9, S. 157 — "This type is used to represent an undefined
8-byte value."

**Repo:** `crates/rtps/src/header_extension.rs` (`wextension8: [u8; 8]`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`

**Status:** `done`

### §9.3.2.10 SequenceNumber_t

**Spec:** §9.3.2.10, S. 157 — "struct SequenceNumber_t { long high; unsigned
long low; }; … sequence_number = low + high * 2^(32)".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::to_bytes_le`,
`from_bytes_le`, `to_bytes_be`, `from_bytes_be`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_split_high_low`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §9.3.2.11 ChangeCount_t

**Spec:** §9.3.2.11, S. 157 — "This type is used to represent a 64-bit count.
… struct ChangeCount_t { long high; unsigned long low; }; … change_count = low
+ high * 2^(32)".

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count: Option<u64>`, `write_body`, `read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — `ChangeCount_t` ist als `u64` geführt: `high` (`long`) und `low` (`unsigned long`) sind acht Oktette in der Byte-Reihenfolge der Submessage, `change_count = low + high * 2^32`; ein negatives `high` ist kein Zähler und wird abgewiesen.

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 0 bis 5

**Spec:** §9.3.2.12, S. 157–158 — "@position(0)
DISC_BUILTIN_ENDPOINT_PARTICIPANT_ANNOUNCER, @position(1) …_PARTICIPANT_DETECTOR,
@position(2) …_PUBLICATIONS_ANNOUNCER, @position(3) …_PUBLICATIONS_DETECTOR,
@position(4) …_SUBSCRIPTIONS_ANNOUNCER, @position(5) …_SUBSCRIPTIONS_DETECTOR".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::PARTICIPANT_ANNOUNCER`
bis `SUBSCRIPTIONS_DETECTOR`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 6 bis 9 veraltet

**Spec:** §9.3.2.12, S. 158 — "/* Positions 6-9 were deprecated in version 2.4
*/"; §9.3.2.1: "These bits should not be used by versions of the protocol
equal to or newer than the deprecated version".

**Repo:** `crates/rtps/src/participant_data.rs` (`ALL_STANDARD` ohne die Bits
6 bis 9)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_all_standard_excludes_secure_bits`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_standard_bits_by_default`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 10 und 11

**Spec:** §9.3.2.12, S. 158 — "@position(10)
BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_DATA_WRITER, @position(11)
BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_DATA_READER".

**Repo:** `crates/rtps/src/participant_data.rs`
(`endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER`, `…_READER`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 12 bis 15 (XTypes)

**Spec:** §9.3.2.12, S. 158 — "Positions 12-15 are reserved by DDS-XTypes 1.2
and its revisions thereof".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::TYPE_LOOKUP_REQUEST`,
`TYPE_LOOKUP_REPLY`)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::announces_type_lookup_endpoints_in_spdp`,
`crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 16 bis 27 (DDS-Security)

**Spec:** §9.3.2.12, S. 158 — "Positions 16-27 are reserved by DDS-Security 1.1
and revisions thereof".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::ALL_SECURE`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_all_secure_covers_bits_16_to_27`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: Positionen 28 und 29

**Spec:** §9.3.2.12, S. 158 — "@position(28)
DISC_BUILTIN_ENDPOINT_TOPICS_ANNOUNCER, @position(29)
DISC_BUILTIN_ENDPOINT_TOPICS_DETECTOR".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::TOPICS_ANNOUNCER`,
`TOPICS_DETECTOR`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`

**Status:** `done`

### §9.4.1 Submessages auf 32-Bit-Grenzen

**Spec:** §9.4.1, S. 158 — "The PSM aligns each Submessage on a 32-bit boundary
with respect to the start of the Message."

**Repo:** `crates/rtps/src/message_builder.rs` (`MessageBuilder::check_aligned`)

**Tests:** `crates/rtps/src/message_builder.rs::tests::nothing_is_appended_after_an_unaligned_submessage`,
`crates/rtps/src/message_builder.rs::tests::four_small_datas_aggregate_into_one_datagram`

**Status:** `done` — der Decoder verlangt die Ausrichtung beim Empfang nicht.

### §9.4.1 Länge der Nachricht aus dem Transport

**Spec:** §9.4.1, S. 158 — "A Message has a well-known length. This length is
not sent explicitly by the RTPS protocol but is part of the underlying
transport with which Messages are sent. In the case of UDP/IP, the length of
the Message is the length of the UDP payload."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: Länge des
übergebenen Datagramms)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`,
`crates/transport-udp/src/udp_transport.rs::tests::loopback_send_and_recv_delivers_datagram`

**Status:** `done`

### §9.4.2 Abbildung der Submessage-Elemente

**Spec:** §9.4.2, S. 158 — "This sub clause describes the PSM mapping for each
of the SubmessageElements defined by the PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Elemente folgen einzeln.

### §9.4.2.1 EntityId

**Spec:** §9.4.2.1, S. 159 — "typedef EntityId_t EntityId;" Leitungsbild
`octet value[4]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::to_bytes`,
`EntityId::from_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §9.4.2.2 GuidPrefix

**Spec:** §9.4.2.2, S. 159 — "typedef GuidPrefix_t GuidPrefix;" Leitungsbild
`octet value[12]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`,
`crates/rtps/src/header.rs::tests::header_guid_prefix_at_bytes_8_to_19`

**Status:** `done`

### §9.4.2.3 VendorId

**Spec:** §9.4.2.3, S. 159 — "typedef VendorId_t VendorId;" Leitungsbild
`octet vendorId[2]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`,
`crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`

**Status:** `done`

### §9.4.2.4 ProtocolVersion

**Spec:** §9.4.2.4, S. 159–160 — "typedef ProtocolVersion_t ProtocolVersion;"
Leitungsbild `octet major | octet minor`.

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_roundtrip`,
`crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`

**Status:** `done`

### §9.4.2.5 SequenceNumber

**Spec:** §9.4.2.5, S. 160 — Leitungsbild `long high`, `unsigned long low`.

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::to_bytes_le`,
`to_bytes_be`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: Struktur und Leitungsbild

**Spec:** §9.4.2.6, S. 160–161 — "struct SequenceNumberSet { SequenceNumber_t
bitmapBase; LongSeq8 bitmap; };" Leitungsbild: bitmapBase, `unsigned long
numBits`, `long bitmap[M]` mit "M = (numBits+31)/32". "The numBits field
encodes both the number of significant bits and the number of bitmap
elements."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::write_to`,
`read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::snset_wire_size_zero_bits_is_12_bytes`,
`crates/rtps/src/submessages.rs::tests::snset_wire_size_33_bits_is_20_bytes`,
`crates/rtps/src/submessages.rs::tests::snset_roundtrip_be`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: Zugehörigkeit nach Bitposition

**Spec:** §9.4.2.6, S. 160 — "(bitmap[deltaN/ 32] & (1 << (31 - deltaN%32))) ==
(1 << (31 - deltaN%32)) where deltaN = seqNum - seqNumSet.bitmapBase"; "In the
bitmap, the bit corresponding to sequence number bitmapBase is on the left."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::from_missing`,
`iter_set`: Bit `31 - offset % 32`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_iter_matches_missing`,
`crates/rtps/src/submessages.rs::tests::snset_roundtrip_le`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: Gültigkeit

**Spec:** §9.4.2.6, S. 160 — "A valid SequenceNumberSet must satisfy the
following conditions: bitmapBase >= 1; 0 <= numBits <= 256; there are
M=(numBits+31)/32 longs containing the pertinent bits".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::tests::seqnumset_rejects_num_bits_above_256`,
`crates/rtps/src/submessages.rs::tests::snset_decode_rejects_truncated_bitmap`,
`crates/rtps/tests/vendor_frames.rs::the_empty_acknack_of_opendds_and_rti_is_invalid_by_default`,
`crates/rtps/tests/vendor_frames.rs::opendds_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`,
`crates/rtps/src/submessages.rs::validity_tests::the_empty_set_with_base_zero_is_accepted_only_on_request`

**Status:** `done` — `bitmapBase >= 1`, `numBits <= 256` und die Zahl der Bitmap-Wörter
werden geprüft.
Vendor-Form: OpenDDS 3.34.0 und RTI Connext 7.7.0 senden als ACKNACK eines Readers
ohne Samples `readerSNState` `0/0:` (bitmapBase 0, numBits 0; gemessen in
`tools/wire-capture/rtps-sessions`: 40 und 8 ACKNACK). Standard: ungültig. Die
Einstellung `wire.accept_empty_acknack_base_zero` (Env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, Standard `false`) nimmt genau diese
leere Menge in einem ACKNACK an, prozessweit.

### §9.4.2.6 Table 9.5 Beispiel 1234/12:00110

**Spec:** §9.4.2.6, S. 161, Table 9.5 — "Example of bitmap: meaning of
“1234/12:00110”".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet`: `iter_set`, `write_to`, `read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_bitmap_1234_12_00110_of_table_9_5_names_1236_and_1237`

**Status:** `done` — bitmapBase 1234, numBits 12, Bits 00110: die Nummern 1236 und 1237 sind gesetzt, 1234, 1235 und 1238 bis 1245 nicht; das Bit für bitmapBase steht links (höchstwertiges Bit des ersten Worts). Der Test schreibt und liest das Beispiel in beiden Byte-Reihenfolgen.

### §9.4.2.7 FragmentNumber

**Spec:** §9.4.2.7, S. 161 — "typedef FragmentNumber_t FragmentNumber;"
Leitungsbild `unsigned long value`.

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber::to_bytes_le`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`

**Status:** `done`

### §9.4.2.8 FragmentNumberSet: Struktur und Leitungsbild

**Spec:** §9.4.2.8, S. 161–162 — "struct FragmentNumberSet { FragmentNumber_t
bitmapBase; LongSeq8 bitmap; };" "The interpretation matches that of a
SequenceNumberSet." Leitungsbild: bitmapBase, numBits, `long bitmap[M]`.

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::write_to`,
`read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::fnset_wire_size_formula`,
`crates/rtps/src/submessages.rs::tests::fnset_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done`

### §9.4.2.8 FragmentNumberSet: Gültigkeit

**Spec:** §9.4.2.8, S. 162 — "The interpretation matches that of a
SequenceNumberSet." (§9.4.2.6: "bitmapBase >= 1", "0 <= numBits <= 256").

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done` — `bitmapBase >= 1` und `numBits <= 256` werden geprüft.

### §9.4.2.9 Timestamp

**Spec:** §9.4.2.9, S. 162 — "typedef Time_t Timestamp;" Leitungsbild `long
seconds`, `unsigned long fraction`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`,
`timestamp_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`

**Status:** `done` — `HeTimestamp::seconds` ist `i32` wie im Leitungsbild;
§9.3.2.1 nennt `unsigned long seconds` (`open`-Item unter §9.3.2.1 Time_t).

### §9.4.2.10 LocatorList

**Spec:** §9.4.2.10, S. 162–163 — "typedef sequence<Locator_t, 8>
LocatorList;" Leitungsbild `unsigned long numLocators`, dann die Locatoren mit
`long kind`, `unsigned long port`, `octet address[16]`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage`,
`read_locator_list`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::info_reply_empty_unicast_list_is_valid`,
`crates/rtps/src/submessages.rs::validity_tests::info_reply_locator_list_holds_at_most_8_locators`

**Status:** `done` — Leitungsbild und die Grenze von 8 Locatoren stimmen.

### §9.4.2.10 LocatorList höchstens 8 Locatoren

**Spec:** §9.4.2.10, S. 162 — "typedef sequence<Locator_t, 8> LocatorList;"

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::read_locator_list`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::info_reply_locator_list_holds_at_most_8_locators`

**Status:** `done` — eine Liste mit mehr als 8 Locatoren wird abgewiesen.

### §9.4.2.11 Parameter: Ausrichtung und Länge

**Spec:** §9.4.2.11, S. 163 — "Each Parameter within the ParameterList starts
aligned on a 4-byte boundary with respect to the start of the ParameterList."
"The length encodes the number of octets following the length to reach the ID
of the next parameter … the length is always a multiple of four."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::to_bytes`,
`from_bytes_with_consumed`: `length % 4 != 0` wird abgewiesen)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::decode_rejects_non_aligned_length`,
`crates/rtps/src/parameter_list.rs::tests::parameter_value_is_padded_to_4_bytes`

**Status:** `done`

### §9.4.2.11 Parameter: CDR je Wert neu ausgerichtet

**Spec:** §9.4.2.11, S. 163 — "The value contains the CDR representation of the
Parameter type that corresponds to the specified parameterId. For alignment
purposes, the CDR stream is logically reset for each parameter value".

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/participant_data.rs` (jeder Wert wird ab Offset 0 kodiert)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.4.2.11 Mehrere Parameter mit derselben ParameterId

**Spec:** §9.4.2.11, S. 163 — "The ParameterList may contain multiple
Parameters with the same value for the parameterId. This is used to provide a
collection of values for that kind of Parameter."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::find_all`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_multiple_parameters`

**Status:** `done`

### §9.4.2.11 PID_SENTINEL

**Spec:** §9.4.2.11, S. 164 — "#define PID_SENTINEL (1)"; "The PID_SENTINEL is
used to terminate the parameter list and its length is ignored."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SENTINEL`;
`from_bytes_with_consumed` endet beim Sentinel ohne Auswertung der Länge)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`,
`crates/rtps/src/parameter_list.rs::tests::decode_rejects_missing_sentinel`

**Status:** `done`

### §9.4.2.11 PID_PAD

**Spec:** §9.4.2.11, S. 164 — "#define PID_PAD (0)"; "The PID_PAD is used to
enforce alignment of the parameter that follows and its length can be anything
(as long as it is a multiple of 4)."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.4.2.11 Sentinel in der HeaderExtension nicht verlangt

**Spec:** §9.4.2.11, S. 164 — "The presence of the PID_SENTINEL is not required
in the ParameterList that appears in the HeaderExtension. See 9.4.5.2. The
presence of the PID_SENTINEL is required in all other cases."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`,
`ParameterList::from_bytes_sentinel_optional`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_takes_a_parameter_list_without_sentinel`,
`crates/rtps/src/parameter_list.rs::tests::decode_rejects_missing_sentinel`

**Status:** `done` — die Parameter der HeaderExtension werden ohne PID_SENTINEL gelesen; in allen anderen Fällen wird der Sentinel verlangt (`decode_rejects_missing_sentinel`).

### §9.4.2.11 Table 9.6 Bit 0x8000: herstellerspezifisch

**Spec:** §9.4.2.11, S. 165, Table 9.6 — "ParameterId & 8000 — 0 — Reserved
ParameterId. — 1 — Vendor-specific ParameterId. Will not be recognized by
other vendors’ implementations."

**Repo:** `crates/rtps/src/parameter_list.rs` (`VENDOR_SPECIFIC_BIT`);
`crates/rtps/src/publication_data.rs` (Herstellerparameter nach VendorId)

**Tests:** `crates/discovery/tests/vendor_discovery_frames.rs::rti_announcements_read_and_write_back`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`

**Status:** `done`

### §9.4.2.11 Table 9.6 Bit 0x4000 = 0: unbekannt überspringen

**Spec:** §9.4.2.11, S. 165, Table 9.6 — "ParameterId & 4000 — 0 — If the
ParameterId is not recognized, skip and ignore the parameter."

**Repo:** `crates/rtps/src/parameter_list.rs`
(`validate_must_understand_in_data_pipeline`); Decoder in
`crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`

**Status:** `done`

### §9.4.2.11 Table 9.6 Bit 0x4000 = 1: unbekannt als Fehler

**Spec:** §9.4.2.11, S. 165, Table 9.6 — "1 — If the ParameterId is not
recognized, treat it as an error. If the ParameterId appears in the
HeaderExtension, ignore the entire RTPS message. If the ParameterId appears in
any other Submessage, ignore the Submessage and continue with the next
Submessage in the RTPS message, if any."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_submessage`);
`crates/rtps/src/parameter_list.rs`
(`validate_must_understand_in_data_pipeline`, `is_known_vendor_pid`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_data_with_an_unknown_must_understand_pid_is_skipped`,
`crates/rtps/src/datagram.rs::tests::a_data_with_a_vendor_specific_must_understand_pid_is_skipped`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`

**Status:** `done` — in der HeaderExtension wird die ganze Nachricht verworfen; eine DATA
oder DATA_FRAG mit einem nicht erkannten PID mit Bit 0x4000 wird übersprungen und
die nächste Submessage gelesen. Das gilt auch im herstellerspezifischen Bereich
(Bit 0x8000): erkannt werden dort nur die PIDs von ZeroDDS (0x8001, 0x8002) von
einem ZeroDDS-Sender.

### §9.4.2.11 Table 9.7 DDS-Security: 0x1000–0x10ff, 0x5000–0x50ff

**Spec:** §9.4.2.11, S. 165, Table 9.7 — "DDS-Security 1.1 — 0x1000-0x10ff and
0x5000-0x50ff".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IDENTITY_TOKEN`,
`PERMISSIONS_TOKEN`, `PARTICIPANT_SECURITY_INFO` u. a.)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`

**Status:** `done`

### §9.4.2.11 Table 9.7 DDS-XTypes: 0x0069, 0x0072–0x0075, 0x3f01–0x3fff, 0x7f01–0x7fff

**Spec:** §9.4.2.11, S. 165, Table 9.7 — "DDS-XTypes 1.2 — 0x0069;
0x0072-0x0075; 0x3f01-0x3fff; 0x7f01-0x7fff".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IGNORE = 0x3F03`,
PID_DATA_REPRESENTATION, PID_TYPE_CONSISTENCY, PID_TYPE_INFORMATION)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.4.2.11 Fehlender PID: Vorgabewert; unbekannter PID nach Table 9.6

**Spec:** §9.4.2.11, S. 165 — "If a ParameterId is expected, but not present,
the protocol will assume the default value. Similarly, if a ParameterId is
present but not recognized, the protocol will either skip and ignore the
parameter or treat the parameter as an incompatible QoS."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`, `crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::the_infinite_defaults_are_left_out_and_read_back`,
`crates/rtps/src/subscription_data.rs::tests::the_readers_infinite_defaults_are_left_out_and_read_back`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`

**Status:** `done` — die Vorgabe je PID ist unter §9.6.3.2 geführt, das
Must-Understand-Verhalten unter Table 9.6.

### §9.4.2.12 SerializedPayload

**Spec:** §9.4.2.12, S. 165 — "For the purpose of interoperability, all
implementations must however use a consistent representation (See, 10
Serialized Payload Representation)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; die Darstellung ist unter §10
geführt.

### §9.4.2.13 Count

**Spec:** §9.4.2.13, S. 166 — "typedef Count_t Count;" Leitungsbild `long
value`.

**Repo:** `crates/rtps/src/submessages.rs` (`count: i32`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_be`

**Status:** `done`

### §9.4.2.14 ChangeCount

**Spec:** §9.4.2.14, S. 166 — "typedef ChangeCount_t ChangeCount;"
Leitungsbild `long high`, `unsigned long low` (8 Oktette).

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (0,0)

**Spec:** §9.4.2.15, S. 166, Table 9.8 — "0, 0 — N/A — The messageChecksum is
not included in the HeaderExtension".

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumKind::None`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/header_extension.rs::tests::empty_he_le_roundtrip`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (0,1) CRC-32C

**Spec:** §9.4.2.15, S. 166, Table 9.8 — "0, 1 — Checksum32_t — … the
big-endian representation of the CRC-32C (Castagnoli) checksum of the RTPS
message. … It shall be serialized into the 4-bytes of the Checksum32_t type
using CDR big endian encoding."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc32c_matches_rfc4960_vector`,
`crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`,
`crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (1,0) CRC-64/XZ

**Spec:** §9.4.2.15, S. 166–167, Table 9.8 — "1, 0 — Checksum64_t — … the
big-endian representation of the CRC-64/XZ checksum of the RTPS message. … CDR
big endian encoding."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc64_xz_matches_known_vector`,
`crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`,
`crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (1,1) MD5

**Spec:** §9.4.2.15, S. 167, Table 9.8 — "1,1 — Checksum128_t — The
messageChecksum is a 128-bit checksum. It shall hold the MD5 digest of the
RTPS message."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_md5_matches_rfc1321_vector`,
`crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`

**Status:** `done`

### §9.4.2.15 Prüfsumme: Byte-Reihenfolge, Rechenbereich, Prüfung

**Spec:** §9.4.2.15, S. 167 — "the checksum shall be computed over the content
of the RTPS Message, which includes the RTPS Header and HeaderExtension
submessage. Moreover, for the purpose of computing the checksum, all the bytes
of the messageChecksum field in the RTPS HeaderExtension shall be set to
zero."; Table 9.8: "CDR big endian encoding".

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`,
`crates/rtps/src/datagram.rs::tests::a_message_with_a_wrong_checksum_is_refused_and_counted`

**Status:** `done` — CRC-32C und CRC-64 stehen in Big-Endian, die Prüfsumme ist über die ganze Nachricht mit genulltem Feld gerechnet, der Empfang prüft sie.

### §9.4.2.15 Table 9.9 Parameter von CRC-32C

**Spec:** §9.4.2.15, S. 168, Table 9.9 — Breite 32 Bit, Polynom 0x1EDC6F41,
Eingabe und Ergebnis gespiegelt, Startwert und Schluss-XOR 0xFFFFFFFF.

**Repo:** `crates/foundation` (`crc32c`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc32c_matches_rfc4960_vector`

**Status:** `done`

### §9.4.2.15 Table 9.10 Beispiele CRC-32C

**Spec:** §9.4.2.15, S. 168, Table 9.10 — "Example Checksum32_t computation":
`00 00 00 00` → 0x48674BC7, `FF FF FF FF` → 0xFFFFFFFF,
`33 22 55 AA BB CC DD EE FF` → 0xB59CA09B.

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::compute`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`

**Status:** `done` — die drei Beispiele der Tabelle (Eingabe, CRC-32C, Checksum32_t-Oktette).

### §9.4.2.15 Table 9.11 Parameter von CRC-64/XZ

**Spec:** §9.4.2.15, S. 168, Table 9.11 — Breite 64 Bit, Polynom
0x42F0E1EBA9EA3693, Eingabe und Ergebnis gespiegelt, Startwert und
Schluss-XOR 0xFFFFFFFFFFFFFFFF.

**Repo:** `crates/foundation` (`crc64_xz`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc64_xz_matches_known_vector`

**Status:** `done`

### §9.4.2.15 Table 9.12 Beispiele CRC-64/XZ

**Spec:** §9.4.2.15, S. 169, Table 9.12 — "Example Checksum64_t computation":
`00 00 00 00` → 0xF4A586351E1B9F4B, `FF FF FF FF` → 0xFFFFFFFF00000000,
`33 22 55 AA BB CC DD EE FF` → 0x701ECEB219A8E5D5.

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::compute`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`

**Status:** `done` — die drei Beispiele der Tabelle.

### §9.4.2.16 MessageLength

**Spec:** §9.4.2.16, S. 169 — "typedef MessageLength_t MessageLength;"
Leitungsbild `unsigned long length`.

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`

**Status:** `done`

### §9.4.2.17 UExtension4

**Spec:** §9.4.2.17, S. 169 — "typedef UExtension4_t UExtension4;"
Leitungsbild `octet value[4]`.

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`

**Status:** `done`

### §9.4.2.18 WExtension8

**Spec:** §9.4.2.18, S. 169 — "typedef WExtension8_t WExtension8;"
Leitungsbild `octet value[8]`.

**Repo:** `crates/rtps/src/header_extension.rs` (`wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`

**Status:** `done`

### §9.4.2.19 GroupDigest

**Spec:** §9.4.2.19, S. 169–170 — "typedef GroupDigest_t GroupDigest;"
Leitungsbild `octet value[4]`.

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §9.4.3.1 LocatorUDPv4

**Spec:** §9.4.3.1, S. 170, Table 9.13 — "The LocatorUDPv4 SubmessageElement
is identical to a LocatorList SubmessageElement containing a single locator of
kind LOCATOR_KIND_UDPv4." Leitungsbild `unsigned long address`, `unsigned long
port`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage::write_locator_udpv4`, `read_locator_udpv4`)

**Tests:** `crates/rtps/src/submessages.rs::tests::locator_udpv4_is_two_unsigned_longs_in_the_byte_order_of_the_submessage`,
`crates/rtps/src/submessages.rs::tests::info_reply_ip4_with_multicast_roundtrip_be`

**Status:** `done` — `address` und `port` sind je ein `unsigned long` in der Byte-Reihenfolge der Submessage; 192.0.2.10 ist die Zahl 0xC000020A (das erste Oktett höchstwertig). Zuvor stand die Adresse in jeder Byte-Reihenfolge als vier Oktette in Netzreihenfolge, was im Little-Endian-Fall abweicht. 2.5 definiert `LocatorUDPv4_t` nicht; die Zuordnung von a.b.c.d auf die Zahl ist eine Annahme. Kein Hersteller ist gemessen.

### §9.4.4 Header

**Spec:** §9.4.4, S. 170 — Leitungsbild `'R' 'T' 'P' 'S'`, `ProtocolVersion
version`, `VendorId vendorId`, `GuidPrefix guidPrefix`. "The structure of the
Header cannot change in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::to_bytes`, `from_bytes`)

**Tests:** `crates/rtps/src/header.rs::tests::header_roundtrip`,
`crates/rtps/src/header.rs::tests::header_total_size_is_20_bytes`

**Status:** `done`

### §9.4.4 Zugewiesene VendorId verwenden

**Spec:** §9.4.4, S. 170 — "To be compliant with the DDS Interoperability
Specification a vendor must have a reserved Vendor ID and use it."

**Repo:** —

**Tests:** —

**Status:** `open` — `VendorId::ZERODDS = [0x01, 0xF0]`
(`crates/rtps/src/wire_types.rs`), laut Kommentar ein Zwischenwert; wie unter §8.3.5.2. Blocker: ein harter externer, die Zuweisung einer VendorId durch die OMG; der Antrag läuft seit dem 22.06. Bis dahin sendet ZeroDDS `0x01F0`, einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

### §9.4.5.1 SubmessageHeader

**Spec:** §9.4.5.1, S. 171 — "struct SubmessageHeader { octet submessageId;
octet flags; unsigned short submessageLength; /* octetsToNextHeader */ };"
"This general structure cannot change in this major version (2) of the
protocol."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::to_bytes`,
`from_bytes`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_le`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_be`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind RTPS_HE = 0x00

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x00) RTPS_HE, /* HeaderExtension */".

**Repo:** `crates/rtps/src/header_extension.rs` (`SUBMESSAGE_ID_HEADER_EXTENSION = 0x00`), `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/tests/cyclone_he_must_understand.rs::local_encoder_produces_must_understand_bit`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind PAD = 0x01

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x01) PAD, /* Pad */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Pad`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind ACKNACK = 0x06

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x06) ACKNACK /* AckNack */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::AckNack`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_acknack_is_0x06`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind HEARTBEAT = 0x07

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x07) HEARTBEAT /* Heartbeat */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Heartbeat`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_heartbeat_is_0x07`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind GAP = 0x08

**Spec:** §9.4.5.1.1, S. 171 — "@value(8x08) GAP /* Gap */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Gap`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_gap_is_0x08`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_TS = 0x09

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x09) INFO_TS /* InfoTimestamp */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoTs`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_SRC = 0x0c

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x0c) INFO_SRC /* InfoSource */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoSrc`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_REPLY_IP4 = 0x0d

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x0d) INFO_REPLY_IP4 /* InfoReplyIp4 */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoReplyIp4`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_DST = 0x0e

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x0e) INFO_DST /* InfoDestination */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoDst`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_REPLY = 0x0f

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x0f) INFO_REPLY /* InfoReply */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoReply`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind NACK_FRAG = 0x12

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x12) NACK_FRAG /* NackFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::NackFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind HEARTBEAT_FRAG = 0x13

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x13) HEARTBEAT_FRAG /* HeartbeatFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::HeartbeatFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind DATA = 0x15

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x15) DATA /* Data */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Data`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_data_is_0x15`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind DATA_FRAG = 0x16

**Spec:** §9.4.5.1.1, S. 171 — "@value(0x16) DATA_FRAG /* DataFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::DataFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 Bedeutung der SubmessageIds unveränderlich

**Spec:** §9.4.5.1.1, S. 171 — "The meaning of the Submessage IDs cannot be
modified in this major version (2). Additional Submessages can be added in
higher minor versions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Anforderung an künftige Fassungen der
Spezifikation.

### §9.4.5.1.1 Kennungen 0x80–0xff nach VendorId

**Spec:** §9.4.5.1.1, S. 171 — "Submessages with ID's 0x80 to 0xff (inclusive)
are vendor-specific; they will not be defined by future versions of the
protocol. Their interpretation is dependent on the vendorId that is current
when the Submessage is encountered."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — ZeroDDS definiert keine eigene Submessage; jede Kennung
0x80–0xff wird übergangen.

### §9.4.5.1.1.1 Table 9.14 DDS-Security 0x30–0x3f

**Spec:** §9.4.5.1.1.1, S. 172, Table 9.14 — "DDS-Security 1.1 — 0x30-0x3f".

**Repo:** `crates/security-rtps/src/codec.rs` (`SEC_PREFIX = 0x31`, `SEC_BODY`,
`SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`)

**Tests:** `crates/security-rtps/src/codec.rs::tests::constants_match_spec`,
`crates/security-rtps/src/codec.rs::tests::three_submessages_carry_spec_ids_flags_and_lengths_in_order`

**Status:** `done`

### §9.4.5.1.2 EndiannessFlag im niederwertigsten Bit

**Spec:** §9.4.5.1.2, S. 172 — "The PSM maps the EndiannessFlag flag into the
least-significant bit (LSB) of the flags. … E=0 means big-endian, E=1 means
little-endian. … E = SubmessageHeader.flags & 0x01".

**Repo:** `crates/rtps/src/submessage_header.rs` (`FLAG_E_LITTLE_ENDIAN = 0x01`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_is_little_endian_flag`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_be`

**Status:** `done`

### §9.4.5.1.2 X-Flags: null senden, beim Empfang ignorieren

**Spec:** §9.4.5.1.2, S. 172 — "Implementations of RTPS version 2.5 should set
these to zero when sending and ignore these when receiving."

**Repo:** `crates/rtps/src/submessages.rs` (die Encoder setzen nur die
definierten Flags; die Decoder lesen nur diese); `crates/rtps/src/datagram.rs`

**Tests:** `crates/rtps/src/datagram.rs::tests::x_flags_are_zero_when_sending_and_ignored_when_receiving`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`,
`crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done` — die Encoder von HEARTBEAT, ACKNACK, GAP, HEARTBEAT_FRAG, NACK_FRAG, INFO_TS, INFO_SRC, INFO_REPLY, INFO_REPLY_IP4 und DATA setzen nur E und die für die Submessage definierten Flags; die Decoder geben mit allen übrigen Flag-Bits gesetzt denselben Wert. Bei unbekannten Submessages gilt §8.3.4.1 Regel 3.

### §9.4.5.1.3 octetsToNextHeader als ushort

**Spec:** §9.4.5.1.3, S. 172 — "The representation of this field is a CDR
unsigned short (ushort)."

**Repo:** `crates/rtps/src/submessage_header.rs` (`octets_to_next_header: u16`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_le`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader > 0

**Spec:** §9.4.5.1.3, S. 172 — "In case octetsToNextHeader > 0, it is the number
of octets from the first octet of the contents of the Submessage until the
first octet of the header of the next Submessage … OR it is the number of
octets remaining in the Message".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader == 0 außer PAD und INFO_TS

**Spec:** §9.4.5.1.3, S. 172 — "In case octetsToNextHeader==0 and the kind of
Submessage is NOT PAD or INFO_TS, the Submessage is the last Submessage in the
Message and extends up to the end of the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader == 0 bei PAD und INFO_TS

**Spec:** §9.4.5.1.3, S. 172 — "In case the octetsToNextHeader==0 and the kind of
Submessage is PAD or INFO_TS, the next Submessage header starts immediately
after the current Submessage header OR the PAD or INFO_TS is the last
Submessage in the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::an_info_ts_without_a_body_is_not_the_last_submessage`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

### §9.4.5.2 HeaderExtension: Leitungsbild

**Spec:** §9.4.5.2, S. 173 — `RTPS_HE |P|C|C|W|U|T|L|E|`, dann messageLength
[L], rtpsSendTimestamp [T], uExtension4 [U], wExtension8 [W], messageChecksum
[CC != 00], parameters [P].

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::encode_body`,
`decode_body`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_le`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension LengthFlag (0x02)

**Spec:** §9.4.5.2, S. 173 — "L = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_L`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::flag_byte_with_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension TimestampFlag (0x04)

**Spec:** §9.4.5.2, S. 173 — "The TimestampFlag is represented with the literal
‘T’." (Ausdruck im Text: "T = SubmessageHeader.flags & 0x04").

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_W`, Bit 2)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_timestamp_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension UExtension4Flag (0x08)

**Spec:** §9.4.5.2, S. 173 — "U = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_U`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension WExtension8Flag (0x10)

**Spec:** §9.4.5.2, S. 173 — "W = SubmessageHeader.flags & 0x10".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_V`, Bit 4)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension ChecksumFlags C1 (0x40), C2 (0x20)

**Spec:** §9.4.5.2, S. 173 — "C1 = SubmessageHeader.flags & 0x40; C2 =
SubmessageHeader.flags & 0x20".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_C_MASK`,
`ChecksumKind::from_flags`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension ParametersFlag (0x80)

**Spec:** §9.4.5.2, S. 174 — "P = SubmessageHeader.flags & 0x80".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_P`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.3 AckNack: Leitungsbild

**Spec:** §9.4.5.3, S. 174 — `ACKNACK |X|X|X|X|X|X|F|E|`, readerId, writerId,
SequenceNumberSet readerSNState, Count count.

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`

**Status:** `done`

### §9.4.5.3 AckNack FinalFlag (0x02)

**Spec:** §9.4.5.3, S. 174 — "The PSM maps the FinalFlag flag into the 2nd
least-significant bit (LSB) of the flags. … F = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`ACKNACK_FLAG_FINAL = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_acknack_preserves_final_flag`,
`crates/rtps/src/submessages.rs::tests::acknack_submessage_with_final_flag`

**Status:** `done`

### §9.4.5.3 AckNack F = 0: Writer antwortet mit Heartbeat

**Spec:** §9.4.5.3, S. 174 — "F=1 means the reader does not require a Heartbeat
from the writer. F=0 means the writer must respond to the AckNack message with
a Heartbeat message."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`note_acknack_final_flag`,
`tick`); `crates/rtps/src/reader_proxy.rs` (`heartbeat_asked`,
`heartbeat_is_asked`); Aufrufe in `crates/dcps/src/runtime.rs`,
`crates/discovery/src/sedp/stack.rs`, `crates/dcps/src/type_lookup_transport.rs`

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_acknack_with_the_final_flag_clear_is_answered_by_a_heartbeat`

**Status:** `done` — ein ACKNACK mit F = 0 wird mit einem HEARTBEAT beantwortet,
sofort, wenn der Reader noch keinen hatte, sonst frühestens eine Periode nach
dem letzten; Nutzer-, SEDP- und TypeLookup-Writer.

### §9.4.5.4 Data: Leitungsbild

**Spec:** §9.4.5.4, S. 174 — `DATA |X|X|X|N|K|D|Q|E|`, extraFlags,
octetsToInlineQos, readerId, writerId, writerSN, inlineQos [Q], serializedPayload
[D || K].

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::write_body`,
`read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_roundtrip_bit_identical`

**Status:** `done`

### §9.4.5.4 Data InlineQosFlag (0x02)

**Spec:** §9.4.5.4, S. 175 — "Q = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_INLINE_QOS = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/tests/vendor_frames.rs::cyclone_session_re_encodes_byte_for_byte`

**Status:** `done`

### §9.4.5.4 Data DataFlag (0x04)

**Spec:** §9.4.5.4, S. 175 — "D = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_DATA = 0x04`)

**Tests:** `crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_payload_decodes_and_carries_payload`,
`crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`

**Status:** `done`

### §9.4.5.4 Data KeyFlag (0x08)

**Spec:** §9.4.5.4, S. 175 — "K = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_KEY = 0x08`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_key_flag_roundtrip`,
`crates/discovery/src/sedp/reader.rs::tests::handle_data_reports_a_cyclone_dispose_without_key_hash_as_removed`

**Status:** `done`

### §9.4.5.4 Data NonStandardPayloadFlag (0x10)

**Spec:** §9.4.5.4, S. 175 — "N = SubmessageHeader.flags & 0x10".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_NON_STANDARD = 0x10`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done`

### §9.4.5.4 Data: Kombinationen von D und K

**Spec:** §9.4.5.4, S. 175 — "D=0 and K=0 means that there is no
serializedPayload SubmessageElement. D=1 and K=0 means that the
serializedPayload SubmessageElement contains the serialized Data. D=0 and K=1
means that the serializedPayload SubmessageElement contains the serialized
Key."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage`);
`crates/rtps/src/reliable_reader.rs` (`handle_data_into`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_be_with_empty_payload`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §9.4.5.4 Data: D = 1 und K = 1 ungültig

**Spec:** §9.4.5.4, S. 175 — "D=1 and K=1 is an invalid combination in this
version of the protocol."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — ein DATA mit D = 1 und K = 1 wird abgewiesen, mit dem Rest der
Nachricht.

### §9.4.5.4 Data extraFlags null

**Spec:** §9.4.5.4, S. 175 — "This version of the protocol should set all the
bits in the extraFlags to zero."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`extra_flags: 0`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_decode_accepts_nonzero_extra_flags_silently`,
`crates/rtps/tests/cyclone_compliance.rs::zerodds_writer_produces_compatible_data_layout`

**Status:** `done`

### §9.4.5.4 Data octetsToInlineQos

**Spec:** §9.4.5.4, S. 175 — "Implementations of the protocol that are
processing a received submessage should always use the octetsToInlineQos to
skip any submessage header elements it does not expect or understand and
continue to process the inlineQos SubmessageElement".

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::octets_to_inline_qos_skips_header_elements_of_a_later_version`,
`crates/rtps/src/submessages.rs::tests::data_submessage_octets_to_inline_qos_is_16`

**Status:** `done`

### §9.4.5.5 DataFrag: Leitungsbild

**Spec:** §9.4.5.5, S. 176 — `DATA_FRAG |X|X|X|X|N|K|Q|E|`, extraFlags,
octetsToInlineQos, readerId, writerId, writerSN, fragmentStartingNum,
`ushort fragmentsInSubmessage`, `ushort fragmentSize`, `unsigned long
sampleSize`, inlineQos [Q], serializedPayload.

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_be`

**Status:** `done`

### §9.4.5.5 DataFrag InlineQosFlag (0x02)

**Spec:** §9.4.5.5, S. 176 — "Q = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_INLINE_QOS = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §9.4.5.5 DataFrag KeyFlag (0x04)

**Spec:** §9.4.5.5, S. 176 — "K = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_KEY = 0x04`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §9.4.5.5 DataFrag NonStandardPayloadFlag (0x08)

**Spec:** §9.4.5.5, S. 176 — "N = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_NON_STANDARD = 0x08`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`,
`crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`

**Status:** `done`

### §9.4.5.6 Gap: Leitungsbild

**Spec:** §9.4.5.6, S. 177 — `GAP |X|X|X|X|X|F|G|E|`, readerId, writerId,
gapStart, gapList, gapStartGSN [G], gapEndGSN [G], ChangeCount filteredCount
[F].

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §9.4.5.6 Gap GroupInfoFlag (0x02)

**Spec:** §9.4.5.6, S. 177 — "G = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_GROUP_INFO = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.6 Gap FilteredCountFlag (0x04)

**Spec:** §9.4.5.6, S. 177 — "F = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_FILTERED_COUNT = 0x04`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.6 Gap gapStartGSN und gapEndGSN

**Spec:** §9.4.5.6, S. 177 — "G=1 means the Gap also includes a gapStartGSN and
a gapEndGSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.7 Heartbeat: Leitungsbild

**Spec:** §9.4.5.7, S. 178 — `HEARTBEAT |X|X|X|X|G|L|F|E|`, readerId, writerId,
firstSN, lastSN, Count count, currentGSN/firstGSN/lastGSN [G].

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_heartbeat_decodes`

**Status:** `done` — der Teil nach `count` ist unter „Heartbeat:
Group-Felder“ geführt.

### §9.4.5.7 Heartbeat FinalFlag (0x02)

**Spec:** §9.4.5.7, S. 178 — "F = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_FINAL = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §9.4.5.7 Heartbeat LivelinessFlag (0x04)

**Spec:** §9.4.5.7, S. 178 — "L = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_LIVELINESS = 0x04`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §9.4.5.7 Heartbeat GroupInfoFlag (0x08)

**Spec:** §9.4.5.7, S. 178 — "G = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_GROUP_INFO = 0x08`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`

**Status:** `done`

### §9.4.5.7 Heartbeat: Group-Felder

**Spec:** §9.4.5.7, S. 178 — "G=1 means the HeartBeat includes the currentGSN,
firstGSN, and lastGSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §9.4.5.8 HeartBeatFrag: Leitungsbild

**Spec:** §9.4.5.8, S. 179 — `HEARTBEAT_FRAG |X|X|X|X|X|X|X|E|`, readerId,
writerId, writerSN, lastFragmentNum, count.

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §9.4.5.9 InfoDestination: Leitungsbild

**Spec:** §9.4.5.9, S. 179 — `INFO_DST |X|X|X|X|X|X|X|E|`, GuidPrefix
guidPrefix.

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`,
`crates/rtps/src/datagram.rs::tests::info_dst_decodes_to_typed_destination_prefix`

**Status:** `done`

### §9.4.5.10 InfoReply: Leitungsbild

**Spec:** §9.4.5.10, S. 179 — `INFO_REPLY |X|X|X|X|X|X|M|E|`, LocatorList
unicastLocatorList, LocatorList multicastLocatorList [M].

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`

**Status:** `done`

### §9.4.5.10 InfoReply MulticastFlag (0x02)

**Spec:** §9.4.5.10, S. 180 — "M = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_REPLY_FLAG_MULTICAST = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_info_reply_with_multicast_via_datagram`

**Status:** `done`

### §9.4.5.11 InfoSource: Leitungsbild

**Spec:** §9.4.5.11, S. 180 — `INFO_SRC |X|X|X|X|X|X|X|E|`, `long unused`,
ProtocolVersion version, VendorId vendorId, GuidPrefix guidPrefix.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_wire_size_is_20`,
`crates/rtps/src/submessages.rs::tests::info_source_unused_field_roundtrips`

**Status:** `done`

### §9.4.5.12 InfoTimestamp: Leitungsbild

**Spec:** §9.4.5.12, S. 180 — `INFO_TS |X|X|X|X|X|X|I|E|`, Timestamp timestamp
[only if I==0].

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_decode_with_invalidate_ignores_body`

**Status:** `done`

### §9.4.5.12 InfoTimestamp InvalidateFlag (0x02)

**Spec:** §9.4.5.12, S. 180 — "I = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_TIMESTAMP_FLAG_INVALIDATE = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_invalidate_flag_yields_empty_body`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`

**Status:** `done`

### §9.4.5.13 Pad: Leitungsbild

**Spec:** §9.4.5.13, S. 180–181 — `PAD |X|X|X|X|X|X|X|E| octetsToNextHeader`.

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: PAD wird
übersprungen)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

### §9.4.5.14 NackFrag: Leitungsbild

**Spec:** §9.4.5.14, S. 181 — `NACK_FRAG |X|X|X|X|X|X|X|E|`, readerId,
writerId, writerSN, FragmentNumberSet fragmentNumberState, count.

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_nack_frag_roundtrip`

**Status:** `done`

### §9.4.5.15 InfoReplyIp4: Leitungsbild

**Spec:** §9.4.5.15, S. 181 — `INFO_REPLY_IP4 |X|X|X|X|X|X|M|E|`, LocatorUDPv4
unicastLocator, LocatorUDPv4 multicastLocator [M]. "Its use and
interpretation are identical to those of an InfoReply Submessage containing a
single unicast and possibly a single multicast locator".

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::locator_udpv4_is_two_unsigned_longs_in_the_byte_order_of_the_submessage`,
`crates/rtps/src/submessages.rs::tests::info_reply_ip4_unicast_only_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_ip4_sets_reply_locators`

**Status:** `done` — Aufbau (Unicast-LocatorUDPv4, mit M-Flag ein Multicast-LocatorUDPv4), Flags (E, M) und Wirkung stimmen; die Adresse folgt `LocatorUDPv4` (Item unter §9.4.3.1).

### §9.4.5.15 InfoReplyIp4 MulticastFlag (0x02)

**Spec:** §9.4.5.15, S. 182 — "M = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_ip4_with_multicast_roundtrip_be`

**Status:** `done`

### §9.5 Eine Nachricht je UDP-Datagramm

**Spec:** §9.5, S. 182 — "When RTPS is used over UDP/IP, a Message is the
contents (payload) of exactly one UDP/IP Datagram."

**Repo:** `crates/transport-udp/src/udp_transport.rs` (`UdpTransport`)

**Tests:** `crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`,
`crates/transport-udp/src/udp_transport.rs::tests::without_the_cache_a_largest_datagram_arrives`

**Status:** `done`

### §9.6.1 ParameterIds in der HeaderExtension

**Spec:** §9.6.1, S. 182 — "This version of the protocol does not specify any
ParameterId that may appear in the HeaderExtension. … Compliant implementations
encountering an unrecognized ParameterId within the HeaderExtension shall
either skip and the parameter or reject the entire RTPS Message, as specified
in Table 9.6."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`:
`validate_must_understand_in_data_pipeline` auf die Parameter der
HeaderExtension)

**Tests:** `crates/rtps/tests/cyclone_he_must_understand.rs::the_decoder_refuses_a_header_extension_with_an_unknown_must_understand_pid`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`

**Status:** `done` — ein nicht erkannter PID mit Bit 0x4000 in der HeaderExtension
verwirft die Nachricht, auch ein herstellerspezifischer (siehe Table 9.6); einer
ohne das Bit wird übergangen.

### §9.6.2.1 Table 9.15 SPDP_WELL_KNOWN_MULTICAST_PORT

**Spec:** §9.6.2.1, S. 182, Table 9.15 — "Multicast —
SPDP_WELL_KNOWN_MULTICAST_PORT — PB + DG * domainId + d0".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_four_ports_follow_the_spec_formula`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_0_is_7400`

**Status:** `done`

### §9.6.2.1 Table 9.15 SPDP_WELL_KNOWN_UNICAST_PORT

**Spec:** §9.6.2.1, S. 182, Table 9.15 — "Unicast — SPDP_WELL_KNOWN_UNICAST_PORT —
PB + DG * domainId + d1 + PG * participantId".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_unicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::domain_zero_ports_match_the_worked_spec_example`,
`crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`

**Status:** `done`

### §9.6.2.1 Eindeutige participantId je Knoten und Domain

**Spec:** §9.6.2.1, S. 182 — "Each Participant on the same node and in the same
domain must use a unique participantId." "participantId values ideally start
at 0 and are incremented for each additional Participant on the same node and
in the same domain."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::start`: participantId 0
bis 119, der erste freie SPDP-Unicast-Port; sonst ein freier Port)

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done` — ist keiner der 120 Ports frei, bindet ZeroDDS einen
beliebigen Port und ist nur über Multicast erreichbar.

### §9.6.2.1 SEDP auf den Ports von SPDP

**Spec:** §9.6.2.1, S. 182 — "The default ports used by the SEDP built-in
Endpoints match those used by the SPDP. If a node chooses not to use the
default ports for the SEDP, it can include the new port numbers as part of the
information exchanged during the SPDP."

**Repo:** `crates/dcps/src/runtime.rs` (Metatraffic-Locatoren = SPDP-Unicast-
Socket und SPDP-Multicast-Gruppe)

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`

**Status:** `done`

### §9.6.2.2 Table 9.16 Nutzerdaten: Multicast-Port

**Spec:** §9.6.2.2, S. 183, Table 9.16 — "Multicast — PB + DG * domainId + d2".
"User-defined Endpoints may choose to not use the default ports."

**Repo:** `crates/rtps/src/wire_types.rs` (`user_traffic_multicast_port`); `crates/dcps/src/runtime.rs` (`transport.user_multicast`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_four_ports_follow_the_spec_formula`,
`crates/dcps/src/runtime.rs::tests::the_user_multicast_port_is_d2_and_announced_unless_turned_off`

**Status:** `done` — ZeroDDS-Endpunkte kündigen den Multicast-Locator auf `PB + DG * domainId + d2` an (Vorgabe; `transport.user_multicast: false` für einen Host ohne Multicast). Erwartet: Cyclone DDS (kündigt 239.255.0.1 auf d2 an), Fast DDS und OpenDDS nehmen einen angekündigten Multicast-Locator an oder ignorieren ihn; ZeroDDS-Writer senden selbst nur an Unicast-Locators.

### §9.6.2.2 Table 9.16 Nutzerdaten: Unicast-Port

**Spec:** §9.6.2.2, S. 183, Table 9.16 — "Unicast — PB + DG * domainId + d3 + PG
* participantId". "In that case, remote Endpoints obtain the port number as
part of the information exchanged during the Simple Endpoint Discovery
Protocol."

**Repo:** `crates/rtps/src/wire_types.rs` (`user_traffic_unicast_port`);
`crates/dcps/src/runtime.rs` (Nutzer-Socket auf einem frei gewählten Port,
angekündigt als Default-Unicast-Locator)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_four_ports_of_one_participant_are_distinct`,
`crates/dcps/src/runtime.rs::tests::announce_locators_automatic_is_non_empty_and_includes_probe`

**Status:** `done` — ZeroDDS verwendet nicht den Vorgabeport, sondern kündigt
den gebundenen an, wie der Abschnitt erlaubt.

### §9.6.2.3 Vorgabewerte PB, DG, PG, d0–d3

**Spec:** §9.6.2.3, S. 183 — "PB = 7400 DG = 250 PG = 2 d0 = 0 d1 = 10 d2 = 1 d3
= 11".

**Repo:** `crates/rtps/src/wire_types.rs` (`SPDP_PORT_BASE`,
`SPDP_DOMAIN_GAIN`, `SPDP_PARTICIPANT_GAIN` und die Offsets)

**Tests:** `crates/rtps/src/wire_types.rs::tests::port_formula_constants_match_spec`

**Status:** `done`

### §9.6.2.3 Parameter für den Anwender einstellbar

**Spec:** §9.6.2.3, S. 183 — "Implementations must expose these parameters so
they can be customized by the user."

**Repo:** `crates/rtps/src/wire_types.rs` (`PortMapping`); `crates/dcps/src/runtime.rs`
(`RuntimeConfig::port_mapping`, `select_user_transport`, `user_port_fallbacks`, `transport.user_multicast`);
`man/man5/zerodds.yaml.5`

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_port_mapping_formulas_use_the_given_parameters`,
`crates/dcps/tests/runtime_config_layering.rs::the_port_mapping_parameters_are_settings`,
`crates/dcps/src/runtime.rs::tests::the_spdp_unicast_port_follows_the_configured_port_mapping`,
`crates/dcps/src/runtime.rs::tests::the_user_unicast_socket_takes_the_d3_port_of_its_participant_id`,
`crates/dcps/src/runtime.rs::tests::a_taken_user_port_is_counted_and_the_socket_takes_an_ephemeral_one`,
`crates/dcps/src/runtime.rs::tests::the_user_multicast_port_is_d2_and_announced_unless_turned_off`

**Status:** `done` — PB, DG, PG und d0 bis d3 sind Einstellungen (Datei und Umgebung, 0..=65535, Vorgabe die Spec-Werte). Der Runtime bindet die Discovery-Ports (d0, d1) und den Nutzer-Unicast-Socket auf `PB + DG * domainId + d3 + PG * participantId` mit der participantId des SPDP-Unicast-Sockets; ist der Port belegt, nimmt der Socket einen flüchtigen Port, das wird gezählt (`user_port_fallbacks`) und als Ereignis `transport.user_unicast.port_taken` protokolliert. Der Nutzer-Multicast-Port d2 wird mit `transport.user_multicast` gebunden (Beitritt zur SPDP-Gruppe, Empfang, Ankündigung als Default-Multicast-Locator); Vorgabe an (die Default-Locators von §9.6.2.3 enthalten d2), `false` für einen Host ohne Multicast. Gemessen in `tools/wire-capture/rtps-sessions` (Locators der SPDP-Ankündigungen): Fast DDS 2.6.10 und RTI 7.7.0 binden d1/d3 + PG * participantId (Fast DDS 23160/23161, 23162/23163; RTI 22910/22911, 22912/22913); Cyclone 11.0.1 nimmt flüchtige Ports für Metatraffic und Nutzerdaten (derselbe Port) und kündigt Default-Multicast 239.255.0.1 auf d2 an; OpenDDS 3.34.0 nimmt einen flüchtigen Metatraffic-Port und kündigt als Nutzer-Locator 127.0.0.1:12345 an (kein d3-Port). Nicht gemessen: ein Lauf gegen die Hersteller mit der Port-Änderung und das Verhalten von ZeroDDS-Writern gegenüber angekündigten Multicast-Locators (sie senden nur an Unicast-Locators).

### §9.6.2.4 Default-Multicast-Adresse 239.255.0.1

**Spec:** §9.6.2.4, S. 183 — "DefaultMulticastLocator = {LOCATOR_KIND_UDPv4,
“239.255.0.1”, PB + DG * domainId + d0}. All Participants must announce and
listen on this multicast address."

**Repo:** `crates/rtps/src/wire_types.rs` (`SPDP_DEFAULT_MULTICAST_ADDRESS`);
`crates/dcps/src/runtime.rs` (`spdp_multicast_group`, Beitritt und
Ankündigung)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_default_multicast_is_239_255_0_1`,
`crates/discovery/tests/spdp_loopback_e2e.rs::loopback_multicast_self_discovery`

**Status:** `done` — die Gruppe ist einstellbar; ohne Multicast arbeitet
ZeroDDS mit Initial-Peers.

### §9.6.2.4 Default-Ankündigungsrate 30 Sekunden

**Spec:** §9.6.2.4, S. 183 — "The default rate by which SPDP periodic
announcements are sent equals 30 seconds. SPDPbuiltinParticipantWriter.
resendPeriod = {30, 0};"

**Repo:** `crates/dcps/src/runtime.rs` (`DEFAULT_SPDP_PERIOD`, `default_spdp_period_for`); `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_SPDP_PERIOD` = 30 s (YAML-Vorgabe, Man-Seite, Beispielkonfiguration); die früheren 5 s bleiben über `discovery.spdp_period_ms: 5000` erreichbar. Eine Lease unter 90 s verkürzt die Vorgabe auf ein Drittel (§8.5.3.1).

### §9.6.3.1 ParticipantMessageData: Struktur und Leitungsbild

**Spec:** §9.6.3.1, S. 184 — "struct ParticipantMessageData { GuidPrefix_t
participantGuidPrefix; OctetArray4 kind; OctetSeq data; };" Leitungsbild:
`GuidPrefix_t participantGuidPrefix` (12 Oktette), `octet[4] kind`,
`unsigned long data.length`, `octet[] data.value`.

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`, `to_cdr`, `from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_with_topic_token`

**Status:** `done`

### §9.6.3.1 ParticipantMessageData: Werte von kind

**Spec:** §9.6.3.1, S. 184 — "#define PARTICIPANT_MESSAGE_DATA_KIND_UNKNOWN
{0x00, 0x00, 0x00, 0x00} #define
PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE {0x00, 0x00, 0x00,
0x01} #define PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_LIVELINESS_UPDATE {0x00,
0x00, 0x00, 0x02}"; "kind.value[0] & 0x80 == 1 // vendor specific kind".

**Repo:** `crates/rtps/src/participant_message_data.rs` (`PARTICIPANT_MESSAGE_DATA_KIND_*`, `ParticipantMessageData::is_vendor_kind`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_kind_constants_match_spec`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`

**Status:** `done`

### §9.6.3.1 ParticipantMessageData: data mindestens 128 Oktette

**Spec:** §9.6.3.1, S. 184 — "Implementations can decide the upper length of the
data field but must be able to support at least 128 bytes."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`MAX_DATA_LEN = 4096`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_rejects_overlong_data`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_encoder_caps_data_length`

**Status:** `done`

### §9.6.3.2 IDL der SPDP- und SEDP-Datentypen

**Spec:** §9.6.3.2, S. 184–185 — "struct SPDPdiscoveredParticipantData { …
ParticipantProxy participantProxy; Duration_t leaseDuration; }; struct
DiscoveredWriterData { … WriterProxy mWriterProxy; }; struct
DiscoveredReaderData { … ReaderProxy mReaderProxy; ContentFilterProperty_t
contentFilter; }; struct DiscoveredTopicData { DDS::TopicBuiltinTopicData
ddsTopicData; };"

**Repo:** `crates/rtps/src/participant_data.rs`,
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done` — die fehlenden Attribute sind unter §8.5.3.2 und §8.5.4.4
geführt.

### §9.6.3.2 Discovery-Daten als ParameterList in CDR

**Spec:** §9.6.3.2, S. 185 — "the wire-representation of the SerializedData
within the Data Submessage uses the format of a ParameterList
SubmessageElement. … Within each parameter, the parameter value is represented
using CDR."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`,
`to_pl_cdr_be`, `from_pl_cdr_le`); `crates/rtps/src/publication_data.rs`
(`to_pl_cdr_le`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done`

### §9.6.3.2 Herstellerspezifisches EDP über eigenen ParameterId

**Spec:** §9.6.3.2, S. 185 — "As this is a vendor-specific parameterId, other
vendors’ implementations would simply ignore the parameter and the information
it contains."

**Repo:** `crates/rtps/src/publication_data.rs` (Herstellerparameter nach
VendorId)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendors_0x8002_is_not_read_as_a_type_identifier`

**Status:** `done`

### §9.6.3.2 Keine redundanten Parameter

**Spec:** §9.6.3.2, S. 185 — "For optimization, implementations of the protocol
shall not include a parameter in the Data submessage if it contains
information that is redundant with other parameters already present in that
same Data submessage."

**Repo:** `crates/rtps/src/participant_data.rs`,
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/publication_data.rs::tests::roundtrip_le`

**Status:** `done` — die einzelnen Fälle stehen unter Table 9.17.

### §9.6.3.2 Table 9.17 SPDPdiscoveredParticipantData: ParticipantProxy::guidPrefix

**Spec:** §9.6.3.2, S. 185, Table 9.17 — "SPDPdiscoveredParticipantData —
ParticipantProxy::guidPrefix — ParticipantBuiltinTopicData::key".

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`: nur
PID_PARTICIPANT_GUID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`

**Status:** `done`

### §9.6.3.2 Table 9.17 DiscoveredReaderData: ReaderProxy::remoteReaderGuid

**Spec:** §9.6.3.2, S. 185, Table 9.17 — "DiscoveredReaderData —
ReaderProxy::remoteReaderGuid — SubscriptionBuiltinTopicData::key".

**Repo:** `crates/rtps/src/subscription_data.rs` (`to_pl_cdr_le`: nur
PID_ENDPOINT_GUID)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::roundtrip_le`

**Status:** `done`

### §9.6.3.2 Table 9.17 DiscoveredWriterData: WriterProxy::remoteWriterGuid

**Spec:** §9.6.3.2, S. 185, Table 9.17 — "DiscoveredWriterData —
WriterProxy::remoteWriterGuid — PublicationBuiltinTopicData::key".

**Repo:** `crates/rtps/src/publication_data.rs` (`to_pl_cdr_le`: nur
PID_ENDPOINT_GUID)

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_endpoint_guid`

**Status:** `done`

### §9.6.3.2 Schlüssel-DATA der Builtin-Topics

**Spec:** §9.6.3.2, S. 185 — "In the case of a DATA submessage containing the
SPDPdiscoveredParticipantData with KeyFlag=1, the only parameterId present
within the ParameterList shall be the PID_PARTICIPANT_GUID. In the case of a
DATA submessage containing one of SEDPdiscoveredPublicationData,
SEDPdiscoveredSubscriptionData, or SEDPdiscoveredTopicData with KeyFlag=1, the
only parameterId present within the ParameterList shall be the
PID_ENDPOINT_GUID."

**Repo:** `crates/rtps/src/parameter_list.rs` (`spdp_participant_key_pl_cdr_le`,
`sedp_endpoint_key_pl_cdr_le`); `crates/discovery/src/spdp.rs`
(`serialize_departure`)

**Tests:** `crates/discovery/src/spdp.rs::tests::a_departure_reads_back_as_departed_with_key_and_status`,
`crates/rtps/src/parameter_list.rs::tests::sedp_endpoint_key_matches_the_measured_cyclone_octets`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PAD

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_PAD — 0x0000 — N/A"; Table 9.19, S. 187 —
"PID_PAD — - — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_SENTINEL

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_SENTINEL — 0x0001 — N/A".

**Repo:** `crates/qos/src/pid.rs` (`Pid::SENTINEL`); `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_USER_DATA

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_USER_DATA — 0x002c — UserDataQosPolicy";
Table 9.19, S. 187 — ParticipantBuiltinTopicData, PublicationBuiltinTopicData,
SubscriptionBuiltinTopicData `user_data`, Vorgabe "See DDS Specification".

**Repo:** `crates/qos/src/pid.rs` (`Pid::USER_DATA`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (`user_data`)

**Tests:** `crates/rtps/tests/user_data_propagation.rs::participant_user_data_roundtrip`,
`crates/rtps/tests/user_data_propagation.rs::subscription_user_data_roundtrip`,
`crates/rtps/tests/user_data_propagation.rs::publication_empty_user_data_omits_pid`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TOPIC_NAME

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_TOPIC_NAME — 0x0005 — string<256>";
Table 9.19, S. 187 — TopicBuiltinTopicData::name, Publication/Subscription
`topic_name`, Vorgabe N/A.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TOPIC_NAME`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::roundtrip_utf8_topic_name`,
`crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_topic_name`,
`crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TYPE_NAME

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_TYPE_NAME — 0x0007 — string<256>"; Table
9.19, S. 187 — TopicBuiltinTopicData, Publication/Subscription `type_name`, Vorgabe
N/A.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TYPE_NAME`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_type_name`,
`crates/rtps/src/publication_data.rs::tests::roundtrip_le`,
`crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`

**Status:** `done`

### §9.6.3.2 string<256> für Namen und domainTag

**Spec:** §9.6.3.2, S. 186, Table 9.18 — PID_TOPIC_NAME, PID_TYPE_NAME,
PID_DOMAIN_TAG: "string<256>".

**Repo:** `crates/rtps/src/publication_data.rs` (`STRING_256_MAX`, `encode_bounded_cdr_string_le`,
`decode_bounded_cdr_string`); `crates/dcps/src/participant.rs`; `crates/dcps/src/runtime.rs` (`start`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`,
`crates/rtps/src/participant_data.rs::tests::a_domain_tag_longer_than_string_256_is_refused`,
`crates/dcps/src/participant.rs::tests::create_topic_rejects_a_name_longer_than_string_256`

**Status:** `done` — Topic-, Typ- und domainTag-Namen sind auf 256 Oktette begrenzt: Senden und Lesen weisen mehr ab (die Ankündigung eines Peers mit längerem Namen wird abgewiesen und gezählt), `create_topic` und `DcpsRuntime::start` lehnen sie ab.

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_DATA

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_GROUP_DATA — 0x002d —
GroupDataQosPolicy"; Table 9.19, S. 187 — Publication/Subscription `group_data`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::GROUP_DATA`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::user_topic_and_group_data_survive_the_sedp_roundtrip`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::topic_data_and_group_data_survive_the_builtin_topic_untouched`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TOPIC_DATA

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_TOPIC_DATA — 0x002e —
TopicDataQosPolicy"; Table 9.19, S. 187 — TopicBuiltinTopicData,
Publication/Subscription `topic_data`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TOPIC_DATA`);
`crates/rtps/src/topic_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::user_topic_and_group_data_survive_the_sedp_roundtrip`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::topic_data_and_group_data_survive_the_builtin_topic_untouched`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DURABILITY

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DURABILITY — 0x001d —
DurabilityQosPolicy"; Table 9.19, S. 187 — Topic, Publication, Subscription
`durability`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DURABILITY`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::durability_try_from_u32_rejects_unknown`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::defaults_roundtrip_stays_default`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DURABILITY_SERVICE

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DURABILITY_SERVICE — 0x001e —
DurabilityServiceQosPolicy"; Table 9.19, S. 187 — Topic, Publication
`durability_service`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DURABILITY_SERVICE`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/qos/src/policies/durability_service.rs::tests::default_matches_spec`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEADLINE

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DEADLINE — 0x0023 — DeadlineQosPolicy";
Table 9.19, S. 188 — Topic, Publication, Subscription `deadline`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DEADLINE`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LATENCY_BUDGET

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_LATENCY_BUDGET — 0x0027 —
LatencyBudgetQosPolicy"; Table 9.19, S. 188 — Topic, Publication, Subscription
`latency_budget`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LATENCY_BUDGET`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LIVELINESS

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_LIVELINESS — 0x001b —
LivelinessQosPolicy"; Table 9.19, S. 188 — Topic, Publication, Subscription
`liveliness`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LIVELINESS`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_RELIABILITY

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_RELIABILITY — 0x001a —
ReliabilityQosPolicy" mit Fußnote 3: "The encoding of
DDS::ReliabilityQoSPolicy::kind is defined by RTPS::ReliabilityKind_t (9.3.2)";
Table 9.19, S. 188 — Topic, Publication, Subscription `reliability`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::RELIABILITY`);
`crates/rtps/src/publication_data.rs` (Art als 1/2)

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::reliability_try_from_u32_rejects_unknown`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LIFESPAN

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_LIFESPAN — 0x002b — LifespanQosPolicy";
Table 9.19, S. 188 — Topic, Publication `lifespan`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LIFESPAN`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DESTINATION_ORDER

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DESTINATION_ORDER — 0x0025 —
DestinationOrderQosPolicy"; Table 9.19, S. 188 — Topic, Publication, Subscription
`destination_order`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DESTINATION_ORDER`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/qos/src/policies/destination_order.rs::tests::roundtrip`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_HISTORY

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_HISTORY — 0x0040 — HistoryQosPolicy";
Table 9.19, S. 188 — TopicBuiltinTopicData::history.

**Repo:** `crates/qos/src/pid.rs` (`Pid::HISTORY`); `crates/rtps/src/topic_data.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_RESOURCE_LIMITS

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_RESOURCE_LIMITS — 0x0041 —
ResourceLimitsQosPolicy"; Table 9.19, S. 188 — TopicBuiltinTopicData::resource_limits.

**Repo:** `crates/qos/src/pid.rs` (`Pid::RESOURCE_LIMITS`);
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_OWNERSHIP

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_OWNERSHIP — 0x001f — OwnershipQosPolicy";
Table 9.19, S. 188 — Topic, Publication, Subscription `ownership`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::OWNERSHIP`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_OWNERSHIP_STRENGTH

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_OWNERSHIP_STRENGTH — 0x0006 —
OwnershipStrengthQosPolicy"; Table 9.19, S. 188 —
PublicationBuiltinTopicData::ownership_strength.

**Repo:** `crates/qos/src/pid.rs` (`Pid::OWNERSHIP_STRENGTH`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PRESENTATION

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_PRESENTATION — 0x0021 —
PresentationQosPolicy"; Table 9.19, S. 188 — Publication, Subscription
`presentation`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::PRESENTATION`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTITION

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_PARTITION — 0x0029 —
PartitionQosPolicy"; Table 9.19, S. 188 — Publication, Subscription `partition`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::PARTITION`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TIME_BASED_FILTER

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_TIME_BASED_FILTER — 0x0004 —
TimeBasedFilterQosPolicy"; Table 9.19, S. 188 —
SubscriptionBuiltinTopicData::time_based_filter.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TIME_BASED_FILTER`);
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18 PID_TRANSPORT_PRIORITY

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_TRANSPORT_PRIORITY — 0x0049 —
TransportPriorityQoSPolicy". Table 9.19 führt keine Zeile dafür.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TRANSPORT_PRIORITY`);
`crates/rtps/src/topic_data.rs`; `crates/rtps/src/publication_data.rs`
(Inline-QoS)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DOMAIN_ID

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID__DOMAIN_ID — 0x000f — DomainId_t"; Table
9.19, S. 188 — ParticipantProxy::domainId, Vorgabe "The domainId of the local
participant receiving the SPDPdiscoveredParticipantData".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DOMAIN_ID`);
`crates/dcps/src/runtime.rs` (`spdp_announced`: fehlend = eigene Domain)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DOMAIN_TAG

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DOMAIN_TAG — 0x4014 — string<256>"; Table
9.19, S. 188 — ParticipantProxy::domainTag, Vorgabe "“” (empty, zero-length string)".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DOMAIN_TAG`);
`crates/rtps/src/participant_data.rs` (fehlend = leer)

**Tests:** `crates/rtps/src/participant_data.rs::tests::domain_tag_goes_out_when_set_and_reads_back`,
`crates/rtps/src/participant_data.rs::tests::domain_tag_is_a_standard_must_understand_pid`,
`crates/rtps/src/participant_data.rs::tests::a_domain_tag_longer_than_string_256_is_refused`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PROTOCOL_VERSION

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_PROTOCOL_VERSION — 0x0015 —
ProtocolVersion_t"; Table 9.19, S. 188 — ParticipantProxy::protocolVersion.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PROTOCOL_VERSION`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_VENDORID

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_VENDORID — 0x0016 — VendorId_t"; Table
9.19, S. 188 — ParticipantProxy::vendorId.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::VENDOR_ID`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendor_id_survives_read_and_write_back`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_UNICAST_LOCATOR

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_UNICAST_LOCATOR — 0x002f — Locator_t";
Table 9.19, S. 188 — ReaderProxy/WriterProxy::unicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::UNICAST_LOCATOR`);
`crates/rtps/src/publication_data.rs` (`collect_locator_params`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_MULTICAST_LOCATOR — 0x0030 —
Locator_t"; Table 9.19, S. 188 — ReaderProxy/WriterProxy::multicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::MULTICAST_LOCATOR`);
`crates/rtps/src/publication_data.rs` (`collect_locator_params`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Endpunkt-Locatoren in PL_CDR_BE

**Spec:** §9.6.3.2, Table 9.19, S. 188 — PID_UNICAST_LOCATOR und
PID_MULTICAST_LOCATOR für ReaderProxy und WriterProxy; §9.6.3.2, S. 185:
"Within each parameter, the
parameter value is represented using CDR."

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEFAULT_UNICAST_LOCATOR

**Spec:** §9.6.3.2, S. 186, Table 9.18 — "PID_DEFAULT_UNICAST_LOCATOR — 0x0031 —
Locator_t"; Table 9.19, S. 188 — ParticipantProxy::defaultUnicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DEFAULT_UNICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEFAULT_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_DEFAULT_MULTICAST_LOCATOR — 0x0048 —
Locator_t"; Table 9.19, S. 189 — ParticipantProxy::defaultMulticastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DEFAULT_MULTICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_METATRAFFIC_UNICAST_LOCATOR

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_METATRAFFIC_UNICAST_LOCATOR — 0x0032 —
Locator_t"; Table 9.19, S. 189 — ParticipantProxy::metatrafficUnicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::METATRAFFIC_UNICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_METATRAFFIC_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_METATRAFFIC_MULTICAST_LOCATOR — 0x0033 —
Locator_t"; Table 9.19, S. 189 — ParticipantProxy::metatrafficMulticastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::METATRAFFIC_MULTICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_EXPECTS_INLINE_QOS

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_EXPECTS_INLINE_QOS — 0x0043 — boolean";
Table 9.19, S. 189 — ParticipantProxy::expectsInlineQos, Vorgabe FALSE.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::EXPECTS_INLINE_QOS`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`,
`crates/rtps/src/subscription_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT —
0x0034 — Count_t"; Table 9.19, S. 189 — ParticipantProxy::manualLivelinessCount.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PARTICIPANT_MANUAL_LIVELINESS_COUNT`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done` — gelesen, geschrieben und in `is_standard_pid`; OpenDDS sendet 0 und bekommt 0 zurück.

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_LEASE_DURATION

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_PARTICIPANT_LEASE_DURATION — 0x0002 —
Duration_t"; Table 9.19, S. 189 — SPDPdiscoveredParticipantData::leaseDuration,
Vorgabe {100, 0}.

**Repo:** `crates/rtps/src/parameter_list.rs`
(`pid::PARTICIPANT_LEASE_DURATION`); `crates/rtps/src/participant_data.rs`
(fehlend = 100 s)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §9.6.3.2 PID_PARTICIPANT_LEASE_DURATION in PL_CDR_BE

**Spec:** §9.6.3.2, S. 185 — "Within each parameter, the parameter value is
represented using CDR."; Table 9.18: Duration_t.

**Repo:** —

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_CONTENT_FILTER_PROPERTY

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_CONTENT_FILTER_PROPERTY — 0x0035 —
ContentFilterProperty_t"; Table 9.19, S. 189 — DiscoveredReaderData::contentFilter.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::CONTENT_FILTER_PROPERTY`);
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::subscription_with_content_filter_roundtrip_le`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_GUID

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_PARTICIPANT_GUID — 0x0050 — GUID_t";
Table 9.19, S. 189 — ParticipantBuiltinTopicData::key, Publication/Subscription
`participant_key`.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PARTICIPANT_GUID`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`,
`crates/rtps/src/publication_data.rs::tests::participant_key_fallback_when_pid_missing`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_GUID

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_GROUP_GUID — 0x0052 — GUID_t"; Table 9.19
— WriterProxy/ReaderProxy::remoteGroupGuid, Vorgabe GUID_UNKNOWN.

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_ENTITY_ID

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_GROUP_ENTITY_ID — 0x0053 — EntityId_t";
Table 9.19, S. 189 — WriterProxy/ReaderProxy::remoteGroupGuid.entityId, Vorgabe
ENTITYID_UNKNOWN.

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done` — gelesen; ZeroDDS sendet die Gruppe als volle GUID in
`PID_GROUP_GUID`.

### §9.6.3.2 Table 9.18/9.19 PID_BUILTIN_ENDPOINT_SET

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_BUILTIN_ENDPOINT_SET — 0x0058 —
BuiltinEndpointSet_t"; Table 9.19, S. 189 — ParticipantProxy::availableBuiltinEndpoints.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::BUILTIN_ENDPOINT_SET`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_roundtrip_through_pl_cdr`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_BUILTIN_ENDPOINT_QOS

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_BUILTIN_ENDPOINT_QOS — 0x0077 —
BuiltinEndpointQos_t"; Table 9.19, S. 189 — ParticipantProxy::builtinEndpointQos.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::BUILTIN_ENDPOINT_QOS`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done` — gelesen, geschrieben und in `is_standard_pid`.

### §9.6.3.2 Table 9.18 PID_PROPERTY_LIST

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_PROPERTY_LIST — 0x0059 —
sequence<Property_t>". Table 9.19 führt keine Zeile dafür.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PROPERTY_LIST`);
`crates/rtps/src/participant_data.rs` (`properties`);
`crates/rtps/src/property_list.rs` (`WirePropertyList::decode`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_empty_properties_omits_pid`,
`crates/rtps/src/property_list.rs::tests::decode_reads_the_rtps_form_without_binary_value`

**Status:** `done` — gesendet in SPDP und, wo ein Endpoint Properties hat, in
SEDP, in der Form von DDS-Security 1.2 §7.5.1.4 (mit `binary_value`). Gelesen
werden beide Formen; eine Liste, die nach ihren Properties endet (diese
Zeile), hat keine binären Einträge.

### §9.6.3.2 Table 9.18/9.19 PID_TYPE_MAX_SIZE_SERIALIZED

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_TYPE_MAX_SIZE_SERIALIZED — 0x0060 —
long"; Table 9.19, S. 189 — "PID_DATA_MAX_SIZE_SERIALIZED —
WriterProxy::dataMaxSizeSerialized".

**Repo:** `crates/rtps/src/publication_data.rs` (`data_max_size_serialized`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`,
`crates/rtps/src/reliable_reader.rs::tests::a_payload_beyond_the_announced_maximum_is_refused_and_counted`

**Status:** `done` — Publications senden und lesen den PID als `long` (`PublicationBuiltinTopicData::data_max_size_serialized`); in `vendor_discovery_frames.rs` ist 0x0060 nicht mehr unter den verworfenen PIDs. Gemessen in `tools/wire-capture/rtps-sessions/rti`: RTI 7.7.0 kündigt den Wert 8 an, und das DATA seines Writers trägt ein SerializedPayload von 8 Oktetten, den Encapsulation-Header eingerechnet; der Wert zählt also wie die Spec mit Header. Fast DDS 2.6.10 kündigt 12, 32 und 52 an (kein Nutzer-DATA im Mitschnitt); Cyclone und OpenDDS kündigen den PID nicht an. Ein Reader weist ein Payload ab, das den genannten Wert überschreitet. Nicht gemessen: ein Lauf gegen die Hersteller mit diesem Abweis.

### §9.6.3.2 Table 9.18 PID_ENTITY_NAME

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_ENTITY_NAME — 0x0062 — EntityName_t".
Table 9.19 führt keine Zeile dafür.

**Repo:** `crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (`entity_name`);
`crates/dcps/src/runtime.rs` (`built_entity_name`, `built_participant_name`, `RuntimeConfig::participant_name`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::the_participant_name_goes_out_and_reads_back`,
`crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`

**Status:** `done` — Participant, Writer und Reader senden und lesen `PID_ENTITY_NAME` als `EntityName_t` (CDR-String). Die DCPS-API hat keine EntityName-QoS; die Namen sind gebaut: Participant `zerodds:` und das GUID-Präfix in Hex (überschreibbar mit `discovery.participant_name`), Writer und Reader `DataWriter:<Topic>:<EntityId hex>` bzw. `DataReader:…`. `UserWriterConfig::entity_name` und `UserReaderConfig::entity_name` nehmen einen gesetzten Namen. Ein nicht lesbarer Name (kein gültiger CDR-String) verwirft die Ankündigung wie ein nicht lesbarer `domainTag`.

### §9.6.3.2 Table 9.18/9.19 PID_ENDPOINT_GUID

**Spec:** §9.6.3.2, S. 187, Table 9.18 — "PID_ENDPOINT_GUID — 0x005a — GUID_t"; Table
9.19, S. 189 — Topic-, Subscription-, PublicationBuiltinTopicData::key.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::ENDPOINT_GUID`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_endpoint_guid`,
`crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Vorgaben "See DDS Specification" für fehlende QoS-PIDs

**Spec:** §9.6.3.2, Table 9.19, S. 187–188 — die QoS-Zeilen nennen als Vorgabe "See DDS
Specification."; §9.6.3.2: "Unrecognized parameterIDs shall be treated as
specified in Table 9.6."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`, `crates/rtps/src/topic_data.rs`
(`unwrap_or_default` je Policy)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::defaults_roundtrip_stays_default`,
`crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`

**Status:** `done`

### §9.6.4 Inline-QoS-ParameterIds

**Spec:** §9.6.4, S. 189 — "Sub clause 8.7.2.1 defines the complete set of
parameters that can appear within the inlineQos SubmessageElement. The
corresponding set of parameterIds is listed in Table 9.20. Unrecognized
parameterIDs shall be treated as specified in Table 9.6."

**Repo:** `crates/rtps/src/inline_qos.rs`; `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/inline_qos.rs::tests::roundtrip_in_data_submessage_inline_qos`,
`crates/rtps/src/datagram.rs::tests::decode_accepts_data_with_known_must_understand_pid_in_inline_qos`

**Status:** `done` — die Behandlung unbekannter PIDs ist unter §9.4.2.11
Table 9.6 geführt.

### §9.6.4 Table 9.20 PID_PAD

**Spec:** §9.6.4, S. 189, Table 9.20 — "PID_PAD — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.6.4 Table 9.20 PID_SENTINEL

**Spec:** §9.6.4, S. 189, Table 9.20 — "PID_SENTINEL — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SENTINEL`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`

**Status:** `done`

### §9.6.4 Table 9.20 PID_TOPIC_NAME

**Spec:** §9.6.4, S. 189, Table 9.20 — "PID_TOPIC_NAME — See Table 9.18 —
string<256>".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DURABILITY

**Spec:** §9.6.4, S. 189, Table 9.20 — "PID_DURABILITY — See Table 9.18 —
DurabilityQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_PRESENTATION

**Spec:** §9.6.4, S. 189, Table 9.20 — "PID_PRESENTATION — PresentationQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DEADLINE

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_DEADLINE — DeadlineQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LATENCY_BUDGET

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_LATENCY_BUDGET —
LatencyBudgetQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_OWNERSHIP

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_OWNERSHIP — OwnershipQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_OWNERSHIP_STRENGTH

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_OWNERSHIP_STRENGTH —
OwnershipStrengthQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LIVELINESS

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_LIVELINESS — LivelinessQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_PARTITION

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_PARTITION — PartitionQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_RELIABILITY

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_RELIABILITY — ReliabilityQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_TRANSPORT_PRIORITY

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_TRANSPORT_PRIORITY —
TransportPriorityQoSPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LIFESPAN

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_LIFESPAN — LifespanQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DESTINATION_ORDER

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_DESTINATION_ORDER —
DestinationOrderQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_CONTENT_FILTER_INFO (0x0055)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_CONTENT_FILTER_INFO — 0x0055 —
ContentFilterInfo_t".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gehört zur Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt (§8.7.3.4); ein empfangener Parameter wird als
unbekannter PID ohne Must-Understand-Bit übergangen.

### §9.6.4 Table 9.20 PID_COHERENT_SET (0x0056)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_COHERENT_SET — 0x0056 —
SequenceNumber_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::COHERENT_SET`);
`crates/rtps/src/inline_qos.rs` (`coherent_set_inline_qos`,
`find_coherent_set`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_roundtrip_both_endiannesses`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DIRECTED_WRITE (0x0057)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_DIRECTED_WRITE — 0x0057 —
GUID_t" mit Fußnote 4: "RTPS protocol versions prior to 2.4 defined this as a
sequence<GUID_t>. … when interacting with protocol versions earlier than 2.4
this parameter should be ignored unless the receiver knows the format used by
the vendor".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DIRECTED_WRITE`);
`crates/rtps/src/inline_qos.rs` (`directed_write_param`, `find_directed_write`);
`crates/rtps/src/datagram.rs` (`ignore_directed_write_before_2_4`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`,
`crates/rtps/src/inline_qos.rs::tests::directed_write_param_carries_target_guid`,
`crates/rtps/src/inline_qos.rs::tests::find_directed_write_rejects_truncated_value`,
`crates/rtps/src/datagram.rs::tests::pid_directed_write_of_a_peer_before_2_4_is_ignored`

**Status:** `done` — 0x0057 mit einer `GUID_t`; Anmerkung 4: bei einer Protokollversion vor 2.4 (dort `sequence<GUID_t>`) wird der Parameter ignoriert. Senden und Auswerten stehen unter §8.7.7 auf `done`.

### §9.6.4 Table 9.20 PID_ORIGINAL_WRITER_INFO (0x0061)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_ORIGINAL_WRITER_INFO — 0x0061 —
OriginalWriterInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::ORIGINAL_WRITER_INFO`);
`crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`

**Status:** `done` — 0x0061 mit `OriginalWriterInfo_t` (24 Oktette, §9.6.4.7). Senden und Auswerten stehen unter §8.7.9 auf `done`.

### §9.6.4 Table 9.20 PID_GROUP_COHERENT_SET (0x0063)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_GROUP_COHERENT_SET — 0x0063 —
SequenceNumber_t".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_GROUP_SEQ_NUM (0x0064)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_GROUP_SEQ_NUM — 0x0064 —
SequenceNumber_t".

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_WRITER_GROUP_INFO (0x0065)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_WRITER_GROUP_INFO — 0x0065 —
WriterGroupInfo_t".

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_SECURE_WRITER_GROUP_INFO (0x0066)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_SECURE_WRITER_GROUP_INFO — 0x0066
— WriterGroupInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SECURE_WRITER_GROUP_INFO`);
`crates/rtps/src/inline_qos.rs` (`secure_writer_group_info_param`, `find_writer_group_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`,
`crates/rtps/src/reliable_writer.rs::tests::the_secure_writer_group_info_stands_where_the_plain_one_is_required`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done` — 0x0066 mit `WriterGroupInfo_t` (4 Oktette); ein Writer unter DDS-Security sendet ihn, ein Reader liest ihn an der Stelle von `PID_WRITER_GROUP_INFO`.

### §9.6.4 Table 9.20 PID_KEY_HASH (0x0070)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_KEY_HASH — 0x0070 — KeyHash_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::KEY_HASH`);
`crates/rtps/src/inline_qos.rs` (`find_key_hash`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`

**Status:** `done`

### §9.6.4 Table 9.20 PID_STATUS_INFO (0x0071)

**Spec:** §9.6.4, S. 190, Table 9.20 — "PID_STATUS_INFO — 0x0071 —
StatusInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::STATUS_INFO`);
`crates/rtps/src/inline_qos.rs` (`status_info_param`, `find_status_info`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`

**Status:** `done`

### §9.6.4.1 PID_CONTENT_FILTER_INFO: Leitungsbild, Bitmap, Signatur

**Spec:** §9.6.4.1, S. 191 — Leitungsbild numBitmaps, bitmaps, numSignatures,
FilterSignature_t; "The content filter info in-line QoS is invalid unless
numBitmaps == ([numSignatures/32] + (numSignatures%32 ? 1 : 0))"; "A filter’s
signature is calculated as the 128-bit MD5 checksum of all strings in the
filter's ContentFilterProperty_t."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gehört zur Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt (§8.7.3.4).

### §9.6.4.1 Table 9.21 filterResult

**Spec:** §9.6.4.1, S. 191, Table 9.21 — "0 — Sample was filtered by the
corresponding filter and did not pass. 1 — Sample was filtered by the
corresponding filter and passed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — gehört zur Filterung auf der Writer-Seite,
die ZeroDDS nicht ausführt (§8.7.3.4).

### §9.6.4.2 PID_COHERENT_SET: Kodierung und Wert

**Spec:** §9.6.4.2, S. 191 — "The coherent set in-line QoS parameter uses the
CDR encoding for SequenceNumber_t. … all Data and DataFrag Submessages that
belong to the same coherent set must contain the coherent set in-line QoS
parameter with value equal to the sequence number of the first sample in the
set."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_roundtrip_both_endiannesses`,
`crates/rtps/src/reliable_writer.rs::tests::coherent_set_member_data_carries_pid_0x0056_with_set_first_sn_octets`

**Status:** `done`

### §9.6.4.2 Table 9.22 Beispiele für das Ende eines Coherent Sets

**Spec:** §9.6.4.2, S. 192, Table 9.22 — "Example Data Submessages to denote the
end of a coherent set".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`receive_data`: `coherent_set_end`);
`crates/rtps/src/reliable_writer.rs` (`emit_coherent_set_end`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::table_9_22_each_form_that_ends_a_coherent_set_ends_it`,
`crates/rtps/src/reliable_reader.rs::tests::end_of_coherent_set_data_is_delivered_as_a_terminator`,
`crates/rtps/src/reliable_writer.rs::tests::end_coherent_set_emits_ecs_data_with_d_and_k_clear`

**Status:** `done` — der Reader erkennt alle drei Formen der Tabelle: Beispiel 1 (DATA mit Daten und `PID_COHERENT_SET` 7, ein neues Set), Beispiel 2 (ohne Daten, `PID_COHERENT_SET` SEQUENCENUMBER_UNKNOWN) und Beispiel 3 (ohne Daten, ohne Inline-QoS); das Set 3 bis 6 ist danach beendet und vollständig. Der Writer sendet Beispiel 3.

### §9.6.4.3 PID_GROUP_COHERENT_SET

**Spec:** §9.6.4.3, S. 192 — "The group coherent set in-line QoS parameter uses
the CDR encoding for SequenceNumber_t. … all Data submessages and the first
DataFrag submessage belonging to a sample must contain the group coherent set
in-line QoS parameter".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.3 Table 9.23 Beispiel eines GROUP-Coherent-Sets

**Spec:** §9.6.4.3, S. 192, Table 9.23 — "Example Data Submessages in a GROUP
coherent set".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`push_set_params`);
`crates/rtps/src/inline_qos.rs` (`find_change_group_info`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::group`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_reader.rs::tests::table_9_23_the_data_of_a_group_coherent_set_reach_the_consumer_with_their_group_numbers`,
`crates/rtps/src/writer_group.rs::tests::the_numbers_of_table_9_23`

**Status:** `done` — zwei Writer eines GROUP-Publishers erzeugen die Werte der Tabelle (writerSN 4, 8, 5, 6, 9; GSN 11 bis 14; `PID_COHERENT_SET` 4 und 8; `PID_GROUP_COHERENT_SET` 11); der Reader gibt zu jedem Sample seine Gruppennummern weiter.

### §9.6.4.4 PID_GROUP_SEQ_NUM

**Spec:** §9.6.4.4, S. 193 — "all Data submessages and the first DataFrag
submessage sent by DataWriters belonging to a Publisher with Presentation
access scope GROUP must contain the group sequence number in-line QoS parameter
with value equal to the group sequence number."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.5 PID_WRITER_GROUP_INFO

**Spec:** §9.6.4.5, S. 193 — "for DataWriters belonging to a Publisher with
Presentation access scope GROUP, the Data submessages and the first DataFrag
submessage of each sample shall contain the publisher writer info in-line QoS
parameter. The End Coherent Set Data submessage … shall also contain the
publisher writer info in-line QoS parameter."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.6 PID_SECURE_WRITER_GROUP_INFO

**Spec:** §9.6.4.6, S. 193 — "The secure publisher writer info in-line QoS is
reserved for DDS Security. In the cases when it is used it shall be added
anywhere that the PID_WRITER_GROUP_INFO in-line QoS is required."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SECURE_WRITER_GROUP_INFO`);
`crates/rtps/src/reliable_writer.rs` (`set_secure_writer_group_info`, `push_set_params`);
`crates/rtps/src/inline_qos.rs` (`secure_writer_group_info_param`, `find_writer_group_info`);
`crates/dcps/src/runtime.rs` (Writer-Anlage unter Security)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_secure_writer_group_info_stands_where_the_plain_one_is_required`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — unter DDS-Security (`RuntimeConfig::security`) senden Writer eines GROUP-Publishers `PID_SECURE_WRITER_GROUP_INFO` überall dort, wo sonst `PID_WRITER_GROUP_INFO` steht; ein Reader nimmt beide. Der Test mit Security-Laufzeit steht nicht in diesem Lauf (nur die Writer-Einstellung ist getestet).

### §9.6.4.7 PID_ORIGINAL_WRITER_INFO: Leitungsbild

**Spec:** §9.6.4.7, S. 193 — Leitungsbild `GUID_t originalWriterGUID`,
`SequenceNumber originalWriterSN`. "The original writer info parameter may
appear in the Data or in the DataFrag submessages."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`,
`find_original_writer_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_be`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — das Leitungsbild stimmt (GUID_t, SequenceNumber_t); der Parameter wird in DATA und DATA_FRAG gesendet und ausgewertet (Items unter §8.7.9).

### §9.6.4.8 KeyHash: 16 Oktette

**Spec:** §9.6.4.8, S. 193 — "The KeyHash_t is defined as a 16-Byte octet array
(see 9.3.2) therefore the key hash inline parameter just copies those 16
Bytes."

**Repo:** `crates/dcps/src/key_hash.rs` (`KEY_HASH_LEN`);
`crates/rtps/src/inline_qos.rs` (`find_key_hash`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_holder_of_exactly_sixteen_is_the_hash`,
`crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`

**Status:** `done`

### §9.6.4.8 KeyHash Schritt 1: FooKeyHolder

**Spec:** §9.6.4.8, S. 193–194 — "FooKeyHolder shall have extensibility kind
'FINAL' … If there are any key members, then remove the non-key members …
Reorder the members in ascending order of their memberId values."

**Repo:** `crates/dcps/src/key_hash.rs` (`keyhash_cdr2_be`: Sortierung nach
`member_id`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::the_member_order_does_not_reach_the_hash`

**Status:** `done`

### §9.6.4.8 KeyHash Schritte 2 und 3: Werte und Rekursion

**Spec:** §9.6.4.8, S. 194 — "Step 2. … setting the members present in
FooKeyHolderObject to the same values as the corresponding members in
FooObject. Step 3. Apply steps 1 and 2 recursively to the members of
FooKeyHolder if they are themselves Aggregated types".

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`);
`crates/dcps/src/runtime.rs` (`reader_key_hash_context`)

**Tests:** `crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §9.6.4.8 KeyHash Schritt 4: PLAIN_CDR2 Big Endian

**Spec:** §9.6.4.8, S. 194 — "Compute the PLAIN_CDR2 Big Endian Serialization
… The serialization shall be performed on a buffer that is initially aligned
to the maximum alignment in PLAIN_CDR2 (i.e., 4). Furthermore, any padding
bytes added due to alignment rules shall be set to zero."

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_u32_goes_out_big_endian`,
`crates/dcps/src/key_hash.rs::tests::a_u64_is_aligned_to_four_and_not_to_eight`,
`crates/dcps/src/key_hash.rs::tests::a_string_carries_its_terminator_in_its_length`

**Status:** `done`

### §9.6.4.8 KeyHash Schritt 5: Polsterung oder MD5 nach der Maximalgröße

**Spec:** §9.6.4.8, S. 194 — "Step 5.1 If the FooKeyHolder has a maximum
serialized size that is less than or equal to 16 bytes, then the KeyHash of
FooObject shall be set to the result of Step 4, extended to 16 bytes. … Step
5.2 If the FooKeyHolder has a maximum serialized size that is greater than 16
bytes, then the KeyHash_t of FooObject shall be set to the MD5 Hash".

**Repo:** `crates/dcps/src/key_hash.rs` (`compute_key_hash`:
`key_holder_max_size`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_short_instance_of_a_bounded_type_still_pads`,
`crates/dcps/src/key_hash.rs::tests::a_holder_over_sixteen_is_hashed`

**Status:** `done`

### §9.6.4.8 KeyHash Beispiele 1 bis 3

**Spec:** §9.6.4.8, S. 194–196 — "Example 1 …", "Example 2 …", "Example 3 …"
mit den erwarteten Oktetten.

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`, `compute_key_hash`);
`crates/dcps/src/dds_type.rs` (`DdsType::compute_key_hash`)

**Tests:** `crates/dcps/src/dds_type.rs::tests::key_hash_example_1_of_the_spec_is_the_holder_padded_with_zeros`,
`crates/dcps/src/dds_type.rs::tests::key_hash_example_2_of_the_spec_is_the_md5_of_the_holder`,
`crates/dcps/src/dds_type.rs::tests::key_hash_example_3_of_the_spec_is_the_md5_of_the_reordered_holder`

**Status:** `done` — die drei Beispiele ergeben die Oktette der Spec: Beispiel 1 den mit Nullen auf 16 Oktette erweiterten Holder, Beispiel 2 und 3 den MD5 des 20- und des 13-Oktett-Holders (`f91a59e3…`, `374b96e2…`).

### §9.6.4.9 StatusInfo_t: 4 Oktette

**Spec:** §9.6.4.9, S. 196 — "The StatusInfo_t is defined as a 4-Byte octet
array (see 9.3.2) therefore the status info inline parameter just copies those
4 Bytes."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info_param`:
Netzreihenfolge; `find_status_info`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** `done`

### §9.6.4.9 StatusInfo in Data oder DataFrag

**Spec:** §9.6.4.9, S. 196 — "The status info parameter may appear in the Data
or in the DataFrag submessages."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`classify_change_kind` für
DATA, `handle_data_frag_opening` für DATA_FRAG)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_key_is_a_dispose_or_a_registration_as_a_data_is`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`

**Status:** `done` — in DATA und in DATA_FRAG ausgewertet (Schlüssel mit D oder U als Dispose und Unregister, F als gefiltertes Sample).

### §9.6.4.9 StatusInfo: X-Flags

**Spec:** §9.6.4.9, S. 197 — "The flags represented with the literal ‘X’ are
unused by this version of the protocol and should be set to zero by the writer
and not interpreted by the reader".

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info`);
`crates/rtps/src/reliable_reader.rs` (`classify_change_kind`: nur D und U)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §9.6.4.9 StatusInfo DisposedFlag

**Spec:** §9.6.4.9, S. 197 — "D=1 indicates that the DDS DataWriter has
disposed the instance of the data-object whose Key appears in the submessage."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info::DISPOSED = 0x1`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §9.6.4.9 StatusInfo UnregisteredFlag

**Spec:** §9.6.4.9, S. 197 — "U=1 indicates that the DDS DataWriter has
unregistered the instance of the data-object whose Key appears in the
submessage."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info::UNREGISTERED = 0x2`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done`

### §9.6.4.9 StatusInfo FilteredFlag

**Spec:** §9.6.4.9, S. 197 — "F=1 indicates that the DDS DataWriter has written
as sample for the instance … but the sample did not pass the content filter
specified by the DDS DataReader." "If both DisposedFlag==0 and
UnregisteredFlag=0 … FilteredFlag==1 indicates it did not pass the filter."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`status_change_kind`, `classify_change_kind`,
`ReliableReader::filtered_sample_count`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — ein DATA oder DATA_FRAG mit F = 1 und D = U = 0 (mit oder ohne Nutzdaten und Key-Flag) ergibt `ChangeKind::AliveFiltered`: die Nummer ist bekannt und NA_FILTERED (der Writer wiederholt sie nicht), das Sample wird nicht ausgeliefert und in `filtered_sample_count` gezählt. Bei gesetztem D oder U gilt die Lebenszyklus-Marke. ZeroDDS sendet F nicht (die Laufzeit filtert nicht auf der Writer-Seite); ob ein anderer Hersteller F sendet, ist nicht gemessen.

### §9.6.4.9 Registrierung nicht übertragen

**Spec:** §9.6.4.9, S. 197 — "Note that the protocol does not require that the
DDS DataWriter propagates the "register" operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Anforderung.

### §9.6.5 Table 9.24 DDS-XTypes

**Spec:** §9.6.5, S. 197, Table 9.24 — "DDS-XTypes 1.1 … 1.3 — 0x0069, 0x0072,
0x0073, 0x0074, 0x0075, 0x0076; Note that 0x0076 was deprecated in DDS-XTypes
1.2".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DATA_REPRESENTATION`,
`TYPE_CONSISTENCY_ENFORCEMENT`, `TYPE_INFORMATION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::issue30_type_identifier_from_standard_type_information`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.6.5 Table 9.24 DDS-Security

**Spec:** §9.6.5, S. 197, Table 9.24 — "DDS-Security 1.1 — Parameter IDs in the
range 0x1000 to 0x1FFF; Parameter IDs in the range 0x5000 to 0x5FFF".

**Repo:** `crates/rtps/src/parameter_list.rs` (Sicherheits-PIDs 0x1001 bis
0x1013)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`,
`crates/rtps/src/participant_data.rs::tests::participant_security_info_pid_roundtrip`

**Status:** `done`

### §9.6.5 Table 9.24 DDS-RPC

**Spec:** §9.6.5, S. 197, Table 9.24 — "DDS-RPC 1.0 — 0x0080, 0x0081, 0x0082,
0x0083".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SERVICE_INSTANCE_NAME`,
`RELATED_ENTITY_GUID`, `TOPIC_ALIASES`, `RELATED_SAMPLE_IDENTITY`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/rtps/src/publication_data.rs::tests::rpc_pid_constants_in_emitted_bytes`

**Status:** `done`

### §9.6.6 Table 9.25 PID_PERSISTENCE (0x0003)

**Spec:** §9.6.6, S. 197–198, Table 9.25 — "PID_PERSISTENCE — 0x0003 — 2.2";
"These parameters should not be used by versions of the protocol equal or
newer than the deprecated version unless they are used with the same meaning
as in versions prior to the deprecated version."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_TYPE_CHECKSUM (0x0008)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_TYPE_CHECKSUM — 0x0008 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_TYPE2_NAME (0x0009)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_TYPE2_NAME — 0x0009 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_TYPE2_CHECKSUM (0x000a)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_TYPE2_CHECKSUM — 0x000a — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_EXPECTS_ACK (0x0010)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_EXPECTS_ACK — 0x0010 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_MANAGER_KEY (0x0012)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_MANAGER_KEY — 0x0012 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_SEND_QUEUE_SIZE (0x0013)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_SEND_QUEUE_SIZE — 0x0013 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang (RTI sendet ihn, `vendor_discovery_frames.rs`).

### §9.6.6 Table 9.25 PID_RELIABILITY_ENABLED (0x0014)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_RELIABILITY_ENABLED — 0x0014 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_VARGAPPS_SEQUENCE_NUMBER_LAST (0x0017)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_VARGAPPS_SEQUENCE_NUMBER_LAST —
0x0017 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

### §9.6.6 Table 9.25 PID_RECV_QUEUE_SIZE (0x0018)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_RECV_QUEUE_SIZE — 0x0018 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang (RTI sendet ihn, `vendor_discovery_frames.rs`).

### §9.6.6 Table 9.25 PID_RELIABILITY_OFFERED (0x0019)

**Spec:** §9.6.6, S. 198, Table 9.25 — "PID_RELIABILITY_OFFERED — 0x0019 —
2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — veralteter PID; ZeroDDS sendet ihn nicht und
übergeht ihn beim Empfang.

## §10 Serialized Payload Representation

### §10.1 Einleitung

**Spec:** §10.1, S. 199 — "The representation of the serialized application
data is not part of the RTPS protocol. The RTPS protocol does not interpret the
content of the SerializedPayload."; "the content and format of the
SerializedPayload is defined in sections 10.3 to 10.5."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Überblick; die Anforderungen stehen in
§10.2 bis §10.5.

### §10.2 SerializedPayloadHeader am Anfang jeder SerializedPayload

**Spec:** §10.2, S. 199 — "All SerializedPayload shall start with the
SerializedPayloadHeader defined below."; "The SerializedPayloadHeader occupies
the first four octets of the SerializedPayload".

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`,
`write_user_sample_pooled`); `crates/rtps/src/participant_data.rs`
(`to_pl_cdr_le`); `crates/rtps/src/participant_message_data.rs` (`to_cdr`);
`crates/security/src/generic_message.rs` (`encode_generic_message`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`,
`crates/security-runtime/src/builtin_topics.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** `done`

### §10.2 SerializedPayloadHeader: Struktur

**Spec:** §10.2, S. 199 — "typedef octet RepresentationIdentifier[2]; typedef
octet RepresentationOptions[2]; struct SerializedPayloadHeader {
RepresentationIdentifier representation_identifier; RepresentationOptions
representation_options; };"

**Repo:** `crates/dcps/src/runtime.rs` (`user_encap_body`,
`encap_representation`, `encap_big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr2_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_rejects_short`

**Status:** `done`

### §10.2 RepresentationOptions im Zusammenhang des RepresentationIdentifier

**Spec:** §10.2, S. 199 — "The RepresentationOptions shall be interpreted in
the context of the RepresentationIdentifier, such that each
RepresentationIdentifier may define the representation_options that it
requires."

**Repo:** `crates/dcps/src/runtime.rs` (`user_encap_body`: die unteren zwei
Bit als Zahl der angehängten Füll-Oktette nach DDS-XTypes 1.3 §7.6.3.1.2)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_refuses_padding_the_body_cannot_hold`

**Status:** `done`

### §10.2 CDR-Strom beginnt nach representation_options

**Spec:** §10.2, S. 199 — "For alignment purposes, the CDR stream is logically
reset at the position that follows the representation_options. Therefore,
there should be no initial padding before the serialized data is added to the
CDR stream."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_sample_pooled`: Kopf und
getrennt kodierter Körper; `user_encap_body`: Körper ab Oktett 4)

**Tests:** `crates/dcps/src/interop.rs::tests::u64_type_xcdr1_and_xcdr2_alignment_differs`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`

**Status:** `done`

### §10.2 Fußnote 5: ältere Versionen

**Spec:** §10.2, S. 199, Fußnote 5 — "Versions of the RTPS specification
previous to version 2.4 did not clearly state where the CDR stream was reset
for alignment purposes. Therefore implementations may need to take into account
the vendor and protocol version when interpreting the Serialized Data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis ohne Pflicht.

### §10.3 Discovery-Builtin-Endpoints verwenden Table 10.1

**Spec:** §10.3, S. 200 — "The SerializedPayload for the data messages
associated with built-in discovery endpoints shall use the
RepresentationIdentifier values and formats defined in Table 10.1 below."

**Repo:** `crates/rtps/src/participant_data.rs`,
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs` (Kodierung mit PL_CDR_LE oder PL_CDR_BE,
andere Kennungen beim Lesen abgewiesen)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_rejects_unknown_encapsulation`,
`crates/rtps/src/publication_data.rs::tests::decode_rejects_unknown_encapsulation`

**Status:** `done`

### §10.3 Table 10.1 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.3, S. 200, Table 10.1 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11). Both the parameter list and its parameters are encapsulated using
OMG CDR Big Endian."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_be`,
`from_pl_cdr_le`); `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (Lesen bei `[0x00, 0x02]`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`,
`crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §10.3 Table 10.1 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.3, S. 200, Table 10.1 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11). Both the parameter list and its parameters are encapsulated using
OMG CDR Little Endian."

**Repo:** `crates/rtps/src/participant_data.rs` (`ENCAPSULATION_PL_CDR_LE`,
`to_pl_cdr_le`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §10.3 representation_options: Sender setzt null

**Spec:** §10.3, S. 200 — "The current version of the protocol does not use
the representation_options: The sender shall set the representation_options to
zero."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`,
`to_pl_cdr_be`: options `0x0000`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`

**Status:** `done`

### §10.3 representation_options: Empfänger ignoriert sie

**Spec:** §10.3, S. 200 — "The receiver shall ignore the value of the
representation_options."

**Repo:** `crates/rtps/src/participant_data.rs`, `publication_data.rs`, `subscription_data.rs` (`from_pl_cdr_le`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::the_receiver_ignores_the_representation_options`,
`crates/rtps/src/participant_data.rs::tests::the_receiver_ignores_the_representation_options_of_a_participant`,
`crates/rtps/src/publication_data.rs::tests::the_receiver_ignores_the_representation_options_of_a_publication`

**Status:** `done` — die Decoder lesen eine Ankündigung mit `representation_options` ≠ 0 wie die mit 0; der Sender schreibt 0.

### §10.4 Andere Builtin-Endpoints verwenden Table 10.2

**Spec:** §10.4, S. 200 — "The SerializedPayload for the data messages
associated with built-in endpoints other than discovery built-in endpoints
shall use one of the RepresentationIdentifier values and formats defined in
Table 10.2 below."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`to_cdr`: CDR_LE oder
CDR_BE); `crates/security/src/generic_message.rs` (CDR_LE)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`,
`crates/security-runtime/src/builtin_topics.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** `done` — ParticipantMessageData und die Nachrichten der
DDS-Security-Endpoints verwenden CDR_LE. Die TypeLookup-Endpoints verwenden
CDR2_LE, die Repräsentation, die ihre eigene Definition nennt (DDS-XTypes 1.4
Beta 1 §7.6.3.3.3), und CDR_LE unter der Einstellung `xcdr1` (Item
„TypeLookup-Endpoints mit CDR2_LE“).

### §10.4 TypeLookup-Endpoints mit CDR2_LE

**Spec:** §10.4, S. 200 — "shall use one of the RepresentationIdentifier values
and formats defined in Table 10.2 below." Table 10.2 nennt CDR_BE, CDR_LE,
PL_CDR_BE und PL_CDR_LE.

**Repo:** `crates/types/src/type_lookup.rs` (`TypeLookupEncoding`,
`TypeLookupEncapsulation`, `to_payload`), `crates/types/src/framing.rs`
(`EncodingVersion`, `encode_optional`, `encode_mutable`),
`crates/dcps/src/runtime.rs` (`send_type_lookup_request_datagram`,
`send_type_lookup_reply`, `dispatch_type_lookup_datagram`,
`RuntimeConfig::type_lookup_encoding`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typelookup/TypeLookupService.java`
(`Encoding`, `Encapsulation`), `.../generated/ZdCodec.java` (`ZdCodec.Xcdr1`)

**Tests:** `crates/types/tests/type_lookup_xcdr1.rs` (acht Tests, darunter
`a_get_types_request_is_cdr_le_with_its_parameters_as_a_parameter_list` und
`a_get_types_reply_carries_its_object_in_the_encoding_version_1`),
`crates/dcps/tests/type_lookup_e2e.rs::{server_handles_inbound_get_types_request_via_dispatch,
a_get_types_request_in_cdr_le_is_answered, the_setting_xcdr1_answers_in_cdr_le,
a_request_is_written_in_the_configured_encoding}`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::a_complete_object_crosses_between_an_xcdr1_and_an_xcdr2_participant`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typelookup/TypeLookupEncodingTest.java`
(fünf Tests), `crates/dcps/tests/java_typelookup_live.rs` (Java fragt die
Rust-Laufzeit, die Rust-Laufzeit fragt Java, über RTPS zwischen zwei
Prozessen, beide mit der Vorgabe XCDR2)

**Status:** `done` — Anfrage und Antwort des TypeLookup-Dienstes gehen
standardmäßig mit `00 07` (CDR2_LE) und XCDR2-Codierung hinaus: die FINAL-Hülle
ohne DHEADER, die appendable Unions mit einem, die MUTABLE-Parameter
`_In`/`_Out` mit DHEADER und einem EMHEADER je Member. DDS-XTypes 1.4 Beta 1
(ptc/2026-02-06) §7.6.3.3.3: "The Data Representation used by these endpoints
shall be Extended CDR Representation (encoding version 2)", die Auflösung von
OMG-Issue DDSXTY14-29. §10.4 überlässt das Format „the definition of each of
those builtin Endpoints“, und diese Definition nennt jetzt XCDR2; CDR2_LE
steht nicht in Table 10.2, und DDSI-RTPS 2.5 ist die aktuelle Fassung. Die
Form von Table 10.2 ist die benannte Einstellung `wire.type_lookup_encoding =
xcdr1` (Env `ZERODDS_WIRE_TYPE_LOOKUP_ENCODING`, zerodds.yaml(5)): `00 01`
(CDR_LE) mit XCDR1, äußerer Typ ohne Kopf, die MUTABLE-Parameter als
PL_CDR1-Listen mit `PID_EXTENDED` (`0x7F01`, Table 34) für die
`@hashid`-Kennungen über `0x3F00` und `PID_LIST_END` (`0x7F02`), die
`@optional`-Member der TypeObjects mit Parameterkopf statt Präsenz-Oktett
(XTypes 1.3 §7.4.1.1.5.2, §7.4.1.2). Sie ist die Lesart von XTypes 1.3, das
für diese Endpoints keine Repräsentation nennt: keine DataRepresentation
(Vorgabe XCDR, §7.6.3.1.1), ein äußerer Typ ohne Annotation ist APPENDABLE
(§7.3.1.2.1.8), Table 60 ergibt CDR_LE. Eine
empfangene Probe wird in der Codierung gelesen, die ihre Kennung nennt
(CDR_BE/CDR_LE → XCDR1, CDR2_BE/CDR2_LE → XCDR2), in beiden Einstellungen;
eine andere Kennung wird mit `typelookup.sample.rejected` verworfen und
gemeldet. Die Probe endet auf einer Vier-Oktett-Grenze, die Optionen zählen
die Füllbytes (XTypes 1.3 §7.6.3.1.2, `5e31120999e`). Die Java-Laufzeit
(`crates/java-omgdds`) hat dieselbe Vorgabe und dieselbe Einstellung
(Property `zerodds.wire.type_lookup_encoding` oder dieselbe Env-Variable,
`48555aad95f`); ihr XCDR1-Codec ist aus derselben unveränderten IDL generiert
und schreibt dieselben Oktette wie die Rust-Laufzeit
(`TypeLookupEncodingTest::aGetTypesRequestIsWrittenInTheEncodingVersion1`,
`theRustReplyReadsAndIsWrittenBackOctetForOctet`). Die erwarteten
XCDR1-Oktette der Tests sind aus den Klauseln hergeleitet, kein
Hersteller-Mitschnitt: kein gemessener Hersteller sendet diese Form.
Gemessen (Linux-Testhost, `internal/release/evidence/rc7-typelookup-encapsulation`,
`ce232e91245`), je Richtung mit einem Typ, den der Peer nicht kennt:

| Hersteller | sendet | liest CDR_LE-Anfrage | liest CDR_LE-Antwort |
|---|---|---|---|
| Cyclone DDS 11.0.1 | CDR2_LE, Optionen = Füllbytes | nein (`deserialization failed`) | nein (`deserialization failed`) |
| Fast DDS 3.6.1 | CDR2_LE (Antworten) | ja, antwortet | nicht gemessen: Fast DDS fragte in keiner Zelle |
| OpenDDS 3.34.0 | CDR2_LE, Optionen `0000` | nein (`failed to deserialize type lookup request`) | nein (`failed to deserialize type lookup reply`) |
| RTI Connext 7.7.0 | CDR2_LE | nein (`no deserialization program … CDR_LE`) | nein (dieselbe Meldung) |

InterCom DDS 4.3.1 sendet ebenfalls CDR2_LE (früherer Mitschnitt,
`an_intercom_get_types_request_padded_without_its_options_is_read`). OpenDDS
setzt das Optionswort auf `0000` auch auf eine Probe, die drei Oktette vor der
Vier-Oktett-Grenze endet; XTypes 1.3 §7.6.3.1.2 verlangt dort die Zahl der
Füllbytes. ZeroDDS liest sie ohne Raten: die Struktur der Hülle sagt, wo sie
endet, und Null-Oktette bis zur nächsten Vier-Oktett-Grenze sind die
Füllung hinter einem Submessage (DDSI-RTPS 2.5 §9.4.1), mit und ohne sie
(`runtime::tests::an_opendds_get_types_request_without_its_padding_count_is_read`
mit der mitgeschnittenen Anfrage, `0e77cf5e96b`). Die
Runner setzen dafür nichts; jedes Paar läuft mit der Vorgabe. Mit CDR2_LE
liefert die Cyclone-Zelle mit angehängtem Member ihre Samples
(`cyc-answers-x2`: 38), mit CDR_LE keine (`cyc-answers-x1`).

### §10.4 Table 10.2 CDR_BE {0x00, 0x00}

**Spec:** §10.4, S. 200, Table 10.2 — "CDR_BE — {0x00, 0x00} — Classic CDR
representation with Big Endian encoding."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`to_cdr(false)`,
`from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_be`

**Status:** `done`

### §10.4 Table 10.2 CDR_LE {0x00, 0x01}

**Spec:** §10.4, S. 200, Table 10.2 — "CDR_LE — {0x00, 0x01} — Classic CDR
representation with Little Endian encoding."

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ENCAPSULATION_CDR_LE`); `crates/security/src/generic_message.rs`
(`ENCAPSULATION_CDR_LE`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`,
`crates/security-runtime/src/builtin_topics.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** `done`

### §10.4 Table 10.2 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.4, S. 200, Table 10.2 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11) with Big Endian encoding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — eine der zulässigen Kennungen; ZeroDDS
verwendet für diese Endpoints CDR_LE und CDR_BE.

### §10.4 Table 10.2 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.4, S. 200, Table 10.2 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11) with Little Endian encoding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — eine der zulässigen Kennungen; ZeroDDS
verwendet für diese Endpoints CDR_LE und CDR_BE.

### §10.4 Definition je Builtin-Endpoint

**Spec:** §10.4, S. 200 — "The definition of each of those builtin Endpoints
should indicate the serialized data format and RepresentationIdentifier used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — richtet sich an die Spezifikationen, die
Builtin-Endpoints definieren.

### §10.5 Anwender-Topics verwenden die Darstellungen aus DDS-XTypes §7.4

**Spec:** §10.5, S. 200 — "The SerializedPayload for the data messages
associated with the user-defined DDS Topics shall use the data representations
defined in DDS-XTYPES clause 7.4 (Data Representation). Accordingly, the
RepresentationIdentifier values and the corresponding formats shall be as
defined in Table 10.3."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — XCDR1, XCDR2 und XML sind umgesetzt. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 CDR_BE {0x00, 0x00}

**Spec:** §10.5, S. 201, Table 10.3 — "CDR_BE — {0x00, 0x00} — Classic CDR
representation with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap` mit
`big_endian`, `encap_big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/subscriber.rs::tests::cache_materialize_honors_xcdr1_encapsulation_le_and_be`

**Status:** `done`

### §10.5 Table 10.3 CDR_LE {0x00, 0x01}

**Spec:** §10.5, S. 201, Table 10.3 — "CDR_LE — {0x00, 0x01} — Classic CDR
representation with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`USER_PAYLOAD_ENCAP`,
`user_payload_encap`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_is_cdr_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`

**Status:** `done`

### §10.5 Table 10.3 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.5, S. 201, Table 10.3 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11) with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`: XCDR1 und
MUTABLE)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done`

### §10.5 Table 10.3 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.5, S. 201, Table 10.3 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11) with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`: XCDR1 und
MUTABLE)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done`

### §10.5 Table 10.3 CDR2_BE {0x00, 0x10}

**Spec:** §10.5, S. 201, Table 10.3 — "CDR2_BE — {0x00, 0x10} — Plain CDR
representation (version2) with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — CDR2_BE: Big-Endian, gerade Kennung. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 CDR2_LE {0x00, 0x11}

**Spec:** §10.5, S. 201, Table 10.3 — "CDR2_LE — {0x00, 0x11} — Plain CDR
representation (version2) with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — CDR2_LE. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 PL_CDR2_BE {0x00, 0x12}

**Spec:** §10.5, S. 201, Table 10.3 — "PL_CDR2_BE — {0x00, 0x12} — Extended
CDR representation (version2) for MUTABLE types with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — PL_CDR2_BE. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 PL_CDR2_LE {0x00, 0x13}

**Spec:** §10.5, S. 201, Table 10.3 — "PL_CDR2_LE — {0x00, 0x13} — Extended
CDR representation (version2) for MUTABLE types with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — PL_CDR2_LE. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 D_CDR_BE {0x00, 0x14}

**Spec:** §10.5, S. 201, Table 10.3 — "D_CDR_BE — {0x00, 0x14} — Extended CDR
representation (version2) for APPENDABLE types with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — D_CDR_BE. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 D_CDR_LE {0x00, 0x15}

**Spec:** §10.5, S. 201, Table 10.3 — "D_CDR_LE — {0x00, 0x15} — Extended CDR
representation (version2) for APPENDABLE types with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — D_CDR_LE. Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3: Kennungen 0x0010 bis 0x0015

**Spec:** §10.5, S. 201, Table 10.3 — "CDR2_BE {0x00, 0x10}", "CDR2_LE
{0x00, 0x11}", "PL_CDR2_BE {0x00, 0x12}", "PL_CDR2_LE {0x00, 0x13}", "D_CDR_BE
{0x00, 0x14}", "D_CDR_LE {0x00, 0x15}".

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — Die sechs Kennungen 0x0010 bis 0x0015 (gerade Big-Endian, ungerade Little-Endian). Gelesen werden beide Wertemengen. Gesendet wird als Vorgabe DDS-XTypes 1.3 Table 60, auf die die Kopfzeile von Table 10.3 verweist und die Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 und RTI 7.7.0 senden (gemessen in `tools/wire-capture/frames/*/*.xcdr2.frames`). Mit `wire.encapsulation_ids: table-39` (Umgebung `ZERODDS_WIRE_ENCAPSULATION_IDS`) sendet ZeroDDS die Kennungen aus Table 39 (§7.4.3.4), deren XCDR2-Werte die aus Table 10.3 sind (XML: {0x01, 0x00}). Erwartet bei den Herstellern: ein Peer, der nur Table 60 liest, kann Daten mit 0x0010 bis 0x0015 nicht lesen; die Einstellung ist für Peers, die Table 10.3 senden und erwarten. Nicht gemessen: ein Lauf gegen die Hersteller mit der Einstellung.

### §10.5 Table 10.3 XML {0x00, 0x04}

**Spec:** §10.5, S. 201, Table 10.3 — "XML — {0x00, 0x04} — See DDS-XTypes [3]
clause 7.4.4."

**Repo:** `crates/dcps/src/runtime.rs` (`XML_ENCAP`, `user_payload_encap`, `xml_wire_body`, `xml_sample_as_cdr`);
`crates/dcps/src/xml_data.rs`; `crates/dcps/src/publisher.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_that_is_no_document_of_the_type_is_refused`,
`crates/dcps/src/runtime.rs::tests::an_xml_dispose_carries_the_instance_key`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`,
`crates/dcps/src/runtime.rs::tests::a_writer_registered_with_xml_first_is_accepted`

**Status:** `done` — ein Writer, dessen erste DataRepresentation XML ist, sendet das Dokument des Samples unter {0x00, 0x04}; ein Reader liest die Probe gegen seinen registrierten Typ und gibt den XCDR2-Rumpf weiter. Nicht gemessen: ein Hersteller, der XML sendet oder liest.

### §10.5 Ältere DDS-Implementierungen ohne DDS-XTypes

**Spec:** §10.5, S. 201 — "Legacy DDS implementations that are not compliant
with DDS-XTYPES should minimally support the RepresentationIdentifier values
CDR_BE and CDR_LE and the type system elements specified in clause F1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — richtet sich an Implementierungen ohne
DDS-XTypes; ZeroDDS unterstützt CDR_BE und CDR_LE (§10.5 Table 10.3).

### §10.6 Beispiel für Daten eines Builtin-Endpoints

**Spec:** §10.6, S. 201–203 — "Following is the SerializedPayload element used
by the SEDPbuiltinSubscriptionsWriter to declare a DataReader." mit
Aufbau und Oktetten.

**Repo:** `crates/rtps/src/subscription_data.rs` (`SubscriptionBuiltinTopicData::from_pl_cdr_le`, `to_pl_cdr_le`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::the_example_of_10_6_is_read`,
`crates/rtps/src/subscription_data.rs::tests::the_parameters_of_the_example_of_10_6_are_written_as_the_spec_prints_them`

**Status:** `done` — die 84 Oktette der Spec werden als der DataReader gelesen, den sie beschreiben; die fünf Parameter des Beispiels schreibt der Encoder mit denselben Oktetten (die Spec lässt die Vorgabewerte aus, ZeroDDS schreibt sie zusätzlich).

### §10.7 Beispiel für Daten eines Anwender-Topics

**Spec:** §10.7, S. 203–204 — "Following is the SerializedPayload element used
by an application DataWriter to send Data on the Topic “Square” with type
“ShapeType”" mit Aufbau und Oktetten.

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `strip_user_encap`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_example_of_10_7_has_the_header_and_body_of_the_spec`

**Status:** `done` — Kopf CDR_LE ({0x00, 0x01}, Optionen 0) und der Rumpf von 24 Oktetten des Beispiels („BLUE“, 34, 100, 24) werden so gelesen, wie die Spec sie druckt; der Kopf für einen XCDR1-Final-Writer ist der der Spec.

## Annex A References

### Annex A Literatur

**Spec:** Annex A, S. 205 — "[1] DDS-SECURITY: DDS Security version 1.1
https://www.omg.org/spec/DDS-SECURITY".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literaturverzeichnis.

---

## Audit-Status

978 done / 3 partial / 2 open / 142 n/a (informative) / 0 n/a (rejected).

Test-Lauf: wird auf dem Linux-Testhost ergänzt.

Offene Punkte und Decision-Records: `ddsi-rtps-2.5.open.md`.
