# Changelog — `zerodds-c-api`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer.

## [Unreleased]

### Added (DDS-Security 1.2 §11)

- Authentication and Cryptographic plugins of an application in C, C++ and
  Java: `ZeroDdsPluginAuthentication` and `ZeroDdsPluginCryptographic` in
  `zerodds_security_plugins.h`, `zerodds_security_set_authentication_plugin`
  and `zerodds_security_set_cryptographic_plugin`. `zerodds_runtime_create_secure`
  takes them instead of building PKI and AES-GCM from files; governance and
  permissions CA paths are required then, and the paths set in the
  configuration go to `validate_local_identity` as the properties of Table 50.
  A Cryptographic plugin without an Authentication plugin is refused.

- Plugins of an application in C, C++ and Java: `include/zerodds_security_plugins.h`
  (AccessControl, Logging, DataTagging), `zerodds_security_plugins.hpp` (C++11
  abstract classes) and `java/org/zerodds/security` (JDK 21, foreign function
  interface); `zerodds_security_set_access_control_plugin`,
  `zerodds_security_set_logging_plugin` and
  `zerodds_security_set_data_tagging_plugin` place one into a security
  configuration, and `zerodds_runtime_create_secure` installs it
  (`validate_local_permissions` and `check_create_participant` first).

### Changed

- `zerodds_topic_get_instance_handle` is the handle of the `DCPSTopic` entry of the topic's participant: the
  key of an entry includes the participant (DDS 1.4 §2.2.5).

### Added

- `zerodds_builtin_type_support_get_type_name` and `zerodds_dp_register_builtin_type`: `get_type_name` and
  `register_type` of the four built-in TypeSupports (DDS-XTypes 1.3 Annex E).


- `ZeroDdsTopicQos`, `ZeroDdsDataWriterQos` and `ZeroDdsDataReaderQos` end in a
  `ZeroDdsDataRepresentationQosPolicy` (`is_set`, `count`, `ids[8]`), `ZeroDdsDataReaderQos` also in a
  `ZeroDdsTypeConsistencyEnforcementQosPolicy` (DDS-XTypes 1.3 Annex D). An all-zero struct leaves both at
  their defaults. The structs grew: a caller that allocates one itself has to use the new size. A reader QoS
  with an undefined `TypeConsistencyKind` is refused by the calls that take it (`dr_qos_from_c_checked`).
- `ZeroDdsDataWriterQos` and `ZeroDdsDataReaderQos` end in the members
  `property` (`ZeroDdsPropertyQosPolicy`: string and binary entries with
  `propagate`) and `data_tags` (`ZeroDdsDataTagQosPolicy`) (DDS-Security 1.2
  §7.3.21). An all-zero value is no entries, so a caller that zeroes the
  struct is unchanged; a caller that sets members one by one must be compiled
  against the new header (the layout grew at the end). `get_qos` writes the
  entries through the cache of the entity; they stay valid until the next
  `get_qos` on it.
- `zerodds_pub_get_listener`, `zerodds_sub_get_listener` and `zerodds_topic_get_listener` return
  the table `zerodds_*_set_listener` registered, as the calls for the participant, the writer and
  the reader do; a null handle or none registered gives NULL.
- `zerodds_types_load` describes an array member or alias as
  `{"k":"array","e":<element>,"dims":[<extent>,…]}`; before it said `unsupported`. A type name
  with a scope (`m::Color`) written inside a module resolves.
### Fixed

- The participant prefix takes its tail from `next_process_unique`, the
  counter the Rust runtime uses, so no two participants of a process share a
  prefix (DDSI-RTPS 2.5 §8.2.4.2).
- `zerodds_dp_find_topic_w_timeout`: `find_topic` with the timeout of DDS 1.4
  §2.2.2.2.1.11; it waits for a topic another thread creates, and returns NULL
  on a timeout.

### Changed

- `zerodds_*_set_listener(NULL, mask)` with a non-empty mask is the nil listener (DDS
  1.4 §2.2.2.1.1.3): `zerodds_poll_listeners` does not fire the status of the mask
  at the levels above that entity (§2.2.4.3.1).
- `zerodds_dw_write` and `zerodds_dw_write_w_timestamp` check their `handle`
  (DDS 1.4 §2.2.2.4.2.11): `PreconditionNotMet` for the handle of another
  instance, `BadParameter` for one that names none; they ignored it.
