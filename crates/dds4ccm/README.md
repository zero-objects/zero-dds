# `zerodds-dds4ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dds4ccm/badge.svg)](https://docs.rs/zerodds-dds4ccm)

OMG DDS for Lightweight CCM 1.1 (`formal/2012-02-01`) — the normative
DDS-DCPS ports and connectors over the DCPS entities of `zerodds-dcps`:
the connector fragments of the ports, the connectors `DDS_State` and
`DDS_Event` with their bases, the DCPS entities and QoS profiles behind
them and the threading policy; the DDS-DLRL ports and connectors over the
DLRL entities of `zerodds-dlrl`. `std`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section | Module |
|------|---------|--------|
| DDS4CCM 1.1 | §7.2.2.1.1 `InstanceHandleManager`, `Writer`, `Updater` | `ccm_dds_typed`, `writer` |
| DDS4CCM 1.1 | §7.2.2.1.2 `ReadInfo`, `QueryFilter`, `Reader`, `Getter`, `Listener`, `StateListener`, Figure 7.2 | `ccm_dds`, `ccm_dds_typed`, `reader`, `listener` |
| DDS4CCM 1.1 | §7.2.2.1.3 `DataListenerControl`, `StateListenerControl` | `ccm_dds`, `listener` |
| DDS4CCM 1.1 | §7.2.2.1.4 attribute `filter`, `ContentFilterSetting` | `ccm_dds`, `ports` |
| DDS4CCM 1.1 | §7.2.2.1.5 `PortStatusListener`, `ConnectorStatusListener` | `ccm_dds`, `status` |
| DDS4CCM 1.1 | §7.2.2.2 `DDS_Write`, `DDS_Update`, `DDS_Read`, `DDS_Get`, `DDS_Listen`, `DDS_StateListen` | `ports` |
| DDS4CCM 1.1 | §7.3.1 `DDS_Base`, `DDS_TopicBase`; §7.3.2 `DDS_State`; §7.3.3 `DDS_Event` | `connector` |
| DDS4CCM 1.1 | §7.4.1 DCPS entities; §7.4.3 use of QoS profiles | `entities` |
| DDS4CCM 1.1 | §7.4.4 threading policy | `threading` |
| DDS4CCM 1.1 | §8.2.1.1 `CacheOperation`, §8.2.1.2 home receptacles, §8.2.2 composition rule, §8.3 DLRL connectors, §8.4.1 DCPS entities and §8.4.2 QoS profiles of a DLRL port | `dlrl` |
| DDS4CCM 1.1 | Annex A IDL3+ of the DDS-DCPS ports and connectors | `idl/ccm_dds.idl` (checked in `tests/annex_idl.rs`) |
| DDS4CCM 1.1 | Annex B IDL of the DDS-DLRL ports and connectors | `idl/ccm_dds_dlrl.idl` (checked in `tests/annex_idl.rs` and, with omniidl, `tests/omniidl.rs`) |
| DDS4CCM 1.1 | Annex E QoS policies of the two patterns | `profiles` (`profiles/*.xml`) |

## How the IDL maps

- An interface is a trait; a port type is a struct with its basic ports
  under the names of the specification. A basic port the component `uses`
  is an accessor (`data`, `fresh_data`, `data_control`, `filter_config`,
  `dds_entity`); one it `provides` is connected (`connect_data_listener`,
  `connect_status`).
- An `attribute` is a getter and, unless `readonly`, a `set_` method; a
  `setraises` clause is the error of the setter.
- An exception is a structure: `AlreadyCreated`, `NonExistent`,
  `InternalError` (the DDS return code and the index), `NonChangeable`.
- `in` parameters are references, `inout` and `out` parameters `&mut`; an
  operation replaces the content of the sequences it is given.

## Behaviour

- `Writer`: `write_one` by the handle or by the key; `write_many` stops at
  the first error and reports its index; with `is_coherent_write` the
  orders are between `begin_coherent_changes` and `end_coherent_changes`.
