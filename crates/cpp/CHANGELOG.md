# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- `runtime/cpp/include/dds/rpc/core.hpp` compiles again: `ListenerErrorSlot` stood in a
  `detail` namespace nested in `dds::rpc::core`, which hid `dds::rpc::detail` (`detail::fulfil`,
  `detail::Deadline`) for the rest of the file; it is in `dds::rpc::detail` with the other helpers.

### Changed

- Shared code in place of copies (CPD gate): `zerodds/psm/handle_seq.hpp` holds the matched-handle
  call and the per-thread sequence of `dds::pub::matched_subscriptions` and
  `dds::sub::matched_publications` (one sequence per side, as before);
  `zerodds/psm/expression_parameters.hpp` the expression and parameters of `Filter` and `Query`;
  `function<R(...)>` of `zerodds/psm/cxx.hpp` keeps its holder in one base for the three arities;
  the status, built-in topic and octet-policy templates spell their accessors through one macro
  each; `OfferedIncompatibleQosStatus` and `RequestedIncompatibleQosStatus` share one delegate. No
  public name or behaviour changed.

### Added

- `zerodds/builtin_type_support.hpp`: `DDS::TypeSupport`, `StringTypeSupport`, `KeyedStringTypeSupport`,
  `BytesTypeSupport`, `KeyedBytesTypeSupport` (DDS-XTypes 1.3 Annex E).


- **The headers of `dds/` are ISO/IEC C++ 2003** (DDS-PSM-Cxx 1.0 §3 [C++], §7.1). All 144 public
  headers compile with `g++ -std=c++03 -pedantic` and with clang as C++03 with the C++11
  extensions as errors; the smoke program runs as C++03 (`dds_psm_cxx_smoke_as_cxx03`) and as
  C++17. `zerodds/psm/cxx.hpp` carries what C++03 lacks: `shared_ptr`/`weak_ptr`/`make_shared`,
  `mutex`/`lock_guard`, `function` (up to two arguments), `array`, `type_index`, type traits,
  `nullptr`, `ZERODDS_STATIC_ASSERT`; with a C++11 compiler they are the names of the standard
  library. The C++11 additions of §7.12 are behind `#if ZERODDS_CXX11`. `zerodds/psm/encoding.hpp`
  gives `zerodds::wire::Encoding` without the C++17 wire runtime. `tools/psm-cxx03-check.py` checks the
  headers; the CMake target
  `zerodds_psm_standalone_headers_cxx03` builds them as C++98. Tests:
  `every_public_header_compiles_on_its_own_as_cxx03`, `dds_psm_cxx_smoke_as_cxx03`.
- **`zerodds-idlc generate --lang c++` writes the PSM mapping by default** (`--cpp-mapping psm`);
  `--cpp-mapping native` is the named option for the structures of the codec. The tests, scripts
  and READMEs that read the fields of a generated header pass `native`.
- `DataRepresentation` in the `TopicQos`, `DataWriterQos` and `DataReaderQos` sets and
  `TypeConsistencyEnforcement`, with the five flags of DDS-XTypes 1.3 Annex D, in the `DataReaderQos` set; both
  policies tell whether the application gave them (`is_set()`) and reach the entity through the C API.
- `dp->lookup_topicdescription(name, index)` (DDS-XTypes 1.3 §7.6.4.2) on the participant delegate: the topics of
  one name in the order they were created, a null topic past the last.
- **`@external` in the PSM mapping is `dds::core::external<T>`** (DDS-XTypes 1.3 §7.5.1.2.3.3;
  `runtime/cpp/include/dds/core/external.hpp`), and `@optional` converts to the plain pointer of the
  native mapping (§7.5.1.2.4.2). `zerodds::psm::Conv` has the specialisations for both.
- **Reference types as Table 7.2 has them.** `AnyDataWriter`, `AnyDataReader` and `AnyTopic` derive
  from `Reference<…>`; every reference type is constructed from `dds::core::null`, assigned it and
  compared with it. A call through a null reference is `NullReferenceError`
  (`Reference::delegate()`, `operator->`); before it dereferenced a null pointer.
- **`ArrayType`.** `dds::core::xtypes::ArrayType(element, extents)` (`TypeKind::ARRAY_TYPE`) as a
  member, a union case or an alias of a `DynamicType`; `TypeProvider` loads types with arrays
  (`zerodds_types_load` describes them as `{"k":"array","e":…,"dims":[…]}`). Not included: bitmasks,
  unions with a default case or an enumeration as discriminator, `fixed`; the normative headers have
  no class for them.