- A typed reader or writer created with `zerodds_sub_create_datareader_typed` or
  `zerodds_pub_create_datawriter_typed` is a NoKey endpoint when its type has no
  `@key` member (DDSI-RTPS 2.5 Table 9.1); wire change: its entityKind is NO_KEY
  (0x03/0x04) instead of WITH_KEY (0x02/0x07). It was always WithKey, so a topic
  without a key had an instance per sample instead of one (DDS 1.4 §2.2.1.2.2).
- `zerodds_dw_get_key_value` and `zerodds_dr_get_key_value` return the key of an
  instance known by its 16 octets where the type of the endpoint reads it out
  of them (DDSI-RTPS 2.5 §9.6.4.8 Step 5.1), and `BadParameter` instead of
  `Unsupported` where it does not (DDS 1.4 §2.2.2.4.2.9, §2.2.2.5.3.29).
- `zerodds_writer_dispose_sample` and its siblings dispose the one instance of a
  typed writer whose type has no `@key` (DDS 1.4 §2.2.1.2.2); they refused it.
- A QueryCondition takes an `ORDER BY` (DDS 1.4 Annex B.2): the `*_w_condition`
  reads and takes return the selected samples sorted ascending by the fields.
- A reader of a keyless topic tracks the topic's one instance (DDS 1.4
  §2.2.1.2.2) and reports its handle and states; a `NO_KEY` writer names it by
  the all-zero key in `zerodds_dw_register_instance`, `_lookup_instance`,
  `_dispose` and `_unregister_instance`, and `zerodds_writer_dispose_sample`
  and its siblings send its marker.
- `zerodds_dp_delete_contentfilteredtopic` returns `PreconditionNotMet` while
  a DataReader created on the topic exists (DDS 1.4 §2.2.2.2.1.8).
- `zerodds_writer_set_qos` and `zerodds_reader_set_qos` refuse a change of
  HISTORY, RESOURCE_LIMITS and the `max_blocking_time` of RELIABILITY with
  `ImmutablePolicy`, as they refuse RELIABILITY and DURABILITY: DDS 1.4
  §2.2.3 lists all of them "Changeable: No", and of the policies the
  DDS-RPC QoS carries (§7.10.2) only DEADLINE and LIFESPAN (writers) are
  changeable. They were not checked before.

### Added

- `zerodds_writer_write_to_readers`: a sample for the readers it names (by
  GUID) and no other. DDS-RPC 1.0 §7.11.1.4.3: the C++ binding of
  `dds/rpc.hpp` addresses an Enhanced request to the readers of the service
  instance it is bound to.
- `zerodds_writer_next_sequence_number`: the RTPS sequence number the next
  sample written through a writer will carry. DDS-RPC 1.0 §7.5.1.1.1: the
  `requestId` of a request header is the identity of the sample it is part
  of and is filled before the write; the C++ binding of `dds/rpc.hpp` asks
  the writer for it instead of keeping a counter of its own.
- **DDS-RPC 1.0 §7.6.2 discovery** (`rpc_discovery_ffi`):
  `zerodds_writer_set_rpc_discovery`, `zerodds_reader_set_rpc_discovery`
  announce `PID_SERVICE_INSTANCE_NAME` (0x0080), `PID_RELATED_ENTITY_GUID`
  (0x0081) and `PID_TOPIC_ALIASES` (0x0082);
  `zerodds_writer_matched_rpc_readers` and `zerodds_reader_matched_rpc_writers`
  read what the matched peers announced; `zerodds_rpc_service_matching_complete`
  (Figure 7.4) and `zerodds_rpc_client_matching_complete` (Figure 7.3, the
  per-client mask) run the Enhanced algorithms of `zerodds-rpc`. New
  dependency `zerodds-rpc`. Tested by `tests/rpc_discovery.rs`.
- **`zerodds_writer_set_data_representation`** — the CDR form (`0` XCDR1,
  `2` XCDR2) the samples of a runtime writer have, so the encapsulation
  identifier names the body (DDS-RPC 1.0 §7.5.1.1: classic CDR).