- `Updater`: the lifecycle check of all submitted instances comes before
  the first DDS order. It is local to the component, or with
  `is_global_scope` global to the data space through a DataReader of the
  topic the fragment keeps for itself. With `is_coherent_write` the write
  and dispose orders of a `*_many` operation are between
  `begin_coherent_changes` and `end_coherent_changes`; a `*_one` operation
  is one order and opens no set.
- `Reader`: reads `READ` or `NOT_READ`, `NEW` or `NOT_NEW`, `ALIVE`,
  through attribute `query` (a QueryCondition), ordered by instance.
- `Getter`: reads `NOT_READ`, `ALIVE` or `NOT_ALIVE`, through the query of
  the Reader of the port, waiting at most `time_out`, at most
  `max_delivered_data` samples.
- `Listener` and `StateListener`: the same data in push mode, by the mode
  of the listener control (`NOT_ENABLED`, `ONE_BY_ONE`, `MANY_BY_MANY`).
- `ReadInfo::instance_status` follows Figure 7.2 per instance of the port.
- `INSTANCE_FILTERED_OUT` and `INSTANCE_FILTERED_IN` are computed for the
  query of the Reader of the port, on the path of fresh data (`Getter`,
  `Listener`, `StateListener`). The first fresh sample of an instance the
  component knows that the query rejects is `INSTANCE_FILTERED_OUT` and
  comes with its data; further rejected samples of that instance, and
  rejected samples of an instance the component does not know, do not
  come. The next sample of the instance that passes is
  `INSTANCE_FILTERED_IN`. An instance outside the query exists: its
  disposal is `INSTANCE_DELETED`. The two statuses are not computed for
  the content filter of a port (attribute `filter`): a sample a
  ContentFilteredTopic rejects does not reach the DataReader.
- `StateListenerControl::is_filter_interpreted`: with TRUE
  `INSTANCE_FILTERED_OUT` is `on_deletion` and `INSTANCE_FILTERED_IN` is
  `on_creation`; with FALSE both are updates. An instance whose exit was
  told as `on_deletion` is deleted for the listener: its disposal outside
  the query is no second `on_deletion`, and its reentry is `on_creation`
  also when the attribute was set to FALSE in between.
- Attribute `filter` of a subscribing port: set before
  `configuration_complete`, which creates the DataReader on a
  ContentFilteredTopic if the filter has an expression; `NonChangeable`
  afterwards. `filter_config` changes the parameters.
- Statuses: `REQUESTED_DEADLINE_MISSED` and `SAMPLE_LOST` go to the
  `PortStatusListener` of the port; `INCONSISTENT_TOPIC`,
  `REQUESTED_INCOMPATIBLE_QOS`, `SAMPLE_REJECTED`,
  `OFFERED_DEADLINE_MISSED` and `OFFERED_INCOMPATIBLE_QOS` to the
  `ConnectorStatusListener`; the other statuses of a reader or writer to
  its `on_unexpected_status`; `DATA_AVAILABLE` stays in the fragment.

## Where the behaviour differs from a plain DDS read

- §7.2.2.1.2 defines a get as a DDS read of `NOT_READ` samples "through
  the query". The fragment reads every `NOT_READ` sample and evaluates the
  query itself: the sample the query rejects is what tells that an
  instance left the query, and a sample without data reports a deletion
  whatever the query says. Every fresh sample is therefore judged once, by
  the query the Reader has at the time of the get or the delivery, and is
  `READ` afterwards. A DDS read through a QueryCondition would leave a
  rejected sample `NOT_READ`; here a sample the query rejected is not
  gotten later under another query. A read of the Reader returns it, as
  `ALREADY_SEEN`, when its query lets it through.
- `INCONSISTENT_TOPIC`: `TopicStatusListener` is the `DDS::TopicListener`
  that calls `ConnectorStatusListener::on_inconsistent_topic`. The
  connector sets it on the topic it creates. A port completed by hand
  (`configuration_complete` with entities of the caller) is given its
  topic; the caller sets the listener there.

