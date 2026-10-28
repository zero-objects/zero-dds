# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- The Java binding has every signature of `omg-dds-rpc-java.zip` (DDS-RPC
  1.0 §7.11.2): `RequesterParams`, `ReplierParams`, `ServiceParams` and
  `ClientParams` take no type arguments and carry `withDataWriterQos`,
  `withDataReaderQos`, `withPublisher`, `withSubscriber` and their getters,
  which the entities are created with; `Requester.getRequestDataWriter()`,
  `getReplyDataReader()`, `Replier.getRequestDataReader()`,
  `getReplyDataWriter()` and the `Class` forms of `ClientEndpoint` and
  `ServiceEndpoint` give the DDS Java PSM entities typed by the topic type
  (`org.zerodds.ddsrpc.TopicSample`, `RpcTopicTypeSupport`, `PsmEndpoint`);
  the operations take an `org.omg.dds.core.SampleIdentity`; `Sample.Iterator`
  is an `org.omg.dds.sub.Sample.Iterator`; `RequestHeader`, `ReplyHeader` and
  `RPCRuntime` have the constructors of the files; the return types are those
  of the files (`sendRequest` an `org.omg.dds.core.SampleIdentity`,
  `sendRequestAsync` and `waitFor…Async` a `java.util.concurrent.Future`, the
  iterators `org.omg.dds.sub.Sample.Iterator`), the objects the runtime's
  (`org.omg.dds.rpc.SampleIdentity`, `org.omg.dds.rpc.Future`,
  `org.zerodds.ddsrpc.RpcSampleIterator`). `SpecSignatureCheck.java` checks
  the 212 declarations of the files by reflection
  (`omg-dds-rpc-java.signatures`, written by `extract_spec_signatures.py`)
  and counts a narrower return type as a deviation.
- Java `receiveNondataSamples(true)` of `Requester` and `Replier` hands over
  the samples without data (valid_data false, DDS 1.4 §2.2.2.5.1) as
  `org.zerodds.ddsrpc.NondataSample`; `TakenSample.withoutData` carries them
  through the binding.
- Java (`runtime/java`): the endpoints of the Enhanced mapping pair by the `topic_aliases` and
  the assignability of the announced TypeInformation (§7.4.3, §7.5.1.2.6, §7.7.2.5.3,
  §7.7.2.5.5), through `RawTypeSupport.matchesByTypeInformation()` of the Java PSM. Measured
  against Rust peers over RTPS in `crates/rpc-rust-e2e/tests/java_peer.rs`: a client of the base
  interface against a service of the derived one in both directions, a changed parameter type
  and a return type widened to `long long` not matched by the Java client, a member appended to
  a `@mutable` structure matched.
- C++ tests of `runtime/cpp/include/dds/rpc.hpp` for the members of the
  normative `omg-dds-rpc-cxx` headers (signatures by `decltype`, take/read/
  wait, `Server`, `ClientEndpoint`), the exception hierarchy, the InstanceName
  bound, an Enhanced request addressed to the readers of its service
  instance, and a function-call Client against a request-reply Replier.
- `java_service_def::request_reply_pair` returns a checked `JavaTypePair`
  (a marked interface, the type of the `header` member, a `header` in one
  class only, Basic or Enhanced), the Java reader takes interface fields,
  dimensions after names and several declarators, `is_compatible` and
  `compatible_evolutions` take the `@Mutable` of the operation,
  `Replier::receive_requests` answers an undecodable payload with
  `REMOTE_EX_INVALID_ARGUMENT`, and `Requester` and `Replier` give their
  endpoints to tests (`__request_writer`, `__reply_reader`,
  `__request_reader`, `__reply_writer`).

- The Java binding: `Server` is an `RPCEntity` with `run()`, `run(Duration)`
  and `getDefaultServiceParams()`, `ClientEndpoint` gives the request writer
  and the reply reader, `Sample.Iterator` is `Closeable`, `RPCRuntime` makes
  services and clients from the generated constructors, `HeaderCodec` holds
  `InstanceName` to `string<255>`, and the generated Enhanced codec refuses a
  second member of a choice. The tests are `RpcInheritanceTest.java` and
  `EnhancedRuntimeChecks.java`.