- **`zerodds_reader_set_arrival_callback`** — calls a function on the
  receive thread once after each run of samples a runtime reader has
  queued, and leaves the samples in the reader, unlike
  `zerodds_reader_set_data_callback`, which hands over the payload and
  forbids ZeroDDS calls. A `release` function frees the caller's data when
  the last call of a removed callback is over. DDS-RPC 1.0 §7.9.2.2: a
  listener takes a request and writes the reply. Tested by
  `tests/arrival_callback.rs`.
- **`zerodds_endpoint_qos_from_profile_url`, `zerodds_writer_set_qos`,
  `zerodds_reader_set_qos`** — a DDS-RPC `@Qos` profile URL
  (`[file://]path[#library]`, §7.10.1) resolved to endpoint QoS over the
  DDS-RPC defaults of §7.10.2, and the mutable policies (DEADLINE, LIFESPAN)
  of a live runtime endpoint changed at run time; RELIABILITY and DURABILITY
  differing from the endpoint's are `ImmutablePolicy`. The crate depends on
  `zerodds-xml` for the profile. Tested by `tests/rpc_qos.rs`.
- **`zerodds_writer_write_related` and `zerodds_reader_take_wire`** —
  write a sample with `PID_RELATED_SAMPLE_IDENTITY` in its inline QoS and
  take one with the writer GUID, the RTPS sequence number and the related
  sample identity of its DATA (DDS-RPC 1.0 §7.8.2). Tested by
  `tests/related_sample_identity.rs`.
- The QosProvider reads the QoS containers of a profile as `FilteredEntityQos` lists
  (DDS-XML 1.0 §7.3.2.4.3) and answers with `default_qos`: the container without a
  `topic_filter`, else the first. DDS-PSM-Cxx 1.0 `QosProvider` has no topic-name form,
  so no call selects by topic name.
- `zerodds_types_load` writes the DDS-XML `int8`/`uint8` primitives and `<annotation>`
  declarations as IDL; an annotation is described as `"kind":"annotation"`.

### Fixed

- `zerodds_dr_read_w_condition`, `zerodds_dr_take_w_condition` and the
  `*_next_instance_w_condition` reads ignored the expression of a
  QueryCondition and returned every sample the state masks let through; the
  trigger value evaluated the expression against a row without fields and
  passed every sample. Both now read the fields through the reader's type.
  A QueryCondition whose expression names a field on a reader without a type
  returns `NO_DATA` while the reader holds no sample to judge and
  `PRECONDITION_NOT_MET` as soon as it holds one, instead of passing it.
  `BAD_PARAMETER` stays for an expression that does not parse.

### Added

- `zerodds_types_load`, `zerodds_types_describe_idl`: the types an IDL or
  DDS-XML type document defines, as JSON (for `dds::core::xtypes::TypeProvider`).
- `zerodds_qosprovider_create` reads `https:` URIs: TLS 1.2/1.3 through rustls
  (ring provider), the certificate checked against the webpki roots. New
  dependencies: `rustls`, `rustls-pki-types`, `webpki-roots` (all in `Cargo.lock`
  already through `ureq`). Not exercised against a TLS server.
- `zerodds_dr_set_type_objects`: gives a reader the type of its samples (root
  and dependency TypeObjects).
- `zerodds_dr_read_instance_w_condition`, `zerodds_dr_take_instance_w_condition`:
  the samples of one instance that a condition selects.

### Added

- `zerodds_dyntype_*` (`dyntype_ffi.rs`): a dynamic type from IDL text (`zerodds_dyntype_from_idl`),
  its name, key, extensibility, TypeObject and TypeObject dependencies, its default value as JSON,
  and `zerodds_dyntype_encode_json` / `zerodds_dyntype_decode_json` between a value in the JSON
  dialect of `zerodds-wire-probe` and the body of a sample (XCDR1 or XCDR2, either byte order).
  New dependencies: `zerodds-ast`, `zerodds-ast-core`, `zerodds-ast-layout`, `zerodds-idl-parse`,
  `zerodds-idl-lower`, `zerodds-wire-codec`, `serde_json`.

### Changed

- `zerodds_qosprovider_create` reads `file:` (RFC 8089: `file:///p`, `file://localhost/p`,
  `file:/p`), `data:` (RFC 2397, percent-encoded or `;base64`) and `http:` (one HTTP/1.1 GET,
  `200` only, no redirect, 10 s timeout) URIs. `https:` and a scheme no specification defines
  are UNSUPPORTED; a text that is no URI is BAD_PARAMETER. New dependency: `base64`.