## Connectors (§7.3)

- `DdsBase`: `error_listener` (connect, disconnect), attributes
  `domain_id` and `qos_profile`. `DdsTopicBase` adds `topic_name` and
  `key_fields`. `DdsState<T>` and `DdsEvent<T>` are the two patterns; they
  dereference to `DdsTopicBase`, which dereferences to `DdsBase`.
- `configuration_complete` reads the QoS profile and checks the
  attributes. From then on a setter raises `NonChangeable`. A failed
  completion leaves the attributes changeable.
- `qos_profile` is "a file URL or the XML string itself": a value that
  starts with `<` is the document; `file:///path`, `file:/path` and
  `file://localhost/path` name it, with `%xx` escapes; on Windows
  `file:///C:/path` names `C:/path` (RFC 8089 Appendix E.2; not run on
  Windows); the empty value is no profile (the default QoS of DDS). A
  document may hold several profiles: the last one directly under `<dds>`
  is used, since a profile inherits only from those before it. A document
  with profiles in `<qos_library>` elements only gives the last profile of
  its last library that has one; where a document has both, the profiles
  directly under `<dds>` come first, wherever the libraries stand. The
  fragment of a URL names another profile
  (`file:///path#Profile`, `#Library::Profile`). The document is read
  with the forms of DDS4CCM that DDS-XML 1.0 lacks (profiles directly
  under `<dds>`, `domainparticipant_qos`).
- `key_fields` is checked against the data type: with the names of the
  key members where the type carries its complete `TypeObject` and has no
  base type, otherwise by whether the type has a key. The key of a topic
  is that of its type; the attribute cannot change it.
- A mirror port is a method of the connector under the name of the
  specification. A publishing role returns its port
  (`observable(&component, is_global_scope)`, `supplier(&component)`); a
  subscribing role is given the port of the component and completes it
  (`passive_observer`, `pull_observer`, `push_observer`,
  `push_state_observer`, `pull_consumer`, `push_consumer`). The fragment
  creates the topic with the connector's `TopicListener`, and the
  DataWriter or DataReader dedicated to the port.
- Connectors that use a topic of one name and data type in one
  DomainParticipant share the topic and its `TopicListener`; the last one
  removed deletes it. A topic that exists there, of another connector or
  of the application, keeps its QoS. It serves a connector when every
  policy the `topic_qos` of the connector's profile sets has the same
  value there; a policy the profile does not set is not requested.
  Otherwise the port is refused with `ConnectorError::TopicQos`, which
  names the policies that differ.
- The DataReader an `observable` with `is_global_scope` looks the
  instances up on reports to `error_listener` too. Its port has no
  `PortStatusListener`: `REQUESTED_DEADLINE_MISSED` and `SAMPLE_LOST` of
  that DataReader are reported with `on_unexpected_status`.
- One connector of a design is deployed as one connector object per
  process, with the same attributes; the fragments meet through DDS.
- `remove` deletes what the fragments created.

## DLRL ports and connectors (§8)

- `CacheOperation` is the interface of the receptacle that uses the
  Cache: the six attributes and seven operations of Annex B, each with
  the signature of the method of `zerodds_dlrl::cache::Cache` of the same
  name; `Cache` implements it. The exceptions are the variants of
  `DlrlException`; `AlreadyDeleted` comes from every operation once the
  connector removed the Cache.
- A home receptacle stands for the generated home of a class, which
  inherits from `DDS::ObjectHome`. Rust has no generated home types: the
  receptacle is the generic `ObjectHome` of its class, of the object model
  and mapping the DLRL generator of `zerodds-dlrl` makes, declared by the
  name of the class and looked up by the name of the receptacle
  (`DlrlPort::home`).