- `RpcEndpointOptions::with_mapping`, which a binding generated for the
  Enhanced Service Mapping passes. `codegen::build_enhanced_service` lists
  the operations of the bases before the interface's own, as the example
  of §7.5.1.2.6 does.
- `codegen::build_enhanced_service` and `build_enhanced_service_with` map
  an interface under the Enhanced Service Mapping (§7.5.1.2): `@Choice
  @Autoid` `Request` and `Reply` structures over all operations with
  `remoteEx`, the `In`/`Out`/`Result` structures per operation, flat
  inheritance (§7.5.1.2.6), `@Mutable` (§7.5.1.2.1.4) and a
  `RpcError::MemberIdCollision` for an `@Autoid` hash collision. The
  `@Empty` reading of an operation without parameters is behind
  `EnhancedOptions::empty_operations`.
- `ServiceDef::inherited` and `MethodDef::mutable`; the `@Mutable`
  annotation lowers to `RpcAnnotation::Mutable`.

- `Requester`/`Replier` run the Enhanced Service Mapping:
  `with_options` takes an `RpcEndpointOptions` (`RpcMapping::Enhanced`).
  The sample is the `Request`/`Reply` structure alone; the request is
  identified by the writer GUID and sequence number of its DATA, the
  reply is written with `PID_RELATED_SAMPLE_IDENTITY` and correlated by it
  (§7.8.2); a remote exception is the `remoteEx` member (§7.5.1.2.5).
- The Enhanced endpoints announce the discovery extension of §7.6.2.1
  (related endpoint, topic aliases, the service-side instance name; none
  on the client side). The client sends no request before it has
  discovered both service endpoints and the service holds the requests of
  a client it has not discovered completely (§7.6.2.2, Figures 7.3 and
  7.4). `ServiceMatching`, `ClientMatching` and
  `RpcEndpointOptions::accept_unannounced` select the other readings.
- `RpcQos::from_service_profiles(request_url, reply_url)` resolves
  `@Qos(RequestProfile, ReplyProfile)` (§7.10.1): the reply endpoints are
  built from `RpcQos::reply_profile` when a reply profile is named. The
  generated Rust binding reads `@Qos` and offers
  `new_with_declared_qos` / `serve_with_declared_qos`.
- `type_pair::lower_type_pairs` reads a service defined by a pair of IDL
  types marked `@RPCRequestType` / `@RPCReplyType` (§7.3.1.4): struct
  types only, a `header` member of `dds::rpc::RequestHeader` (request) and
  `dds::rpc::ReplyHeader` (reply; `TypePairOptions::reply_header` selects
  the literal `RequestHeader` of the clause) in both for Basic and in
  neither for Enhanced, and the topic names from `@DDSRequestTopic` /
  `@DDSReplyTopic` on the types (§7.4.2.2).
- `Replier::activate`, `activate_async` and `Requester::activate` call the
  synchronous listener, the asynchronous listener or the reply delivery as
  the request or reply arrives (§7.9.2.2); `deactivate_listener` returns
  to pumping. `Replier::close` and `Requester::close` delete the DataReader
  and DataWriter (§7.9.3).
- `header_pair::{RequestWithHeader, ReplyWithHeader}`,
  `Requester::with_pair` and `Replier::with_pair`: a service defined by a
  pair of types that carry their own `header` (§7.3.1.4, Basic). The
  runtime puts the identity and the instance name into the request type's
  header and reads the identity from the leading octets of the sample; the
  reply type's header names the request and carries the remote exception
  code. The generated Rust binding (`emit::type_pair_rust`) writes the
  traits, the `DdsType` impls and `Req::requester` / `Req::replier`.
- `endpoint_options`, `enhanced` modules; `RpcError::ServiceNotMatched`.

### Removed

- `RpcAnnotation::RpcRequest` and `RpcReply` (`@RPCRequest`, `@RPCReply`):
  the specification defines neither.

- `codegen::build_enhanced_pair`, `build_enhanced_all` and `MethodPair`:
  they described one topic pair per operation with a header, which
  §7.4.3 and §7.5.1.2 do not.

