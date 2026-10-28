# Changelog — `zerodds-rtps`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The GAPs of a `ReliableWriter` in a group (GROUP-scope Publisher) carry `gapStartGSN` and `gapEndGSN` with the GroupInfoFlag (DDSI-RTPS 2.5 §8.3.8.4); the writer keeps the group sequence number of up to 4096 changes past their removal from the cache.

### Added (DDS-Security 1.2, §7 package)

- The four secure TypeLookup EntityIds (Table 9), `PID_DATA_TAGS` (0x1003),
  `PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT` (0x1007) with its Table 12 bits,
  `properties` and `data_tags` on the publication and subscription data,
  `ParticipantSecurityAlgorithmInfo` and `EndpointSecurityAlgorithmInfo`, the
  endpoint `supported_mask` (not serialized).


### Added

- `PublicationBuiltinTopicData` and `SubscriptionBuiltinTopicData` carry
  `properties` (`PID_PROPERTY_LIST` 0x0059, DDS-Security 1.2 §7.3.21,
  §7.5.1.7, §7.5.1.8): the `propagate = TRUE` entries of the endpoint's
  `PropertyQosPolicy`, string and binary. A list that does not decode rejects
  the sample. Existing struct literals need `properties`.

### Fixed

- `ParticipantSecurityInfo::optional_attributes`
  (`ParticipantSecurityAttributesMaskExt`, DDS-Security 1.2 §7.3.23): read from
  the third member (12 octets), written only when `is_set` is not 0; the
  bits `IS_KEY_REVISION_ENABLED`, `IS_RTPS_PSK_PROTECTED`,
  `IS_RTPS_PSK_ENCRYPTED` and `opt_attrs`.
- Algorithm infos (DDS-Security 1.2 §7.3.10 to §7.3.15): the spec defaults of
  `ParticipantSecurityDigitalSignatureAlgorithmInfo`,
  `...KeyEstablishmentAlgorithmInfo` and `...SymmetricCipherAlgorithmInfo`
  (the required masks were the supported ones, and AES128); the
  compatibility-mode bit (`CRYPTO_ALGORITHM_COMPATIBILITY_MODE`),
  `check_crypto_algorithm_compatibility` and `compatible_with` of each info;
  `Publication`/`SubscriptionBuiltinTopicData::sym_cipher_algo_info` for
  `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO` 0x1013.
- `ParticipantSecurityInfo::is_compatible_with` and
  `EndpointSecurityInfo::is_compatible_with` (§7.3.23, §7.3.24: equal masks,
  a mask without `IS_VALID` on either side is not compared).

- `WirePropertyList` carries the `binary_value` of the `PropertyQosPolicy`
  (`binary_entries`); `PID_PROPERTY_LIST` wrote an empty binary sequence and
  discarded the received one (DDS-Security 1.2 §7.3.21, §7.5.1.4).

- A write that a full KEEP_ALL history refuses gives its sequence number
  back; the next change follows the last written one, with no hole.

### Changed