- **Extensions leave the normative classes.** `find_topic`, `lookup_topicdescription`,
  `get_builtin_subscriber`, `get_discovered_*` and `delete_contained_entities` are no longer
  members of `DomainParticipant`, `suspend_publications`/`begin_coherent_changes` and their
  counterparts no longer members of `Publisher`, `begin_access`/`end_access` no longer members of
  `Subscriber` (§2.2 allows no extension in the normative namespaces). They are reached through
  `operator->` (`dp->find_topic(name)`), the standard way to the delegate.
- **Names of the normative headers (PSM §2, §2.2).** `matched_publications_data`,
  `offered_deadlined_missed_status` and `offered_incompatible_qos_staus` (the
  spellings of the OMG headers) next to the existing names; `ParticipantTopicData`;
  `is_topic_type`, `topic_type_name` and `REGISTER_TOPIC_TYPE`; `get_status<STATUS>()`;
  `tuple_size`, `tuple_element` and `get<I>` for `dds::core::array`;
  `OMG_DDS_POLICY_TRAITS` and `OMG_DDS_DEFINE_POLICY_TRAITS`; the policy classes
  `DataRepresentation` and `TypeConsistencyEnforcement` with `DataRepresentationId`
  and `TypeConsistencyEnforcementKind` (members of the QoS sets from the entry
  "`DataRepresentation` in the `TopicQos`" below).
- **`retain()` and listeners.** An entity retained with `retain()` stays in use until `close()`,
  and so does an entity with a non-null listener until the listener is removed
  (`dds/core/detail/Retain.hpp`, DDS-PSM-Cxx 1.0 §7.5.1.2); before, `retain()` did nothing and a
  writer with a listener was destroyed with its last reference.
- **`delete_contained_entities()` of a participant.** The participant tracks the publishers,
  subscribers, topics and content-filtered topics created with it (`ChildTracker`); the call
  closes them, the writers and readers first, before the C API frees what it holds. Before, the
  C API freed the children and their delegates freed them again when destroyed.
- **Construction from a delegate reference** for `DomainParticipant` and
  `DataWriter<T>`, which `WeakReference<T>::lock()` needs.
- **Tests.** `smoke_dds_psm.cpp` runs the xtypes and built-in topic type cases that were
  compile-only programs, and the operations, accessors and listener callbacks that no test
  called.

- **xtypes (PSM §7.5).** `dds/core/xtypes/`: `DynamicType`, the primitive,
  string, sequence, map, structure and union types, members and annotations
  (`Key`, `Optional`, `Id`, `Final`/`Extensible`/`Mutable`, `Nested`,
  `MustUnderstand`, `Shared`), `DynamicData` as `TDynamicData<DELEGATE>` with
  `value<T>` by member name or id, `member_type`, `member_id`, `encode` and
  `decode`, and `TypeProvider` (`load_type`, `load_types` from IDL or a DDS-XML
  type library at a `file:`, `data:`, `http:` or `https:` URI). A type is
  written as IDL and lowered by the C API (`zerodds_dyntype_from_idl`), so
  TypeObject, key and encoding come from the same code as the generated
  types. `EnumerationType`, `AliasType` and `BitSetType` are not classes of the
  specification's headers (`TypeKind` names the kinds); they are provided in
  `dds/core/xtypes/EnumTypes.hpp`. Not included: bitmasks, unions with a default
  case or an enumeration as discriminator, `fixed`.
- **`Topic<DynamicData>`.** `Topic<DynamicData>(dp, name, type)` registers the
  type under its name; `DataWriter<DynamicData>` and `DataReader<DynamicData>`
  find its TypeObject there, and a `QueryCondition` or `ContentFilteredTopic`
  reads its fields.
- **Built-in topic types.** `dds/core/BuiltinTopicTypes.hpp`: `BytesTopicType`,
  `StringTopicType`, `KeyedBytesTopicType`, `KeyedStringTopicType` as
  `DDS::Bytes`, `DDS::String`, `DDS::KeyedBytes`, `DDS::KeyedString` of
  DDS-XTypes 1.3 Annex E.
- A `DataReader` of a generated type tells the C API its type
  (`zerodds_dr_set_type_objects`), so a `QueryCondition` reads fields;
  `select().instance(h).content(query)` works
  (`zerodds_dr_read_instance_w_condition`).

### Fixed

- **Getters of policies and statuses return values.** `UserData::value()`, `Partition::name()`,
  the `Duration` getters, `last_*_handle()` and `policies()` of the statuses follow the normative
  headers (by value); `on_offered_incompatible_qos` takes the writer by value. The QoS bridge
  points at the storage of a policy, not at the copy a getter returns.