### Changed
- Java `ServiceProxy.waitForService…` return nothing, wait, and throw
  `TimeoutError` when the wait runs out, as the machine-readable files and
  the C++ binding have them; `isServiceMatched()` asks without waiting.
  `RPCRuntime.createRequester(Class, Class, params)` and `createReplier` take
  the participant the params name, and params that name an interface as they
  are.

- `RpcQos::from_xml_profile` and `from_qos_url` follow the profile's
  `base_name` chain (DDS-XML §7.3.2.4.2, cycles and missing bases are
  `QosProfileNotFound`) and refuse a profile that sets something `RpcQos`
  cannot carry — a policy beyond reliability, durability, history, resource
  limits, lifespan and deadline, the container of an entity the RPC layer
  does not configure, `lifespan` in a reader container, a policy the writer
  and reader containers set differently — with
  `RpcError::UnmodelledQos`. `XmlProfileOptions::ignore_unmodelled`
  (`from_xml_profile_with`, `from_qos_url_with`,
  `from_service_profiles_with`) reads what it can.

- The generated Rust binding writes the XCDR1 pair (`encode_xcdr1`,
  `decode_xcdr1`) of every topic type, exception and nested declared type,
  so nested `@appendable` members have no DHEADER and nested `@mutable`
  members are parameter lists under classic CDR (XTypes 1.3 §7.4.1). A topic
  type that reaches a type without an XCDR1 form leaves the `DdsType` XCDR1
  methods at the trait's refusing defaults.

- Basic mapping: samples are classic CDR (XCDR1) by default
  (`RpcEndpointOptions::wire_encoding`, `WireEncoding::Xcdr1`; §7.5.1.1
  targets DDS 1.3). The `Call`/`Return` union is written and read where it
  stands in the sample, so an 8-octet member aligns to the sample, and the
  endpoints offer the XCDR1 data representation. `WireEncoding::Xcdr2`
  selects the earlier XCDR2 samples. The Enhanced mapping stays XCDR2.

- The topics of `Requester`/`Replier` are announced with the type names
  `${interfaceName}_Request` and `_Reply` (§7.5.1.1.6/7) instead of
  `zerodds::RawBytes`.
- Basic mapping: the `Call` union starts on a 4-octet boundary after the
  `RequestHeader` (the string of the header is variable), and an error
  reply carries the `Return` union's `default` case instead of no `data`
  (`wire_codec::ERROR_REPLY_DATA`).
- `discovery_ext` holds the types and algorithms of §7.6.2 (service
  instance name, related endpoint key, topic aliases;
  `service_matching_complete`, `client_matching_complete`); the earlier
  types with a service name and a mapping profile, and
  `client_matches_service`/`service_matches_client`, are removed.

- `evolution_rules::is_compatible` matches §7.7: the Basic mapping treats
  add/remove operation and reordering as compatible and only a signature
  change as breaking; the Enhanced mapping treats parameter/return
  add/remove/reorder as breaking (default `@final`) and type changes as
  allowed, and gains an `AddRemoveException` entry.
- `service_mapping::ServiceDef` lowers interface attributes into an
  `attributes` list and `all_methods` implies `get_attribute_<name>` and
  `set_attribute_<name>` (§7.5.1.1.3), so `codegen` includes them in the
  call and return unions. A declared operation colliding with an implied
  one is refused. The lang-rules seam and the F#/D/Zig/Swift/Go/Nim
  adapters derive an attribute's native accessor from the attribute name.
- `Requester`/`Replier` gain `with_instance_style` and
  `with_topics_style`, so the request-reply style of §7.4.1 drops the
  interface segment of the topic name; the existing constructors stay
  function-call.
- Generated Rust dispatchers answer an unknown operation with
  `REMOTE_EX_UNSUPPORTED` (§7.7.1.1) and an undeclared exception with
  `REMOTE_EX_UNKNOWN_EXCEPTION` (§7.5.2). The replier answers a failed
  reply encoding with `REMOTE_EX_UNKNOWN_EXCEPTION`, not
  `REMOTE_EX_OUT_OF_RESOURCES`.
- `function_call` opcodes are the HASH of the operation name
  (§7.5.1.1.6 rule 5a) instead of a sequential number, matching the case
  labels of the call and return unions.