- BREAKING (timing): `DEFAULT_NACK_RESPONSE_DELAY` is 200 ms and `DEFAULT_HEARTBEAT_RESPONSE_DELAY` 500 ms, the defaults of DDSI-RTPS 2.5 §8.4.7.1 and §8.4.10.1 (they were 0). A NACK is answered after 200 ms, a HEARTBEAT after 500 ms; `set_nack_response_delay` and `ReliableReaderConfig::heartbeat_response_delay` set 0 again. The built-in SEDP readers take the same default.
- BREAKING: `HeTimestamp::seconds` is `u32` (DDSI-RTPS 2.5 §9.3.2.1 `unsigned long seconds`), `TIME_INVALID` has seconds 0xFFFFFFFF; `ReceiverState::note_clock_skew` takes `u32`. The wire octets are unchanged.
- BREAKING: `LocatorKind::Tcpv4` and `Tcpv6` have the wire values `0x01F0_0004` and `0x01F0_0005` (vendor range, §9.3.2.4); they were 4 and 8. `set_tcp_kinds_fast_dds(true)` writes 4 and 8, the values of Fast DDS; reading accepts both. A ZeroDDS TCP peer of an older build does not read the new values.
- GAPs of `ReliableWriter` carry `filteredCount` (1 for a change the cache holds and the reader is not sent, 0 for a number the cache no longer holds), as the pseudo-code of Tables 8.65 and 8.66 T4 has it. The FilteredCountFlag is set and the submessage is 8 octets longer.
- `ReliableWriter` puts as many fragments into one DATA_FRAG as the message to the reader carries (§8.4.14.1), also in the answer to a NACK_FRAG; a DATA_FRAG holds its fixed octets, the in-line QoS and the fragment in 65535 octets, which bounds the fragment size.
- A re-assembled sample is interpreted as a DATA: F = 1 with D = U = 0 in the first fragment's in-line QoS is a filtered sample also without the key flag, and a key-only ALIVE sample is a registration (§8.3.8.3, §9.6.4.9).

### Added

- `type_id_v1` (`TypeIdV1`, `PID` 0x0069) and `type_object_v1` (`TypeObjectV1`, `PID` 0x0072) on
  `TopicBuiltinTopicData`, `PublicationBuiltinTopicData` and `SubscriptionBuiltinTopicData` (DDS-XTypes 1.3 Annex D,
  §7.6.3.2.2): written when set, read back; a parameter that does not decode is left out. An endpoint that announces a
  `TypeObjectV1` and no `PID_TYPE_INFORMATION` has the Complete hash of the type it describes as its
  `type_identifier`. The three structures have two more fields.
- `WriterProxy::change_status`, `ChangeFromWriterStatus` and `ReliableReader::change_status`: the status of each change with the NOT_AVAILABLE sub-kinds kept per change (DDSI-RTPS 2.5 §8.4.6 Table 8.52, §8.4.10.5 Table 8.74); `lost_changes_update` enters NA_REMOVED and counts it; a sample with the FilteredFlag enters NA_FILTERED.
- `remote_group_entity_id` on `ReaderProxy` and `WriterProxy` (Tables 8.61, 8.72).
- `ReliableStatelessWriter::unsent_changes_reset` (Table 8.56).
- `ChangeFilter` and `ReaderProxy::with_filter`: `DDS_FILTER` of a reader; a change that fails it is sent as a GAP with `filteredCount` 1 (Tables 8.65, 8.66 T4).
- `ReliableWriter::try_new`, `MAX_FRAGMENT_SIZE` (65535), `fragment_size_for_message_size`; `ReliableWriter::new` panics for a fragment size above the bound.
- `ReaderProxy::next_requested_fragment_run_after`.
- `datagram::set_rejection_hook`, `Rejection`, `RejectionKind`: a function told of every
  message refused, every rest invalidated and every must-understand skip, with the
  submessage id, its offset, the sender's VendorId and the error (the cases the three
  counters count). `datagram::ParsedDatagram` is unchanged.
- `inline_qos::InlineQosOverride`: the OWNERSHIP_STRENGTH and LIFESPAN a DATA
  carries in-line, which override its writer's (§8.3.8.2), in
  `DeliveredSample::inline_qos_override`. `submessages::skipped_unknown_locators`.
- `PublicationBuiltinTopicData::data_max_size_serialized` (`PID_TYPE_MAX_SIZE_SERIALIZED`),
  `entity_name` on the participant, publication and subscription data
  (`PID_ENTITY_NAME`), and `ReliableReader::set_writer_data_max_size` /
  `oversize_payload_count`: the reader refuses a payload beyond the bound its
  writer announced (DDSI-RTPS 2.5 Table 8.74). BREAKING: the three data structs
  have new fields.
- `datagram::ReceiveContext`, `ReceptionClock`, `ParsedDatagram::receiver_walk_in` and
  `receiver_walk_from`: a receive path walks a datagram with its source and the
  reception clock (DDSI-RTPS 2.5 §8.3.4).