- **Union of the `psm` layer.** The codec conversion of a union reads the selected branch only;
  before it read the uninitialised branches. The class follows IDL4 to C++ 1.0 §7.2.4.3.2.
- **Thread sanitizer test.** The sanitizer programs start with `setarch -R`; with address space
  randomisation the runtime printed nothing and the test saw an empty output.
- **Integer types in the global namespace (finding 18).** Every header that
  names `uint32_t`, `int64_t` and the like includes `<stdint.h>`, which
  declares them globally as DDS-PSM-Cxx 1.0 §7.4.2 requires; before, they
  relied on `<cstdint>` doing so, which the C++ standard leaves open.

- Every public header compiles on its own; a CMake object library builds one
  translation unit per header. Found and fixed: include cycles around
  `TopicDescription`/`AnyTopicDescription`/`AnyTopicDetail`, a second
  `QueryCondition` in `sub/cond/QueryCondition.hpp`, `CoherentAccess` taking a
  non-const `Subscriber&`, and `DomainParticipantListener` defined twice.
  `TopicDescription<T>` names a description with the default delegate.
- `DomainParticipantListener` is the listener of §7.7 (publisher, subscriber
  and topic listener in one); its C callbacks are defined with
  `DomainParticipant.hpp`. The participant level of the C API delivers
  `on_inconsistent_topic` and `on_data_on_readers` only.
- `examples/cpp_smoke.cpp` called `wait_for_matched` through `->` after the
  previous commit; it is a member of `zerodds::Writer` again.

### Changed

- **No vendor names in the normative namespaces (PSM §2.2).** `DomainParticipantFactoryQos` is
  removed. `dds::sub::builtin_subscriber(dp)` returns a `dds::sub::Subscriber` (`BuiltinSubscriber`
  is gone); `dds::sub::find` finds the four built-in readers on it. `TTopic`, `TMultiTopic`,
  `TContentFilteredTopic`, `TTopicDescription` and `TDataReader` are in `detail`. The test
  `no_vendor_name_in_the_normative_namespaces` and `tests/vendor_names.txt` hold the names that
  remain with their reason. `dds::code` is an alias of `dds::core`, `MultiTopic` is reachable
  through `TMultiTopic.hpp`.

- **The value types are `T…<D>` templates over a delegate as well (finding 9, part).** New
  headers `core/policy/TCorePolicy.hpp` (the 22 policies), `core/policy/TQosPolicyCount.hpp`,
  `core/status/TStatus.hpp` (the 11 status classes), `core/TInstanceHandle.hpp`,
  `core/TQosProvider.hpp`, `sub/{TRank,TGenerationCount,TSampleInfo,TSample}.hpp`,
  `topic/{TFilter,TBuiltinTopic}.hpp`, `domain/TDomainId.hpp` and `domain/DomainId.hpp`
  (`dds::domain::DomainId`), and `core/refmacros.hpp` (`OMG_DDS_REF_TYPE` and the
  other macros that declare a reference type). The former classes moved into `detail`
  namespaces as the delegates; `dds::core::policy::Reliability` and the others are
  `typedef T…<detail::…>` now. A `Value<D>` takes its delegate by move
  (`Value(D&&)`). `TSample<T, DELEGATE>` takes the full delegate type, not a template
  template parameter.

- **The reference types are `T…<DELEGATE>` templates with ZeroDDS's classes as instantiations
  (finding 9, part).** New headers: `pub/TPublisher.hpp`, `sub/TSubscriber.hpp`,
  `sub/TDataReader.hpp`, `domain/TDomainParticipant.hpp`, `topic/TTopicDescription.hpp`,
  `topic/TTopic.hpp`, `topic/TContentFilteredTopic.hpp`, `core/cond/{TGuardCondition,
  TStatusCondition,TWaitSet}.hpp`. `Publisher`, `Subscriber`, `DomainParticipant`,
  `Topic<T>`, `ContentFilteredTopic<T>`, `DataReader<T>`, `GuardCondition`, `StatusCondition`
  and `WaitSet` derive from them over the ZeroDDS delegates; the delegates took over the
  listener installation (`listener(l, mask)`). `TEntity` defines its five operations.
  Differences from the normative text: the delegate parameter of the `T…` classes whose type
  parameter is `T` is a full delegate type (`DataReaderDelegate<T>`), not a template
  template parameter. `TWaitSet` has `dispatch`, `conditions` and `detach_condition`
  returning `bool`; `TGuardCondition` has `handler`/`reset_handler`.