- `JavaServiceDef` carries the interface name and `<service_name>`
  separately. `@DDSService(name = "…")` supplies §7.4.1's middle topic
  segment (`<interface>_<name>_Request`), no longer replacing the
  interface name; without a name the service segment is `"Service"`.
- `RemoteExceptionCode` has the six values of §7.5.1.1.1; the seventh
  `UnknownInterface` (wire value 6) is removed and `from_u32(6)` is rejected.
- The DDS-RPC annotations are pre-defined builtins (`crates/ast`'s
  `BUILTIN_ANNOTATIONS`): `@DDSRequestTopic`, `@DDSReplyTopic`,
  `@RPCRequestType`, `@RPCReplyType`, `@Qos`, `@Choice`, `@Autoid`,
  `@Empty`, `@Mutable`. A file uses them without declaring them. The RPC
  layer lowers §7.10.1's `@Qos(RequestProfile, ReplyProfile)` besides the
  legacy one-profile `@RPCInterfaceQos`.
- `SampleIdentity::RELATED_SAMPLE_IDENTITY_INVALID` is
  `{ GUID_UNKNOWN, SEQUENCENUMBER_UNKNOWN }` (§7.8.2): sequence number
  `{ high = -1, low = 0 }`, not 0. `SampleIdentity::UNKNOWN` is its alias.
- `Requester` names the identity of its own request sample in the
  `RequestHeader`: on a live runtime the RTPS GUID of the request writer and
  the sequence number the writer is about to assign (DDS-RPC 1.0 §7.2.1,
  §7.8.1), instead of a synthesized GUID and a local counter. Offline the
  synthesized GUID remains. New `Requester::__writer_rtps_guid` test helper.
- The C++ and Java request-reply bindings correlate over the explicit
  `RequestHeader::requestId` of §7.8.1 rather than over transport sample
  metadata: `DataWriter` gains `next_identity()`/`nextIdentity()`, the
  requester stamps the identity into the header before the write, and the
  replier reads the request identity from the header. A binding whose reader
  surfaces no sample identity — the ZeroDDS path, where the C ABI has no
  accessor for `PID_RELATED_SAMPLE_IDENTITY` — now correlates.
- The C++ and Java `Requester` apply §7.2.1's content-based filter: a reply
  whose related request id carries another client's request-writer GUID is
  dropped, so a client on a shared reply topic no longer receives foreign
  replies. The C++ and Java `Replier` bind strictly to their instance
  (§7.11.1.4.3): a request whose `instanceName` differs from the replier's is
  ignored, including the previously unhandled case where either name is
  empty.
- The C++ `DomainParticipant::create_writer`/`create_reader` take the DDS
  type name beside the topic name, and `Requester`/`Replier` supply it from
  `dds_type_traits<T>::type_name()`. The ZeroDDS binding no longer announces
  the topic name as the type name, so a request topic publishes
  `${interfaceName}_Request`/`_Reply` (§7.5.1.1.6/7).
- `Replier::tick` binds every request to its instance (DDS-RPC 1.0 §7.6.2,
  §7.11.1.4.3): the request's instance name must equal the replier's, so a
  named replier no longer answers requests with no instance name.
- The `Replier` provides all three §7.9.2.2 ways: the synchronous
  `ReplierHandler`/`tick`, the asynchronous `AsyncReplierHandler`/
  `dispatch_async` (the callback gets the replier and the identity), and
  polling through `receive_requests`/`send_reply`/`send_exception`. Ways 2
  and 3 hand the request `RequestIdentity` of §7.8.2.1 to the application.
- `RpcQos::from_qos_url` resolves a §7.10.1 `@Qos` profile URL
  (`[file://]path[#library]`, `file` the default and only supported
  protocol): it reads the DDS4CCM XML and applies the named library's
  profile. The runtime accessors to change mutable policies remain.
- `RpcQos::default_basic`/`default_enhanced` use `HistoryKind::KeepAll`
  instead of `KeepLast`, as Spec §7.10.2 requires for the default QoS of the
  RPC endpoints. `DEFAULT_HISTORY_QOS` replaces the two depth constants.