- `wire_types::PortMapping`: PB, DG, PG, d0 to d3 as parameters of the port
  formulas (§9.6.2.3); the free port functions use `PortMapping::SPEC`.

### Fixed

- A HeaderExtension whose `messageLength` differs from the length of the message,
  counted from the RTPS Header, is refused (§8.3.7.5); one that is not directly
  after the header invalidates the rest of the message and leaves the submessages
  before it (§8.3.7.3, §8.3.4.1 rule 6).
- A locator of an unknown kind in an INFO_REPLY is left out and counted instead of
  invalidating the submessage (§9.3.2.4).
- On a path that carries a larger message than the smallest transport
  (`ReaderProxy::set_message_limit`), a DATA_FRAG holds as many consecutive
  fragments as the message takes (§8.4.14.1).
- `ReliableStatelessWriter` follows DDSI-RTPS 2.5 §8.4.8.2: a `ReaderLocator` per
  locator (`reader_locator_add`, `reader_locator_remove`), the push and repair
  machines of Table 8.64, a GAP with `filteredCount` 0 before DATA after a hole
  and for a requested change that left the cache, HEARTBEAT with the FinalFlag,
  `nackResponseDelay`, and replies at the reply locators of the receiver state.
  BREAKING: `set_locators`, `lowest_unacked` and the one-argument
  `handle_acknack` are gone; the type was used by nothing else.
- `ReceiverState` starts from the values of DDSI-RTPS 2.5 Table 8.17: `timestamp`
  TIME_INVALID, reply-locator lists `{ LOCATOR_INVALID }` (InfoSource) or the
  transport values (`ParsedDatagram::receiver_walk_from`); an InfoReply without
  the M flag empties the multicast list; `rtps_reception_timestamp`,
  `note_reception`; an InfoTimestamp is corrected by a detected clock skew.
- A DATA with the FilteredFlag (`PID_STATUS_INFO` F = 1, D = U = 0, §9.6.4.9) is
  known but not delivered, and counted (`ReliableReader::filtered_sample_count`).
  It was read as an ALIVE sample, or as a registration or a coherent-set end.
- `ReliableReader::handle_gap` sorts the changes of a GAP into NA_FILTERED,
  NA_REMOVED and NA_UNSPECIFIED by `filteredCount` (§8.4.10.4); counters:
  `ReliableReader::not_available_count`, `WriterProxy::not_available_count`.
- A `ReliableWriter` sends no HEARTBEAT (periodic, piggyback or in answer to an
  ACKNACK with the final flag clear) to a reader proxy that is not reliable
  (DDSI-RTPS 2.5 §8.4.9.1); it sent them.
- **BREAKING (wire):** `LocatorKind::Shm`, `Uds` and `Tsn` are `0x01F0_0001` to
  `0x01F0_0003`, inside the vendor-specific range of DDSI-RTPS 2.5 §9.3.2.4;
  they were `0x8100_0000` to `0x8100_0002`, negative as a `long` and outside every
  range. A build before this change and a build after it do not read each
  other's SHM, UDS and TSN locators: the decoder of a participant or endpoint
  announcement skips a locator whose kind it does not know (it is not an error),
  and `LocatorKind::from_i32` refuses it (an INFO_REPLY with such a locator is
  invalid). What follows: the locators of the other kinds in the same
  announcement are used, normally the UDP ones, so peers that announce UDP
  beside SHM, UDS or TSN talk over UDP, also on one host. A peer whose only
  locator is of one of the three kinds is not reachable by the other build.
  Same-host shared memory through the same-host channel (`transport.same_host_shm`,
  `PID_SHM_LOCATOR`) does not use the locator kind and is not affected.
  Migration: move all participants of a deployment that announces SHM, UDS or
  TSN locators to the new build together, or let them announce UDP as well.
- `PID_TOPIC_NAME`, `PID_TYPE_NAME`, `PID_DOMAIN_TAG` and the three names of a
  `ContentFilterProperty_t` are `string<256>` (DDSI-RTPS 2.5 Table 9.18,
  §9.3.2.1): a longer one is refused when encoding and when reading
  (`publication_data::STRING_256_MAX`).