- **The `discover` forms without a participant, and the iterator form of
  `matched_subscriptions`/`matched_publications` (finding 9, part).**
  `topic::discover<AnyTopic | Topic<T> | TopicBuiltinTopicData>(begin[, max_size])`
  read from all participants the process created and still holds (the
  specification gives these forms no participant). A discovered `AnyTopic`
  carries the announcement; `get<T>()` creates the local `Topic<T>` of that
  name when the type name matches, its `qos(q)` setter throws
  `IllegalOperationError`. `matched_*(entity, begin, end)` copies the handles
  into the range and returns the whole sequence, valid until the next call
  on the same thread.

- `QosProvider` reads `file:`, `data:` and `http:` URIs (C API `zerodds_qosprovider_create`).
  `https:` has no client in the C API and is `UnsupportedError`, as is a scheme no
  specification defines.

- **`QueryCondition::expression(str)`, `parameters(begin, end)` and
  `add_parameter` work** (C API `zerodds_querycondition_set_expression`,
  `zerodds_querycondition_set_parameters`; DDS 1.4 §2.2.2.5.9
  `set_query_parameters`). `select().next_instance(h).content(query)` reads
  through `zerodds_dr_{read,take}_next_instance_w_condition`; a content filter
  together with `instance(h)` is refused, DDS has no instance-bound read with a
  condition. `StatusCondition::entity()` returns a `dds::core::Entity` that
  forwards `enable`, `status_changes`, `instance_handle`, `close` and `retain`
  to the entity.

- **`ReadCondition` and `QueryCondition` as DDS-PSM-Cxx 1.0 §7.5.10.6/7 have
  them (finding 4, part).** Both are classes, not templates: `ReadCondition(dr,
  DataState)`, with an optional functor, `state_filter()`, `data_reader()`;
  `QueryCondition(Query, DataState)` with `expression()`, the parameter
  iterators and `parameters_length()`. `dispatch()` calls the functor when the
  condition holds, with the condition as argument if the functor takes one.
  `Query::data_reader()` returns the `AnyDataReader` the query was made for
  and replaces `native_reader_handle()`. `QueryCondition::expression(str)`,
  `parameters(..)` and `add_parameter` throw `UnsupportedError`: the C API has
  no call that changes a query condition. `StatusCondition::entity()` is not
  implemented, the entity classes do not derive from `dds::core::Entity`.

- **Vendor interfaces left the namespaces DDS-PSM-Cxx 1.0 §2.2 reserves
  (finding 17, second part).** `check_status`, `DataRepresentationKind`,
  `cdr_lite` and `Fixed` moved to `zerodds::psm`
  (`zerodds/psm/check_status.hpp`, `type_support.hpp`, `Fixed.hpp`);
  `dds/core/Fixed.hpp` is gone. `dds::topic::core` no longer exists, so
  `core::Time` inside `namespace dds::topic` finds `dds::core` again.
  `DomainParticipantFactory`, the `ignore_*` members, `discovered_*_count`,
  `native_handle` and `participant_handle` are removed from the public
  classes; `default_participant_qos` stays on `DomainParticipant`.
  `StatusCondition` takes an entity (`StatusCondition(const E&)`) instead of
  `void*`; the conditions and `WaitSet` have no public `native_handle`.
  The generator `zerodds-idlc` now emits `::zerodds::psm::DataRepresentationKind`.

- **Entity classes no longer carry vendor members in their public API
  (finding 17, first part).** `borrow`, `native_handle`, the per-entity
  `*_handle` accessors, `wait_for_matched` and `native_cft_handle` are removed
  from the DDS-PSM-Cxx wrapper classes. Vendor access goes through
  `operator->` / `delegate()`; the shared helper is `core/detail/borrow.hpp`.
  `DataReader::listener(l, mask)` and `listener()` stay.

### Added

- **`zerodds-idlc generate --lang c++ --cpp-mapping psm` (finding 15).** The mapping of
  topic types DDS-PSM-Cxx 1.0 §8.1 asks for, selectable beside the existing one, which stays
  the default. Structures are classes with the accessors of §7.4.6, an all-members
  constructor, `swap` and `==`; a union is a class with `_d()` and one accessor per case
  (a setter selects its case); `wchar` is `wchar_t`, `wstring` `std::wstring`, `long double`
  `long double`, `map` `std::map`, an array `dds::core::array`, `@optional`
  `dds::core::optional`; enumerations, bitmasks and bitsets keep the native type under the
  IDL's name. The wire codec, the TypeObject and the endpoint gates are the native ones: the
  native types move into the namespace `zd_native`, and `zerodds::psm::Conv<P>`
  (`zerodds/psm/convert.hpp`) converts between a PSM class and its native type; the
  `topic_type_support` of a PSM class forwards to the native one through it. A PSM sample
  costs one conversion on `write` and one on `take`. `long double` converts through IEEE
  binary128 and rounds to the width of the platform's `long double`. Not remapped:
  exceptions, value types and constants stay with the native types; the spelling `@Optional`
  and `@Shared` of the PSM text is not recognised, because IDL 4.2 §8.2 makes annotation
  names case-sensitive and the wire carries `@optional`.
  Test: `generated_psm_roundtrip.cpp` from `tests/generated/psm_probe.idl`, driven by
  `generated_type_via_cargo.rs`.

