# Changelog — `zerodds-discovery`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added (DDS-Security 1.2 §9.8)

- Test `figure_23_calls_the_operations_of_both_plugins_in_order`: the
  operations of Figure 23 on both Authentication plugins, in order.

### Changed

- `StatelessMessageWriter::write` sends a message with a
  `destination_participant_key` to the reader proxy of that participant only;
  it went to every known reader before.
- `SecurityBuiltinStack` runs the state machine of DDS-Security 1.2 §9.3.2.9
  with the `ValidationResult` of the plugin: the role comes from
  `validate_remote_identity`; `VALIDATION_PENDING_RETRY` is called again after
  `set_retry_timeout` (500 ms); a handshake not complete after
  `set_authentication_timeout` (60 s) ends; a refused reply or final makes the
  cached message go out again at once (rule 3); every `AUTH_REQUEST` is passed
  to the plugin (rule 1); a participant that cannot be authenticated, whatever
  the reason, is one case (`authentication_status`, `mark_unauthenticated`).
- The stack installs an `AuthenticationListener`: `take_revoked_peers`,
  `take_identity_status_changed`, `take_handshake_failures`, `take_completed`.
- `c.pdata` comes from `set_local_participant_data` of the stack
  (`serialized_local_participant_data`); `c.perm` and `c.id` of the peer come
  from its AuthenticatedPeerCredentialToken.
- The plain SPDP beacon omits the `IdentityStatusToken`, the secure beacon
  (DCPSParticipantsSecure) carries it.
### Changed (DDS-Security 1.2, §7 package)

- The stateless reader leaves a message alone whose `destination_endpoint_guid`
  names another endpoint (§7.5.3.4); a replier answers in the NUL form of the
  request's algorithm names; the identity_status_token is in the secure SPDP
  only (§7.5.1.6).


### Added

- `SecurityBuiltinStack::peer_credential_token`, `peer_identity_handle` and
  `local_identity_handle`: the `AuthenticatedPeerCredentialToken` (`c.id`, `c.perm`,
  §10.3.2.3) and the handles for the AccessControl plugin.

### Added

- `SpdpReader::samples_in`, `sample_datagram_in` and `SedpStack::handle_datagram_in`
  take the `ReceiveContext` of the datagram.
- `sedp::topics_correspond`, `match_publications_aliased` and
  `match_subscriptions_aliased`: topic aliases count as names of the topic
  (DDS-RPC 1.0 §7.4.3).
### Fixed

- `SecurityBuiltinStack::on_participant_lost` returns the identity handle of
  the lost peer to the authentication plugin.

- The volatile secure writer (DDS-Security 1.2 Table 19, KEEP_ALL) holds a
  message back while its history is full and sends it, in order, from `tick`
  once the readers acknowledged room; only a full waiting queue
  (`VOLATILE_SECURE_PENDING_LIMIT`) is an error. `pending_count` and
  `deferred_count` report it.

- `SpdpBeacon::serialize_secure` numbers a change of the participant data
  with the next sequence number of the secure SPDP writer
  (`secure_sequence_number`); `serialize_secure_departure` is new, as are
  `SpdpReader::samples_with_channel`, `SpdpChannel` and
  `DiscoveredParticipantsCache::touch` (DDS-Security 1.2 §7.5.1.6).

- ParticipantStatelessMessage and ParticipantVolatileMessageSecure
  (DDS-Security 1.2 §7.5.3.3, §7.5.4.3, Tables 19 and 20): `message_identity`
  names the GUID of the writer, not of the participant, and its
  `sequence_number` starts at one and rises for each different message of
  that writer (it was one counter per peer, and always 1 for the crypto
  tokens). The volatile writer's history is KEEP_ALL (limit 1024) and its
  durability VOLATILE: a reader matched later gets no earlier message.

- `SpdpBeacon` resends the one data-object of the SPDP writer with its sequence
  number each period and takes the next number only when the participant data
  change (DDSI-RTPS 2.5 §8.5.3.1). `set_new_sn_each_period(true)` gives every
  announcement a new number, as Cyclone DDS and OpenDDS send them.
- TypeLookup and strongly connected components (DDS-XTypes 1.3 §7.6.3.3,
  §7.6.3.3.4.1, §7.6.3.3.4.2). `getTypes` answers the MINIMAL identifier of
  a component with its minimal members as it answers the COMPLETE one, and
  a COMPLETE answer names the minimal counterpart of each member in
  `complete_to_minimal`. The types a component's members name by hash travel
  with it. `getTypeDependencies` for a component lists those types by hash
  and the other components its members reach by their `SCCIdentifier`; the
  component's own `SCCIdentifier` is not among them.

- ParticipantVolatileMessageSecure follows the implied content filter of
  DDS-Security 1.2 §7.5.4.4: the writer sends a message with a
  `destination_participant_guid` to that participant's reader only and
  gives the others a GAP; the reader takes only messages for its
  participant and its endpoints and counts the rest
  (`VolatileSecureMessageReader::not_addressed_count`,
  `volatile_secure::is_addressed_to`). Every reader got every token
  message.
- `SecurityBuiltinStack::poll` is `poll_volatile` followed by
  `poll_stateless`, so the caller protects the volatile datagrams only.
- `TypeLookupServer::handle_get_types` answers an SCCIdentifier with every
  member of the strongly connected component, each under its own index, or
  with none (DDS-XTypes 1.3 §7.6.3.3). The identifier was dropped without a
  reply entry.

## [1.0.0-rc.1] — 2026-05-06

### RC1 audit

- License header (SPDX-Apache-2.0) on all 18 src files (root + sedp/
  + security/ + type_lookup/).
- Cargo.toml RC1 metadata (homepage, documentation, keywords, categories).
- `publish = false` → `publish = true`.
- Crate-header rewrite with full spec anchoring
  (DDSI-RTPS §8.5.3+§8.5.4, XTypes 1.3 §7.6.3.3.4, DDS-Security 1.2
  §7.4.4+§7.4.5).
- Phase-X markers cleaned up:
  - `lib.rs`: "Phase-0 scope (WP 0.7-A)" → full public-API
    description with a layer-boundary statement toward DCPS.
  - `spdp.rs`: "Phase-0 scope" removed; lease tracking documented as a
    caller-layer responsibility.
  - `sedp/mod.rs`: "Phase-1 scope (WP 1.4)" → module-contents list.
  - `sedp/stack.rs`: "Phase-2 cosmetics" → deliberate architecture
    decision explained; `silence unused in Phase 1` → commented
    as "currently not evaluated".
  - `endpoint_match.rs`: "Phase-1 fallback" → "fallback".
  - `security/mod.rs`: "Out-of-scope (C3.4-c)" → layer-boundary
    statement with spec section.
  - `type_lookup/mod.rs` + `endpoints.rs`: "TODO: wire" + "Phase-4
    followup" → layer-boundary statement toward the DCPS builtin-endpoint
    spawn path.
  - `capabilities.rs`: test "phase0_peer_only" → "legacy_peer_only".
- README + CHANGELOG.

### Properties

- Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`.
- Safety class **SAFE**.
- 144+ tests green; clippy/doc clean.

### Known cross-layer findings

- **F-DISC-1 / F-DCPS-typelookup-wiring** (RC2 target): TypeLookup
  service endpoints are fully implemented in `discovery`, but not
  spawned as reliable writer/reader pairs in `dcps::runtime`. To be
  fixed during the DCPS layer-3 review (XTypes 1.3 §7.6.3.3.4
  spec compliance).

[Unreleased]: https://github.com/zero-objects/zero-dds/compare/v1.0.0-rc.1...HEAD
[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