### Added

- `zerodds_querycondition_set_expression`, `zerodds_querycondition_set_parameters`
  (the expression and the parameters of a query condition sit behind a lock now),
  `zerodds_dr_read_next_instance_w_condition`, `zerodds_dr_take_next_instance_w_condition`.

### Changed

- Two tests (`cpp_typed_writer_declares_extensibility`, `cpp_typed_writer_dispose_sample`)
  name `::zerodds::psm::DataRepresentationKind` in their embedded C++, the type
  moved out of `dds::topic::core::policy`. No change to the C API.

### Added

- **`zerodds_qosprovider_create`, `_destroy`, `_participant_qos`,
  `_topic_qos`, `_publisher_qos`, `_subscriber_qos`, `_datawriter_qos`,
  `_datareader_qos`** — DDS-XML QoS profiles for the QosProvider of
  DDS-PSM-Cxx 1.0 §7.6.2.1: a `file:` URI names the document, a profile
  (`Library::Profile`) the QoS, with `base_name` inheritance
  (DDS-XML 1.0 §7.3.2.4.2); another URI scheme is `ZERODDS_UNSUPPORTED`. The
  crate depends on `zerodds-xml` for it. `tests/qos_provider.rs`.
- **`zerodds_dw_instance_key_hash`** — the key hash of the instance a handle
  names, so `dispose_instance(handle)` and `unregister_instance(handle)` of
  the C++ API can reach `zerodds_dw_dispose`, which takes the key.
  `tests/instance_key_hash.rs`.
- **`zerodds_dw_get_offered_incompatible_qos_status_ex`,
  `zerodds_dr_get_requested_incompatible_qos_status_ex`** — the incompatible-QoS
  statuses with the count per policy (`ZeroDdsIncompatibleQosStatusEx`, up to
  32 `ZeroDdsQosPolicyCount`); the plain status has the last policy id only.
  `tests/incompatible_qos_policies.rs`.

- **`zerodds_{dp,topic,pub,sub,dw,dr}_enable`, `_get_status_changes`,
  `_get_instance_handle`** (DDS 1.4 §2.2.2.1.1.6 to §2.2.2.1.1.8), and
  ENTITY_FACTORY `autoenable_created_entities` (§2.2.3.20): an entity
  created from a disabled factory or one that does not autoenable is created
  disabled; a DataWriter or DataReader is registered with the runtime
  (announced, matched) at its first `enable`. Operations the specification
  does not allow on a disabled entity return `ZERODDS_NOT_ENABLED`.
  `zerodds_dpf_set_qos` reaches the factory that creates participants, and
  `zerodds_dp_contains_entity` recognises the C entities' handles.
  `tests/entity_enable.rs`, `tests/entity_enable_factory.rs`.
- **The built-in types of DDS-XTypes 1.3 §7.6.5 and Annex E.**
  `zerodds_dp_create_builtin_topic`, `zerodds_pub_create_builtin_datawriter`
  and `zerodds_sub_create_builtin_datareader` for `DDS::String`,
  `DDS::KeyedString`, `DDS::Bytes` and `DDS::KeyedBytes`
  (`ZERODDS_BUILTIN_*`); `zerodds_string_writer_write`,
  `zerodds_keyed_string_writer_*` (`write_string_w_key`,
  `register_instance_w_key`, `unregister_instance_w_key`, `dispose_w_key`,
  `lookup_instance_w_key`, `get_key_value_w_key`, each with its
  `_w_timestamp` form where Annex E has one), `zerodds_bytes_writer_write_w_bytes`
  and `zerodds_keyed_bytes_writer_*`; the readers
  `zerodds_*_reader_take_next` and `_read_next`;
  `zerodds_builtin_string_free` and `zerodds_builtin_bytes_free`.
- **`zerodds_dp_lookup_topicdescription_at(p, name, index)`**
  (DDS-XTypes 1.3 §7.6.4.2): the topics of one name in creation order.

### Changed

- `zerodds_dp_create_topic` creates a second topic under a name when the
  type differs and the QoS equals the first object's (DDS-XTypes 1.3
  §7.6.4.1); it returned NULL for any other type under a name in use.