- **Generated structures have `swap` (finding 15, part).** `zerodds-idlc
  generate --lang c++` (`zerodds-lang-rules`, `adapter/cpp.rs`) writes a
  member `swap(T&)` and an `inline swap(T&, T&)` in the type's namespace
  (DDS-PSM-Cxx 1.0 §7.12). A structure with a member spelled `swap` gets
  neither. Not changed, with the reason: `wchar`/`long double`/`wstring`/`map`
  (the codec's framing and widths fix them), accessor functions instead of
  public fields and `dds::core::optional`/`dds::core::array` instead of
  `std::optional`/`std::array` (the generated wire codec reads the fields and
  the `std` types directly), `@Optional`/`@Shared` spelling (IDL 4.2 §8.2 makes
  annotation names case-sensitive, and PSM §8.1.4 writes them as comments).

- **The headers DDS-PSM-Cxx 1.0 §7.2 lists and the library lacked (finding 9).**
  `dds/core/macros.hpp`, `conformance.hpp` (the `OMG_DDS_*_SUPPORT` switches:
  content subscription, ownership and persistence are defined), `Optional.hpp`
  (`dds::core::optional<T>`), `array.hpp` (`dds::core::array<T, N>`),
  `dds/pub/CoherentSet.hpp`, `SuspendedPublication.hpp`, `sub/CoherentAccess.hpp`
  (all three begin in the constructor, end in `end()`/`resume()` or the
  destructor), `topic/BuiltinTopicKey.hpp`, and `dds/{pub,sub,topic}/find.hpp`
  with an entity tracker: publishers and subscribers keep weak references to
  the writers and readers they created, `find<DataWriter<T>>`,
  `find<AnyDataWriter>`, the reader forms by topic name, by topic description
  and by `DataState`, and `topic::find`. `topic::discover<TOPIC>(dp, name,
  timeout)` waits for a discovered topic.
- Not implemented from those headers: the four `topic::discover` forms
  without a participant argument (the specification gives them none to read
  from), the iterator range form of `matched_subscriptions`, the `T*`
  template headers (`TCorePolicy.hpp` and the like) and `dds/core/xtypes/**`.
- `BuiltinTopicKey::value()` is `dds::core::array<uint8_t, 16>`: DDS 1.4
  declares three implementation-defined integers, a GUID has sixteen octets.
  `ParticipantBuiltinTopicData` has `key()` (was `guid()`) and a `UserData`
  policy as `user_data()`.

- **`dds::domain::find`, `dds::domain::ignore`, `dds::pub::ignore`,
  `dds::sub::ignore`, `dds::topic::ignore`** (single handle and iterator
  range). `find` reads a registry of weak references to the participants the
  process created.

- **Time and Duration as DDS-PSM-Cxx 1.0 §7.5.6 has them.** Both have
  `from_microsecs`, `from_millisecs`, `from_secs(double)`, `to_microsecs`,
  `to_millisecs`, `to_secs`, `compare` and the six comparison operators, `+=`,
  `-=`, `+`, `-`; `Duration` has `*` and `/` by a number. `Time::sec()` is
  `int64_t`. `Duration::from_millis` and `to_millis` are now `from_millisecs`
  and `to_millisecs`; `Duration::is_infinite()` is gone (compare with
  `Duration::infinite()`, which also accepts the C API's spelling of
  infinite). `Duration` arithmetic saturates at the infinite duration;
  conversions that leave the `int32_t` seconds throw `std::overflow_error`.
- **`dds::core::QosProvider`** (DDS-PSM-Cxx 1.0 §7.6.2.1; also
  `dds::core::qos::QosProvider`) — loads the DDS-XML QoS profiles of a
  `file:` URI and gives `participant_qos`, `topic_qos`, `publisher_qos`,
  `subscriber_qos`, `datawriter_qos` and `datareader_qos` by profile id or
  for the provider's profile; another URI scheme is an `UnsupportedError`, a
  profile the document lacks an `InvalidArgumentError`. `test_qos_provider`
  in `smoke_dds_psm.cpp`.
- **Publisher, Subscriber, DataWriter, DataReader, their listeners and
  AnyDataWriter/AnyDataReader as DDS-PSM-Cxx 1.0 §7.5.3, §7.9 and §7.10 have
  them.** `Publisher` and `Subscriber` have `qos()`, `<<`/`>>`, the default
  QoS of their endpoints (`default_writer_qos`, `default_datareader_qos`),
  `listener(l, mask)`, `participant()`, `notify_datareaders()` and the
  constructors with listener and mask. `DataWriter<T>` has `write` with an
  instance handle, with a `TopicInstance`, over iterators, the stream
  operators, `register_instance`, `unregister_instance(handle)`,
  `dispose_instance(handle)` (they took a sample), `key_value`, `qos()`,
  `topic()`, `publisher()`. `DataReader<T>` has `qos()`, `subscriber()`,
  `topic_description()`, `key_value`, `lookup_instance` and the statuses as
  const references. The listener classes are the normative ones: pure
  virtual `AnyDataWriterListener`, `PublisherListener`, `DataWriterListener<T>`,
  `AnyDataReaderListener`, `SubscriberListener` (not a template),
  `DataReaderListener<T>`, `AnyTopicListener`, `TopicListener<T>`,
  `DomainParticipantListener`, each with a `NoOp` class to derive from. A
  Publisher or Subscriber listener hears the statuses of its writers and
  readers, through `AnyDataWriter` and `AnyDataReader`. Limits: the C API's
  participant listener carries `on_inconsistent_topic` and
  `on_data_on_readers` only, so a DomainParticipantListener hears those two;
  `key_value` answers for instances this object registered or returned a
  sample of, and `DataReader::lookup_instance` needs a `key_hash` in the type
  support.
- **Topic, ContentFilteredTopic, AnyTopic and AnyTopicDescription as
  DDS-PSM-Cxx 1.0 §7.5.3 and §7.8 have them.** `Topic<T>` takes a `const
  DomainParticipant&`, has the constructors with a type name and with QoS,
  listener and mask, `listener(l, mask)` installs the listener with the C
  API (it stored the pointer and did nothing else), and
  `inconsistent_topic_status()` is const. `ContentFilteredTopic<T>(topic,
  name, Filter)` replaces the constructor with participant, expression and
  parameter list; `topic()` replaces `related_topic()`. `dds::topic::Filter`
  and `dds::topic::TopicInstance<T>` are new. `AnyTopic` holds a `Topic<T>` or
  a topic known by its C handle and has `domain_participant()`,
  `inconsistent_topic_status()`, `qos()`, `get<T>()` (`InvalidDowncastError`
  for another type) and `operator->`; `DomainParticipant::find_topic` and
  `lookup_topicdescription` return it (they returned an empty one), and the
  listener callback of the participant passes the topic and its status (an
  empty AnyTopic and a zero status). `AnyTopicDescription` is new.
  `TopicDescription::domain_participant()` was declared and not defined.
  `test_topic_any_topic_and_listener` in `smoke_dds_psm.cpp`.
- **QoS as DDS-PSM-Cxx 1.0 §7.6 has it.** The policies are in
  `dds::core::policy` with `safe_enum` kinds (`HistoryKind::KEEP_LAST`,
  `ReliabilityKind::BEST_EFFORT`), static helpers (`Reliability::Reliable()`,
  `History::KeepLast(10)`, `Durability::Transient()`,
  `Presentation::GroupAccessScope()`), getters, reference getters and
  chainable setters, `policy_id<P>` and `policy_name<P>`. The QoS classes
  `dds::{topic,pub,sub,domain}::qos::*Qos` are `TEntityQos` containers with
  `policy<P>()`, `qos << Policy` and `qos >> policy`, and take the policies
  another kind shares (`DataWriterQos dwqos(tqos)`). `DataWriterQos` and
  `DataReaderQos` no longer have PARTITION, PRESENTATION, GROUP_DATA and
  TOPIC_DATA: those are the Publisher's, Subscriber's and Topic's.
  `Topic::qos()` and `DomainParticipant::qos()` read the QoS back from the C
  API; `default_participant_qos()` returned the default constructed one.
  `Reliability::Reliable_` is `Reliability::Reliable`. `dds/core/qos.hpp` and
  the `dds::core::*Qos` classes are gone. Not mapped: DATA_REPRESENTATION and
  TYPE_CONSISTENCY_ENFORCEMENT, which the C QoS structs do not carry.
  `test_qos_composition` in `smoke_dds_psm.cpp`.
- **Status classes, `SampleInfo` and samples as DDS-PSM-Cxx 1.0 §7.5.4 and
  §7.12 have them.** The status classes have constructors and the accessors
  of §7.4.6 instead of public fields (`dds/core/status/Status.hpp`);
  `OfferedIncompatibleQosStatus` and `RequestedIncompatibleQosStatus` carry
  `policies()`, the count per policy, and `last_policy_id()` is a
  `QosPolicyId`. `StatusMask` and `SampleRejectedState` are bit sets with the
  named masks of the normative header (`dds/core/status/State.hpp`);
  `StatusCondition::enabled_statuses` takes a `StatusMask`. `SampleInfo` has
  `timestamp()`, `state()`, `generation_count()`, `rank()`, `valid()`,
  `instance_handle()` and `publication_handle()`; `SampleState`, `ViewState`,
  `InstanceState` and `DataState` are the state classes. `LoanedSamples<T>` is
  move-only with `cbegin()`/`cend()`, a namespace level `move`, `begin`, `end`;
  `SharedSamples<T>` is new. `DataReader::read`/`take` have the iterator
  forms, `select()` with `instance`, `next_instance`, `state`, `content`,
  `max_samples`, the streaming operators with `read`, `take`, `max_samples`,
  `state`, `content`, `instance`, `next_instance`, and
  `default_filter_state`. `Query` is a class of its own, not a template.
  `take(max_samples)` and `read(max_samples)` are gone; `select().max_samples(n)`
  replaces them. Tests: `test_samples_api`, `test_entity_operations`,
  `test_default_qos_matches_the_specification` in `smoke_dds_psm.cpp`.
- **`enable`, `status_changes`, `instance_handle` of every entity call the C
  API** (DDS 1.4 §2.2.2.1.1.6 to §2.2.2.1.1.8): DomainParticipant, Topic,
  Publisher, Subscriber, DataWriter, DataReader. They returned `0` or a nil
  handle, and `DataWriter::enable` called a C function that does not exist, so
  the headers did not compile. `Topic::qos(q)` and
  `Topic::inconsistent_topic_status()` reach the C API too.
  `test_entity_operations` in `smoke_dds_psm.cpp`.

- **`dds::topic::TopicBuiltinTopicData`** (`dds/topic/BuiltinTopic.hpp`) holds
  every member DDS 1.4 §2.2.5 lists for a `DCPSTopic` sample, each as its
  policy class: `durability()`, `durability_service()`, `deadline()`,
  `latency_budget()`, `liveliness()`, `reliability()`,
  `transport_priority()`, `lifespan()`, `destination_order()`, `history()`,
  `resource_limits()`, `ownership()` and `topic_data()`, beside `key()`,
  `name()` and `type_name()`. It is built by `from_native` from
  `zerodds_ZeroDdsTopicBuiltinTopicData`, as the two endpoint classes are
  from theirs. The class held four public fields — a key, two names and two
  `uint32_t` kinds — and nothing filled it.

- **`dds::pub::matched_subscriptions`, `dds::pub::matched_subscription_data`,
  `dds::sub::matched_publications`, `dds::sub::matched_publication_data`**
  (DDS-PSM-Cxx, `dds/pub/discovery.hpp`, `dds/sub/discovery.hpp`). The
  data comes back as `dds::topic::PublicationBuiltinTopicData` and
  `SubscriptionBuiltinTopicData`, which now hold each member DDS 1.4
  §2.2.5 lists as its policy class, durations to the nanosecond; they
  held whole seconds and nothing filled them. A handle that names no
  matched endpoint throws `dds::core::InvalidArgumentError`.

- **`zerodds::Writer::dispose/unregister_instance/unregister_with_dispose(const uint8_t *sample, std::size_t len)`**
  and **`zerodds::TypedWriter<T>::dispose/unregister_instance/unregister_with_dispose(const T &value)`**
  — dispose/unregister lifecycle markers derived from the sample/value
  itself, over the new C ABI `zerodds_writer_dispose_sample`/
  `_unregister_sample`/`_unregister_with_dispose_sample`. The existing
  `key_hash[16]`-taking overloads send `PID_KEY_HASH` alone, a form
  Eclipse Cyclone 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 were all
  measured not to act on as a dispose (OMG dds-rtps
  `Test_FinalInstanceState_1`, 0/4); the new overloads derive the real
  KeyHash and a `serializedKey` from the encoded sample through the
  writer's registered `TypeObject`, the form all three (and RTI Connext/
  CoreDX) accept (4/4). `TypedWriter<T>` requires a typed-create writer
  with a keyed type; a `create_kind` fallback writer or an unkeyed type
  throws `StatusError` with status `PreconditionNotMet`, not a silent
  fallback. The `key_hash[16]` overloads stay, documented with which
  vendors ignore that form.