- The `messageChecksum` of a HeaderExtension is big endian also in a
  little-endian HeaderExtension (DDSI-RTPS 2.5 Table 9.8) and is the checksum
  of the entire message with the field set to zero (§8.3.7.4):
  `HeaderExtension::message_checksum`, `seal_message`, `verify_message`. The
  decoder refuses a message whose checksum does not match and counts it in
  `datagram::refused_messages`.
- A `ReliableReader` sends ACKNACK and NACK_FRAG only as the response to a
  HEARTBEAT or HEARTBEAT_FRAG, apart from the ACKNACK at the first discovery
  of a writer (DDSI-RTPS 2.5 §8.4.2.3, §8.4.14.1). A DATA_FRAG that leaves
  fragments missing, and the renewed announcement of a writer that is
  already known (`add_writer_proxy`), no longer arm one. A lost fragment is
  asked for at the writer's next HEARTBEAT.
- The decoders refuse what the submessage "Validity" clauses of DDSI-RTPS 2.5
  call invalid, and with them the rest of the message (§8.3.4.1 rule 6): a
  DATA or DATA_FRAG with `writerSN` below 1 or UNKNOWN (§8.3.8.2, §8.3.8.3), a
  DATA with D and K both set (§9.4.5.3), a DATA_FRAG with `fragmentSize` above
  `dataSize`, `fragmentStartingNum` outside the fragments or more serialized
  data than `fragmentsInSubmessage * fragmentSize`, a HEARTBEAT with
  `firstSN` below 1, `lastSN` negative or `lastSN < firstSN - 1` (§8.3.8.6), a
  GAP with `gapStart` below 1 (§8.3.8.4), a HEARTBEAT_FRAG or NACK_FRAG with
  `writerSN` or `lastFragmentNum` below 1 (§8.3.8.7, §8.3.8.12), a
  `SequenceNumberSet` or `FragmentNumberSet` with `bitmapBase` below 1
  (§9.4.2.6, §9.4.2.8) and an INFO_REPLY locator list of more than 8
  locators (§9.4.2.10).
- A vendor-specific ParameterId (0x8000) with the must-understand bit
  (0x4000) that the receiver does not recognize is an error
  (DDSI-RTPS 2.5 Table 9.6): the DATA or DATA_FRAG carrying it is skipped, a
  HeaderExtension carrying it ends the message. It was passed over whatever
  its sender. `validate_must_understand_in_data_pipeline` takes the sender's
  `VendorId`; `is_known_vendor_pid` says which ones are recognized.
- A `ReliableWriter` keeps one fragment size for all its readers
  (DDSI-RTPS 2.5 §8.4.14.1). `set_fragmentation` and
  `LOOPBACK_FRAGMENT_SIZE` are gone; a reader on a path with a larger message
  size (`ReaderProxy::set_message_limit`) gets a sample that fits as one
  DATA, the others get DATA_FRAGs of the writer's size. The fragment size no
  longer depends on whether a reader expects in-line QoS.
- A subscription announcement without PID 0x0074 is read as
  DISALLOW_TYPE_COERCION (`TypeConsistencyEnforcement::not_announced`), as
  DDS-XTypes 1.3 §7.6.3.4.1 requires; it was read as the default
  ALLOW_TYPE_COERCION. `SubscriptionBuiltinTopicData::type_consistency_announced`
  says whether the PID was there, so a reader that announces the same
  policy itself is told apart.
- `DataRepMatchMode::default()` is `Strict`, the clause (DDS-XTypes 1.3
  §7.6.3.1.1); it was `Tolerant`, which the runtime no longer used as its
  default.
- `ParameterList` decoding ends a list at the parameter id `0x3F02`
  (`pid::LIST_END`), with or without the must-understand bit, as well as at
  `PID_SENTINEL` (DDS-XTypes 1.3 §7.4.1.2.1, Table 34).