- `DlrlPortType` is a `porttype` of the composition rule: one
  `CacheOperation` receptacle and at least one home receptacle, one per
  class, under distinct IDL identifiers (also without case) that collide
  with no IDL keyword (`home`, `Uses` and `PortType` are refused).
- `DlrlConnector` dereferences to `DdsBase` and has the mirror ports it
  was declared with. `connect(mirror_port, &component, cache_usage)`
  returns the `DlrlPort` of the component: the fragment creates the Cache
  for `cache_usage`, registers a home per home receptacle, applies the QoS
  profile and enables the DCPS entities. `cache_of` gives the deployment
  the Cache of a port; `remove` deletes the Caches and the
  DomainParticipants dedicated to a port.
- QoS profile: a topic the DomainParticipant does not have is created with
  the `topic_qos` of the profile; a topic it has when the Cache creates
  the topics (of another port, also one connected at the same time, or of
  the application) keeps its QoS and serves the port when every policy the
  `topic_qos` of the profile sets has that value there
  (`ConnectorError::TopicQos` otherwise). `publisher_qos`,
  `subscriber_qos`, `datawriter_qos` and `datareader_qos` are set over the
  QoS the Cache gave the entities before they are enabled; a policy the
  profile does not set keeps the Cache's value (the DataReaders of a Cache
  are RELIABLE). `topic_filter` selects the `topic_qos`, `datawriter_qos`
  and `datareader_qos` of each topic by its name in the mapping. The
  DomainParticipant, the Publisher and the Subscriber serve all topics of
  the port: each takes the container of its kind selected for every topic
  (one without `topic_filter`, or one whose filter matches all of them),
  none when none is selected for any topic; topics that select different
  containers of one kind refuse the port with `ConnectorError::TopicFilter`.
- DCPS entities: a Publisher (`WRITE_ONLY`, `READ_WRITE`) and a Subscriber
  (`READ_ONLY`, `READ_WRITE`) of the port's own, a DataWriter and a
  DataReader per topic of its classes. The DomainParticipant is the
  container's default, else the component's, if it has what the
  `domainparticipant_qos` of the profile sets, else one dedicated to the
  port. The default Publishers and Subscribers serve no DLRL port.
- `error_listener` gets INCONSISTENT_TOPIC of the topics, the statuses of
  the DataWriters, and those of the DataReaders but DATA_AVAILABLE (which
  the Cache takes) through the listener of the Subscriber;
  REQUESTED_DEADLINE_MISSED and SAMPLE_LOST with `on_unexpected_status`.

## DCPS entities and QoS profiles (§7.4.1, §7.4.3)

- `Container` and its `Component`s hold default entities per domain: a
  DomainParticipant, a Publisher, a Subscriber. They are given
  (`set_default_entities`) or created from the QoS profile of the level
  (`set_qos_profile`) at the first port that asks. `topic_qos`,
  `datareader_qos` and `datawriter_qos` of such a profile are ignored.
- A container always has defaults, with the default QoS of DDS where its
  profile is silent. A component has a default DomainParticipant when its
  profile has a `domainparticipant_qos` or it was given one, and then a
  Publisher and a Subscriber in it; with only a `publisher_qos` or a
  `subscriber_qos` it has that default in the DomainParticipant of the
  container.
- The profile of the connector is what a port requests. A default entity
  is compatible when every policy the connector's profile sets for that
  kind of entity has the same value in the QoS of the entity; a policy the
  profile does not set is not requested. `topic_filter` selects the
  containers by the topic name of the connector.
- A port gets, in this order: the container's default Publisher or
  Subscriber with the container's DomainParticipant, if both are
  compatible; the component's, with the DomainParticipant it lives in; a
  Publisher or Subscriber dedicated to the port in the first default
  DomainParticipant that is compatible; a DomainParticipant dedicated to
  the port, with a Publisher or Subscriber in it.
- A `DDS_Update` port with `is_global_scope` has two entities, its
  DataWriter and the DataReader it looks the instances up on. Each is
  placed as above; a DomainParticipant dedicated to the port holds both.