### Changed

- One CDR codec, not two. `include/dds/topic/xcdr2.hpp` and `xcdr2_md5.hpp` — a
  hand-rolled XCDR2 (and MD5-for-KeyHash) implementation the codegen no longer
  called — are removed. `topic_type_support<T>::decode`'s third parameter is
  now `zerodds::wire::Encoding` (`runtime/cpp/include/zerodds/wire.hpp`)
  instead of a second, `xcdr2.hpp`-local `XcdrVersion` enum; the extensibility
  tag `dds::topic::core::policy::DataRepresentationKind` moved to
  `include/dds/topic/TopicTraits.hpp`, unchanged name and namespace.
  `zerodds-cpp` now links `zerodds::zerodds-wire-cpp`
  (`runtime/cpp/CMakeLists.txt`, new) for the four translation units that
  implement `zerodds::wire::Writer`/`Reader`.
- `zerodds-idlc generate --lang c++` no longer emits
  `#include "dds/topic/xcdr2.hpp"`; a generated file's `decode` refuses the
  encoding it was not generated for by the name `zerodds::wire::Encoding`
  gives it (e.g. `PlainCdr2`), not `XcdrVersion`'s.
- No deprecated alias for either removed header or for `XcdrVersion` — rc
  means the contract is fixed properly, not grown a compatibility shim.