- `RequestHeader::to_cdr_le`/`to_cdr_be` and
  `wire_codec::encode_request_frame` return `RpcResult` and reject an
  `instance_name` longer than `string<255>` (Spec §7.5.1.1.1);
  `MAX_STRING_LEN` is 255.
- `service_mapping::lower_service` refuses the IDL constructs the
  service-definition grammar of §7.3.1.1 removed: `import`,
  `typeid`/`typeprefix`, CCM and non-plain interface forms, declarations
  of types/constants/exceptions inside an interface body, the `oneway`
  modifier (and `@oneway`) and `context` clauses. A constant in an
  interface body no longer vanishes silently and a `oneway` operation no
  longer loses its reply structure and Return-union case. The new
  `RpcError::OutsideServiceGrammar` names the first construct found.
- The Java `DomainParticipant` seam gains `deleteWriter`/`deleteReader`,
  and `Requester.close()`/`Replier.close()` call them, so deactivation
  deletes the underlying DataWriter and DataReader as §7.9.3 requires.
- `Requester::set_qos` and `Replier::set_qos` apply an `RpcQos` to the
  live endpoints, so the mutable QoS policies can be changed at run time
  as §7.10.2 requires; a refused (immutable or inconsistent) policy is
  returned as `RpcError::Dcps`.
### Fixed
- `Replier::activate` and `activate_async` hand a request held for its client (§7.6.2.2.1,
  Figure 7.3) on once the client is matched: while requests are held, a thread of the replier
  runs the pass every 5 ms; it ends when none is held, the listening stopped or the replier is
  gone. Before, only an arriving request made the replier look at them again
  (`crates/rpc-rust-e2e/tests/replier_activate_client_matching.rs`: held 1, handled 0).
- Java (`runtime/java`): `RpcTopicTypeSupport`, the typed DataWriter/DataReader views of §7.11.2,
  answers `matchesByTypeInformation` as the announced `RawTypeSupport` does; its endpoints paired
  by the type name (`RpcTopicTypeSupportTest`, `java_peer.rs`).
- Java (`runtime/java`): a replier whose listener runs on arrival hands a request it held back for
  its client (§7.6.2.2.1, Figure 7.3) on once the client is matched; the match completes by
  discovery, which delivers no sample, and the request stayed held (measured: Rust client of a
  base interface against a Java service of the derived one under load). Test
  `ReplierClientMatchingTest`.

- `RpcQos::from_xml_profile` read the named profile without its
  `base_name` chain, so policies of a base profile were missing
  (DDS-XML 1.0 §7.3.2.4.2). It resolves the chain through
  `zerodds_xml::resolve_profile` now; `from_resolved_default` takes the
  merged profile.
- `RpcQos::from_profile_default`/`from_resolved_default` read a
  profile's single `<datawriter_qos>`/`<datareader_qos>` container. A
  profile may hold several, filtered per topic name (DDS-XML 1.0
  §7.3.2.4.3); RPC endpoints carry no topic name, so the filterless
  container applies, else the first in document order
  (`zerodds_xml::default_qos`).