### Added

- `CacheChange::destination_readers` and
  `ReliableWriter::set_next_destination_readers`: the next change is for the
  readers named by GUID and a GAP for every other matched reader, on the first
  send and on every repair (as `CacheChange::destination` does by participant).
- `SubscriptionBuiltinTopicData::content_filter_unreadable`: the announcement
  had a `PID_CONTENT_FILTER_PROPERTY` that did not read (a name over the bound
  of `string<256>`).
- `ParticipantBuiltinTopicData::manual_liveliness_count` and
  `builtin_endpoint_qos` (`PID_PARTICIPANT_MANUAL_LIVELINESS_COUNT` 0x0034,
  `PID_BUILTIN_ENDPOINT_QOS` 0x0077; DDSI-RTPS 2.5 Table 9.18), and
  `participant_data::builtin_endpoint_qos`.
- `submessages::set_accept_empty_acknack_base_zero`: an ACKNACK with an empty
  `readerSNState` of `bitmapBase` 0, which OpenDDS 3.34.0 and RTI Connext 7.7.0
  send, is accepted instead of being invalid (DDSI-RTPS 2.5 §9.4.2.6). Off by
  default. `SequenceNumberSet::read_from_with` takes the choice per call.
- `datagram::refused_messages`, `invalid_message_rests` and
  `skipped_must_understand_submessages` count what the decoders refuse,
  per process.
- `ReliableReader` takes forwarded and directed changes as DDSI-RTPS 2.5
  §8.7.9 and §8.7.7 say: a change with `PID_ORIGINAL_WRITER_INFO` goes out
  as the original writer's (`DeliveredSample::original_writer`,
  `forwarded_by`), a duplicate of one it has is dropped
  (`forwarded_duplicate_count`), a forwarder it does not trust delivers as
  itself (`set_forwarder_trusted`, `untrusted_forward_count`); a reader no
  `PID_DIRECTED_WRITE` names absorbs the change as after a GAP.
  `accept_forwarded` applies the duplicate check to a change forwarded
  inside the participant.
- `PayloadOpener`, `handle_data_opening`, `handle_data_frag_opening`: the
  reader opens a protected SerializedPayload (DDS-Security 1.2
  §9.5.3.3.4.4) before it records the change as received, a reassembled
  DATA_FRAG one too; one it cannot open stays MISSING
  (`refused_payload_count`).
- `CacheChange::non_standard_payload` and
  `ReliableWriter::write_non_standard_sample_with_inline_qos`: the
  NonStandardPayloadFlag on every DATA and DATA_FRAG of the change.
- `CacheChange::destination`, `ReliableWriter::write_with_heartbeat_to`,
  `write_to`: a change for the readers of one participant; every other
  matched reader gets a GAP for its number, on the first send and on every
  repair (the implied content filter of DDS-Security 1.2 §7.5.4.4).
- `ReliableWriter::write_lifecycle_with_inline_qos`: a dispose or
  unregister with in-line QoS of its own, on the first send and on every
  repair.

- `datagram::encode_datagram` — writes a `ParsedDatagram` back out, header
  and every submessage, the inverse of `decode_datagram`. The crate could
  take a datagram apart and could not put one back together
  (`encode_data_datagram` wrote a header and a list of DATA and nothing
  else), so nothing had ever asked whether a decode was lossless.
- `datagram::SubmessageFrame` — the two framing decisions a submessage
  header carries and the decoded body. `little_endian` is §8.3.4.1's `E`
  flag, which is per submessage and which an encoder must be told by the
  message rather than by its caller; `octets_elided` is §8.3.3.2.3's
  last-submessage `octetsToNextHeader = 0`, which OpenDDS 3.34.0 writes
  on every datagram and Cyclone, Fast DDS and RTI never do.