- `dds::core::Duration` now uses the DDS 1.2 infinite sentinel
  `{0x7fffffff, 0x7fffffff}` (DDS 1.2 IDL §9.3.2) instead of
  `{0x7fffffff, 0xffffffff}`; `is_infinite()` accepts both forms. `to_millis()`
  reports whole milliseconds. `from_millis` throws `std::overflow_error` when
  the seconds component would not fit an `int32_t` instead of silently
  wrapping. `nanosec(uint32_t)` normalises a value of one second or more into
  the seconds component. `wait_for_acknowledgments`/`wait_for_matched` map an
  infinite `Duration` to `ZERODDS_BLOCKING_INFINITE` instead of
  `2^31 * 1000 - 706` ms.
- DataWriter/DataReader listener callbacks now receive the triggering entity
  with its real handle and its real status, instead of a default-constructed
  entity without a handle and an all-zero status. Each entity owns its own
  listener vtable (no thread-local table shared between entities of the same
  type), and destroying or detaching an entity unregisters it so a later
  `zerodds_poll_listeners` never reaches a freed writer/reader or vtable
  (DDS 1.2 §7.1.2.4.4, §7.1.2.5.7; DDS-PSM-Cxx 1.0 §7.3). `DataReader<T>`
  gains `sample_rejected_status()` and `requested_incompatible_qos_status()`.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-cpp` crate.

### Spec references

- C++17 RAII wrapper headers over [`zerodds-c-api`](../zerodds-c-api).
- The spec-compliant DDS-PSM-Cxx 1.0 namespace is generated by the codegen in [`zerodds-idl-cpp`](../idl-cpp).

### Public API

No Rust public API. This crate's surface lives in:

- `include/zerodds/dds.hpp` — `zerodds::Runtime`, `zerodds::Writer`, `zerodds::Reader` (move-only RAII classes).
- `examples/cpp_smoke.cpp` + `examples/build_cpp_smoke.sh` — a compilable end-to-end probe.

### Implementation

`zerodds::Runtime::Runtime(domain_id)` calls `zerodds_runtime_create`; the destructor calls `zerodds_runtime_destroy`. `Writer::write(span)` calls `zerodds_writer_write` with pointer + length; errors are propagated via `std::runtime_error("zerodds: ...")`. `Reader::take()` calls `zerodds_reader_take`, copies the `bytes` array into a `std::vector<uint8_t>`, and calls `zerodds_reader_release` for cleanup.

The classes are move-only — copy is deleted because the underlying handles must not be jointly owned.

`#![deny(unsafe_code)]` in the Rust stub `lib.rs`. The actual unsafe FFI path lives in the C++ header (via `extern "C"` declarations), not in Rust.

### Architecture

- **Layer:** 6 (PSM/Bindings).
- **Dependencies (in):** no ZeroDDS crate deps. The C++ build links at link time against `libzerodds.{so,dylib,a}` from [`zerodds-c-api`](../zerodds-c-api).
- **Dependents (out):** Apex.AI plugin builds, ROS-2 RMW backends, embedded C++ apps.
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserved).
- `publish = false` — distribution happens as a separate header tarball, not via crates.io.

### Stability

The C++ API + header ABI are RC1-stable. Major bumps happen in sync with `zerodds-c-api`.