- `topic_qos_from_c` sets the new `TopicQos::data_representation` to `None`;
  the C struct has no field for it.
- **The typed path has a producer.** `zerodds-idlc generate --lang c`
  writes `<stem>_typesupport.h` with one `zerodds_typesupport_t` per topic
  type for `zerodds_writer_write_typed` and `zerodds_reader_take_typed`;
  `tests/typed_c_via_cargo.rs` writes and takes three keyed samples of a
  generated type through them. `REPR_BIG_ENDIAN` (Rust) and
  `ZERODDS_TS_REPR_BIG_ENDIAN` (C) name the bit of `decode_repr`'s
  `representation` byte that says the sample is big-endian.
- **`zerodds_writer_dispose_sample`, `zerodds_writer_unregister_sample`,
  `zerodds_writer_unregister_with_dispose_sample`** — dispose/unregister
  lifecycle markers derived from the sample itself instead of a
  caller-supplied 16-byte key hash. `zerodds_writer_dispose`/
  `_unregister`/`_unregister_with_dispose` hand `write_user_lifecycle`
  only a key hash (`None` key holder), so the DATA they send carries
  `PID_KEY_HASH` alone — measured (OMG dds-rtps
  `Test_FinalInstanceState_1`): Eclipse Cyclone 11.0.1, Fast DDS 3.6.1 and
  OpenDDS 3.35 do not treat that form as a dispose (0/4); with a
  `serializedKey` all three do (4/4). The new functions decode the
  sample against the writer's registered `TypeObject`
  (`zerodds_writer_create_typed`), derive the real KeyHash and the
  canonical key holder (`DcpsRuntime::writer_sample_key_material`, new),
  and send the full form. A writer with no registered `TypeObject`, or
  whose type has no `@key` member, returns `PreconditionNotMet` rather
  than falling back to the hash-only marker silently. Tests:
  `tests/writer_dispose_sample.rs`.
- **`zerodds_sub_create_datareader_kind`** — DataReader creation with an
  explicit keyed / no-keyed flag, the topic surface's counterpart to
  `zerodds_reader_create_kind`. `is_keyed = 0` announces a `NO_KEY` reader
  endpoint (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1) and makes `zerodds_dr_take` /
  `zerodds_dr_read` report the topic's single instance (DDS 1.4 §2.2.1.2.2).
  Additive; `zerodds_sub_create_datareader` keeps its keyed behaviour.

- **`xcdr2` module** — implements the `zerodds-xcdr2-c-1.0`
  vendor spec.
  - `ZeroDdsTypeSupport` (`zerodds_typesupport_t` C-side) as a
    function-table-based TypeSupport struct.
  - FFI functions `zerodds_topic_create_typed`,
    `zerodds_topic_destroy_typed`, `zerodds_writer_write_typed`,
    `zerodds_reader_take_typed`, `zerodds_xcdr2_encode`,
    `zerodds_xcdr2_decode`.
  - `ZeroDdsTopic` opaque handle for typed topics.
  - Helper functions `copy_to_out_buf`, `input_slice`, `write_out_len`
    for codegen encoders/decoders.
- **`include/zerodds_xcdr2.h`** — hand-maintained C99 header with the
  `zerodds_typesupport_t` struct + FFI forward declarations. No codec of
  its own: an `encode`/`decode` function pointer a caller fills the
  table with is expected to call the one C wire codec,
  `runtime/c/include/zerodds_wire.h` (`zdw_*`) — the same one
  `zerodds-idlc generate --lang c`'s `zd_enc_*`/`zd_dec_*` free
  functions call. An earlier revision of this header also carried
  inline helpers reimplementing that codec
  (`zerodds_xcdr2_c_write_uN/iN/fN/string`, `zerodds_xcdr2_c_read_*`,
  `zerodds_xcdr2_c_kh_write_*`, `zerodds_xcdr2_c_compute_key_hash`, a
  built-in MD5); nothing called them and they are gone before this ever
  released.
- **L1 wire-conformance tests** in `tests/xcdr2_wire_vectors.rs` —
  check V-1..V-12 byte-exactly over the FFI pattern.
- **L2 codegen-conformance tests** in `tests/xcdr2_c_codegen.rs` —
  verify the `idl-cpp::generate_c_header` output for each
  V-1..V-12 IDL snippet.