- The DataWriter and the DataReader take their QoS from the connector's
  profile, and so does a topic the connector creates. A topic that exists
  is held to the profile as said under the connectors.
- `Container::remove` deletes the defaults the container and its
  components created, after the connectors were removed. While a port has
  its DataReader or DataWriter in a default Publisher or Subscriber, or in
  one dedicated to it in a default DomainParticipant, it returns
  `PRECONDITION_NOT_MET` and deletes nothing. A default DomainParticipant
  is deleted without what it contains: one that still has a topic stays
  the default of its domain, and a later call deletes it.
- `Container::with_runtime_config` gives the DomainParticipants the
  container creates their `RuntimeConfig`; `Container::new` reads the
  standard configuration.

## Threading policy (§7.4.4)

- A `ThreadingPolicy` is the number of threads of a `ThreadPool`. It is
  set on a subscribing port (`set_threading_policy`, before the
  configuration is complete), on a `Component` and on a `Container`.
- The facets of a port are `data_listener` and `status`. They are called
  on the pool of the port; without a port policy on the pool of the
  component; without that on the pool of the container; without any
  policy on the thread that brings the event (the thread DDS reports on,
  or the thread of the caller for data the port already holds).
- The calls of one port are made one after the other in their order, also
  on a pool of several threads.
- A policy set on a component or a container holds for the ports
  activated after it.
- `error_listener` is a receptacle of the connector, no facet of a port:
  it is called on the thread DDS reports the status on.

## QoS profiles of the patterns (Annex E)

- `profiles::STATE_TRANSFER` and `profiles::EVENT_TRANSFER` are XML
  documents for the attribute `qos_profile`, with the values of the two
  tables of their pattern: the DataWriter and Publisher of the writing role,
  the DataReader and Subscriber of the reading role, the Topic.
- A connector does not apply them by itself. PRESENTATION and
  DESTINATION_ORDER are compared between DataWriter and DataReader: a port
  with one of these profiles serves a DataReader with the default QoS, but a
  DataWriter with the default QoS does not serve it.

## The IDL of Annex A and Annex B

`idl/ccm_dds.idl` is the text of Annex A, with the types of module `DDS` it
refers to in `idl/dds_rtf2_dcps.idl`. The tests read it through the IDL
frontend of ZeroDDS and check the traits of `ccm_dds` and `ccm_dds_typed`
against it; the traits are written by hand, not generated.

`idl/ccm_dds_dlrl.idl` is the text of Annex B, the local interface
`CCM_DDS::CacheOperation`. Its `idl/dds_rtf2_dlrl.idl` includes the generic
DLRL IDL of DDS 1.2 from `zerodds-dlrl` (`dds_dlrl.idl` in its directory
`idl/`, `crates/dlrl/idl` in the repository), which must be on the include
path, after `idl/dds_rtf2_dcps.idl`; the two annexes compile in one unit.
The tests check `CacheOperation` against `DDS::Cache` of the DLRL IDL and
the trait `dlrl::CacheOperation` against the IDL (mapping in the
documentation of the trait); omniidl compiles the file in the weekly job
`integration-bridges-vendor-corba`.

## What is not covered

- The hosting of a fragment in the CCM container of `zerodds-corba-ccm`:
  the fragments run in the `Container` of this crate.
- Home receptacles of the type of a home class generated for Rust
  (`FooHome`, DDS 1.2 §8.1.6.6): no generator writes the DLRL classes for
  Rust; a home receptacle is the generic `ObjectHome` of its class.

## Example

```rust
use zerodds_dds4ccm::ccm_dds::{ListenerMode, QueryFilter, UNLIMITED};

// No query, and a listener that is not called until the component
// enables it.
assert!(QueryFilter::default().is_empty());
assert_eq!(ListenerMode::default(), ListenerMode::NotEnabled);
assert_eq!(UNLIMITED, 0);
```

## License

Apache-2.0.
