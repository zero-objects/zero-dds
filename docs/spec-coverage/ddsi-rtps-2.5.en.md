# DDSI-RTPS 2.5 — Spec Coverage

**Source:** `internal/standards/cache/omg/ddsi-rtps-2.5.pdf` (206 pages, OMG formal/2022-04-01) —
[OMG DDSI-RTPS 2.5](https://www.omg.org/spec/DDSI-RTPS/2.5/PDF)

Item-by-item audit against the PDF following `internal/spec-coverage/PROCESS.md`.
Page numbers are PDF pages (PDF page = printed page + 6). Every row of a
normative table is an item of its own. `done` stands only where code and a
test cover the requirement; a test citation names the test function as
`file::module::name`.

**Context:** The DDSI-RTPS 2.5 stack is spread over:

- `crates/rtps/` — header, submessages, HeaderExtension, ParameterList,
  inline QoS, receiver state, writer and reader state machines with their
  proxies, HistoryCache, fragmentation, discovery data types.
- `crates/discovery/` — SPDP, SEDP, TypeLookup, the secure builtin endpoints.
- `crates/transport/`, `crates/transport-udp/` — the transport interface and
  the UDP/IP PSM (§9.5).
- `crates/dcps/` — the DDS side (§8.2.10), the Writer Liveliness Protocol,
  KeyHash, the encapsulation header (§10).
- `crates/qos/` — the QoS policies and their wire format.

**Open items:** see [`ddsi-rtps-2.5.open.en.md`](ddsi-rtps-2.5.open.en.md).

---

## §1 Scope

### §1 Scope

**Spec:** §1, p. 7 — "This specification defines an interoperability wire
protocol for DDS. Its purpose and scope are to ensure that applications based
on different vendors' implementations of DDS can interoperate."

**Repo:** `crates/rtps/src/lib.rs`

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose; the requirements are
in §7.5–§10 and listed there one by one.

---

## §2 Conformance

### §2 Conformance

**Spec:** §2, p. 7 — "Implementations of this specification must comply with
the conformance statements listed in 8.4.2 of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference without a requirement of its
own; the conformance statements of §8.4.2 are listed one by one under
§8.4.2.1–§8.4.2.3.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, p. 7 — list of normative documents [1] DDS 1.4, [2] IDL 4.2,
[3] DDS-XTypes 1.2, [4] NTP version 3 (RFC 1305), [5] MD5 (RFC 1321),
[6] SCTP CRC32c (RFC 4960 appendix B), [7] AUTOSAR CRC routines R20-11.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — cross-reference list. What follows from the
references is listed where it is used: Time_t in NTP format under §9.3.2.2,
MD5 under §9.4.2.15 and §9.6.4.8, CRC32c and CRC64 under §9.4.2.15.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, p. 7 — "For the purposes of this specification, the terms and
definitions given in the normative references apply."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference to terms, no requirement.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, p. 7 — abbreviations CDR, DDS, EDP, GUID, PDP, PIM, PSM, RTPS,
SEDP.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — list of abbreviations.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, p. 9 — "This specification does not change any adopted OMG
specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial statement.

### §6.2 How to Read this Specification

**Spec:** §6.2, p. 9 — reading guide to PIM, PSM and the modules.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reading guide.

### §6.3 Acknowledgments

**Spec:** §6.3, pp. 9–11 — contributing companies and persons per revision.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

### §6.4 Statement of Maturity

**Spec:** §6.4, p. 11 — maturity and known implementations.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background.

---

## §7 Overview

### §7.1 Introduction

**Spec:** §7.1, p. 13 — motivation for a DDS wire protocol.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction without a requirement.

### §7.2 Requirements for a DDS Wire-protocol

**Spec:** §7.2, pp. 13–14 — requirements on a DDS wire protocol as design
goals (no central name server, no single point of failure, extensibility).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — design goals; the mechanisms that follow
from them (SPDP/SEDP without a server, versioning) are listed under §8.5 and
§8.6.

### §7.3 The RTPS Wire-protocol

**Spec:** §7.3, p. 14 — origin and properties of RTPS.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description.

### §7.4 The RTPS Platform Independent Model (PIM)

**Spec:** §7.4–§7.4.4, pp. 14–17 — structure of the PIM into the modules
Structure, Messages, Behavior and Discovery.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the modules are listed under
§8.2–§8.5.

### §7.5 The UDP/IP PSM is mandatory

**Spec:** §7.5, p. 17 — "Multiple PSMs may be supported, but all
implementations of DDS must at least implement the PSM on top of UDP/IP,
which is presented in Clause 9."

**Repo:** `crates/transport-udp/src/udp_transport.rs` (`UdpTransport`);
`crates/dcps/src/runtime.rs` (SPDP/SEDP and user data over UDP).

**Tests:** `crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`,
`crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_multiple_in_order`,
`crates/transport-udp/src/udp_transport.rs::tests::send_recv_v6_loopback`

**Status:** `done`

### §7.6 Transport: unicast address within 16 octets

**Spec:** §7.6, p. 18 — "The transport has a generalized notion of a unicast
address (shall fit within 16 bytes)."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::address: [u8; 16]`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v6_constructor_keeps_full_address`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`

**Status:** `done`

### §7.6 Transport: port within 4 octets

**Spec:** §7.6, p. 18 — "The transport has a generalized notion of a port
(shall fit within 4 bytes)".

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::port: u32`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_roundtrip_le`

**Status:** `done`

### §7.6 Transport: send a datagram to an address/port

**Spec:** §7.6, p. 18 — "The transport can send a datagram (uninterpreted
sequence of octets) to a specific address/port."

**Repo:** `crates/transport/src/lib.rs` (`Transport::send`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransport::send`)

**Tests:** `crates/transport-udp/src/udp_transport.rs::tests::loopback_send_and_recv_delivers_datagram`,
`crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`

**Status:** `done`

### §7.6 Transport: receive a datagram at an address/port

**Spec:** §7.6, p. 18 — "The transport can receive a datagram at a specific
address/port."

**Repo:** `crates/transport/src/lib.rs` (`Transport::recv`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransport::recv`,
`UdpTransport::bind_v4`)

**Tests:** `crates/transport-udp/src/udp_transport.rs::tests::bind_v4_returns_local_locator_with_assigned_port`,
`crates/transport-udp/src/udp_transport.rs::tests::loopback_multiple_datagrams_in_order`

**Status:** `done`

### §7.6 Transport drops incomplete or corrupted messages

**Spec:** §7.6, p. 18 — "The transport will drop messages if incomplete or
corrupted during transfer (i.e., RTPS assumes messages are complete and not
corrupted)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an assumption about the transport, not a
requirement on the RTPS implementation. For UDP the operating system checks
the UDP header checksum; the optional message checksum of the
HeaderExtension is listed under §8.3.3.2 and §9.4.2.15.

---

## §8.1 Introduction

### §8.1 The PIM as a virtual machine

**Spec:** §8.1, p. 19 — "This description is not intended to constrain the
internal implementation in any way. The only criteria for a compliant
implementation is that the externally-observable behavior satisfies the
requirements for interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — sets the yardstick for the PIM items: what
is checked is the externally observable behaviour, not the class structure.

---

## §8.2 Structure Module

### §8.2.1 Overview

**Spec:** §8.2.1, p. 19 — every RTPS entity corresponds to a DDS entity; the
HistoryCache is the interface between the two.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the relation is listed per
transition under §8.2.10.

### §8.2.1.1 Table 8.1 classes of the virtual machine

**Spec:** §8.2.1.1, pp. 20–21, Table 8.1 — summary of the classes Entity,
Endpoint, Participant, Writer, Reader, HistoryCache, CacheChange, Data.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — summary; every class is listed with its
attributes under §8.2.2–§8.2.9.

### §8.2.1.2 Table 8.2 GUID_t

**Spec:** §8.2.1.2, p. 21, Table 8.2 — "GUID_t … Must be possible to
represent using 16 octets. The following values are reserved by the
protocol: GUID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`, `Guid::WIRE_SIZE`,
`Guid::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/rtps/src/wire_types.rs::tests::guid_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done` — `Guid::UNKNOWN` is composed of the two tested UNKNOWN
parts.

### §8.2.1.2 Table 8.2 GuidPrefix_t

**Spec:** §8.2.1.2, p. 21, Table 8.2 — "GuidPrefix_t … Must be possible to
represent using 12 octets. The following values are reserved by the
protocol: GUIDPREFIX_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix`, `GuidPrefix::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`

**Status:** `done`

### §8.2.1.2 Table 8.2 EntityId_t

**Spec:** §8.2.1.2, p. 21, Table 8.2 — "EntityId_t … Must be possible to
represent using 4 octets. The following values are reserved by the
protocol: ENTITYID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId`, `EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §8.2.1.2 Table 8.2 SequenceNumber_t

**Spec:** §8.2.1.2, p. 21, Table 8.2 — "SequenceNumber_t … Must be possible
to represent using 64 bits. The following values are reserved by the
protocol: SEQUENCENUMBER_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber(i64)`,
`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §8.2.1.2 Table 8.2 Locator_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "It must be possible to represent
the discriminator and port number using 4 octets each, the address using 16
octets." Reserved: LOCATOR_INVALID, LOCATOR_KIND_INVALID,
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

**Status:** `done` — the values of the constants are listed under §9.3.2.4.

### §8.2.1.2 Table 8.2 TopicKind_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "Enumeration used to distinguish
whether a Topic has defined some fields within to be used as the 'key' …
NO_KEY, WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs` (`TopicKind::{NoKey, WithKey}`,
`EntityKind::topic_kind`, `TopicKind::writer_entity_kind`,
`TopicKind::reader_entity_kind`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::topic_kind_entity_kind_pairs_round_trip`

**Status:** `done`

### §8.2.1.2 Table 8.2 ChangeKind_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "It can take the values: ALIVE,
ALIVE_FILTERED, NOT_ALIVE_DISPOSED, NOT_ALIVE_UNREGISTERED".

**Repo:** `crates/rtps/src/history_cache.rs` (`ChangeKind::{Alive,
AliveFiltered, NotAliveDisposed, NotAliveUnregistered,
NotAliveDisposedUnregistered}`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::change_kind_distinct_variants`,
`crates/rtps/src/history_cache.rs::tests::change_kind_alive_filtered_is_alive_but_not_relevant`,
`crates/rtps/src/history_cache.rs::tests::change_kind_not_alive_kinds_are_not_alive`

**Status:** `done` — `NotAliveDisposedUnregistered` is an additional
combination for a PID_STATUS_INFO with both bits.

### §8.2.1.2 Table 8.2 ChangeCount_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "Type used to hold a counter
representing the number of HistoryCache changes that belong to a certain
category."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`, `Option<u64>`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — the counter is 64 bits wide (high, low; eight octets, §9.3.2.11).

### §8.2.1.2 Table 8.2 ReliabilityKind_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "Enumeration used to indicate the
level of the reliability used for communications. It can take the values:
BEST_EFFORT, RELIABLE."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind::{BestEffort,
Reliable}`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::try_from_u32_is_strict`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`

**Status:** `done`

### §8.2.1.2 Table 8.2 InstanceHandle_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "Type used to represent the identity
of a data-object whose changes in value are communicated by the RTPS
protocol."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`);
`crates/dcps/src/instance_handle.rs` (`InstanceHandle`)

**Tests:** `crates/dcps/src/instance_handle.rs::tests::from_guid_distinguishes_keys`,
`crates/rtps/src/history_cache.rs::tests::keep_last_per_instance_keeps_the_newest_of_each_instance`

**Status:** `done` — at the RTPS level the KeyHash (§9.6.4.8) carries the
identity of the instance.

### §8.2.1.2 Table 8.2 ProtocolVersion_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — reserves PROTOCOLVERSION,
PROTOCOLVERSION_1_0, _1_1, _2_0, _2_1, _2_2, _2_4; "PROTOCOLVERSION is an
alias for the most recent version, in this case PROTOCOLVERSION_2_4".

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion::{V1_0, V1_1,
V2_0, V2_1, V2_2, V2_3, V2_4, V2_5, CURRENT}`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_aliases_match_spec`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_all_aliases_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_default_is_2_5`

**Status:** `done` — `CURRENT` is 2.5 per §8.3.3.1 ("protocol version
2.5"); the statement "in this case PROTOCOLVERSION_2_4" in Table 8.2
contradicts §8.3.3.1.

### §8.2.1.2 Table 8.2 VendorId_t

**Spec:** §8.2.1.2, p. 22, Table 8.2 — "The possible values for the vendorId
are assigned by the OMG. The following values are reserved by the protocol:
VENDORID_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId`, `VendorId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_unknown_is_two_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`

**Status:** `done` — the value ZeroDDS sends is listed under §8.3.3.1
(vendorId).

### §8.2.1.3 Configuration attributes of the RTPS Entities

**Spec:** §8.2.1.3, p. 23, Figure 8.2 — configuration attributes of the
entities; writer and reader attributes follow in §8.4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the attributes are listed one by
one in Table 8.8, Table 8.9 and §8.4.7/§8.4.10.

### §8.2.2 HistoryCache: role on the writer and the reader side

**Spec:** §8.2.2, p. 23 — writer side: "the partial history of changes …
needed to service existing and future matched RTPS Reader endpoints"; reader
side: "the partial superposition of changes to data-objects made by all the
matched RTPS Writer endpoints".

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter`, field `cache`);
`crates/rtps/src/reliable_reader.rs` (`ReliableReader`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`,
`crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`

**Status:** `done`

### §8.2.2 Table 8.3 HistoryCache::changes

**Spec:** §8.2.2, p. 24, Table 8.3 — "changes: CacheChange[*] — The list of
CacheChanges contained in the HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache`, `get`,
`iter_range`, `len`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::insert_and_get`,
`crates/rtps/src/history_cache.rs::tests::iter_range_is_ordered`

**Status:** `done`

### §8.2.2.1 HistoryCache new

**Spec:** §8.2.2.1, p. 25 — "This operation creates a new RTPS HistoryCache.
The newly-created history cache is initialized with an empty list of
changes."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::new`,
`HistoryCache::new_with_kind`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::new_cache_is_empty`

**Status:** `done`

### §8.2.2.2 HistoryCache add_change

**Spec:** §8.2.2.2, p. 25 — "This operation inserts the CacheChange a_change
into the HistoryCache. This operation will only fail if there are not enough
resources to add the change to the HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::insert`,
`HistoryCache::insert_returning_evicted`, `CacheError`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::insert_and_get`,
`crates/rtps/src/history_cache.rs::tests::insert_at_capacity_is_err`,
`crates/rtps/src/history_cache.rs::tests::insert_duplicate_is_err`

**Status:** `done` — `insert` also refuses a sequence number already
present; not observable from outside, since a writer assigns each number
once.

### §8.2.2.2 RESOURCE_LIMITS and propagation of errors to the DDS user

**Spec:** §8.2.2.2, p. 25 — "It is the responsibility of the DDS service
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

**Spec:** §8.2.2.3, p. 25 — "This operation indicates that a
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

**Spec:** §8.2.2.4, p. 25 — "This operation retrieves the smallest value of
the CacheChange::sequenceNumber attribute among the CacheChange stored in the
HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::min_sn`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::min_max_sn_reflect_content`

**Status:** `done`

### §8.2.2.5 HistoryCache get_seq_num_max

**Spec:** §8.2.2.5, p. 25 — "This operation retrieves the largest value of
the CacheChange::sequenceNumber attribute among the CacheChange stored in the
HistoryCache."

**Repo:** `crates/rtps/src/history_cache.rs` (`HistoryCache::max_sn`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::min_max_sn_reflect_content`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::kind

**Spec:** §8.2.3, p. 26, Table 8.5 — "kind: ChangeKind_t — Identifies the
kind of change."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::kind`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::kind`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::cache_change_alive_constructor`,
`crates/rtps/src/reliable_reader.rs::tests::dispose_data_yields_not_alive_disposed`,
`crates/rtps/src/reliable_reader.rs::tests::unregister_data_yields_not_alive_unregistered`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::writerGuid

**Spec:** §8.2.3, p. 26, Table 8.5 — "writerGuid: GUID_t — The GUID_t that
identifies the RTPS Writer that made the change".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DeliveredSample::writer_guid`);
writer side implicit, one `HistoryCache` per writer.

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::instanceHandle

**Spec:** §8.2.3, p. 26, Table 8.5 — "instanceHandle: InstanceHandle_t —
Identifies the instance of the data-object to which the change applies."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::key_hash`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::the_instance_index_follows_inserts_evictions_and_removals`,
`crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::sequenceNumber

**Spec:** §8.2.3, p. 26, Table 8.5 — "sequenceNumber: SequenceNumber_t —
Sequence number assigned by the RTPS Writer to uniquely identify the change."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::sequence_number`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/writer.rs::tests::writer_increments_sequence_number_per_write`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::data_value

**Spec:** §8.2.3, p. 26, Table 8.5 — "data_value: Data — The data value
associated with the change. Depending on the kind of CacheChange, there may
be no associated data."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::payload`,
`CacheChange::lifecycle`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::cache_change_alive_constructor`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_without_a_key_clears_k_and_sends_no_payload`

**Status:** `done`

### §8.2.3 Table 8.5 CacheChange::inlineQos

**Spec:** §8.2.3, p. 26, Table 8.5 — "inlineQos: ParameterList — Contains QoS
that may affect the interpretation of the CacheChange::data_value."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::inline_qos`,
`CacheChange::with_inline_qos`); reader side:
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample` with the evaluated
parameters `key_hash`, `coherent_set_sn`, `related_sample_identity`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`,
`crates/rtps/src/reliable_reader.rs::tests::delivered_sample_carries_the_coherent_set_marker`

**Status:** `done`

### §8.2.4 Table 8.6 Entity::guid

**Spec:** §8.2.4, p. 26, Table 8.6 — "guid: GUID_t — Globally and uniquely
identifies the RTPS Entity within the DDS domain. Maps to the value of the
DDS BuiltinTopicKey_t".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`);
`crates/dcps/src/builtin_topics.rs` (key of the builtin topic data)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/dcps/src/instance_handle.rs::tests::from_guid_distinguishes_keys`

**Status:** `done`

### §8.2.4.1 Table 8.7 GUID = <prefix, entityId>

**Spec:** §8.2.4.1, pp. 26–27, Table 8.7 — "The GUID is built as a tuple
<prefix, entityId> combining a GuidPrefix_t prefix and an EntityId_t
entityId."

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid::new`, `Guid::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`

**Status:** `done`

### §8.2.4.2 Participant GUID <prefix, ENTITYID_PARTICIPANT>

**Spec:** §8.2.4.2, p. 27 — "Every Participant has GUID <prefix,
ENTITYID_PARTICIPANT>".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::PARTICIPANT`);
`crates/dcps/src/participant.rs` (`random_guid_prefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`,
`crates/dcps/src/participant.rs::tests::random_guid_prefixes_share_host_id_within_process`

**Status:** `done`

### §8.2.4.2 Prefix unique in the domain

**Spec:** §8.2.4.2, p. 27 — "The implementation is free to choose the
prefix, as long as every Participant in the Domain has a unique GUID."

**Repo:** `crates/dcps/src/participant.rs` (`next_process_unique`, `namespaced_pid`, `process_unique`); `crates/dcps/src/runtime.rs` (`DcpsRuntime::prefix_collisions`)

**Tests:** `crates/dcps/src/participant.rs::tests::the_prefixes_of_one_process_are_pairwise_different`,
`crates/dcps/src/participant.rs::tests::two_pid_namespaces_with_the_same_pid_give_different_prefixes`,
`crates/dcps/src/runtime.rs::tests::a_beacon_with_the_own_prefix_from_another_participant_is_counted`

**Status:** `done` — the prefixes of the participants of one process are pairwise different (counter of the process, 65536 per process, also for the C API and the bridges); across processes and hosts the construction prevents the collision (item "Prefix uniqueness across hosts and processes").

### §8.2.4.2 Prefix uniqueness across hosts and processes

**Spec:** §8.2.4.2, p. 27 — "every Participant in the Domain has a unique GUID".

**Repo:** `crates/dcps/src/participant.rs` (`namespaced_pid`, `pid_namespace_inode`, `next_process_unique`, `process_unique`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::prefix_collisions`, `same_process`)

**Tests:** `crates/dcps/src/participant.rs::tests::two_pid_namespaces_with_the_same_pid_give_different_prefixes`,
`crates/dcps/src/participant.rs::tests::the_prefixes_of_one_process_share_ten_octets_and_differ_in_the_tail`,
`crates/dcps/src/participant.rs::tests::random_guid_prefixes_share_host_id_within_process`,
`crates/dcps/src/runtime.rs::tests::a_beacon_with_the_own_prefix_from_another_participant_is_counted`

**Status:** `done` — the prefix consists of the VendorId, the host value (4 octets), the process id and a process part (a seed from the operating system's randomness plus a counter of the process). The process id carries the mark of the PID namespace (`namespaced_pid`: XOR with a mask from the inode of `/proc/self/ns/pid`, zero in the initial namespace, one-to-one within a namespace). Containers with the host's network (the same host value) and a PID namespace of their own whose first process has the number 1 therefore carry different prefixes. `same_process` (first ten octets) and `same_host` (host value) read as before. Remaining collision: two different host names with the same 32-bit hash, the same marked process id and the same process part; `DcpsRuntime::prefix_collisions` counts an SPDP beacon with the own prefix and foreign locators.

### §8.2.4.3 GUID of an endpoint group

**Spec:** §8.2.4.3, p. 27 — "The endpoint Groups contained by a Participant
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

### §8.2.4.4 GUID of the endpoints of a participant

**Spec:** §8.2.4.4, p. 27 — "The Endpoints contained by a Participant with
GUID <participantPrefix, ENTITYID_PARTICIPANT> have the GUID
<participantPrefix, entityId>. The entityId is the unique identification of
the Endpoint relative to the Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`next_entity_key`, `entity_key_in_use`, `held_entity_keys`, `release_entity_key`);
`crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs` (`Drop` of the inner types)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_the_key_of_a_live_writer`,
`crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_reserved_and_group_keys_until_they_are_given_back`

**Status:** `done` — the 3-octet key comes from a counter; after the 24 bits wrap, a key is skipped that a registered writer, reader, an announced topic, a reserved id that is not registered yet or a group id (publisher, subscriber) holds; a key that is given back is free again.

### §8.2.5 Table 8.8 Participant::defaultUnicastLocatorList

**Spec:** §8.2.5, p. 28, Table 8.8 — "Default list of unicast locators …
These are the unicast locators that will be used in case the Endpoint does
not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::default_unicast_locators`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done` — the endpoint's fallback to the default locators is
listed under §8.5.5.1.

### §8.2.5 Table 8.8 Participant::defaultMulticastLocatorList

**Spec:** §8.2.5, p. 28, Table 8.8 — "Default list of multicast locators …
used in case the Endpoint does not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::default_multicast_locators`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.5 Table 8.8 Participant::protocolVersion

**Spec:** §8.2.5, p. 28, Table 8.8 — "protocolVersion: ProtocolVersion_t —
Identifies the version of the RTPS protocol that the Participant uses to
communicate."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::protocol_version`);
`crates/rtps/src/header.rs` (`RtpsHeader::new`)

**Tests:** `crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.5 Table 8.8 Participant::vendorId

**Spec:** §8.2.5, p. 28, Table 8.8 — "vendorId: VendorId_t — Identifies the
vendor of the RTPS middleware that contains the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`ParticipantBuiltinTopicData::vendor_id`); `crates/rtps/src/header.rs`

**Tests:** `crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.2.6 RTPS Group: a publisher contains writers, a subscriber readers

**Spec:** §8.2.6, pp. 28–29 — "The RTPS Publisher contains RTPS Writer
endpoints. The RTPS Publisher maps to a DDS Publisher. The RTPS Subscriber
contains RTPS Reader endpoints. The RTPS Subscriber maps to a DDS
Subscriber."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_writer_of_another_publisher_is_left_as_it_was`,
`crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`,
`crates/dcps/tests/entity_deletion.rs::publisher_delete_contained_entities_deletes_every_writer`

**Status:** `done` — the GUID of the group is listed under §8.2.4.3.

### §8.2.7 Table 8.9 Endpoint::unicastLocatorList

**Spec:** §8.2.7, p. 29, Table 8.9 — "List of unicast locators … that can be
used to send messages to the Endpoint. The list may be empty."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::unicast_locators`);
`crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::unicast_locators`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_locators_roundtrip_le`

**Status:** `done` — the reader side uses the same helpers
(`encode_locator_params`, `collect_locator_params`); its test is listed
under §8.5.4.4.

### §8.2.7 Table 8.9 Endpoint::multicastLocatorList

**Spec:** §8.2.7, p. 29, Table 8.9 — "List of multicast locators … that can
be used to send messages to the Endpoint. The list may be empty."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::multicast_locators`);
`crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::multicast_locators`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_locators_roundtrip_le`

**Status:** `done` — the reader side uses the same helpers; its test is
listed under §8.5.4.4.

### §8.2.7 Table 8.9 Endpoint::reliabilityLevel

**Spec:** §8.2.7, p. 29, Table 8.9 — "reliabilityLevel: ReliabilityKind_t —
The levels of reliability supported by the Endpoint. Maps to the RELIABILITY
QoS 'kind.'"

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::reliability`); `crates/rtps/src/writer.rs`
(`BestEffortWriter`); `crates/rtps/src/reliable_writer.rs` (`ReliableWriter`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::roundtrip_le`,
`crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`

**Status:** `done`

### §8.2.7 Table 8.9 Endpoint::topicKind

**Spec:** §8.2.7, p. 29, Table 8.9 — "topicKind: TopicKind_t — Used to
indicate whether the Endpoint supports instance lifecycle management
operations (see 8.7.4)."

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::topic_kind`);
`crates/dcps/src/publisher.rs` (choice of the `entityKind` from
`DdsType::HAS_KEY`)

**Tests:** `crates/dcps/src/publisher.rs::tests::live_datawriter_entity_kind_is_nokey_for_keyless_type`,
`crates/rtps/src/wire_types.rs::tests::topic_kind_of_the_predefined_entity_ids`

**Status:** `done`

### §8.2.7 Table 8.9 Endpoint::endpointGroup

**Spec:** §8.2.7, p. 29, Table 8.9 — "endpointGroup: EntityId_t — Used to
identify the RTPS Group (Publisher or Subscriber) to which the Endpoint
belongs."

**Repo:** `crates/dcps/src/runtime.rs` (`UserWriterConfig::group_entity_id`,
`UserReaderConfig::group_entity_id`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §8.2.8 An RTPS Writer belongs to a publisher

**Spec:** §8.2.8, pp. 29–30 — "Its role is to transfer all CacheChange
changes in its HistoryCache to the HistoryCache of the matching remote RTPS
Readers. An RTPS Writer belongs to an RTPS Group of type Publisher."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`,
`crates/rtps/tests/reliable_e2e.rs::e2e_zero_loss_delivers_all_in_order`

**Status:** `done`

### §8.2.9 An RTPS Reader belongs to a subscriber

**Spec:** §8.2.9, p. 30 — "RTPS Reader specializes RTPS Endpoint and
represents the actor that receives CacheChange messages from the matched
RTPS Writer endpoints. An RTPS Reader belongs to an RTPS Group of type
Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`);
`crates/rtps/src/reliable_reader.rs` (`ReliableReader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`,
`crates/rtps/tests/reliable_e2e.rs::e2e_10_percent_loss_delivers_all_in_order`

**Status:** `done`

### §8.2.10 Relation to DDS Entities

**Spec:** §8.2.10, p. 30 — "How exactly a DDS Entity interacts with the
HistoryCache however, is implementation specific and not formally modeled by
the RTPS protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — explains the transitions; they are listed
one by one under §8.2.10.1 and §8.2.10.2.

### §8.2.10.1 DataWriter: every change is relevant to every reader

**Spec:** §8.2.10.1, pp. 30–31 — "By default, all changes in the
HistoryCache are considered relevant for each matching remote RTPS Reader.
… The Reader itself will always filter."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`,
delivery to every `ReaderProxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done` — the reader-side filter is listed under §8.7.3.

### §8.2.10.1 Table 8.11 DataWriter T1 (new DDS DataWriter)

**Spec:** §8.2.10.1, p. 32, Table 8.11 T1 — "the_rtps_writer = new
RTPS::Writer; the_dds_writer.related_rtps_writer := the_rtps_writer;"

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`);
`crates/dcps/src/runtime.rs` (`register_user_writer_as`)

**Tests:** `crates/dcps/src/publisher.rs::tests::live_datawriter_entity_kind_is_nokey_for_keyless_type`,
`crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T2 (write)

**Spec:** §8.2.10.1, p. 32, Table 8.11 T2 — "a_change :=
the_rtps_writer.new_change(ALIVE, data, inlineQos, handle);
the_rtps_writer.writer_cache.add_change(a_change);" postcondition
"the_rtps_writer.writer_cache.get_seq_num_max() == a_change.sequenceNumber".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T3 (dispose, WITH_KEY)

**Spec:** §8.2.10.1, p. 32 — "if (the_rtps_writer.topicKind == WITH_KEY) {
a_change := the_rtps_writer.new_change(NOT_ALIVE_DISPOSED, <nil>, inlineQos,
handle); the_rtps_writer.writer_cache.add_change(a_change); }"

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose_w_timestamp`);
`crates/dcps/src/runtime.rs` (`write_user_lifecycle`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write_lifecycle`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_with_a_key_sets_k_and_carries_it`

**Status:** `done`

### §8.2.10.1 Table 8.11 DataWriter T4 (unregister, WITH_KEY)

**Spec:** §8.2.10.1, p. 33 — "if (the_rtps_writer.topicKind == WITH_KEY) {
a_change := the_rtps_writer.new_change(NOT_ALIVE_UNREGISTERED, <nil>,
inlineQos, handle); the_rtps_writer.writer_cache.add_change(a_change); }"

**Repo:** `crates/dcps/src/publisher.rs`
(`DataWriter::unregister_instance_w_timestamp`);
`crates/dcps/src/runtime.rs` (`write_user_lifecycle`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`,
`crates/rtps/src/reliable_reader.rs::tests::unregister_data_yields_not_alive_unregistered`

**Status:** `done`

### §8.2.10.1 T3/T4 without effect for NO_KEY

**Spec:** §8.2.10.1, pp. 32–33 — dispose and unregister: "This operation has
no effect if the topicKind==NO_KEY."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle_with`, `keyless_lifecycle_not_sent`); `writer.keyless_lifecycle_on_wire`

**Tests:** `crates/dcps/src/runtime.rs::tests::dispose_and_unregister_of_a_no_key_writer_send_no_marker_unless_asked_for`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`,
`crates/dcps/tests/keyless_instance.rs::the_writer_disposes_the_one_instance_of_a_keyless_topic`

**Status:** `done` — dispose and unregister of a writer without a key create no change and send no marker (counted in `DcpsRuntime::keyless_lifecycle_not_sent`); the writer's own instance state changes (DDS 1.4 §2.2.1.2.2: one instance). `writer.keyless_lifecycle_on_wire` (environment `ZERODDS_WRITER_KEYLESS_LIFECYCLE_ON_WIRE`) sends the marker (status bits only) for a peer that expects it. Expected from the vendors: readers of topics without a key no longer see NOT_ALIVE_DISPOSED from ZeroDDS writers; whether Cyclone DDS, Fast DDS or OpenDDS themselves send a marker for NO_KEY is not measured.

### §8.2.10.1 Table 8.11 DataWriter T5 (delete DDS DataWriter)

**Spec:** §8.2.10.1, p. 33 — "delete the_dds_writer.related_rtps_writer;"

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`

**Status:** `done`

### §8.2.10.2 DataReader: read and take access the HistoryCache

**Spec:** §8.2.10.2, p. 33 — "The changes returned to the user are those in
the HistoryCache that pass all Reader specific filters, if any."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`,
`DataReader::take`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T1 (new DDS DataReader)

**Spec:** §8.2.10.2, p. 34, Table 8.12 T1 — "the_rtps_reader = new
RTPS::Reader; the_dds_reader.related_rtps_reader := the_rtps_reader;"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`);
`crates/dcps/src/runtime.rs` (`register_user_reader`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T2 (read)

**Spec:** §8.2.10.2, p. 34 — "Changes returned to the application remain in
the RTPS Reader's HistoryCache such that subsequent read or take operations
can find them again."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`,
`DataReader::read_with_info`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/tests/reader_history_resource_limits.rs::plain_read_keep_last_1_keeps_only_the_newest_per_instance_and_is_idempotent`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T3 (take)

**Spec:** §8.2.10.2, pp. 34–35 — "Changes returned to the application are
removed from the RTPS Reader's HistoryCache such that subsequent read or take
operations do not find the same change."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take`,
`DataReader::take_with_info`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** `done`

### §8.2.10.2 Table 8.12 DataReader T4 (delete DDS DataReader)

**Spec:** §8.2.10.2, p. 35 — "delete the_dds_reader.related_rtps_reader;"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`,
`crates/dcps/tests/entity_deletion.rs::lookup_datareader_hands_out_a_handle_to_the_same_reader`

**Status:** `done`

---

## §8.3 Messages Module

### §8.3.1 Overview

**Spec:** §8.3.1, p. 35 — structure of the module; "The number of
Submessages that can be sent in a single RTPS Message is only limited by the
maximum message size the underlying transport can support."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the requirements follow in
§8.3.2–§8.3.8.

### §8.3.2 Table 8.13 ProtocolId_t (PROTOCOL_RTPS)

**Spec:** §8.3.2, p. 35, Table 8.13 — "ProtocolId_t: Enumeration used to
identify the protocol. The following values are reserved by the protocol:
PROTOCOL_RTPS".

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`,
`crates/rtps/src/header.rs::tests::header_decode_rejects_invalid_magic`

**Status:** `done`

### §8.3.2 Table 8.13 SubmessageFlag

**Spec:** §8.3.2, p. 35, Table 8.13 — "A Submessage flag takes a boolean value
and affects the parsing of the Submessage by the receiver."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::flags`,
`FLAG_E_LITTLE_ENDIAN`); flag constants in `crates/rtps/src/submessages.rs`

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_is_little_endian_flag`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §8.3.2 Table 8.13 SubmessageKind

**Spec:** §8.3.2, p. 36, Table 8.13 — reserves "RTPS_HE, DATA, GAP,
HEARTBEAT, ACKNACK, PAD, INFO_TS, INFO_REPLY, INFO_DST, INFO_SRC, DATA_FRAG,
NACK_FRAG, HEARTBEAT_FRAG".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId`);
`crates/rtps/src/header_extension.rs` (`SUBMESSAGE_ID_HEADER_EXTENSION`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`,
`crates/rtps/tests/cyclone_he_must_understand.rs::local_encoder_produces_must_understand_bit`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.2 Table 8.13 Time_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold a timestamp. Should
have at least nano-second resolution. The following values are reserved by
the protocol: TIME_ZERO, TIME_INVALID, TIME_INFINITE".

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`, `TIME_INVALID`, `TIME_INFINITE`); `crates/dcps/src/time.rs`

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::time_t_seconds_are_unsigned`,
`crates/dcps/src/time.rs::tests::the_reserved_time_t_values_cross_the_wire_as_the_spec_gives_them`

**Status:** `done` — `seconds` is unsigned (`u32`, §9.3.2.1), the three reserved values TIME_ZERO, TIME_INVALID and TIME_INFINITE are constants and go on the wire with the values of the spec. The DCPS `Time` counts seconds as a `long`; the octets are the same.

### §8.3.2 Table 8.13 Count_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold a count that is
incremented monotonically, used to identify message duplicates."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::count`,
`AckNackSubmessage::count`, `HeartbeatFragSubmessage::count`,
`NackFragSubmessage::count`, each `i32`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_wraps_around_at_i32_max_per_spec_8_4_15_7`

**Status:** `done` — the receiver's use of the counter is listed under
§8.3.8.1, §8.3.8.6 and §8.4.15.7.

### §8.3.2 Table 8.13 Checksum_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold a checksum. Used to
detect RTPS message corruption by the underlying transport. The following
values are reserved by the protocol: CHECKSUM_INVALID."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::{None,
Crc32c, Crc64, Md5}`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_value_kind_matches`,
`crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`

**Status:** `done` — `ChecksumValue::None` stands for CHECKSUM_INVALID.

### §8.3.2 Table 8.13 MessageLength_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold the length of an
RTPS Message. The following values are reserved by the protocol:
MESSAGE_LENGTH_INVALID".

**Repo:** `crates/rtps/src/header_extension.rs`
(`HeaderExtension::message_length: Option<u32>`); `crates/rtps/src/receiver_state.rs`
(`ReceiverState::message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_message_length_only_le`,
`crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done` — `None` stands for MESSAGE_LENGTH_INVALID.

### §8.3.2 Table 8.13 ParameterId_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to uniquely identify a
parameter in a parameter list. … A range of values is reserved for
protocol-defined parameters, while another range can be used for
vendor-defined parameters".

**Repo:** `crates/rtps/src/parameter_list.rs` (`Parameter::id: u16`,
`VENDOR_SPECIFIC_BIT`, `MUST_UNDERSTAND_BIT`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`,
`crates/rtps/src/parameter_list.rs::tests::must_understand_unknown_optional_pid_skips`

**Status:** `done` — the split into ranges is listed under §8.3.5.9 and
§9.4.2.11.

### §8.3.2 Table 8.13 FragmentNumber_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold fragment numbers.
Must be possible to represent using 32 bits."

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_wire_size_is_four`,
`crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`

**Status:** `done`

### §8.3.2 Table 8.13 GroupDigest_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold a digest value that
uniquely identifies a group of Entities belonging to the same Participant."

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §8.3.2 Table 8.13 UExtension4_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold an undefined 4-byte
value. It is intended to be used in future revisions of the specification."

**Repo:** `crates/rtps/src/wire_types.rs` (`UExtension4`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::uextension4_roundtrip`

**Status:** `done`

### §8.3.2 Table 8.13 WExtension8_t

**Spec:** §8.3.2, p. 36, Table 8.13 — "Type used to hold an undefined 8-byte
value. It is intended to be used in future revisions of the specification."

**Repo:** `crates/rtps/src/wire_types.rs` (`WExtension8`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::wextension8_roundtrip`

**Status:** `done`

### §8.3.3 Message = header + submessages

**Spec:** §8.3.3, p. 36 — "The overall structure of an RTPS Message consists
of a fixed-size leading RTPS Header followed by a variable number of RTPS
Submessage parts. Each Submessage in turn consists of a SubmessageHeader and
a variable number of SubmessageElements."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`);
`crates/rtps/src/message_builder.rs` (`MessageBuilder`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/tests/vendor_frames.rs::cyclone_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.3 Message length from the transport or the HeaderExtension

**Spec:** §8.3.3, p. 37 — "The length may also be sent by the underlying
transport that carries the RTPS message. In this case it may be omitted from
the HeaderExtension. … a stream-oriented transport (like TCP) would need to
include the length in the RTPS HeaderExtension".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: length from the
UDP datagram)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`

**Status:** `done` — for UDP (§9) the transport carries the length; the TCP
mapping is not part of the UDP/IP PSM.

### §8.3.3.1 Header at the start of every message

**Spec:** §8.3.3.1, p. 37 — "The RTPS Header must appear at the beginning of
every message."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::to_bytes`,
`RtpsHeader::from_bytes`); `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_invalid_magic`,
`crates/rtps/src/writer.rs::tests::writer_produces_decodable_datagram`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.protocol = PROTOCOL_RTPS

**Spec:** §8.3.3.1, p. 38 — "The protocol identifies the message as an RTPS
message. This value is set to PROTOCOL_RTPS."

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`, `RtpsHeader::to_bytes`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.version = 2.5

**Spec:** §8.3.3.1, p. 38 — "Implementations following this version of the
document implement protocol version 2.5 (major = 2, minor = 5) and have this
field set to PROTOCOLVERSION."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::new` sets
`ProtocolVersion::V2_5`)

**Tests:** `crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`

**Status:** `done`

### §8.3.3.1 Table 8.14 Header.vendorId

**Spec:** §8.3.3.1, p. 38 — "The vendorId identifies the vendor of the
middleware that implemented the RTPS protocol … The possible values for the
vendorId are assigned by the OMG."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::vendor_id`);
`crates/rtps/src/wire_types.rs` (`VendorId::ZERODDS = [0x01, 0xF0]`)

**Tests:** `crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_zerodds_constant`

**Status:** `partial` — the field is sent and read; ZeroDDS sends `0x01F0`,
according to the comment on `VendorId::ZERODDS` an interim value and not one
assigned by the OMG (an `open` item of its own under §8.3.5.2). Blocker: a hard external one, the assignment of a VendorId by the OMG; the request has been pending since 22 June. Until then ZeroDDS sends `0x01F0`, a value the OMG list does not hold (assigned up to `{0x01,0x1C}`, `{0x01,0x1D}` and `{0x01,0x1E}` reserved).

### §8.3.3.1 Table 8.14 Header.guidPrefix

**Spec:** §8.3.3.1, p. 38 — "The guidPrefix defines a default prefix that can
be used to reconstruct the Globally Unique Identifiers (GUIDs) that appear
within the Submessages contained in the message."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::guid_prefix`);
`crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`)

**Tests:** `crates/rtps/src/header.rs::tests::header_guid_prefix_at_bytes_8_to_19`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.3.1 Header structure fixed in version 2

**Spec:** §8.3.3.1, p. 38 — "The structure of the RTPS Header cannot be
changed in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::WIRE_SIZE = 20`)

**Tests:** `crates/rtps/src/header.rs::tests::header_total_size_is_20_bytes`,
`crates/rtps/src/header.rs::tests::header_roundtrip`

**Status:** `done`

### §8.3.3.2 HeaderExtension immediately after the header

**Spec:** §8.3.3.2, p. 38 — "The HeaderExtension may optionally appear
immediately following the Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: HE only as the
first submessage)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`

**Status:** `done`

### §8.3.3.2 HeaderExtension skippable for versions up to 2.4

**Spec:** §8.3.3.2, p. 39 — "versions of the protocol that do not understand
the HeaderExtension will treat it as 'unknown messge kind', skip it, and
continue processing the submessages that follow it".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension`

**Status:** `n/a (informative)` — describes the behaviour of older
implementations; for ZeroDDS itself §8.3.4.1 applies.

### §8.3.3.2 Table 8.15 HE EndiannessFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_E`,
`HeaderExtension::little_endian`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::empty_he_be_roundtrip`,
`crates/rtps/src/header_extension.rs::tests::wire_layout_be_message_length_correct`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE LengthFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "LengthFlag … Indicates the
messageLength field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_L`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::flag_byte_with_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE TimestampFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "TimestampFlag … Indicates the
rtpsSendTimestamp field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_T`, bit 2)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_timestamp_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE UExtensionFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "UExtensionFlag … Indicates the
uExtension4 field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_U`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE WExtensionFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "WExtensionFlag … Indicates the
wExtension8 field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_W`, bit 4)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE ChecksumFlags

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "ChecksumFlags (2 flags) … Indicates
the presence and format of the messageChecksum field."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_C0`, `HE_FLAG_C1`,
`ChecksumKind::from_flags`, `ChecksumKind::to_flag_bits`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE ParametersFlag

**Spec:** §8.3.3.2, p. 39, Table 8.15 — "ParametersFlag … Indicates the
parameters field is present."

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_P`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE messageLength

**Spec:** §8.3.3.2, pp. 39–40 — "The messageLength indicates the length of the
complete RTPS Message, starting from the beginning of the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, HeaderExtension); `crates/rtps/src/header_extension.rs` (`validate_message_length`, `restate_for_rebuilt_message`); `crates/dcps/src/runtime.rs` (`unprotect_volatile_datagram`, `unprotect_user_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_too_small_for_header_and_extension_is_invalid`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_counts_from_the_start_of_the_rtps_header`,
`crates/rtps/src/datagram.rs::tests::a_rebuilt_message_takes_the_header_extension_with_its_own_length`

**Status:** `done` — the decoder refuses a `messageLength` below header and HeaderExtension and one that differs from the length of the message received, counted from the RTPS header. A message the receiver rebuilds after it opened protected submessages is checked as received, then gets the HeaderExtension with its own length and without the checksum (`restate_for_rebuilt_message`); kept as it was, every opened message of CoreDX 6.14.1, which sends a `messageLength`, was refused, its crypto tokens among them (OMG dds-security, Linux test host).

### §8.3.3.2 Table 8.15 HE rtpsSendTimestamp

**Spec:** §8.3.3.2, p. 40 — "The rtpsSendTimestamp indicates the time when
the RTPS message was sent by the originating Participant. … shall collected
as close as possible to the moment when the RTPS Message is sent … The time
origin … shall be the same as the one used in the Timestamp Submessage
Element".

**Repo:** `crates/rtps/src/header_extension.rs`, `crates/rtps/src/receiver_state.rs` (`apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_timestamp_is_no_source_timestamp`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`

**Status:** `done` — field encoded and read; on reception it sets `Receiver.rtpsSendTimestamp` (§8.3.7.4), not the source timestamp. ZeroDDS sends no HeaderExtension (it is optional, §8.3.3.2); the rule for the time of collection holds for a sender that writes one.

### §8.3.3.2 Table 8.15 HE uExtension4

**Spec:** §8.3.3.2, p. 40 — "The uExtension4 is undefined in this version of
the protocol."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE wExtension8

**Spec:** §8.3.3.2, p. 40 — "The wExtension8 is undefined in this version of
the protocol."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.3.2 Table 8.15 HE messageChecksum

**Spec:** §8.3.3.2, p. 40 — "The messageChecksum provides a checksum computed
over the complete of the RTPS Message, which includes the RTPS Header, the
HeaderExtension, and all Submessages that may follow."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`,
`crates/rtps/src/datagram.rs::tests::a_message_with_a_wrong_checksum_is_refused_and_counted`

**Status:** `done` — the checksum of the whole message with the field zeroed is computed (`seal_message`) and checked on receipt; a mismatch refuses the message and is counted (`refused_messages`). The runtime has no send path that writes a HeaderExtension with a checksum.

### §8.3.3.2 Table 8.15 HE parameters

**Spec:** §8.3.3.2, p. 40 — "The parameters provide an extensibility
mechanism. It enables revisions of the protocol to add information to the
HeaderExtension without breaking interoperability".

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::parameters`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_malformed_parameter_list_when_p_flag_set`,
`crates/rtps/src/header_extension.rs::tests::decode_takes_a_parameter_list_without_sentinel`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.submessageId

**Spec:** §8.3.3.3, p. 41, Table 8.16 — "submessageId: SubmessageKind —
Identifies the kind of Submessage."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::submessage_id`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_le`,
`crates/rtps/src/submessage_header.rs::tests::submessage_id_roundtrip_for_known_ids`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.flags (bit 0 = endianness)

**Spec:** §8.3.3.3, pp. 41–42 — "The first flag, the EndiannessFlag, is
present and located in the same position in all Submessages … If the
EndiannessFlag is set to FALSE, the Submessage is encoded in big-endian
format, EndiannessFlag set to TRUE means little-endian."

**Repo:** `crates/rtps/src/submessage_header.rs` (`FLAG_E_LITTLE_ENDIAN`,
`SubmessageHeader::is_little_endian`); `crates/rtps/src/datagram.rs`
(`SubmessageFrame::little_endian`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_be`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_le`

**Status:** `done`

### §8.3.3.3 Table 8.16 SubmessageHeader.submessageLength

**Spec:** §8.3.3.3, p. 42 — "In case submessageLength > 0, it is either: The
length from the start of the contents of the Submessage until the start of the
header of the next Submessage … Or else it is the remaining Message length".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end =
body_start + octets`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`

**Status:** `done`

### §8.3.3.3 submessageLength == 0: last submessage

**Spec:** §8.3.3.3, p. 42 — "In case submessageLength==0, the Submessage is
the last Submessage in the Message and extends up to the end of the
Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`SubmessageFrame::octets_elided`, `encode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`,
`crates/rtps/src/datagram.rs::tests::an_info_ts_without_a_body_is_not_the_last_submessage`,
`crates/rtps/tests/vendor_frames.rs::opendds_datagrams_re_encode_byte_for_byte`

**Status:** `done` — the exception for PAD and INFO_TS is in §9.4.5.1.3.

### §8.3.3.3 Submessage structure fixed in version 2

**Spec:** §8.3.3.3, p. 41 — "The structure of the RTPS Submessage cannot be
changed in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::WIRE_SIZE = 4`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_be`

**Status:** `done`

### §8.3.4 Receiver state reset for every message

**Spec:** §8.3.4, p. 42 — "This state is modeled as the state of an RTPS
Receiver that is reset each time a new message is processed and provides
context for the interpretation of each Submessage."

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`
creates a new `ReceiverState` per datagram)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`

**Status:** `done`

### §8.3.4 Table 8.17 initial sourceVersion

**Spec:** §8.3.4, p. 43, Table 8.17 — "sourceVersion: PROTOCOLVERSION".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_version`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — initial value PROTOCOLVERSION (2.5).

### §8.3.4 Table 8.17 initial sourceVendorId

**Spec:** §8.3.4, p. 43, Table 8.17 — "sourceVendorId: VENDORID_UNKNOWN".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_vendor_id`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — initial value VENDORID_UNKNOWN.

### §8.3.4 Table 8.17 initial sourceGuidPrefix

**Spec:** §8.3.4, p. 43, Table 8.17 — "sourceGuidPrefix: GUIDPREFIX_UNKNOWN".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `source_guid_prefix`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — initial value GUIDPREFIX_UNKNOWN.

### §8.3.4 Table 8.17 initial destGuidPrefix

**Spec:** §8.3.4, p. 43, Table 8.17 — "destGuidPrefix: GUID prefix of the
participant receiving the message".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`);
`crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.4 Table 8.17 initial unicastReplyLocatorList

**Spec:** §8.3.4, p. 43, Table 8.17 — "The list is initialized to contain a
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

**Status:** `done` — initial value per Table 8.17 (transport kind, source address, `LOCATOR_PORT_INVALID`). Every receive path goes through `receiver_walk_in` with the datagram's source and the reception clock: user data, SPDP, SEDP, WLP, TypeLookup and the security builtins. The reply side uses the list: a reader proxy with no locator of its own takes the entries of the unicast list that carry a port (which an InfoReply supplies; the initial values have `LOCATOR_PORT_INVALID`) and keeps them for later answers.

### §8.3.4 Table 8.17 initial multicastReplyLocatorList

**Spec:** §8.3.4, p. 43, Table 8.17 — "The list is initialized to contain a
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

**Status:** `done` — initial value per Table 8.17 (transport kind, `LOCATOR_ADDRESS_INVALID`, `LOCATOR_PORT_INVALID`). Every receive path goes through `receiver_walk_in` with the datagram's source and the reception clock: user data, SPDP, SEDP, WLP, TypeLookup and the security builtins. The reply side uses the list: a reader proxy with no locator of its own takes the entries of the unicast list that carry a port (which an InfoReply supplies; the initial values have `LOCATOR_PORT_INVALID`) and keeps them for later answers.

### §8.3.4 Table 8.17 initial haveTimestamp

**Spec:** §8.3.4, p. 43, Table 8.17 — "haveTimestamp: FALSE".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 initial timestamp

**Spec:** §8.3.4, p. 43, Table 8.17 — "timestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`);
`crates/rtps/src/header_extension.rs` (`HeTimestamp::TIME_INVALID`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`

**Status:** `done` — {0xffffffff, 0xffffffff}; also after InfoSource and an InfoTimestamp with the I flag.

### §8.3.4 Table 8.17 initial messageLength

**Spec:** §8.3.4, p. 43, Table 8.17 — "messageLength: MESSAGE_LENGTH_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`:
`message_length: None`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 initial messageChecksum

**Spec:** §8.3.4, p. 43, Table 8.17 — "messageChecksum: CHECKSUM_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`:
`ChecksumValue::None`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 initial rtpsSendTimestamp

**Spec:** §8.3.4, p. 43, Table 8.17 — "rtpsSendTimestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::rtps_send_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`,
`crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.4 Table 8.17 initial rtpsReceptionTimestamp

**Spec:** §8.3.4, p. 43, Table 8.17 — "rtpsReceptionTimestamp: TIME_INVALID".

**Repo:** `crates/rtps/src/receiver_state.rs` (`rtps_reception_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`

**Status:** `done` — the field starts as TIME_INVALID (`None`).

### §8.3.4 Table 8.17 initial clockSkewDetected

**Spec:** §8.3.4, p. 43, Table 8.17 — "clockSkewDetected: FALSE".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::new_state_has_default_fields`

**Status:** `done`

### §8.3.4 Table 8.17 initial parameters

**Spec:** §8.3.4, p. 43, Table 8.17 — "parameters: The list is initialized as
an empty list."

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::new`, `parameters`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_initial_source_values_and_parameters_are_those_of_table_8_17`

**Status:** `done` — initial value an empty list.

### §8.3.4.1 Rule 1: unreadable submessage header

**Spec:** §8.3.4.1, p. 43 — "If the full Submessage header cannot be read,
the rest of the Message is considered invalid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `UnexpectedEof`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_truncated_submessage_header_invalidates_the_rest`

**Status:** `done` — the rest of the message is discarded, the error is in
`ParsedDatagram::invalid_rest`; the submessages before it stay.

### §8.3.4.1 Rule 2: invalid submessageLength

**Spec:** §8.3.4.1, p. 43 — "The submessageLength field defines where the
next Submessage starts … If this field is invalid, the rest of the Message is
invalid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end >
bytes.len()` gives `UnexpectedEof`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`,
`crates/rtps/tests/fuzz_smoke.rs::fuzz_decode_datagram_does_not_panic`

**Status:** `done` — as rule 1: the rest is discarded, the submessages before it
stay.

### §8.3.4.1 Rule 3: ignore an unknown SubmessageId

**Spec:** §8.3.4.1, p. 43 — "A Submessage with an unknown SubmessageId must be
ignored and parsing must continue with the next Submessage. … SubmessageIds in
the vendor-specific range coming from a vendorId that is unknown must also be
ignored".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_unknown_submessage_without_must_understand`,
`crates/rtps/src/datagram.rs::tests::decode_marks_unknown_submessage_id_without_failing`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done`

### §8.3.4.1 Rule 3: no must-understand at the submessage level

**Spec:** §8.3.4.1, pp. 43–44 — rules 3 and 4: unknown submessages and unknown
flags are ignored; RTPS 2.5 knows must-understand only per ParameterId
(Table 9.6, p. 165).

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — an unknown submessage is skipped whatever its flags.

### §8.3.4.1 Rule 4: ignore unknown flags

**Spec:** §8.3.4.1, p. 44 — "The receiver of a Submessage should ignore unknown
flags. An implementation of RTPS 2.5 should skip all flags that are marked as
'X' (unused) in the protocol."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::unknown_flags_of_a_known_submessage_are_ignored`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — for known submessages the top three flag bits, which no submessage defines, change nothing in what is read; an unknown submessage is skipped whatever flags it carries.

### §8.3.4.1 Rule 5: submessageLength also for known ids

**Spec:** §8.3.4.1, p. 44 — "A valid submessageLength field must always be used
to find the next Submessage, even for Submessages with known IDs."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `pos = body_end`
for every submessage)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`,
`crates/rtps/tests/vendor_frames.rs::rti_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.4.1 Rule 6: a known but invalid submessage

**Spec:** §8.3.4.1, p. 44 — "A known but invalid Submessage invalidates the
rest of the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — a submessage its decoder cannot read, or whose "Validity" condition (§8.3.8) is violated, invalidates the rest of the message; the submessages before it stand (`invalid_rest`, counted). The one condition that contradicts the definition of currentGSN is under §8.3.8.6 "Heartbeat invalid: group fields".

### §8.3.4.1 Valid submessages before an invalid one

**Spec:** §8.3.4.1, pp. 43–44 — rules 1, 2 and 6 declare only "the rest of the
Message" invalid.

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — `decode_datagram` returns the submessages read as valid
before an invalid one and notes the error in `invalid_rest`; the receive paths
process them.

### §8.3.4.1 Rule 6 for submessages invalid in content

**Spec:** §8.3.4.1, p. 44 — "A known but invalid Submessage invalidates the
rest of the Message." The validity conditions per submessage are in §8.3.8
under "Validity".

**Repo:** `crates/rtps/src/submessages.rs` (decoder per submessage); `crates/rtps/src/datagram.rs`
(`ParsedDatagram::invalid_rest`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`,
`crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`,
`crates/dcps/src/runtime.rs::tests::the_empty_acknack_setting_is_off_by_default`

**Status:** `done` — the decoders of the submessages check the conditions of the “Validity”
sections of §8.3.8; a violation is a decoder error and makes the rest of the
message invalid, the submessages before it stand.
Counted, not dropped silently: `zerodds_rtps::datagram::invalid_message_rests()`,
`refused_messages()` and `skipped_must_understand_submessages()` (process-wide),
on the runtime as `DcpsRuntime::invalid_message_rests`, `refused_messages`,
`skipped_must_understand_submessages`.

### §8.3.4.1 Submessages that change the receiver state

**Spec:** §8.3.4.1, p. 44 — "In this version of the protocol, only the Header
and the Submessages HeaderExtension, InfoSource, InfoReply, InfoDestination,
and InfoTimestamp change the state of the Receiver."

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_yields_every_submessage_including_info_ones`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_ip4_sets_reply_locators`

**Status:** `done` — InfoReplyIp4 (PSM §9.4.5.15) also changes the state as
InfoReply does.

### §8.3.5 SubmessageElements

**Spec:** §8.3.5, p. 44 — "Each RTPS Submessage in turn is built from a set of
predefined atomic building blocks called SubmessageElements."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the elements are listed one
by one under §8.3.5.1–§8.3.5.19.

### §8.3.5.1 Table 8.18 GuidPrefix element

**Spec:** §8.3.5.1, p. 44, Table 8.18 — "value: GuidPrefix_t — Identifies the
GuidPrefix_t part of the GUID_t of the Entity that is the source or target of
the message."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::guid_prefix`);
`crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`,
`crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`

**Status:** `done`

### §8.3.5.1 Table 8.19 EntityId element

**Spec:** §8.3.5.1, p. 45, Table 8.19 — "value: EntityId_t — Identifies the
EntityId_t part of the GUID_t of the Entity that is the source or target of
the message."

**Repo:** `crates/rtps/src/submessages.rs` (fields `reader_id`, `writer_id`
of every entity submessage)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §8.3.5.2 Table 8.20 VendorId element

**Spec:** §8.3.5.2, p. 45, Table 8.20 — "value: VendorId_t — Identifies the
vendor of the middleware that implements the protocol."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::vendor_id`);
`crates/rtps/src/header.rs` (`RtpsHeader::vendor_id`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`

**Status:** `done`

### §8.3.5.2 VendorId assigned by the OMG

**Spec:** §8.3.5.2, p. 45 — "The following values are reserved by the
protocol: VENDORID_UNKNOWN. Other values must be assigned by the OMG."

**Repo:** —

**Tests:** —

**Status:** `open` — ZeroDDS sends `VendorId::ZERODDS = 0x01F0`
(`crates/rtps/src/wire_types.rs`); the comment there calls the value an
interim one until a VendorId has been applied for. Blocker: a hard external one, the assignment of a VendorId by the OMG; the request has been pending since 22 June. Until then ZeroDDS sends `0x01F0`, a value the OMG list does not hold (assigned up to `{0x01,0x1C}`, `{0x01,0x1D}` and `{0x01,0x1E}` reserved).

### §8.3.5.3 Table 8.21 ProtocolVersion element

**Spec:** §8.3.5.3, p. 45, Table 8.21 — "value: ProtocolVersion_t — Identifies
the major and minor version of the RTPS protocol." Special values
PROTOCOLVERSION_1_0 … PROTOCOLVERSION_2_4.

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion`);
`crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::protocol_version`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_all_aliases_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_info_source_via_datagram`

**Status:** `done`

### §8.3.5.4 Table 8.22 SequenceNumber: 64 bits signed

**Spec:** §8.3.5.4, p. 45 — "A sequence number is a 64-bit signed integer,
that can take values in the range: -2^63 <= N <= 2^63-1."

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber(i64)`,
`SequenceNumber::split`, `SequenceNumber::from_high_low`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_split_high_low`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §8.3.5.4 Sequence numbers begin at 1

**Spec:** §8.3.5.4, p. 45 — "Sequence numbers begin at 1."

**Repo:** `crates/rtps/src/writer.rs` (`BestEffortWriter`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/writer.rs::tests::writer_starts_at_sequence_number_one`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::new_change_assigns_monotonic_sn`

**Status:** `done`

### §8.3.5.4 SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.5.4, p. 46 — "The protocol reserves the following value:
SEQUENCENUMBER_UNKNOWN".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`

**Status:** `done`

### §8.3.5.5 Table 8.23 SequenceNumberSet base/set

**Spec:** §8.3.5.5, p. 46, Table 8.23 — "base: SequenceNumber_t — Identifies
the first sequence number in the set. set: SequenceNumber_t[*] — A set of
sequence numbers, each verifying that: base <= element(set) <= base+255".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet`,
`from_missing`, `iter_set`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_iter_matches_missing`,
`crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_roundtrip_le`

**Status:** `done`

### §8.3.5.5 Validity of a SequenceNumberSet

**Spec:** §8.3.5.5, p. 46 — "a valid SequenceNumberSet must verify that:
maximum(SequenceNumberSet) - minimum(SequenceNumberSet) < 256,
minimum(SequenceNumberSet) >= 1".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`:
`numBits <= 256`, `bitmapBase >= 1`; `from_missing` caps the window at 256)

**Tests:** `crates/rtps/tests/fuzz_smoke.rs::fuzz_sequence_number_set_read_from_does_not_panic`,
`crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_wire_size_matches_encoded`,
`crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/tests/vendor_frames.rs::the_empty_acknack_of_opendds_and_rti_is_invalid_by_default`,
`crates/rtps/tests/vendor_frames.rs::opendds_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`,
`crates/rtps/src/submessages.rs::validity_tests::the_empty_set_with_base_zero_is_accepted_only_on_request`

**Status:** `done` — the span is limited to 256, the decoder checks `minimum >= 1`.
Vendor form: OpenDDS 3.34.0 and RTI Connext 7.7.0 send `readerSNState` `0/0:`
(bitmapBase 0, numBits 0) as the ACKNACK of a reader without samples (measured in
`tools/wire-capture/rtps-sessions`: 40 and 8 ACKNACKs). Default: invalid. The
setting `wire.accept_empty_acknack_base_zero` (env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, default `false`) accepts exactly this
empty set in an ACKNACK, for the process.

### §8.3.5.6 Table 8.24 FragmentNumber

**Spec:** §8.3.5.6, p. 46 — "A fragment number is a 32-bit unsigned integer …
Fragment numbers start at 1."

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`);
`crates/rtps/src/reliable_writer.rs` (fragmentation from number 1)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`

**Status:** `done`

### §8.3.5.7 Table 8.25 FragmentNumberSet

**Spec:** §8.3.5.7, p. 46, Table 8.25 — "base: FragmentNumber_t … set:
FragmentNumber_t[*] — A set of fragment numbers, each verifying that: base <=
element(set) <= base+255". Valid only with "maximum - minimum < 256,
minimum >= 1".

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet`,
`from_missing`, `read_from`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::frag_set_roundtrip_le`,
`crates/rtps/tests/fuzz_smoke.rs::fuzz_fragment_number_set_read_from_does_not_panic`,
`crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`

**Status:** `done` — span limited to 256, `bitmapBase >= 1` is checked.

### §8.3.5.8 Table 8.26 Timestamp

**Spec:** §8.3.5.8, p. 47 — "Timestamp is used to represent time. The
representation should be capable of having a resolution of nano-seconds or
better." Special values TIME_ZERO, TIME_INVALID, TIME_INFINITE.

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`);
`crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`); `crates/dcps/src/time.rs`

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::validity_tests::time_t_seconds_are_unsigned`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done` — resolution 2⁻³² s, unsigned seconds, TIME_INVALID and TIME_INFINITE as constants.

### §8.3.5.9 Table 8.27/8.28 ParameterList and Parameter

**Spec:** §8.3.5.9, p. 47, Tables 8.27 and 8.28 — "parameter: Parameter[*]";
per parameter "parameterId: ParameterId_t", "length: short", "value:
octet[length]".

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList`, `Parameter`,
`to_bytes`, `from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::single_parameter_encodes_id_length_value`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_multiple_parameters`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_be_endianness`

**Status:** `done`

### §8.3.5.9 At most 2^16 ParameterIds

**Spec:** §8.3.5.9, p. 47 — "There shall be no more than 2^16 possible values
of the ParameterId_t parameterId."

**Repo:** `crates/rtps/src/parameter_list.rs` (`Parameter::id: u16`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::single_parameter_encodes_id_length_value`

**Status:** `done`

### §8.3.5.9 2^15 values for protocol-defined parameters

**Spec:** §8.3.5.9, p. 47 — "A range of 2^15 values is reserved for
protocol-defined parameters. All the parameter_id values defined by the 2.5
version of the protocol and all future revisions of the same major version
must use values in this range."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid`, `is_standard_pid`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`,
`crates/rtps/src/parameter_list.rs::tests::is_standard_pid_unknown_pid_returns_false`

**Status:** `done`

### §8.3.5.9 2^15 values for vendor-defined parameters

**Spec:** §8.3.5.9, p. 47 — "A range of 2^15 values is reserved for
vendor-defined parameters."

**Repo:** `crates/rtps/src/parameter_list.rs` (`VENDOR_SPECIFIC_BIT`,
`pid::SHM_LOCATOR`, `pid::ZERODDS_TYPE_ID`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendors_0x8002_is_not_read_as_a_type_identifier`,
`crates/rtps/src/publication_data.rs::tests::a_zerodds_0x8002_is_read_and_the_announcement_states_the_vendor`

**Status:** `done`

### §8.3.5.9 Parameter length at most 2^16 octets

**Spec:** §8.3.5.9, p. 47 — "The maximum length of any parameter is limited to
2^16 octets."

**Repo:** `crates/rtps/src/parameter_list.rs` (`length` as `u16`)

**Tests:** `crates/rtps/tests/user_data_propagation.rs::user_data_large_payload_32kib`

**Status:** `done`

### §8.3.5.10 Table 8.29 Count

**Spec:** §8.3.5.10, pp. 47–48 — "Count is used by several Submessages and
enables a receiver to detect duplicates of the same Submessage."

**Repo:** `crates/rtps/src/submessages.rs` (`count` fields)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`

**Status:** `done` — the element exists; its use by the receiver is listed
under §8.3.8.1 and §8.3.8.6.

### §8.3.5.11 Table 8.30 ChangeCount

**Spec:** §8.3.5.11, p. 48 — "ChangeCount is used in the Gap Submessage and
enables the sender to indicate the number of changes within the Gap that
belong to a certain category."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — in the GAP submessage 64 bits (eight octets, high and low).

### §8.3.5.12 Table 8.31 Checksum

**Spec:** §8.3.5.12, p. 48, Table 8.31 — "value: Checksum32_t, or
Checksum64_t, or Checksum128_t — A checksum of the RTPS Message, including the
RTPS Header."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue`,
`ChecksumKind`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_crc32c_le`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_crc64_be`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_md5_le`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`

**Status:** `done` — the three formats, the CRCs in big endian.

### §8.3.5.12 Checksum defined per transport PSM

**Spec:** §8.3.5.12, p. 48 — "the Checksum may be omitted from the
HeaderExtension. The specific behavior shall be defined for each Transport
PSM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a task for the PSMs; the UDP/IP PSM defines
the checksum in §9.4.2.15.

### §8.3.5.13 Table 8.32 MessageLength

**Spec:** §8.3.5.13, p. 48 — "MessageLength is used in the HeaderExtension
Submessage and enables the sender to indicate the length of the RTPS
message."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.14 Table 8.33 UExtension4

**Spec:** §8.3.5.14, pp. 48–49 — "UExtension4 is used in the HeaderExtension
Submessage and enables the sender to add 4 octets of information to the
header."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.15 Table 8.34 WExtension8

**Spec:** §8.3.5.15, p. 49 — "WExtension8 is used in the HeaderExtension
Submessage and enables the sender to add 8 octets of information to the
header."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.5.16 Table 8.35 LocatorList

**Spec:** §8.3.5.16, p. 49 — "LocatorList is used to specify a list of
locators."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::unicast_locators`,
`multicast_locators`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_info_reply_with_multicast_via_datagram`

**Status:** `done`

### §8.3.5.17 Table 8.36 SerializedData

**Spec:** §8.3.5.17, p. 49 — "The RTPS protocol does not interpret the
serialized data-stream. Therefore, it is represented as opaque data."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::serialized_payload`);
`crates/rtps/src/payload.rs` (`Payload`)

**Tests:** `crates/rtps/src/datagram.rs::tests::the_shared_decode_gives_ranges_of_the_datagram`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_roundtrip_bit_identical`

**Status:** `done`

### §8.3.5.18 Table 8.37 SerializedDataFragment

**Spec:** §8.3.5.18, p. 49 — "SerializedDataFragment contains the serialized
representation of a data-object that has been fragmented. … the RTPS protocol
does not interpret the fragmented serialized data-stream."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::serialized_payload`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_frag_preserves_flags_and_payload`

**Status:** `done`

### §8.3.5.19 Table 8.38 GroupDigest

**Spec:** §8.3.5.19, pp. 49–50 — "GroupDigest is used to communicate a set of
EntityId_t in a compact manner."

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §8.3.6.1 Header: Purpose

**Spec:** §8.3.6.1, p. 50 — "The Header is used to identify the message as
belonging to the RTPS protocol, to identify the version of the RTPS protocol
used, and to provide context information".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose; the fields are listed
under §8.3.3.1.

### §8.3.6.2 Header: Content

**Spec:** §8.3.6.2, p. 50 — "The structure of the Header can only be changed
if the major version of the protocol is also changed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — repeats §8.3.3.1; listed there as the item
"Header structure fixed in version 2".

### §8.3.6.3 Header invalid: too few octets

**Spec:** §8.3.6.3, p. 50 — "The Message has less than the required number
of octets to contain a full Header. The number required is defined by the
PSM."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::from_bytes`:
`UnexpectedEof` below 20 octets)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_datagram_shorter_than_the_header`,
`crates/rtps/src/header.rs::tests::header_decode_rejects_truncated_input`

**Status:** `done`

### §8.3.6.3 Header invalid: protocol ≠ PROTOCOL_RTPS

**Spec:** §8.3.6.3, p. 50 — "Its protocol value does not match the value of
PROTOCOL_RTPS."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::from_bytes`:
`InvalidMagic`)

**Tests:** `crates/rtps/src/header.rs::tests::header_decode_rejects_invalid_magic`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_invalid_magic`

**Status:** `done`

### §8.3.6.3 Header invalid: major version too high

**Spec:** §8.3.6.3, p. 50 — "The major protocol version is larger than the
major protocol version supported by the implementation."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram_into`: comparison
with `ProtocolVersion::CURRENT.major`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_major_version_above_supported`,
`crates/rtps/src/datagram.rs::tests::decode_accepts_minor_version_above_supported`

**Status:** `done`

### §8.3.6.4 Receiver.sourceGuidPrefix = Header.guidPrefix

**Spec:** §8.3.6.4, p. 50 — "Receiver.sourceGuidPrefix = Header.guidPrefix".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::init_from_header_overrides_source_fields`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_without_info_source_attributes_to_header_prefix`

**Status:** `done`

### §8.3.6.4 Receiver.sourceVersion = Header.version

**Spec:** §8.3.6.4, p. 50 — "Receiver.sourceVersion = Header.version".

**Repo:** `crates/rtps/src/receiver_state.rs` (`init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_header_sets_the_source_and_clears_have_timestamp`

**Status:** `done` — the header sets `sourceVersion`, `sourceVendorId` and `sourceGuidPrefix`.

### §8.3.6.4 Receiver.sourceVendorId = Header.vendorId

**Spec:** §8.3.6.4, p. 50 — "Receiver.sourceVendorId = Header.vendorId".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::init_from_header_overrides_source_fields`

**Status:** `done`

### §8.3.6.4 Receiver.haveTimestamp = false

**Spec:** §8.3.6.4, p. 50 — "Receiver.haveTimestamp = false".

**Repo:** `crates/rtps/src/receiver_state.rs` (`init_from_header`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_header_sets_the_source_and_clears_have_timestamp`

**Status:** `done` — the header sets `haveTimestamp = false`.

### §8.3.6.5 Header: Logical Interpretation

**Spec:** §8.3.6.5, p. 50 — "None".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §8.3.7.1 HeaderExtension immediately after the header

**Spec:** §8.3.7.1, p. 50 — "This submessage, if present, shall appear
immediately after the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.2 HeaderExtension: Content

**Spec:** §8.3.7.2, p. 50 — "The elements that form the structure of the
HeaderExtension were described in 8.3.3.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; items under §8.3.3.2.

### §8.3.7.3 HE invalid: not directly after the header

**Spec:** §8.3.7.3, p. 51 — "The submessage is present but does not
immediately follow the RTPS Header."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_rejects_header_extension_after_data_submessage`

**Status:** `done` — a HeaderExtension behind another submessage is invalid and the rest of the message with it; the submessages before it stand.

### §8.3.7.3 HE invalid: submessageLength too small for the fields

**Spec:** §8.3.7.3, p. 51 — "The submessageLength in the Submessage header is
too small to fit the fields that are present as indicated by the submessage
flags."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_uextension4_body`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_md5_checksum_body`,
`crates/rtps/src/header_extension.rs::tests::decode_rejects_truncated_body_before_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.3 HE invalid: messageLength too small

**Spec:** §8.3.7.3, p. 51 — "The messageLength is too small to fit the RTPS
Header and the HeaderExtension."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_too_small_for_header_and_extension_is_invalid`

**Status:** `done` — a `messageLength` below header and HeaderExtension (28 octets) makes the message invalid.

### §8.3.7.3 HE invalid: malformed parameters

**Spec:** §8.3.7.3, p. 51 — "The parameters are malformed."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`
through `ParameterList::from_bytes`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_rejects_malformed_parameter_list_when_p_flag_set`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 LengthFlag: Receiver.messageLength

**Spec:** §8.3.7.4, p. 51 — "IF ( LengthFlag ) { Receiver.messageLength =
HeaderExtension.messageLength }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 TimestampFlag: rtpsSendTimestamp, rtpsReceptionTimestamp, clockSkewDetected

**Spec:** §8.3.7.4, p. 51 — "IF ( TimestampFlag ) { Receiver.rtpsSendTimestamp
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

**Status:** `done` — every receive path sets `rtpsReceptionTimestamp` (wall clock) for a HeaderExtension with a timestamp. The spec gives `CLOCK_SKEW_DETECTED` no threshold; it is the setting `wire.clock_skew_threshold_s` (seconds). Default 0 = off: `clockSkewDetected` then stays FALSE. There is no default threshold.

### §8.3.7.4 uExtensionFlag: Receiver.uExtension4

**Spec:** §8.3.7.4, p. 51 — "IF ( uExtensionFlag ) { Receiver.uExtension4 =
HeaderExtension.uExtension4 }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.7.4 wExtensionFlag: Receiver.wExtension8

**Spec:** §8.3.7.4, p. 51 — "IF ( wExtensionFlag ) { Receiver.wExtension8 =
HeaderExtension.wExtension8 }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`

**Status:** `done`

### §8.3.7.4 ChecksumFlags: Receiver.messageChecksum

**Spec:** §8.3.7.4, p. 51 — "IF ( ChecksumFlags != 00 ) {
Receiver.messageChecksum = HeaderExtension.messageChecksum }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_updates_fields`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.4 ParametersFlag: Receiver.parameters

**Spec:** §8.3.7.4, p. 51 — "IF ( ParamatersFlag) { Receiver.parameters =
HeaderExtension.parameters }".

**Repo:** `crates/rtps/src/receiver_state.rs`
(`ReceiverState::apply_header_extension`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_header_extension_with_parameters_sets_pl`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §8.3.7.5 messageLength from the start of the RTPS header

**Spec:** §8.3.7.5, p. 51 — "The value of the messageLength shall indicate the
length of the entire RTPS Message starting from the beginning of the RTPS
Header."

**Repo:** `crates/rtps/src/datagram.rs`, `crates/rtps/src/header_extension.rs` (`validate_message_length`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_header_extension_message_length_counts_from_the_start_of_the_rtps_header`

**Status:** `done` — the length is counted from the first octet of the RTPS Header and compared with the message received.

### §8.3.7.5 rtpsSendTimestamp and rtpsReceptionTimestamp

**Spec:** §8.3.7.5, p. 51 — "The rtpsSendTimestamp represents the time the
RTPS message was sent. It is measured using the clock of the sending
Participant. The rtpsReceptionTimestamp represents the time when the RTPS
message was received."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition; the assignments are listed
under §8.3.7.4 TimestampFlag.

### §8.3.7.5 CLOCK_SKEW_DETECTED

**Spec:** §8.3.7.5, pp. 51–52 — "The function CLOCK_SKEW_DETECTED() represents
an implementation-specific function used to determine whether the sending and
receiving participant clocks are not synchronized or have too large a skew."

**Repo:** `crates/rtps/src/receiver_state.rs` (`note_reception`, `note_clock_skew`); `crates/dcps/src/runtime.rs` (`receive_context`, `wire.clock_skew_threshold_s`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_reception_time_is_kept_and_checked_against_the_send_time`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`,
`crates/dcps/tests/runtime_config_layering.rs::the_clock_skew_threshold_is_a_setting_and_off_by_default`

**Status:** `done` — the spec calls the function implementation specific; ZeroDDS compares reception and send time against the threshold `wire.clock_skew_threshold_s` (default 0 = off, no default threshold).

### §8.3.7.5 messageChecksum with the checksum field zeroed

**Spec:** §8.3.7.5, p. 52 — "It shall be computed over the entire RTPS
Message. For the purpose of computing the checksum, the value of the
messageChecksum field in the RTPS HeaderExtension shall be set to zero."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`,
`crates/rtps/src/header_extension.rs::tests::the_checksum_is_big_endian_in_a_little_endian_header_extension`,
`crates/rtps/src/datagram.rs::tests::a_message_with_a_wrong_checksum_is_refused_and_counted`

**Status:** `done` — `message_checksum` computes over the whole message with the field zeroed.

### §8.3.7.5 uExtension4, wExtension8, parameters without meaning

**Spec:** §8.3.7.5, p. 52 — "The uExtension4 and wExtension8 may be useful to
communicate additional data on the header in a future version … This version
of the specification does not define any parameters in the HeaderExtension."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement on this version.

### §8.3.8 Entity and interpreter submessages

**Spec:** §8.3.8, pp. 52–53, Table 8.39 — division into entity submessages
(Data, DataFrag, Heartbeat, HeartbeatFrag, Gap, AckNack, NackFrag) and
interpreter submessages (HeaderExtension, InfoSource, InfoDestination,
InfoReply, InfoTimestamp, Pad); outline of every description.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the submessages are listed one by
one under §8.3.8.1–§8.3.8.13.

### §8.3.8.1 AckNack: Purpose

**Spec:** §8.3.8.1, p. 54 — "This Submessage is used to communicate the state
of a Reader to a Writer. … This Submessage can be used to do both positive and
negative acknowledgments."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose; the contents follow.

### §8.3.8.1 Table 8.40 AckNack EndiannessFlag

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::write_body`,
`AckNackSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/tests/vendor_frames.rs::fastdds_datagrams_re_encode_byte_for_byte`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack FinalFlag

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "FinalFlag … Indicates to the Writer
whether a response is mandatory."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::final_flag`,
`ACKNACK_FLAG_FINAL`); `crates/rtps/src/reliable_reader.rs`
(`build_acknack_datagram`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_with_final_flag`,
`crates/rtps/src/datagram.rs::tests::decode_acknack_preserves_final_flag`

**Status:** `done` — the writer's duty to respond is listed under §9.4.5.3.

### §8.3.8.1 Table 8.40 AckNack readerId

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "readerId: EntityId — Identifies the
Reader entity that acknowledges receipt of certain sequence numbers and/or
requests to receive certain sequence numbers."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack writerId

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "writerId: EntityId — Identifies the
Writer entity that is the target of the AckNack message."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::writer_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: `rt.writer_slot(ack.writer_id)`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack readerSNState

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "All sequence numbers up to the one
prior to readerSNState.base are confirmed as received by the reader. The
sequence numbers that appear in the set indicate missing sequence numbers on
the reader side."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::reader_sn_state`);
`crates/rtps/src/reliable_writer.rs` (`handle_acknack_set_at`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_updates_proxy_state`,
`crates/rtps/src/reliable_writer.rs::tests::handle_acknack_with_lower_base_leaves_requested`

**Status:** `done`

### §8.3.8.1 Table 8.40 AckNack count

**Spec:** §8.3.8.1, p. 54, Table 8.40 — "count: Count — A counter that is
incremented each time a new AckNack message is sent."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`build_acknack_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::the_counts_of_acknack_and_nack_frag_rise_with_each_message`

**Status:** `done` — successive ACKNACKs carry the counts 1, 2, 3.

### §8.3.8.1 AckNack invalid: submessageLength too small

**Spec:** §8.3.8.1, p. 54 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.1 AckNack invalid: invalid readerSNState

**Spec:** §8.3.8.1, p. 54 — "readerSNState is invalid (as defined in Section
8.3.5.5)."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::tests::seqnumset_rejects_num_bits_above_256`,
`crates/rtps/tests/vendor_frames.rs::the_empty_acknack_of_opendds_and_rti_is_invalid_by_default`,
`crates/rtps/tests/vendor_frames.rs::opendds_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`,
`crates/rtps/src/submessages.rs::validity_tests::the_empty_set_with_base_zero_is_accepted_only_on_request`

**Status:** `done` — `numBits > 256` and `bitmapBase < 1` are refused, with the rest of the
message.
Vendor form: OpenDDS 3.34.0 and RTI Connext 7.7.0 send `readerSNState` `0/0:`
(bitmapBase 0, numBits 0) as the ACKNACK of a reader without samples (measured in
`tools/wire-capture/rtps-sessions`: 40 and 8 ACKNACKs). Default: invalid. The
setting `wire.accept_empty_acknack_base_zero` (env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, default `false`) accepts exactly this
empty set in an ACKNACK, for the process.

### §8.3.8.1 AckNack: writerGUID = {destGuidPrefix, writerId}

**Spec:** §8.3.8.1, p. 54 — "writerGUID = { Receiver.destGuidPrefix,
AckNack.writerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: lookup
by `ack.writer_id`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.1 AckNack: readerGUID = {sourceGuidPrefix, readerId}

**Spec:** §8.3.8.1, p. 55 — "readerGUID = { Receiver.sourceGuidPrefix,
AckNack.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`Guid::new(src_prefix, ack.reader_id)`); `crates/rtps/src/reliable_writer.rs`
(`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_unknown_source_counts_but_noops`,
`crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.3.8.1 AckNack: positive acknowledgement up to base − 1

**Spec:** §8.3.8.1, p. 55 — "The Submessage acknowledges all sequence numbers
up to and including the one just before the lowest sequence number in the
SequenceNumberSet (that is readerSNState.base -1)."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`:
`acked_changes_set(base)`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::acked_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`,
`crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_follows_the_slowest_reader_proxy`

**Status:** `done`

### §8.3.8.1 AckNack: negative acknowledgement of the set

**Spec:** §8.3.8.1, p. 55 — "The Submessage negatively-acknowledges (requests)
the sequence numbers that appear explicitly in the set."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`:
`requested_changes_set`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.3.8.2 Data: Purpose

**Spec:** §8.3.8.2, p. 55 — "The Submessage notifies the RTPS Reader of a
change to a data-object belonging to the RTPS Writer."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.2 Table 8.41 Data EndiannessFlag

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::write_body`,
`DataSubmessage::read_body_with_flags`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_be_with_empty_payload`,
`crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data InlineQosFlag

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "InlineQosFlag … Indicates to the
Reader the presence of a ParameterList containing QoS parameters that should
be used to interpret the message."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_INLINE_QOS`,
`DataSubmessage::inline_qos`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/src/submessages.rs::tests::a_pid_ignore_in_the_inline_qos_does_not_move_the_payload`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data DataFlag

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "DataFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_DATA`,
`DataSubmessage::data_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/src/reliable_reader.rs::tests::alive_data_yields_alive_changekind`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data KeyFlag

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "KeyFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
key of the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_KEY`,
`DataSubmessage::key_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_key_flag_roundtrip`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_with_a_key_sets_k_and_carries_it`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data NonStandardPayloadFlag

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "NonStandardPayloadFlag … Indicates to
the Reader that the serializedPayload submessage element is not formatted
according to Section 10."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_NON_STANDARD`,
`DataSubmessage::non_standard_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data readerId

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "readerId: EntityId — Identifies the
RTPS Reader entity that is being informed of the change to the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::reader_id`)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_drops_data_addressed_to_other_reader`,
`crates/rtps/src/reader.rs::tests::reader_delivers_data_addressed_to_us`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data writerId

**Spec:** §8.3.8.2, p. 55, Table 8.41 — "writerId: EntityId — Identifies the
RTPS Writer entity that made the change to the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::writer_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::mismatched_writer_id_is_counted`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data writerSN

**Spec:** §8.3.8.2, p. 56, Table 8.41 — "writerSN: SequenceNumber — Uniquely
identifies the change and the relative order for all changes made by the
RTPS Writer … Each change gets a consecutive sequence number."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::writer_sn`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::write`)

**Tests:** `crates/rtps/src/writer.rs::tests::writer_three_writes_have_increasing_sn_in_decoded`,
`crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data inlineQos

**Spec:** §8.3.8.2, p. 56, Table 8.41 — "inlineQos: ParameterList — Present
only if the InlineQosFlag is set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`:
ParameterList only with Q)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_survives_data_submessage_roundtrip`

**Status:** `done`

### §8.3.8.2 Table 8.41 Data serializedPayload

**Spec:** §8.3.8.2, p. 56, Table 8.41 — "serializedPayload: SerializedPayload
— Present only if either the DataFlag or the KeyFlag are set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::serialized_payload`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::lifecycle_without_a_key_clears_k_and_sends_no_payload`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_empty_payload_decodes`

**Status:** `done`

### §8.3.8.2 Data invalid: submessageLength too small

**Spec:** §8.3.8.2, p. 56 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.2 Data invalid: writerSN ≤ 0 or SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.8.2, p. 56 — "writerSN.value is not strictly positive (1, 2,
...) or is SEQUENCENUMBER_UNKNOWN."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — a `writerSN` below 1 and SEQUENCENUMBER_UNKNOWN ({-1, 0}) are refused;
the submessage is invalid and the rest of the message with it.

### §8.3.8.2 Data invalid: invalid inlineQos

**Spec:** §8.3.8.2, p. 56 — "inlineQos is invalid."

**Repo:** `crates/rtps/src/parameter_list.rs`
(`ParameterList::from_bytes_with_consumed`); `crates/rtps/src/datagram.rs`
(`validate_must_understand_in_data_pipeline`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::decode_rejects_truncated_value`,
`crates/rtps/src/datagram.rs::tests::a_data_with_an_unknown_must_understand_pid_is_skipped`,
`crates/rtps/src/datagram.rs::tests::the_submessages_before_an_invalid_one_stand`

**Status:** `done` — a malformed ParameterList makes the DATA invalid and
discards the rest of the message, the submessages before it stay (§8.3.4.1 rule
6); an unknown must-understand PID skips only the DATA (Table 9.6).

### §8.3.8.2 Data: DataFlag without KeyFlag is the value

**Spec:** §8.3.8.2, p. 56 — "If the DataFlag is set and the KeyFlag is not
set, the serializedPayload element is interpreted as the value of the
data-object."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`classify_change_kind`,
`handle_data_into`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::alive_data_yields_alive_changekind`,
`crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`

**Status:** `done`

### §8.3.8.2 Data: KeyFlag without DataFlag is the key

**Spec:** §8.3.8.2, p. 56 — "If the KeyFlag is set and the DataFlag is not
set, the serializedPayload element is interpreted as the value of the key
that identifies the registered instance of the data-object."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`: a
key-only DATA without lifecycle status as a registration)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::key_only_alive_registration_is_acked_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::dispose_data_yields_not_alive_disposed`

**Status:** `done`

### §8.3.8.2 Data: inline QoS overrides the writer's QoS

**Spec:** §8.3.8.2, p. 56 — "If the InlineQosFlag is set, the inlineQos
element contains QoS values that override those of the RTPS Writer and should
be used to process the update. For a complete list of possible in-line QoS
parameters, see Table 8.85."

**Repo:** `crates/rtps/src/inline_qos.rs` (`InlineQosOverride`); `crates/rtps/src/reliable_reader.rs` (`DeliveredSample::inline_qos_override`);
`crates/dcps/src/runtime.rs` (`sample_strength`, `sample_lifespan_nanos`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_data_hands_on_the_in_line_strength_and_lifespan`,
`crates/dcps/src/runtime.rs::tests::the_in_line_strength_and_lifespan_of_a_sample_override_the_writers`

**Status:** `done` — OWNERSHIP_STRENGTH and LIFESPAN that a DATA carries in-line apply to that sample instead of the values the writer announced. The other policies of Table 8.85 do not act on a single sample (they enter the match: DURABILITY, RELIABILITY, PRESENTATION, PARTITION, DEADLINE, LATENCY_BUDGET, DESTINATION_ORDER, LIVELINESS), TRANSPORT_PRIORITY is the sender's.

### §8.3.8.2 Data: NonStandardPayloadFlag informational only

**Spec:** §8.3.8.2, p. 56 — "This flag is informational. It indicates that
the SerializedPayload has been transformed as described in another
specification. For example, this flag should be set when the
SerializedPayload is transformed as described in the DDS-Security
specification."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::non_standard_flag`
is read, not interpreted)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done` — whether ZeroDDS sets the flag on payloads transformed by
DDS-Security was not checked.

### §8.3.8.2 Data: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.2, p. 56 — "writerGUID = { Receiver.sourceGuidPrefix,
Data.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`proxy_index_by_writer`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: `src_prefix` from
`receiver_walk`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`,
`crates/discovery/tests/opensplice_infosrc_regression.rs::opensplice_bundled_infosrc_datagrams_attribute_to_the_real_source`

**Status:** `done`

### §8.3.8.2 Data: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.2, p. 56 — "readerGUID = { Receiver.destGuidPrefix,
Data.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`rt.reader_slot(d.reader_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.2 Data: readerId ENTITYID_UNKNOWN

**Spec:** §8.3.8.2, p. 56 — "The Data.readerId can be ENTITYID_UNKNOWN, in
which case the Data applies to all Readers of that writerGUID within the
Participant identified by the GuidPrefix_t Receiver.destGuidPrefix."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: with
`EntityId::UNKNOWN` to every reader slot, filtered by the writer proxy)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_accepts_unknown_wildcard_target`

**Status:** `done` — the restriction to the participant of `destGuidPrefix`
depends on the item under §8.3.8.8.

### §8.3.8.3 DataFrag: Purpose

**Spec:** §8.3.8.3, p. 57 — "The DataFrag Submessage extends the Data
Submessage by enabling the serializedData to be fragmented and sent as
multiple DataFrag Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.3 Table 8.42 DataFrag EndiannessFlag

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::write_body`,
`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag InlineQosFlag

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "InlineQosFlag … Indicates to the
Reader the presence of a ParameterList containing QoS parameters".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_INLINE_QOS`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`,
`crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag NonStandardPayloadFlag

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "NonStandardPayloadFlag … Indicates to
the Reader that the serializedPayload submessage element is not formatted
according to Section 10."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_NON_STANDARD`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag KeyFlag

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "KeyFlag … Indicates to the Reader
that the dataPayload submessage element contains the serialized value of the
key of the data-object."

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_KEY`,
`DataFragSubmessage::key_flag`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_data_frag_preserves_flags_and_payload`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag readerId

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "readerId: EntityId — Identifies the
RTPS Reader entity that is being informed of the change".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag writerId

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "writerId: EntityId — Identifies the
RTPS Writer entity that made the change".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::writer_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag writerSN

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "writerSN: SequenceNumber — Uniquely
identifies the change and the relative order for all changes made by the
RTPS Writer".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::writer_sn`);
`crates/rtps/src/fragment_assembler.rs` (buffer per `writer_sn`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::incomplete_sns_enumerates_in_order`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentStartingNum

**Spec:** §8.3.8.3, p. 57, Table 8.42 — "fragmentStartingNum: FragmentNumber —
Indicates the starting fragment for the series of fragments in serializedData.
Fragment numbering starts with number 1."

**Repo:** `crates/rtps/src/submessages.rs`
(`DataFragSubmessage::fragment_starting_num`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragment_index_zero_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::two_fragments_complete_in_order`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentsInSubmessage

**Spec:** §8.3.8.3, p. 58, Table 8.42 — "fragmentsInSubmessage: ushort — The
number of consecutive fragments contained in this Submessage, starting at
fragmentStartingNum."

**Repo:** `crates/rtps/src/submessages.rs`
(`DataFragSubmessage::fragments_in_submessage`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_all_full`,
`crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_including_last_with_tail`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag dataSize

**Spec:** §8.3.8.3, p. 58, Table 8.42 — "dataSize: ulong — The total size in
bytes of the original data before fragmentation."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::sample_size`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::inconsistent_sample_size_drops_fragment`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag fragmentSize

**Spec:** §8.3.8.3, p. 58, Table 8.42 — "fragmentSize: ushort — The size of an
individual fragment in bytes. The maximum fragment size equals 64K."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::fragment_size`,
type `u16`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragment_size_zero_dropped`,
`crates/rtps/src/submessages.rs::tests::data_frag_last_fragment_shorter_than_fragment_size`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag inlineQos

**Spec:** §8.3.8.3, p. 58, Table 8.42 — "inlineQos: ParameterList — Present
only if the InlineQosFlag is set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::inline_qos`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/rtps/src/submessages.rs::tests::data_frag_ignored_parameter_does_not_become_payload`

**Status:** `done`

### §8.3.8.3 Table 8.42 DataFrag serializedPayload

**Spec:** §8.3.8.3, p. 58, Table 8.42 — "A consecutive series of fragments,
starting at fragmentStartingNum for a total of fragmentsInSubmessage. …
If the KeyFlag is set, then it contains a consecutive set of fragments of the
key".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::serialized_payload`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::last_fragment_shorter_than_fragment_size`

**Status:** `done`

### §8.3.8.3 DataFrag invalid: submessageLength too small

**Spec:** §8.3.8.3, p. 58 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.3 DataFrag invalid: writerSN ≤ 0 or SEQUENCENUMBER_UNKNOWN

**Spec:** §8.3.8.3, p. 58 — "writerSN.value is not strictly positive (1, 2,
...) or is SEQUENCENUMBER_UNKNOWN."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`

**Status:** `done` — as for DATA.

### §8.3.8.3 DataFrag invalid: fragmentStartingNum out of range

**Spec:** §8.3.8.3, p. 58 — "fragmentStartingNum.value is not strictly
positive (1, 2, ...) or exceeds the total number of fragments".

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`FragmentIndexZero`, `FragmentIndexOutOfRange`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/fragment_assembler.rs::tests::fragment_index_zero_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::fragment_index_out_of_range_dropped`

**Status:** `done` — a `fragmentStartingNum` below 1 or above the number of fragments
(`dataSize / fragmentSize`, rounded up) makes the submessage and the rest of the
message invalid.

### §8.3.8.3 DataFrag invalid: fragmentSize > dataSize

**Spec:** §8.3.8.3, p. 58 — "fragmentSize exceeds dataSize."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`

**Status:** `done` — a `fragmentSize` above `dataSize` and a `fragmentSize` of 0 are refused.

### §8.3.8.3 DataFrag invalid: serializedData too large

**Spec:** §8.3.8.3, p. 58 — "The size of serializedData exceeds
fragmentsInSubmessage * fragmentSize."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`);
`crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`PayloadSizeMismatch`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_validity_rules`,
`crates/rtps/src/fragment_assembler.rs::tests::payload_size_mismatch_dropped`,
`crates/rtps/src/fragment_assembler.rs::tests::bundled_fragments_payload_size_mismatch_rejected`

**Status:** `done` — more octets than `fragmentsInSubmessage * fragmentSize` make the
submessage invalid. The last fragment of the sample is as long as the sample
leaves, rounded up to 4 octets: Cyclone DDS and Fast DDS pad it to the alignment
of the submessage (72 octets for 71, measured).

### §8.3.8.3 DataFrag invalid: invalid inlineQos

**Spec:** §8.3.8.3, p. 58 — "inlineQos is invalid."

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_frag_with_an_invalid_inline_qos_is_invalid`

**Status:** `done` — a parameter list that runs past the body or has no sentinel makes the submessage invalid.

### §8.3.8.3 DataFrag: reassembly, offset and number of fragments

**Spec:** §8.3.8.3, pp. 58–59 — offset "(fragmentStartingNum - 1) *
fragmentSize"; "The data is fully re-assembled when all fragments have been
received. The total number of fragments to expect equals: (dataSize /
fragmentSize) + ((dataSize % fragmentSize) ? 1 : 0)".

**Repo:** `crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`:
`byte_start`, `total_fragments = sample_size.div_ceil(fragment_size)`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::missing_fragments_enumerates_gaps`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_10_percent_loss`

**Status:** `done`

### §8.3.8.3 DataFrag: once reassembled, interpreted as Data

**Spec:** §8.3.8.3, p. 58 — "Once the serializedData is re-assembled by the
RTPS Reader, the interpretation of the DataFrag Submessages is identical to
that of the Data Submessage."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_fragmented_dispose_is_delivered_as_a_dispose`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.3.8.3 DataFrag: fragment size from the smallest transport

**Spec:** §8.3.8.3, p. 59 — "An RTPS Writer will select fragmentSize based on
the smallest message size supported across all underlying transports. …
the RTPS Writer can pack multiple fragments into a single DataFrag Submessage
or may even send a regular Data Submessage if fragmentation is no longer
required."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`fragment_size_for_message_size`, `fragments_per_message`, `ReliableWriterConfig::fragment_size`);
`crates/transport/src/lib.rs` (`Transport::max_message_size`); `crates/dcps/src/runtime.rs` (writer creation)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_under_fragment_size_produces_single_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`,
`crates/dcps/src/runtime.rs::tests::a_writers_fragment_size_follows_the_message_size_of_its_transport`

**Status:** `done` — the fragment size of a writer follows the smallest message size of its transports (`Transport::max_message_size`, item under §8.4.14.1); several fragments go into one DATA_FRAG where the message carries them; a path that needs no fragmentation gets a DATA.

### §8.3.8.3 DataFrag: inline QoS needed on the first fragment only

**Spec:** §8.3.8.3, p. 59 — "When sending inlineQos with DataFrag
Submessages, it is only required to send the inlineQos with the first
DataFrag Submessage for a given Writer sequence number."

**Repo:** `crates/rtps/src/reliable_writer.rs` (inline QoS with PID_KEY_HASH on
the DATA_FRAGs of a sample)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`

**Status:** `done` — the inline QoS of a coherent-set member on the first
DATA_FRAG is listed under §8.7.6.

### §8.3.8.4 Gap: Purpose and filtered changes

**Spec:** §8.3.8.4, p. 59 — "indicates to the RTPS Reader that a set of
changes in the RTPS Writer HistoryCache are not available to one or more RTPS
Reader endpoints and will not be sent to them. … Filtered changes should not
be treated as samples lost by the RTPS Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`:
`irrelevant_change_set`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::gap_marks_irrelevant`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_counts_what_was_neither_received_nor_irrelevant`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap EndiannessFlag

**Spec:** §8.3.8.4, p. 59, Table 8.43 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::write_body`,
`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap GroupInfoFlag

**Spec:** §8.3.8.4, p. 59, Table 8.43 — "GroupInfoFlag … Indicates the
presence of additional information about the group of writers (Writer Group)
the sender belongs to."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap FilteredCountFlag

**Spec:** §8.3.8.4, p. 59, Table 8.43 — "FilteredCountFlag … Indicates the
presence of the filteredCount submessage element."

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_FILTERED_COUNT`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap readerId

**Spec:** §8.3.8.4, p. 59, Table 8.43 — "The readerId may be set to
ENTITYID_UNKNOWN to indicate the Gap message applies to all Readers within
the receiving Participant that are matched with the Writer specified by the
writerId."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::reader_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: fan-out on
`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_submessage_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap writerId

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "writerId: EntityId — Identifies the
Writer Entity to which the not-available changes apply."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::writer_id`);
`crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapStart

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "gapStart: SequenceNumber — Identifies
the first sequence number in the interval of sequence numbers corresponding
to the set of not-available changes."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::gap_start`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_before_any_heartbeat_lets_the_next_data_through`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapList

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "gapList: SequenceNumberSet — …
(1) Identifies the last sequence number in the interval … (2) Identifies an
additional list of sequence numbers corresponding to not-available changes."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::gap_list`);
`crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapStartGSN

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "gapStartGSN: SequenceNumber —
Present only if the GroupInfoFlag is set in the header."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §8.3.8.4 Table 8.43 Gap gapEndGSN

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "gapEndGSN: SequenceNumber — Present
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

**Spec:** §8.3.8.4, p. 60, Table 8.43 — "filteredCount: ChangeCount — Present
only if the FilteredCountFlag is set in the header. Indicates the number of
changes corresponding to the set of sequence numbers in the Gap that are still
present in the Writer cache but are being filtered for the RTPS Reader."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §8.3.8.4 Gap invalid: submessageLength too small

**Spec:** §8.3.8.4, p. 60 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.4 Gap invalid: gapStart ≤ 0

**Spec:** §8.3.8.4, p. 60 — "gapStart is zero or negative."

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`

**Status:** `done` — a `gapStart` below 1 is refused.

### §8.3.8.4 Gap invalid: invalid gapList

**Spec:** §8.3.8.4, p. 60 — "gapList is invalid (as defined in 8.3.5.5)."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::gap_and_sequence_number_set_validity_rules`,
`crates/rtps/src/submessages.rs::tests::seqnumset_rejects_num_bits_above_256`

**Status:** `done` — `numBits > 256` and `bitmapBase < 1` are refused.

### §8.3.8.4 Gap invalid: group fields

**Spec:** §8.3.8.4, p. 60 — "If GroupInfoFlag is set and: gapStartGSN.value is
zero or negative; gapEndGSN.value is zero or negative; gapEndGSN.value <
gapStartGSN.value-1".

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_truncated_group_info`

**Status:** `done`

### §8.3.8.4 Gap::sequence_number_set

**Spec:** §8.3.8.4, pp. 60–61 — "1. All sequence numbers in the range gapStart
<= sequence_number <= gapList.base -1; 2. All the sequence numbers that appear
explicitly listed in the gapList."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`

**Status:** `done`

### §8.3.8.4 Gap: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.4, p. 61 — "writerGUID = { Receiver.sourceGuidPrefix,
Gap.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`:
`proxy_index_by_writer(Guid::new(source_prefix, gap.writer_id))`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`

**Status:** `done`

### §8.3.8.4 Gap: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.4, p. 61 — "readerGUID = { Receiver.destGuidPrefix,
Gap.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.4 Gap: content of the GroupInfoFlag

**Spec:** §8.3.8.4, p. 61 — "The Writer sets the GroupInfoFlag to indicate the
presence of the gapStartGSN and gapEndGSN elements. … The gapEndGSN can extend
past the Group Sequence Number that corresponds to gapList.bitmapBase".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`gap_group_info`, `note_gsn`, `GapGroupInfo`); `crates/rtps/src/submessages.rs`; `crates/dcps/src/runtime.rs` (`note_writer_group_progress`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_gap_of_a_group_writer_carries_the_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/dcps/src/group_order.rs::tests::a_gap_rules_the_number_out_for_its_writer`

**Status:** `done` — the writer of a GROUP publisher sends GAPs with the GroupInfoFlag set: `gapStartGSN` is the group sequence number of the change taken away, `gapEndGSN` the same (a range of one number; valid under §8.3.8.4); the number comes from the cache or from a bounded list (4096) for changes that left the cache. A writer outside a group sends no flag. The reader takes both numbers into the commit rule (`DcpsRuntime::note_writer_group_progress`).

### §8.3.8.4 Gap: content of filteredCount

**Spec:** §8.3.8.4, p. 61 — "The filteredCount, if present, shall include:
Changes filtered due to the writer-side application of a content-filtered-topic
… a time filter … Changes that are written in ways that intend to exclude
delivery to the Reader. … The filteredCount shall not include changes that are
no longer in the RTPS Writer HistoryCache".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a conditional rule for a writer that sends
`filteredCount`; ZeroDDS does not filter on the writer side and does not send
the field (`append_gap` sets `filtered_count: None`).

### §8.3.8.5 HeaderExtension

**Spec:** §8.3.8.5, p. 61 — "This submessage is logically part of the header
and it is defined in 8.3.7."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; items under §8.3.7.

### §8.3.8.6 Heartbeat: Purpose

**Spec:** §8.3.8.6, p. 61 — "This message is sent from an RTPS Writer to an
RTPS Reader to communicate the sequence numbers of changes that the Writer has
available."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.6 Table 8.44 Heartbeat EndiannessFlag

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::write_body`,
`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_heartbeat_decodes`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat FinalFlag

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "FinalFlag … Indicates whether the
Reader is required to respond to the Heartbeat or if it is just an advisory
heartbeat."

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_FINAL`,
`HeartbeatSubmessage::final_flag`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_no_final_flag_when_disabled`,
`crates/rtps/src/reliable_writer.rs::tests::periodic_heartbeat_has_final_flag_unset`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat LivelinessFlag

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "LivelinessFlag … Indicates that the
DDS DataWriter associated with the RTPS Writer of the message has manually
asserted its LIVELINESS."

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_LIVELINESS`);
`crates/rtps/src/reliable_writer.rs` (`ReliableWriter::liveliness_heartbeat`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`crates/rtps/src/reliable_writer.rs::tests::liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat GroupInfoFlag

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "GroupInfoFlag … Indicates the
presence of additional information about the group of writers (Writer Group)
the sender belongs to."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat readerId

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "readerId … Can be set to
ENTITYID_UNKNOWN to indicate all readers for the writer that sent the
message."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::reader_id`);
`crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: fan-out on
`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::final_heartbeat_from_unknown_to_data_writer_requests_the_missing_sn`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat writerId

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "writerId: EntityId — Identifies the
Writer Entity to which the range of sequence numbers applies."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::writer_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::unknown_writer_id_in_heartbeat_counts_not_crashes`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat firstSN

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "If samples are available in the
Writer, identifies the first (lowest) sequence number that is available in the
Writer. If no samples are available in the Writer, identifies the lowest
sequence number that is yet to be written by the Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`,
`crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat lastSN

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "lastSN: SequenceNumber — Identifies
the last (highest) sequence number that the Writer has ever written."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`last_written_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat count

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "count: Count — A counter that is
incremented each time a new Heartbeat message is sent."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`heartbeat_count.wrapping_add(1)`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_count_increments`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat currentGSN

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "currentGSN … Identifies the last
(highest) group sequence number written by any DataWriter in the Writer's
Group at the time that the HeartBeat was sent."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat firstGSN

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "firstGSN … Identifies the group
sequence number corresponding to the sample identified by sequence number
firstSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat lastGSN

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "lastGSN … Identifies the group
sequence number corresponding to the sample identified by sequence number
lastSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §8.3.8.6 Table 8.44 Heartbeat writerSet

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "writerSet: GroupDigest … Identifies
the subset of Writers that belong to the Writer's Group at the time the sample
with currentGSN was written."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatGroupInfo`, `HeartbeatSubmessage::write_body`, `read_body`); `crates/rtps/src/dialect.rs` (`HeartbeatGroupDigests`); `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`); `crates/dcps/src/group_order.rs`; `wire.omit_heartbeat_group_digests`

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`

**Status:** `done` — with G=1 the writer sends writerSet after lastGSN as a GroupDigest (four octets, §9.4.2.19; the first four octets of the MD5 over the EntityIdSet_t, sorted, §9.3.2.5; empty group: zero). The reader reads both when the submessageLength leaves room and accepts their absence (figure of §9.4.5.7); the commit rule of §8.7.5 compares them with the discovered writers. `wire.omit_heartbeat_group_digests` leaves them out on send. Contradiction PIM/PSM: see "Contradictions within the spec".

### §8.3.8.6 Table 8.44 Heartbeat secureWriterSet

**Spec:** §8.3.8.6, p. 62, Table 8.44 — "secureWriterSet: GroupDigest …
Reserved for use by the DDS-Security Specification."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatGroupInfo`, `HeartbeatSubmessage::write_body`, `read_body`); `crates/rtps/src/dialect.rs` (`HeartbeatGroupDigests`); `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`); `crates/dcps/src/group_order.rs`; `wire.omit_heartbeat_group_digests`

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`

**Status:** `done` — with G=1 the writer sends secureWriterSet after lastGSN as a GroupDigest (four octets, §9.4.2.19; the first four octets of the MD5 over the EntityIdSet_t, sorted, §9.3.2.5; empty group: zero; secureWriterSet is reserved for DDS-Security and is sent as zero). The reader reads both when the submessageLength leaves room and accepts their absence (figure of §9.4.5.7); the commit rule of §8.7.5 compares them with the discovered writers. `wire.omit_heartbeat_group_digests` leaves them out on send. Contradiction PIM/PSM: see "Contradictions within the spec".

### §8.3.8.6 Heartbeat example 1: never written

**Spec:** §8.3.8.6, p. 63 — "A Writer that has never written any samples
before sending a Heartbeat will send a Heartbeat with firstSN.value = 1,
lastSN.value = 0."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_empty_writer_heartbeats_a_reader_until_it_answers`

**Status:** `done`

### §8.3.8.6 Heartbeat example 2: one sample in the cache

**Spec:** §8.3.8.6, p. 63 — "A Writer that has only one sample in its cache
with sequence number SN will send a Heartbeat with firstSN.value =
lastSN.value = SN."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done`

### §8.3.8.6 Heartbeat example 3: part of the history in the cache

**Spec:** §8.3.8.6, p. 63 — "A Writer that has written 10 samples and still
has the last 5 samples in its cache will send a Heartbeat with firstSN.value
= 6, lastSN.value = 10."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`

**Status:** `done`

### §8.3.8.6 Heartbeat example 4: cache empty after writing

**Spec:** §8.3.8.6, p. 63 — "A Writer that has written 10 samples before
sending a Heartbeat but does not have any samples available at the time of the
Heartbeat will send a Heartbeat with firstSN.value = 11, lastSN.value = 10."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_heartbeat`:
`first_sn.min(last + 1)`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_heartbeat_after_the_cache_emptied_announces_the_last_sn_written`

**Status:** `done`

### §8.3.8.6 Heartbeat invalid: submessageLength too small

**Spec:** §8.3.8.6, p. 63 — "submessageLength in the Submessage header is too
small".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.6 Heartbeat invalid: firstSN ≤ 0

**Spec:** §8.3.8.6, p. 63 — "firstSN.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — a `firstSN` below 1 is refused.

### §8.3.8.6 Heartbeat invalid: lastSN < 0

**Spec:** §8.3.8.6, p. 63 — "lastSN.value is negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — a negative `lastSN` is refused.

### §8.3.8.6 Heartbeat invalid: lastSN < firstSN − 1

**Spec:** §8.3.8.6, p. 63 — "lastSN.value < firstSN.value - 1".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::heartbeat_validity_rules`

**Status:** `done` — `lastSN < firstSN - 1` is refused; `lastSN = firstSN - 1` (empty
history) is valid.

### §8.3.8.6 Heartbeat invalid: group fields

**Spec:** §8.3.8.6, p. 63 — six conditions for currentGSN, firstGSN and
lastGSN when the GroupInfoFlag is set.

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_current_gsn_past_the_writers_last_is_accepted`,
`crates/rtps/src/submessages.rs::tests::heartbeat_decode_rejects_truncated_group_info`

**Status:** `done` — five of the six conditions are checked. The sixth, `currentGSN > lastGSN`, contradicts the definition of currentGSN (pp. 62–63: the number of the sample written last, which lastGSN may not include yet) and is not applied; the test `a_current_gsn_past_the_writers_last_is_accepted` holds that. The reading chosen is under "Contradictions within the spec". Cyclone DDS, Fast DDS and OpenDDS send no group HEARTBEATs; expected: no effect.

### §8.3.8.6 Heartbeat: report availability

**Spec:** §8.3.8.6, p. 63 — "It informs the Reader of the sequence numbers
that are available in the writer's HistoryCache so that the Reader may
request (using an AckNack) any that it has missed."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`:
`update_from_heartbeat`); `crates/rtps/src/writer_proxy.rs`

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::heartbeat_sets_available_range`,
`crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`

**Status:** `done`

### §8.3.8.6 Heartbeat without FinalFlag requires an AckNack

**Spec:** §8.3.8.6, p. 63 — "if the FinalFlag is not set, then the Reader must
send an AckNack message indicating which CacheChange changes it has received,
even if the AckNack indicates it has received all CacheChange changes in the
writer's HistoryCache."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_without_the_final_flag_is_answered_when_everything_is_received`

**Status:** `done` — a HEARTBEAT without the FinalFlag triggers an ACKNACK even when everything is received (`bitmapBase` behind the last number, nothing missing), with and without the LivelinessFlag.

### §8.3.8.6 Heartbeat LivelinessFlag renews the manual lease

**Spec:** §8.3.8.6, p. 63 — "The RTPS Reader should therefore renew the manual
liveliness lease of the corresponding remote DDS DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`liveliness.writer_asserted` on `h.liveliness_flag`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`

**Status:** `done`

### §8.3.8.6 Heartbeat: content of the GroupInfoFlag

**Spec:** §8.3.8.6, p. 64 — "The Writer sets the GroupInfoFlag to indicate the
presence of the currentGSN, firstGSN, lastGSN, writerSet, and secureWriterSet
elements."

**Repo:** `crates/rtps/src/submessages.rs`; `crates/rtps/src/reliable_writer.rs` (`heartbeat_group_info`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_digests_follow_last_gsn_and_are_optional_on_read`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_writer_sets_unless_the_peer_omits_them`,
`crates/dcps/src/group_order.rs::tests::a_heartbeat_whose_writer_sets_are_not_the_discovered_ones_keeps_the_number_open`,
`crates/dcps/tests/runtime_config_layering.rs::the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done` — currentGSN, firstGSN, lastGSN, writerSet and secureWriterSet are sent and read (see the writerSet and secureWriterSet items).

### §8.3.8.6 Heartbeat: writerGUID = {sourceGuidPrefix, writerId}

**Spec:** §8.3.8.6, p. 64 — "writerGUID = { Receiver.sourceGuidPrefix,
Heartbeat.writerId }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.3.8.6 Heartbeat: readerGUID = {destGuidPrefix, readerId}

**Spec:** §8.3.8.6, p. 64 — "readerGUID = { Receiver.destGuidPrefix,
Heartbeat.readerId }. The Heartbeat.readerId can be ENTITYID_UNKNOWN, in which
case the Heartbeat applies to all Readers of that writerGUID within the
Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.7 HeartbeatFrag: Purpose

**Spec:** §8.3.8.7, p. 64 — "When fragmenting data and until all fragments are
available, the HeartbeatFrag Submessage is sent from an RTPS Writer to an RTPS
Reader to communicate which fragments the Writer has available."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose; ZeroDDS sends a
sample only once all its fragments are available.

### §8.3.8.7 Table 8.45 HeartbeatFrag EndiannessFlag

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::write_body`,
`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_be`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag readerId

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "readerId … Can be set to
ENTITYID_UNKNOWN to indicate all readers for the writer that sent the
message."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag writerId

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "writerId: EntityId — Identifies the
Writer Entity that sent the Submessage."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::writer_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag writerSN

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "writerSN: SequenceNumber — Identifies
the sequence number of the data change for which fragments are available."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::writer_sn`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag lastFragmentNum

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "lastFragmentNum: FragmentNumber — All
fragments up to and including this last (highest) fragment are available on
the Writer for the change identified by writerSN."

**Repo:** `crates/rtps/src/submessages.rs`
(`HeartbeatFragSubmessage::last_fragment_num`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §8.3.8.7 Table 8.45 HeartbeatFrag count

**Spec:** §8.3.8.7, p. 64, Table 8.45 — "count: Count — A counter that is
incremented each time a new HeartbeatFrag message is sent."

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::count`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`

**Status:** `done` — ZeroDDS sends no HEARTBEAT_FRAG; the field is encoded
and read.

### §8.3.8.7 HeartbeatFrag invalid: submessageLength too small

**Spec:** §8.3.8.7, p. 64 — "submessageLength in the Submessage header is too
small".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.7 HeartbeatFrag invalid: writerSN ≤ 0

**Spec:** §8.3.8.7, p. 64 — "writerSN.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — a `writerSN` below 1 is refused.

### §8.3.8.7 HeartbeatFrag invalid: lastFragmentNum ≤ 0

**Spec:** §8.3.8.7, p. 64 — "lastFragmentNum.value is zero or negative".

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — a `lastFragmentNum` below 1 is refused.

### §8.3.8.7 HeartbeatFrag: the reader answers with a NackFrag

**Spec:** §8.3.8.7, p. 65 — "The RTPS Reader will respond by sending a
NackFrag message, but only if it is missing any of the available
fragments."

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`ReliableReader::handle_heartbeat_frag`), `crates/dcps/src/runtime.rs`
(`handle_user_datagram_from`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done`

### §8.3.8.7 HeartbeatFrag: writerGUID/readerGUID

**Spec:** §8.3.8.7, p. 65 — "writerGUID = { Receiver.sourceGuidPrefix,
HeartbeatFrag.writerId } … readerGUID = { Receiver.destGuidPrefix,
HeartbeatFrag.readerId } … can be ENTITYID_UNKNOWN".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`: readerId or
ENTITYID_UNKNOWN), `crates/rtps/src/reliable_reader.rs`
(`proxy_index_by_writer`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done`

### §8.3.8.8 InfoDestination: Purpose

**Spec:** §8.3.8.8, p. 65 — "This message is sent from an RTPS Writer to an
RTPS Reader to modify the GuidPrefix used to interpret the Reader entityIds
appearing in the Submessages that follow it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.8 Table 8.46 InfoDestination EndiannessFlag

**Spec:** §8.3.8.8, p. 65, Table 8.46 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`
for `InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_decodes_to_typed_destination_prefix`

**Status:** `done`

### §8.3.8.8 Table 8.46 InfoDestination guidPrefix

**Spec:** §8.3.8.8, p. 65, Table 8.46 — "guidPrefix: GuidPrefix — Provides the
GuidPrefix that should be used to reconstruct the GUIDs of all the RTPS Reader
entities whose EntityIds appears in the Submessages that follow."

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`);
`crates/rtps/src/reliable_reader.rs` (`wrap_to_writer` sends INFO_DST before
ACKNACK/NACK_FRAG)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_carries_info_dst`,
`crates/rtps/src/reliable_writer.rs::tests::every_proxy_directed_datagram_carries_info_dst_not_just_the_heartbeat_path`

**Status:** `done`

### §8.3.8.8 InfoDestination invalid: submessageLength too small

**Spec:** §8.3.8.8, p. 65 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, InfoDestination)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.8 InfoDestination: destGuidPrefix = guidPrefix

**Spec:** §8.3.8.8, p. 65 — "if (InfoDestination.guidPrefix !=
GUIDPREFIX_UNKNOWN) { Receiver.destGuidPrefix = InfoDestination.guidPrefix }".

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_info_source_does_not_reset_dest_guid_prefix`

**Status:** `done`

### §8.3.8.8 InfoDestination: GUIDPREFIX_UNKNOWN sets the own prefix

**Spec:** §8.3.8.8, p. 65 — "else { Receiver.destGuidPrefix = <GuidPrefix_t of
the Participant receiving the_message> }".

**Repo:** `crates/rtps/src/datagram.rs` (`ReceiverWalk::next`,
`InfoDestination`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.8 destGuidPrefix on the user-data path

**Spec:** §8.3.8.2, p. 56 — "readerGUID = { Receiver.destGuidPrefix,
Data.readerId }"; likewise §8.3.8.4, p. 61 (Gap), §8.3.8.6, p. 64 (Heartbeat),
§8.3.8.7, p. 65 (HeartbeatFrag); §8.3.8.3, p. 58 — "the interpretation of the
DataFrag Submessages is identical to that of the Data Submessage"; §8.3.8.1,
p. 54 — "writerGUID = { Receiver.destGuidPrefix, AckNack.writerId }";
likewise §8.3.8.12, p. 69 (NackFrag).

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.9 InfoReply: Purpose

**Spec:** §8.3.8.9, p. 66 — "It contains explicit information on where to send
a reply to the Submessages that follow it within the same message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.9 Table 8.47 InfoReply EndiannessFlag

**Spec:** §8.3.8.9, p. 66, Table 8.47 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::write_body`,
`InfoReplySubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`

**Status:** `done`

### §8.3.8.9 Table 8.47 InfoReply MulticastFlag

**Spec:** §8.3.8.9, p. 66, Table 8.47 — "MulticastFlag … Indicates whether the
Submessage also contains a multicast address."

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_REPLY_FLAG_MULTICAST`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`

**Status:** `done`

### §8.3.8.9 Table 8.47 InfoReply unicastLocatorList

**Spec:** §8.3.8.9, p. 66, Table 8.47 — "unicastLocatorList: LocatorList —
Indicates an alternative set of unicast addresses that the Writer should use
to reach the Readers when replying to the Submessages that follow."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::unicast_locators`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_empty_unicast_list_is_valid`,
`crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`

**Status:** `done` — whether a writer uses the list when replying is listed
under §8.4.

### §8.3.8.9 Table 8.47 InfoReply multicastLocatorList

**Spec:** §8.3.8.9, p. 66, Table 8.47 — "multicastLocatorList … Only present
when the MulticastFlag is set."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::multicast_locators`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`

**Status:** `done`

### §8.3.8.9 InfoReply invalid: submessageLength too small

**Spec:** §8.3.8.9, p. 66 — "submessageLength in the Submessage header is too
small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`,
`crates/rtps/src/submessages.rs::validity_tests::an_info_reply_locator_of_an_unknown_kind_is_left_out_and_counted`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand. A locator of an unknown kind in the list does not make the submessage invalid (§9.3.2.4: vendor kinds): it is left out and counted (`skipped_unknown_locators`), the others stand.

### §8.3.8.9 InfoReply: Receiver.unicastReplyLocatorList

**Spec:** §8.3.8.9, p. 66 — "Receiver.unicastReplyLocatorList =
InfoReply.unicastLocatorList".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_reply`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_reply_sets_locators`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_sets_reply_locators`

**Status:** `done`

### §8.3.8.9 InfoReply: Receiver.multicastReplyLocatorList

**Spec:** §8.3.8.9, p. 66 — "if ( MulticastFlag ) {
Receiver.multicastReplyLocatorList = InfoReply.multicastLocatorList } else {
Receiver.multicastReplyLocatorList = <empty> }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_reply`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_reply_sets_locators`,
`crates/rtps/src/receiver_state.rs::tests::an_info_reply_without_the_multicast_flag_empties_the_multicast_list`

**Status:** `done` — without the MulticastFlag the list is emptied.

### §8.3.8.10 InfoSource: Purpose

**Spec:** §8.3.8.10, p. 66 — "This message modifies the logical source of the
Submessages that follow."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.10 Table 8.48 InfoSource EndiannessFlag

**Spec:** §8.3.8.10, p. 67, Table 8.48 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::write_body`,
`InfoSourceSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_be`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource protocolVersion

**Spec:** §8.3.8.10, p. 67, Table 8.48 — "protocolVersion: ProtocolVersion —
Indicates the protocol used for subsequent Submessages."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::protocol_version`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_le`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource vendorId

**Spec:** §8.3.8.10, p. 67, Table 8.48 — "vendorId: VendorId — Indicates the
VendorId of the vendor that originated the subsequent Submessages."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::vendor_id`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_roundtrip_le`

**Status:** `done`

### §8.3.8.10 Table 8.48 InfoSource guidPrefix

**Spec:** §8.3.8.10, p. 67, Table 8.48 — "guidPrefix: GuidPrefix — Identifies
the Participant that is the container of the RTPS Writer entities that are the
source of the Submessages that follow."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::guid_prefix`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`

**Status:** `done`

### §8.3.8.10 InfoSource invalid: submessageLength too small

**Spec:** §8.3.8.10, p. 67 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.10 InfoSource: sourceGuidPrefix, sourceVersion, sourceVendorId

**Spec:** §8.3.8.10, p. 67 — "Receiver.sourceGuidPrefix =
InfoSource.guidPrefix; Receiver.sourceVersion = InfoSource.protocolVersion;
Receiver.sourceVendorId = InfoSource.vendorId".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`,
`crates/discovery/tests/opensplice_infosrc_regression.rs::opensplice_bundled_infosrc_datagrams_attribute_to_the_real_source`

**Status:** `done`

### §8.3.8.10 InfoSource: reply lists = {LOCATOR_INVALID}

**Spec:** §8.3.8.10, p. 67 — "Receiver.unicastReplyLocatorList = {
LOCATOR_INVALID }; Receiver.multicastReplyLocatorList = { LOCATOR_INVALID }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`

**Status:** `done` — both lists are set to `{ LOCATOR_INVALID }`.

### §8.3.8.10 InfoSource: haveTimestamp = false

**Spec:** §8.3.8.10, p. 67 — "haveTimestamp = false".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_source`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_source_resets_reply_locators_and_timestamp`

**Status:** `done`

### §8.3.8.11 InfoTimestamp: Purpose

**Spec:** §8.3.8.11, p. 67 — "This Submessage is used to send a timestamp
which applies to the Submessages that follow within the same message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.11 Table 8.49 InfoTimestamp EndiannessFlag

**Spec:** §8.3.8.11, p. 67, Table 8.49 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::write_body`,
`InfoTimestampSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`

**Status:** `done`

### §8.3.8.11 Table 8.49 InfoTimestamp InvalidateFlag

**Spec:** §8.3.8.11, p. 67, Table 8.49 — "InvalidateFlag … Indicates whether
subsequent Submessages should be considered as having a timestamp or not."

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_TIMESTAMP_FLAG_INVALIDATE`,
`InfoTimestampSubmessage::invalidate`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_invalidate_flag_yields_empty_body`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_decode_with_invalidate_ignores_body`

**Status:** `done`

### §8.3.8.11 Table 8.49 InfoTimestamp timestamp

**Spec:** §8.3.8.11, p. 67, Table 8.49 — "timestamp: Timestamp — Present only
if the InvalidateFlag is not set in the header."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::timestamp`,
`InfoTimestampSubmessage::timestamp_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`

**Status:** `done`

### §8.3.8.11 InfoTimestamp invalid: submessageLength too small

**Spec:** §8.3.8.11, p. 68 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.11 InfoTimestamp: set haveTimestamp and timestamp

**Spec:** §8.3.8.11, p. 68 — "if ( !InfoTimestamp.InvalidateFlag ) {
Receiver.haveTimestamp = true; Receiver.timestamp = InfoTimestamp.timestamp
… } else { Receiver.haveTimestamp = false }".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::apply_info_timestamp`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::apply_info_timestamp_sets_value`,
`crates/rtps/src/receiver_state.rs::tests::apply_info_timestamp_with_invalidate_clears`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`

**Status:** `done`

### §8.3.8.11 InfoTimestamp: correction on clockSkewDetected

**Spec:** §8.3.8.11, p. 68 — "if ( Receiver.clockSkewDetected ) {
Receiver.timestamp += Receiver.rtpsReceptionTime - Receiver.rtpsSendTimestamp
}".

**Repo:** `crates/rtps/src/receiver_state.rs` (`ReceiverState::note_reception`, `note_clock_skew`,
`apply_info_timestamp`); `crates/rtps/src/datagram.rs` (`ReceptionClock`, `ReceiveContext`);
`crates/dcps/src/runtime.rs` (`receive_context`, `RuntimeConfig::clock_skew_threshold_s`)

**Tests:** `crates/rtps/src/receiver_state.rs::tests::the_reception_time_is_kept_and_checked_against_the_send_time`,
`crates/rtps/src/receiver_state.rs::tests::an_info_timestamp_is_corrected_by_the_clock_skew`,
`crates/rtps/src/datagram.rs::tests::the_walk_with_a_reception_clock_detects_a_skew_and_corrects_the_timestamp`,
`crates/dcps/tests/runtime_config_layering.rs::the_clock_skew_threshold_is_a_setting_and_off_by_default`

**Status:** `done` — with `clockSkewDetected`, `apply_info_timestamp` adds reception time minus send time; in every receive path this takes effect once `wire.clock_skew_threshold_s` is set (default off, see §8.3.7.4 TimestampFlag).

### §8.3.8.11 InfoTimestamp: source timestamp of the following Data/DataFrag

**Spec:** §8.3.8.11, p. 68 — "The reception of the InfoTimestamp submessage
sets the source timestamp associated with any subsequent Data and DataFrag
submessages."

**Repo:** `crates/rtps/src/receiver_state.rs`, `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_timestamp_is_no_source_timestamp`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done` — an INFO_TS sets the source timestamp of the following Data and DataFrag, the I-flag case clears it; the timestamp of a HeaderExtension is `rtpsSendTimestamp` and no source timestamp.

### §8.3.8.12 NackFrag: Purpose

**Spec:** §8.3.8.12, p. 68 — "This Submessage can only contain negative
acknowledgements. … Fragments can be negatively acknowledged in any order."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.12 Table 8.50 NackFrag EndiannessFlag

**Spec:** §8.3.8.12, p. 68, Table 8.50 — "EndiannessFlag … Indicates
endianness."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::write_body`,
`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_be`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag readerId

**Spec:** §8.3.8.12, p. 68, Table 8.50 — "readerId: EntityId — Identifies the
Reader entity that requests to receive certain fragments."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::reader_id`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nackfrag_dispatches_only_to_matching_proxy`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag writerId

**Spec:** §8.3.8.12, p. 69, Table 8.50 — "writerId: EntityId — Identifies the
Writer entity that is the target of the NackFrag message."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::writer_id`);
`crates/rtps/src/reliable_writer.rs` (`handle_nackfrag`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_queues_fragment_resends`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag writerSN

**Spec:** §8.3.8.12, p. 69, Table 8.50 — "writerSN: SequenceNumber — The
sequence number for which some fragments are missing."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::writer_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_fragments`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag fragmentNumberState

**Spec:** §8.3.8.12, p. 69, Table 8.50 — "fragmentNumberState:
FragmentNumberSet — … The fragment numbers that appear in the set indicate
missing fragments on the reader side."

**Repo:** `crates/rtps/src/submessages.rs`
(`NackFragSubmessage::fragment_number_state`); `crates/rtps/src/reader_proxy.rs`
(`requested_fragments_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::requested_fragments_set_above_ack_only`,
`crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_queues_fragment_resends`

**Status:** `done`

### §8.3.8.12 Table 8.50 NackFrag count

**Spec:** §8.3.8.12, p. 69, Table 8.50 — "count: Count — A counter that is
incremented each time a new NackFrag message is sent."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`build_nackfrag_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::the_counts_of_acknack_and_nack_frag_rise_with_each_message`

**Status:** `done` — successive NACK_FRAGs carry the counts 1, 2, 3.

### §8.3.8.12 NackFrag invalid: submessageLength too small

**Spec:** §8.3.8.12, p. 69 — "submessageLength in the Submessage header is
too small."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/datagram.rs::tests::a_submessage_length_too_small_invalidates_the_rest_and_keeps_what_came_before`,
`crates/rtps/src/datagram.rs::tests::a_submessage_length_past_the_end_invalidates_the_rest`

**Status:** `done` — the shortening is recognised; the submessage is invalid and the rest of the message with it, the submessages before it stand (`ParsedDatagram::invalid_rest`, counted).

### §8.3.8.12 NackFrag invalid: writerSN ≤ 0

**Spec:** §8.3.8.12, p. 69 — "writerSN.value is zero or negative."

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage::read_body`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`

**Status:** `done` — a `writerSN` below 1 is refused.

### §8.3.8.12 NackFrag invalid: invalid fragmentNumberState

**Spec:** §8.3.8.12, p. 69 — "fragmentNumberState is invalid (as defined in
8.3.5.7)."

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done` — `numBits > 256` and `bitmapBase < 1` are refused.

### §8.3.8.12 NackFrag: writerGUID = {destGuidPrefix, writerId}

**Spec:** §8.3.8.12, p. 69 — "writerGUID = { Receiver.destGuidPrefix,
NackFrag.writerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`rt.writer_slot(nf.writer_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_submessages_after_an_info_destination_for_another_participant_are_not_ours`

**Status:** `done`

### §8.3.8.12 NackFrag: readerGUID = {sourceGuidPrefix, readerId}

**Spec:** §8.3.8.12, p. 69 — "readerGUID = { Receiver.sourceGuidPrefix,
NackFrag.readerId }".

**Repo:** `crates/dcps/src/runtime.rs` (`handle_user_datagram_from`:
`Guid::new(src_prefix, nf.reader_id)`); `crates/rtps/src/reliable_writer.rs`
(`handle_nackfrag`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_nackfrag_unknown_source_counts`

**Status:** `done`

### §8.3.8.13 Pad: Purpose

**Spec:** §8.3.8.13, p. 69 — "The purpose of this Submessage is to allow the
introduction of any padding necessary to meet any desired memory- alignment
requirements. It has no other meaning."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of purpose.

### §8.3.8.13 Pad: length from submessageLength, always valid

**Spec:** §8.3.8.13, p. 69 — "This Submessage has no contents. … The amount of
padding is determined by the value of submessageLength. … This Submessage is
always valid."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: PAD skipped as
`ParsedSubmessage::Unknown`, length 0 as a length)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

---

## §8.4 Behavior Module

### §8.4.1 Overview

**Spec:** §8.4.1, p. 70 — structure of the module; §8.4.2 holds the
obligations, §8.4.7–§8.4.12 two reference implementations.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure.

### §8.4.1 The DataWriter removes changes only per communication status and QoS

**Spec:** §8.4.1, p. 70 — "It is the responsibility of the DDS DataWriter to
only remove those CacheChange changes that can be removed based on the
communication status and the DDS DataWriter's QoS. For example, the HISTORY
QoS setting of KEEP_LAST with a depth of 1 allows a DataWriter to remove a
CacheChange if a more recent change replaces the value of the same
data-object."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_acknowledged`,
KEEP_LAST eviction through `HistoryCache::insert_returning_evicted`);
`crates/dcps/src/runtime.rs` (`await_writer_room` for KEEP_ALL)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_evicts_oldest_on_overflow`,
`crates/rtps/src/history_cache.rs::tests::keep_last_per_instance_keeps_the_newest_of_each_instance`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** `done`

### §8.4.1.1 Example Behavior

**Spec:** §8.4.1.1, pp. 70–72 — "The contents of this sub clause are not part
of the formal specification of the protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — explicitly not part of the formal spec.

### §8.4.2 Behavior Required for Interoperability: scope

**Spec:** §8.4.2, pp. 72–73 — the obligations apply to exchanges between
implementations of different vendors.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — scope; the obligations follow one by one.

### §8.4.2.1 Only RTPS messages

**Spec:** §8.4.2.1, p. 73 — "No other messages can be used than the RTPS
Messages defined in 8.3. … Vendors may extend Messages for vendor specific
needs using the extension mechanisms provided by the protocol (see 8.6)."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `encode_datagram`);
`crates/rtps/src/parameter_list.rs` (vendor PIDs `SHM_LOCATOR`,
`ZERODDS_TYPE_ID`)

**Tests:** `crates/rtps/tests/vendor_frames.rs::cyclone_session_re_encodes_byte_for_byte`,
`crates/rtps/tests/vendor_frames.rs::rti_session_re_encodes_byte_for_byte`

**Status:** `done` — the deviation in the HeaderExtension id is listed under
§9.4.5.1.1.

### §8.4.2.1 Message receiver implemented

**Spec:** §8.4.2.1, p. 73 — "Implementations must implement the rules
followed by the RTPS Message Receiver, as introduced in 8.3.4 … This
requirement also includes proper Message formatting by preceding Entity
Submessages with Interpreter Submessages when required".

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedDatagram::receiver_walk`);
`crates/rtps/src/reliable_writer.rs` (INFO_TS and INFO_DST before DATA,
HEARTBEAT); `crates/rtps/src/reliable_reader.rs` (`wrap_to_writer`)

**Tests:** `crates/rtps/src/datagram.rs::tests::receiver_walk_attributes_submessages_after_info_source_to_the_new_prefix`,
`crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_carries_info_dst`

**Status:** `done` — the receiver is implemented; the items under §8.3.4, §8.3.4.1 and §8.3.8 in which it differed from the spec ("Valid submessages before an invalid one", "destGuidPrefix on the data path" and others) are `done`.

### §8.4.2.1 Timing tunable

**Spec:** §8.4.2.1, p. 73 — "where the requirements on the protocol behavior
allow delayed responses or specify periodic events, implementations must
allow the end-user to tune those timing characteristics."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::nack_response_delay`,
`nack_suppression_duration`, `writer_heartbeat_period`, `reader_heartbeat_response_delay`,
`reader_heartbeat_suppression_duration`, `spdp_period`, `wlp_period`)

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`,
`crates/rtps/src/reliable_writer.rs::tests::a_nack_inside_the_suppression_duration_is_ignored`

**Status:** `done` — nackResponseDelay, nackSuppressionDuration, heartbeatPeriod, heartbeatResponseDelay, heartbeatSuppressionDuration, the SPDP and the WLP period are settings (`writer.*_ms`, `reader.*_ms`, `discovery.*_ms`, man page and example configuration).

### §8.4.2.1 HEARTBEAT timings of the user endpoints tunable

**Spec:** §8.4.2.1, p. 73 — as the previous item; §8.4.7.1 Table 8.53
`heartbeatPeriod`, §8.4.10.1 Table 8.67 `heartbeatResponseDelay`,
`heartbeatSuppressionDuration`.

**Repo:** `crates/dcps/src/runtime.rs` (`writer_heartbeat_period`, `reader_heartbeat_response_delay`,
`reader_heartbeat_suppression_duration`)

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `writer.heartbeat_period_ms`, `reader.heartbeat_response_delay_ms`, `reader.heartbeat_suppression_duration_ms`.

### §8.4.2.1 SPDP and SEDP implemented

**Spec:** §8.4.2.1, p. 73 — "For the purpose of interoperability,
implementations must implement at least the Simple Participant Discovery
Protocol and Simple Endpoint Discovery Protocol (see 8.5.1)."

**Repo:** `crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`,
`crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`

**Status:** `done` — the individual obligations are under §8.5.

### §8.4.2.2 Writers do not send out of order

**Spec:** §8.4.2.2, p. 73 — "A Writer must send out data samples in the order
they were added to its HistoryCache."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`:
sends only to a proxy that is in step; repairs ascending through
`next_requested_change`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_zero_loss_delivers_all_in_order`,
`crates/rtps/src/reliable_writer.rs::tests::repairs_in_order_bring_a_lagging_proxy_into_step`

**Status:** `done`

### §8.4.2.2 Writers send in-line QoS on the reader's request

**Spec:** §8.4.2.2, p. 73 — "A Writer must honor a Reader's request to receive
data messages with in-line QoS."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_inline_writer_qos`,
`sample_inline_qos`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::expects_inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`

**Status:** `done`

### §8.4.2.2 Periodic HEARTBEATs (reliable only)

**Spec:** §8.4.2.2, pp. 73–74 — "A Writer must periodically inform each
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

### §8.4.2.2 Response to a negative acknowledgement (reliable only)

**Spec:** §8.4.2.2, p. 74 — "When receiving an ACKNACK Message indicating a
Reader is missing some data samples, the Writer must respond by either sending
the missing data samples, sending a GAP message when the sample is not
relevant, or sending a HEARTBEAT message when the sample is no longer
available."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: DATA from the cache,
GAP through `append_gap` for changes no longer present)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.4.2.2 HEARTBEAT and GAP with writer group information

**Spec:** §8.4.2.2, p. 74 — "A Writer belonging to a Group shall send
HEARTBEAT or GAP Submessages to its matched Readers even if the Reader has
acknowledged all of that Writer's samples."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_writer_heartbeats_an_acknowledged_reader_when_the_group_moved_on`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writer_with_an_empty_cache_is_due_for_a_heartbeat`

**Status:** `done`

### §8.4.2.3 Best-effort readers are passive

**Spec:** §8.4.2.3, p. 74 — "A best-effort Reader is completely passive as it
only receives data and does not send messages itself."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_sends_no_acknack`

**Status:** `done` — a reader with `set_best_effort(true)` sends neither ACKNACK
nor NACK_FRAG, neither on matching nor in answer to HEARTBEATs.

### §8.4.2.3 Response to a HEARTBEAT without FinalFlag

**Spec:** §8.4.2.3, p. 74 — "Upon receiving a HEARTBEAT Message with final
flag not set, the Reader must respond with an ACKNACK Message. … The response
may be delayed to avoid message storms."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`,
`tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.2.3 Response to a HEARTBEAT with missing samples

**Spec:** §8.4.2.3, p. 74 — "a Reader that is missing some data samples must
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

### §8.4.2.3 Once acknowledged, always acknowledged (reader)

**Spec:** §8.4.2.3, p. 74 — "Once a Reader has positively acknowledged
receiving a sample using an ACKNACK Message, it can no longer negatively
acknowledge that same sample at a later point."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`acknack_base` monotonic;
`unreceive_change_set` only above the base)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::acknack_base_walks_up`,
`crates/rtps/src/reliable_reader.rs::tests::sample_rejected_by_a_full_cache_stays_missing_for_the_acknack`

**Status:** `done`

### §8.4.2.3 The writer resends acknowledged samples requested again

**Spec:** §8.4.2.3, p. 74 — "if a Writer receives a negative acknowledgement
to a previously positively acknowledged sample, and the Writer can still
service the request, the Writer should send the sample."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes_set`);
`crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`, `tick`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`,
`crates/rtps/src/reliable_writer.rs::tests::an_acknowledged_change_asked_for_again_is_sent_again`

**Status:** `done` — a new request for an acknowledged sample is kept; the
writer sends it while its cache holds it, a GAP otherwise.

### §8.4.2.3 ACKNACK only in response to a HEARTBEAT

**Spec:** §8.4.2.3, p. 74 — "In steady state, an ACKNACK Message can only be
sent as a response to a HEARTBEAT Message from a Writer. ACKNACK Messages can
be sent from a Reader when it first discovers a Writer as an optimization."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`, `handle_heartbeat`,
`handle_data_frag_opening`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::acknack_and_nack_frag_wait_for_a_heartbeat`,
`crates/rtps/src/reliable_reader.rs::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`

**Status:** `done` — an ACKNACK follows from a HEARTBEAT (Table 8.77 T2) and from the first
match of a writer; neither the repeated `add_writer_proxy` of a known writer nor a
DATA_FRAG with holes arms one.

### §8.4.3 Implementing the RTPS Protocol

**Spec:** §8.4.3, p. 75 — "a compliant implementation of the protocol need
only satisfy the requirements presented in 8.4.2. … Actual implementations
need not necessarily follow the reference implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — makes the reference implementations
optional. The transitions under §8.4.8–§8.4.12 are checked against the
ZeroDDS types that model a reference implementation: `ReliableWriter`
(stateful writer, best effort and reliable, user-data path), `ReliableReader`
(stateful reader), `BestEffortWriter`, `ReliableStatelessWriter`,
`BestEffortReader` (used in tests only) and the SPDP beacon.

### §8.4.4 Permitted combinations of reliabilityLevel

**Spec:** §8.4.4, p. 75 — "An RTPS Writer cannot be matched to an RTPS Reader
unless either the RTPS Writer has the reliabilityLevel set to RELIABLE, or
else both the RTPS Writer and RTPS Reader have the reliabilityLevel set to
BEST_EFFORT."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind` ordering);
QoS comparison on matching in `crates/dcps/src/runtime.rs`; the ReaderProxy is
reliable only with a reliable writer and reader (`ReaderProxy::new` in the SEDP
match path of `crates/dcps/src/runtime.rs`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`,
`crates/qos/src/policies/reliability.rs::tests::ordering_reliable_higher_than_besteffort`,
`crates/dcps/tests/transient_local_durability.rs::linux::transient_local_writer_delivers_history_to_late_joiner`

**Status:** `done` — the comparison of offered and requested QoS is listed in
the DCPS audit.

### §8.4.4 Stateful and stateless interoperate

**Spec:** §8.4.4, p. 75 — "a Stateful Writer is able to communicate with a
Stateless Reader and vice versa."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — follows from §8.4.2; no requirement of its
own.

### §8.4.5 Notational Conventions

**Spec:** §8.4.5, pp. 75–76, Table 8.51 — abbreviations DW, DR, W, RP, RL, R,
WP, WHC, RHC.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation.

### §8.4.6 Table 8.52 Duration_t

**Spec:** §8.4.6, p. 76, Table 8.52 — "Duration_t: Type used to hold time
differences. Should have at least nano-second resolution."

**Repo:** `crates/qos/src/duration.rs` (`Duration {seconds, fraction}`,
2⁻³² s)

**Tests:** `crates/dcps/src/time.rs::tests::duration_zero_and_infinite_sentinels`,
`crates/rtps/src/participant_data.rs::tests::duration_roundtrip_le`

**Status:** `done`

### §8.4.6 Table 8.52 ChangeForReaderStatusKind

**Spec:** §8.4.6, p. 76, Table 8.52 — "UNSENT, UNACKNOWLEDGED, REQUESTED,
ACKNOWLEDGED, UNDERWAY".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn`,
`highest_acked_sn`, `requested_changes` instead of a status per change)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`,
`crates/rtps/src/reader_proxy.rs::tests::acking_also_prunes_requested_changes`

**Status:** `done` — modelled as counters and sets (§8.4.15.1).

### §8.4.6 Table 8.52 ChangeFromWriterStatusKind

**Spec:** §8.4.6, p. 76, Table 8.52 — "NOT_AVAILABLE, MISSING, RECEIVED,
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

**Status:** `done` — UNKNOWN, MISSING, RECEIVED and NOT_AVAILABLE with the sub-kinds NA_FILTERED, NA_REMOVED and NA_UNSPECIFIED are kept per change (`WriterProxy::change_status`). The NOT_AVAILABLE changes are held as runs of one sub-kind (at most 4096; older means `None`). `lost_changes_update` enters NA_REMOVED and counts it; a change with the FilteredFlag enters NA_FILTERED.

### §8.4.6 Table 8.52 InstanceHandle_t

**Spec:** §8.4.6, p. 76, Table 8.52 — "Type used to represent the identity of
a data-object whose changes in value are communicated by the RTPS protocol."

**Repo:** `crates/rtps/src/history_cache.rs` (`CacheChange::key_hash`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::the_instance_index_follows_inserts_evictions_and_removals`

**Status:** `done` — as Table 8.2 (§8.2.1.2).

### §8.4.6 Table 8.52 ParticipantMessageData

**Spec:** §8.4.6, p. 76, Table 8.52 — "Type used to hold data exchanged
between Participants. The most notable use of this type is for the Writer
Liveliness Protocol."

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`

**Status:** `done` — content and wire format under §8.4.13.4 and §9.6.3.1.

### §8.4.7.1 Table 8.53 Writer pushMode

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "If pushMode==true, then the Writer
will push changes to the reader. If pushMode==false, changes will only be
announced via heartbeats and only be sent as response to the request of a
reader."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_push_mode`, `push_mode`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::push_mode_false_announces_by_heartbeat_and_sends_only_on_request`,
`crates/rtps/src/reliable_writer.rs::tests::push_mode_false_sends_no_data_on_the_piggyback_path`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer heartbeatPeriod

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "heartbeatPeriod: Protocol tuning
parameter that allows the RTPS Writer to repeatedly announce the
availability of data by sending a Heartbeat Message."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`ReliableWriterConfig::heartbeat_period`, `DEFAULT_HEARTBEAT_PERIOD`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_due_follows_the_last_heartbeat`,
`crates/rtps/src/reliable_writer.rs::tests::tick_emits_heartbeat_after_period`

**Status:** `done` — its tunability for the user is listed under §8.4.2.1.

### §8.4.7.1 Table 8.53 Writer nackResponseDelay

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "nackResponseDelay: Protocol tuning
parameter that allows the RTPS Writer to delay the response to a request for
data from a negative acknowledgment."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`set_nack_response_delay`);
`crates/rtps/src/reader_proxy.rs` (`repair_due`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`,
`crates/rtps/src/reliable_writer.rs::tests::a_zero_nack_response_delay_repairs_on_the_next_tick`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer nackSuppressionDuration

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "nackSuppressionDuration: Protocol
tuning parameter that allows the RTPS Writer to ignore requests for data from
negative acknowledgments that arrive 'too soon' after the corresponding change
is sent."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`set_nack_suppression_duration`, `too_soon`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_nack_inside_the_suppression_duration_is_ignored`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer lastChangeSequenceNumber

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "Internal counter used to assign
increasing sequence number to each change made by the Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`next_sn`, `last_written_sn`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer writer_cache

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "writer_cache: HistoryCache — Contains
the history of CacheChange changes for this Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::cache`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_at_cache_capacity_is_error`

**Status:** `done`

### §8.4.7.1 Table 8.53 Writer dataMaxSizeSerialized

**Spec:** §8.4.7.1, p. 78, Table 8.53 — "Optional attribute that indicates the
maximum size of any SerializedPayload that may be sent by the Writer."

**Repo:** `crates/dcps/src/dds_type.rs` (`MAX_SERIALIZED_SIZE`);
`crates/dcps/src/publisher.rs` (`WriterRegistration`); `crates/dcps/src/runtime.rs` (`UserWriterConfig::data_max_size_serialized`,
`build_publication_data`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`

**Status:** `done` — the attribute is optional. The writer announces `dataMaxSizeSerialized` as `PID_TYPE_MAX_SIZE_SERIALIZED` when it is set: from `DdsType::MAX_SERIALIZED_SIZE` or from `UserWriterConfig::data_max_size_serialized` (the value is without the encapsulation header; it is announced with the 4 octets of the header). Without a value it announces nothing.

### §8.4.7.1 Table 8.54 Writer new

**Spec:** §8.4.7.1, p. 79 — "this.lastChangeSequenceNumber := 0;
this.writer_cache := new HistoryCache;" and the attributes taken from the
constructor.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::new`:
`next_sn: 0`, a new `HistoryCache`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`

**Status:** `done`

### §8.4.7.1 Table 8.54 Writer new_change

**Spec:** §8.4.7.1, p. 79 — "The sequence number of the CacheChange is
automatically set to be the sequenceNumber of the previous change plus one.
… ++this.lastChangeSequenceNumber; a_change := new CacheChange(kind,
this.guid, this.lastChangeSequenceNumber, data, inlineQos, handle);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`,
`write_lifecycle`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/writer.rs::tests::writer_sn_overflow_is_error`

**Status:** `done`

### §8.4.7.1 Default nackResponseDelay = 200 ms

**Spec:** §8.4.7.1, p. 79 — "nackResponseDelay.sec = 0;
nackResponseDelay.nanosec = 200 * 1000 * 1000; //200 milliseconds".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_RESPONSE_DELAY`, `SPEC_NACK_RESPONSE_DELAY`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::nack_response_delay`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_NACK_RESPONSE_DELAY` and `RuntimeConfig::nack_response_delay` are 200 ms; a NACK is answered with the repair only after that time (test). `writer.nack_response_delay_ms: 0` answers on the next tick.

### §8.4.7.1 Default nackResponseDelay

**Spec:** §8.4.7.1, p. 79 — "The following timing-related values are used as
the defaults in order to facilitate 'out-of-the-box' interoperability between
implementations."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_RESPONSE_DELAY`, `DEFAULT_NACK_SUPPRESSION_DURATION`);
`crates/dcps/src/runtime.rs`; `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`

**Status:** `done` — the values of the spec are the defaults: nackResponseDelay 200 ms, nackSuppressionDuration 0 (runtime, YAML default, man page, example configuration). The former 0 ms stay reachable through `writer.nack_response_delay_ms`; tests that drive a millisecond clock set 0 explicitly.

### §8.4.7.1 Default nackSuppressionDuration = 0

**Spec:** §8.4.7.1, p. 79 — "nackSuppressionDuration.sec = 0;
nackSuppressionDuration.nanosec = 0;".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`DEFAULT_NACK_SUPPRESSION_DURATION`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::nack_response_delay_defaults_to_the_spec_200_ms_and_suppression_to_zero`

**Status:** `done`

### §8.4.7.2 Table 8.55 StatelessWriter reader_locators

**Spec:** §8.4.7.2, p. 80, Table 8.55 — "The StatelessWriter maintains the
list of locators to which it sends the CacheChanges. This list may include
both unicast and multicast locators."

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::reader_locators`,
`ReaderLocator`); `crates/discovery/src/spdp.rs` (`SpdpBeacon`, target locators)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — the list consists of `ReaderLocator` objects with state of their own (`highest_sent_seq_num`, requested changes, `expects_inline_qos`).

### §8.4.7.2 Table 8.56 StatelessWriter new

**Spec:** §8.4.7.2, p. 80 — "this.reader_locators := <empty>;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs`
(`ReliableStatelessWriter::new`: `reader_locators` leer)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::shutdown_clears_the_cache_and_the_requests`

**Status:** `done`

### §8.4.7.2 Table 8.56 StatelessWriter reader_locator_add

**Spec:** §8.4.7.2, p. 80 — "ADD a_locator TO {this.reader_locators};"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)
`reader_locator_add`

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — a locator is added as a ReaderLocator with `expectsInlineQos`, once per locator. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.7.2 Table 8.56 StatelessWriter reader_locator_remove

**Spec:** §8.4.7.3, p. 81 — "REMOVE a_locator FROM {this.reader_locators};"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::reader_locator_remove`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — a ReaderLocator is removed; removing it again reports `false`.

### §8.4.7.2 Table 8.56 StatelessWriter unsent_changes_reset

**Spec:** §8.4.7.2, p. 81 — "This operation resets the 'highestSentChangeSN'
for all the ReaderLocators … useful when called periodically to cause the
StatelessWriter to keep re-sending all available changes".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter::unsent_changes_reset`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_reset_makes_every_change_unsent_again`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — `unsent_changes_reset` sets `highestSentChangeSN` of all ReaderLocators to 0; the next tick sends all changes of the cache again to every locator. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.7.3 Table 8.57 ReaderLocator attributes

**Spec:** §8.4.7.3, p. 81, Table 8.57 — highestSentChangeSN,
requestedChanges, locator, expectsInlineQos.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — `ReaderLocator` holds `locator`, `expects_inline_qos`, `highest_sent_seq_num` and the requested changes. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.7.3 Table 8.58 ReaderLocator operations

**Spec:** §8.4.7.3, pp. 81–82, Table 8.58 — new, next_requested_change,
next_unsent_change, requested_changes, requested_changes_set,
unsent_changes.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — `requested_changes`, `requested_changes_set`, `next_requested_change`, `unsent_changes` are methods of `ReaderLocator`; `next_unsent_change` is in the push machine (`next_cached_above`). `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.7.4 Table 8.59 StatefulWriter matched_readers

**Spec:** §8.4.7.4, p. 83, Table 8.59 — "The StatefulWriter keeps track of all
the RTPS Readers matched with it. Each matched reader is represented by an
instance of the ReaderProxy class."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`reader_proxies`,
`reader_proxies()`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter new

**Spec:** §8.4.7.4, p. 83 — "this.matched_readers := <empty>;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::new` with
`ReliableWriterConfig::reader_proxies`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_without_proxies_is_true`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter is_acked_by_all

**Spec:** §8.4.7.4, p. 83 — "return true IF and only IF FOREACH proxy IN
this.matched_readers a_change_seq_num IN proxy.acknowledged_changes".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`is_acked_by_all`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_follows_the_slowest_reader_proxy`,
`crates/rtps/src/reliable_writer.rs::tests::is_acked_by_all_without_proxies_is_true`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_add

**Spec:** §8.4.7.4, pp. 83–84 — "ADD a_reader_proxy TO
{this.matched_readers};"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::add_reader_proxy_is_idempotent_on_same_guid`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_after_add_reader_proxy_is_non_final`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_remove

**Spec:** §8.4.7.4, p. 84 — "REMOVE a_reader_proxy FROM
{this.matched_readers}; delete proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`

**Status:** `done`

### §8.4.7.4 Table 8.60 StatefulWriter matched_reader_lookup

**Spec:** §8.4.7.4, p. 84 — "FIND proxy IN this.matched_readers SUCH-THAT
(proxy.remoteReaderGuid == a_reader_guid); return proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`,
`handle_nackfrag`: lookup by `remote_reader_guid`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::acknack_dispatches_to_matching_proxy_only`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy remoteReaderGuid

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "remoteReaderGuid: GUID_t — Identifies
the remote matched RTPS Reader that is represented by the ReaderProxy."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::remote_reader_guid`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::add_reader_proxy_is_idempotent_on_same_guid`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy remoteGroupEntityId

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "remoteGroupEntityId: EntityId_t —
Identifies the group to which the matched Reader belongs".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::remote_group_entity_id`, `with_remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (match path, `PID_GROUP_GUID` of the subscription)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — the proxy holds `remoteGroupEntityId`; the match path fills it from `PID_GROUP_GUID` of the reader's SEDP announcement, `ENTITYID_UNKNOWN` without a group.

### §8.4.7.5 Table 8.61 ReaderProxy unicastLocatorList

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "List of unicast locators … that can be
used to send messages to the matched RTPS Reader. The list may be empty."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::unicast_locators`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_every_unicast_locator_of_a_proxy`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy multicastLocatorList

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "List of multicast locators … that can
be used to send messages to the matched RTPS Reader. The list may be empty."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::multicast_locators`);
`crates/rtps/src/reliable_writer.rs` (`proxy_targets_of`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_proxy_with_multicast_locators_is_sent_to_at_the_multicast_locator`

**Status:** `done` — a proxy with multicast locators is sent to there (DATA and HEARTBEAT, one datagram for the group), one without at its unicast locators. The SEDP match path passes an empty list ("The list may be empty"); the unicast locators of the reader apply then.

### §8.4.7.5 Table 8.61 ReaderProxy highestSentChangeSN

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "The highest sequence number of the
changes that have been sent to the matched RTPS Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy requestedChanges

**Spec:** §8.4.7.5, p. 84, Table 8.61 — "A list of sequence numbers
representing changes that were requested by the matched RTPS Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy acknowledgedChanges

**Spec:** §8.4.7.5, pp. 84–85, Table 8.61 — "A list of sequence numbers
representing changes that have been acknowledged by the matched RTPS
Reader."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_acked_sn`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`

**Status:** `done` — kept as the highest acknowledged number.

### §8.4.7.5 Table 8.61 ReaderProxy expectsInlineQos

**Spec:** §8.4.7.5, p. 85, Table 8.61 — "Specifies whether the remote matched
RTPS Reader expects in-line QoS to be sent along with any data."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`expects_inline_qos`,
`set_expects_inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`

**Status:** `done`

### §8.4.7.5 Table 8.61 ReaderProxy isActive

**Spec:** §8.4.7.5, p. 85, Table 8.61 — "isActive: bool — Specifies whether the
remote Reader is responsive to the Writer."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`note_activity`, `is_inactive`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::proxy_becomes_inactive_after_threshold_elapses`,
`crates/rtps/src/reader_proxy.rs::tests::proxy_is_active_after_note_activity`

**Status:** `done`

### §8.4.7.5 Matching follows the DDS match

**Spec:** §8.4.7.5, p. 85 — "The matching is a consequence of the match of the
corresponding DDS entities. That is, the DDS DataWriter matches a DDS
DataReader by Topic, has compatible QoS, and is not being explicitly ignored
by the application that uses DDS."

**Repo:** `crates/dcps/src/runtime.rs` (SEDP match, `add_reader_proxy`);
`crates/dcps/src/participant.rs` (`ignore_subscription`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`,
`crates/dcps/tests/xtypes_aware_match.rs::final_vs_appendable_mismatch_rejected_at_match_time`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy new

**Spec:** §8.4.7.5, p. 85 — "this.requested_changes := <empty>;
this.acknowledged_changes := <empty>; this.highest_sent_seq_num := 0;"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::new`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::fresh_proxy_has_zero_state`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy acked_changes_set

**Spec:** §8.4.7.5, p. 86 — "FOR_EACH seq_num <= committed_seq_num DO ADD
seq_num TO this.acknowledged_changes".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`acked_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::acked_changes_set_monotonic`,
`crates/rtps/src/reader_proxy.rs::tests::acking_also_prunes_requested_changes`

**Status:** `done` — the parameter is `readerSNState.base`, the acknowledged
upper bound `base − 1`.

### §8.4.7.5 Table 8.62 ReaderProxy next_requested_change

**Spec:** §8.4.7.5, p. 86 — "return MIN( this.requested_changes() );"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`next_requested_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_requested_change_pulls_smallest_first`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy next_unsent_change

**Spec:** §8.4.7.5, p. 86 — "unsent_changes := { changes SUCH-THAT
change.sequenceNumber > this.higuest_sent_seq_num } … ELSE return MIN {
unsent_changes.sequenceNumber }".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`next_unsent_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_walks_sequentially`,
`crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_picks_up_after_cache_grows`

**Status:** `done` — returns `highest_sent + 1` up to the cache maximum;
`tick` answers numbers no longer present with a GAP.

### §8.4.7.5 Table 8.62 ReaderProxy requested_changes

**Spec:** §8.4.7.5, p. 86 — "return this.requested_changes".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`pending_requested_count`,
`next_requested_change`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy requested_changes_set

**Spec:** §8.4.7.5, p. 86 — "FOR_EACH seq_num IN req_seq_num_set DO ADD
seq_num TO this.requested_changes;"

**Repo:** `crates/rtps/src/reader_proxy.rs` (`requested_changes_set`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::a_request_for_an_acknowledged_change_is_kept`

**Status:** `done` — takes every requested number from the ACKNACK's base on,
also one acknowledged before (§8.4.2.3).

### §8.4.7.5 Table 8.62 ReaderProxy unsent_changes

**Spec:** §8.4.7.5, p. 86 — "return ( this.next_unsent_change() !=
SEQUENCE_NUMBER_INVALID )".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`highest_sent_sn` against the
cache maximum)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::next_unsent_change_picks_up_after_cache_grows`

**Status:** `done`

### §8.4.7.5 Table 8.62 ReaderProxy unacked_changes

**Spec:** §8.4.7.5, p. 86 — "return ( highest_available_seq_num >
highest_acked_seq_num )".

**Repo:** `crates/rtps/src/reader_proxy.rs` (`unacked_changes`)

**Tests:** `crates/rtps/src/reader_proxy.rs::tests::unacked_changes_detects_gap`

**Status:** `done`

### §8.4.8.1 Table 8.63 best-effort StatelessWriter T1

**Spec:** §8.4.8.1, pp. 87–88, Table 8.63 T1 — "a_locator := new
ReaderLocator( locator, expectsInlineQos ); the_rtps_writer.reader_locator_add(
a_locator );"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::initial_peers`, the SPDP
multicast locator)

**Tests:** `crates/dcps/src/runtime.rs::tests::initial_peer_specs_expand_per_the_configured_participant_limit`,
`crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done` — the SPDP beacon is ZeroDDS' best-effort StatelessWriter;
its target locators come from the configuration (§8.5.3.1).

### §8.4.8.1 Table 8.63 best-effort StatelessWriter T2

**Spec:** §8.4.8.1, p. 88 — "triggered by the guard condition
[RL::unsent_changes() == true] … The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action; T4 is
what is observable.

### §8.4.8.1 Table 8.63 best-effort StatelessWriter T3

**Spec:** §8.4.8.1, p. 88 — "[RL::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.8.1 Table 8.63 best-effort StatelessWriter T4

**Spec:** §8.4.8.1, p. 88 — next unsent change; GAP for skipped numbers;
"DATA = new DATA(a_change); IF (the_reader_locator.expectsInlineQos) {
DATA.inlineQos := … } DATA.readerId := ENTITYID_UNKNOWN; sendto
the_reader_locator.locator, DATA;"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`: one DATA
with a consecutive number per announcement)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done` — the DATA carries `readerId =
ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR` instead of UNKNOWN (§8.3.8.2
allows either); since every number is sent, no GAP arises.

### §8.4.8.1 Table 8.63 best-effort StatelessWriter T5

**Spec:** §8.4.8.1, pp. 88–89 — "the_rtps_writer.reader_locator_remove(
the_reader_locator); delete the_reader_locator;"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — for SPDP the locators are configured and
fixed (§8.5.3.1); a removal by discovery does not occur.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T1

**Spec:** §8.4.8.2, p. 90, Table 8.64 T1 — "a_locator := new ReaderLocator(
locator, expectsInlineQos ); the_rtps_writer.reader_locator_add( a_locator );"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — `reader_locator_add(locator, expects_inline_qos)`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T2

**Spec:** §8.4.8.2, p. 90 — "[RL::unsent_changes() == true] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — unsent changes put the push machine in `Pushing`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T3

**Spec:** §8.4.8.2, p. 90 — "[RL::unsent_changes == false] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`

**Status:** `done` — with no unsent changes the push machine goes to `Announcing`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T4

**Spec:** §8.4.8.2, p. 91 — send the next unsent change as DATA with
`readerId := ENTITYID_UNKNOWN`, a GAP for skipped numbers; postcondition
`higuest_sent_seq_num == a_change_seq_num`.

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::unsent_changes_are_pushed_in_order_then_announced`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::a_gap_precedes_the_data_after_a_hole_in_the_cache`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_related_qos_goes_out_with_the_data_only_where_it_is_expected`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_resend_per_tick_is_bounded`

**Status:** `done` — the next unsent change goes as DATA with `readerId = ENTITYID_UNKNOWN` to the locator, preceded by a GAP with `filteredCount = 0` over the missing numbers; inline QoS of the related DDS writer only with `expectsInlineQos`; `RL::can_send()` is the bound `max_per_tick`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T5

**Spec:** §8.4.8.2, p. 91 — periodically "HEARTBEAT := new
HEARTBEAT(the_rtps_writer.writerGuid, seq_num_min, seq_num_max);
HEARTBEAT.FinalFlag := SET; HEARTBEAT.readerId := ENTITYID_UNKNOWN;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_announcing_locator_gets_a_final_heartbeat_each_period`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::a_heartbeat_of_an_empty_cache_has_first_after_last`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_heartbeat_count_wraps_at_u32_max`

**Status:** `done` — HEARTBEAT with FinalFlag set and `readerId = ENTITYID_UNKNOWN` each `heartbeatPeriod` to an announcing locator; an empty cache announces `firstSN = lastSN + 1`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T6

**Spec:** §8.4.8.2, pp. 91–92 — "FOREACH reply_locator_t IN {
Receiver.unicastReplyLocatorList, Receiver.multicastReplyLocatorList }
reader_locator := the_rtps_writer.reader_locator_lookup(reply_locator_t);
reader_locator.requested_changes_set(ACKNACK.readerSNState.set);"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_without_a_usable_reply_locator_is_counted`

**Status:** `done` — an ACKNACK sets `requested_changes_set(readerSNState)` for each reply locator of the receiver state that names a port and is a ReaderLocator; an ACKNACK without a usable reply locator (no InfoReply) is counted (`unmatched_acknacks`). `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T7

**Spec:** §8.4.8.2, p. 92 — "[RL::requested_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — requested changes put the repair machine in `MustRepair`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T8

**Spec:** §8.4.8.2, p. 92 — "The transition performs the same logical actions
performed by Transition T6".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — another ACKNACK in `MustRepair` is handled as in T6. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T9

**Spec:** §8.4.8.2, p. 92 — "triggered by the firing of a timer indicating
that the duration of W::nackResponseDelay has elapsed since the state
must_repair was entered."

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — after `nackResponseDelay` (`set_nack_response_delay`) the machine goes to `Repairing`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T10 (DATA)

**Spec:** §8.4.8.2, p. 92 — "a_change_seq_num :=
the_reader_locator.next_requested_change(); … IF ( a_change != <nil> ) { DATA
= new DATA(a_change); … DATA.readerId := ENTITYID_UNKNOWN; sendto
the_reader_locator.locator, DATA; }".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`,
`crates/rtps/src/reliable_stateless_writer.rs::tests::the_related_qos_goes_out_with_the_data_only_where_it_is_expected`

**Status:** `done` — `next_requested_change` goes as DATA with `readerId = ENTITYID_UNKNOWN` to the locator; inline QoS of the DDS writer and of the change with `expectsInlineQos`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T10 (GAP)

**Spec:** §8.4.8.2, p. 92 — "ELSE { GAP = new GAP(a_change.sequenceNumber);
GAP.readerId := ENTITYID_UNKNOWN; GAP.filteredCount := 0; sendto
the_reader_locator.locator, GAP; }".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::a_requested_change_that_left_the_cache_is_answered_by_a_gap`

**Status:** `done` — a requested change that is no longer in the cache is answered with a GAP (`filteredCount = 0`). `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T11

**Spec:** §8.4.8.2, p. 93 — "[RL::requested_changes() == <empty>] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::an_acknack_is_answered_at_its_reply_locator_after_the_delay`

**Status:** `done` — with no requested changes the machine goes to `Waiting`. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.8.2 Table 8.64 reliable StatelessWriter T12

**Spec:** §8.4.8.2, p. 93 — "the_rtps_writer.reader_locator_remove(
the_reader_locator); delete the_reader_locator;"

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs` (`ReliableStatelessWriter`, `ReaderLocator`)

**Tests:** `crates/rtps/src/reliable_stateless_writer.rs::tests::reader_locators_are_added_looked_up_and_removed`

**Status:** `done` — `reader_locator_remove` deletes the ReaderLocator. `ReliableStatelessWriter` is used by no production path; no vendor is measured.

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T1

**Spec:** §8.4.9.1, p. 94, Table 8.65 T1 — "a_reader_proxy := new ReaderProxy(
remoteReaderGuid, remoteGroupEntityId, expectsInlineQos, unicastLocatorList,
multicastLocatorList); the_rtps_writer.matched_reader_add(a_reader_proxy);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy` with
`ReaderProxy::new(.., is_reliable: false)`); `crates/rtps/src/reader_proxy.rs` (`remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (match path)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_gets_the_next_write`,
`crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_reader_add` takes `remoteGroupEntityId`, `expectsInlineQos` and both locator lists.

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T2

**Spec:** §8.4.9.1, p. 94 — "[RP::unsent_changes() == true] … Note that for a
Best-Effort Writer, W::pushMode == true … The transition performs no logical
actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T3

**Spec:** §8.4.9.1, p. 94 — "[RP::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T4

**Spec:** §8.4.9.1, pp. 94–95 — next unsent change; a GAP with filteredCount
0 for skipped numbers; DATA if DDS_FILTER passes, otherwise a GAP with
filteredCount 1; in-line QoS on expectsInlineQos; postcondition
`higuest_sent_seq_num == a_change_seq_num`.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`,
`tick`, `append_change`, `is_relevant`, `passes_dds_filter`, `gap_filtered_count`);
`crates/rtps/src/reader_proxy.rs` (`ChangeFilter`, `with_filter`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_gets_the_next_write`,
`crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds`,
`crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_behind_is_pushed_gaps_with_the_filtered_count`,
`crates/rtps/src/reliable_writer.rs::tests::a_change_that_fails_the_readers_filter_is_a_gap_with_filtered_count_one`,
`crates/rtps/src/reliable_writer.rs::tests::a_gap_for_a_number_the_cache_no_longer_holds_has_filtered_count_zero`,
`crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done` — a proxy in step gets every new change as DATA; one that is behind gets every change not yet sent as DATA in `tick`, a number the cache no longer holds as a GAP with `filteredCount` 0. If a change fails `DDS_FILTER(the_reader_proxy, a_change)` (`ReaderProxy::with_filter`), a GAP with `filteredCount` 1 goes out instead of the DATA. The runtime sets no filter (`DDS_FILTER` may be a no-op, §8.2.10.1); the behaviour against Cyclone DDS, Fast DDS and OpenDDS is not measured.

### §8.4.9.1 Best-effort StatefulWriter: lagging proxy

**Spec:** §8.4.9.1, p. 95 — T4 sends `next_unsent_change()` to every proxy,
"if ( a_change_seq_num > the_reader_proxy.higuest_sent_seq_num +1 ) { GAP = …
}".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `next_unsent_change` per
best-effort proxy, `append_change`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_best_effort_reader_matched_late_is_pushed_what_the_cache_holds`

**Status:** `done` — a lagging best-effort proxy gets the changes not yet sent
to it in order, unasked; a number the cache no longer holds as a GAP.

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T5

**Spec:** §8.4.9.1, pp. 95–96 — "ADD a_change TO the_rtps_writer.writer_cache;"
postcondition "FOREACH proxy IN the_rtps_writer.matched_readers
proxy.unsent_changes() == true".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.9.1 Table 8.65 best-effort StatefulWriter T6

**Spec:** §8.4.9.1, p. 96 — "the_rtps_writer.matched_reader_remove(
the_reader_proxy); delete the_reader_proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T1

**Spec:** §8.4.9.2, p. 98, Table 8.66 T1 — "a_reader_proxy := new ReaderProxy(
remoteReaderGuid, remoteGroupEntityId, expectsInlineQos, unicastLocatorList,
multicastLocatorList); the_rtps_writer.matched_reader_add(a_reader_proxy);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`add_reader_proxy`);
`crates/rtps/src/reader_proxy.rs` (`remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (match path, `skip_samples_up_to` for VOLATILE)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::heartbeat_after_add_reader_proxy_is_non_final`,
`crates/rtps/src/reliable_writer.rs::tests::a_reader_matched_inside_an_open_coherent_set_gets_the_whole_set`,
`crates/rtps/src/reader_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_reader_add` takes `remoteGroupEntityId`, `expectsInlineQos` and both locator lists.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T2

**Spec:** §8.4.9.2, pp. 98–99 — "[RP::unsent_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T3

**Spec:** §8.4.9.2, p. 99 — "[RP::unsent_changes() == false] … The transition
performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T4

**Spec:** §8.4.9.2, pp. 99–100 — the next unsent change as DATA (or a GAP if
DDS_FILTER = false, filteredCount 1), a GAP for numbers removed from the
cache (filteredCount 0); "DATA.readerId := ENTITYID_UNKNOWN; // or
ReaderProxy.entityId".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`,
`write_with_heartbeat_stamped`; `tick` for lagging proxies)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_increments_sn_and_returns_data_datagram`,
`crates/rtps/src/reliable_writer.rs::tests::repairs_in_order_bring_a_lagging_proxy_into_step`,
`crates/rtps/src/reliable_writer.rs::tests::a_reliable_proxy_behind_the_cache_is_pushed_into_step`,
`crates/rtps/src/reliable_writer.rs::tests::a_repair_out_of_order_is_followed_by_the_changes_before_it`,
`crates/rtps/src/reliable_writer.rs::tests::a_reader_asking_for_a_replaced_change_gets_the_current_one`

**Status:** `done` — `write` sends a change at once to the proxies in step;
to a lagging proxy (a late joiner, or one whose request a GAP answered)
`tick` under pushMode sends the unsent changes in order, DATA for those in
the cache, GAP for those removed. A change repaired out of order in the same
round does not go out twice.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T5

**Spec:** §8.4.9.2, p. 100 — "[RP::unacked_changes() == false] … The
transition performs no logical actions".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: no periodic HEARTBEAT
without unacknowledged changes)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_acknowledged_the_cache_gets_no_periodic_heartbeat`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T6

**Spec:** §8.4.9.2, p. 100 — "[RP::unacked_changes() == true] … The transition
performs no logical actions".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `announcing`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_heartbeat_sent_with_a_change_is_answered_by_the_next_acknack`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T7

**Spec:** §8.4.9.2, p. 100 — "HEARTBEAT := new
HEARTBEAT(the_rtps_writer.writerGuid, seq_num_min, seq_num_max);
HEARTBEAT.FinalFlag := NOT_SET; HEARTBEAT.readerId := ENTITYID_UNKNOWN; send
HEARTBEAT;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`, `append_heartbeat`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::periodic_heartbeat_has_final_flag_unset`,
`crates/rtps/src/reliable_writer.rs::tests::heartbeat_carries_cache_range`

**Status:** `done` — the HEARTBEAT goes per proxy with its `readerId`;
`firstSN` is `max(seq_num_min, highest_acked + 1)` of the proxy.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T8

**Spec:** §8.4.9.2, p. 100 — "the_rtps_writer.acked_changes_set(
ACKNACK.readerSNState.base - 1); the_reader_proxy.requested_changes_set(
ACKNACK.readerSNState.set);"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::handle_acknack_updates_proxy_state`,
`crates/rtps/src/reliable_writer.rs::tests::handle_acknack_with_lower_base_leaves_requested`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T9

**Spec:** §8.4.9.2, p. 100 — "[RP::requested_changes() != <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T10

**Spec:** §8.4.9.2, p. 101 — "The transition performs the same logical actions
as Transition T8".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`handle_acknack_window`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_full_nack_window_gets_a_heartbeat_with_its_repair`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T11

**Spec:** §8.4.9.2, p. 101 — "triggered by the firing of a timer indicating
that the duration of W::nackResponseDelay has elapsed since the state
must_repair was entered."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`repair_due`, `note_nack`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_repair_waits_for_the_nack_response_delay`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T12

**Spec:** §8.4.9.2, pp. 101–102 — the requested change from the cache as DATA
(or a GAP with filteredCount 1 if DDS_FILTER = false), otherwise "GAP = new
GAP(a_change.sequenceNumber, the_reader_proxy.remoteReaderGuid);
GAP.filteredCount := 0;"; postcondition: the change is no longer requested.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`tick`: `append_data`,
`append_lifecycle`, `append_gap`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::tick_resends_requested_as_data_aggregated_with_hb`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`,
`crates/rtps/src/reliable_writer.rs::tests::lifecycle_repair_reproduces_the_key`

**Status:** `done` — the GAP carries no filteredCount field; that equals a
change not counted.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T13

**Spec:** §8.4.9.2, p. 102 — "[RP::requested_changes() == <empty>] … The
transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.9.2 Table 8.66 reliable StatefulWriter T14

**Spec:** §8.4.9.2, p. 102 — "ADD a_change TO the_rtps_writer.writer_cache;"
postcondition: every proxy has unsent changes.

**Repo:** `crates/rtps/src/reliable_writer.rs` (`stage_sample`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_fans_out_to_all_reader_proxies`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T15

**Spec:** §8.4.9.2, p. 102 — "triggered by the removal of a CacheChange
'a_change' from the HistoryCache of the RTPS Writer".

**Repo:** `crates/rtps/src/reliable_writer.rs` (KEEP_LAST eviction in
`stage_sample`, `remove_acknowledged`, `remove_samples_up_to`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keep_last_stalled_reader_does_not_block_fresh_writes`,
`crates/rtps/src/reliable_writer.rs::tests::tick_resends_evicted_request_as_gap`

**Status:** `done`

### §8.4.9.2 Table 8.66 reliable StatefulWriter T16

**Spec:** §8.4.9.2, p. 102 — "the_rtps_writer.matched_reader_remove(
the_reader_proxy); delete the_reader_proxy;"

**Repo:** `crates/rtps/src/reliable_writer.rs` (`remove_reader_proxy`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::remove_reader_proxy_by_guid`,
`crates/rtps/src/reliable_writer.rs::tests::the_unacked_window_fills_and_acknowledgements_free_it`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader heartbeatResponseDelay

**Spec:** §8.4.10.1, p. 103, Table 8.67 — "heartbeatResponseDelay: Protocol
tuning parameter that allows the RTPS Reader to delay the sending of a
positive or negative acknowledgment".

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`ReliableReaderConfig::heartbeat_response_delay`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`

**Status:** `done` — its tunability for the user is listed under §8.4.2.1.

### §8.4.10.1 Table 8.67 Reader heartbeatSuppressionDuration

**Spec:** §8.4.10.1, p. 103, Table 8.67 — "heartbeatSuppressionDuration:
Protocol tuning parameter that allows the RTPS Reader to ignore HEARTBEATs
that arrive 'too soon' after a previous HEARTBEAT was received."

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`set_heartbeat_suppression_duration`, `handle_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_suppression_duration_ignores_a_heartbeat_that_arrives_too_soon`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader reader_cache

**Spec:** §8.4.10.1, p. 103, Table 8.67 — "reader_cache: History Cache —
Contains the history of CacheChange changes for this RTPS Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`WriterProxyState::received_cache`);
`crates/dcps/src/subscriber.rs` (the DataReader's sample store)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`,
`crates/rtps/src/reliable_reader.rs::tests::a_full_cache_takes_the_change_in_front_of_what_it_holds`

**Status:** `done`

### §8.4.10.1 Table 8.67 Reader expectsInlineQos

**Spec:** §8.4.10.1, p. 104, Table 8.67 — "expectsInlineQos: bool — Specifies
whether the RTPS Reader expects in-line QoS to be sent along with any data."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::expects_inline_qos`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §8.4.10.1 Default heartbeatResponseDelay = 500 ms

**Spec:** §8.4.10.1, p. 104 — "heartbeatResponseDelay.sec = 0;
heartbeatResponseDelay.nanosec = 500 * 1000 * 1000; // 500 milliseconds".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DEFAULT_HEARTBEAT_RESPONSE_DELAY`, `SPEC_HEARTBEAT_RESPONSE_DELAY`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::reader_heartbeat_response_delay`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_HEARTBEAT_RESPONSE_DELAY` and `RuntimeConfig::reader_heartbeat_response_delay` are 500 ms; `reader.heartbeat_response_delay_ms: 0` answers on the next tick.

### §8.4.10.1 Default heartbeatResponseDelay

**Spec:** §8.4.10.1, p. 104 — "The following timing-related values are used as
the defaults in order to facilitate 'out-of-the-box' interoperability between
implementations."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`DEFAULT_HEARTBEAT_RESPONSE_DELAY`, `DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`);
`crates/dcps/src/runtime.rs`; `crates/dcps/src/type_lookup_transport.rs`; `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`,
`crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`

**Status:** `done` — the values of the spec are the defaults: heartbeatResponseDelay 500 ms, heartbeatSuppressionDuration 0 (runtime, man page, example configuration). The former 0 ms stay reachable through `reader.heartbeat_response_delay_ms`. §8.4.7.1 names no heartbeatPeriod; it stays 100 ms (`writer.heartbeat_period_ms`). The built-in readers of SEDP, secure volatile and TypeLookup use the same default.

### §8.4.10.1 Default heartbeatSuppressionDuration = 0

**Spec:** §8.4.10.1, p. 104 — "heartbeatSuppressionDuration.sec = 0;
heartbeatSuppressionDuration.nanosec = 0;".

**Repo:** `crates/rtps/src/reliable_reader.rs`
(`DEFAULT_HEARTBEAT_SUPPRESSION_DURATION`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::default_heartbeat_suppression_duration_is_the_spec_zero`,
`crates/rtps/src/reliable_reader.rs::tests::heartbeat_storm_from_a_writer_that_never_fulfils_a_nack_cannot_arise`,
`crates/discovery/src/sedp/reader.rs::tests::sedp_reader_does_not_storm_against_a_writer_that_never_delivers`

**Status:** `done` — every reader runs with 0, the built-in ones of SEDP and
VolatileSecure too. A writer that re-announces the same range at short
intervals (HDDS 1.4.8: a HEARTBEAT `[1,1]` about every 1.5 ms) gets one
response per heartbeatResponseDelay: the delay runs from the first HEARTBEAT
that asks for a response.

### §8.4.10.1 Table 8.68 Reader new

**Spec:** §8.4.10.1, p. 104 — takes over guid, locator lists,
reliabilityLevel, topicKind, expectsInlineQos, heartbeatResponseDelay;
"this.reader_cache := new HistoryCache;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::new`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`

**Status:** `done`

### §8.4.10.2 Table 8.69 StatelessReader new

**Spec:** §8.4.10.2, pp. 104–105 — "The RTPS StatelessReader has no knowledge
of the number of matched writers, nor does it maintain any state for each
matched RTPS Writer."

**Repo:** `crates/rtps/src/reader.rs` (`BestEffortReader::new`);
`crates/discovery/src/spdp.rs` (`SpdpReader::new`)

**Tests:** `crates/rtps/src/reader.rs::tests::reader_delivers_data_addressed_to_us`,
`crates/discovery/src/spdp.rs::tests::reader_rejects_non_spdp_datagram`

**Status:** `done`

### §8.4.10.3 Table 8.70 StatefulReader matched_writers

**Spec:** §8.4.10.3, p. 105, Table 8.70 — "matched_writers: WriteProxy[*] —
Used to maintain state on the remote Writers matched up with the Reader."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`writer_proxies`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::two_writers_with_overlapping_sn_spaces_both_delivered`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader new

**Spec:** §8.4.10.3, pp. 105–106 — "this.matched_writers := <empty>;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`ReliableReader::new`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::no_pre_emptive_acknack_without_proxy`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_add

**Spec:** §8.4.10.3, p. 106 — "ADD a_writer_proxy TO {this.matched_writers};"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`,
`crates/rtps/src/reliable_reader.rs::tests::re_adding_known_writer_preserves_reliability_state`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_remove

**Spec:** §8.4.10.3, p. 106 — "REMOVE a_writer_proxy FROM
{this.matched_writers}; delete a_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`,
`depart_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_departed_writer_leaves_its_held_samples_with_the_reader`

**Status:** `done`

### §8.4.10.3 Table 8.71 StatefulReader matched_writer_lookup

**Spec:** §8.4.10.3, p. 106 — "FIND proxy IN this.matched_writers SUCH-THAT
(proxy.remoteWriterGuid == a_writer_guid); return proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`proxy_index_by_writer`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.4.10.4 The WriterProxy follows the DDS match

**Spec:** §8.4.10.4, p. 106 — "The association is a consequence of the
matching of the corresponding DDS Entities … the DDS DataReader matching a DDS
DataWriter by Topic, having compatible QoS, belonging to a common partition,
and not being explicitly ignored by the application that uses DDS."

**Repo:** `crates/dcps/src/runtime.rs` (match path, `add_writer_proxy`);
`crates/dcps/src/participant.rs` (`ignore_publication`)

**Tests:** `crates/dcps/src/runtime.rs::tests::partition_mismatch_blocks_match_without_any_incompatible_qos_event`,
`crates/dcps/src/participant.rs::tests::ignore_publication_records_handle`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy remoteWriterGuid

**Spec:** §8.4.10.4, p. 106, Table 8.72 — "remoteWriterGuid: GUID_t —
Identifies the matched Writer."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::remote_writer_guid`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::same_entity_id_different_prefix_not_confused`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy unicastLocatorList

**Spec:** §8.4.10.4, p. 106, Table 8.72 — "List of unicast (address, port)
combinations that can be used to send messages to the matched Writer or
Writers. The list may be empty."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::unicast_locators`);
`crates/rtps/src/reliable_reader.rs` (`tick_outbound`: ACKNACK to these
locators)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::tick_emits_one_acknack_per_writer_with_missing`

**Status:** `done`

### §8.4.10.4 Table 8.72 WriterProxy multicastLocatorList

**Spec:** §8.4.10.4, p. 106, Table 8.72 — "List of multicast (address, port)
combinations that can be used to send messages to the matched Writer or
Writers. The list may be empty."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::multicast_locators`,
`refresh_locators`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::re_adding_known_writer_preserves_reliability_state`

**Status:** `done` — the list is kept; ACKNACKs go to the unicast locators.

### §8.4.10.4 Table 8.72 WriterProxy dataMaxSizeSerialized

**Spec:** §8.4.10.4, p. 106, Table 8.72 — "Optional attribute that indicates
the maximum size of any SerializedPayload that may be sent by the matched
Writer."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`set_writer_data_max_size`, `oversize_payload_count`);
`crates/dcps/src/runtime.rs` (wiring when the writer proxy is created, `reader.honor_data_max_size_serialized`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_payload_beyond_the_announced_maximum_is_refused_and_counted`,
`crates/dcps/tests/runtime_config_layering.rs::the_data_max_size_setting_is_on_by_default_and_read_from_yaml`

**Status:** `done` — the reader takes the attribute from the announcement and refuses a SerializedPayload beyond it (after a protected payload is opened; the number counts as not available and the counter `oversize_payload_count` rises). A DATA_FRAG sample is checked once reassembled; `AssemblerCaps` still bounds the size of reassembled samples. The named setting `reader.honor_data_max_size_serialized=false` takes the payload anyway, for a writer that exceeds its own announcement (Fast DDS 3.6.1: 160 octets announced, 100 036 sent).

### §8.4.10.4 Table 8.72 WriterProxy changes_from_writer

**Spec:** §8.4.10.4, p. 106, Table 8.72 — "List of CacheChange changes
received or expected from the matched RTPS Writer."

**Repo:** `crates/rtps/src/writer_proxy.rs` (`known_up_to`, `received`,
`irrelevant`, `first_available_sn`, `last_available_sn`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::the_known_run_answers_as_the_sets_of_every_sn_did`

**Status:** `done` — modelled as counters and sets (§8.4.15.1).

### §8.4.10.4 Table 8.72 WriterProxy remoteGroupEntityId

**Spec:** §8.4.10.4, p. 107, Table 8.72 — "remoteGroupEntityId: EntityId_t —
Identifies the group to which the matched Reader belongs".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::remote_group_entity_id`, `with_remote_group_entity_id`);
`crates/dcps/src/runtime.rs` (match path, `PID_GROUP_GUID` of the publication)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — the proxy holds `remoteGroupEntityId`; the match path fills it from `PID_GROUP_GUID` of the writer's SEDP announcement, `ENTITYID_UNKNOWN` without a group.

### §8.4.10.4 Table 8.73 WriterProxy new

**Spec:** §8.4.10.4, p. 107 — "this.changes_from_writer := <all past and future
samples from the writer>; … initialized to UNKNOWN".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`WriterProxy::new`:
`first_available_sn = 1`, `last_available_sn = 0`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::fresh_proxy_has_no_missing`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy available_changes_max

**Spec:** §8.4.10.4, p. 108 — "seq_num := MAX { change.sequenceNumber
SUCH-THAT ( change IN this.changes_from_writer AND ( change.status ==
RECEIVED OR change.status == NOT_AVAILABLE) ) }" provided that no smaller
number is MISSING or UNKNOWN.

**Repo:** `crates/rtps/src/writer_proxy.rs` (`available_changes_max`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::available_changes_max_stops_at_the_first_hole`,
`crates/rtps/src/writer_proxy.rs::tests::available_changes_max_passes_over_an_irrelevant_change`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy not_available_change_set

**Spec:** §8.4.10.4, p. 108 — "IF ( filteredCount == COUNT(this.
a_seq_num_set) ) THEN change.status := NA_FILTERED; ELSE IF ( filteredCount ==
0) change.status := NA_REMOVED; ELSE change.status := NA_UNSPECIFIED;"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`NotAvailableKind`, `WriterProxy::not_available_change_set`,
`not_available_count`, `change_status`); `crates/rtps/src/reliable_reader.rs` (`handle_gap`, `not_available_count`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`,
`crates/rtps/src/writer_proxy.rs::tests::change_status_is_kept_per_change_with_the_sub_kind`

**Status:** `done` — `filteredCount` equal to the size of the set gives NA_FILTERED, 0 gives NA_REMOVED, anything between gives NA_UNSPECIFIED; the status is kept per change. A GAP without the FilteredCountFlag counts as 0 (assumption: the field is missing from peers before 2.5, whose GAP reports removed changes only; not measured against a vendor). A number already known is not counted or changed again.

### §8.4.10.4 Table 8.73 WriterProxy lost_changes_update

**Spec:** §8.4.10.4, p. 108 — "FOREACH change IN this.changes_from_writer
SUCH-THAT ( change.status == UNKNOWN OR change.status == MISSING AND seq_num
< first_available_seq_num ) DO { change.status := NA_REMOVED; }".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`lost_changes_update`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_counts_what_was_neither_received_nor_irrelevant`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_retires_everything_below_the_new_base`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy missing_changes

**Spec:** §8.4.10.4, p. 109 — "return { change IN this.changes_from_writer
SUCH-THAT change.status == MISSING};"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`missing_changes`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::missing_changes_respects_max_count`,
`crates/rtps/src/writer_proxy.rs::tests::received_removes_from_missing`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy missing_changes_update

**Spec:** §8.4.10.4, p. 109 — "FOREACH change IN this.changes_from_writer
SUCH-THAT ( change.status == UNKNOWN AND seq_num <= last_available_seq_num )
DO { change.status := MISSING; }".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`update_from_heartbeat`:
`last_available_sn`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::heartbeat_sets_available_range`

**Status:** `done`

### §8.4.10.4 Table 8.73 WriterProxy received_change_set

**Spec:** §8.4.10.4, p. 109 — "FIND change FROM this.changes_from_writer
SUCH-THAT change.sequenceNumber == a_seq_num; change.status := RECEIVED".

**Repo:** `crates/rtps/src/writer_proxy.rs` (`received_change_set`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::received_removes_from_missing`,
`crates/rtps/src/writer_proxy.rs::tests::received_before_first_is_ignored`

**Status:** `done`

### §8.4.10.5 Table 8.74 ChangeFromWriter status

**Spec:** §8.4.10.5, pp. 109–110, Table 8.74 — "status: ChangeFromWriter
StatusKind — Indicates the status of a CacheChange relative to the RTPS
Writer represented by the WriterProxy." Values UNKNOWN, MISSING, RECEIVED,
NOT_AVAILABLE with the sub-kinds NA_FILTERED, NA_REMOVED, NA_UNSPECIFIED.

**Repo:** `crates/rtps/src/writer_proxy.rs` (`ChangeFromWriterStatus`, `change_status`, `is_known`, `missing_changes`,
`not_available_count`)

**Tests:** `crates/rtps/src/writer_proxy.rs::tests::change_status_is_kept_per_change_with_the_sub_kind`,
`crates/rtps/src/writer_proxy.rs::tests::lost_changes_update_sets_na_removed_per_change`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`

**Status:** `done` — the four main values and the three sub-kinds of NOT_AVAILABLE are kept per change (item under §8.4.6 Table 8.52 ChangeFromWriterStatusKind).

### §8.4.10.5 Table 8.74 ChangeFromWriter is_relevant

**Spec:** §8.4.10.5, p. 109, Table 8.74 — "is_relevant: bool — Indicates
whether the change is relevant to the RTPS Reader. The determination of
relevant changes is affected by DDS DataReader TIME_BASED_FILTER QoS and also
by the use of DDS ContentFilteredTopics."

**Repo:** `crates/rtps/src/history_cache.rs` (`ChangeKind::is_relevant`,
`ChangeKind::AliveFiltered`); `crates/dcps/src/runtime.rs`
(`tbf_should_deliver`)

**Tests:** `crates/rtps/src/history_cache.rs::tests::change_kind_alive_filtered_is_alive_but_not_relevant`

**Status:** `done`

### §8.4.11.1 Table 8.75 best-effort StatelessReader T1

**Spec:** §8.4.11.1, pp. 110–111, Table 8.75 T1 — "triggered by the creation
of an RTPS StatelessReader … The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.11.1 Table 8.75 best-effort StatelessReader T2

**Spec:** §8.4.11.1, p. 111 — "a_change := new CacheChange(DATA);
the_rtps_reader.reader_cache.add_change(a_change);"

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpReader::samples`);
`crates/rtps/src/reader.rs` (`BestEffortReader::recv_datagram`)

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_reads_every_participant_a_datagram_bundles`,
`crates/rtps/src/reader.rs::tests::reader_handles_multiple_data_in_one_datagram`

**Status:** `done`

### §8.4.11.1 Table 8.75 best-effort StatelessReader T3

**Spec:** §8.4.11.1, p. 111 — "triggered by the destruction of an RTPS Reader
… The transition performs no logical actions".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.11.2 Reliable StatelessReader not supported

**Spec:** §8.4.11.2, p. 111 — "This combination is not supported by the RTPS
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement; ZeroDDS has no reliable
StatelessReader.

### §8.4.12.1 Table 8.76 best-effort StatefulReader T1

**Spec:** §8.4.12.1, p. 112, Table 8.76 T1 — "a_writer_proxy := new
WriterProxy(remoteWriterGuid, remoteGroupEntityId, unicastLocatorList,
multicastLocatorList); the_rtps_reader.matched_writer_add(a_writer_proxy);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`,
`set_best_effort`); `crates/rtps/src/writer_proxy.rs` (`remote_group_entity_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_writer_add` takes `remoteGroupEntityId` and both locator lists.

### §8.4.12.1 Table 8.76 best-effort StatefulReader T2

**Spec:** §8.4.12.1, p. 113 — "expected_seq_num :=
writer_proxy.available_changes_max() + 1; if ( a_change.sequenceNumber >=
expected_seq_num ) { … add_change(a_change); received_change_set(…); if (
a_change.sequenceNumber > expected_seq_num ) { lost_changes_update(…) } }".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`:
`gap_skipped`, `lost_changes_update` with `best_effort`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/reliable_reader.rs::tests::duplicate_data_is_rejected`,
`crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`

**Status:** `done`

### §8.4.12.1 Table 8.76 best-effort StatefulReader T3

**Spec:** §8.4.12.1, p. 113 — "the_rtps_reader.matched_writer_remove(
the_writer_proxy); delete the_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`

**Status:** `done`

### §8.4.12.1 Table 8.76 best-effort StatefulReader T4

**Spec:** §8.4.12.1, p. 113 — "the_writer_proxy.not_available_change_set(GAP.
sequence_number_set, GAP.filteredCount);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_best_effort_reader_counts_the_samples_a_gap_lost`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — the numbers of the GAP are marked not available with the sub-kind given by `filteredCount` and are kept per change.

### §8.4.12.2 Table 8.77 reliable StatefulReader T1

**Spec:** §8.4.12.2, pp. 114–115, Table 8.77 T1 — "a_writer_proxy := new
WriterProxy(remoteWriterGuid, remoteGroupEntityId, unicastLocatorList,
multicastLocatorList); the_rtps_reader.matched_writer_add(a_writer_proxy);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`add_writer_proxy`); `crates/rtps/src/writer_proxy.rs` (`remote_group_entity_id`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::add_writer_proxy_increases_count`,
`crates/rtps/src/reliable_reader.rs::tests::changes_a_writer_sent_before_the_match_are_requested_at_once`,
`crates/rtps/src/writer_proxy.rs::tests::the_remote_group_entity_id_is_kept`,
`crates/dcps/src/runtime.rs::tests::the_proxies_of_a_match_carry_the_remote_group_entity_id`

**Status:** `done` — `matched_writer_add` takes `remoteGroupEntityId` and both locator lists.

### §8.4.12.2 Table 8.77 reliable StatefulReader T2

**Spec:** §8.4.12.2, p. 114 — HEARTBEAT received: "if (HB.FinalFlag ==
NOT_SET) then must_send_ack else if (HB.LivelinessFlag == NOT_SET) then
may_send_ack else waiting".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.12.2 Table 8.77 reliable StatefulReader T3

**Spec:** §8.4.12.2, p. 115 — "[W::missing_changes() == <empty>] … The
transition performs no logical actions."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`: no ACKNACK
on F = 1 with nothing missing)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_without_missing_and_final_schedules_no_acknack`

**Status:** `done`

### §8.4.12.2 Table 8.77 reliable StatefulReader T4

**Spec:** §8.4.12.2, p. 115 — "[W::missing_changes() != <empty>] … The
transition performs no logical actions."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`:
`has_missing` sets `pending_acknack_since`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::final_heartbeat_from_unknown_to_data_writer_requests_the_missing_sn`

**Status:** `done`

### §8.4.12.2 Table 8.77 reliable StatefulReader T5

**Spec:** §8.4.12.2, pp. 115–116 — after heartbeatResponseDelay
"missing_seq_num_set.base := the_writer_proxy.available_changes_max() + 1; …
send ACKNACK(missing_seq_num_set);" — with too many missing numbers "the
subset with smallest value of the sequence number".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`tick_outbound`,
`build_acknack_datagram`); `crates/rtps/src/submessages.rs`
(`SequenceNumberSet::from_missing`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::heartbeat_with_missing_triggers_acknack_after_delay`,
`crates/rtps/src/submessages.rs::tests::snset_from_missing_spread_wide_stays_within_256_bits`,
`crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done` — base and smallest subset are right; the set holds only numbers of which no fragment has arrived (NACK_FRAG asks for the others, §8.4.14.1).

### §8.4.12.2 Table 8.77 reliable StatefulReader T6

**Spec:** §8.4.12.2, p. 116 — "Similar to T1 … The transition performs no
logical actions in the virtual machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition without a logical action.

### §8.4.12.2 Table 8.77 reliable StatefulReader T7

**Spec:** §8.4.12.2, p. 116 — "the_writer_proxy.missing_changes_update(
HEARTBEAT.lastSN); the_writer_proxy.lost_changes_update(HEARTBEAT.firstSN,
true);"

**Repo:** `crates/rtps/src/writer_proxy.rs` (`update_from_heartbeat`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_with_the_liveliness_flag_is_taken_as_a_heartbeat`

**Status:** `done`

### §8.4.12.2 Table 8.77 reliable StatefulReader T8

**Spec:** §8.4.12.2, p. 116 — "a_change := new CacheChange(DATA);
the_reader.reader_cache.add_change(a_change);
the_writer_proxy.received_change_set(a_change.sequenceNumber);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_into`,
`store_received`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`,
`crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`

**Status:** `done`

### §8.4.12.2 Table 8.77 reliable StatefulReader T9

**Spec:** §8.4.12.2, p. 116 — "the_writer_proxy.not_available_change_set(GAP.
sequence_number_set, GAP.filteredCount);"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_gap`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::gap_after_the_held_data_releases_it`,
`crates/rtps/src/reliable_reader.rs::tests::a_gap_sorts_its_changes_by_the_filtered_count`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — `filteredCount` is evaluated (item under §8.4.10.4 Table 8.73 WriterProxy not_available_change_set); the numbers are held per change as NOT_AVAILABLE.

### §8.4.12.2 Table 8.77 reliable StatefulReader T10

**Spec:** §8.4.12.2, p. 116 — "the_rtps_reader.matched_writer_remove(
the_writer_proxy); delete the_writer_proxy;"

**Repo:** `crates/rtps/src/reliable_reader.rs` (`remove_writer_proxy`,
`depart_writer_proxy`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::remove_writer_proxy_drops_its_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_departed_writer_counts_what_is_missing_in_front_of_its_held_samples`

**Status:** `done`

### §8.4.12.3 ChangeFromWriter illustrated

**Spec:** §8.4.12.3, pp. 117–118, Figure 8.25 — states Unknown, Missing,
NotRequested, Requested, Received, NotAvailable (Filtered, Removed,
Unspecified) and the events that trigger them.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — illustrates the transitions of §8.4.10.4
and §8.4.12.2 ("It is illustrative"); no requirement of its own.

### §8.4.13 Writer Liveliness Protocol: support is mandatory

**Spec:** §8.4.13, p. 118 — "All implementations must support the Writer
Liveliness Protocol in order to be interoperable."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::wlp`)

**Tests:** `crates/dcps/tests/wlp_integration.rs::linux_only::two_participants_exchange_automatic_wlp_heartbeats`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_exchange_wlp_heartbeat`,
`crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — writer and reader of the protocol run in every participant; the writer has the QoS of §8.4.13.3 and treats the reader as `builtinEndpointQos` says. The writer's wire change is not measured against Cyclone, Fast DDS, OpenDDS and RTI.

### §8.4.13.1 Association with the built-in endpoints of a discovered participant

**Spec:** §8.4.13.1, p. 118 — "once a Participant knows of the presence of
another Participant, it can assume the presence of the built-in Endpoints
made available by the remote Participant and establish the association with
the locally matching built-in Endpoints."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::add_peer_reader`);
`crates/dcps/src/runtime.rs` (`spdp_announced`, `wlp_unicast_targets`,
`answer_wlp_heartbeats`)

**Tests:** `crates/dcps/src/runtime.rs::tests::wlp_unicast_targets_prefers_metatraffic_then_default`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`,
`crates/dcps/src/wlp.rs::tests::only_a_participant_with_a_message_reader_gets_a_proxy`

**Status:** `done` — every SPDP announcement makes (unchanged: keeps) a reader proxy for the
participant's BuiltinParticipantMessageReader when its `availableBuiltinEndpoints`
name it; the target is its metatraffic locators.

### §8.4.13.1 The same protocol as for application-defined endpoints

**Spec:** §8.4.13.1, p. 118 — "The protocol used to communicate between
built-in Endpoints is the same as used for application-defined Endpoints."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`,
`crates/dcps/src/wlp.rs::tests::the_writer_heartbeats_until_the_reader_has_acknowledged`

**Status:** `done` — the WLP writer is a `ReliableWriter` with a reader proxy per participant:
DATA with a piggyback HEARTBEAT, periodic HEARTBEATs until acknowledged, repair of
what was asked for only, GAP for superseded numbers. The reader answers HEARTBEATs
(`heartbeat_responses`) and keeps the highest sequence number received per writer.

### §8.4.13.2 Built-in endpoints BuiltinParticipantMessageWriter and -Reader

**Spec:** §8.4.13.2, p. 118 — "The RTPS Protocol reserves the following values
of the EntityId_t for these built-in Endpoints:
ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER
ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER`,
`EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER`); `crates/dcps/src/wlp.rs`
(`WlpEndpoint::handle_datagram`)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`,
`crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_ignores_non_wlp_traffic`

**Status:** `done` — the values are listed under §9.3.1.3.

### §8.4.13.3 Writer durability.kind = TRANSIENT_LOCAL

**Spec:** §8.4.13.3, p. 118 — "both the BuiltinParticipantMessageWriter and
BuiltinParticipantMessageReader shall use the following QoS values:
durability.kind = TRANSIENT_LOCAL_DURABILITY".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`

**Status:** `done` — a reader that comes later gets the held samples with the first tick, without asking (the history keeps one sample per kind, so a request would meet a newer sample), and a HEARTBEAT says what the history holds; asking again it gets them once more.

### §8.4.13.3 Writer history KEEP_LAST, depth 1

**Spec:** §8.4.13.3, pp. 118–119 — "history.kind = KEEP_LAST_HISTORY_QOS",
"history.depth = 1".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_history_holds_the_last_sample_of_each_kind`

**Status:** `done` — the history holds the last sample of each instance (participantGuidPrefix,
kind; `PID_KEY_HASH` in the in-line QoS); superseded samples are not repeated.

### §8.4.13.3 Reader durability.kind = TRANSIENT_LOCAL

**Spec:** §8.4.13.3, p. 118 — "both the BuiltinParticipantMessageWriter and
BuiltinParticipantMessageReader shall use the following QoS values:
durability.kind = TRANSIENT_LOCAL_DURABILITY".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::heartbeat_responses`: the
request starts at the HEARTBEAT's firstSN, not at the match)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`,
`crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done`

### §8.4.13.3 Reader history KEEP_LAST, depth 1

**Spec:** §8.4.13.3, pp. 118–119 — "history.kind = KEEP_LAST_HISTORY_QOS",
"history.depth = 1".

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::peers`: per participant the
state last received)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`

**Status:** `done` — the reader keeps no samples; it hands every sample
received at once to the DataReaders' liveliness bookkeeping and keeps
`last_seen` and `last_kind` per participant.

### §8.4.13.3 Writer reliability.kind = RELIABLE

**Spec:** §8.4.13.3, p. 119 — "The BuiltinParticipantMessageWriter shall use
reliability.kind = RELIABLE_RELIABILITY_QOS."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint`: `make_writer`, `write_pulse`, `add_peer_reader`,
`handle_reader_messages`, `tick`); `crates/dcps/src/runtime.rs` (WLP-Tick,
`spdp_announced`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_writer_heartbeats_until_the_reader_has_acknowledged`,
`crates/dcps/src/wlp.rs::tests::a_late_reader_is_told_by_heartbeat_and_gets_the_held_samples_on_request`

**Status:** `done` — the writer keeps state per reader (reader proxy), sends HEARTBEATs until
acknowledged and repairs on ACKNACK (`handle_reader_messages`).

### §8.4.13.3 Reader RELIABLE or BEST_EFFORT

**Spec:** §8.4.13.3, p. 119 — "The BuiltinParticipantMessageReader may be
configured to use either RELIABLE_RELIABILITY_QOS or
BEST_EFFORT_RELIABILITY_QOS. If the BuiltinParticipantMessageReader is
configured to use BEST_EFFORT_RELIABILITY_QOS then the
BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER flag in
ParticipantProxy::builtinEndpointQos shall be set."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::heartbeat_responses`)

**Tests:** `crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`,
`crates/dcps/src/wlp.rs::tests::once_the_pulse_arrived_a_final_heartbeat_gets_no_answer_and_a_plain_one_the_ack`

**Status:** `done` — the reader answers HEARTBEATs as a RELIABLE reader does;
the flag does not apply, ZeroDDS sends no PID_BUILTIN_ENDPOINT_QOS.

### §8.4.13.3 The writer treats the reader by builtinEndpointQos

**Spec:** §8.4.13.3, p. 119 — "If the ParticipantProxy::builtinEndpointQos is
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

**Status:** `done` — with `builtinEndpointQos` and the flag the reader is a best-effort proxy (no HEARTBEATs), otherwise RELIABLE.

### §8.4.13.4 Figure 8.26 ParticipantMessageData

**Spec:** §8.4.13.4, p. 119, Figure 8.26 — "Figure 8.26 defines the
ParticipantMessageData datatype associated with the RTPS built-in Endpoint for
the DCPSParticipantMessage Topic." Attributes participantGuidPrefix, kind,
data.

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`, `to_cdr`, `from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_with_topic_token`

**Status:** `done`

### §8.4.13.5 AUTOMATIC: a sample faster than the shortest lease

**Spec:** §8.4.13.5, p. 119 — "If the Participant contains one or more Writers
with a liveliness of AUTOMATIC_LIVELINESS_QOS, then one sample is written at a
rate faster than the smallest lease duration among the Writers sharing this
QoS."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_wlp_period`: a third of the
shortest finite lease of an AUTOMATIC writer, at most a third of the
participant lease); `crates/dcps/src/wlp.rs` (`WlpEndpoint::tick`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`,
`crates/dcps/src/wlp.rs::tests::wlp_tick_emits_again_after_period`,
`crates/dcps/src/runtime.rs::tests::wlp_period_default_is_lease_over_three`

**Status:** `done` — the AUTOMATIC sample also goes out without an AUTOMATIC
writer.

### §8.4.13.5 MANUAL_BY_PARTICIPANT: a separate sample

**Spec:** §8.4.13.5, p. 119 — "Similarly, a separate sample is written if the
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

**Status:** `done` — the sample is written by `DomainParticipant::assert_liveliness` and by
`write`, `assert_liveliness`, `dispose` and `unregister_instance` of a
MANUAL_BY_PARTICIPANT writer; calls before the tick share one sample.

### §8.4.13.5 Two instances, key participantGuidPrefix and kind

**Spec:** §8.4.13.5, p. 119 — "The instances are distinguished using their DDS
key, which is comprised of the participantGuidPrefix and kind fields. Each of
the two types of liveliness QoS handled through this protocol will result in a
unique kind field and therefore form two distinct instances in the
HistoryCache."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::write_pulse`)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_history_holds_the_last_sample_of_each_kind`,
`crates/dcps/src/wlp.rs::tests::wlp_assert_participant_emits_manual_pulse`,
`crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`

**Status:** `done` — the instance is (participantGuidPrefix, kind); the KeyHash (12 octets of
prefix, 4 octets of kind, big endian) is `PID_KEY_HASH` in the in-line QoS of every
DATA, and the history holds one sample per instance.

### §8.4.13.5 participantGuidPrefix of the writing participant

**Spec:** §8.4.13.5, p. 119 — "In both liveliness cases the
participantGuidPrefix field contains the GuidPrefix_t of the Participant that
is writing the data (and therefore asserting the liveliness of its Writers)."

**Repo:** `crates/dcps/src/wlp.rs` (`WlpEndpoint::encode_pulse`:
`ParticipantMessageData::automatic(self.own_prefix)`)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_handle_datagram_updates_peer_state`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_automatic_default_data_empty`

**Status:** `done`

### §8.4.13.5 MANUAL_BY_TOPIC not through the BuiltinParticipantMessageWriter

**Spec:** §8.4.13.5, p. 119 — "The DDS liveliness kind
MANUAL_BY_TOPIC_LIVELINESS_QOS is not implemented using the
BuiltinParticipantMessageWriter and BuiltinParticipantMessageReader. It is
discussed in 8.7.2.2.3."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::assert_liveliness` →
`DcpsRuntime::assert_writer_liveliness_eid`: HEARTBEAT with the
LivelinessFlag)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/rtps/src/reliable_writer.rs::tests::liveliness_heartbeat_is_a_final_heartbeat_with_the_liveliness_flag`

**Status:** `done` — `DcpsRuntime::assert_writer_liveliness` also writes a
sample with the vendor kind 0x80000001; the DCPS API does not call it.

### §8.4.14 Optional behavior

**Spec:** §8.4.14, p. 119 — "Optional features may not be supported by all
RTPS implementations. An optional feature does not affect basic
interoperability, but is only available if all implementations involved
support it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — classification of the following
subclauses, no requirement.

### §8.4.14.1 Large Data: transport limits and fragment submessages

**Spec:** §8.4.14.1, p. 120 — "a transport may impose its own limitations.
For example, it may limit the maximum packet size (e.g., 64K for UDP) …
8.3.8 introduces the following Submessages to enable fragmenting large data:
DataFrag, HeartbeatFrag, NackFrag".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale, no requirement; the submessages
are listed under §8.3.8.

### §8.4.14.1 Fragment size from the writer's transports

**Spec:** §8.4.14.1, p. 120 — "All transports available to the Writer must be
able to accommodate DataFrag Submessages containing at least one fragment.
This means the transport with the smallest maximum message size determines
the fragment size." "Note the fragment size is determined by all transports
available to the Writer, not simply the subset of transports required to reach
all currently known Readers."

**Repo:** `crates/transport/src/lib.rs` (`Transport::max_message_size`, `DEFAULT_MAX_MESSAGE_SIZE`);
`crates/transport-udp/src/udp_transport.rs` (`UdpTransportConfig::max_message_size`);
`crates/transport-shm`, `transport-uds`, `transport-tcp`, `transport-tsn` (each `max_message_size`);
`crates/dcps/src/layered_transport.rs` (minimum of the legs); `crates/rtps/src/reliable_writer.rs`
(`fragment_size_for_message_size`); `crates/dcps/src/runtime.rs` (writer creation)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writers_fragment_size_follows_the_message_size_of_its_transport`,
`crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_is_bounded_by_what_the_field_holds`,
`crates/transport-udp/src/udp_transport.rs::tests::max_message_size_is_the_configured_one_up_to_the_datagram_limit`,
`crates/dcps/tests/runtime_config_layering.rs::the_udp_message_size_is_a_setting`,
`crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`

**Status:** `done` — every transport states its largest message (`Transport::max_message_size`; for UDP it is a setting, `transport.udp.max_message_size`, default 1472; SHM and UDS their datagram limit, TCP the frame limit, TSN 1500; the layered transport the minimum of its legs). The fragment size of a writer is the message of the smallest transport less 128 octets, fixed for the life of the writer: 1344 at the default.

### §8.4.14.1 Fragment size fixed per writer

**Spec:** §8.4.14.1, p. 120 — "The fragment size must be fixed for a given
Writer and is identical for all remote Readers. By fixing the fragment size,
the data a fragment number refers to does not depend on a particular remote
Reader."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::fragment_size_for`,
`needs_fragmentation`); `crates/rtps/src/reader_proxy.rs`
(`ReaderProxy::message_limit`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`,
`crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_does_not_depend_on_the_reader_expecting_inline_qos`

**Status:** `done` — the fragment size is not changed after the writer is created
(`set_fragmentation` is gone) and is the same for all readers; the in-line QoS
that shortens a fragment is counted at its largest length for any reader, so a
reader's `expectsInlineQos` does not change it either.

### §8.4.14.1 Fragment size ≤ 65536

**Spec:** §8.4.14.1, p. 120 — "The fragment size must satisfy: fragment size
<= 65536 bytes."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`MAX_FRAGMENT_SIZE`, `ReliableWriter::try_new`, `ReliableWriter::new`,
`fragment_size_for`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_fragment_size_is_bounded_by_what_the_field_holds`

**Status:** `done` — `fragmentSize` of the DATA_FRAG is an `unsigned short`; 65536 has no encoding, the bound is 65535. `try_new` refuses larger values with an error (`new` panics). A DATA_FRAG holds its 32 fixed octets, the in-line QoS and the fragment in the 65535 octets of `octetsToNextHeader`; the fragment size is accordingly at most 65503.

### §8.4.14.1 DataFrag in increasing fragment number

**Spec:** §8.4.14.1, p. 120 — "DataFrag Submessages are sent in order, where
ordering is defined by increasing fragment numbers."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`push_sample_datagrams`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`

**Status:** `done`

### §8.4.14.1 Fragment only if required (per sequence number)

**Spec:** §8.4.14.1, p. 120 — "Data must only be fragmented if required. …
Likewise, for variable size data types, a regular Data Submessage must be used
if fragmentation is not required for a particular sequence number."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`needs_fragmentation`,
`push_sample_datagrams`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_under_fragment_size_produces_single_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_mixed_data_and_data_frag`

**Status:** `done`

### §8.4.14.1 Regular Data on transports without the need to fragment

**Spec:** §8.4.14.1, p. 120 — "If multiple transports are available to the
Writer and some transports do not require fragmentation, a regular Data
Submessage must be sent on those transports instead."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::set_message_limit`);
`crates/rtps/src/reliable_writer.rs` (`needs_fragmentation`,
`unfragmented_room`); `crates/dcps/src/runtime.rs` (match of a reader on
the same host)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::fragment_size_is_fixed_and_a_larger_path_gets_one_data`

**Status:** `done` — a reader on the same host (loopback, message size `LOOPBACK_MTU`)
gets a sample that fits in one message as one DATA; a remote reader of the same
writer gets the same sample as DATA_FRAGs of the writer's size.

### §8.4.14.1 Inline QoS with the first DataFrag

**Spec:** §8.4.14.1, p. 120 — "For a given sequence number, if in-line QoS
parameters are used, they must be included with the first DataFrag Submessage
(containing the fragment with fragment number equal to 1)."

**Repo:** `crates/rtps/src/reliable_writer.rs`
(`build_data_frag_submessage_datagram`: `sample_inline_qos` on every
DATA_FRAG)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_oversized_coherent_set_member_goes_out_as_marked_fragments`,
`crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.4.14.1 Recommendation: several fragments per DataFrag

**Spec:** §8.4.14.1, p. 120 — "If a transport can accommodate multiple
fragments of the given fragment size, it is recommended that implementations
concatenate as many fragments as possible into a single DataFrag message."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`fragments_per_message`, `push_sample_datagrams`, reply to NACK_FRAG in `tick`);
`crates/rtps/src/reader_proxy.rs` (`next_requested_fragment_run_after`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_fragments_of_a_sample_fill_the_messages_and_no_more`,
`crates/rtps/src/reliable_writer.rs::tests::write_above_fragment_size_packs_the_fragments_a_message_carries`,
`crates/rtps/src/reader_proxy.rs::tests::next_requested_fragment_run_after_pulls_neighbours_of_one_sn`

**Status:** `done` — a DATA_FRAG holds as many fragments as the message to the reader carries (`fragmentsInSubmessage`); this holds for the first send and for the answer to a NACK_FRAG (neighbouring requested fragments). At the default of 1472 octets one fragment fits a message.

### §8.4.14.1 Flow control

**Spec:** §8.4.14.1, p. 121 — "When sending multiple DataFrag messages, flow
control may be required to avoid flooding the network. … This is not part of
the RTPS specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — by its text not part of the specification.

### §8.4.14.1 Reassembly: afterwards the behaviour of Data

**Spec:** §8.4.14.1, p. 121 — "Once all fragments have been received, the same
protocol behavior applies as for a regular Data Submessage."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_frag`:
`received_change_set`, `store_received`, `collect_in_order_for`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_many_samples_3_fragments_each_10_percent_loss`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_key_is_a_dispose_or_a_registration_as_a_data_is`

**Status:** `done` — acknowledgement, in-order delivery and evaluation of the in-line QoS of the first fragment (status info with D, U and F, key registration, directed write, coherent set, group) as for Data.

### §8.4.14.1 Fragments in any order

**Spec:** §8.4.14.1, p. 121 — "Note that implementations must be able to handle
out-of-order arrival of DataFrag submessages."

**Repo:** `crates/rtps/src/fragment_assembler.rs` (`FragmentAssembler::insert`)

**Tests:** `crates/rtps/src/fragment_assembler.rs::tests::fragments_complete_out_of_order`,
`crates/rtps/src/fragment_assembler.rs::tests::out_of_order_fragments_preserve_known_zero_key`

**Status:** `done`

### §8.4.14.1 Heartbeat only for numbers fully available

**Spec:** §8.4.14.1, p. 121 — "A Heartbeat message must only include those
sequence numbers for which all fragments are available."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`write_sample_with_inline_qos`:
`stage_sample` puts the whole sample into the HistoryCache before any fragment
is sent; the HEARTBEAT's `lastSN` is `last_written_sn`)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`

**Status:** `done`

### §8.4.14.1 Positive acknowledgement only after all fragments

**Spec:** §8.4.14.1, p. 121 — "an AckNack message must only positively
acknowledge a sequence number when all fragments were received for that
sequence number."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_data_frag`:
`received_change_set` only when `FragmentAssembler::insert` returns the
sample)

**Tests:** `crates/rtps/tests/reliable_e2e.rs::e2e_frag_large_sample_30_percent_loss`,
`crates/rtps/tests/reliable_e2e.rs::e2e_frag_many_samples_3_fragments_each_10_percent_loss`

**Status:** `done`

### §8.4.14.1 AckNack only for numbers missing entirely

**Spec:** §8.4.14.1, p. 121 — "Likewise, a sequence number must be negatively
acknowledged only when all fragments are missing."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done`

### §8.4.14.1 NackFrag for a subset of the fragments

**Spec:** §8.4.14.1, p. 121 — "In order to negatively acknowledge a subset of
fragments for a given sequence number, a NackFrag Submessage must be used.
When data is fragmented, a Heartbeat may trigger both AckNack and NackFrag
Submessages."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`tick_outbound`: one
NACK_FRAG per incomplete sequence number, then the ACKNACK;
`build_nackfrag_datagram`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_partly_received_sample_is_asked_for_by_nack_frag_not_acknack`

**Status:** `done`

### §8.4.14.1 HeartbeatFrag for partially available numbers

**Spec:** §8.4.14.1, p. 121 — "If a Writer wants to inform a Reader on the
partial availability of fragments for a given sequence number, a
HeartbeatFrag Submessage can be used instead."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement; the reader's answer to
HeartbeatFrag is listed under §8.3.8.7.

### §8.4.14.1 NackFrag only in response to Heartbeat or HeartbeatFrag

**Spec:** §8.4.14.1, p. 121 — "A NackFrag Submessage can only be sent in
response to a Heartbeat or HeartbeatFrag submessage."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`handle_heartbeat`,
`handle_heartbeat_frag`, `handle_data_frag_opening`, `tick_outbound`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::acknack_and_nack_frag_wait_for_a_heartbeat`,
`crates/rtps/src/reliable_reader.rs::tests::a_heartbeat_frag_is_answered_when_an_available_fragment_is_missing`

**Status:** `done` — a NACK_FRAG goes out only in the tick after a HEARTBEAT or
HEARTBEAT_FRAG; a DATA_FRAG that leaves fragments open arms none. A lost fragment
is asked for at the writer's next HEARTBEAT.

### §8.4.15 Implementation Guidelines

**Spec:** §8.4.15, p. 121 — "The contents of this sub clause are not part of
the formal specification of the protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — by its text not part of the specification.

### §8.4.15.1 Implementation of ReaderProxy and WriterProxy

**Spec:** §8.4.15.1, pp. 121–122 — "In practice, what is required is that the
ReaderProxy is able to implement the operations used by the protocol and this
does not require the use of explicit associations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.2 Efficient use of Gap and AckNack

**Spec:** §8.4.15.2, p. 122 — "implementations that combine multiple Gap and
AckNack Submessages and take advantage of the ability of these Submessages to
contain a set of sequence number will be more efficient".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.3 Coalescing multiple Data submessages

**Spec:** §8.4.15.3, p. 122 — "implementations that combine Submessages into a
single RTPS message will in general make better utilization of CPU and
bandwidth."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.4 Piggybacking HeartBeat on Data

**Spec:** §8.4.15.4, p. 122 — "A particularly useful case is the piggybacking
of HeartBeat Submessages following Data Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.5 Sending to unknown readerId

**Spec:** §8.4.15.5, p. 122 — "Implementations are encouraged to use this
feature to minimize bandwidth usage."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.6 Reclaiming resources from unresponsive readers

**Spec:** §8.4.15.6, p. 123 — "the Writer should be allowed to deem the Reader
as 'Inactive' and proceed in updating its queue."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15.

### §8.4.15.7 Count in Heartbeat, HeartbeatFrag, AckNack and NackFrag

**Spec:** §8.4.15.7, p. 123 — "an implementation should ensure that same
logical HEARTBEATs are tagged with the same Count." "The Count element should
be incremented and compared according to modular arithmetic rules in order to
accommodate the integer overflow."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — guideline under §8.4.15; the duties on
Count are under §8.3.8.

## §8.5 Discovery Module

### §8.5 Discovery of remote entities and built-in topics

**Spec:** §8.5, p. 123 — "DDS implementations must automatically discover the
presence of remote entities, both when they join and leave the network. This
discovery information is made accessible to the user through DDS built-in
topics."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber`);
`crates/dcps/src/runtime.rs` (`handle_spdp_datagram`, SEDP events)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::spdp_receive_populates_dcps_participant_reader`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`,
`crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`

**Status:** `done`

### §8.5.1 SPDP and SEDP as the minimum

**Spec:** §8.5.1, p. 124 — "For the purpose of interoperability, all RTPS
implementations must provide at least the following discovery protocols:
1. Simple Participant Discovery Protocol (SPDP) 2. Simple Endpoint Discovery
Protocol (SEDP)". Likewise §8.5.3, p. 125, and §8.5.4, p. 130.

**Repo:** `crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`
(`SedpStack`)

**Tests:** `crates/discovery/tests/spdp_loopback_e2e.rs::two_zerodds_participants_discover_each_other`,
`crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`

**Status:** `done`

### §8.5.1 Further PDPs and EDPs

**Spec:** §8.5.1, p. 124 — "Implementations may choose to support multiple
PDPs and EDPs, possibly vendor-specific." "How this information is used by a
Participant to configure its local Endpoints depends on the actual
implementation of the RTPS protocol and is not part of the discovery protocol
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §8.5.2 Built-in topics and their RTPS endpoints

**Spec:** §8.5.2, p. 124 — "There are four pre-defined built-in Topics:
“DCPSParticipant,” “DCPSSubscription,” “DCPSPublication,” and “DCPSTopic.”"
"The built-in DataReaders act as regular DDS DataReaders and can also be
accessed by the user through the DDS API." "RTPS maps each built-in DDS
DataWriter or DataReader to an associated built-in RTPS Endpoint."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (four built-in DataReaders);
`crates/discovery/src/spdp.rs`; `crates/discovery/src/sedp/stack.rs`

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/src/builtin_subscriber.rs::tests::sinks_push_topic_lands_in_reader`

**Status:** `done`

### §8.5.3.1 SPDP built-in endpoints

**Spec:** §8.5.3.1, p. 125 — "For each Participant, the SPDP creates two RTPS
built-in Endpoints: the SPDPbuiltinParticipantWriter and the
SPDPbuiltinParticipantReader."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`, `SpdpReader`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::spdp_beacon`)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`

**Status:** `done`

### §8.5.3.1 SPDPbuiltinParticipantWriter: best-effort StatelessWriter

**Spec:** §8.5.3.1, p. 125 — "The SPDPbuiltinParticipantWriter is an RTPS
Best-Effort StatelessWriter."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`: DATA without
HEARTBEAT, no state per recipient)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_uses_spdp_builtin_writer_id`,
`crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done`

### §8.5.3.1 One data-object in the HistoryCache, replaced on change

**Spec:** §8.5.3.1, p. 125 — "The HistoryCache of the
SPDPbuiltinParticipantWriter contains a single data-object of type
SPDPdiscoveredParticipantData. The value of this data-object is set from the
attributes in the Participant. If the attributes change, the data-object is
replaced."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::data`);
`crates/dcps/src/runtime.rs` (beacon update on a changed USER_DATA)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** `done`

### §8.5.3.1 Periodic resend of the same change

**Spec:** §8.5.3.1, p. 125 — "This is achieved by periodically calling
StatelessWriter::unsent_changes_reset, which causes the StatelessWriter to
resend all changes present in its HistoryCache to all locators."

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon::serialize`, `set_new_sn_each_period`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::spdp_new_sequence_per_period`)

**Tests:** `crates/discovery/src/spdp.rs::tests::the_resent_beacon_keeps_its_sequence_number_until_the_data_changes`,
`crates/discovery/src/spdp.rs::tests::beacon_increments_sequence_number`

**Status:** `done` — the periodic announcement resends the one change with its sequence number; a change of the participant data is a new change. The form of Cyclone DDS and OpenDDS (a new number each period, measured in `tools/wire-capture/rtps-sessions`) is the setting `discovery.spdp_new_sequence_per_period`. Not measured: whether a Cyclone or OpenDDS reader keeps the participant alive on a repeated number.

### §8.5.3.1 Period: default by the PSM, smaller than leaseDuration

**Spec:** §8.5.3.1, p. 125 — "The periodic rate at which the
SPDPbuiltinParticipantWriter sends out the SPDPdiscoveredParticipantData
defaults to a PSM specified value. This period should be smaller than the
leaseDuration specified in the SPDPdiscoveredParticipantData".

**Repo:** `crates/dcps/src/runtime.rs` (`default_spdp_period_for`, `DEFAULT_SPDP_PERIOD`, key `discovery.spdp_period_ms`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — the default is the PSM's, 30 s; with a lease under 90 s at most a third of the lease, so that the period stays smaller than `leaseDuration`.

### §8.5.3.1 Pre-configured unicast and multicast locators

**Spec:** §8.5.3.1, p. 125 — "The pre-configured list of locators may include
both unicast and multicast locators. Port numbers are defined by each PSM."

**Repo:** `crates/dcps/src/runtime.rs` (`send_spdp_multicast`,
`send_spdp_unicast`, `expand_initial_peer`)

**Tests:** `crates/dcps/src/runtime.rs::tests::multicast_free_discovery_via_initial_peers`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done`

### §8.5.3.1 SPDPbuiltinParticipantReader and the choice of EDP

**Spec:** §8.5.3.1, p. 125 — "The contained information includes what Endpoint
Discovery Protocols the remote Participant supports. The proper Endpoint
Discovery Protocol is then used for exchanging Endpoint information with the
remote Participant."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`: by
`builtin_endpoint_set`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.5.3.1 Optional: an announcement to a participant not known before

**Spec:** §8.5.3.1, p. 125 — "Implementations can minimize any start-up delays
by sending an additional SPDPdiscoveredParticipantData in response to
receiving this data-object from a previously unknown Participant, but this
behavior is optional."

**Repo:** `crates/dcps/src/runtime.rs` (`directed_spdp_targets`,
`SpdpResponseTargets`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_spdp_response_axis_decides_the_targets`,
`crates/rtps/src/dialect.rs::tests::the_spdp_response_default_answers_the_beacon_source`

**Status:** `done`

### §8.5.3.1 Optional: extend the locator list by discovered participants

**Spec:** §8.5.3.1, p. 125 — "Implementations may also enable the user to
choose whether to automatically extend the pre-configured list of locators
with new locators from newly discovered Participants. … These last two
features are optional and not required for the purpose of interoperability."

**Repo:** `crates/dcps/src/runtime.rs` (`send_spdp_unicast`: the periodic
announcement also goes to the metatraffic locators of discovered
participants)

**Tests:** `crates/dcps/src/runtime.rs::tests::an_initial_peer_a_discovered_participant_covers_is_left_to_the_fan_out`

**Status:** `done` — the extension cannot be switched off.

### §8.5.3.2 Figure 8.27 SPDPdiscoveredParticipantData

**Spec:** §8.5.3.2, p. 125 — "the SPDPdiscoveredParticipantData specializes
the ParticipantProxy and therefore includes all the information necessary to
configure a discovered Participant. The SPDPdiscoveredParticipantData also
specializes the DDS-defined DDS::ParticipantBuiltinTopicData".

**Repo:** `crates/rtps/src/participant_data.rs` (`ParticipantBuiltinTopicData`:
attributes of the ParticipantProxy, `user_data`, `lease_duration`, `manual_liveliness_count`,
`builtin_endpoint_qos`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`

**Status:** `done`

### §8.5.3.2 Table 8.78 domainId

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "domainId: DomainId_t — Identifies the
DDS domainId of the associated DDS DomainParticipant."

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_id`, PID_DOMAIN_ID);
`crates/dcps/src/runtime.rs` (beacon with `domain_id: Some(domain_id)`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 domainTag

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "domainTag: string — Identifies the
DDS domainTag of the associated DDS DomainParticipant."

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_tag`, PID_DOMAIN_TAG)

**Tests:** `crates/rtps/src/participant_data.rs::tests::domain_tag_goes_out_when_set_and_reads_back`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** `done`

### §8.5.3.2 Table 8.78 protocolVersion

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "protocolVersion: ProtocolVersion_t —
Identifies the RTPS protocol version used by the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`protocol_version`,
PID_PROTOCOL_VERSION)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 guidPrefix

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "guidPrefix: GuidPrefix_t — The
common GuidPrefix_t of the Participant and all the Endpoints contained within
the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`guid`, PID_PARTICIPANT_GUID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §8.5.3.2 Table 8.78 vendorId

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "vendorId: VendorId_t — Identifies the
vendor of the DDS middleware that contains the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`vendor_id`, PID_VENDOR_ID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `partial` — ZeroDDS sends `0x01F0`, not a value assigned by the OMG
(`open` item under §8.3.5.2). Blocker: a hard external one, the assignment of a VendorId by the OMG; the request has been pending since 22 June. Until then ZeroDDS sends `0x01F0`, a value the OMG list does not hold (assigned up to `{0x01,0x1C}`, `{0x01,0x1D}` and `{0x01,0x1E}` reserved).

### §8.5.3.2 Table 8.78 expectsInlineQos

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "expectsInlineQos: bool — Describes
whether the Readers within the Participant expect that the QoS values that
apply to each data modification are encapsulated included with each Data."

**Repo:** `crates/rtps/src/participant_data.rs` (`expects_inline_qos`,
PID_EXPECTS_INLINE_QOS)

**Tests:** `crates/rtps/src/participant_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §8.5.3.2 Table 8.78 metatrafficUnicastLocatorList

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "List of unicast locators (transport,
address, port combinations) that can be used to send messages to the built-in
Endpoints contained in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`metatraffic_unicast_locators`, PID_METATRAFFIC_UNICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`,
`crates/dcps/src/runtime.rs::tests::wlp_unicast_targets_prefers_metatraffic_then_default`

**Status:** `done`

### §8.5.3.2 Table 8.78 metatrafficMulticastLocatorList

**Spec:** §8.5.3.2, p. 126, Table 8.78 — "List of multicast locators
(transport, address, port combinations) that can be used to send messages to
the built-in Endpoints contained in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs`
(`metatraffic_multicast_locators`, PID_METATRAFFIC_MULTICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/participant_data.rs::tests::decode_zero_locators_yields_empty_lists`

**Status:** `done`

### §8.5.3.2 Table 8.78 defaultUnicastLocatorList

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "defaultUnicastLocatorList:
Locator_t[1..*] … These are the unicast locators that will be used in case the
Endpoint does not specify its own set of Locators, so at least one Locator
must be present."

**Repo:** `crates/rtps/src/participant_data.rs` (`default_unicast_locators`);
`crates/dcps/src/runtime.rs` (`announce_locators`: never empty)

**Tests:** `crates/dcps/src/runtime.rs::tests::announce_locators_automatic_is_non_empty_and_includes_probe`,
`crates/rtps/src/participant_data.rs::tests::encode_dedups_exact_duplicate_locators`

**Status:** `done`

### §8.5.3.2 Table 8.78 defaultMulticastLocatorList

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "Default list of multicast locators …
These are the multicast locators that will be used in case the Endpoint does
not specify its own set of Locators."

**Repo:** `crates/rtps/src/participant_data.rs` (`default_multicast_locators`,
PID_DEFAULT_MULTICAST_LOCATOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done` — ZeroDDS announces no default multicast locator.

### §8.5.3.2 Table 8.78 availableBuiltinEndpoints

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "All Participants must support the
SEDP. This attribute identifies the kinds of built-in SEDP Endpoints that are
available in the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_set`,
`endpoint_flag`); `crates/dcps/src/runtime.rs` (`ALL_STANDARD` plus
TOPICS_ANNOUNCER/TOPICS_DETECTOR)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_standard_bits_by_default`

**Status:** `done`

### §8.5.3.2 Table 8.78 leaseDuration

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "How long a Participant should be
considered alive every time an announcement is received from the Participant.
If a Participant fails to send another announcement within this time period,
the Participant can be considered gone."

**Repo:** `crates/rtps/src/participant_data.rs` (`lease_duration`,
PID_PARTICIPANT_LEASE_DURATION); `crates/discovery/src/spdp.rs`
(`DiscoveredParticipantsCache::drain_expired`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §8.5.3.2 Table 8.78 manualLivelinessCount

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "manualLivelinessCount: Count_t —
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

**Status:** `done` — when a MANUAL_BY_PARTICIPANT sample is written the count goes up (at most once per tick) and a new announcement goes out; a receiver that sees the count of a known participant change takes it as that participant's manual liveliness assertion (test: n, n + 1 renews the participant's MANUAL_BY_PARTICIPANT writers at the reader).

### §8.5.3.2 Table 8.78 builtinEndpointQos

**Spec:** §8.5.3.2, p. 127, Table 8.78 — "builtinEndpointQos:
BuiltinEndpointQos_t — Provides additional information on the QoS of the
built-in Endpoints supported by the Participant."

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_qos`);
`crates/dcps/src/wlp.rs` (`WlpEndpoint::add_peer_reader`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — the field is read and written; the WLP writer treats the reader accordingly (§8.4.13.3). ZeroDDS announces none itself: its reader is RELIABLE.

### §8.5.3.2 Further EDPs through the extension mechanisms

**Spec:** §8.5.3.2, p. 127 — "There are currently no other Endpoint Discovery
Protocols defined by the RTPS specification. In order to extend
SPDPdiscoveredParticipantData to include additional EDPs, the standard RTPS
extension mechanisms can be used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §8.5.3.2.1 availableBuiltinEndpoints: endpoints that may be announced

**Spec:** §8.5.3.2, p. 127 — "The builtin endpoints that may be announced in
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

### §8.5.3.3 EntityIds of the SPDP endpoints

**Spec:** §8.5.3.3, p. 128 — "The Protocol reserves the following values of the
EntityId_t for the SPDP built-in Endpoints:
ENTITYID_SPDP_BUILTIN_PARTICIPANT_WRITER
ENTITYID_SPDP_BUILTIN_PARTICIPANT_READER"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`,
`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`,
`crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP writer unicastLocatorList

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "unicastLocatorList: <auto-detected>
Transport-kinds and addresses are either auto-detected or configured by the
application. Ports are a parameter to the SPDP initialization or else are set
to a PSM-specified value that depends on the domainId."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_locators`,
`spdp_unicast_port`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`,
`crates/dcps/src/runtime.rs::tests::announce_locators_pins_to_single_interface`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP writer multicastLocatorList

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "multicastLocatorList: <parameter to
the SPDP initialization> Defaults to a PSM-specified value."

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`, group
239.255.0.1); `crates/dcps/src/runtime.rs` (`spdp_multicast_group`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_default_multicast_is_239_255_0_1`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_0_is_7400`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP writer reliabilityLevel

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "reliabilityLevel: BEST_EFFORT".

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpBeacon`)

**Tests:** `crates/discovery/src/spdp.rs::tests::beacon_serializes_to_decodable_datagram`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP writer topicKind

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "topicKind: WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`: `BuiltinWriterWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`

**Status:** `done`

### §8.5.3.3 Table 8.79 SPDP writer resendPeriod

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "resendPeriod: <parameter to the SPDP
initialization> Defaults to a PSM-specified value."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::spdp_period`, `ZERODDS_SPDP_PERIOD_MS`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — settable (`discovery.spdp_period_ms`); the default is 30 s, the PSM's.

### §8.5.3.3 Table 8.79 SPDP writer readerLocators

**Spec:** §8.5.3.3, p. 128, Table 8.79 — "readerLocators: <parameter to the
SPDP initialization>".

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::initial_peers`,
`expand_initial_peer`, multicast target)

**Tests:** `crates/dcps/src/runtime.rs::tests::initial_peer_specs_reach_the_runtime_without_touching_the_environment`,
`crates/dcps/src/runtime.rs::tests::initial_peer_specs_expand_per_the_configured_participant_limit`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP reader unicastLocatorList

**Spec:** §8.5.3.3, p. 129, Table 8.80 — "unicastLocatorList: <auto-detected>
… Ports are a parameter to the SPDP initialization or else are set to a
PSM-specified value that depends on the domainId."

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_unicast` socket on
`spdp_unicast_port`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP reader multicastLocatorList

**Spec:** §8.5.3.3, p. 129, Table 8.80 — "multicastLocatorList: <parameter to
the SPDP initialization>. Defaults to a PSM-specified value."

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_multicast_rx`,
`spdp_multicast_group`)

**Tests:** `crates/dcps/src/runtime.rs::tests::start_refuses_spdp_group_outside_allowlist`,
`crates/discovery/tests/spdp_loopback_e2e.rs::loopback_multicast_self_discovery`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP reader reliabilityLevel

**Spec:** §8.5.3.3, p. 129, Table 8.80 — "reliabilityLevel: BEST_EFFORT".

**Repo:** `crates/discovery/src/spdp.rs` (`SpdpReader`: stateless, no
ACKNACK)

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_reads_every_participant_a_datagram_bundles`

**Status:** `done`

### §8.5.3.3 Table 8.80 SPDP reader topicKind

**Spec:** §8.5.3.3, p. 129, Table 8.80 — "topicKind: WITH_KEY".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`: `BuiltinReaderWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §8.5.3.3 SPDP reader: HistoryCache by participant GUID

**Spec:** §8.5.3.3, p. 129 — "The HistoryCache of the
SPDPbuiltinParticipantReader contains information on all active discovered
participants; the key used to identify each data-object corresponds to the
Participant GUID." "If an entry with a matching key is not there, a new entry
is added keyed by the GUID of the Participant."

**Repo:** `crates/discovery/src/spdp.rs` (`DiscoveredParticipantsCache`: key
`data.guid.prefix`)

**Tests:** `crates/discovery/src/spdp.rs::tests::cache_insert_returns_true_for_new_participant`,
`crates/discovery/src/spdp.rs::tests::cache_get_returns_inserted_participant`

**Status:** `done`

### §8.5.3.3 SPDP reader: remove stale entries

**Spec:** §8.5.3.3, p. 129 — "Periodically, the SPDP examines the
SPDPbuiltinParticipantReader HistoryCache looking for stale entries defined as
those that have not been refreshed for a period longer than their specified
leaseDuration. Stale entries are removed."

**Repo:** `crates/discovery/src/spdp.rs`
(`DiscoveredParticipantsCache::drain_expired`); `crates/dcps/src/runtime.rs`
(called per tick)

**Tests:** `crates/discovery/src/spdp.rs::tests::participant_lease_lifecycle_observation_and_infinite`,
`crates/dcps/src/runtime.rs::tests::participant_lease_lifecycle_hard_death_and_same_prefix`

**Status:** `done` — `drain_expired` already removes when the elapsed time
equals leaseDuration.

### §8.5.3.4 Table 8.81 SPDP_WELL_KNOWN_UNICAST_PORT

**Spec:** §8.5.3.4, p. 129, Table 8.81 — "SPDP_WELL_KNOWN_UNICAST_PORT —
entries in SPDPbuiltinParticipantReader.unicastLocatorList, unicast entries in
SPDPbuiltinParticipantWriter.readerLocators".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_unicast_port`);
`crates/dcps/src/runtime.rs` (`expand_initial_peer`)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done`

### §8.5.3.4 Table 8.81 SPDP_WELL_KNOWN_MULTICAST_PORT

**Spec:** §8.5.3.4, p. 129, Table 8.81 — "SPDP_WELL_KNOWN_MULTICAST_PORT —
entries in SPDPbuiltinParticipantReader.multicastLocatorList, multicast entries
in SPDPbuiltinParticipantWriter.readerLocators".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_1_is_7650`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_5_is_8650`

**Status:** `done`

### §8.5.4.1 Association with the SEDP endpoints of a discovered participant

**Spec:** §8.5.4.1, p. 130 — "once a Participant knows of the presence of
another Participant, it can assume the presence of the built-in Endpoints
made available by the remote participant and establish the association with
the locally-matching built-in Endpoints."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::self_discovery_is_ignored`

**Status:** `done`

### §8.5.4.1 SEDP: the same protocol as for application-defined endpoints

**Spec:** §8.5.4.1, p. 130 — "The protocol used to communicate between
built-in Endpoints is the same as used for application-defined Endpoints."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`:
`ReliableWriter`); `crates/discovery/src/sedp/reader.rs` (`ReliableReader`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::end_to_end_discovery_between_two_stacks`,
`crates/discovery/src/sedp/stack.rs::tests::a_heartbeat_past_a_hole_releases_the_buffered_subscription`

**Status:** `done`

### §8.5.4.2 SEDP endpoints reliable

**Spec:** §8.5.4.2, p. 130 — "The SEDP therefore maps each corresponding
built-in DDS DataWriter or DataReader into corresponding reliable RTPS Writer
and Reader Endpoints." "it is sufficient that an implementation provides the
required built-in Endpoints and reliable communication that satisfies the
general requirements listed in 8.4.2."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`);
`crates/discovery/src/sedp/reader.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::a_heartbeat_past_a_hole_releases_the_buffered_subscription`,
`crates/discovery/tests/cyclone_sedp_replay.rs::cyclone_sedp_publication_fixture_flows_through_stack`

**Status:** `done` — the deviations of the reliable writers and readers from
§8.4.2 are listed there.

### §8.5.4.2 EntityIds of the SEDP endpoints

**Spec:** §8.5.4.2, pp. 130–131 — "The RTPS Protocol reserves the following
values of the EntityId_t for the built-in Endpoints:
ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER … ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR"

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`,
`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER` and the others)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_publications_writer_layout`,
`crates/rtps/src/wire_types.rs::tests::sedp_subscriptions_reader_layout`

**Status:** `done` — the values are listed under §9.3.1.3.

### §8.5.4.3 Topics endpoints optional

**Spec:** §8.5.4.3, p. 131 — "it is not required to implement the
SEDPbuiltinTopicsReader and SEDPbuiltinTopicsWriter built-in Endpoints and for
the purpose of interoperability, implementations should not rely on their
presence in remote Participants."

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`:
topics proxies only when the bits are set)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.5.4.3 Only the built-in endpoints needed

**Spec:** §8.5.4.3, p. 131 — "a Participant is only required to provide the
built-in Endpoints required for matching up local and remote Endpoints."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement; ZeroDDS provides all SEDP
endpoints.

### §8.5.4.4 Figure 8.30 DiscoveredWriterData

**Spec:** §8.5.4.4, p. 131 — "DiscoveredWriterData extends
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

**Status:** `done` — the attributes of the WriterProxy are in the announcement: writer GUID, both locator lists, `dataMaxSizeSerialized` (`PID_TYPE_MAX_SIZE_SERIALIZED`) and the group (`PID_GROUP_GUID`, giving `remoteGroupEntityId`).

### §8.5.4.4 Figure 8.30 DiscoveredReaderData

**Spec:** §8.5.4.4, p. 131 — "DiscoveredReaderData extends the DDS-defined
DDS::SubscriptionBuiltinTopicData".

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData`: QoS, `key`, `unicast_locators`,
`multicast_locators`, `expects_inline_qos`, `content_filter`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §8.5.4.4 Figure 8.30 DiscoveredTopicData

**Spec:** §8.5.4.4, p. 131 — "DiscoveredTopicData extends
DDS::TopicBuiltinTopicData."

**Repo:** `crates/rtps/src/topic_data.rs` (`TopicBuiltinTopicData`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`,
`crates/discovery/src/sedp/stack.rs::tests::a_cyclone_topic_and_its_dispose_are_read_by_cyclones_own_key`

**Status:** `done`

### §8.5.4.4 Missing information: defaults of the PSM

**Spec:** §8.5.4.4, p. 132 — "An implementation of the protocol need not
necessarily send all information contained in the DataTypes. If any
information is not present, the implementation can assume the default
values, as defined by the PSM."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (default per missing PID)

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::participant_key_fallback_when_pid_missing`

**Status:** `done` — the default per PID is listed under §9.6.3.2.

### §8.5.4.4 Table 8.82 Publications: DataType

**Spec:** §8.5.4.4, p. 133, Table 8.82 — "DataType: DiscoveredWriterData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpPublicationsWriter::announce`:
`PublicationBuiltinTopicData::to_pl_cdr_le`)

**Tests:** `crates/discovery/tests/sedp_loopback.rs::publication_announce_then_reader_fills_cache`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: Cardinality

**Spec:** §8.5.4.4, p. 133, Table 8.82 — "Cardinality: The number of
DataWriters contained by the DomainParticipant. There is a one-to-one
correspondence between each DataWriter in the participant and a data-object
that describes the DataWriter stored in the WriterHistoryCache for the
SEDPbuiltinPublicationsWriter."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`make_sedp_writer`: one change
per instance, `SEDP_MAX_CHANGES`; `SedpPublicationsWriter::announce` keyed on
the endpoint GUID)

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: insertion

**Spec:** §8.5.4.4, p. 133, Table 8.82 — "Data-Object insertion: Each time a
DataWriter is created in the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_publication` when the writer
is created)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: modification

**Spec:** §8.5.4.4, p. 133, Table 8.82 — "Data-Object modification: Each time
the QoS of an existing DataWriter is modified."

**Repo:** `crates/dcps/src/runtime.rs` (`reannounce_publication`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** `done`

### §8.5.4.4 Table 8.82 Publications: deletion

**Spec:** §8.5.4.4, p. 133, Table 8.82 — "Data-Object deletion: Each time an
existing DataWriter belonging to the DomainParticipant is deleted."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpPublicationsWriter::dispose`);
`crates/dcps/src/runtime.rs` (`dispose_publication` on deletion)

**Tests:** `crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: DataType

**Spec:** §8.5.4.4, p. 134, Table 8.83 — "DataType: DiscoveredReaderData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpSubscriptionsWriter::announce`)

**Tests:** `crates/discovery/tests/sedp_loopback.rs::subscription_announce_then_reader_fills_cache`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: Cardinality

**Spec:** §8.5.4.4, p. 134, Table 8.83 — "Cardinality: The number of
DataReaders contained by the DomainParticipant. There is a one-to-one
correspondence between each DataReaders in the Participant and a data-object
that describes the DataReaders stored in the WriterHistoryCache for the
SEDPbuiltinSubscriptionsWriter."

**Repo:** —

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done` — the same writer setup (`make_sedp_writer`) as under Table
8.82 Cardinality, announced keyed on the GUID.

### §8.5.4.4 Table 8.83 Subscriptions: insertion

**Spec:** §8.5.4.4, p. 134, Table 8.83 — "Data-Object insertion: Each time a
DataReader is created in the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_subscription` when the reader
is created)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: modification

**Spec:** §8.5.4.4, p. 134, Table 8.83 — "Data-Object modification: Each time
the QoS of an existing DataReader is modified."

**Repo:** `crates/dcps/src/runtime.rs` (`announce_subscription` after
`set_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.5.4.4 Table 8.83 Subscriptions: deletion

**Spec:** §8.5.4.4, p. 134, Table 8.83 — "Data-Object deletion: Each time an
existing DataReader belonging to the DomainParticipant is deleted."

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpSubscriptionsWriter::dispose`)

**Tests:** `crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_reader_unmatches_the_remote_writer`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: DataType

**Spec:** §8.5.4.4, p. 134, Table 8.84 — "DataType: DiscoveredTopicData".

**Repo:** `crates/discovery/src/sedp/writer.rs` (`SedpTopicsWriter::announce`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: Cardinality

**Spec:** §8.5.4.4, p. 134, Table 8.84 — "Cardinality: The number of Topics
created by the DomainParticipant. There is a one-to-one correspondence between
each Topic created by the DomainParticipant and a data-object that describes
the Topic stored in the WriterHistoryCache for the builtinTopicsWriter."

**Repo:** —

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::every_endpoint_keeps_one_change_for_a_reader_that_matches_later`

**Status:** `done` — the same writer setup (`make_sedp_writer`) as under Table
8.82 Cardinality, announced keyed on the GUID.

### §8.5.4.4 Table 8.84 Topics: insertion

**Spec:** §8.5.4.4, p. 134, Table 8.84 — "Data-Object insertion: Each time a
Topic is created in the DomainParticipant."

**Repo:** `crates/dcps/src/participant.rs` (`announce_topic` on creation or
enable)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** `done`

### §8.5.4.4 Table 8.84 Topics: modification

**Spec:** §8.5.4.4, p. 134, Table 8.84 — "Data-Object modification: Each time
the QoS of an existing Topic is modified."

**Repo:** `crates/dcps/src/topic.rs` (`set_qos` → `announce_topic`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done` — the test checks the SEDP stack, not the call from
`Topic::set_qos`.

### §8.5.4.4 Table 8.84 Topics: deletion

**Spec:** §8.5.4.4, p. 134, Table 8.84 — "Data-Object deletion: Each time an
existing Topic belonging to the DomainParticipant is deleted."

**Repo:** `crates/dcps/src/runtime.rs` (`dispose_local_topic`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::delete_topic_disposes_the_announced_topic_and_a_writer_holds_it`

**Status:** `done`

### §8.5.5 Interaction with the virtual machine

**Spec:** §8.5.5, p. 134 — "To further illustrate the SPDP and SEDP, this
specification describes how the information provided by the SPDP can be used
to configure the SEDP built-in Endpoints in the RTPS virtual machine."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction to §8.5.5.1 and §8.5.5.2.

### §8.5.5.1 New participant: compare domainId

**Spec:** §8.5.5.1, pp. 134–135 — "IF ( participant_data.domainId !=
local_participant.domainId ) THEN RETURN;"

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_announced`: PID_DOMAIN_ID)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/dcps/src/runtime.rs::tests::a_beacon_of_another_domain_is_named_once_per_sender`

**Status:** `done` — a missing PID_DOMAIN_ID counts as the own domain.

### §8.5.5.1 New participant: compare domainTag

**Spec:** §8.5.5.1, p. 135 — "IF ( !STRING_EQUAL(participant_data.domainTag,
local_participant.domainTag) ) THEN RETURN;"

**Repo:** `crates/dcps/src/runtime.rs` (`spdp_announced`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** `done`

### §8.5.5.1 New participant: proxies per announced endpoint

**Spec:** §8.5.5.1, pp. 135–136 — "IF ( PUBLICATIONS_DETECTOR IS_IN
participant_data.availableEndpoints ) THEN … writer.matched_reader_add(proxy);"
and likewise for PUBLICATIONS_ANNOUNCER, SUBSCRIPTIONS_DETECTOR,
SUBSCRIPTIONS_ANNOUNCER, TOPICS_DETECTOR, TOPICS_ANNOUNCER.

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::discovered_participant_wires_all_four_endpoints_when_present`,
`crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`,
`crates/discovery/tests/opensplice_spdp_bundle.rs::the_participant_with_the_detectors_gets_the_local_sedp_writers_proxies`

**Status:** `done` — the proxies get the metatraffic unicast locators (or else
the default unicast locators), no multicast locators.

### §8.5.5.2 Participant removed: remove the proxies of the SEDP endpoints

**Spec:** §8.5.5.2, p. 136 — "Based on the remote Participant’s leaseDuration,
a local Participant ‘local_participant’ concludes that a previously discovered
Participant with GUID_t participant_guid is no longer present. The Participant
‘local_participant’ must reconfigure any local Endpoints that were
communicating with Endpoints in the Participant".

**Repo:** `crates/discovery/src/sedp/stack.rs` (`on_participant_lost`);
`crates/dcps/src/runtime.rs` (lease expiry and departure)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::on_participant_lost_clears_proxies_and_cache`,
`crates/discovery/tests/cyclone_sedp_replay.rs::replay_after_participant_lost_stops_accepting_publications`,
`crates/dcps/src/runtime.rs::tests::participant_lease_lifecycle_hard_death_and_same_prefix`

**Status:** `done`

### §8.5.6 Other discovery protocols

**Spec:** §8.5.6, p. 136 — "Remote Participants using a different vendor’s
RTPS implementation must be contacted using at least the SPDP to ensure
interoperability. There is no such requirement when the remote Participant
uses the same RTPS implementation."

**Repo:** `crates/dcps/src/runtime.rs` (SPDP runs in every participant; a
shortcut within one process through `inproc_announce_publication`)

**Tests:** `crates/dcps/src/runtime.rs::tests::handle_spdp_registers_real_fastdds_participant`,
`crates/discovery/tests/fastdds_spdp_repro.rs::zerodds_parses_real_fastdds_spdp`

**Status:** `done`

### §8.5.6 A common EDP

**Spec:** §8.5.6, p. 137 — "All Participants must support at least the SEDP, so
they always have at least one EDP in common. However, if two Participants both
support another EDP, this alternative protocol can be used instead."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the duty to SEDP is listed under §8.5.1; the
rest is a permission.

## §8.6 Versioning and Extensibility

### §8.6 Processing higher minor versions

**Spec:** §8.6, p. 137 — "Implementations of this version of the RTPS protocol
should be able to process RTPS Messages not only with the same major version
but possibly higher minor versions."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: only a higher
major version is refused)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_accepts_minor_version_above_supported`,
`crates/rtps/src/datagram.rs::tests::decode_rejects_major_version_above_supported`

**Status:** `done`

### §8.6.1 Skip unknown submessages

**Spec:** §8.6.1, p. 137 — "Additional Submessages with other submessageIds can
be introduced and used anywhere in an RTPS Message. An implementation should
skip over unknown Submessages using the submessageLength field in the
SubmessageHeader."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`,
`ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_unknown_submessage_without_must_understand`,
`crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — a submessage with an unknown SubmessageId is skipped by `submessageLength`, also with flag bit 7 set (§8.3.4.1 rule 3 has no must-understand at submessage level).

### §8.6.1 Skip additional fields at the end of a submessage

**Spec:** §8.6.1, p. 137 — "Additional fields can be added to the end of a
Submessage that was already defined in the current minor version. An
implementation should skip over additional fields using the submessageLength
field in the SubmessageHeader."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_submessage`); the `read_body` of the submessages in
`crates/rtps/src/submessages.rs`

**Tests:** `crates/rtps/src/datagram.rs::tests::additional_fields_at_the_end_of_a_known_submessage_are_skipped`

**Status:** `done` — HEARTBEAT, ACKNACK, GAP, HEARTBEAT_FRAG, NACK_FRAG, INFO_TS, INFO_SRC, INFO_REPLY, INFO_REPLY_IP4 and INFO_DST give the same value with eight more octets at the end of the body as without. For DATA and DATA_FRAG the body ends with the payload; new fields before it are skipped through `octetsToInlineQos`.

### §8.6.1 Ignore unknown built-in endpoints

**Spec:** §8.6.1, p. 137 — "Additional built-in Endpoints with new IDs can be
added. An implementation should ignore any unknown built-in Endpoints."

**Repo:** `crates/discovery/src/sedp/stack.rs` (routing of known EntityIds
only); `crates/discovery/src/sedp/stack.rs` (`on_participant_discovered`: known
bits only)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::routing_unknown_with_wrong_writer_does_not_match`,
`crates/discovery/src/sedp/stack.rs::tests::partial_endpoint_set_wires_only_matching_sides`

**Status:** `done`

### §8.6.1 Ignore unknown parameters

**Spec:** §8.6.1, p. 137 — "Additional parameters with new parameterIds can be
added. An implementation should ignore any unknown parameters."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::from_bytes`);
`crates/rtps/src/publication_data.rs` (decoders pass over unknown PIDs)

**Tests:** `crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`

**Status:** `done` — unknown PIDs with the must-understand bit are listed under
§9.4.2.11.

### §8.6.1 Extensions raise the minor version

**Spec:** §8.6.1, p. 137 — "All such changes require an increase of the minor
version number."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a requirement on future editions of the
specification.

### §8.6.2 What cannot change within the major version

**Spec:** §8.6.2, p. 137 — "A Submessage cannot be deleted. A Submessage cannot
be modified except as described in 8.6.1. The meaning of submessageIds cannot
be modified."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a requirement on future editions of the
specification.

## §8.7 Implementing DDS QoS and advanced DDS features using RTPS

### §8.7 DDS QoS and advanced features over RTPS

**Spec:** §8.7, p. 137 — "This sub clause defines how to use RTPS to implement
the DDS QoS parameters." "This sub clause forms a normative part of the
specification for the purpose of interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the requirements are in
§8.7.1 to §8.7.10.

### §8.7.1 The writer's inline QoS at the reader's request

**Spec:** §8.7.1, p. 138 — "A stateless Reader’s need for receiving in-line QoS
to get information on remote Writers is the justification for requiring a
Writer to send in-line QoS if the Reader requests them (8.4.2.2.2)." "For
immutable QoS, all RxO QoS are sent in-line … Mutable QoS relevant to the
Reader are sent in-line so they may take effect immediately".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`sample_inline_qos`:
`inline_writer_qos` for readers with `expectsInlineQos`);
`crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_reader_that_expects_inline_qos_gets_the_writers_qos_with_each_data`,
`crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.1 TOPIC_NAME in the inline QoS

**Spec:** §8.7.1, p. 138 — "If in-line QoS parameters are expected,
implementations must also include the topic name as an in-line parameter."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::TOPIC_NAME` first)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.1 Coherent-set and filter parameters

**Spec:** §8.7.1, p. 138 — "Independent of whether Readers expect in-line QoS
parameters, a Data Submessage may also contain in-line parameters related to
coherent sets and content-filtered topics."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — refers to §8.7.3 and §8.7.6.

### §8.7.1 Stateful readers may pass over the inline QoS

**Spec:** §8.7.1, p. 138 — "For improved performance, stateful implementations
may ignore in-line QoS and instead rely solely on cached values obtained
through Discovery."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement; ZeroDDS readers take the
writer's QoS from SEDP.

### §8.7.2 Table 8.85 USER_DATA

**Spec:** §8.7.2, p. 138, Table 8.85 — "USER_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TOPIC_DATA

**Spec:** §8.7.2, p. 138, Table 8.85 — "TOPIC_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 GROUP_DATA

**Spec:** §8.7.2, p. 138, Table 8.85 — "GROUP_DATA — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DURABILITY

**Spec:** §8.7.2, p. 138, Table 8.85 — "DURABILITY — See 8.7.2.2.1 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DURABILITY`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DURABILITY_SERVICE

**Spec:** §8.7.2, p. 138, Table 8.85 — "DURABILITY_SERVICE — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 PRESENTATION

**Spec:** §8.7.2, p. 138, Table 8.85 — "PRESENTATION — See 8.7.2.2.2 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::PRESENTATION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DEADLINE

**Spec:** §8.7.2, p. 138, Table 8.85 — "DEADLINE — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DEADLINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 LATENCY_BUDGET

**Spec:** §8.7.2, p. 138, Table 8.85 — "LATENCY_BUDGET — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LATENCY_BUDGET`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 OWNERSHIP

**Spec:** §8.7.2, p. 138, Table 8.85 — "OWNERSHIP — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::OWNERSHIP`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 OWNERSHIP_STRENGTH

**Spec:** §8.7.2, p. 138, Table 8.85 — "OWNERSHIP_STRENGTH — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::OWNERSHIP_STRENGTH`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 LIVELINESS

**Spec:** §8.7.2, p. 138, Table 8.85 — "LIVELINESS — See 8.7.2.2.3 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LIVELINESS`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TIME_BASED_FILTER

**Spec:** §8.7.2, p. 139, Table 8.85 — "TIME_BASED_FILTER — See 8.7.2.2.4 —
No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 PARTITION

**Spec:** §8.7.2, p. 139, Table 8.85 — "PARTITION — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::PARTITION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 RELIABILITY

**Spec:** §8.7.2, p. 139, Table 8.85 — "RELIABILITY — See 8.7.2.2.5 — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::RELIABILITY`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 TRANSPORT_PRIORITY

**Spec:** §8.7.2, p. 139, Table 8.85 — "TRANSPORT_PRIORITY — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::TRANSPORT_PRIORITY` when
not 0)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §8.7.2 Table 8.85 LIFESPAN

**Spec:** §8.7.2, p. 139, Table 8.85 — "LIFESPAN — None — Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::LIFESPAN`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 DESTINATION_ORDER

**Spec:** §8.7.2, p. 139, Table 8.85 — "DESTINATION_ORDER — See 8.7.2.2.6 —
Yes".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: `pid::DESTINATION_ORDER`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 HISTORY

**Spec:** §8.7.2, p. 139, Table 8.85 — "HISTORY — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 RESOURCE_LIMITS

**Spec:** §8.7.2, p. 139, Table 8.85 — "RESOURCE_LIMITS — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 ENTITY_FACTORY

**Spec:** §8.7.2, p. 139, Table 8.85 — "ENTITY_FACTORY — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 WRITER_DATA_LIFECYCLE

**Spec:** §8.7.2, p. 139, Table 8.85 — "WRITER_DATA_LIFECYCLE — See 8.7.2.2.7 —
No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2 Table 8.85 READER_DATA_LIFECYCLE

**Spec:** §8.7.2, p. 139, Table 8.85 — "READER_DATA_LIFECYCLE — None — No".

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`: not in `IN_LINE`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2.1 Missing inline QoS: default by DDS

**Spec:** §8.7.2.1, p. 139 — "If a Reader expects to receive in-line QoS
parameters and any of these QoS parameters are missing, it will assume the
default value for that QoS parameter, where the default is defined by DDS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the condition does not occur: ZeroDDS
readers announce `expectsInlineQos` = FALSE and take the writer's QoS from
SEDP.

### §8.7.2.1 TOPIC_NAME for self-describing messages

**Spec:** §8.7.2.1, p. 139 — "In order to achieve self-describing messages, not
only the parameters defined in Table 8.85 have to be sent with the submessage,
but also a parameter TOPIC_NAME."

**Repo:** `crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §8.7.2.2.1 DURABILITY

**Spec:** §8.7.2.2.1, p. 139 — "While volatile and transient-local durability
do not affect the RTPS protocol, support for transient and persistent
durability may. This is not covered in the current version of the
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §8.7.2.2.2 PRESENTATION

**Spec:** §8.7.2.2.2, p. 139 — "Sub clause 8.7.5 defines how to implement the
GROUP ordered access policy of the PRESENTATION QoS. Sub clause 8.7.6 defines
how to implement the coherent access policy of the PRESENTATION QoS."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — refers to §8.7.5 and §8.7.6.

### §8.7.2.2.3 LIVELINESS AUTOMATIC

**Spec:** §8.7.2.2.3, p. 139 — "DDS_AUTOMATIC_LIVELINESS_QOS : liveliness is
maintained through the BuiltinParticipantMessageWriter. … implementations must
refresh the Participant’s liveliness (i.e., send the ParticipantMessageData,
see (8.4.13.5) at a rate faster than the smallest lease duration among the
Writers."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_wlp_period`);
`crates/dcps/src/wlp.rs` (`WlpEndpoint::tick`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`

**Status:** `done`

### §8.7.2.2.3 LIVELINESS MANUAL_BY_PARTICIPANT

**Spec:** §8.7.2.2.3, p. 139 — "If the Participant has any
MANUAL_BY_PARTICIPANT Writers, implementations must check periodically to see
if write(), assert_liveliness(), dispose(), or unregister_instance() was called
for any of them. The period for this check equals the smallest lease duration
among the Writers. If any of the operations were called, implementations must
refresh the Participant’s liveliness (i.e., send the ParticipantMessageData,
see 8.4.13.5)."

**Repo:** `crates/dcps/src/runtime.rs` (`refresh_manual_by_participant`, calls in
`write_user_sample_*`, `write_user_lifecycle_with`,
`assert_writer_liveliness_eid`, `bump_manual_liveliness_count`); `crates/dcps/src/wlp.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_by_participant_writer_refreshes_the_participant_liveliness`,
`crates/dcps/src/wlp.rs::tests::refreshes_before_the_tick_share_one_manual_by_participant_sample`,
`crates/dcps/src/runtime.rs::tests::the_manual_liveliness_count_of_the_announcement_counts_up`

**Status:** `done` — `write`, `assert_liveliness`, `dispose` and `unregister_instance` of a MANUAL_BY_PARTICIPANT writer cause a ParticipantMessageData with MANUAL_LIVELINESS_UPDATE at the next tick (instead of a check at the rate of the smallest lease) and raise the `manualLivelinessCount` of the SPDP announcement.

### §8.7.2.2.3 LIVELINESS MANUAL_BY_TOPIC

**Spec:** §8.7.2.2.3, p. 140 — "The standard RTPS Messages that result from
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

### §8.7.2.2.4 TIME_BASED_FILTER on the writer side

**Spec:** §8.7.2.2.4, p. 140 — "Implementations may optimize bandwidth usage by
applying a time-based filter on the Writer side. … When one or more data
updates are filtered out on the Writer side, implementations must send a Gap
Submessage instead, indicating which samples were filtered out. This
Submessage must be sent before the next update".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the condition does not occur: ZeroDDS
applies TIME_BASED_FILTER on the reader side only (`tbf_should_deliver`).

### §8.7.2.2.5 RELIABILITY

**Spec:** §8.7.2.2.5, p. 140 — "Implementations must meet the reliable RTPS
protocol requirements for interoperability, defined in 8.4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference; the requirements are listed
under §8.4.2.

### §8.7.2.2.6 DESTINATION_ORDER BY_SOURCE_TIMESTAMP

**Spec:** §8.7.2.2.6, p. 140 — "In order to implement the
DDS_BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS policy, implementations must
include an InfoTimestamp Submessage with every update from a Writer."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`append_info_ts` before every
DATA and every DATA_FRAG); `crates/dcps/src/runtime.rs` (every `write` gets a
source timestamp)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::write_stamped_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_writer.rs::tests::write_lifecycle_prepends_info_ts_before_data`,
`crates/rtps/src/reliable_writer.rs::tests::resend_carries_original_info_ts`

**Status:** `done`

### §8.7.2.2.7 WRITER_DATA_LIFECYCLE autodispose

**Spec:** §8.7.2.2.7, p. 140 — "If autodispose_unregistered_instances is
enabled, Data Messages that unregister an instance must also dispose it."

**Repo:** `crates/dcps/src/publisher.rs` (`unregister_instance`: DISPOSED with
UNREGISTERED under `autodispose_unregistered_instances`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** `done`

### §8.7.3 Filtering on the reader side

**Spec:** §8.7.3, p. 140 — "When filtering on the Reader side only, samples
which do not pass the filter are simply dropped by the middleware. In this
case, no further extensions to RTPS are needed."

**Repo:** `crates/dcps/src/subscriber.rs` (filter of the ContentFilteredTopic)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filter.rs::filter_drops_samples_that_return_false`

**Status:** `done`

### §8.7.3 Filtering on the writer side

**Spec:** §8.7.3, p. 140 — "In order to support Writer side filtering, standard
RTPS extension mechanisms are used to: Include Reader filter information during
the Endpoint discovery phase. Include filter results with each data sample."
§8.7.3.4, p. 142: "Writer side filtering constitutes an optimization and is
optional".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — optional; ZeroDDS does not filter on the
writer side. The reader's filter information in SEDP is listed under §8.7.3.1.

### §8.7.3 Hints by the writer: ALIVE_FILTERED, ContentFilterInfo, omitted data

**Spec:** §8.7.3, p. 140 — "The Writer may indicate to a Reader that a Sample
has been filtered … by sending a directed Data message that includes only the
key information (DataFlag=0), indicating in the Inline Qos that the instance
state is ALIVE_FILTERED." "Alternatively, the Writer may not send a Data
message at all. This is only allowed if the previous sample for that Instance
was already filtered for that Reader".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side,
which ZeroDDS does not do.

### §8.7.3.1 ContentFilterProperty_t in DiscoveredReaderData

**Spec:** §8.7.3.1, p. 141 — "RTPS adds an additional ContentFilterProperty_t
field to DiscoveredReaderData, defined in Table 8.86."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`SubscriptionBuiltinTopicData::content_filter`, `ContentFilterProperty`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::subscription_with_content_filter_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.1 Table 8.86 contentFilteredTopicName

**Spec:** §8.7.3.1, p. 141, Table 8.86 — "contentFilteredTopicName: string —
Name of the Content-filtered Topic associated with the Reader. Must have
non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::content_filtered_topic_name`);
`crates/dcps/src/participant.rs` (`create_contentfilteredtopic` refuses an
empty name)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_name`

**Status:** `done`

### §8.7.3.1 Table 8.86 relatedTopicName

**Spec:** §8.7.3.1, p. 141, Table 8.86 — "relatedTopicName: string — Name of
the Topic related to the Content-filtered Topic. Must have non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::related_topic_name`); `crates/dcps/src/topic.rs`
(`ContentFilteredTopic::property`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`

**Status:** `done`

### §8.7.3.1 Table 8.86 filterClassName

**Spec:** §8.7.3.1, p. 141, Table 8.86 — "filterClassName: string … Must have
non-zero length. RTPS predefines the following values: “DDSSQL” Default filter
class name if none specified."

**Repo:** `crates/rtps/src/subscription_data.rs` (`filter_class::DDSSQL`);
`crates/dcps/src/topic.rs` (`ContentFilteredTopic::content_filter_property`: `DDSSQL` or the
registered class)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`,
`crates/dcps/tests/custom_content_filter.rs::a_registered_filter_class_is_named_announced_and_applied`

**Status:** `done`

### §8.7.3.1 Table 8.86 filterExpression

**Spec:** §8.7.3.1, p. 141, Table 8.86 — "filterExpression: string — The actual
filter expression. Must be a valid expression for the filter class specified
using filterClassName. Must have non-zero length."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::filter_expression`); `crates/dcps/src/participant.rs`
(`create_contentfilteredtopic` refuses an empty expression)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_expression`

**Status:** `done`

### §8.7.3.1 Table 8.86 expressionParameters

**Spec:** §8.7.3.1, p. 141, Table 8.86 — "expressionParameters: stringSequence
— Defines the value for each parameter in the filter expression. Can have zero
length if the filter expression contains no parameters."

**Repo:** `crates/rtps/src/subscription_data.rs`
(`ContentFilterProperty::expression_parameters`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.1 A writer without the matching filter class

**Spec:** §8.7.3.1, p. 141 — "In order for the Writer to apply the filter, it
must have been configured to handle filters of the specified filter class. If
not, the Writer will simply ignore the filter information and not filter any
data samples."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side,
which ZeroDDS does not do.

### §8.7.3.1 Changed filter parameters through endpoint discovery

**Spec:** §8.7.3.1, p. 141 — "Each time the parameters are modified, the
updated information is exchanged using the Endpoint discovery protocol."

**Repo:** `crates/dcps/src/runtime.rs` (`update_user_reader_content_filter`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** `done`

### §8.7.3.2 Indicating a filtered sample

**Spec:** §8.7.3.2, p. 141 — "When this happens, the Writer can use a Data
submessage that does not contain a Data payload (DataFlag=0) and sets
FilteredFlag=1, see 8.3.8.2.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side,
which ZeroDDS does not do. The Data submessage has no FilteredFlag; the
FilteredFlag belongs to StatusInfo_t (§9.6.4.9).

### §8.7.3.3 GAP instead of DATA and filter results in the inline QoS

**Spec:** §8.7.3.3, p. 142 — "In that case, the Data submessage is replaced by
a Gap submessage." "The sample passes some or all of the filters. In that
case, the sample must still be sent and the writer must include information
with the Data submessage on what filters were applied and the according
result."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side,
which ZeroDDS does not do.

### §8.7.3.3 Table 8.87 filterResult

**Spec:** §8.7.3.3, p. 142, Table 8.87 — "filterResult: FilterResult_t — For
each filter signature, the results indicate whether the sample passed the
filter."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO is sent only with
filtering on the writer side; ZeroDDS readers always filter themselves.

### §8.7.3.3 Table 8.87 filterSignatures

**Spec:** §8.7.3.3, p. 142, Table 8.87 — "filterSignatures:
FilterSignature_t[] — A list of filters that were applied to the sample."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO is sent only with
filtering on the writer side; ZeroDDS readers always filter themselves.

### §8.7.3.4 Readers filter when the writer does not

**Spec:** §8.7.3.4, p. 142 — "Samples will always be filtered on the Reader
side if: The Writer side did not apply any filtering. The Writer side did not
apply the filter expected by the Reader. … The Reader side does not support
Writer side filtering".

**Repo:** `crates/dcps/src/subscriber.rs` (filter per received sample)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filter.rs::filter_applies_also_to_read_peek`

**Status:** `done`

### §8.7.3.4 Filter class DDSSQL

**Spec:** §8.7.3.4, p. 143 — "If an implementation supports Content-filtered
Topics, it must at least recognize the “DDSSQL” filter class, as mandated by
the DDS specification."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic`, SQL filter);
`crates/rtps/src/subscription_data.rs` (`filter_class::DDSSQL`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filtered_reader.rs::a_field_the_type_does_not_answer_delivers_nothing`

**Status:** `done`

### §8.7.3.4 Custom filter classes can be registered

**Spec:** §8.7.3.4, p. 143 — "For all other filter classes, both
implementations must allow the user to register the same custom filter class."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilter`, `ContentFilteredTopic`);
`crates/dcps/src/participant.rs` (`DomainParticipant::register_contentfilter`,
`lookup_contentfilter`, `unregister_contentfilter`, `create_contentfilteredtopic_with_class`)

**Tests:** `crates/dcps/tests/custom_content_filter.rs::a_registered_filter_class_is_named_announced_and_applied`,
`crates/dcps/tests/custom_content_filter.rs::the_registry_refuses_what_it_cannot_serve`

**Status:** `done` — the application registers a filter class per participant and type (`register_contentfilter::<T>(name, Arc<dyn ContentFilter<T>>)`; `DDSSQL` is the implementation's own class and cannot be registered; names up to 256 octets). `create_contentfilteredtopic_with_class` names the class; the class checks expression and parameters (`compile`, at creation and at `set_filter_parameters`) and filters the samples of a DataReader on the topic (`evaluate`). The class name goes out as `filterClassName` in the reader's `PID_CONTENT_FILTER_PROPERTY`. A class a peer has not registered is ignored by it per §8.7.3.4, and the samples are filtered on the reader side; ZeroDDS does not filter on the writer side.

### §8.7.3.4 Information for ALIVE_FILTERED despite filtering at the writer

**Spec:** §8.7.3.4, p. 143 — "Even if the Writer is performing writer-side
filtering, the Writer must provide enough information for the Reader to
correctly transition the instance state to ALIVE_FILTERED."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side,
which ZeroDDS does not do; every sample goes to every matched reader.

### §8.7.4 Instance state changes through Data and inline QoS

**Spec:** §8.7.4, p. 143 — "RTPS uses regular Data Submessages and the in-line
QoS parameter extension mechanism to communicate instance state changes. The
serialized information within the inline QoS contains the new InstanceState".

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`);
`crates/rtps/src/reliable_writer.rs` (`write_lifecycle`: PID_STATUS_INFO)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §8.7.4 Only the key in the SerializedPayload

**Spec:** §8.7.4, p. 143 — "When RTPS sends a Data Submessage to communicate
instance state changes it may include only the Key of the Data-Object within
the SerializedPayload submessage element".

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`:
`lifecycle_serialized_key`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §8.7.4 Registration only with the first value

**Spec:** §8.7.4, p. 143 — "An implementation of RTPS is not required to
propagate registration changes until the DDS DataWriter writes the first value
for that Data-Object instance."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §8.7.4 A value that does not pass the filter

**Spec:** §8.7.4, p. 143 — "If the previous update to the instance passed the
filter, then the Writer must send a Data Submessage that either includes the
data value, or else indicates the InstanceState is ALIVE_FILTERED." "If the
previous update to the instance did not pass the filter, then the Writer may
omit sending the Data Submessage to the Reader."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — applies only to filtering on the writer side;
ZeroDDS sends every value.

### §8.7.4 Dispose and unregister message

**Spec:** §8.7.4, pp. 143–144 — "This so called “dispose/unregister message”
shall be sent if any of the following conditions is met: 1. The Reader does
not have a Content Filter. 2. The Writer has previously sent a Data message to
the Reader for that same instance. 3. The Reader has OWNERSHIP QosPolicy kind
EXCLUSIVE …"

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_lifecycle`: to every
matched reader)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done` — ZeroDDS sends the message in every case.

### §8.7.5 PID_GROUP_SEQ_NUM and PID_WRITER_GROUP_INFO

**Spec:** §8.7.5, p. 144 — "In order to support group ordered access, RTPS uses
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

**Spec:** §8.7.5, p. 144, Table 8.88 — "writerSet: GroupDigest_t — Identifies
the set of Writer EntityIds that are announced in the DiscoveredWriterData
that belonged to the Publisher at the time the sample was written."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/writer_group.rs::tests::the_writer_set_follows_the_writers`,
`crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`

**Status:** `done`

### §8.7.5 Group sequence number on every Data and the first DataFrag

**Spec:** §8.7.5, p. 144 — "When a Publisher is configured with access scope
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

### §8.7.5 Subscriber: commit by group sequence number

**Spec:** §8.7.5, pp. 144–145 — "A sample with GROUP sequence number GSN can be
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

### §8.7.5 Only writers with liveliness

**Spec:** §8.7.5, p. 145 — "The above rules should only take into consideration
DataWriters that have not lost their liveliness, see 8.7.2.2.3."

**Repo:** `crates/dcps/src/runtime.rs`
(`DcpsRuntime::user_reader_counted_writers`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_that_lost_its_liveliness_is_not_counted_for_the_group_order`

**Status:** `done`

### §8.7.6 Publisher and subscriber coherent sets

**Spec:** §8.7.6, p. 145 — "A “Publisher coherent set” is defined as the set of
all CacheChanges performed by all DataWriters in the Publisher delimited by the
operations begin_coherent_changes() and end_coherent_changes()." "What
constitutes a “Subscriber coherent set” depends on the PRESENTATION
access_scope of the Subscriber".

**Repo:** —

**Tests:** `crates/dcps/tests/group_access_e2e.rs::a_group_coherent_set_across_two_topics_appears_after_its_end_over_udp`,
`crates/dcps/tests/group_access_e2e.rs::a_group_coherent_set_across_two_topics_appears_after_its_end_same_runtime`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`

**Status:** `done`

### §8.7.6 Discard incomplete coherent sets

**Spec:** §8.7.6, p. 145 — "Incomplete coherent sets shall not be added to the
history of the RTPS DataReaders and the corresponding CacheChanges shall be
discarded by the Subscriber."

**Repo:** —

**Tests:** `crates/dcps/src/group_order.rs::tests::a_set_short_of_one_member_is_discarded_and_counted`,
`crates/rtps/src/reliable_reader.rs::tests::a_set_short_of_a_change_ends_incomplete`,
`crates/rtps/src/reliable_reader.rs::tests::a_set_ended_by_a_sample_is_reported_in_front_of_it_when_incomplete`,
`crates/dcps/src/subscriber.rs::tests::an_incomplete_set_is_discarded`,
`crates/dcps/tests/group_access_e2e.rs::a_reader_made_inside_a_set_does_not_get_its_tail_same_runtime`

**Status:** `done`

### §8.7.6 TOPIC scope: PID_COHERENT_SET with the first sequence number

**Spec:** §8.7.6, p. 146 — "a coherent set is uniquely identified by the
sequence number of the first sample update belonging to the coherent set. All
sample updates belonging to the same coherent set contain an in-line QoS
parameter with this same sequence number." "the same approach is followed for
DataFrag Submessages."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::coherent_set_member_data_carries_pid_0x0056_with_set_first_sn_octets`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`

**Status:** `done`

### §8.7.6 End of a writer's coherent set

**Spec:** §8.7.6, p. 146 — "The end of a Writer’s coherent set is defined by the
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

### §8.7.6 GROUP scope: three inline QoS parameters

**Spec:** §8.7.6, p. 146 — "All Data submessages and the first DataFrag
submessage belonging to the same group coherent set shall have three in-line
QoS parameters: The PID_GROUP_SEQ_NUM … The PID_COHERENT_SET … The
PID_GROUP_COHERENT_SET".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`

**Status:** `done`

### §8.7.6 ECS Data submessage from all writers

**Spec:** §8.7.6, p. 146 — "A group’s coherent set is marked as being finished
by sending an End Coherent Set (ECS) Data submessage from all Writers within
the Publisher." Properties: no serializedPayload, group sequence number of the
last sample + 1, not filtered, does not count towards resource limits, inline
QoS PID_GROUP_SEQ_NUM, PID_GROUP_COHERENT_SET, PID_WRITER_GROUP_INFO.

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/dcps/src/group_order.rs::tests::a_set_waits_for_the_end_of_every_counted_writer`

**Status:** `done`

### §8.7.6 No writers added or removed during a coherent set

**Spec:** §8.7.6, p. 146 — "Note that Writers are not allowed to be added or
removed from a Publisher from the time that a coherent set begins until after
it ends."

**Repo:** —

**Tests:** `crates/dcps/tests/group_access_e2e.rs::a_writer_is_not_created_or_deleted_while_a_group_coherent_set_is_open`

**Status:** `done`

### §8.7.6 DataReader: commit a group coherent set

**Spec:** §8.7.6, p. 147 — "The group coherent set becomes ready to be
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

### §8.7.6 Inline QoS of a coherent-set member on the first DataFrag

**Spec:** §8.7.6, p. 146 — "The remainder of the discussion only refers to Data
Submessages, but the same approach is followed for DataFrag Submessages."

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_oversized_coherent_set_member_goes_out_as_marked_fragments`,
`crates/rtps/src/fragment_assembler.rs::tests::the_first_fragments_inline_qos_reaches_the_completed_sample`

**Status:** `done`

### §8.7.7 Directed write: PID_DIRECTED_WRITE

**Spec:** §8.7.7, p. 147 — "RTPS supports directed writes by using the in-line
QoS parameter extension mechanism. The serialized information denotes the
GUIDs of the targeted reader(s)."

**Repo:** `crates/rtps/src/inline_qos.rs` (`directed_write_param`,
`find_directed_write`, `directed_write_accepts`); `crates/rtps/src/parameter_list.rs`
(`pid::DIRECTED_WRITE`); `crates/rtps/src/reliable_reader.rs` (`addressed_elsewhere`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::directed_write_param_carries_target_guid`,
`crates/rtps/src/inline_qos.rs::tests::find_directed_write_returns_target_when_present`,
`crates/rtps/src/reliable_reader.rs::tests::a_directed_sample_is_absorbed_by_a_reader_it_does_not_name`

**Status:** `done` — encoding and reading of the parameter; the receive path reads
every PID_DIRECTED_WRITE of a DATA, not only the first. No ZeroDDS writer
sends a directed write; the spec offers it to the writer and does not require
it.

### §8.7.7 Directed write: only the addressed reader takes it

**Spec:** §8.7.7, p. 147 — "When a writer sends a directed sample, only
recipients with a matching GUID accept the sample; all other recipients
acknowledge but absorb the sample, as if it were a GAP message."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`addressed_elsewhere`,
`irrelevant_change_set`); `crates/rtps/src/inline_qos.rs` (`directed_write_accepts`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_directed_sample_is_absorbed_by_a_reader_it_does_not_name`,
`crates/rtps/src/inline_qos.rs::tests::directed_write_matches_reader_returns_false_for_other_target`

**Status:** `done` — a reader that no PID_DIRECTED_WRITE of the DATA names
acknowledges the sample and absorbs it as after a GAP; a named reader takes
it. Measured: green on the Linux test host.

### §8.7.8 Property lists

**Spec:** §8.7.8, p. 147 — "The RTPS protocol supports Property Lists as
in-line parameters. Properties can then be propagated during Discovery or as
in-line QoS."

**Repo:** `crates/rtps/src/property_list.rs` (`WirePropertyList`);
`crates/rtps/src/participant_data.rs` (`properties`, PID_PROPERTY_LIST)

**Tests:** `crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — ZeroDDS carries property lists in SPDP; not in SEDP and
not as inline QoS.

### §8.7.9 Original writer info: mark the forwarding

**Spec:** §8.7.9, p. 147 — "This service that forwards messages needs to
indicate that the forwarded message belongs to the message-stream of another
writer, such that if the reader receives the same messages from another source
… it can treat them as duplicates."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_sample_on_behalf`,
`write_user_lifecycle_on_behalf`, `write_user_sample_relayed`);
`crates/durability-service/src/lib.rs` (`write_replay_sample`);
`crates/routing-service/src/forwarding.rs` (relay mode)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_sample_written_on_behalf_of_another_writer_is_that_writers`,
`crates/dcps/src/runtime.rs::tests::a_dispose_written_on_behalf_of_another_writer_is_that_writers`,
`crates/durability-service/src/lib.rs::tests::a_replayed_sample_goes_out_on_behalf_of_its_writer`,
`crates/routing-service/tests/relay_route.rs::a_relay_route_forwards_on_behalf_of_the_writer`,
`crates/rtps/src/reliable_writer.rs::tests::a_lifecycle_marker_keeps_its_own_inline_qos_on_every_send`

**Status:** `done` — a writer sends a sample, a dispose or an unregister on behalf of
another with PID_ORIGINAL_WRITER_INFO, on the first send and on every
repair. The durability service sends its stored samples so, the routing
service in relay mode (`Route::relay`) every sample and every lifecycle
marker. Measured: green on the Linux test host.

### §8.7.9 Original writer info: hand-over to the reader of the original

**Spec:** §8.7.9, p. 147 — "When a RTPS Reader receives this information, it
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

**Status:** `done` — a change with PID_ORIGINAL_WRITER_INFO goes out as the
original writer's, with its sequence number, also where the reader has no
proxy of the original; one the reader has from the original or another
forwarder already is a duplicate. A writer the reader does not trust as
forwarder delivers as itself (under DDS-Security only with `<relay>` in the
grant, `dds-security-1.2` §7.1.1.4). Measured: green on the Linux test host.

### §8.7.9 Table 8.89 originalWriterGUID

**Spec:** §8.7.9, p. 147, Table 8.89 — "originalWriterGUID: GUID_t — The GUID
of the RTPS Writer that first generated the message."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_be`

**Status:** `done` — encoded, sent and evaluated on receipt (items under §8.7.9).

### §8.7.9 Table 8.89 originalWriterSN

**Spec:** §8.7.9, p. 147, Table 8.89 — "originalWriterSN: SequenceNumber_t —
The Sequence Number of the CacheChange as sent from the original writer."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`

**Status:** `done` — encoded, sent and evaluated on receipt (items under §8.7.9).

### §8.7.10 Compute the KeyHash from the data

**Spec:** §8.7.10, p. 148 — "When the key hash is not received by a DataReader,
it should be computed from the data itself. If there is no data in the
submessage, then a default zero-valued key hash should be used by the
DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`reader_key_hash_context`);
`crates/dcps/src/key_hash.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`,
`crates/dcps/src/runtime.rs::tests::lifecycle_marker_without_key_hash_gets_the_alive_instances_hash_and_holder`

**Status:** `done`

### §8.7.10 Compute the KeyHash by §9.6.4.8

**Spec:** §8.7.10, p. 148 — "A Key Hash, if present, shall be computed as
described in 9.6.4.3."

**Repo:** `crates/dcps/src/key_hash.rs`

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_holder_over_sixteen_is_hashed`,
`crates/dcps/src/key_hash.rs::tests::a_u32_goes_out_big_endian`

**Status:** `done` — the reference to §9.6.4.3 is wrong in the spec; the
computation is in §9.6.4.8.

## §9 Platform Specific Model (PSM): UDP/IP

### §9.1 PSM UDP/IP: introduction

**Spec:** §9.1, p. 149 — "This clause defines the Platform Specific Model (PSM)
that maps the Protocol PIM to UDP/IP."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale for the choice of UDP/IP, no
requirement.

### §9.2.1 Name space

**Spec:** §9.2.1, p. 149 — "All the definitions in this document are part of
the “RTPS” name-space."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — convention of the document.

### §9.2.2 IDL representation and CDR wire form

**Spec:** §9.2.2, p. 149 — "These definitions use the OMG IDL … When these
structures are sent on the wire, they are encoded using the corresponding CDR
representation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — convention; the individual mappings are
listed under §9.3 and §9.4.

### §9.2.3 Representation of bits and bytes

**Spec:** §9.2.3, pp. 149–150 — "the leftmost bit (bit 7) is the most
significant bit"; bytes in rows of four.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation of the document.

### §9.3.1 GUID from GuidPrefix and EntityId

**Spec:** §9.3.1, p. 150 — "The PIM defines the GUID as composed of a
GuidPrefix_t prefix capable of holding 12 bytes, and an EntityId_t entityId
capable of holding 4 bytes."

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`, `Guid::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`,
`crates/rtps/src/wire_types.rs::tests::guid_roundtrip`

**Status:** `done`

### §9.3.1.1 GuidPrefix_t

**Spec:** §9.3.1.1, p. 150 — "typedef octet GuidPrefix_t[12];"

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`

**Status:** `done`

### §9.3.1.1 GUIDPREFIX_UNKNOWN

**Spec:** §9.3.1.1, p. 150 — "#define GUIDPREFIX_UNKNOWN
{0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00}".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`

**Status:** `done`

### §9.3.1.2 EntityId_t

**Spec:** §9.3.1.2, p. 150 — "struct EntityId_t { OctetArray3 entityKey; octet
entityKind; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::entity_id_user_writer_with_key_layout`

**Status:** `done`

### §9.3.1.2 ENTITYID_UNKNOWN

**Spec:** §9.3.1.2, p. 150 — "#define ENTITYID_UNKNOWN {{0x00, 0x00, 0x00},
0x00}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done`

### §9.3.1.2 entityKey unique within the participant

**Spec:** §9.3.1.2, p. 150 — "When not pre-defined (see below), the entityKey
field within the EntityId_t can be chosen arbitrarily by the middleware
implementation as long as the resulting EntityId_t is unique within the
Participant."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::next_entity_key`, `entity_key_in_use`,
`pinned_entity_keys`, `new_group_entity_id`, `reserve_user_writer_id`, `reserve_user_reader_id`)

**Tests:** `crates/dcps/src/runtime.rs::tests::entity_keys_are_unique_across_groups_reserved_and_registered_endpoints`,
`crates/dcps/src/runtime.rs::tests::a_wrapped_entity_key_counter_skips_the_key_of_a_live_writer`

**Status:** `done` — the counter (24 bit) hands out every key once; after the wrap it skips the keys of registered writers, readers and announced topics and those of Publishers, Subscribers and of reserved, not yet registered endpoints. The key of a deleted endpoint is handed out again; a reserved key that is never registered stays taken.

### §9.3.1.2 entityKind: kind of entity in the two upper bits

**Spec:** §9.3.1.2, p. 150 — "The information on whether the object is a
built-in entity, a vendor-specific entity, or a user-defined entity is encoded
in the two most-significant bits of the entityKind." "a vendor-specific
Entity … can therefore be ignored by another vendor’s implementation".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::from_byte`,
`EntityKind::Other`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unnamed_kind_byte_is_kept`

**Status:** `done`

### §9.3.1.2 Table 9.1 unknown

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "unknown — 0x00 — 0xc0".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Unknown`,
`EntityKind::Other(0xC0)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.2 Table 9.1 Participant

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Participant — N/A — 0xc1".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Participant`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer (with Key)

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Writer (with Key) — 0x02 — 0xc2".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserWriterWithKey`,
`BuiltinWriterWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer (no Key)

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Writer (no Key) — 0x03 — 0xc3".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserWriterNoKey`,
`BuiltinWriterNoKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader (no Key)

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Reader (no Key) — 0x04 — 0xc4".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserReaderNoKey`,
`BuiltinReaderNoKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader (with Key)

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Reader (with Key) — 0x07 — 0xc7".

**Repo:** `crates/rtps/src/wire_types.rs` (`UserReaderWithKey`,
`BuiltinReaderWithKey`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`

**Status:** `done`

### §9.3.1.2 Table 9.1 Writer Group

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Writer Group — 0x08 — 0xc8".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Other`, octet kept
unchanged)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.2 Table 9.1 Reader Group

**Spec:** §9.3.1.2, p. 151, Table 9.1 — "Reader Group — 0x09 — 0xc9".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityKind::Other`, octet kept
unchanged)

**Tests:** `crates/rtps/src/wire_types.rs::tests::topic_kind_follows_entity_kind_octet_of_table_9_1`,
`crates/rtps/src/wire_types.rs::tests::entity_kind_octet_round_trips`

**Status:** `done`

### §9.3.1.3 Meaning of the predefined EntityIds

**Spec:** §9.3.1.3, p. 151 — "The meaning of these Entity IDs cannot change in
this major version (2) of the protocol, but future minor versions may add
additional reserved Entity IDs."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a requirement on future editions of the
specification.

### §9.3.1.3 Table 9.2 ENTITYID_PARTICIPANT

**Spec:** §9.3.1.3, p. 151, Table 9.2 — "ENTITYID_PARTICIPANT = {{00,00,01},c1}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::PARTICIPANT`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_participant_constant`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER

**Spec:** §9.3.1.3, p. 151, Table 9.2 — "ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER =
{{00,00,02},c2}".

**Repo:** `EntityId::SEDP_BUILTIN_TOPICS_WRITER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — the constant carries {{00,00,02},c2}; the test checks all values of Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR

**Spec:** §9.3.1.3, p. 151, Table 9.2 — "ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR =
{{00,00,02},c7}".

**Repo:** `EntityId::SEDP_BUILTIN_TOPICS_READER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — the constant carries {{00,00,02},c7}; the test checks all values of Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER = {{00,00,03},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_publications_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_PUBLICATIONS_DETECTOR

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_PUBLICATIONS_DETECTOR = {{00,00,03},c7}".

**Repo:** `EntityId::SEDP_BUILTIN_PUBLICATIONS_READER` (`crates/rtps/src/wire_types.rs`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_entity_ids_of_table_9_2_are_those_of_the_table`

**Status:** `done` — the constant carries {{00,00,03},c7}; the test checks all values of Table 9.2.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_ANNOUNCER

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_ANNOUNCER = {{00,00,04},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER`)

**Tests:** `crates/discovery/tests/vendor_discovery_frames.rs::fastdds_announcements_read_and_write_back`

**Status:** `done` — the test sorts captured announcements by this EntityId.

### §9.3.1.3 Table 9.2 ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_DETECTOR

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_DETECTOR = {{00,00,04},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sedp_subscriptions_reader_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SPDP_BUILTIN_PARTICIPANT_ANNOUNCER

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SPDP_BUILTIN_PARTICIPANT_ANNOUNCER = {{00,01,00},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_SPDP_BUILTIN_PARTICIPANT_DETECTOR = {{00,01,00},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::SPDP_BUILTIN_PARTICIPANT_READER`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_builtin_participant_reader_layout`

**Status:** `done`

### §9.3.1.3 Table 9.2 ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER = {{00,02,00},c2}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER`)

**Tests:** `crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done` — the test runs only with `--ignored` and a Cyclone
`ddsperf`.

### §9.3.1.3 Table 9.2 ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER

**Spec:** §9.3.1.3, p. 151, Table 9.2 —
"ENTITYID_P2P_BUILTIN_PARTICIPANT_MESSAGE_READER = {{00,02,00},c7}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER`)

**Tests:** `crates/dcps/tests/cyclone_live_wlp.rs::cyclone_live_wlp_handshake`

**Status:** `done` — the test runs only with `--ignored` and a Cyclone
`ddsperf`.

### §9.3.1.3 Table 9.3 DDS-Security: range {ff,00,00}–{ff,ff,ff}

**Spec:** §9.3.1.3, p. 152, Table 9.3 — "EntityIds that have both an entityKey
in the range {ff, 00, 00} – {ff, ff, ff} and an entityKind in the range
0xc0-0xff (inclusive)."

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER` and the other secure
built-in EntityIds)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_12_secure_entityids_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** `done`

### §9.3.1.3 Table 9.3 DDS-Security: {{00,02,01},c3} and {{00,02,01},c4}

**Spec:** §9.3.1.3, p. 152, Table 9.3 — "DDS-Security 1.1 — {{00, 02, 01},
c3} and {{00, 02, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs`
(`EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`,
`EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER`)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/stateless.rs::tests::reader_has_expected_entity_id`

**Status:** `done`

### §9.3.1.3 Table 9.3 DDS-XTypes: TypeLookup EntityIds

**Spec:** §9.3.1.3, p. 152, Table 9.3 — "DDS-XTypes 1.2 — {{00, 03, 00}, c3},
{{00, 03, 00}, c4}, {{00, 03, 01}, c3}, {{00, 03, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::TL_SVC_REQ_WRITER`,
`TL_SVC_REQ_READER`, `TL_SVC_REPLY_WRITER`, `TL_SVC_REPLY_READER`)

**Tests:** `crates/discovery/src/type_lookup/endpoints.rs::tests::entity_ids_match_spec_constants`

**Status:** `done`

### §9.3.1.4 Table 9.4 Client (0x05)

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "Client — 0x05"; "These EntityIds
should not be used by future versions of the protocol unless they are used with
the same meaning as in versions prior to 2.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 Server (0x06)

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "Server — 0x06".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 writerApplications

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "writerApplications — {{00,00,01},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 readerApplications

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "readerApplications — {{00,00,01},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 writerClients

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "writerClients — {{00,00,05},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 readerClients

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "readerClients — {{00,00,05},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 writerServices

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "writerServices — {{00,00,06},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 readerServices

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "readerServices — {{00,00,06},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 writerManagers

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "writerManagers — {{00,00,07},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 readerManagers

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "readerManagers — {{00,00,07},c7}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.4 Table 9.4 writerApplicationsSelf

**Spec:** §9.3.1.4, p. 152, Table 9.4 — "writerApplicationsSelf —
{{00,00,08},c2}".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated id; ZeroDDS does not use it.

### §9.3.1.5 GUID_t

**Spec:** §9.3.1.5, p. 152 — "struct GUID_t { GuidPrefix_t guidPrefix;
EntityId_t entityId; } ;"

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_layout_is_prefix_then_entity_id`

**Status:** `done`

### §9.3.1.5 First two octets of the guidPrefix = vendorId

**Spec:** §9.3.1.5, pp. 152–153 — "implementations of the RTPS protocol shall
set the first two bytes of the guidPrefix to match their assigned vendorId …
guidPrefix[0] = vendorId[0] guidPrefix[1] = vendorId[1]".

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::zerodds`);
`crates/dcps/src/participant.rs` (`random_guid_prefix`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_host_id_follows_the_vendor_id`

**Status:** `partial` — the first two octets are `01 F0`, not a value assigned
by the OMG (`open` item under §8.3.5.2). `DcpsRuntime::start` also accepts a
freely chosen prefix; under DDS-Security the prefix derived from the identity
replaces the one built. DDS-Security 1.2 §10.3.3 Table 59
(`validate_local_identity`) sets its first 48 bits: the first bit 1, then 47
bits of the SHA-256 of the SubjectName; by the more specific spec the vendorId
is not in them (`crates/security-pki/src/guid_adjust.rs`, test
`adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`). Blocker: a hard external one, the assignment of a VendorId by the OMG; the request has been pending since 22 June. Until then ZeroDDS sends `0x01F0`, a value the OMG list does not hold (assigned up to `{0x01,0x1C}`, `{0x01,0x1D}` and `{0x01,0x1E}` reserved).

### §9.3.1.5 GUID_UNKNOWN

**Spec:** §9.3.1.5, p. 153 — "#define GUID_UNKNOWN{ GUIDPREFIX_UNKNOWN,
ENTITYID_UNKNOWN }".

**Repo:** `crates/rtps/src/wire_types.rs` (`Guid::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_unknown_is_zero`,
`crates/rtps/src/wire_types.rs::tests::entity_id_unknown_is_four_zero_octets`

**Status:** `done`

### §9.3.2.1 DomainId_t

**Spec:** §9.3.2.1, p. 153 — "typedef unsigned long DomainId_t;"

**Repo:** `crates/rtps/src/participant_data.rs` (`domain_id: Option<u32>`,
PID_DOMAIN_ID with 4 octets)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.3.2.1 Time_t structure

**Spec:** §9.3.2.1, p. 153 — "struct Time_t { unsigned long seconds; // time in
seconds unsigned long fraction; // time in sec/2^32 };"

**Repo:** `crates/rtps/src/header_extension.rs` (`HeTimestamp`);
`crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`

**Status:** `done` — `seconds` and `fraction` are each an `unsigned long` (`HeTimestamp::seconds: u32`), 4 + 4 octets in the byte order of the submessage; second values from 2^31 on stay positive (the test `roundtrip_all_fields_set_be` uses 0xFFFF_FFFD). §9.4.2.9 shows `long seconds`; the structure of §9.3.2.1 applies.

### §9.3.2.1 Time_t: reserved values and unsigned seconds

**Spec:** §9.3.2.1, p. 153 — "TIME_ZERO: seconds = 0, fraction = 0;
TIME_INVALID: seconds = 0xffffffff, fraction = 0xffffffff; TIME_INFINITE:
seconds = 0xffffffff, fraction = 0xfffffffe".

**Repo:** `crates/dcps/src/time.rs` (`time_to_he_timestamp`, `he_timestamp_to_time`); `crates/rtps/src/header_extension.rs` (`HeTimestamp`, `TIME_INVALID`)

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_time_t_values_cross_the_wire_as_the_spec_gives_them`,
`crates/rtps/src/receiver_state.rs::tests::the_initial_state_is_the_one_of_table_8_17`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`

**Status:** `done` — `Time::INVALID` goes on the wire as {0xffffffff, 0xffffffff} and `Time::INFINITE` as {0xffffffff, 0xfffffffe} and is read back so; `HeTimestamp::seconds` is `u32`, `TIME_INVALID` has seconds 0xFFFFFFFF. The callers in dcps, xrce-agent and opcua-bridge are adapted (`cargo check --tests` and clippy green). Whether another vendor sends the special values as a source timestamp is not checked.

### §9.3.2.1 Duration_t, DURATION_ZERO, DURATION_INFINITE

**Spec:** §9.3.2.1, p. 153 — "DURATION_ZERO: seconds = 0, fraction = 0;
DURATION_INFINITE: seconds = 0x7fffffff, fraction = 0xffffffff; struct
Duration_t { long seconds; unsigned long fraction; };"

**Repo:** `crates/qos/src/duration.rs` (`Duration::INFINITE`, `Duration::ZERO`)

**Tests:** `crates/qos/src/duration.rs::tests::infinite_constant_matches_spec`,
`crates/qos/src/duration.rs::tests::zero_is_default_and_zero`

**Status:** `done`

### §9.3.2.1 VendorId_t and VENDORID_UNKNOWN

**Spec:** §9.3.2.1, p. 153 — "VENDORID_UNKNOWN: VendorId_t[0] = 0,
VendorId_t[1] = 0; typedef octet VendorId_t[2];"

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId`, `VendorId::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_unknown_is_two_zero_octets`,
`crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`

**Status:** `done`

### §9.3.2.1 SEQUENCENUMBER_UNKNOWN

**Spec:** §9.3.2.1, p. 153 — "SEQUENCENUMBER_UNKNOWN: high = -1, low = 0".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::UNKNOWN`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_unknown_high_minus_one_low_zero`

**Status:** `done`

### §9.3.2.1 FragmentNumber_t

**Spec:** §9.3.2.1, p. 153 — "typedef unsigned long FragmentNumber_t;"

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber(u32)`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`,
`crates/rtps/src/wire_types.rs::tests::fragment_number_wire_size_is_four`

**Status:** `done`

### §9.3.2.1 LOCATOR_KIND_INVALID, _RESERVED, _UDPv4, _UDPv6

**Spec:** §9.3.2.1, pp. 153–154 — "const long LOCATOR_KIND_INVALID = -1; const
long LOCATOR_KIND_RESERVED = 0; const long LOCATOR_KIND_UDPv4 = 1; const long
LOCATOR_KIND_UDPv6 = 2;"

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Invalid`, `Reserved`,
`UdpV4`, `UdpV6`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_reserved_constant_kind_is_zero`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v4_any_kind_is_one`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v6_any_kind_is_two`

**Status:** `done`

### §9.3.2.1 LOCATOR_PORT_INVALID, LOCATOR_ADDRESS_INVALID, LOCATOR_INVALID

**Spec:** §9.3.2.1, p. 154 — "const unsigned long LOCATOR_PORT_INVALID = 0;
LOCATOR_ADDRESS_INVALID: {0,…,0}; LOCATOR_INVALID: kind = LOCATOR_KIND_INVALID,
port = LOCATOR_PORT_INVALID, address = LOCATOR_ADDRESS_INVALID".

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::INVALID`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_invalid_constant_matches_spec`,
`crates/rtps/src/wire_types.rs::tests::locator_invalid_kind_decoded`

**Status:** `done`

### §9.3.2.1 Locator_t

**Spec:** §9.3.2.1, p. 154 — "struct Locator_t { long kind; unsigned long
port; octet address[16]; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator`, `Locator::to_bytes_le`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`,
`crates/rtps/src/wire_types.rs::tests::locator_roundtrip_le`

**Status:** `done`

### §9.3.2.1 BEST_EFFORT = 1, RELIABLE = 2

**Spec:** §9.3.2.1, p. 154 — "The values of the following constants as defined
in the DDS Specification should be mapped to the below values before being sent
on the wire. const long BEST_EFFORT = 1; const long RELIABLE = 2;"

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind::BestEffort
= 1`, `Reliable = 2`); `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::reliability_try_from_u32_rejects_unknown`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_lt_reliable`

**Status:** `done`

### §9.3.2.1 Count_t

**Spec:** §9.3.2.1, p. 154 — "typedef long Count_t;"

**Repo:** `crates/rtps/src/submessages.rs` (`count: i32` in ACKNACK,
HEARTBEAT, HEARTBEAT_FRAG, NACK_FRAG)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`

**Status:** `done`

### §9.3.2.1 ProtocolVersion_t

**Spec:** §9.3.2.1, p. 154 — "struct ProtocolVersion_t { octet major; octet
minor; };"

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_roundtrip`,
`crates/rtps/src/wire_types.rs::tests::protocol_version_aliases_match_spec`

**Status:** `done`

### §9.3.2.1 KeyHash_t, StatusInfo_t, ParameterId_t

**Spec:** §9.3.2.1, p. 154 — "typedef octet KeyHash_t[16]; typedef octet
StatusInfo_t[4]; typedef short ParameterId_t;"

**Repo:** `crates/rtps/src/inline_qos.rs` (PID_KEY_HASH with 16 octets,
PID_STATUS_INFO with 4 octets); `crates/rtps/src/parameter_list.rs`
(`Parameter::id: u16`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done` — ParameterId_t is kept as `u16`; the bits 0x8000 and 0x4000
are evaluated as bit fields (§9.4.2.11.2).

### §9.3.2.1 ContentFilterProperty_t

**Spec:** §9.3.2.1, p. 154 — "struct ContentFilterProperty_t { string<256>
contentFilteredTopicName; string<256> relatedTopicName; string<256>
filterClassName; string filterExpression; sequence<string>
expressionParameters; };"

**Repo:** `crates/rtps/src/subscription_data.rs` (`ContentFilterProperty`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_property_roundtrip_le`,
`crates/rtps/src/subscription_data.rs::tests::content_filter_names_are_bounded_by_string_256`

**Status:** `done` — order, encoding and the bound of 256 octets for the three names.

### §9.3.2.1 ContentFilterProperty_t: string<256>

**Spec:** §9.3.2.1, p. 154 — "string<256> contentFilteredTopicName;
string<256> relatedTopicName; string<256> filterClassName;"

**Repo:** `crates/rtps/src/subscription_data.rs` (`encode_content_filter_property_le`,
`decode_content_filter_property`); `crates/dcps/src/participant.rs` (`create_contentfilteredtopic`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::content_filter_names_are_bounded_by_string_256`

**Status:** `done` — encoding and decoding bound the three names to 256 octets; `create_contentfilteredtopic` refuses a longer name. A ContentFilterProperty with a name that is too long or otherwise unreadable counts as absent: the reader is taken without a filter (it filters itself), counted in `DcpsRuntime::unreadable_content_filters` and logged as `sedp.content_filter.unreadable`.

### §9.3.2.1 ContentFilterInfo_t, FilterResult_t, FilterSignature_t

**Spec:** §9.3.2.1, p. 154 — "typedef sequence<long> FilterResult_t; typedef
long FilterSignature_t[4]; … struct ContentFilterInfo_t { FilterResult_t
filterResult; FilterSignatureSequence filterSignatures; };"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — PID_CONTENT_FILTER_INFO belongs to filtering
on the writer side, which ZeroDDS does not do (§8.7.3.4).

### §9.3.2.1 Property_t and EntityName_t

**Spec:** §9.3.2.1, p. 154 — "struct Property_t { string name; string value;
}; typedef string EntityName_t;"

**Repo:** `crates/rtps/src/property_list.rs` (`WirePropertyList`)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`,
`crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`

**Status:** `done` — EntityName_t (PID_ENTITY_NAME) is listed under §9.6.3.2.

### §9.3.2.1 OriginalWriterInfo_t

**Spec:** §9.3.2.1, p. 154 — "struct OriginalWriterInfo_t { GUID_t
originalWriterGUID; SequenceNumber_t originalWriterSN; ParameterList
originalWriterQos; };"

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`, `find_original_writer_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — the wire form of the parameter is `originalWriterGUID` and `originalWriterSN`, 24 octets (§9.6.4.7; Table 8.89 names the same two attributes); `originalWriterQos` of the IDL structure has no counterpart in the wire form or in Table 8.89. The send and receive paths are `done` under §8.7.9.

### §9.3.2.1 GroupDigest_t

**Spec:** §9.3.2.1, p. 155 — "typedef octet GroupDigest_t[4];"

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §9.3.2.1 BuiltinEndpointSet_t (IDL)

**Spec:** §9.3.2.1, p. 155 — "bitmask BuiltinEndpointSet_t { @position(0)
DISC_BUILTIN_ENDPOINT_PARTICIPANT_ANNOUNCER, … @position(29)
DISC_BUILTIN_ENDPOINT_TOPICS_DETECTOR };"

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done` — the positions are listed one by one under §9.3.2.12.

### §9.3.2.1 BuiltinEndpointQos_t

**Spec:** §9.3.2.1, p. 155 — "bitmask BuiltinEndpointQos_t { @position(0)
BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER };"

**Repo:** `crates/rtps/src/participant_data.rs` (`builtin_endpoint_qos::BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/dcps/src/wlp.rs::tests::the_reader_is_treated_as_builtin_endpoint_qos_says`

**Status:** `done` — bitmask with `@position(0)` BEST_EFFORT_PARTICIPANT_MESSAGE_DATA_READER.

### §9.3.2.1 ProtocolId_t (PROTOCOL_RTPS)

**Spec:** §9.3.2.1, p. 155 — "PROTOCOL_RTPS: ProtocolId_t[0] = 'R' …
ProtocolId_t[3] = 'S'; typedef octet ProtocolId_t[4];"

**Repo:** `crates/rtps/src/header.rs` (`RTPS_MAGIC`)

**Tests:** `crates/rtps/src/header.rs::tests::header_layout_first_four_bytes_are_rtps_magic`

**Status:** `done`

### §9.3.2.1 MessageLength_t and MESSAGE_LENGTH_INVALID

**Spec:** §9.3.2.1, p. 155 — "typedef unsigned long MessageLength_t; const
MessageLength_t MESSAGE_LENGTH_INVALID = 0;"

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length: Option<u32>`, `MESSAGE_LENGTH_INVALID`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_message_length_only_le`,
`crates/rtps/src/datagram.rs::tests::a_header_extension_announcing_message_length_invalid_refuses_the_message`

**Status:** `done` — `u32`, 32 bit unsigned; `MESSAGE_LENGTH_INVALID` is 0 and the initial value of the receiver (`None`). A HeaderExtension with that value is invalid (messageLength smaller than the RTPS header and the HeaderExtension, §8.3.7.3); the whole message is refused.

### §9.3.2.1 UExtension4_t, WExtension8_t, Checksum32_t, Checksum64_t, Checksum128_t

**Spec:** §9.3.2.1, p. 155 — "typedef octet UExtension4_t[4]; typedef octet
WExtension8_t[8]; typedef octet Checksum32_t[4]; typedef octet
Checksum64_t[8]; typedef octet Checksum128_t[16];"

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4: [u8; 4]`,
`wextension8: [u8; 8]`, `ChecksumValue`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_le`,
`crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`

**Status:** `done` — the byte order of the checksums is listed under
§9.4.2.15.

### §9.3.2.2 Time_t in NTP format

**Spec:** §9.3.2.2, p. 156 — "time = seconds + (fraction / 2^(32))".

**Repo:** `crates/dcps/src/time.rs` (`time_to_he_timestamp`,
`he_timestamp_to_time`); `crates/qos/src/duration.rs`
(`fraction_of_nanosec`)

**Tests:** `crates/qos/src/duration.rs::tests::a_whole_nanosecond_survives_the_way_there_and_back`,
`crates/rtps/tests/reliable_e2e.rs::e2e_source_timestamp_round_trips`

**Status:** `done`

### §9.3.2.3 Duration_t in NTP format

**Spec:** §9.3.2.3, p. 156 — "time = seconds + (fraction / 2^(32))". "Versions
of the RTPS specification previous to version 2.4 did not specify the
representation of Duration_t, therefore implementations should take into
account the vendor and protocol version when interpreting these fields."

**Repo:** `crates/qos/src/duration.rs` (`Duration`, `to_bytes_le`,
`from_bytes_le`)

**Tests:** `crates/qos/src/duration.rs::tests::from_millis_splits_correctly`,
`crates/qos/src/duration.rs::tests::to_from_bytes_le_roundtrip`

**Status:** `done` — there is no distinction by vendor and protocol version
before 2.4.

### §9.3.2.4 Locator_t: IPv4 address

**Spec:** §9.3.2.4, p. 156 — "If the Locator_t kind is LOCATOR_KIND_UPDv4, the
address contains an IPv4 address. In this case, the leading 12 octets of the
address must be zero."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::udp_v4`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v4_layout`

**Status:** `done`

### §9.3.2.4 Locator_t: IPv6 address

**Spec:** §9.3.2.4, p. 156 — "If the Locator_t kind is LOCATOR_KIND_UPDv6, the
address contains an IPv6 address."

**Repo:** `crates/rtps/src/wire_types.rs` (`Locator::udp_v6`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::locator_udp_v6_constructor_keeps_full_address`,
`crates/rtps/src/wire_types.rs::tests::locator_udp_v6_roundtrip_le`

**Status:** `done`

### §9.3.2.4 Ranges of the locator kinds

**Spec:** §9.3.2.4, p. 156 — "0x00000003 - 0x01ffffff (inclusive) are reserved
for vendor-specific Locator_t kinds … 0x02000000 - 0x02ffffff (inclusive) are
reserved for future use by the RTPS specification … 0x03000000 and greater are
reserved for Locator_t kinds that identify a transport developed by a
third-party".

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind`, `LocatorKind::from_i32`, `set_tcp_kinds_fast_dds`);
`crates/dcps/src/runtime.rs` (`RuntimeConfig::tcp_locator_kinds_fast_dds`, key `transport.tcp_locator_kinds`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_zerodds_locator_kinds_are_in_the_vendor_specific_range`,
`crates/rtps/src/wire_types.rs::tests::the_tcp_kinds_are_vendor_range_by_default_and_fast_dds_by_setting`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_rejects_unknown`,
`crates/transport-tcp/src/tcp_transport.rs::tests::locator_kinds_are_zerodds_vendor_range_values`,
`crates/dcps/tests/runtime_config_layering.rs::the_tcp_locator_kinds_are_a_setting`,
`crates/transport-shm/src/posix.rs::tests::locator_kind_is_the_vendor_reserved_shm_value`,
`crates/transport-uds/src/lib.rs::tests::locator_kind_is_the_vendor_reserved_uds_value`,
`crates/rtps/src/submessages.rs::tests::info_reply_leaves_out_a_locator_of_a_kind_it_does_not_know`

**Status:** `done` — all kinds besides UDPv4 (1) and UDPv6 (2) lie in the vendor range 3 to 0x01ffffff: `Shm`, `Uds`, `Tsn` (0x01F0_0001 to 0x01F0_0003) and now `Tcpv4` (0x01F0_0004) and `Tcpv6` (0x01F0_0005). ZeroDDS sends the values 4 and 8 of Fast DDS (and of the DDS-TCP-PSM) only with the named setting `transport.tcp_locator_kinds: fast-dds` (environment `ZERODDS_TCP_LOCATOR_KINDS`, man page, example configuration); both values are read. A kind the receiver does not know does not invalidate an INFO_REPLY, the locator is left out. Expected Fast DDS behaviour: with `fast-dds` it recognises the TCP locators of ZeroDDS (kinds 4 and 8); without the setting it ignores kinds in the vendor range and opens no TCP connection to ZeroDDS. Not measured.

### §9.3.2.5 GroupDigest_t

**Spec:** §9.3.2.5, p. 156 — "The GroupDigest_t is computed from an
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

**Spec:** §9.3.2.6, p. 157 — "Checksum32_t represents a 32-bit checksum.
Checksum64_t, and Checksum128_t represent a 64-bit, and 128-bit checksum,
respectively."

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::Crc32c`,
`Crc64`, `Md5`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_wire_size`,
`crates/rtps/src/header_extension.rs::tests::checksum_value_kind_matches`

**Status:** `done`

### §9.3.2.7 MessageLength_t

**Spec:** §9.3.2.7, p. 157 — "This type is used to represent the length of an
RTPS message. The representation uses a 32-bit unsigned integer."

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length: Option<u32>`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`,
`crates/rtps/src/header_extension.rs::tests::wire_layout_be_message_length_correct`

**Status:** `done`

### §9.3.2.8 UExtension4_t

**Spec:** §9.3.2.8, p. 157 — "This type is used to represent an undefined
4-byte value."

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4: [u8; 4]`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`

**Status:** `done`

### §9.3.2.9 WExtension8_t

**Spec:** §9.3.2.9, p. 157 — "This type is used to represent an undefined
8-byte value."

**Repo:** `crates/rtps/src/header_extension.rs` (`wextension8: [u8; 8]`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`

**Status:** `done`

### §9.3.2.10 SequenceNumber_t

**Spec:** §9.3.2.10, p. 157 — "struct SequenceNumber_t { long high; unsigned
long low; }; … sequence_number = low + high * 2^(32)".

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::to_bytes_le`,
`from_bytes_le`, `to_bytes_be`, `from_bytes_be`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_split_high_low`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §9.3.2.11 ChangeCount_t

**Spec:** §9.3.2.11, p. 157 — "This type is used to represent a 64-bit count.
… struct ChangeCount_t { long high; unsigned long low; }; … change_count = low
+ high * 2^(32)".

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::filtered_count: Option<u64>`, `write_body`, `read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done` — `ChangeCount_t` is held as `u64`: `high` (`long`) and `low` (`unsigned long`) are eight octets in the byte order of the submessage, `change_count = low + high * 2^32`; a negative `high` is no count and is refused.

### §9.3.2.12 BuiltinEndpointSet_t: positions 0 to 5

**Spec:** §9.3.2.12, pp. 157–158 — "@position(0)
DISC_BUILTIN_ENDPOINT_PARTICIPANT_ANNOUNCER, @position(1) …_PARTICIPANT_DETECTOR,
@position(2) …_PUBLICATIONS_ANNOUNCER, @position(3) …_PUBLICATIONS_DETECTOR,
@position(4) …_SUBSCRIPTIONS_ANNOUNCER, @position(5) …_SUBSCRIPTIONS_DETECTOR".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::PARTICIPANT_ANNOUNCER`
to `SUBSCRIPTIONS_DETECTOR`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: positions 6 to 9 deprecated

**Spec:** §9.3.2.12, p. 158 — "/* Positions 6-9 were deprecated in version 2.4
*/"; §9.3.2.1: "These bits should not be used by versions of the protocol
equal to or newer than the deprecated version".

**Repo:** `crates/rtps/src/participant_data.rs` (`ALL_STANDARD` without the
bits 6 to 9)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_all_standard_excludes_secure_bits`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_standard_bits_by_default`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: positions 10 and 11

**Spec:** §9.3.2.12, p. 158 — "@position(10)
BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_DATA_WRITER, @position(11)
BUILTIN_ENDPOINT_PARTICIPANT_MESSAGE_DATA_READER".

**Repo:** `crates/rtps/src/participant_data.rs`
(`endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER`, `…_READER`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: positions 12 to 15 (XTypes)

**Spec:** §9.3.2.12, p. 158 — "Positions 12-15 are reserved by DDS-XTypes 1.2
and its revisions thereof".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::TYPE_LOOKUP_REQUEST`,
`TYPE_LOOKUP_REPLY`)

**Tests:** `crates/dcps/tests/type_lookup_e2e.rs::announces_type_lookup_endpoints_in_spdp`,
`crates/discovery/src/capabilities.rs::tests::type_lookup_capabilities_require_the_four_distinct_vendor_bits`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: positions 16 to 27 (DDS-Security)

**Spec:** §9.3.2.12, p. 158 — "Positions 16-27 are reserved by DDS-Security 1.1
and revisions thereof".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::ALL_SECURE`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_all_secure_covers_bits_16_to_27`

**Status:** `done`

### §9.3.2.12 BuiltinEndpointSet_t: positions 28 and 29

**Spec:** §9.3.2.12, p. 158 — "@position(28)
DISC_BUILTIN_ENDPOINT_TOPICS_ANNOUNCER, @position(29)
DISC_BUILTIN_ENDPOINT_TOPICS_DETECTOR".

**Repo:** `crates/rtps/src/participant_data.rs` (`endpoint_flag::TOPICS_ANNOUNCER`,
`TOPICS_DETECTOR`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/discovery/src/sedp/stack.rs::tests::the_topics_bits_wire_the_topics_writer_and_reader`

**Status:** `done`

### §9.4.1 Submessages on 32-bit boundaries

**Spec:** §9.4.1, p. 158 — "The PSM aligns each Submessage on a 32-bit boundary
with respect to the start of the Message."

**Repo:** `crates/rtps/src/message_builder.rs` (`MessageBuilder::check_aligned`)

**Tests:** `crates/rtps/src/message_builder.rs::tests::nothing_is_appended_after_an_unaligned_submessage`,
`crates/rtps/src/message_builder.rs::tests::four_small_datas_aggregate_into_one_datagram`

**Status:** `done` — the decoder does not require the alignment on reception.

### §9.4.1 Message length from the transport

**Spec:** §9.4.1, p. 158 — "A Message has a well-known length. This length is
not sent explicitly by the RTPS protocol but is part of the underlying
transport with which Messages are sent. In the case of UDP/IP, the length of
the Message is the length of the UDP payload."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: length of the
datagram passed in)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`,
`crates/transport-udp/src/udp_transport.rs::tests::loopback_send_and_recv_delivers_datagram`

**Status:** `done`

### §9.4.2 Mapping of the submessage elements

**Spec:** §9.4.2, p. 158 — "This sub clause describes the PSM mapping for each
of the SubmessageElements defined by the PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the elements follow one by
one.

### §9.4.2.1 EntityId

**Spec:** §9.4.2.1, p. 159 — "typedef EntityId_t EntityId;" wire form `octet
value[4]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`EntityId::to_bytes`,
`EntityId::from_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::entity_id_roundtrip`

**Status:** `done`

### §9.4.2.2 GuidPrefix

**Spec:** §9.4.2.2, p. 159 — "typedef GuidPrefix_t GuidPrefix;" wire form
`octet value[12]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`GuidPrefix::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::guid_prefix_roundtrip`,
`crates/rtps/src/header.rs::tests::header_guid_prefix_at_bytes_8_to_19`

**Status:** `done`

### §9.4.2.3 VendorId

**Spec:** §9.4.2.3, p. 159 — "typedef VendorId_t VendorId;" wire form `octet
vendorId[2]`.

**Repo:** `crates/rtps/src/wire_types.rs` (`VendorId::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::vendor_id_roundtrip`,
`crates/rtps/src/header.rs::tests::header_vendor_id_at_bytes_6_7`

**Status:** `done`

### §9.4.2.4 ProtocolVersion

**Spec:** §9.4.2.4, pp. 159–160 — "typedef ProtocolVersion_t ProtocolVersion;"
wire form `octet major | octet minor`.

**Repo:** `crates/rtps/src/wire_types.rs` (`ProtocolVersion::to_bytes`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_roundtrip`,
`crates/rtps/src/header.rs::tests::header_protocol_version_at_bytes_4_5`

**Status:** `done`

### §9.4.2.5 SequenceNumber

**Spec:** §9.4.2.5, p. 160 — wire form `long high`, `unsigned long low`.

**Repo:** `crates/rtps/src/wire_types.rs` (`SequenceNumber::to_bytes_le`,
`to_bytes_be`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_le`,
`crates/rtps/src/wire_types.rs::tests::sequence_number_roundtrip_be`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: structure and wire form

**Spec:** §9.4.2.6, pp. 160–161 — "struct SequenceNumberSet { SequenceNumber_t
bitmapBase; LongSeq8 bitmap; };" wire form: bitmapBase, `unsigned long
numBits`, `long bitmap[M]` with "M = (numBits+31)/32". "The numBits field
encodes both the number of significant bits and the number of bitmap
elements."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::write_to`,
`read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::snset_wire_size_zero_bits_is_12_bytes`,
`crates/rtps/src/submessages.rs::tests::snset_wire_size_33_bits_is_20_bytes`,
`crates/rtps/src/submessages.rs::tests::snset_roundtrip_be`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: membership by bit position

**Spec:** §9.4.2.6, p. 160 — "(bitmap[deltaN/ 32] & (1 << (31 - deltaN%32))) ==
(1 << (31 - deltaN%32)) where deltaN = seqNum - seqNumSet.bitmapBase"; "In the
bitmap, the bit corresponding to sequence number bitmapBase is on the left."

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet::from_missing`,
`iter_set`: bit `31 - offset % 32`)

**Tests:** `crates/rtps/tests/proptest_roundtrip.rs::seqnum_set_iter_matches_missing`,
`crates/rtps/src/submessages.rs::tests::snset_roundtrip_le`

**Status:** `done`

### §9.4.2.6 SequenceNumberSet: validity

**Spec:** §9.4.2.6, p. 160 — "A valid SequenceNumberSet must satisfy the
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

**Status:** `done` — `bitmapBase >= 1`, `numBits <= 256` and the number of bitmap
words are checked.
Vendor form: OpenDDS 3.34.0 and RTI Connext 7.7.0 send `readerSNState` `0/0:`
(bitmapBase 0, numBits 0) as the ACKNACK of a reader without samples (measured in
`tools/wire-capture/rtps-sessions`: 40 and 8 ACKNACKs). Default: invalid. The
setting `wire.accept_empty_acknack_base_zero` (env
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, default `false`) accepts exactly this
empty set in an ACKNACK, for the process.

### §9.4.2.6 Table 9.5 example 1234/12:00110

**Spec:** §9.4.2.6, p. 161, Table 9.5 — "Example of bitmap: meaning of
“1234/12:00110”".

**Repo:** `crates/rtps/src/submessages.rs` (`SequenceNumberSet`: `iter_set`, `write_to`, `read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_bitmap_1234_12_00110_of_table_9_5_names_1236_and_1237`

**Status:** `done` — bitmapBase 1234, numBits 12, bits 00110: the numbers 1236 and 1237 are set, 1234, 1235 and 1238 to 1245 are not; the bit for bitmapBase is on the left (most significant bit of the first word). The test writes and reads the example in both byte orders.

### §9.4.2.7 FragmentNumber

**Spec:** §9.4.2.7, p. 161 — "typedef FragmentNumber_t FragmentNumber;" wire
form `unsigned long value`.

**Repo:** `crates/rtps/src/wire_types.rs` (`FragmentNumber::to_bytes_le`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::fragment_number_roundtrip_le_be`

**Status:** `done`

### §9.4.2.8 FragmentNumberSet: structure and wire form

**Spec:** §9.4.2.8, pp. 161–162 — "struct FragmentNumberSet { FragmentNumber_t
bitmapBase; LongSeq8 bitmap; };" "The interpretation matches that of a
SequenceNumberSet." Wire form: bitmapBase, numBits, `long bitmap[M]`.

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::write_to`,
`read_from`)

**Tests:** `crates/rtps/src/submessages.rs::tests::fnset_wire_size_formula`,
`crates/rtps/src/submessages.rs::tests::fnset_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done`

### §9.4.2.8 FragmentNumberSet: validity

**Spec:** §9.4.2.8, p. 162 — "The interpretation matches that of a
SequenceNumberSet." (§9.4.2.6: "bitmapBase >= 1", "0 <= numBits <= 256").

**Repo:** `crates/rtps/src/submessages.rs` (`FragmentNumberSet::read_from`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::fragment_submessages_validity_rules`,
`crates/rtps/src/submessages.rs::tests::fnset_rejects_num_bits_above_256`

**Status:** `done` — `bitmapBase >= 1` and `numBits <= 256` are checked.

### §9.4.2.9 Timestamp

**Spec:** §9.4.2.9, p. 162 — "typedef Time_t Timestamp;" wire form `long
seconds`, `unsigned long fraction`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`,
`timestamp_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_be`

**Status:** `done` — `HeTimestamp::seconds` is `i32` as in the wire form;
§9.3.2.1 names `unsigned long seconds` (`open` item under §9.3.2.1 Time_t).

### §9.4.2.10 LocatorList

**Spec:** §9.4.2.10, pp. 162–163 — "typedef sequence<Locator_t, 8>
LocatorList;" wire form `unsigned long numLocators`, then the locators with
`long kind`, `unsigned long port`, `octet address[16]`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage`,
`read_locator_list`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::info_reply_empty_unicast_list_is_valid`,
`crates/rtps/src/submessages.rs::validity_tests::info_reply_locator_list_holds_at_most_8_locators`

**Status:** `done` — the wire layout and the limit of 8 locators are as specified.

### §9.4.2.10 LocatorList at most 8 locators

**Spec:** §9.4.2.10, p. 162 — "typedef sequence<Locator_t, 8> LocatorList;"

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage::read_locator_list`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::info_reply_locator_list_holds_at_most_8_locators`

**Status:** `done` — a list of more than 8 locators is refused.

### §9.4.2.11 Parameter: alignment and length

**Spec:** §9.4.2.11, p. 163 — "Each Parameter within the ParameterList starts
aligned on a 4-byte boundary with respect to the start of the ParameterList."
"The length encodes the number of octets following the length to reach the ID
of the next parameter … the length is always a multiple of four."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::to_bytes`,
`from_bytes_with_consumed`: `length % 4 != 0` is refused)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::decode_rejects_non_aligned_length`,
`crates/rtps/src/parameter_list.rs::tests::parameter_value_is_padded_to_4_bytes`

**Status:** `done`

### §9.4.2.11 Parameter: CDR reset per value

**Spec:** §9.4.2.11, p. 163 — "The value contains the CDR representation of the
Parameter type that corresponds to the specified parameterId. For alignment
purposes, the CDR stream is logically reset for each parameter value".

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/participant_data.rs` (every value is encoded from offset 0)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.4.2.11 Several parameters with the same ParameterId

**Spec:** §9.4.2.11, p. 163 — "The ParameterList may contain multiple
Parameters with the same value for the parameterId. This is used to provide a
collection of values for that kind of Parameter."

**Repo:** `crates/rtps/src/parameter_list.rs` (`ParameterList::find_all`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`,
`crates/rtps/src/parameter_list.rs::tests::roundtrip_multiple_parameters`

**Status:** `done`

### §9.4.2.11 PID_SENTINEL

**Spec:** §9.4.2.11, p. 164 — "#define PID_SENTINEL (1)"; "The PID_SENTINEL is
used to terminate the parameter list and its length is ignored."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SENTINEL`;
`from_bytes_with_consumed` ends at the sentinel without evaluating its length)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`,
`crates/rtps/src/parameter_list.rs::tests::decode_rejects_missing_sentinel`

**Status:** `done`

### §9.4.2.11 PID_PAD

**Spec:** §9.4.2.11, p. 164 — "#define PID_PAD (0)"; "The PID_PAD is used to
enforce alignment of the parameter that follows and its length can be anything
(as long as it is a multiple of 4)."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.4.2.11 Sentinel not required in the HeaderExtension

**Spec:** §9.4.2.11, p. 164 — "The presence of the PID_SENTINEL is not required
in the ParameterList that appears in the HeaderExtension. See 9.4.5.2. The
presence of the PID_SENTINEL is required in all other cases."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::decode_body`,
`ParameterList::from_bytes_sentinel_optional`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::decode_takes_a_parameter_list_without_sentinel`,
`crates/rtps/src/parameter_list.rs::tests::decode_rejects_missing_sentinel`

**Status:** `done` — the parameters of the HeaderExtension are read without PID_SENTINEL; in all other cases the sentinel is required (`decode_rejects_missing_sentinel`).

### §9.4.2.11 Table 9.6 bit 0x8000: vendor-specific

**Spec:** §9.4.2.11, p. 165, Table 9.6 — "ParameterId & 8000 — 0 — Reserved
ParameterId. — 1 — Vendor-specific ParameterId. Will not be recognized by
other vendors’ implementations."

**Repo:** `crates/rtps/src/parameter_list.rs` (`VENDOR_SPECIFIC_BIT`);
`crates/rtps/src/publication_data.rs` (vendor parameters by vendorId)

**Tests:** `crates/discovery/tests/vendor_discovery_frames.rs::rti_announcements_read_and_write_back`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`

**Status:** `done`

### §9.4.2.11 Table 9.6 bit 0x4000 = 0: skip if unknown

**Spec:** §9.4.2.11, p. 165, Table 9.6 — "ParameterId & 4000 — 0 — If the
ParameterId is not recognized, skip and ignore the parameter."

**Repo:** `crates/rtps/src/parameter_list.rs`
(`validate_must_understand_in_data_pipeline`); decoders in
`crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`

**Status:** `done`

### §9.4.2.11 Table 9.6 bit 0x4000 = 1: an error if unknown

**Spec:** §9.4.2.11, p. 165, Table 9.6 — "1 — If the ParameterId is not
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

**Status:** `done` — in the HeaderExtension the whole message is dropped; a DATA or
DATA_FRAG with an unrecognized PID with bit 0x4000 is skipped and the next
submessage is read. That holds in the vendor-specific range (bit 0x8000) too:
there only the ZeroDDS PIDs (0x8001, 0x8002) from a ZeroDDS sender are recognized.

### §9.4.2.11 Table 9.7 DDS-Security: 0x1000–0x10ff, 0x5000–0x50ff

**Spec:** §9.4.2.11, p. 165, Table 9.7 — "DDS-Security 1.1 — 0x1000-0x10ff and
0x5000-0x50ff".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IDENTITY_TOKEN`,
`PERMISSIONS_TOKEN`, `PARTICIPANT_SECURITY_INFO` and others)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`

**Status:** `done`

### §9.4.2.11 Table 9.7 DDS-XTypes: 0x0069, 0x0072–0x0075, 0x3f01–0x3fff, 0x7f01–0x7fff

**Spec:** §9.4.2.11, p. 165, Table 9.7 — "DDS-XTypes 1.2 — 0x0069;
0x0072-0x0075; 0x3f01-0x3fff; 0x7f01-0x7fff".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::IGNORE = 0x3F03`,
PID_DATA_REPRESENTATION, PID_TYPE_CONSISTENCY, PID_TYPE_INFORMATION)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_ignore_skipped_in_pl_cdr_decode`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.4.2.11 Missing PID: default; unknown PID by Table 9.6

**Spec:** §9.4.2.11, p. 165 — "If a ParameterId is expected, but not present,
the protocol will assume the default value. Similarly, if a ParameterId is
present but not recognized, the protocol will either skip and ignore the
parameter or treat the parameter as an incompatible QoS."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`, `crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::the_infinite_defaults_are_left_out_and_read_back`,
`crates/rtps/src/subscription_data.rs::tests::the_readers_infinite_defaults_are_left_out_and_read_back`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`

**Status:** `done` — the default per PID is listed under §9.6.3.2, the
must-understand behaviour under Table 9.6.

### §9.4.2.12 SerializedPayload

**Spec:** §9.4.2.12, p. 165 — "For the purpose of interoperability, all
implementations must however use a consistent representation (See, 10
Serialized Payload Representation)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a reference; the representation is listed
under §10.

### §9.4.2.13 Count

**Spec:** §9.4.2.13, p. 166 — "typedef Count_t Count;" wire form `long value`.

**Repo:** `crates/rtps/src/submessages.rs` (`count: i32`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_be`

**Status:** `done`

### §9.4.2.14 ChangeCount

**Spec:** §9.4.2.14, p. 166 — "typedef ChangeCount_t ChangeCount;" wire form
`long high`, `unsigned long low` (8 octets).

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (0,0)

**Spec:** §9.4.2.15, p. 166, Table 9.8 — "0, 0 — N/A — The messageChecksum is
not included in the HeaderExtension".

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumKind::None`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/header_extension.rs::tests::empty_he_le_roundtrip`

**Status:** `done`

### §9.4.2.15 Table 9.8 ChecksumFlags (0,1) CRC-32C

**Spec:** §9.4.2.15, p. 166, Table 9.8 — "0, 1 — Checksum32_t — … the
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

**Spec:** §9.4.2.15, pp. 166–167, Table 9.8 — "1, 0 — Checksum64_t — … the
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

**Spec:** §9.4.2.15, p. 167, Table 9.8 — "1,1 — Checksum128_t — The
messageChecksum is a 128-bit checksum. It shall hold the MD5 digest of the
RTPS message."

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::message_checksum`, `seal_message`,
`verify_message`, `encode_body`, `decode_body`); `crates/rtps/src/datagram.rs`
(`decode_submessage`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_md5_matches_rfc1321_vector`,
`crates/rtps/src/header_extension.rs::tests::a_sealed_message_verifies_and_a_changed_one_does_not`

**Status:** `done`

### §9.4.2.15 Checksum: byte order, range, verification

**Spec:** §9.4.2.15, p. 167 — "the checksum shall be computed over the content
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

**Status:** `done` — CRC-32C and CRC-64 are in big endian, the checksum is over the whole message with the field zeroed, and the receiver checks it.

### §9.4.2.15 Table 9.9 parameters of CRC-32C

**Spec:** §9.4.2.15, p. 168, Table 9.9 — width 32 bits, polynomial 0x1EDC6F41,
input and result reflected, initial value and final XOR 0xFFFFFFFF.

**Repo:** `crates/foundation` (`crc32c`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc32c_matches_rfc4960_vector`

**Status:** `done`

### §9.4.2.15 Table 9.10 examples CRC-32C

**Spec:** §9.4.2.15, p. 168, Table 9.10 — "Example Checksum32_t computation":
`00 00 00 00` → 0x48674BC7, `FF FF FF FF` → 0xFFFFFFFF,
`33 22 55 AA BB CC DD EE FF` → 0xB59CA09B.

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::compute`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`

**Status:** `done` — the three examples of the table.

### §9.4.2.15 Table 9.11 parameters of CRC-64/XZ

**Spec:** §9.4.2.15, p. 168, Table 9.11 — width 64 bits, polynomial
0x42F0E1EBA9EA3693, input and result reflected, initial value and final XOR
0xFFFFFFFFFFFFFFFF.

**Repo:** `crates/foundation` (`crc64_xz`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_compute_crc64_xz_matches_known_vector`

**Status:** `done`

### §9.4.2.15 Table 9.12 examples CRC-64/XZ

**Spec:** §9.4.2.15, p. 169, Table 9.12 — "Example Checksum64_t computation":
`00 00 00 00` → 0xF4A586351E1B9F4B, `FF FF FF FF` → 0xFFFFFFFF00000000,
`33 22 55 AA BB CC DD EE FF` → 0x701ECEB219A8E5D5.

**Repo:** `crates/rtps/src/header_extension.rs` (`ChecksumValue::compute`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::the_checksums_of_tables_9_10_and_9_12`

**Status:** `done` — the three examples of the table.

### §9.4.2.16 MessageLength

**Spec:** §9.4.2.16, p. 169 — "typedef MessageLength_t MessageLength;" wire
form `unsigned long length`.

**Repo:** `crates/rtps/src/header_extension.rs` (`message_length`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::wire_layout_le_message_length_correct`

**Status:** `done`

### §9.4.2.17 UExtension4

**Spec:** §9.4.2.17, p. 169 — "typedef UExtension4_t UExtension4;" wire form
`octet value[4]`.

**Repo:** `crates/rtps/src/header_extension.rs` (`uextension4`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`

**Status:** `done`

### §9.4.2.18 WExtension8

**Spec:** §9.4.2.18, p. 169 — "typedef WExtension8_t WExtension8;" wire form
`octet value[8]`.

**Repo:** `crates/rtps/src/header_extension.rs` (`wextension8`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`

**Status:** `done`

### §9.4.2.19 GroupDigest

**Spec:** §9.4.2.19, pp. 169–170 — "typedef GroupDigest_t GroupDigest;" wire
form `octet value[4]`.

**Repo:** `crates/rtps/src/group_digest.rs` (`GroupDigest`,
`GroupDigest::of_entities`)

**Tests:** `crates/rtps/src/group_digest.rs::tests::two_writers_digest_as_section_9_3_2_5_computes_it`,
`crates/rtps/src/group_digest.rs::tests::the_digest_does_not_depend_on_the_order_given`,
`crates/rtps/src/group_digest.rs::tests::the_empty_group_is_zero_and_not_the_hash_of_an_empty_sequence`

**Status:** `done`

### §9.4.3.1 LocatorUDPv4

**Spec:** §9.4.3.1, p. 170, Table 9.13 — "The LocatorUDPv4 SubmessageElement
is identical to a LocatorList SubmessageElement containing a single locator of
kind LOCATOR_KIND_UDPv4." Wire form `unsigned long address`, `unsigned long
port`.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage::write_locator_udpv4`, `read_locator_udpv4`)

**Tests:** `crates/rtps/src/submessages.rs::tests::locator_udpv4_is_two_unsigned_longs_in_the_byte_order_of_the_submessage`,
`crates/rtps/src/submessages.rs::tests::info_reply_ip4_with_multicast_roundtrip_be`

**Status:** `done` — `address` and `port` are each an `unsigned long` in the byte order of the submessage; 192.0.2.10 is the number 0xC000020A (first octet most significant). Before, the address was four octets in network order in every byte order, which differs in the little-endian case. 2.5 does not define `LocatorUDPv4_t`; the mapping of a.b.c.d to the number is an assumption. No vendor is measured.

### §9.4.4 Header

**Spec:** §9.4.4, p. 170 — wire form `'R' 'T' 'P' 'S'`, `ProtocolVersion
version`, `VendorId vendorId`, `GuidPrefix guidPrefix`. "The structure of the
Header cannot change in this major version (2) of the protocol."

**Repo:** `crates/rtps/src/header.rs` (`RtpsHeader::to_bytes`, `from_bytes`)

**Tests:** `crates/rtps/src/header.rs::tests::header_roundtrip`,
`crates/rtps/src/header.rs::tests::header_total_size_is_20_bytes`

**Status:** `done`

### §9.4.4 Use an assigned VendorId

**Spec:** §9.4.4, p. 170 — "To be compliant with the DDS Interoperability
Specification a vendor must have a reserved Vendor ID and use it."

**Repo:** —

**Tests:** —

**Status:** `open` — `VendorId::ZERODDS = [0x01, 0xF0]`
(`crates/rtps/src/wire_types.rs`), by its comment an interim value; as under §8.3.5.2. Blocker: a hard external one, the assignment of a VendorId by the OMG; the request has been pending since 22 June. Until then ZeroDDS sends `0x01F0`, a value the OMG list does not hold (assigned up to `{0x01,0x1C}`, `{0x01,0x1D}` and `{0x01,0x1E}` reserved).

### §9.4.5.1 SubmessageHeader

**Spec:** §9.4.5.1, p. 171 — "struct SubmessageHeader { octet submessageId;
octet flags; unsigned short submessageLength; /* octetsToNextHeader */ };"
"This general structure cannot change in this major version (2) of the
protocol."

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageHeader::to_bytes`,
`from_bytes`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_le`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_layout_be`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind RTPS_HE = 0x00

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x00) RTPS_HE, /* HeaderExtension */".

**Repo:** `crates/rtps/src/header_extension.rs` (`SUBMESSAGE_ID_HEADER_EXTENSION = 0x00`), `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/tests/cyclone_he_must_understand.rs::local_encoder_produces_must_understand_bit`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind PAD = 0x01

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x01) PAD, /* Pad */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Pad`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind ACKNACK = 0x06

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x06) ACKNACK /* AckNack */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::AckNack`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_acknack_is_0x06`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind HEARTBEAT = 0x07

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x07) HEARTBEAT /* Heartbeat */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Heartbeat`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_heartbeat_is_0x07`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind GAP = 0x08

**Spec:** §9.4.5.1.1, p. 171 — "@value(8x08) GAP /* Gap */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Gap`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_gap_is_0x08`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_TS = 0x09

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x09) INFO_TS /* InfoTimestamp */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoTs`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_SRC = 0x0c

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x0c) INFO_SRC /* InfoSource */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoSrc`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_REPLY_IP4 = 0x0d

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x0d) INFO_REPLY_IP4 /* InfoReplyIp4 */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoReplyIp4`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_DST = 0x0e

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x0e) INFO_DST /* InfoDestination */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoDst`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind INFO_REPLY = 0x0f

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x0f) INFO_REPLY /* InfoReply */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::InfoReply`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind NACK_FRAG = 0x12

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x12) NACK_FRAG /* NackFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::NackFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind HEARTBEAT_FRAG = 0x13

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x13) HEARTBEAT_FRAG /* HeartbeatFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::HeartbeatFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind DATA = 0x15

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x15) DATA /* Data */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::Data`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_data_is_0x15`

**Status:** `done`

### §9.4.5.1.1 SubmessageKind DATA_FRAG = 0x16

**Spec:** §9.4.5.1.1, p. 171 — "@value(0x16) DATA_FRAG /* DataFrag */".

**Repo:** `crates/rtps/src/submessage_header.rs` (`SubmessageId::DataFrag`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_id_octets_match_spec_table_8_13`

**Status:** `done`

### §9.4.5.1.1 The meaning of the SubmessageIds is fixed

**Spec:** §9.4.5.1.1, p. 171 — "The meaning of the Submessage IDs cannot be
modified in this major version (2). Additional Submessages can be added in
higher minor versions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a requirement on future editions of the
specification.

### §9.4.5.1.1 Ids 0x80–0xff by vendorId

**Spec:** §9.4.5.1.1, p. 171 — "Submessages with ID's 0x80 to 0xff (inclusive)
are vendor-specific; they will not be defined by future versions of the
protocol. Their interpretation is dependent on the vendorId that is current
when the Submessage is encountered."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`, `ParsedSubmessage::Unknown`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_skips_an_unknown_submessage_whatever_its_flags`

**Status:** `done` — ZeroDDS defines no submessage of its own; every id
0x80–0xff is passed over.

### §9.4.5.1.1.1 Table 9.14 DDS-Security 0x30–0x3f

**Spec:** §9.4.5.1.1.1, p. 172, Table 9.14 — "DDS-Security 1.1 — 0x30-0x3f".

**Repo:** `crates/security-rtps/src/codec.rs` (`SEC_PREFIX = 0x31`, `SEC_BODY`,
`SEC_POSTFIX`, `SRTPS_PREFIX`, `SRTPS_POSTFIX`)

**Tests:** `crates/security-rtps/src/codec.rs::tests::constants_match_spec`,
`crates/security-rtps/src/codec.rs::tests::three_submessages_carry_spec_ids_flags_and_lengths_in_order`

**Status:** `done`

### §9.4.5.1.2 EndiannessFlag in the least significant bit

**Spec:** §9.4.5.1.2, p. 172 — "The PSM maps the EndiannessFlag flag into the
least-significant bit (LSB) of the flags. … E=0 means big-endian, E=1 means
little-endian. … E = SubmessageHeader.flags & 0x01".

**Repo:** `crates/rtps/src/submessage_header.rs` (`FLAG_E_LITTLE_ENDIAN = 0x01`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_is_little_endian_flag`,
`crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_be`

**Status:** `done`

### §9.4.5.1.2 X flags: send zero, ignore on reception

**Spec:** §9.4.5.1.2, p. 172 — "Implementations of RTPS version 2.5 should set
these to zero when sending and ignore these when receiving."

**Repo:** `crates/rtps/src/submessages.rs` (the encoders set only the
defined flags; the decoders read only these); `crates/rtps/src/datagram.rs`

**Tests:** `crates/rtps/src/datagram.rs::tests::x_flags_are_zero_when_sending_and_ignored_when_receiving`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`,
`crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done` — the encoders of HEARTBEAT, ACKNACK, GAP, HEARTBEAT_FRAG, NACK_FRAG, INFO_TS, INFO_SRC, INFO_REPLY, INFO_REPLY_IP4 and DATA set only E and the flags defined for the submessage; the decoders give the same value with all other flag bits set. For unknown submessages §8.3.4.1 rule 3 applies.

### §9.4.5.1.3 octetsToNextHeader as a ushort

**Spec:** §9.4.5.1.3, p. 172 — "The representation of this field is a CDR
unsigned short (ushort)."

**Repo:** `crates/rtps/src/submessage_header.rs` (`octets_to_next_header: u16`)

**Tests:** `crates/rtps/src/submessage_header.rs::tests::submessage_header_roundtrip_le`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader > 0

**Spec:** §9.4.5.1.3, p. 172 — "In case octetsToNextHeader > 0, it is the number
of octets from the first octet of the contents of the Submessage until the
first octet of the header of the next Submessage … OR it is the number of
octets remaining in the Message".

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: `body_end`)

**Tests:** `crates/rtps/src/datagram.rs::tests::encode_decode_two_data_submessages`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_the_data_behind_it_still_decodes`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader == 0 except PAD and INFO_TS

**Spec:** §9.4.5.1.3, p. 172 — "In case octetsToNextHeader==0 and the kind of
Submessage is NOT PAD or INFO_TS, the Submessage is the last Submessage in the
Message and extends up to the end of the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_handles_last_submessage_zero_length`

**Status:** `done`

### §9.4.5.1.3 octetsToNextHeader == 0 for PAD and INFO_TS

**Spec:** §9.4.5.1.3, p. 172 — "In case the octetsToNextHeader==0 and the kind of
Submessage is PAD or INFO_TS, the next Submessage header starts immediately
after the current Submessage header OR the PAD or INFO_TS is the last
Submessage in the Message."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`)

**Tests:** `crates/rtps/src/datagram.rs::tests::an_info_ts_without_a_body_is_not_the_last_submessage`,
`crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

### §9.4.5.2 HeaderExtension: wire form

**Spec:** §9.4.5.2, p. 173 — `RTPS_HE |P|C|C|W|U|T|L|E|`, then messageLength
[L], rtpsSendTimestamp [T], uExtension4 [U], wExtension8 [W], messageChecksum
[CC != 00], parameters [P].

**Repo:** `crates/rtps/src/header_extension.rs` (`HeaderExtension::encode_body`,
`decode_body`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_le`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_all_fields_set_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension LengthFlag (0x02)

**Spec:** §9.4.5.2, p. 173 — "L = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_L`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::flag_byte_with_message_length`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension TimestampFlag (0x04)

**Spec:** §9.4.5.2, p. 173 — "The TimestampFlag is represented with the literal
‘T’." (expression in the text: "T = SubmessageHeader.flags & 0x04").

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_W`, bit 2)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_timestamp_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension UExtension4Flag (0x08)

**Spec:** §9.4.5.2, p. 173 — "U = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_U`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_uextension4_be`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension WExtension8Flag (0x10)

**Spec:** §9.4.5.2, p. 173 — "W = SubmessageHeader.flags & 0x10".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_V`, bit 4)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_wextension8_le`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension ChecksumFlags C1 (0x40), C2 (0x20)

**Spec:** §9.4.5.2, p. 173 — "C1 = SubmessageHeader.flags & 0x40; C2 =
SubmessageHeader.flags & 0x20".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_C_MASK`,
`ChecksumKind::from_flags`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::checksum_kind_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.2 HeaderExtension ParametersFlag (0x80)

**Spec:** §9.4.5.2, p. 174 — "P = SubmessageHeader.flags & 0x80".

**Repo:** `crates/rtps/src/header_extension.rs` (`HE_FLAG_P`)

**Tests:** `crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`

**Status:** `done`

### §9.4.5.3 AckNack: wire form

**Spec:** §9.4.5.3, p. 174 — `ACKNACK |X|X|X|X|X|X|F|E|`, readerId, writerId,
SequenceNumberSet readerSNState, Count count.

**Repo:** `crates/rtps/src/submessages.rs` (`AckNackSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::acknack_submessage_roundtrip_le`

**Status:** `done`

### §9.4.5.3 AckNack FinalFlag (0x02)

**Spec:** §9.4.5.3, p. 174 — "The PSM maps the FinalFlag flag into the 2nd
least-significant bit (LSB) of the flags. … F = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`ACKNACK_FLAG_FINAL = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_acknack_preserves_final_flag`,
`crates/rtps/src/submessages.rs::tests::acknack_submessage_with_final_flag`

**Status:** `done`

### §9.4.5.3 AckNack F = 0: the writer answers with a Heartbeat

**Spec:** §9.4.5.3, p. 174 — "F=1 means the reader does not require a Heartbeat
from the writer. F=0 means the writer must respond to the AckNack message with
a Heartbeat message."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`note_acknack_final_flag`,
`tick`); `crates/rtps/src/reader_proxy.rs` (`heartbeat_asked`,
`heartbeat_is_asked`); calls in `crates/dcps/src/runtime.rs`,
`crates/discovery/src/sedp/stack.rs`, `crates/dcps/src/type_lookup_transport.rs`

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::an_acknack_with_the_final_flag_clear_is_answered_by_a_heartbeat`

**Status:** `done` — an ACKNACK with F = 0 is answered by a HEARTBEAT, at once
when the reader had none yet, otherwise a period after the last one at the
earliest; user, SEDP and TypeLookup writers.

### §9.4.5.4 Data: wire form

**Spec:** §9.4.5.4, p. 174 — `DATA |X|X|X|N|K|D|Q|E|`, extraFlags,
octetsToInlineQos, readerId, writerId, writerSN, inlineQos [Q], serializedPayload
[D || K].

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::write_body`,
`read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_roundtrip_bit_identical`

**Status:** `done`

### §9.4.5.4 Data InlineQosFlag (0x02)

**Spec:** §9.4.5.4, p. 175 — "Q = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_INLINE_QOS = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`,
`crates/rtps/tests/vendor_frames.rs::cyclone_session_re_encodes_byte_for_byte`

**Status:** `done`

### §9.4.5.4 Data DataFlag (0x04)

**Spec:** §9.4.5.4, p. 175 — "D = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_DATA = 0x04`)

**Tests:** `crates/rtps/tests/cyclone_compliance.rs::cyclone_data_cdr2_payload_decodes_and_carries_payload`,
`crates/rtps/src/submessages.rs::tests::data_submessage_all_valid_flags_combined_roundtrip`

**Status:** `done`

### §9.4.5.4 Data KeyFlag (0x08)

**Spec:** §9.4.5.4, p. 175 — "K = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_KEY = 0x08`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_key_flag_roundtrip`,
`crates/discovery/src/sedp/reader.rs::tests::handle_data_reports_a_cyclone_dispose_without_key_hash_as_removed`

**Status:** `done`

### §9.4.5.4 Data NonStandardPayloadFlag (0x10)

**Spec:** §9.4.5.4, p. 175 — "N = SubmessageHeader.flags & 0x10".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FLAG_NON_STANDARD = 0x10`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_non_standard_flag_roundtrip`

**Status:** `done`

### §9.4.5.4 Data: combinations of D and K

**Spec:** §9.4.5.4, p. 175 — "D=0 and K=0 means that there is no
serializedPayload SubmessageElement. D=1 and K=0 means that the
serializedPayload SubmessageElement contains the serialized Data. D=0 and K=1
means that the serializedPayload SubmessageElement contains the serialized
Key."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage`);
`crates/rtps/src/reliable_reader.rs` (`handle_data_into`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_submessage_roundtrip_be_with_empty_payload`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §9.4.5.4 Data: D = 1 and K = 1 invalid

**Spec:** §9.4.5.4, p. 175 — "D=1 and K=1 is an invalid combination in this
version of the protocol."

**Repo:** `crates/rtps/src/submessages.rs` (`DataSubmessage::read_body_into`)

**Tests:** `crates/rtps/src/submessages.rs::validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`

**Status:** `done` — a DATA with D = 1 and K = 1 is refused, with the rest of the message.

### §9.4.5.4 Data extraFlags zero

**Spec:** §9.4.5.4, p. 175 — "This version of the protocol should set all the
bits in the extraFlags to zero."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`extra_flags: 0`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_decode_accepts_nonzero_extra_flags_silently`,
`crates/rtps/tests/cyclone_compliance.rs::zerodds_writer_produces_compatible_data_layout`

**Status:** `done`

### §9.4.5.4 Data octetsToInlineQos

**Spec:** §9.4.5.4, p. 175 — "Implementations of the protocol that are
processing a received submessage should always use the octetsToInlineQos to
skip any submessage header elements it does not expect or understand and
continue to process the inlineQos SubmessageElement".

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::octets_to_inline_qos_skips_header_elements_of_a_later_version`,
`crates/rtps/src/submessages.rs::tests::data_submessage_octets_to_inline_qos_is_16`

**Status:** `done`

### §9.4.5.5 DataFrag: wire form

**Spec:** §9.4.5.5, p. 176 — `DATA_FRAG |X|X|X|X|N|K|Q|E|`, extraFlags,
octetsToInlineQos, readerId, writerId, writerSN, fragmentStartingNum,
`ushort fragmentsInSubmessage`, `ushort fragmentSize`, `unsigned long
sampleSize`, inlineQos [Q], serializedPayload.

**Repo:** `crates/rtps/src/submessages.rs` (`DataFragSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::data_frag_roundtrip_be`

**Status:** `done`

### §9.4.5.5 DataFrag InlineQosFlag (0x02)

**Spec:** §9.4.5.5, p. 176 — "Q = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_INLINE_QOS = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §9.4.5.5 DataFrag KeyFlag (0x04)

**Spec:** §9.4.5.5, p. 176 — "K = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_KEY = 0x04`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`

**Status:** `done`

### §9.4.5.5 DataFrag NonStandardPayloadFlag (0x08)

**Spec:** §9.4.5.5, p. 176 — "N = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`DATA_FRAG_FLAG_NON_STANDARD = 0x08`)

**Tests:** `crates/rtps/src/datagram.rs::tests::data_frag_spec_and_cyclone_flag_oracle`,
`crates/rtps/src/submessages.rs::tests::data_frag_flags_survive_roundtrip`

**Status:** `done`

### §9.4.5.6 Gap: wire form

**Spec:** §9.4.5.6, p. 177 — `GAP |X|X|X|X|X|F|G|E|`, readerId, writerId,
gapStart, gapList, gapStartGSN [G], gapEndGSN [G], ChangeCount filteredCount
[F].

**Repo:** `crates/rtps/src/submessages.rs` (`GapSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_filtered_count_is_eight_octets_high_then_low`,
`crates/rtps/src/submessages.rs::tests::gap_with_filtered_count_roundtrip_le`

**Status:** `done`

### §9.4.5.6 Gap GroupInfoFlag (0x02)

**Spec:** §9.4.5.6, p. 177 — "G = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_GROUP_INFO = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.6 Gap FilteredCountFlag (0x04)

**Spec:** §9.4.5.6, p. 177 — "F = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`GAP_FLAG_FILTERED_COUNT = 0x04`)

**Tests:** `crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.6 Gap gapStartGSN and gapEndGSN

**Spec:** §9.4.5.6, p. 177 — "G=1 means the Gap also includes a gapStartGSN and
a gapEndGSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::gap_with_group_info_roundtrip_be`,
`crates/rtps/src/submessages.rs::tests::gap_decode_rejects_invalid_group_sequence_numbers`,
`crates/rtps/src/submessages.rs::tests::a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid`,
`crates/rtps/src/submessages.rs::tests::the_gap_flags_are_those_of_9_4_5_6`

**Status:** `done`

### §9.4.5.7 Heartbeat: wire form

**Spec:** §9.4.5.7, p. 178 — `HEARTBEAT |X|X|X|X|G|L|F|E|`, readerId, writerId,
firstSN, lastSN, Count count, currentGSN/firstGSN/lastGSN [G].

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatSubmessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_roundtrip_le`,
`crates/rtps/tests/cyclone_compliance.rs::cyclone_heartbeat_decodes`

**Status:** `done` — the part after `count` is listed under "Heartbeat: group
fields".

### §9.4.5.7 Heartbeat FinalFlag (0x02)

**Spec:** §9.4.5.7, p. 178 — "F = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_FINAL = 0x02`)

**Tests:** `crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §9.4.5.7 Heartbeat LivelinessFlag (0x04)

**Spec:** §9.4.5.7, p. 178 — "L = SubmessageHeader.flags & 0x04".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_LIVELINESS = 0x04`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_preserves_final_and_liveliness_flags`

**Status:** `done`

### §9.4.5.7 Heartbeat GroupInfoFlag (0x08)

**Spec:** §9.4.5.7, p. 178 — "G = SubmessageHeader.flags & 0x08".

**Repo:** `crates/rtps/src/submessages.rs` (`HEARTBEAT_FLAG_GROUP_INFO = 0x08`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`

**Status:** `done`

### §9.4.5.7 Heartbeat: group fields

**Spec:** §9.4.5.7, p. 178 — "G=1 means the HeartBeat includes the currentGSN,
firstGSN, and lastGSN."

**Repo:** —

**Tests:** `crates/rtps/src/submessages.rs::tests::the_group_info_is_the_three_sequence_numbers_of_9_4_5_7`,
`crates/rtps/src/submessages.rs::tests::heartbeat_with_group_info_roundtrip_le`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_heartbeat_carries_the_groups_numbers`

**Status:** `done`

### §9.4.5.8 HeartBeatFrag: wire form

**Spec:** §9.4.5.8, p. 179 — `HEARTBEAT_FRAG |X|X|X|X|X|X|X|E|`, readerId,
writerId, writerSN, lastFragmentNum, count.

**Repo:** `crates/rtps/src/submessages.rs` (`HeartbeatFragSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::heartbeat_frag_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_heartbeat_frag_roundtrip`

**Status:** `done`

### §9.4.5.9 InfoDestination: wire form

**Spec:** §9.4.5.9, p. 179 — `INFO_DST |X|X|X|X|X|X|X|E|`, GuidPrefix
guidPrefix.

**Repo:** `crates/rtps/src/datagram.rs` (`ParsedSubmessage::InfoDestination`)

**Tests:** `crates/rtps/src/datagram.rs::tests::info_dst_body_of_exactly_twelve_decodes`,
`crates/rtps/src/datagram.rs::tests::info_dst_decodes_to_typed_destination_prefix`

**Status:** `done`

### §9.4.5.10 InfoReply: wire form

**Spec:** §9.4.5.10, p. 179 — `INFO_REPLY |X|X|X|X|X|X|M|E|`, LocatorList
unicastLocatorList, LocatorList multicastLocatorList [M].

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplySubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_unicast_only_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_be`

**Status:** `done`

### §9.4.5.10 InfoReply MulticastFlag (0x02)

**Spec:** §9.4.5.10, p. 180 — "M = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_REPLY_FLAG_MULTICAST = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_with_multicast_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_info_reply_with_multicast_via_datagram`

**Status:** `done`

### §9.4.5.11 InfoSource: wire form

**Spec:** §9.4.5.11, p. 180 — `INFO_SRC |X|X|X|X|X|X|X|E|`, `long unused`,
ProtocolVersion version, VendorId vendorId, GuidPrefix guidPrefix.

**Repo:** `crates/rtps/src/submessages.rs` (`InfoSourceSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_source_wire_size_is_20`,
`crates/rtps/src/submessages.rs::tests::info_source_unused_field_roundtrips`

**Status:** `done`

### §9.4.5.12 InfoTimestamp: wire form

**Spec:** §9.4.5.12, p. 180 — `INFO_TS |X|X|X|X|X|X|I|E|`, Timestamp timestamp
[only if I==0].

**Repo:** `crates/rtps/src/submessages.rs` (`InfoTimestampSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_roundtrip_le`,
`crates/rtps/src/submessages.rs::tests::info_timestamp_decode_with_invalidate_ignores_body`

**Status:** `done`

### §9.4.5.12 InfoTimestamp InvalidateFlag (0x02)

**Spec:** §9.4.5.12, p. 180 — "I = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`INFO_TIMESTAMP_FLAG_INVALIDATE = 0x02`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_timestamp_invalidate_flag_yields_empty_body`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_timestamp_invalidate_clears_have_timestamp`

**Status:** `done`

### §9.4.5.13 Pad: wire form

**Spec:** §9.4.5.13, pp. 180–181 — `PAD |X|X|X|X|X|X|X|E| octetsToNextHeader`.

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`: PAD is skipped)

**Tests:** `crates/rtps/src/datagram.rs::tests::pad_submessage_is_skipped_and_re_encoded_byte_for_byte`

**Status:** `done`

### §9.4.5.14 NackFrag: wire form

**Spec:** §9.4.5.14, p. 181 — `NACK_FRAG |X|X|X|X|X|X|X|E|`, readerId,
writerId, writerSN, FragmentNumberSet fragmentNumberState, count.

**Repo:** `crates/rtps/src/submessages.rs` (`NackFragSubmessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::nack_frag_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::decode_nack_frag_roundtrip`

**Status:** `done`

### §9.4.5.15 InfoReplyIp4: wire form

**Spec:** §9.4.5.15, p. 181 — `INFO_REPLY_IP4 |X|X|X|X|X|X|M|E|`, LocatorUDPv4
unicastLocator, LocatorUDPv4 multicastLocator [M]. "Its use and
interpretation are identical to those of an InfoReply Submessage containing a
single unicast and possibly a single multicast locator".

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage`)

**Tests:** `crates/rtps/src/submessages.rs::tests::locator_udpv4_is_two_unsigned_longs_in_the_byte_order_of_the_submessage`,
`crates/rtps/src/submessages.rs::tests::info_reply_ip4_unicast_only_roundtrip_le`,
`crates/rtps/src/datagram.rs::tests::receiver_walk_info_reply_ip4_sets_reply_locators`

**Status:** `done` — structure (unicast LocatorUDPv4, with the M flag a multicast LocatorUDPv4), flags (E, M) and effect are right; the address follows `LocatorUDPv4` (item under §9.4.3.1).

### §9.4.5.15 InfoReplyIp4 MulticastFlag (0x02)

**Spec:** §9.4.5.15, p. 182 — "M = SubmessageHeader.flags & 0x02".

**Repo:** `crates/rtps/src/submessages.rs` (`InfoReplyIp4Submessage::write_body`,
`read_body`)

**Tests:** `crates/rtps/src/submessages.rs::tests::info_reply_ip4_with_multicast_roundtrip_be`

**Status:** `done`

### §9.5 One message per UDP datagram

**Spec:** §9.5, p. 182 — "When RTPS is used over UDP/IP, a Message is the
contents (payload) of exactly one UDP/IP Datagram."

**Repo:** `crates/transport-udp/src/udp_transport.rs` (`UdpTransport`)

**Tests:** `crates/transport-udp/tests/e2e.rs::e2e_write_udp_read_single_datagram`,
`crates/transport-udp/src/udp_transport.rs::tests::without_the_cache_a_largest_datagram_arrives`

**Status:** `done`

### §9.6.1 ParameterIds in the HeaderExtension

**Spec:** §9.6.1, p. 182 — "This version of the protocol does not specify any
ParameterId that may appear in the HeaderExtension. … Compliant implementations
encountering an unrecognized ParameterId within the HeaderExtension shall
either skip and the parameter or reject the entire RTPS Message, as specified
in Table 9.6."

**Repo:** `crates/rtps/src/datagram.rs` (`decode_datagram`:
`validate_must_understand_in_data_pipeline` on the parameters of the
HeaderExtension)

**Tests:** `crates/rtps/tests/cyclone_he_must_understand.rs::the_decoder_refuses_a_header_extension_with_an_unknown_must_understand_pid`,
`crates/rtps/src/header_extension.rs::tests::roundtrip_with_parameter_list`,
`crates/rtps/src/datagram.rs::tests::decode_data_after_header_extension`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`

**Status:** `done` — an unrecognized PID with bit 0x4000 in the HeaderExtension drops the
message, a vendor-specific one included (see Table 9.6); one without the bit is
skipped.

### §9.6.2.1 Table 9.15 SPDP_WELL_KNOWN_MULTICAST_PORT

**Spec:** §9.6.2.1, p. 182, Table 9.15 — "Multicast —
SPDP_WELL_KNOWN_MULTICAST_PORT — PB + DG * domainId + d0".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_multicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_four_ports_follow_the_spec_formula`,
`crates/rtps/src/wire_types.rs::tests::spdp_multicast_port_domain_0_is_7400`

**Status:** `done`

### §9.6.2.1 Table 9.15 SPDP_WELL_KNOWN_UNICAST_PORT

**Spec:** §9.6.2.1, p. 182, Table 9.15 — "Unicast — SPDP_WELL_KNOWN_UNICAST_PORT —
PB + DG * domainId + d1 + PG * participantId".

**Repo:** `crates/rtps/src/wire_types.rs` (`spdp_unicast_port`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::domain_zero_ports_match_the_worked_spec_example`,
`crates/dcps/src/runtime.rs::tests::spdp_unicast_port_follows_rtps_formula`

**Status:** `done`

### §9.6.2.1 A unique participantId per node and domain

**Spec:** §9.6.2.1, p. 182 — "Each Participant on the same node and in the same
domain must use a unique participantId." "participantId values ideally start
at 0 and are incremented for each additional Participant on the same node and
in the same domain."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::start`: participantId 0
to 119, the first free SPDP unicast port; otherwise a free port)

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`,
`crates/dcps/src/runtime.rs::tests::expand_initial_peer_ip_only_yields_well_known_port_range`

**Status:** `done` — if none of the 120 ports is free, ZeroDDS binds any port
and is reachable only through multicast.

### §9.6.2.1 SEDP on the ports of SPDP

**Spec:** §9.6.2.1, p. 182 — "The default ports used by the SEDP built-in
Endpoints match those used by the SPDP. If a node chooses not to use the
default ports for the SEDP, it can include the new port numbers as part of the
information exchanged during the SPDP."

**Repo:** `crates/dcps/src/runtime.rs` (metatraffic locators = the SPDP
unicast socket and the SPDP multicast group)

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_discover_each_other_via_spdp`

**Status:** `done`

### §9.6.2.2 Table 9.16 user traffic: multicast port

**Spec:** §9.6.2.2, p. 183, Table 9.16 — "Multicast — PB + DG * domainId + d2".
"User-defined Endpoints may choose to not use the default ports."

**Repo:** `crates/rtps/src/wire_types.rs` (`user_traffic_multicast_port`); `crates/dcps/src/runtime.rs` (`transport.user_multicast`)

**Tests:** `crates/rtps/src/wire_types.rs::tests::all_four_ports_follow_the_spec_formula`,
`crates/dcps/src/runtime.rs::tests::the_user_multicast_port_is_d2_and_announced_unless_turned_off`

**Status:** `done` — ZeroDDS endpoints announce the multicast locator on `PB + DG * domainId + d2` (default; `transport.user_multicast: false` for a host without multicast). Expected: Cyclone DDS (announces 239.255.0.1 on d2), Fast DDS and OpenDDS accept an announced multicast locator or ignore it; ZeroDDS writers themselves send to unicast locators only.

### §9.6.2.2 Table 9.16 user traffic: unicast port

**Spec:** §9.6.2.2, p. 183, Table 9.16 — "Unicast — PB + DG * domainId + d3 + PG
* participantId". "In that case, remote Endpoints obtain the port number as
part of the information exchanged during the Simple Endpoint Discovery
Protocol."

**Repo:** `crates/rtps/src/wire_types.rs` (`user_traffic_unicast_port`);
`crates/dcps/src/runtime.rs` (user socket on a freely chosen port, announced as
the default unicast locator)

**Tests:** `crates/rtps/src/wire_types.rs::tests::the_four_ports_of_one_participant_are_distinct`,
`crates/dcps/src/runtime.rs::tests::announce_locators_automatic_is_non_empty_and_includes_probe`

**Status:** `done` — ZeroDDS does not use the default port but announces the
port bound, as the subclause permits.

### §9.6.2.3 Default values PB, DG, PG, d0–d3

**Spec:** §9.6.2.3, p. 183 — "PB = 7400 DG = 250 PG = 2 d0 = 0 d1 = 10 d2 = 1 d3
= 11".

**Repo:** `crates/rtps/src/wire_types.rs` (`SPDP_PORT_BASE`,
`SPDP_DOMAIN_GAIN`, `SPDP_PARTICIPANT_GAIN` and the offsets)

**Tests:** `crates/rtps/src/wire_types.rs::tests::port_formula_constants_match_spec`

**Status:** `done`

### §9.6.2.3 Parameters customisable by the user

**Spec:** §9.6.2.3, p. 183 — "Implementations must expose these parameters so
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

**Status:** `done` — PB, DG, PG and d0 to d3 are settings (file and environment, 0..=65535, default the spec's values). The runtime binds the discovery ports (d0, d1) and the user unicast socket to `PB + DG * domainId + d3 + PG * participantId`, with the participantId of the SPDP unicast socket; if the port is taken the socket takes an ephemeral port, which is counted (`user_port_fallbacks`) and logged as the event `transport.user_unicast.port_taken`. The user multicast port d2 is bound with `transport.user_multicast` (join of the SPDP group, reception, announcement as the default multicast locator); default on (the default locators of §9.6.2.3 include d2), `false` for a host without multicast. A peer otherwise reaches it at its unicast locators. Measured in `tools/wire-capture/rtps-sessions` (locators of the SPDP announcements): Fast DDS 2.6.10 and RTI 7.7.0 bind d1/d3 + PG * participantId (Fast DDS 23160/23161, 23162/23163; RTI 22910/22911, 22912/22913); Cyclone 11.0.1 takes ephemeral ports for metatraffic and user data (the same port) and announces default multicast 239.255.0.1 on d2; OpenDDS 3.34.0 takes an ephemeral metatraffic port and announces 127.0.0.1:12345 as user locator (no d3 port). Not measured: a run against the vendors with the port change, and the behaviour of ZeroDDS writers towards announced multicast locators (they send to unicast locators only).

### §9.6.2.4 Default multicast address 239.255.0.1

**Spec:** §9.6.2.4, p. 183 — "DefaultMulticastLocator = {LOCATOR_KIND_UDPv4,
“239.255.0.1”, PB + DG * domainId + d0}. All Participants must announce and
listen on this multicast address."

**Repo:** `crates/rtps/src/wire_types.rs` (`SPDP_DEFAULT_MULTICAST_ADDRESS`);
`crates/dcps/src/runtime.rs` (`spdp_multicast_group`, joining and announcing)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_default_multicast_is_239_255_0_1`,
`crates/discovery/tests/spdp_loopback_e2e.rs::loopback_multicast_self_discovery`

**Status:** `done` — the group is configurable; without multicast ZeroDDS
works with initial peers.

### §9.6.2.4 Default announcement rate 30 seconds

**Spec:** §9.6.2.4, p. 183 — "The default rate by which SPDP periodic
announcements are sent equals 30 seconds. SPDPbuiltinParticipantWriter.
resendPeriod = {30, 0};"

**Repo:** `crates/dcps/src/runtime.rs` (`DEFAULT_SPDP_PERIOD`, `default_spdp_period_for`); `man/man5/zerodds.yaml.5`; `packaging/linux/configs/zerodds.yaml.example`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_short_lease_without_a_period_beacons_three_times_a_lease`,
`crates/dcps/tests/runtime_config_layering.rs::the_protocol_timing_parameters_are_settings`

**Status:** `done` — `DEFAULT_SPDP_PERIOD` = 30 s (YAML default, man page, example configuration); the former 5 s stay reachable through `discovery.spdp_period_ms: 5000`. A lease under 90 s shortens the default to a third (§8.5.3.1).

### §9.6.3.1 ParticipantMessageData: structure and wire form

**Spec:** §9.6.3.1, p. 184 — "struct ParticipantMessageData { GuidPrefix_t
participantGuidPrefix; OctetArray4 kind; OctetSeq data; };" wire form:
`GuidPrefix_t participantGuidPrefix` (12 octets), `octet[4] kind`,
`unsigned long data.length`, `octet[] data.value`.

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ParticipantMessageData`, `to_cdr`, `from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_with_topic_token`

**Status:** `done`

### §9.6.3.1 ParticipantMessageData: values of kind

**Spec:** §9.6.3.1, p. 184 — "#define PARTICIPANT_MESSAGE_DATA_KIND_UNKNOWN
{0x00, 0x00, 0x00, 0x00} #define
PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE {0x00, 0x00, 0x00,
0x01} #define PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_LIVELINESS_UPDATE {0x00,
0x00, 0x00, 0x02}"; "kind.value[0] & 0x80 == 1 // vendor specific kind".

**Repo:** `crates/rtps/src/participant_message_data.rs` (`PARTICIPANT_MESSAGE_DATA_KIND_*`, `ParticipantMessageData::is_vendor_kind`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_kind_constants_match_spec`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_octets_are_those_of_9_6_3_1`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_reads_the_fast_dds_beat`

**Status:** `done`

### §9.6.3.1 ParticipantMessageData: data at least 128 octets

**Spec:** §9.6.3.1, p. 184 — "Implementations can decide the upper length of the
data field but must be able to support at least 128 bytes."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`MAX_DATA_LEN = 4096`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_rejects_overlong_data`,
`crates/rtps/src/participant_message_data.rs::tests::participant_message_data_encoder_caps_data_length`

**Status:** `done`

### §9.6.3.2 IDL of the SPDP and SEDP data types

**Spec:** §9.6.3.2, pp. 184–185 — "struct SPDPdiscoveredParticipantData { …
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

**Status:** `done` — the missing attributes are listed under §8.5.3.2 and
§8.5.4.4.

### §9.6.3.2 Discovery data as a ParameterList in CDR

**Spec:** §9.6.3.2, p. 185 — "the wire-representation of the SerializedData
within the Data Submessage uses the format of a ParameterList
SubmessageElement. … Within each parameter, the parameter value is represented
using CDR."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`,
`to_pl_cdr_be`, `from_pl_cdr_le`); `crates/rtps/src/publication_data.rs`
(`to_pl_cdr_le`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done`

### §9.6.3.2 A vendor-specific EDP through a parameterId of its own

**Spec:** §9.6.3.2, p. 185 — "As this is a vendor-specific parameterId, other
vendors’ implementations would simply ignore the parameter and the information
it contains."

**Repo:** `crates/rtps/src/publication_data.rs` (vendor parameters by vendorId)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendors_0x8002_is_not_read_as_a_type_identifier`

**Status:** `done`

### §9.6.3.2 No redundant parameters

**Spec:** §9.6.3.2, p. 185 — "For optimization, implementations of the protocol
shall not include a parameter in the Data submessage if it contains
information that is redundant with other parameters already present in that
same Data submessage."

**Repo:** `crates/rtps/src/participant_data.rs`,
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`,
`crates/rtps/src/publication_data.rs::tests::roundtrip_le`

**Status:** `done` — the individual cases are under Table 9.17.

### §9.6.3.2 Table 9.17 SPDPdiscoveredParticipantData: ParticipantProxy::guidPrefix

**Spec:** §9.6.3.2, p. 185, Table 9.17 — "SPDPdiscoveredParticipantData —
ParticipantProxy::guidPrefix — ParticipantBuiltinTopicData::key".

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`: only
PID_PARTICIPANT_GUID)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`

**Status:** `done`

### §9.6.3.2 Table 9.17 DiscoveredReaderData: ReaderProxy::remoteReaderGuid

**Spec:** §9.6.3.2, p. 185, Table 9.17 — "DiscoveredReaderData —
ReaderProxy::remoteReaderGuid — SubscriptionBuiltinTopicData::key".

**Repo:** `crates/rtps/src/subscription_data.rs` (`to_pl_cdr_le`: only
PID_ENDPOINT_GUID)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::roundtrip_le`

**Status:** `done`

### §9.6.3.2 Table 9.17 DiscoveredWriterData: WriterProxy::remoteWriterGuid

**Spec:** §9.6.3.2, p. 185, Table 9.17 — "DiscoveredWriterData —
WriterProxy::remoteWriterGuid — PublicationBuiltinTopicData::key".

**Repo:** `crates/rtps/src/publication_data.rs` (`to_pl_cdr_le`: only
PID_ENDPOINT_GUID)

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_endpoint_guid`

**Status:** `done`

### §9.6.3.2 Key-only DATA of the built-in topics

**Spec:** §9.6.3.2, p. 185 — "In the case of a DATA submessage containing the
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

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_PAD — 0x0000 — N/A"; Table 9.19, p. 187 —
"PID_PAD — - — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_SENTINEL

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_SENTINEL — 0x0001 — N/A".

**Repo:** `crates/qos/src/pid.rs` (`Pid::SENTINEL`); `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_USER_DATA

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_USER_DATA — 0x002c — UserDataQosPolicy";
Table 9.19, p. 187 — ParticipantBuiltinTopicData, PublicationBuiltinTopicData,
SubscriptionBuiltinTopicData `user_data`, default "See DDS Specification".

**Repo:** `crates/qos/src/pid.rs` (`Pid::USER_DATA`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (`user_data`)

**Tests:** `crates/rtps/tests/user_data_propagation.rs::participant_user_data_roundtrip`,
`crates/rtps/tests/user_data_propagation.rs::subscription_user_data_roundtrip`,
`crates/rtps/tests/user_data_propagation.rs::publication_empty_user_data_omits_pid`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TOPIC_NAME

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_TOPIC_NAME — 0x0005 — string<256>";
Table 9.19, p. 187 — TopicBuiltinTopicData::name, Publication/Subscription
`topic_name`, default N/A.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TOPIC_NAME`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::roundtrip_utf8_topic_name`,
`crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_topic_name`,
`crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TYPE_NAME

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_TYPE_NAME — 0x0007 — string<256>"; Table
9.19, p. 187 — TopicBuiltinTopicData, Publication/Subscription `type_name`, default
N/A.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TYPE_NAME`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_type_name`,
`crates/rtps/src/publication_data.rs::tests::roundtrip_le`,
`crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`

**Status:** `done`

### §9.6.3.2 string<256> for names and domainTag

**Spec:** §9.6.3.2, p. 186, Table 9.18 — PID_TOPIC_NAME, PID_TYPE_NAME,
PID_DOMAIN_TAG: "string<256>".

**Repo:** `crates/rtps/src/publication_data.rs` (`STRING_256_MAX`, `encode_bounded_cdr_string_le`,
`decode_bounded_cdr_string`); `crates/dcps/src/participant.rs`; `crates/dcps/src/runtime.rs` (`start`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::topic_and_type_names_are_bounded_by_string_256`,
`crates/rtps/src/participant_data.rs::tests::a_domain_tag_longer_than_string_256_is_refused`,
`crates/dcps/src/participant.rs::tests::create_topic_rejects_a_name_longer_than_string_256`

**Status:** `done` — topic, type and domain-tag names are bounded to 256 octets: encoding and decoding refuse more (an announcement of a peer with a longer name is refused and counted), `create_topic` and `DcpsRuntime::start` refuse them.

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_DATA

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_GROUP_DATA — 0x002d —
GroupDataQosPolicy"; Table 9.19, p. 187 — Publication/Subscription `group_data`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::GROUP_DATA`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::user_topic_and_group_data_survive_the_sedp_roundtrip`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::topic_data_and_group_data_survive_the_builtin_topic_untouched`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TOPIC_DATA

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_TOPIC_DATA — 0x002e —
TopicDataQosPolicy"; Table 9.19, p. 187 — TopicBuiltinTopicData,
Publication/Subscription `topic_data`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TOPIC_DATA`);
`crates/rtps/src/topic_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::user_topic_and_group_data_survive_the_sedp_roundtrip`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::topic_data_and_group_data_survive_the_builtin_topic_untouched`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DURABILITY

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DURABILITY — 0x001d —
DurabilityQosPolicy"; Table 9.19, p. 187 — Topic, Publication, Subscription
`durability`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DURABILITY`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::durability_try_from_u32_rejects_unknown`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::defaults_roundtrip_stays_default`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DURABILITY_SERVICE

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DURABILITY_SERVICE — 0x001e —
DurabilityServiceQosPolicy"; Table 9.19, p. 187 — Topic, Publication
`durability_service`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DURABILITY_SERVICE`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/qos/src/policies/durability_service.rs::tests::default_matches_spec`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEADLINE

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DEADLINE — 0x0023 — DeadlineQosPolicy";
Table 9.19, p. 188 — Topic, Publication, Subscription `deadline`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DEADLINE`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LATENCY_BUDGET

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_LATENCY_BUDGET — 0x0027 —
LatencyBudgetQosPolicy"; Table 9.19, p. 188 — Topic, Publication, Subscription
`latency_budget`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LATENCY_BUDGET`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LIVELINESS

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_LIVELINESS — 0x001b —
LivelinessQosPolicy"; Table 9.19, p. 188 — Topic, Publication, Subscription
`liveliness`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LIVELINESS`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_RELIABILITY

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_RELIABILITY — 0x001a —
ReliabilityQosPolicy" with footnote 3: "The encoding of
DDS::ReliabilityQoSPolicy::kind is defined by RTPS::ReliabilityKind_t (9.3.2)";
Table 9.19, p. 188 — Topic, Publication, Subscription `reliability`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::RELIABILITY`);
`crates/rtps/src/publication_data.rs` (kind as 1/2)

**Tests:** `crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`,
`crates/rtps/src/publication_data.rs::tests::reliability_try_from_u32_rejects_unknown`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_LIFESPAN

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_LIFESPAN — 0x002b — LifespanQosPolicy";
Table 9.19, p. 188 — Topic, Publication `lifespan`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::LIFESPAN`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DESTINATION_ORDER

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DESTINATION_ORDER — 0x0025 —
DestinationOrderQosPolicy"; Table 9.19, p. 188 — Topic, Publication, Subscription
`destination_order`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::DESTINATION_ORDER`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/qos/src/policies/destination_order.rs::tests::roundtrip`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_HISTORY

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_HISTORY — 0x0040 — HistoryQosPolicy";
Table 9.19, p. 188 — TopicBuiltinTopicData::history.

**Repo:** `crates/qos/src/pid.rs` (`Pid::HISTORY`); `crates/rtps/src/topic_data.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_RESOURCE_LIMITS

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_RESOURCE_LIMITS — 0x0041 —
ResourceLimitsQosPolicy"; Table 9.19, p. 188 — TopicBuiltinTopicData::resource_limits.

**Repo:** `crates/qos/src/pid.rs` (`Pid::RESOURCE_LIMITS`);
`crates/rtps/src/topic_data.rs`

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_OWNERSHIP

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_OWNERSHIP — 0x001f — OwnershipQosPolicy";
Table 9.19, p. 188 — Topic, Publication, Subscription `ownership`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::OWNERSHIP`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_OWNERSHIP_STRENGTH

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_OWNERSHIP_STRENGTH — 0x0006 —
OwnershipStrengthQosPolicy"; Table 9.19, p. 188 —
PublicationBuiltinTopicData::ownership_strength.

**Repo:** `crates/qos/src/pid.rs` (`Pid::OWNERSHIP_STRENGTH`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PRESENTATION

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_PRESENTATION — 0x0021 —
PresentationQosPolicy"; Table 9.19, p. 188 — Publication, Subscription
`presentation`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::PRESENTATION`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTITION

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_PARTITION — 0x0029 —
PartitionQosPolicy"; Table 9.19, p. 188 — Publication, Subscription `partition`.

**Repo:** `crates/qos/src/pid.rs` (`Pid::PARTITION`)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::publication_data_roundtrip_preserves_all_new_qos_policies`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_TIME_BASED_FILTER

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_TIME_BASED_FILTER — 0x0004 —
TimeBasedFilterQosPolicy"; Table 9.19, p. 188 —
SubscriptionBuiltinTopicData::time_based_filter.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TIME_BASED_FILTER`);
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::subscription_data_roundtrip_preserves_reader_qos_policies`

**Status:** `done`

### §9.6.3.2 Table 9.18 PID_TRANSPORT_PRIORITY

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_TRANSPORT_PRIORITY — 0x0049 —
TransportPriorityQoSPolicy". Table 9.19 has no row for it.

**Repo:** `crates/qos/src/pid.rs` (`Pid::TRANSPORT_PRIORITY`);
`crates/rtps/src/topic_data.rs`; `crates/rtps/src/publication_data.rs`
(inline QoS)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DOMAIN_ID

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID__DOMAIN_ID — 0x000f — DomainId_t"; Table
9.19, p. 188 — ParticipantProxy::domainId, default "The domainId of the local
participant receiving the SPDPdiscoveredParticipantData".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DOMAIN_ID`);
`crates/dcps/src/runtime.rs` (`spdp_announced`: missing = own domain)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DOMAIN_TAG

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DOMAIN_TAG — 0x4014 — string<256>"; Table
9.19, p. 188 — ParticipantProxy::domainTag, default "“” (empty, zero-length string)".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DOMAIN_TAG`);
`crates/rtps/src/participant_data.rs` (missing = empty)

**Tests:** `crates/rtps/src/participant_data.rs::tests::domain_tag_goes_out_when_set_and_reads_back`,
`crates/rtps/src/participant_data.rs::tests::domain_tag_is_a_standard_must_understand_pid`,
`crates/rtps/src/participant_data.rs::tests::a_domain_tag_longer_than_string_256_is_refused`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PROTOCOL_VERSION

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_PROTOCOL_VERSION — 0x0015 —
ProtocolVersion_t"; Table 9.19, p. 188 — ParticipantProxy::protocolVersion.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PROTOCOL_VERSION`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_VENDORID

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_VENDORID — 0x0016 — VendorId_t"; Table
9.19, p. 188 — ParticipantProxy::vendorId.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::VENDOR_ID`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_foreign_vendor_id_survives_read_and_write_back`,
`crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_UNICAST_LOCATOR

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_UNICAST_LOCATOR — 0x002f — Locator_t";
Table 9.19, p. 188 — ReaderProxy/WriterProxy::unicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::UNICAST_LOCATOR`);
`crates/rtps/src/publication_data.rs` (`collect_locator_params`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_MULTICAST_LOCATOR — 0x0030 —
Locator_t"; Table 9.19, p. 188 — ReaderProxy/WriterProxy::multicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::MULTICAST_LOCATOR`);
`crates/rtps/src/publication_data.rs` (`collect_locator_params`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Endpoint locators in PL_CDR_BE

**Spec:** §9.6.3.2, Table 9.19, p. 188 — PID_UNICAST_LOCATOR and PID_MULTICAST_LOCATOR
for ReaderProxy and WriterProxy; §9.6.3.2, p. 185: "Within each parameter, the
parameter value is represented using CDR."

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEFAULT_UNICAST_LOCATOR

**Spec:** §9.6.3.2, p. 186, Table 9.18 — "PID_DEFAULT_UNICAST_LOCATOR — 0x0031 —
Locator_t"; Table 9.19, p. 188 — ParticipantProxy::defaultUnicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DEFAULT_UNICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_DEFAULT_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_DEFAULT_MULTICAST_LOCATOR — 0x0048 —
Locator_t"; Table 9.19, p. 189 — ParticipantProxy::defaultMulticastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DEFAULT_MULTICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_METATRAFFIC_UNICAST_LOCATOR

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_METATRAFFIC_UNICAST_LOCATOR — 0x0032 —
Locator_t"; Table 9.19, p. 189 — ParticipantProxy::metatrafficUnicastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::METATRAFFIC_UNICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::decode_retains_all_repeated_unicast_locators_in_order_le_and_be`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_METATRAFFIC_MULTICAST_LOCATOR

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_METATRAFFIC_MULTICAST_LOCATOR — 0x0033 —
Locator_t"; Table 9.19, p. 189 — ParticipantProxy::metatrafficMulticastLocatorList.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::METATRAFFIC_MULTICAST_LOCATOR`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_roundtrip_full`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_EXPECTS_INLINE_QOS

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_EXPECTS_INLINE_QOS — 0x0043 — boolean";
Table 9.19, p. 189 — ParticipantProxy::expectsInlineQos, default FALSE.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::EXPECTS_INLINE_QOS`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`,
`crates/rtps/src/subscription_data.rs::tests::expects_inline_qos_goes_out_when_set_and_reads_back`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT —
0x0034 — Count_t"; Table 9.19, p. 189 — ParticipantProxy::manualLivelinessCount.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PARTICIPANT_MANUAL_LIVELINESS_COUNT`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done` — read, written and in `is_standard_pid`; OpenDDS sends 0 and gets 0 back.

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_LEASE_DURATION

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_PARTICIPANT_LEASE_DURATION — 0x0002 —
Duration_t"; Table 9.19, p. 189 — SPDPdiscoveredParticipantData::leaseDuration,
default {100, 0}.

**Repo:** `crates/rtps/src/parameter_list.rs`
(`pid::PARTICIPANT_LEASE_DURATION`); `crates/rtps/src/participant_data.rs`
(missing = 100 s)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §9.6.3.2 PID_PARTICIPANT_LEASE_DURATION in PL_CDR_BE

**Spec:** §9.6.3.2, p. 185 — "Within each parameter, the parameter value is
represented using CDR."; Table 9.18: Duration_t.

**Repo:** —

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_CONTENT_FILTER_PROPERTY

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_CONTENT_FILTER_PROPERTY — 0x0035 —
ContentFilterProperty_t"; Table 9.19, p. 189 — DiscoveredReaderData::contentFilter.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::CONTENT_FILTER_PROPERTY`);
`crates/rtps/src/subscription_data.rs`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::subscription_with_content_filter_roundtrip_le`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_PARTICIPANT_GUID

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_PARTICIPANT_GUID — 0x0050 — GUID_t";
Table 9.19, p. 189 — ParticipantBuiltinTopicData::key, Publication/Subscription
`participant_key`.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PARTICIPANT_GUID`);
`crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_requires_guid_pid`,
`crates/rtps/src/publication_data.rs::tests::participant_key_fallback_when_pid_missing`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_GUID

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_GROUP_GUID — 0x0052 — GUID_t"; Table 9.19
— WriterProxy/ReaderProxy::remoteGroupGuid, default GUID_UNKNOWN.

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_GROUP_ENTITY_ID

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_GROUP_ENTITY_ID — 0x0053 — EntityId_t";
Table 9.19, p. 189 — WriterProxy/ReaderProxy::remoteGroupGuid.entityId, default
ENTITYID_UNKNOWN.

**Repo:** —

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_writers_group_travels_as_pid_group_guid`

**Status:** `done` — read; ZeroDDS sends the group as a full GUID in
`PID_GROUP_GUID`.

### §9.6.3.2 Table 9.18/9.19 PID_BUILTIN_ENDPOINT_SET

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_BUILTIN_ENDPOINT_SET — 0x0058 —
BuiltinEndpointSet_t"; Table 9.19, p. 189 — ParticipantProxy::availableBuiltinEndpoints.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::BUILTIN_ENDPOINT_SET`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_roundtrip_through_pl_cdr`

**Status:** `done`

### §9.6.3.2 Table 9.18/9.19 PID_BUILTIN_ENDPOINT_QOS

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_BUILTIN_ENDPOINT_QOS — 0x0077 —
BuiltinEndpointQos_t"; Table 9.19, p. 189 — ParticipantProxy::builtinEndpointQos.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::BUILTIN_ENDPOINT_QOS`);
`crates/rtps/src/participant_data.rs`

**Tests:** `crates/rtps/src/participant_data.rs::tests::manual_liveliness_count_and_builtin_endpoint_qos_go_out_and_read_back`,
`crates/discovery/tests/vendor_discovery_frames.rs::opendds_announcements_read_and_write_back`

**Status:** `done` — read, written and in `is_standard_pid`.

### §9.6.3.2 Table 9.18 PID_PROPERTY_LIST

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_PROPERTY_LIST — 0x0059 —
sequence<Property_t>". Table 9.19 has no row for it.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PROPERTY_LIST`);
`crates/rtps/src/participant_data.rs` (`properties`);
`crates/rtps/src/property_list.rs` (`WirePropertyList::decode`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_empty_properties_omits_pid`,
`crates/rtps/src/property_list.rs::tests::decode_reads_the_rtps_form_without_binary_value`

**Status:** `done` — sent in SPDP and, where an endpoint has properties, in
SEDP, in the form of DDS-Security 1.2 §7.5.1.4 (with `binary_value`). Both
forms are read; a list that ends after its properties (this row) has no
binary entries.

### §9.6.3.2 Table 9.18/9.19 PID_TYPE_MAX_SIZE_SERIALIZED

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_TYPE_MAX_SIZE_SERIALIZED — 0x0060 —
long"; Table 9.19, p. 189 — "PID_DATA_MAX_SIZE_SERIALIZED —
WriterProxy::dataMaxSizeSerialized".

**Repo:** `crates/rtps/src/publication_data.rs` (`data_max_size_serialized`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`,
`crates/rtps/src/reliable_reader.rs::tests::a_payload_beyond_the_announced_maximum_is_refused_and_counted`

**Status:** `done` — publications send and read the PID as a `long` (`PublicationBuiltinTopicData::data_max_size_serialized`); `vendor_discovery_frames.rs` no longer lists 0x0060 among the dropped PIDs. Measured in `tools/wire-capture/rtps-sessions/rti`: RTI 7.7.0 announces the value 8, and its writer's DATA carries a SerializedPayload of 8 octets, the encapsulation header included; the value counts with the header, as the spec does. Fast DDS 2.6.10 announces 12, 32 and 52 (no user DATA in the capture); Cyclone and OpenDDS do not announce the PID. A reader refuses a payload that exceeds the value named. Not measured: a run against the vendors with this refusal.

### §9.6.3.2 Table 9.18 PID_ENTITY_NAME

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_ENTITY_NAME — 0x0062 — EntityName_t".
Table 9.19 has no row for it.

**Repo:** `crates/rtps/src/participant_data.rs`, `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (`entity_name`);
`crates/dcps/src/runtime.rs` (`built_entity_name`, `built_participant_name`, `RuntimeConfig::participant_name`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::the_participant_name_goes_out_and_reads_back`,
`crates/rtps/src/publication_data.rs::tests::the_max_payload_size_and_the_name_of_a_writer_travel`

**Status:** `done` — participant, writer and reader send and read `PID_ENTITY_NAME` as `EntityName_t` (CDR string). The DCPS API has no EntityName QoS; the names are built: participant `zerodds:` and the GUID prefix in hex (overridable with `discovery.participant_name`), writer and reader `DataWriter:<topic>:<entity id hex>` and `DataReader:…`. `UserWriterConfig::entity_name` and `UserReaderConfig::entity_name` take a name that is set. A name that cannot be read (not a valid CDR string) refuses the announcement, as an unreadable `domainTag` does.

### §9.6.3.2 Table 9.18/9.19 PID_ENDPOINT_GUID

**Spec:** §9.6.3.2, p. 187, Table 9.18 — "PID_ENDPOINT_GUID — 0x005a — GUID_t"; Table
9.19, p. 189 — Topic-, Subscription-, PublicationBuiltinTopicData::key.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::ENDPOINT_GUID`);
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/topic_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::decode_rejects_missing_endpoint_guid`,
`crates/discovery/src/sedp/stack.rs::tests::two_stacks_exchange_a_topic_its_new_qos_and_its_dispose`

**Status:** `done`

### §9.6.3.2 Defaults "See DDS Specification" for missing QoS PIDs

**Spec:** §9.6.3.2, Table 9.19, pp. 187–188 — the QoS rows name "See DDS Specification." as
the default; §9.6.3.2: "Unrecognized parameterIDs shall be treated as
specified in Table 9.6."

**Repo:** `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs`, `crates/rtps/src/topic_data.rs`
(`unwrap_or_default` per policy)

**Tests:** `crates/rtps/tests/sedp_qos_roundtrip.rs::defaults_roundtrip_stays_default`,
`crates/rtps/src/publication_data.rs::tests::an_absent_reliability_parameter_reads_as_reliable_on_a_publication`

**Status:** `done`

### §9.6.4 Inline QoS ParameterIds

**Spec:** §9.6.4, p. 189 — "Sub clause 8.7.2.1 defines the complete set of
parameters that can appear within the inlineQos SubmessageElement. The
corresponding set of parameterIds is listed in Table 9.20. Unrecognized
parameterIDs shall be treated as specified in Table 9.6."

**Repo:** `crates/rtps/src/inline_qos.rs`; `crates/rtps/src/parameter_list.rs`

**Tests:** `crates/rtps/src/inline_qos.rs::tests::roundtrip_in_data_submessage_inline_qos`,
`crates/rtps/src/datagram.rs::tests::decode_accepts_data_with_known_must_understand_pid_in_inline_qos`

**Status:** `done` — the treatment of unknown PIDs is listed under §9.4.2.11
Table 9.6.

### §9.6.4 Table 9.20 PID_PAD

**Spec:** §9.6.4, p. 189, Table 9.20 — "PID_PAD — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::PAD`,
`ParameterList::from_bytes_with_consumed`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::pid_pad_is_skipped`

**Status:** `done`

### §9.6.4 Table 9.20 PID_SENTINEL

**Spec:** §9.6.4, p. 189, Table 9.20 — "PID_SENTINEL — N/A".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SENTINEL`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::empty_parameter_list_is_just_sentinel`

**Status:** `done`

### §9.6.4 Table 9.20 PID_TOPIC_NAME

**Spec:** §9.6.4, p. 189, Table 9.20 — "PID_TOPIC_NAME — See Table 9.18 —
string<256>".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DURABILITY

**Spec:** §9.6.4, p. 189, Table 9.20 — "PID_DURABILITY — See Table 9.18 —
DurabilityQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_PRESENTATION

**Spec:** §9.6.4, p. 189, Table 9.20 — "PID_PRESENTATION — PresentationQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DEADLINE

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_DEADLINE — DeadlineQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LATENCY_BUDGET

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_LATENCY_BUDGET —
LatencyBudgetQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_OWNERSHIP

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_OWNERSHIP — OwnershipQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_OWNERSHIP_STRENGTH

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_OWNERSHIP_STRENGTH —
OwnershipStrengthQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LIVELINESS

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_LIVELINESS — LivelinessQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_PARTITION

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_PARTITION — PartitionQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_RELIABILITY

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_RELIABILITY — ReliabilityQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_TRANSPORT_PRIORITY

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_TRANSPORT_PRIORITY —
TransportPriorityQoSPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::a_transport_priority_goes_in_line_and_not_into_the_announcement`

**Status:** `done`

### §9.6.4 Table 9.20 PID_LIFESPAN

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_LIFESPAN — LifespanQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DESTINATION_ORDER

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_DESTINATION_ORDER —
DestinationOrderQosPolicy".

**Repo:** `crates/rtps/src/publication_data.rs` (`PublicationBuiltinTopicData::inline_qos`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::inline_qos_is_the_announcements_in_line_policies_and_the_topic_name`

**Status:** `done`

### §9.6.4 Table 9.20 PID_CONTENT_FILTER_INFO (0x0055)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_CONTENT_FILTER_INFO — 0x0055 —
ContentFilterInfo_t".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — belongs to filtering on the writer side,
which ZeroDDS does not do (§8.7.3.4); a received parameter is passed over as
an unknown PID without the must-understand bit.

### §9.6.4 Table 9.20 PID_COHERENT_SET (0x0056)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_COHERENT_SET — 0x0056 —
SequenceNumber_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::COHERENT_SET`);
`crates/rtps/src/inline_qos.rs` (`coherent_set_inline_qos`,
`find_coherent_set`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_roundtrip_both_endiannesses`

**Status:** `done`

### §9.6.4 Table 9.20 PID_DIRECTED_WRITE (0x0057)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_DIRECTED_WRITE — 0x0057 —
GUID_t" with footnote 4: "RTPS protocol versions prior to 2.4 defined this as a
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

**Status:** `done` — 0x0057 with a `GUID_t`; note 4: for a protocol version before 2.4 (a `sequence<GUID_t>` there) the parameter is ignored. Sending and evaluating are `done` under §8.7.7.

### §9.6.4 Table 9.20 PID_ORIGINAL_WRITER_INFO (0x0061)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_ORIGINAL_WRITER_INFO — 0x0061 —
OriginalWriterInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::ORIGINAL_WRITER_INFO`);
`crates/rtps/src/inline_qos.rs` (`original_writer_info_param`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_le`

**Status:** `done` — 0x0061 with `OriginalWriterInfo_t` (24 octets, §9.6.4.7). Sending and evaluating are `done` under §8.7.9.

### §9.6.4 Table 9.20 PID_GROUP_COHERENT_SET (0x0063)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_GROUP_COHERENT_SET — 0x0063 —
SequenceNumber_t".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_GROUP_SEQ_NUM (0x0064)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_GROUP_SEQ_NUM — 0x0064 —
SequenceNumber_t".

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_WRITER_GROUP_INFO (0x0065)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_WRITER_GROUP_INFO — 0x0065 —
WriterGroupInfo_t".

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4 Table 9.20 PID_SECURE_WRITER_GROUP_INFO (0x0066)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_SECURE_WRITER_GROUP_INFO — 0x0066
— WriterGroupInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SECURE_WRITER_GROUP_INFO`);
`crates/rtps/src/inline_qos.rs` (`secure_writer_group_info_param`, `find_writer_group_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`,
`crates/rtps/src/reliable_writer.rs::tests::the_secure_writer_group_info_stands_where_the_plain_one_is_required`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done` — 0x0066 with `WriterGroupInfo_t` (4 octets); a writer under DDS-Security sends it, a reader reads it in place of `PID_WRITER_GROUP_INFO`.

### §9.6.4 Table 9.20 PID_KEY_HASH (0x0070)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_KEY_HASH — 0x0070 — KeyHash_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::KEY_HASH`);
`crates/rtps/src/inline_qos.rs` (`find_key_hash`)

**Tests:** `crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`,
`crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`

**Status:** `done`

### §9.6.4 Table 9.20 PID_STATUS_INFO (0x0071)

**Spec:** §9.6.4, p. 190, Table 9.20 — "PID_STATUS_INFO — 0x0071 —
StatusInfo_t".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::STATUS_INFO`);
`crates/rtps/src/inline_qos.rs` (`status_info_param`, `find_status_info`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`

**Status:** `done`

### §9.6.4.1 PID_CONTENT_FILTER_INFO: wire form, bitmap, signature

**Spec:** §9.6.4.1, p. 191 — wire form numBitmaps, bitmaps, numSignatures,
FilterSignature_t; "The content filter info in-line QoS is invalid unless
numBitmaps == ([numSignatures/32] + (numSignatures%32 ? 1 : 0))"; "A filter’s
signature is calculated as the 128-bit MD5 checksum of all strings in the
filter's ContentFilterProperty_t."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — belongs to filtering on the writer side,
which ZeroDDS does not do (§8.7.3.4).

### §9.6.4.1 Table 9.21 filterResult

**Spec:** §9.6.4.1, p. 191, Table 9.21 — "0 — Sample was filtered by the
corresponding filter and did not pass. 1 — Sample was filtered by the
corresponding filter and passed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — belongs to filtering on the writer side,
which ZeroDDS does not do (§8.7.3.4).

### §9.6.4.2 PID_COHERENT_SET: encoding and value

**Spec:** §9.6.4.2, p. 191 — "The coherent set in-line QoS parameter uses the
CDR encoding for SequenceNumber_t. … all Data and DataFrag Submessages that
belong to the same coherent set must contain the coherent set in-line QoS
parameter with value equal to the sequence number of the first sample in the
set."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::coherent_set_param_is_pid_0x0056_with_sequence_number_layout`,
`crates/rtps/src/inline_qos.rs::tests::coherent_set_roundtrip_both_endiannesses`,
`crates/rtps/src/reliable_writer.rs::tests::coherent_set_member_data_carries_pid_0x0056_with_set_first_sn_octets`

**Status:** `done`

### §9.6.4.2 Table 9.22 examples for the end of a coherent set

**Spec:** §9.6.4.2, p. 192, Table 9.22 — "Example Data Submessages to denote the
end of a coherent set".

**Repo:** `crates/rtps/src/reliable_reader.rs` (`receive_data`: `coherent_set_end`);
`crates/rtps/src/reliable_writer.rs` (`emit_coherent_set_end`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::table_9_22_each_form_that_ends_a_coherent_set_ends_it`,
`crates/rtps/src/reliable_reader.rs::tests::end_of_coherent_set_data_is_delivered_as_a_terminator`,
`crates/rtps/src/reliable_writer.rs::tests::end_coherent_set_emits_ecs_data_with_d_and_k_clear`

**Status:** `done` — the reader recognises all three forms of the table: Example 1 (DATA with data and `PID_COHERENT_SET` 7, a new set), Example 2 (no data, `PID_COHERENT_SET` SEQUENCENUMBER_UNKNOWN) and Example 3 (no data, no in-line QoS); the set 3 to 6 is ended and complete afterwards. The writer sends Example 3.

### §9.6.4.3 PID_GROUP_COHERENT_SET

**Spec:** §9.6.4.3, p. 192 — "The group coherent set in-line QoS parameter uses
the CDR encoding for SequenceNumber_t. … all Data submessages and the first
DataFrag submessage belonging to a sample must contain the group coherent set
in-line QoS parameter".

**Repo:** —

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_go_without_it_gets_no_group_coherent_set_pid`,
`crates/rtps/src/reliable_writer.rs::tests::a_participant_set_to_coherent_sets_only_gets_no_group_information`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.3 Table 9.23 example of a GROUP coherent set

**Spec:** §9.6.4.3, p. 192, Table 9.23 — "Example Data Submessages in a GROUP
coherent set".

**Repo:** `crates/rtps/src/reliable_writer.rs` (`push_set_params`);
`crates/rtps/src/inline_qos.rs` (`find_change_group_info`);
`crates/rtps/src/reliable_reader.rs` (`DeliveredSample::group`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_group_coherent_set_carries_the_parameters_of_table_9_23`,
`crates/rtps/src/reliable_reader.rs::tests::table_9_23_the_data_of_a_group_coherent_set_reach_the_consumer_with_their_group_numbers`,
`crates/rtps/src/writer_group.rs::tests::the_numbers_of_table_9_23`

**Status:** `done` — two writers of a GROUP publisher produce the values of the table (writerSN 4, 8, 5, 6, 9; GSN 11 to 14; `PID_COHERENT_SET` 4 and 8; `PID_GROUP_COHERENT_SET` 11); the reader passes on the group numbers with each sample.

### §9.6.4.4 PID_GROUP_SEQ_NUM

**Spec:** §9.6.4.4, p. 193 — "all Data submessages and the first DataFrag
submessage sent by DataWriters belonging to a Publisher with Presentation
access scope GROUP must contain the group sequence number in-line QoS parameter
with value equal to the group sequence number."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064`,
`crates/rtps/src/reliable_writer.rs::tests::a_group_writers_data_carries_its_group_sequence_number`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.5 PID_WRITER_GROUP_INFO

**Spec:** §9.6.4.5, p. 193 — "for DataWriters belonging to a Publisher with
Presentation access scope GROUP, the Data submessages and the first DataFrag
submessage of each sample shall contain the publisher writer info in-line QoS
parameter. The End Coherent Set Data submessage … shall also contain the
publisher writer info in-line QoS parameter."

**Repo:** —

**Tests:** `crates/rtps/src/inline_qos.rs::tests::writer_group_info_carries_the_four_digest_octets_with_pid_0x0065`,
`crates/rtps/src/parameter_list.rs::tests::the_group_and_name_parameter_ids_are_those_of_table_9_18`

**Status:** `done`

### §9.6.4.6 PID_SECURE_WRITER_GROUP_INFO

**Spec:** §9.6.4.6, p. 193 — "The secure publisher writer info in-line QoS is
reserved for DDS Security. In the cases when it is used it shall be added
anywhere that the PID_WRITER_GROUP_INFO in-line QoS is required."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SECURE_WRITER_GROUP_INFO`);
`crates/rtps/src/reliable_writer.rs` (`set_secure_writer_group_info`, `push_set_params`);
`crates/rtps/src/inline_qos.rs` (`secure_writer_group_info_param`, `find_writer_group_info`);
`crates/dcps/src/runtime.rs` (writer creation under security)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::the_secure_writer_group_info_stands_where_the_plain_one_is_required`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — under DDS-Security (`RuntimeConfig::security`) writers of a GROUP publisher send `PID_SECURE_WRITER_GROUP_INFO` wherever `PID_WRITER_GROUP_INFO` would stand; a reader takes both. The test with a security runtime is not in this run (only the writer setting is tested).

### §9.6.4.7 PID_ORIGINAL_WRITER_INFO: wire form

**Spec:** §9.6.4.7, p. 193 — wire form `GUID_t originalWriterGUID`,
`SequenceNumber originalWriterSN`. "The original writer info parameter may
appear in the Data or in the DataFrag submessages."

**Repo:** `crates/rtps/src/inline_qos.rs` (`original_writer_info_param`,
`find_original_writer_info`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::original_writer_info_param_24_byte_layout_le`,
`crates/rtps/src/inline_qos.rs::tests::original_writer_info_roundtrip_be`,
`crates/rtps/src/inline_qos.rs::tests::table_9_20_directed_write_original_writer_info_and_secure_group_info`

**Status:** `done` — the wire form is right (GUID_t, SequenceNumber_t); the parameter is sent and evaluated in DATA and DATA_FRAG (items under §8.7.9).

### §9.6.4.8 KeyHash: 16 octets

**Spec:** §9.6.4.8, p. 193 — "The KeyHash_t is defined as a 16-Byte octet array
(see 9.3.2) therefore the key hash inline parameter just copies those 16
Bytes."

**Repo:** `crates/dcps/src/key_hash.rs` (`KEY_HASH_LEN`);
`crates/rtps/src/inline_qos.rs` (`find_key_hash`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_holder_of_exactly_sixteen_is_the_hash`,
`crates/rtps/src/submessages.rs::tests::data_frag_key_hash_roundtrips_both_byte_orders`

**Status:** `done`

### §9.6.4.8 KeyHash step 1: FooKeyHolder

**Spec:** §9.6.4.8, pp. 193–194 — "FooKeyHolder shall have extensibility kind
'FINAL' … If there are any key members, then remove the non-key members …
Reorder the members in ascending order of their memberId values."

**Repo:** `crates/dcps/src/key_hash.rs` (`keyhash_cdr2_be`: sorting by
`member_id`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::the_member_order_does_not_reach_the_hash`

**Status:** `done`

### §9.6.4.8 KeyHash steps 2 and 3: values and recursion

**Spec:** §9.6.4.8, p. 194 — "Step 2. … setting the members present in
FooKeyHolderObject to the same values as the corresponding members in
FooObject. Step 3. Apply steps 1 and 2 recursively to the members of
FooKeyHolder if they are themselves Aggregated types".

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`);
`crates/dcps/src/runtime.rs` (`reader_key_hash_context`)

**Tests:** `crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_marker_carries_the_key_fields_of_a_string_keyed_type`

**Status:** `done`

### §9.6.4.8 KeyHash step 4: PLAIN_CDR2 big endian

**Spec:** §9.6.4.8, p. 194 — "Compute the PLAIN_CDR2 Big Endian Serialization
… The serialization shall be performed on a buffer that is initially aligned
to the maximum alignment in PLAIN_CDR2 (i.e., 4). Furthermore, any padding
bytes added due to alignment rules shall be set to zero."

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_u32_goes_out_big_endian`,
`crates/dcps/src/key_hash.rs::tests::a_u64_is_aligned_to_four_and_not_to_eight`,
`crates/dcps/src/key_hash.rs::tests::a_string_carries_its_terminator_in_its_length`

**Status:** `done`

### §9.6.4.8 KeyHash step 5: padding or MD5 by the maximum size

**Spec:** §9.6.4.8, p. 194 — "Step 5.1 If the FooKeyHolder has a maximum
serialized size that is less than or equal to 16 bytes, then the KeyHash of
FooObject shall be set to the result of Step 4, extended to 16 bytes. … Step
5.2 If the FooKeyHolder has a maximum serialized size that is greater than 16
bytes, then the KeyHash_t of FooObject shall be set to the MD5 Hash".

**Repo:** `crates/dcps/src/key_hash.rs` (`compute_key_hash`:
`key_holder_max_size`)

**Tests:** `crates/dcps/src/key_hash.rs::tests::a_short_instance_of_a_bounded_type_still_pads`,
`crates/dcps/src/key_hash.rs::tests::a_holder_over_sixteen_is_hashed`

**Status:** `done`

### §9.6.4.8 KeyHash examples 1 to 3

**Spec:** §9.6.4.8, pp. 194–196 — "Example 1 …", "Example 2 …", "Example 3 …"
with the expected octets.

**Repo:** `crates/dcps/src/key_hash.rs` (`PlainCdr2BeKeyHolder`, `compute_key_hash`);
`crates/dcps/src/dds_type.rs` (`DdsType::compute_key_hash`)

**Tests:** `crates/dcps/src/dds_type.rs::tests::key_hash_example_1_of_the_spec_is_the_holder_padded_with_zeros`,
`crates/dcps/src/dds_type.rs::tests::key_hash_example_2_of_the_spec_is_the_md5_of_the_holder`,
`crates/dcps/src/dds_type.rs::tests::key_hash_example_3_of_the_spec_is_the_md5_of_the_reordered_holder`

**Status:** `done` — the three examples give the octets of the spec: Example 1 the holder extended with zeros to 16 octets, Examples 2 and 3 the MD5 of the 20-octet and the 13-octet holder (`f91a59e3…`, `374b96e2…`).

### §9.6.4.9 StatusInfo_t: 4 octets

**Spec:** §9.6.4.9, p. 196 — "The StatusInfo_t is defined as a 4-Byte octet
array (see 9.3.2) therefore the status info inline parameter just copies those
4 Bytes."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info_param`: network order;
`find_status_info`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** `done`

### §9.6.4.9 StatusInfo in Data or DataFrag

**Spec:** §9.6.4.9, p. 196 — "The status info parameter may appear in the Data
or in the DataFrag submessages."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`classify_change_kind` for
DATA, `handle_data_frag_opening` for DATA_FRAG)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_key_is_a_dispose_or_a_registration_as_a_data_is`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`

**Status:** `done` — evaluated in DATA and in DATA_FRAG (a key with D or U as dispose and unregister, F as a filtered sample).

### §9.6.4.9 StatusInfo: X flags

**Spec:** §9.6.4.9, p. 197 — "The flags represented with the literal ‘X’ are
unused by this version of the protocol and should be set to zero by the writer
and not interpreted by the reader".

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info`);
`crates/rtps/src/reliable_reader.rs` (`classify_change_kind`: D and U only)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §9.6.4.9 StatusInfo DisposedFlag

**Spec:** §9.6.4.9, p. 197 — "D=1 indicates that the DDS DataWriter has
disposed the instance of the data-object whose Key appears in the submessage."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info::DISPOSED = 0x1`)

**Tests:** `crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`

**Status:** `done`

### §9.6.4.9 StatusInfo UnregisteredFlag

**Spec:** §9.6.4.9, p. 197 — "U=1 indicates that the DDS DataWriter has
unregistered the instance of the data-object whose Key appears in the
submessage."

**Repo:** `crates/rtps/src/inline_qos.rs` (`status_info::UNREGISTERED = 0x2`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done`

### §9.6.4.9 StatusInfo FilteredFlag

**Spec:** §9.6.4.9, p. 197 — "F=1 indicates that the DDS DataWriter has written
as sample for the instance … but the sample did not pass the content filter
specified by the DDS DataReader." "If both DisposedFlag==0 and
UnregisteredFlag=0 … FilteredFlag==1 indicates it did not pass the filter."

**Repo:** `crates/rtps/src/reliable_reader.rs` (`status_change_kind`, `classify_change_kind`,
`ReliableReader::filtered_sample_count`)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::a_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::a_reassembled_sample_with_the_filtered_flag_is_known_but_not_delivered`,
`crates/rtps/src/reliable_reader.rs::tests::the_reader_keeps_the_status_of_each_change`

**Status:** `done` — a DATA or DATA_FRAG with F = 1 and D = U = 0 (with or without payload and key flag) gives `ChangeKind::AliveFiltered`: the number is known and NA_FILTERED (the writer does not repeat it), the sample is not delivered and is counted in `filtered_sample_count`. With D or U set the lifecycle marker applies. ZeroDDS does not send F (the runtime does not filter on the writer side); whether another vendor sends F is not measured.

### §9.6.4.9 Registration not propagated

**Spec:** §9.6.4.9, p. 197 — "Note that the protocol does not require that the
DDS DataWriter propagates the "register" operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §9.6.5 Table 9.24 DDS-XTypes

**Spec:** §9.6.5, p. 197, Table 9.24 — "DDS-XTypes 1.1 … 1.3 — 0x0069, 0x0072,
0x0073, 0x0074, 0x0075, 0x0076; Note that 0x0076 was deprecated in DDS-XTypes
1.2".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::DATA_REPRESENTATION`,
`TYPE_CONSISTENCY_ENFORCEMENT`, `TYPE_INFORMATION`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::issue30_type_identifier_from_standard_type_information`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §9.6.5 Table 9.24 DDS-Security

**Spec:** §9.6.5, p. 197, Table 9.24 — "DDS-Security 1.1 — Parameter IDs in the
range 0x1000 to 0x1FFF; Parameter IDs in the range 0x5000 to 0x5FFF".

**Repo:** `crates/rtps/src/parameter_list.rs` (security PIDs 0x1001 to 0x1013)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`,
`crates/rtps/src/participant_data.rs::tests::participant_security_info_pid_roundtrip`

**Status:** `done`

### §9.6.5 Table 9.24 DDS-RPC

**Spec:** §9.6.5, p. 197, Table 9.24 — "DDS-RPC 1.0 — 0x0080, 0x0081, 0x0082,
0x0083".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SERVICE_INSTANCE_NAME`,
`RELATED_ENTITY_GUID`, `TOPIC_ALIASES`, `RELATED_SAMPLE_IDENTITY`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/rtps/src/publication_data.rs::tests::rpc_pid_constants_in_emitted_bytes`

**Status:** `done`

### §9.6.6 Table 9.25 PID_PERSISTENCE (0x0003)

**Spec:** §9.6.6, pp. 197–198, Table 9.25 — "PID_PERSISTENCE — 0x0003 — 2.2";
"These parameters should not be used by versions of the protocol equal or
newer than the deprecated version unless they are used with the same meaning
as in versions prior to the deprecated version."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_TYPE_CHECKSUM (0x0008)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_TYPE_CHECKSUM — 0x0008 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_TYPE2_NAME (0x0009)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_TYPE2_NAME — 0x0009 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_TYPE2_CHECKSUM (0x000a)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_TYPE2_CHECKSUM — 0x000a — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_EXPECTS_ACK (0x0010)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_EXPECTS_ACK — 0x0010 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_MANAGER_KEY (0x0012)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_MANAGER_KEY — 0x0012 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_SEND_QUEUE_SIZE (0x0013)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_SEND_QUEUE_SIZE — 0x0013 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception (RTI sends it, `vendor_discovery_frames.rs`).

### §9.6.6 Table 9.25 PID_RELIABILITY_ENABLED (0x0014)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_RELIABILITY_ENABLED — 0x0014 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_VARGAPPS_SEQUENCE_NUMBER_LAST (0x0017)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_VARGAPPS_SEQUENCE_NUMBER_LAST —
0x0017 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

### §9.6.6 Table 9.25 PID_RECV_QUEUE_SIZE (0x0018)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_RECV_QUEUE_SIZE — 0x0018 — 2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception (RTI sends it, `vendor_discovery_frames.rs`).

### §9.6.6 Table 9.25 PID_RELIABILITY_OFFERED (0x0019)

**Spec:** §9.6.6, p. 198, Table 9.25 — "PID_RELIABILITY_OFFERED — 0x0019 —
2.2".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — deprecated PID; ZeroDDS does not send it and
passes over it on reception.

## §10 Serialized Payload Representation

### §10.1 Introduction

**Spec:** §10.1, p. 199 — "The representation of the serialized application
data is not part of the RTPS protocol. The RTPS protocol does not interpret the
content of the SerializedPayload."; "the content and format of the
SerializedPayload is defined in sections 10.3 to 10.5."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the requirements are in §10.2 to
§10.5.

### §10.2 SerializedPayloadHeader at the start of every SerializedPayload

**Spec:** §10.2, p. 199 — "All SerializedPayload shall start with the
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

### §10.2 SerializedPayloadHeader: structure

**Spec:** §10.2, p. 199 — "typedef octet RepresentationIdentifier[2]; typedef
octet RepresentationOptions[2]; struct SerializedPayloadHeader {
RepresentationIdentifier representation_identifier; RepresentationOptions
representation_options; };"

**Repo:** `crates/dcps/src/runtime.rs` (`user_encap_body`,
`encap_representation`, `encap_big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr2_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_rejects_short`

**Status:** `done`

### §10.2 RepresentationOptions in the context of the RepresentationIdentifier

**Spec:** §10.2, p. 199 — "The RepresentationOptions shall be interpreted in
the context of the RepresentationIdentifier, such that each
RepresentationIdentifier may define the representation_options that it
requires."

**Repo:** `crates/dcps/src/runtime.rs` (`user_encap_body`: the low two bits as
the count of appended padding octets per DDS-XTypes 1.3 §7.6.3.1.2)

**Tests:** `crates/dcps/src/runtime.rs::tests::strip_user_encap_drops_the_padding_the_options_name`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_refuses_padding_the_body_cannot_hold`

**Status:** `done`

### §10.2 CDR stream starts after representation_options

**Spec:** §10.2, p. 199 — "For alignment purposes, the CDR stream is logically
reset at the position that follows the representation_options. Therefore,
there should be no initial padding before the serialized data is added to the
CDR stream."

**Repo:** `crates/dcps/src/runtime.rs` (`write_user_sample_pooled`: header and
a separately encoded body; `user_encap_body`: body from octet 4)

**Tests:** `crates/dcps/src/interop.rs::tests::u64_type_xcdr1_and_xcdr2_alignment_differs`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`

**Status:** `done`

### §10.2 Footnote 5: earlier versions

**Spec:** §10.2, p. 199, footnote 5 — "Versions of the RTPS specification
previous to version 2.4 did not clearly state where the CDR stream was reset
for alignment purposes. Therefore implementations may need to take into account
the vendor and protocol version when interpreting the Serialized Data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a note without an obligation.

### §10.3 Discovery built-in endpoints use Table 10.1

**Spec:** §10.3, p. 200 — "The SerializedPayload for the data messages
associated with built-in discovery endpoints shall use the
RepresentationIdentifier values and formats defined in Table 10.1 below."

**Repo:** `crates/rtps/src/participant_data.rs`,
`crates/rtps/src/publication_data.rs`, `crates/rtps/src/subscription_data.rs`,
`crates/rtps/src/topic_data.rs` (encoding with PL_CDR_LE or PL_CDR_BE, other
identifiers refused on reading)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_decode_rejects_unknown_encapsulation`,
`crates/rtps/src/publication_data.rs::tests::decode_rejects_unknown_encapsulation`

**Status:** `done`

### §10.3 Table 10.1 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.3, p. 200, Table 10.1 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11). Both the parameter list and its parameters are encapsulated using
OMG CDR Big Endian."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_be`,
`from_pl_cdr_le`); `crates/rtps/src/publication_data.rs`,
`crates/rtps/src/subscription_data.rs` (reading on `[0x00, 0x02]`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::to_pl_cdr_be_is_big_endian_roundtrips_and_omits_credential_tokens`,
`crates/rtps/src/publication_data.rs::tests::locator_params_of_a_big_endian_list_are_read`

**Status:** `done`

### §10.3 Table 10.1 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.3, p. 200, Table 10.1 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11). Both the parameter list and its parameters are encapsulated using
OMG CDR Little Endian."

**Repo:** `crates/rtps/src/participant_data.rs` (`ENCAPSULATION_PL_CDR_LE`,
`to_pl_cdr_le`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`,
`crates/discovery/tests/vendor_discovery_frames.rs::cyclone_announcements_read_and_write_back`

**Status:** `done`

### §10.3 representation_options: the sender sets zero

**Spec:** §10.3, p. 200 — "The current version of the protocol does not use
the representation_options: The sender shall set the representation_options to
zero."

**Repo:** `crates/rtps/src/participant_data.rs` (`to_pl_cdr_le`,
`to_pl_cdr_be`: options `0x0000`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_first_4_bytes_are_pl_cdr_le_encapsulation`

**Status:** `done`

### §10.3 representation_options: the receiver ignores them

**Spec:** §10.3, p. 200 — "The receiver shall ignore the value of the
representation_options."

**Repo:** `crates/rtps/src/participant_data.rs`, `publication_data.rs`, `subscription_data.rs` (`from_pl_cdr_le`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::the_receiver_ignores_the_representation_options`,
`crates/rtps/src/participant_data.rs::tests::the_receiver_ignores_the_representation_options_of_a_participant`,
`crates/rtps/src/publication_data.rs::tests::the_receiver_ignores_the_representation_options_of_a_publication`

**Status:** `done` — the decoders read an announcement with `representation_options` ≠ 0 like one with 0; the sender writes 0.

### §10.4 Other built-in endpoints use Table 10.2

**Spec:** §10.4, p. 200 — "The SerializedPayload for the data messages
associated with built-in endpoints other than discovery built-in endpoints
shall use one of the RepresentationIdentifier values and formats defined in
Table 10.2 below."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`to_cdr`: CDR_LE or
CDR_BE); `crates/security/src/generic_message.rs` (CDR_LE)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`,
`crates/security-runtime/src/builtin_topics.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** `done` — ParticipantMessageData and the messages of the
DDS-Security endpoints use CDR_LE. The TypeLookup endpoints use CDR2_LE, the
representation their own definition names (DDS-XTypes 1.4 Beta 1
§7.6.3.3.3), and CDR_LE under the setting `xcdr1` (item "TypeLookup endpoints
with CDR2_LE").

### §10.4 TypeLookup endpoints with CDR2_LE

**Spec:** §10.4, p. 200 — "shall use one of the RepresentationIdentifier values
and formats defined in Table 10.2 below." Table 10.2 names CDR_BE, CDR_LE,
PL_CDR_BE and PL_CDR_LE.

**Repo:** `crates/types/src/type_lookup.rs` (`TypeLookupEncoding`,
`TypeLookupEncapsulation`, `to_payload`), `crates/types/src/framing.rs`
(`EncodingVersion`, `encode_optional`, `encode_mutable`),
`crates/dcps/src/runtime.rs` (`send_type_lookup_request_datagram`,
`send_type_lookup_reply`, `dispatch_type_lookup_datagram`,
`RuntimeConfig::type_lookup_encoding`),
`crates/java-omgdds/java/src/main/java/org/zerodds/typelookup/TypeLookupService.java`
(`Encoding`, `Encapsulation`), `.../generated/ZdCodec.java` (`ZdCodec.Xcdr1`)

**Tests:** `crates/types/tests/type_lookup_xcdr1.rs` (eight tests, among them
`a_get_types_request_is_cdr_le_with_its_parameters_as_a_parameter_list` and
`a_get_types_reply_carries_its_object_in_the_encoding_version_1`),
`crates/dcps/tests/type_lookup_e2e.rs::{server_handles_inbound_get_types_request_via_dispatch,
a_get_types_request_in_cdr_le_is_answered, the_setting_xcdr1_answers_in_cdr_le,
a_request_is_written_in_the_configured_encoding}`,
`crates/dcps/tests/typelookup_resolution_e2e.rs::a_complete_object_crosses_between_an_xcdr1_and_an_xcdr2_participant`,
`crates/java-omgdds/java/src/test/java/org/zerodds/typelookup/TypeLookupEncodingTest.java`
(five tests), `crates/dcps/tests/java_typelookup_live.rs` (Java asks the Rust
runtime and the Rust runtime asks Java, over RTPS between two processes, both
with the default XCDR2)

**Status:** `done` — request and reply of the TypeLookup service go out with
`00 07` (CDR2_LE) and the XCDR2 encoding by default: the FINAL envelope
without a DHEADER, the appendable unions with one, the MUTABLE `_In`/`_Out`
parameters with a DHEADER and an EMHEADER per member. DDS-XTypes 1.4 Beta 1
(ptc/2026-02-06) §7.6.3.3.3: "The Data Representation used by these endpoints
shall be Extended CDR Representation (encoding version 2)", the resolution of
OMG issue DDSXTY14-29. §10.4 leaves the format to "the definition of each of
those builtin Endpoints", and that definition now names XCDR2; CDR2_LE is not
in Table 10.2, and DDSI-RTPS 2.5 is the current version. The form of Table
10.2 is the named setting `wire.type_lookup_encoding = xcdr1` (env
`ZERODDS_WIRE_TYPE_LOOKUP_ENCODING`, zerodds.yaml(5)): `00 01` (CDR_LE) with
XCDR1, the outer type without a header, the MUTABLE parameters as PL_CDR1
lists with `PID_EXTENDED` (`0x7F01`, Table 34) for the `@hashid` ids above
`0x3F00` and `PID_LIST_END` (`0x7F02`), the `@optional` members of the
TypeObjects behind a parameter header instead of a presence octet (XTypes 1.3
§7.4.1.1.5.2, §7.4.1.2). It is the reading of XTypes 1.3, which names no
representation for these endpoints: no DataRepresentation (default XCDR,
§7.6.3.1.1), an unannotated outer type is APPENDABLE (§7.3.1.2.1.8), Table 60
gives CDR_LE. A received
sample is read in the encoding its identifier names (CDR_BE/CDR_LE → XCDR1,
CDR2_BE/CDR2_LE → XCDR2) under either setting; any other identifier is
dropped and reported as `typelookup.sample.rejected`. The sample ends on a
four-octet boundary and the options count the padding (XTypes 1.3 §7.6.3.1.2,
`5e31120999e`). The Java runtime (`crates/java-omgdds`) has the same default
and the same setting (property `zerodds.wire.type_lookup_encoding` or the
same environment variable, `48555aad95f`); its XCDR1 codec is generated from
the same unmodified IDL and writes the octets the Rust runtime writes
(`TypeLookupEncodingTest::aGetTypesRequestIsWrittenInTheEncodingVersion1`,
`theRustReplyReadsAndIsWrittenBackOctetForOctet`). The expected XCDR1 octets
of the tests are derived from the clauses, not a vendor capture: no measured
vendor sends this form. Measured (Linux test host,
`internal/release/evidence/rc7-typelookup-encapsulation`, `ce232e91245`), in
each direction with a type the peer does not know:

| Vendor | sends | reads a CDR_LE request | reads a CDR_LE reply |
|---|---|---|---|
| Cyclone DDS 11.0.1 | CDR2_LE, options = padding | no (`deserialization failed`) | no (`deserialization failed`) |
| Fast DDS 3.6.1 | CDR2_LE (replies) | yes, answers | not measured: Fast DDS asked in no cell |
| OpenDDS 3.34.0 | CDR2_LE, options `0000` | no (`failed to deserialize type lookup request`) | no (`failed to deserialize type lookup reply`) |
| RTI Connext 7.7.0 | CDR2_LE | no (`no deserialization program … CDR_LE`) | no (the same message) |

InterCom DDS 4.3.1 sends CDR2_LE as well (earlier capture,
`an_intercom_get_types_request_padded_without_its_options_is_read`). OpenDDS
writes the options `0000` also on a sample that ends three octets short of a
four-octet offset; XTypes 1.3 §7.6.3.1.2 asks for the padding count there.
ZeroDDS reads it without a guess: the envelope's structure says where it
ends, and zero octets up to the next four-octet offset are the padding behind
a submessage (DDSI-RTPS 2.5 §9.4.1), with and without them
(`runtime::tests::an_opendds_get_types_request_without_its_padding_count_is_read`
with the captured request, `0e77cf5e96b`). The
runners set nothing for it; every pair runs the default. With CDR2_LE the
Cyclone cell with the appended member delivers its samples
(`cyc-answers-x2`: 38), with CDR_LE none (`cyc-answers-x1`).

### §10.4 Table 10.2 CDR_BE {0x00, 0x00}

**Spec:** §10.4, p. 200, Table 10.2 — "CDR_BE — {0x00, 0x00} — Classic CDR
representation with Big Endian encoding."

**Repo:** `crates/rtps/src/participant_message_data.rs` (`to_cdr(false)`,
`from_cdr`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_be`

**Status:** `done`

### §10.4 Table 10.2 CDR_LE {0x00, 0x01}

**Spec:** §10.4, p. 200, Table 10.2 — "CDR_LE — {0x00, 0x01} — Classic CDR
representation with Little Endian encoding."

**Repo:** `crates/rtps/src/participant_message_data.rs`
(`ENCAPSULATION_CDR_LE`); `crates/security/src/generic_message.rs`
(`ENCAPSULATION_CDR_LE`)

**Tests:** `crates/rtps/src/participant_message_data.rs::tests::participant_message_data_roundtrip_le`,
`crates/security-runtime/src/builtin_topics.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** `done`

### §10.4 Table 10.2 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.4, p. 200, Table 10.2 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11) with Big Endian encoding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — one of the permitted identifiers; ZeroDDS
uses CDR_LE and CDR_BE for these endpoints.

### §10.4 Table 10.2 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.4, p. 200, Table 10.2 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11) with Little Endian encoding."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — one of the permitted identifiers; ZeroDDS
uses CDR_LE and CDR_BE for these endpoints.

### §10.4 Definition per built-in endpoint

**Spec:** §10.4, p. 200 — "The definition of each of those builtin Endpoints
should indicate the serialized data format and RepresentationIdentifier used."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — addressed to the specifications that define
built-in endpoints.

### §10.5 User topics use the representations of DDS-XTypes §7.4

**Spec:** §10.5, p. 200 — "The SerializedPayload for the data messages
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

**Status:** `done` — XCDR1, XCDR2 and XML are implemented. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 CDR_BE {0x00, 0x00}

**Spec:** §10.5, p. 201, Table 10.3 — "CDR_BE — {0x00, 0x00} — Classic CDR
representation with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap` with
`big_endian`, `encap_big_endian`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/src/subscriber.rs::tests::cache_materialize_honors_xcdr1_encapsulation_le_and_be`

**Status:** `done`

### §10.5 Table 10.3 CDR_LE {0x00, 0x01}

**Spec:** §10.5, p. 201, Table 10.3 — "CDR_LE — {0x00, 0x01} — Classic CDR
representation with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`USER_PAYLOAD_ENCAP`,
`user_payload_encap`)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_is_cdr_le`,
`crates/dcps/src/runtime.rs::tests::strip_user_encap_xcdr1_le`

**Status:** `done`

### §10.5 Table 10.3 PL_CDR_BE {0x00, 0x02}

**Spec:** §10.5, p. 201, Table 10.3 — "PL_CDR_BE — {0x00, 0x02} — ParameterList
(9.4.2.11) with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`: XCDR1 and
MUTABLE)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done`

### §10.5 Table 10.3 PL_CDR_LE {0x00, 0x03}

**Spec:** §10.5, p. 201, Table 10.3 — "PL_CDR_LE — {0x00, 0x03} — ParameterList
(9.4.2.11) with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`: XCDR1 and
MUTABLE)

**Tests:** `crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`

**Status:** `done`

### §10.5 Table 10.3 CDR2_BE {0x00, 0x10}

**Spec:** §10.5, p. 201, Table 10.3 — "CDR2_BE — {0x00, 0x10} — Plain CDR
representation (version2) with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — CDR2_BE: big endian, even identifier. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 CDR2_LE {0x00, 0x11}

**Spec:** §10.5, p. 201, Table 10.3 — "CDR2_LE — {0x00, 0x11} — Plain CDR
representation (version2) with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — CDR2_LE. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 PL_CDR2_BE {0x00, 0x12}

**Spec:** §10.5, p. 201, Table 10.3 — "PL_CDR2_BE — {0x00, 0x12} — Extended
CDR representation (version2) for MUTABLE types with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — PL_CDR2_BE. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 PL_CDR2_LE {0x00, 0x13}

**Spec:** §10.5, p. 201, Table 10.3 — "PL_CDR2_LE — {0x00, 0x13} — Extended
CDR representation (version2) for MUTABLE types with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — PL_CDR2_LE. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 D_CDR_BE {0x00, 0x14}

**Spec:** §10.5, p. 201, Table 10.3 — "D_CDR_BE — {0x00, 0x14} — Extended CDR
representation (version2) for APPENDABLE types with Big Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — D_CDR_BE. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 D_CDR_LE {0x00, 0x15}

**Spec:** §10.5, p. 201, Table 10.3 — "D_CDR_LE — {0x00, 0x15} — Extended CDR
representation (version2) for APPENDABLE types with Little Endian encoding."

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — D_CDR_LE. Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3: identifiers 0x0010 to 0x0015

**Spec:** §10.5, p. 201, Table 10.3 — "CDR2_BE {0x00, 0x10}", "CDR2_LE
{0x00, 0x11}", "PL_CDR2_BE {0x00, 0x12}", "PL_CDR2_LE {0x00, 0x13}", "D_CDR_BE
{0x00, 0x14}", "D_CDR_LE {0x00, 0x15}".

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `encap_in_table`, `user_encap_body`); `wire.encapsulation_ids`

**Tests:** `crates/dcps/src/runtime.rs::tests::encap_in_table_maps_table_60_onto_table_39`,
`crates/dcps/src/runtime.rs::tests::a_writer_stamps_the_identifiers_of_table_39_when_the_runtime_is_set_to_them`,
`crates/dcps/src/runtime.rs::tests::the_encapsulation_identifiers_of_table_39_are_read`,
`crates/dcps/src/runtime.rs::tests::user_payload_encap_maps_repr_and_extensibility`,
`crates/dcps/tests/runtime_config_layering.rs::the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment`

**Status:** `done` — The six identifiers 0x0010 to 0x0015 (even big endian, odd little endian). Both sets of values are read. The default sent is DDS-XTypes 1.3 Table 60, to which the header of Table 10.3 refers and which Cyclone DDS 11.0.1, Fast DDS, OpenDDS 3.34.0 and RTI 7.7.0 send (measured in `tools/wire-capture/frames/*/*.xcdr2.frames`). With `wire.encapsulation_ids: table-39` (environment `ZERODDS_WIRE_ENCAPSULATION_IDS`) ZeroDDS sends the identifiers of Table 39 (§7.4.3.4), whose XCDR2 values are those of Table 10.3 (XML: {0x01, 0x00}). Expected from the vendors: a peer that reads Table 60 only cannot read data with 0x0010 to 0x0015; the setting is for peers that send and expect Table 10.3. Not measured: a run against the vendors with the setting.

### §10.5 Table 10.3 XML {0x00, 0x04}

**Spec:** §10.5, p. 201, Table 10.3 — "XML — {0x00, 0x04} — See DDS-XTypes [3]
clause 7.4.4."

**Repo:** `crates/dcps/src/runtime.rs` (`XML_ENCAP`, `user_payload_encap`, `xml_wire_body`, `xml_sample_as_cdr`);
`crates/dcps/src/xml_data.rs`; `crates/dcps/src/publisher.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_xml_sample_reaches_the_reader_as_its_cdr_body`,
`crates/dcps/src/runtime.rs::tests::an_xml_sample_that_is_no_document_of_the_type_is_refused`,
`crates/dcps/src/runtime.rs::tests::an_xml_dispose_carries_the_instance_key`,
`crates/dcps/src/runtime.rs::tests::an_xml_writer_sends_the_document_of_the_sample`,
`crates/dcps/src/runtime.rs::tests::a_writer_registered_with_xml_first_is_accepted`

**Status:** `done` — a writer whose first DataRepresentation is XML sends the document of the sample under {0x00, 0x04}; a reader reads the sample against its registered type and hands on the XCDR2 body. Not measured: a vendor that sends or reads XML.

### §10.5 Legacy DDS implementations without DDS-XTypes

**Spec:** §10.5, p. 201 — "Legacy DDS implementations that are not compliant
with DDS-XTYPES should minimally support the RepresentationIdentifier values
CDR_BE and CDR_LE and the type system elements specified in clause F1".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — addressed to implementations without
DDS-XTypes; ZeroDDS supports CDR_BE and CDR_LE (§10.5 Table 10.3).

### §10.6 Example for built-in endpoint data

**Spec:** §10.6, pp. 201–203 — "Following is the SerializedPayload element used
by the SEDPbuiltinSubscriptionsWriter to declare a DataReader." with layout and
octets.

**Repo:** `crates/rtps/src/subscription_data.rs` (`SubscriptionBuiltinTopicData::from_pl_cdr_le`, `to_pl_cdr_le`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::the_example_of_10_6_is_read`,
`crates/rtps/src/subscription_data.rs::tests::the_parameters_of_the_example_of_10_6_are_written_as_the_spec_prints_them`

**Status:** `done` — the 84 octets of the spec are read as the DataReader they describe; the encoder writes the five parameters of the example with the same octets (the spec leaves the default values out, ZeroDDS writes them in addition).

### §10.7 Example for user-defined topic data

**Spec:** §10.7, pp. 203–204 — "Following is the SerializedPayload element used
by an application DataWriter to send Data on the Topic “Square” with type
“ShapeType”" with layout and octets.

**Repo:** `crates/dcps/src/runtime.rs` (`user_payload_encap`, `strip_user_encap`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_example_of_10_7_has_the_header_and_body_of_the_spec`

**Status:** `done` — the CDR_LE header ({0x00, 0x01}, options 0) and the 24-octet body of the example ("BLUE", 34, 100, 24) are read as the spec prints them; the header for an XCDR1 final writer is the one of the spec.

## Annex A References

### Annex A References

**Spec:** Annex A, p. 205 — "[1] DDS-SECURITY: DDS Security version 1.1
https://www.omg.org/spec/DDS-SECURITY".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — list of references.

---

## Audit-Status

978 done / 3 partial / 2 open / 142 n/a (informative) / 0 n/a (rejected).

Test run: to be added on the Linux test host.

Open items and decision records: `ddsi-rtps-2.5.open.en.md`.