- **L2 C-compile tests** in `tests/xcdr2_c_compile.rs` — compile
  each generated header with `cc -std=c99 -Wall -Werror`, to guarantee C99
  validity (skipped if no C compiler is in PATH).

### Fixed

- **No list in the C API is cut short without saying so.** Seven functions
  copied `min(len, cap)` entries and set `*out_count` to that, so a caller
  with too small a buffer got a shorter list and could not tell. They now
  write the first `min(len, cap)` entries, set `*out_count` to the **total**
  length and return `ZERODDS_OUT_OF_RESOURCES` when `len > cap`; `cap == 0`
  with a NULL buffer asks for the length alone
  (`docs/specs/zerodds-c-api-1.0.md` §2.8). Affected:
  `zerodds_dp_get_discovered_topics`, `zerodds_dp_get_discovered_participants`,
  `zerodds_dw_get_matched_subscriptions`,
  `zerodds_dr_get_matched_publications`, `zerodds_waitset_get_conditions`,
  the active-conditions list of `zerodds_waitset_wait` and
  `zerodds_sub_get_datareaders`. The C++ and C# callers retry with the
  reported total. Tests: `ffi_helpers::tests::copy_list_out_*`,
  `condition_ffi::tests::{waitset_get_conditions_reports_the_total,
  waitset_wait_reports_the_total}`,
  `subscriber_ffi::tests::sub_get_datareaders_reports_the_total`.
- **`zerodds_xcdr2.h` and `zerodds.h` compile together.** The typed
  header declared `zerodds_topic_create_typed` and the other typed
  functions again over `struct zerodds_typesupport_s`; it is a thin header
  over `zerodds.h` now (`zerodds_typesupport_t` is
  `struct zerodds_ZeroDdsTypeSupport`) with the `ZERODDS_TS_*` status
  values.
- **`zerodds_reader_take_typed` passes the byte order** to `decode_repr`;
  a table with only `decode` answers a big-endian sample with UNSUPPORTED.
- **`zerodds_writer_write_typed` writes a keyed sample with its KeyHash**
  and sets the writer's wire extensibility from the table; `is_keyed`
  without `key_hash` is BAD_PARAMETER.
- **Unbounded instance growth on `zerodds_dr_take` / `zerodds_dr_read`.**
  The take path called `InstanceTracker::observe_sample` for every alive
  sample, with an instance identity derived from the whole payload and the
  payload itself stored as the entry's key holder. Nothing removes such an
  entry — `autopurge` only drops NOT_ALIVE instances, which a topic with a
  live writer never becomes — and nothing on the C surface reads the tracker
  except the EXCLUSIVE-ownership arbitration, so on a SHARED reader (the
  default OWNERSHIP) every entry was write-only. An instance is now
  registered only where one is arbitrated, and a keyless topic is one
  instance. Tests:
  `subscriber_ffi::tests::{keyless_take_does_not_grow_the_instance_tracker,
  keyed_take_under_shared_ownership_registers_no_instance,
  keyless_exclusive_arbitration_uses_one_instance}`.
- **EXCLUSIVE ownership was inert on a keyless topic**: every payload looked
  like a fresh instance, so no writer ever lost an arbitration it should have
  lost. Test: `keyless_exclusive_arbitration_uses_one_instance`.
- **`SampleInfo.instance_handle` and `zerodds_dr_lookup_instance` disagreed.**
  The take path handed out `InstanceTracker`-allocated counters while
  `lookup_instance` derives the handle from the low 8 bytes of the KeyHash, so
  a handle from `lookup_instance` never matched one from `take` and
  `zerodds_dr_take_instance` filtered every sample away. Both now use the
  KeyHash derivation. Test: `take_handle_matches_lookup_instance_handle`.

### Changed

- **`zerodds_dp_delete_topic` applies the DDS 1.4 §2.2.2.2.1.6
  precondition** and reaches the Rust participant. It refused only a topic
  another participant owns and then freed its own handle; it now also
  returns `ZERODDS_PRECONDITION_NOT_MET` while a DataWriter, DataReader or
  ContentFilteredTopic of that participant still uses the topic — the
  clause names all four kinds — and hands the name to
  `zerodds_dcps::DomainParticipant::delete_topic_by_name`, which removes
  the typed topic registry entry and disposes the announcement on the SEDP
  topics writer. A caller that destroys a topic before its endpoints sees
  the refusal instead of a freed handle.