- `DataSubmessage::data_flag` — the `D` flag of §8.3.7.2.5 Tab. 8.43. It
  was read off the wire and dropped, and `write_body` set it
  unconditionally, so a key-only DATA (`D=0, K=1`) came back as
  `D=1, K=1`, which is not a state the clause has, and a DATA carrying
  neither (`D=0, K=0`) came back claiming a sample. 83 datagrams in the
  capture sets have `D` clear — OpenDDS, Cyclone and Fast DDS all send
  them.
- `tests/vendor_frames.rs` — 2,114 RTPS datagrams six foreign stacks put
  on a socket, decoded and written back octet for octet: 1,356 from the
  corpus run and 758 from the session captures.

### Fixed

- `inline_qos::find_related_sample_identity` masks the must-understand bit
  only. A vendor-specific `0x8083` was read as DDS-RPC's `0x0083`
  (DDS-RPC 1.0 §7.8.2, DDSI-RTPS 2.5 §9.6.2.2.1).
- **A writer with nothing written heartbeats a reliable reader until it
  answers.** It sent no HEARTBEAT with an empty cache; an OpenDDS 3.34
  reader, which counts a reliable writer as matched only after a
  HEARTBEAT/ACKNACK exchange, stayed unmatched until the first sample
  (measured). Each reliable reader gets a HEARTBEAT (first 1, last 0) at
  the heartbeat period until it has answered one; a preemptive ACKNACK
  does not count (`ReaderProxy::handshake_owed`).
- **A reader proxy brought up by repair is in step again.** The send
  cursor of a proxy that joined behind the cache (SEDP's TRANSIENT_LOCAL
  writers) moved only on direct sends, so every later change to it waited
  for the next HEARTBEAT (100 ms) and the ACKNACK. An acknowledgement and
  an in-order repair move it now (`ReaderProxy::repaired`).
- **A reliable reader requests what a writer sent before the match.** DATA
  from a writer without a proxy is dropped (§8.4.10); the reader now
  remembers the highest SN each such writer announced (32 writers) and the
  new proxy's first ACKNACK requests them, instead of waiting for the
  writer's next HEARTBEAT (Fast DDS 3.6.1: 3 s).
- `build_lifecycle_datagram` sent a dispose or an unregister with the `D`
  flag set and an empty payload, so a peer was told there was a sample
  and given zero octets to read as one. `D` is clear now and `K` alone
  says the payload is the key.
- `EntityKind::from_byte` folded every `entityKind` octet Table 9.1 does
  not name into `Unknown`, and `EntityId::to_bytes` wrote `0x00` for it.
  RTI Connext 7.7.0 writes `0x82` and `0x87` for two endpoints of every
  participant, so `0x00020082` and `0x00020087` both decoded to
  `0x00020000` — one `EntityId` where the wire has two. `EntityKind`
  gained `Other(u8)` and `to_byte`, and the pairing is checked over all
  256 octets.
- `ParsedSubmessage::Unknown` dropped the body of every submessage class
  the crate does not decode. §8.3.3.2 asks a receiver to *skip* what it
  does not understand, not to erase it; anything that decoded a datagram
  and wrote it out again lost 56 octets per Fast DDS 3.6.1 datagram.

### Changed

- BREAKING: `HeTimestamp::seconds` is `u32` (DDSI-RTPS 2.5 §9.3.2.1 types
  `Time_t.seconds` unsigned); `HeTimestamp::TIME_INVALID` and `TIME_INFINITE` are
  the constants of the spec. `ReceiverState::parameters` is a `ParameterList`
  (Table 8.17: an empty list at the start), `ReceiverState::note_clock_skew` takes
  `u32` seconds, `DeliveredSample` has a field `inline_qos_override`.
- **BREAKING:** `ParsedDatagram::submessages` is `Vec<SubmessageFrame>`
  and not `Vec<ParsedSubmessage>`. `ParsedDatagram::contents()` gives the
  old element type for a caller that asks what arrived and not how it was
  framed.
- **BREAKING:** `ParsedSubmessage::Unknown` gained a `body: Vec<u8>`
  field.