- `RpcQos::from_xml_profile` reported every failure of the profile
  resolution as `QosProfileNotFound`, including a circular `base_name`
  chain and a base defined after its derived profile. Those are
  `QosProfileInvalid { path, reason }` now; only an unknown library or
  profile is `QosProfileNotFound`.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-rpc` crate.

### Spec references

- **OMG DDS-RPC 1.0** (`formal/16-12-04`):
  - §7.3 IDL annotations (`@service`, `@oneway`, `@in`, `@out`, `@inout`).
  - §7.4 service mapping (IDL → ServiceDef/MethodDef/ParamDef).
  - §7.5 common types (RequestHeader, ReplyHeader, SampleIdentity, RemoteExceptionCode_t) + member hash (§7.5.4).
  - §7.6 evolution rules + compatibility mappings.
  - §7.7 function_call / dispatch_request.
  - §7.8 topic naming, request identity, discovery extensions.
  - §7.9 requester API.
  - §7.10 replier API.
  - §7.11 QoS profile (foundation + enhanced).
- Coverage status: `docs/spec-coverage/dds-rpc-1.0.md` — 94 done / 0 partial / 0 open / 10 n/a.

### Public API

**Foundation:**
- `RequestHeader`, `ReplyHeader`, `SampleIdentity`, `RemoteExceptionCode`, `MAX_HEADER_BYTES`, `MAX_STRING_LEN`.
- `topic_naming::{request_topic_name, reply_topic_name, validate_service_name, REQUEST_SUFFIX, REPLY_SUFFIX, ServiceTopicNames}`.
- `annotations::{LoweredRpc, RpcAnnotation, lower_rpc_annotations}`.
- `service_mapping::{ServiceDef, MethodDef, ParamDef, ParamDirection, TypeRef, lower_service}`.
- `codegen::{ServiceLayout, RequestType, ReplyType, MethodPair, CallUnionDef, CallUnionCase, MemberType, StructMember, build_basic_pair, build_enhanced_pair, build_enhanced_all}`.
- `rpc_hash::rpc_member_hash`.

**Runtime:**
- `Requester<TIn, TOut>::{new, with_instance, send_request_blocking, send_request_async, tick}`.
- `Replier<TIn, TOut>::{new, with_instance, tick}` + `ReplierHandler` trait + `FnHandler` adapter.
- `RpcEndpointBuilder`, `RequesterEndpoint`, `ReplierEndpoint`.
- `RpcQos::{default_basic, default_enhanced, from_xml_profile, request_writer_qos, request_reader_qos, reply_writer_qos, reply_reader_qos}`.
- Constants: `DEFAULT_BASIC_HISTORY_DEPTH = 10`, `DEFAULT_ENHANCED_HISTORY_DEPTH = 64`, `DEFAULT_RESOURCE_LIMITS`.
- `wire_codec::{encode_request_frame, decode_request_frame, encode_reply_frame, decode_reply_frame}`.

**Cross-Cutting:**
- `discovery_ext::{PublicationBuiltinTopicDataExt, SubscriptionBuiltinTopicDataExt, ServiceMappingProfile, client_matches_service, service_matches_client}`.
- `function_call::{FunctionStub, FunctionSkeleton, OperationDescriptor, ServiceDescriptor, dispatch_request}`.
- `evolution_rules::{Evolution, Mapping, compatible_evolutions, is_compatible}`.
- `request_identity::RequestIdentity`.

**Errors:** `RpcError`, `RpcResult`.

### Implementation

Common types are encoded via XCDR2-Final (Spec §7.5.1.1) and match byte-exactly with the RTI/Cyclone-DDS reply wire. `SampleIdentity` (16-byte writer GUID + 8-byte sequence number) is the correlation token between request and reply — the replier sets it in `ReplyHeader::related_request_id` and the requester routes the response via an `mpsc::Sender` pending slot.

`Requester` is synchronous + tick-driven: `send_request_blocking` calls `tick` in a polling loop until reply or timeout. Callers with their own event loop can use `send_request_async`, which only sends the request and returns an `mpsc::Receiver`.

`Replier` carries a `ReplierHandler` trait — apps provide a closure (`FnHandler::new(|req| reply)`) or their own trait impl. `dispatch_request` (Spec §7.7) makes the function-call path from the IDL type to the handler method.

`RpcQos` brings two foundation defaults (`default_basic` with KeepLast(10), `default_enhanced` with KeepLast(64)) and an XML profile resolver — profiles under `library::profile` are merged with the defaults, so that policies not specified in the XML fall back to the spec default.

A process-global instance registry prevents duplicates of `(participant, role, service, instance)` (Spec §7.6.2). Anonymous instances (`instance_name = ""`) allow multiple registration as the default instance.

`Codegen` produces basic + enhanced request/reply struct pairs along with the `CallUnion` discriminator type for the server-side match (Spec §7.5.1.3).

`forbid(unsafe_code)` is set.

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** `zerodds-dcps`, `zerodds-idl`, `zerodds-qos`, `zerodds-rtps`, `zerodds-types`, `zerodds-xml`.
- **Dependents (out):** end-user RPC apps, `crates/rmw-zerodds-shim` (ROS-2 service path), bridges (`grpc-bridge` does its own wire paths).
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserve hook).

### Stability

Public API + wire format RC1-stable. Major bumps for breaking changes of the spec wire form.