- **The participant tracks its ContentFilteredTopics.** `zerodds_dp_delete_contentfilteredtopic`
  takes its handle out of that count (and reports `ZERODDS_BAD_HANDLE` for
  one that is not in it), and `zerodds_dp_delete_contained_entities` frees
  them; before, it freed the box alone and the CFTs of a torn-down
  participant were never released.
- **`ZeroDdsTopicBuiltinTopicData` carries every DCPSTopic member** — it held
  a key, two names and `durability_kind`/`reliability_kind` as `uint32_t`;
  it now holds each member DDS 1.4 §2.2.5 lists for the
  TopicBuiltinTopicData, as the policy structs `get_qos` already uses, a
  duration in seconds and nanoseconds. The struct changes shape: a caller
  compiled against an earlier header has to be rebuilt.
  `zerodds_dp_get_discovered_topic_data` fills every member and hands the
  two names and the TOPIC_DATA octet sequence to the caller, who releases
  them with the new **`zerodds_topic_builtin_topic_data_free`** (a name with
  a NUL is refused with `ZERODDS_INVALID_UTF8` and fills nothing, where the
  call wrote an empty string).
- **`ZeroDdsEndpointQos` carries RESOURCE_LIMITS and `max_blocking_time`**
  — four fields at its end: `max_samples`, `max_instances`,
  `max_samples_per_instance` (`-1` LENGTH_UNLIMITED) and
  `max_blocking_time_ns` (`0` does not wait, `ZERODDS_BLOCKING_INFINITE`
  waits without end). `zerodds_writer_create_qos` applies them as
  `create_datawriter` does (DDS 1.4 §2.2.2.4.2.11, §2.2.3.19), and
  `zerodds_endpoint_qos_default` sets them to unlimited and 100 ms. The
  struct grows: a caller compiled against an earlier header passes a
  smaller struct and has to be rebuilt. Values outside §2.2.3.19 are
  refused by name.
- **The flat write, dispose and unregister calls return
  `ZERODDS_TIMEOUT` and `ZERODDS_OUT_OF_RESOURCES`** —
  `zerodds_writer_write`, `zerodds_writer_dispose`,
  `zerodds_writer_unregister`, `zerodds_writer_unregister_with_dispose`,
  the `_sample` variants and `zerodds_writer_commit_loan` answered every
  failure with `ZERODDS_ERROR`.

- `Cargo.toml`: add `zerodds-cdr` as a runtime dependency (with
  `alloc` + `std` features), `zerodds-idl` + `zerodds-idl-cpp` as
  dev-dependencies for codegen/wire tests.
- `src/lib.rs`: registers `pub mod xcdr2`.

### Notes

- **Codegen path choice** (vendor spec §4): **Option A** implemented — a new
  module `crates/idl-cpp/src/c_mode.rs` with `generate_c_header` /
  `CGenOptions`, integrated into the existing `zerodds-idl-cpp`
  crate. Limited to the V-1..V-12 corpus + extensible form (final/
  appendable/mutable structs, primitive types, `string`, `sequence<T>`,
  nested modules, `@key`, `@id`).
- **Wire-vector errata** vs. `zerodds-xcdr2-bindings-conformance-1.0`
  §6:
  - **V-3** the spec doc shows a 40-byte wire with three numeric errors
    (`l = -1234567`, `ul = 2345678`, `ll = -987654321`); XCDR2-spec-
    conformant LE bytes yield 48 bytes. Tests check the corrected
    form, which interoperates with `zerodds-cdr` and Cyclone DDS.
  - **V-10**/V-11/V-12 EMHEADER is shown visually BE-grouped in the spec doc;
    the wire bytes are LE-serialized (XCDR2 stream
    endianness applies to all fields, incl. EMHEADER). Tests assert
    the LE form, consistent with `zerodds-cdr::struct_enc` and Cyclone DDS.

### Added — ABI compat test

- `tests/abi_compat.rs` + `abi.snapshot.json` as a 185-symbol baseline
  for drift detection: every new or renamed FFI symbol entry
  makes the test fail, until `abi.snapshot.json` is updated with a
  justification. `serde` + `serde_json` as dev-deps.