- **BREAKING:** `DataSubmessage` gained `data_flag: bool`.
- **BREAKING:** `EntityKind` gained `Other(u8)` and lost its
  `#[repr(u8)]` discriminants; `entity_kind as u8` becomes
  `entity_kind.to_byte()`. The octet each variant stands for is stated
  once, in `to_byte`.
- `zerodds-cdr` → `zerodds-wire`. Eight call sites in
  `publication_data.rs` and `subscription_data.rs`:
  `BufferWriter`/`BufferReader` → `Writer`/`Reader`, `into_bytes` →
  `finish`, and two `?` gone because `zerodds-qos`'s `encode_into` is
  infallible now.

- `ParsedSubmessage::InfoDestination([u8; 12])` — typed INFO_DST destination
  GuidPrefix (PIM 8.3.8.8, PSM 9.4.5.9), used to scope directed SEDP traffic to
  one participant so a stale heartbeat cannot poison a restarted peer (#32).

### Changed

- **BREAKING:** `ParticipantBuiltinTopicData` locator fields are now
  `Vec<Locator>` (renamed to plural: `default_unicast_locators`,
  `default_multicast_locators`, `metatraffic_unicast_locators`,
  `metatraffic_multicast_locators`), not `Option<Locator>` — every field that is
  a locator list per DDSI-RTPS 2.5 §9.6.1 (#27). Decode retains all repeated
  locator PIDs in LE and BE (new `Locator::from_bytes_be`); no routing filtering
  in decode. New helpers: `primary_default_unicast_locator`,
  `primary_metatraffic_unicast_locator`, `metatraffic_or_default_unicast_locators`,
  and the public `locator_looks_routable` predicate. See the workspace CHANGELOG
  for migration notes.
- **Breaking (pre-1.0):** `ParsedSubmessage` is now `#[non_exhaustive]`.
  DDSI-RTPS defines further submessage kinds and ZeroDDS decodes more of them
  over time; a downstream crate matching `ParsedSubmessage` exhaustively must
  add a wildcard arm. This follows the same policy as the crate other growable
  public wire enums (`WireError`, `HistoryCacheError`, `FragmentAssemblerError`).
  Migration: add `_ => { /* ignore future submessage kinds */ }` to exhaustive
  matches.

### Fixed

- An INFO_DST submessage whose body is shorter than 12 bytes is rejected as a
  `WireError` instead of being normalised to GUIDPREFIX_UNKNOWN. Malformed wire
  data can no longer reset the effective destination to the local participant
  (#32).

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all 31 src files.
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Spec-anchoring cleanup: 54 Phase-X markers (`Phase-0-Scope`,
  `Phase-1-Wire`, `Phase-N D.x`, etc.) and out-of-scope lists
  cleaned up — all implemented submessages explicitly listed,
  layer boundaries clearly documented.
- README rewrite: updated test count (647), lock-free-read-path
  section without Phase-X language.

### Features

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 647 tests green; clippy clean.
- DDSI-RTPS 2.5 fully spec-conformant (K3b audit 2026-04-28: 121 done /
  0 partial / 0 open / 3 n/a).

### Public API

- Wire types: `Guid`, `EntityId`, `SequenceNumber`, `Locator`,
  `ProtocolVersion`, `VendorId`.
- Submessages: DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, ACKNACK,
  NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY.
- State machines: `BestEffortWriter`/`Reader`, `ReliableWriter`/
  `Reader`, `ReliableStatelessWriter` (for SPDP).
- History cache: `HistoryCache` with atomic stats +
  `LockFreeReadHistoryCache` with RCU snapshot.
- BuiltinTopicData: `ParticipantBuiltinTopicData`,
  `PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`.
- ParameterList (PL_CDR_LE) with all DDSI/security/XTypes PIDs.
- Fragmentation: `FragmentAssembler` with DoS caps.

### Cross-vendor interop

Wire-byte-identical against Cyclone DDS, FastDDS, RTI Connext,
OpenSplice. Cross-vendor live tests in the `discovery` crate via
`--features live-interop`.

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
