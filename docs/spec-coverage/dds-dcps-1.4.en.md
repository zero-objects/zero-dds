# DDS DCPS 1.4 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-dcps-1.4.pdf` (180 pages, OMG formal/2015-04-10) —
[OMG DDS 1.4](https://www.omg.org/spec/DDS/1.4/)

Item-by-item audit against the PDF following `internal/spec-coverage/PROCESS.md`.
Page numbers are PDF pages (PDF page = printed page + 12). Every operation,
every return-code and precondition rule, every QoS field, every status field
and every field of a built-in topic is an item of its own. `done` stands only
where code and test cover the requirement; a test citation names the test
function as `file::module::name`.

**Context:** The DCPS side of ZeroDDS is spread over:

- `crates/dcps/` — the Rust API (`DomainParticipantFactory`,
  `DomainParticipant`, `Topic`, `ContentFilteredTopic`, `MultiTopic`,
  `Publisher`, `DataWriter`, `Subscriber`, `DataReader`, conditions, `WaitSet`,
  listeners, statuses, `SampleInfo`, built-in topics) and the runtime
  (`runtime.rs`) that binds it to RTPS.
- `crates/qos/` — the QoS policies with defaults, compatibility rules and
  consistency checks.
- `crates/sql-filter/` — the filter and query language of Annex B.
- `crates/zerodds-c-api/` — the C binding. It is cited here only where the Rust
  API does not offer a PIM operation; its other behaviour is not the subject of
  this audit.

The Rust API is not a mapping of the IDL PSM of §2.3: return codes are
`Result<T, DdsError>`, collections are `Vec`, a type is passed as the type
parameter `T: DdsType`. Where the Rust signature lacks a parameter of the PIM
(such as listener and mask on the `create_*` operations), that is listed as a
deviation.

**Open items:** none; every item is `done` or `n/a (informative)`.

---

## §1 Overview

### §1.1 Introduction

**Spec:** §1.1, p. 13 — "This specification defines both the Application
Interfaces (APIs) and the Communication Semantics (behavior and quality of
service) that enable the efficient delivery of information from information
producers to matching consumers." Design goals of the interfaces follow
(pre-allocation, no unbounded resources, few copies, typed interfaces, QoS as a
list of independent policies, separation of the publishing and the subscribing
side).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — purpose and design goals of the
specification; they address the design of the interfaces, not an
implementation. The requirements are in §2.2 and §2.3 and listed there one by
one.

### §1.2 Purpose

**Spec:** §1.2, pp. 13–14 — application domains, the model of the “global data
space”, reference to DDSI-RTPS for interoperability on the wire.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background and scope; no requirement. The
wire side is covered in `ddsi-rtps-2.5.en.md`.

---

## §2.1 Summary

### §2.1 Summary

**Spec:** §2.1, p. 15 — "The description is organized into two sub clauses: The
Platform Independent Model (PIM). The Platform Specific Model (PSM) for the OMG
IDL platform based on the PIM."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure of the specification.

---

## §2.2.1 Overview and Design Rationale

### §2.2.1.1 Format and Conventions — class tables

**Spec:** §2.2.1.1, pp. 15–16 — form of the class tables; parameters without a
modifier are “in”; "<type> [ ]" denotes a collection whose mapping the PSM
decides.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notation rule of the specification.

### §2.2.1.1 ReturnCode_t — mapping by the PSM

**Spec:** §2.2.1.1, p. 16 — "At the PIM level we have modeled errors as
operation return codes typed ReturnCode_t. Each PSM may map these to either
return codes or exceptions."

**Repo:** `crates/dcps/src/error.rs` (`DdsError`, `Result`,
`DdsError::as_return_code`, module `return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`,
`crates/dcps/src/error.rs::tests::rc_error_maps_for_wire_and_transport_and_other`

**Status:** done — errors are `Err(DdsError)`, success is `Ok`;
`as_return_code` yields the numeric value of the IDL PSM.

### §2.2.1.1 ReturnCode_t OK

**Spec:** §2.2.1.1, p. 17 — "OK Successful return."

**Repo:** `crates/dcps/src/error.rs` (`Result::Ok`, `return_code::OK`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`

**Status:** done

### §2.2.1.1 ReturnCode_t ERROR

**Spec:** §2.2.1.1, p. 17 — "ERROR Generic, unspecified error."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Other`, `DdsError::WireError`,
`DdsError::TransportError` → `return_code::ERROR`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_error_maps_for_wire_and_transport_and_other`

**Status:** done

### §2.2.1.1 ReturnCode_t BAD_PARAMETER

**Spec:** §2.2.1.1, p. 17 — "BAD_PARAMETER Illegal parameter value."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::BadParameter`,
`DdsError::Configuration` → `return_code::BAD_PARAMETER`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_bad_parameter_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t UNSUPPORTED

**Spec:** §2.2.1.1, p. 17 — "UNSUPPORTED Unsupported operation. Can only be
returned by operations that are optional."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Unsupported` →
`return_code::UNSUPPORTED`); returned among others by
`crates/dcps/src/publisher.rs` (`DataWriter::get_key_value`) and
`crates/dcps/src/subscriber.rs` (`DataReader::get_key_value`,
`DataReader::decode_or_keyholder`).

**Tests:** `crates/dcps/src/error.rs::tests::rc_unsupported_maps`,
`crates/dcps/tests/unsupported_qos.rs::a_data_representation_the_service_does_not_emit_is_unsupported`

**Status:** done — the code is mapped, and an operation returns it only for a value the service does not support (DATA_REPRESENTATION other than XCDR1, XCDR2 and XML). `get_key_value` no longer returns `Unsupported` (§2.2.2.4.2.9, §2.2.2.5.3.29); the other sources of `DdsError::Unsupported` are the optional flat-data binding and an operating system error.

### §2.2.1.1 ReturnCode_t ALREADY_DELETED

**Spec:** §2.2.1.1, p. 17 — "ALREADY_DELETED The object target of this
operation has already been deleted."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::AlreadyDeleted`);
`crates/dcps/src/entity.rs` (`EntityState::mark_deleted`,
`EntityState::check_not_deleted`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_already_deleted_maps`,
`crates/dcps/src/entity.rs::tests::check_not_deleted_returns_already_deleted_after_mark`

**Status:** done — the code is mapped; where it is returned is listed under the
rule “use of a deleted entity” below.

### §2.2.1.1 ReturnCode_t OUT_OF_RESOURCES

**Spec:** §2.2.1.1, p. 17 — "OUT_OF_RESOURCES Service ran out of the resources
needed to complete the operation."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::OutOfResources`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_out_of_resources_maps`,
`crates/dcps/src/condition.rs::tests::waitset_attach_above_max_returns_out_of_resources`

**Status:** done

### §2.2.1.1 ReturnCode_t NOT_ENABLED

**Spec:** §2.2.1.1, p. 17 — "NOT_ENABLED Operation invoked on an Entity that is
not yet enabled."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::NotEnabled`);
`crates/dcps/src/entity.rs` (`EntityState::check_enabled`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_not_enabled_maps`,
`crates/dcps/src/entity.rs::tests::check_enabled_returns_not_enabled_for_disabled_entity`

**Status:** done — the code is mapped; which operations return it is listed
under §2.2.2.1.1.7.

### §2.2.1.1 ReturnCode_t IMMUTABLE_POLICY

**Spec:** §2.2.1.1, p. 17 — "IMMUTABLE_POLICY Application attempted to modify
an immutable QosPolicy."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::ImmutablePolicy`);
`crates/dcps/src/entity.rs` (`immutable_if_enabled`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_immutable_policy_maps`,
`crates/dcps/src/entity.rs::tests::immutable_if_enabled_returns_correct_error`

**Status:** done

### §2.2.1.1 ReturnCode_t INCONSISTENT_POLICY

**Spec:** §2.2.1.1, p. 17 — "INCONSISTENT_POLICY Application specified a set of
policies that are not consistent with each other."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::InconsistentPolicy`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_inconsistent_policy_maps`

**Status:** done — the code is mapped; that the `create_*` and `set_qos`
operations do not return it for an inconsistent QoS is listed under
§2.2.2.1.1.1 and the `create_*` items.

### §2.2.1.1 ReturnCode_t PRECONDITION_NOT_MET

**Spec:** §2.2.1.1, p. 17 — "PRECONDITION_NOT_MET A pre-condition for the
operation was not met."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::PreconditionNotMet`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_precondition_not_met_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t TIMEOUT

**Spec:** §2.2.1.1, p. 17 — "TIMEOUT The operation timed out."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::Timeout`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_timeout_maps`,
`crates/dcps/src/condition.rs::tests::waitset_wait_timeout_returns_err`

**Status:** done

### §2.2.1.1 ReturnCode_t ILLEGAL_OPERATION

**Spec:** §2.2.1.1, p. 17 — "ILLEGAL_OPERATION An operation was invoked on an
inappropriate object or at an inappropriate time … There is no precondition
that could be changed to make the operation succeed."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::IllegalOperation`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_illegal_operation_maps`

**Status:** done

### §2.2.1.1 ReturnCode_t NO_DATA

**Spec:** §2.2.1.1, p. 17 — "NO_DATA Indicates a transient situation where the
operation did not return any data but there is no inherent error."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::NoData`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_no_data_maps`

**Status:** done — the code is mapped; that `read`/`take` without data return
`Ok` with an empty list instead of NO_DATA is listed under §2.2.2.5.3.8.

### §2.2.1.1 Standard return codes

**Spec:** §2.2.1.1, p. 17 — "The return codes OK, ERROR, ILLEGAL_OPERATION,
ALREADY_DELETED, UNSUPPORTED, and BAD_PARAMETER are the standard return codes
and the specification won’t mention them explicitly for each operation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reading convention for the operation
descriptions that follow.

### §2.2.1.1 Use of a deleted entity

**Spec:** §2.2.1.1, p. 17 — "It is an error for an application to use an Entity
that has already been deleted … In the cases where the implementation can
detect the use of a deleted entity, the operation should fail and return
ALREADY_DELETED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_not_deleted`,
`EntityState::check_enabled`); called in `crates/dcps/src/publisher.rs`
(`Publisher::create_datawriter`, DataWriter operations),
`crates/dcps/src/subscriber.rs` (DataReader and Subscriber operations),
`crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`,
`DomainParticipant::delete_publisher`, `DomainParticipant::delete_subscriber`).

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`,
`crates/dcps/tests/entity_deletion.rs::participant_delete_contained_entities_deletes_everything_it_created`,
`crates/dcps/src/entity.rs::tests::check_not_deleted_returns_already_deleted_after_mark`

**Status:** done — publishers, subscribers, DataWriters, DataReaders, topics, ContentFilteredTopics and MultiTopics carry the state "deleted" and return `AlreadyDeleted` afterwards. The `DomainParticipant` is deleted after `delete_participant`. `create_publisher` and `create_subscriber` return no `Result` (the spec lets a create return `nil`): on a deleted participant they return an entity that is deleted, `try_create_publisher` and `try_create_subscriber` the error `AlreadyDeleted`.

### §2.2.1.2.1 Conceptual Outline — Overview

**Spec:** §2.2.1.2.1, pp. 18–19 — Publisher/DataWriter on the sending side,
Subscriber/DataReader on the receiving side; a Topic joins a name unique in the
domain, a data type and the QoS of the data.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview of terms; the requirements on the
classes are in §2.2.2 and listed there one by one.

### §2.2.1.2.2 Overall Conceptual Model

**Spec:** §2.2.1.2.2, pp. 19–21 — every entity has QoS, at most one listener and
a StatusCondition; all entities hang off a DomainParticipant; equal key values
denote the same instance, a topic without a key has exactly one instance;
built-in DataReaders "should be provided with every implementation" (footnote
8).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the statements are made normatively elsewhere
and listed there: one listener per entity under §2.2.2.1.1.3, instances and keys
under §2.2.2.5.1, built-in topics under §2.2.5.

### §2.2.2 PIM Description — modules

**Spec:** §2.2.2, pp. 21–22 — five modules: Infrastructure, Domain,
Topic-Definition, Publication, Subscription; entities the application extends
are modelled as interfaces, the others as classes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the modules are listed one by one
under §2.2.2.1–§2.2.2.5.

---

## §2.2.2.1 Infrastructure Module

### §2.2.2.1 Classes of the Infrastructure Module

**Spec:** §2.2.2.1, p. 23 — Entity, DomainEntity, QosPolicy, Listener, Status,
WaitSet, Condition, GuardCondition, StatusCondition.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; each class is listed below.

### §2.2.2.1.1 Entity — abstract base

**Spec:** §2.2.2.1.1, pp. 23–24 — "This class is the abstract base class for all
the DCPS objects that support QoS policies, a listener and a status condition."
Operations: set_qos, get_qos, set_listener, get_listener, get_statuscondition,
get_status_changes, enable, get_instance_handle.

**Repo:** `crates/dcps/src/entity.rs` (`Entity`, `EntityState`); implemented
for `DomainParticipant` (`crates/dcps/src/participant.rs`), `Topic<T>`
(`crates/dcps/src/topic.rs`), `Publisher` and `DataWriter<T>`
(`crates/dcps/src/publisher.rs`), `Subscriber` and `DataReader<T>`
(`crates/dcps/src/subscriber.rs`).

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::entity_handles_unique_across_types`,
`crates/dcps/src/entity.rs::tests::entity_state_starts_disabled`

**Status:** done — `set_listener` and `get_listener` are not trait methods but
defined per entity type, because the listener type differs per entity; the
specification allows that ("must be provided by each of the derived Entity
classes").

### §2.2.2.1.1 StatusKind

**Spec:** §2.2.2.1.1, p. 24 — "StatusKind is an enumerated type that identifies
each concrete Status type."

**Repo:** `crates/dcps/src/psm_constants.rs` (module `status`),
`crates/dcps/src/entity.rs` (`StatusMask`)

**Tests:** `crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`

**Status:** done — as a bit mask following the IDL PSM (§2.3.3); the values are
listed one by one under §2.3.3.

### §2.2.2.1.1.1 set_qos — per derived entity, values replace the existing ones

**Spec:** §2.2.2.1.1.1, p. 24 — "This operation must be provided by each of the
derived Entity classes (DomainParticipant, Topic, Publisher, DataWriter,
Subscriber, DataReader) … The set of policies specified as the qos_list
parameter are applied on top of the existing QoS, replacing the values of any
policies previously set."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::set_qos`); implementations in
`crates/dcps/src/participant.rs`, `crates/dcps/src/topic.rs`,
`crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`,
`crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`

**Status:** done — the Rust API takes the complete QoS set of the entity; it
replaces the existing one.

### §2.2.2.1.1.1 set_qos — IMMUTABLE_POLICY after enable

**Spec:** §2.2.2.1.1.1, p. 24 — "If set_qos is invoked after the Entity is
enabled and it attempts to change the value of an “immutable” policy, the
operation will fail and it returns IMMUTABLE_POLICY."

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`);
`crates/dcps/src/publisher.rs` (`Publisher::set_qos`: PRESENTATION);
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`: PRESENTATION).

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`

**Status:** done — the list of immutable policies per entity matches the
“Changeable” column of the QoS table (§2.2.3); DataWriter and DataReader
additionally list PRESENTATION and DATA_REPRESENTATION, the DataReader
TYPE_CONSISTENCY_ENFORCEMENT (XTypes 1.3).

### §2.2.2.1.1.1 set_qos — INCONSISTENT_POLICY

**Spec:** §2.2.2.1.1.1, p. 24 — "The set_qos operation will also fail if it
specifies a set of values that once combined with the existing values would
result in an inconsistent set of policies. In this case, the return value is
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`

**Status:** `done` — a `set_qos` with contradicting policies returns
INCONSISTENT_POLICY and leaves the QoS as it was.

### §2.2.2.1.1.1 set_qos — UNSUPPORTED for an unsupported value

**Spec:** §2.2.2.1.1.1, p. 24 — "If the application supplies a non-default
value for a QoS policy that is not supported by the implementation of the
service, the set_qos operation will fail and return UNSUPPORTED."

**Repo:** `crates/dcps/src/publisher.rs` (`validate_writer_data_representation`), `crates/dcps/src/runtime.rs` (`check_emittable_offer`)

**Tests:** `crates/dcps/tests/unsupported_qos.rs::a_data_representation_the_service_does_not_emit_is_unsupported`

**Status:** done — a DATA_REPRESENTATION value the service does not emit returns `Unsupported` (return code 2), and the writer is not created. Values that were executed in part no longer exist: OWNERSHIP EXCLUSIVE changes the owner (§2.2.3.9.2), TRANSIENT and PERSISTENT have the fictitious entities (§2.2.3.4).

### §2.2.2.1.1.1 set_qos — no change on failure

**Spec:** §2.2.2.1.1.1, p. 24 — "The existing set of policies are only changed
if the set_qos operation succeeds. This is indicated by the OK return value. In
all other cases, none of the policies is modified."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_qos`),
`crates/dcps/src/subscriber.rs` (`DataReader::set_qos`),
`crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`,
`crates/dcps/tests/set_qos_atomic.rs::a_refused_set_qos_leaves_the_entity_factory_of_publisher_and_subscriber`

**Status:** done — the DataWriter, the DataReader and the Topic refuse before any assignment; the Publisher and the Subscriber set ENTITY_FACTORY after the new PARTITION is checked against the runtime (next item).

### §2.2.2.1.1.1 set_qos — no change on failure: Publisher and Subscriber

**Spec:** §2.2.2.1.1.1, p. 24 — "In all other cases, none of the policies is
modified."

**Repo:** —

**Tests:** `crates/dcps/tests/set_qos_atomic.rs::a_refused_set_qos_leaves_the_entity_factory_of_publisher_and_subscriber`

**Status:** done — a `set_qos` that changes ENTITY_FACTORY and the immutable PRESENTATION at once is refused, and ENTITY_FACTORY stays (a writer and a reader created afterwards are enabled). The runtime refuses a PARTITION only under DDS-Security (`DcpsRuntime::check_user_group_partition`); that path is read, not measured.

### §2.2.2.1.1.1 set_qos — special value *_QOS_DEFAULT

**Spec:** §2.2.2.1.1.1, pp. 24–25 — "This special value may be used as a
parameter to the set_qos operation to indicate that the QoS of the Entity should
be changed to match the current default QoS set in the Entity’s factory."

**Repo:** `crates/dcps/src/qos.rs` (`QosArg`), `crates/dcps/src/entity.rs`
(`Entity::set_qos`), the six `set_qos`;
`crates/zerodds-c-api/src/extra_ffi.rs::current_default`

**Tests:** `crates/dcps/tests/default_qos.rs::the_participant_factory_default`,
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`,
`crates/dcps/tests/default_qos.rs::set_qos_with_the_default_keeps_the_immutable_rule`,
`crates/dcps/tests/default_qos.rs::set_qos_with_the_default_on_topic_subscriber_and_reader`,
`crates/zerodds-c-api/src/extra_ffi.rs::tests::set_qos_with_null_takes_the_current_default_of_the_factory`

**Status:** `done` — `set_qos(*_QOS_DEFAULT)` takes the current default of the
entity's factory and applies it under the rules of the same value; after
enabling, a differing immutable policy is IMMUTABLE_POLICY. In the C API, `NULL`
stands for the special value.

### §2.2.2.1.1.2 get_qos

**Spec:** §2.2.2.1.1.2, p. 25 — "This operation allows access to the existing
set of QoS policies for the Entity."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_qos`); implementations per
entity.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::topic_qos_get_returns_clone`

**Status:** done

### §2.2.2.1.1.3 set_listener — install a listener with a mask

**Spec:** §2.2.2.1.1.3, p. 25 — "This operation installs a Listener on the
Entity. The listener will only be invoked on the changes of communication status
indicated by the specified mask."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_listener`),
`crates/dcps/src/topic.rs` (`Topic::set_listener`),
`crates/dcps/src/publisher.rs` (`Publisher::set_listener`,
`DataWriter::set_listener`), `crates/dcps/src/subscriber.rs`
(`Subscriber::set_listener`, `DataReader::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_with_only_data_available_bit_does_not_call_reader_for_others`,
`crates/dcps/tests/listener_integration.rs::push_raw_with_zero_mask_at_reader_bubbles_to_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::reader_with_only_data_available_bit_passes_other_events_up`

**Status:** done

### §2.2.2.1.1.3 set_listener — nil listener

**Spec:** §2.2.2.1.1.3, p. 25 — "It is permitted to use ‘nil’ as the value of
the listener. The ‘nil’ listener behaves as a Listener whose operations perform
no action."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` with a non-empty mask installs `NilListener`: the statuses of the mask are handled by doing nothing, do not bubble up (for DATA_ON_READERS no `on_data_available` follows) and are not reset. `get_listener` returns none for it. An empty mask is no listener. The C ABI (polled listeners) takes NULL with a mask as the nil listener too: `zerodds_poll_listeners` leaves out of the publisher, subscriber and participant levels the statuses a nil listener of the writer, reader, subscriber or topic below has in its mask (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.2.1.1.3 set_listener — one listener per entity, replacement

**Spec:** §2.2.2.1.1.3, p. 25 — "Only one listener can be attached to each
Entity. If a listener was already set, the operation set_listener will replace
it with the new one. Consequently if the value ‘nil’ is passed for the listener
parameter to the set_listener operation, any existing listener will be removed."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_listener`:
one slot `Option<(Listener, StatusMask)>`); likewise per entity.

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_listener_replacement_works`,
`crates/dcps/tests/listener_integration.rs::participant_listener_replacement_works`,
`crates/dcps/tests/listener_integration.rs::listener_replacement_during_dispatch_is_safe`

**Status:** done

### §2.2.2.1.1.4 get_listener

**Spec:** §2.2.2.1.1.4, p. 25 — "This operation allows access to the existing
Listener attached to the Entity."

**Repo:** `get_listener` per entity in `crates/dcps/src/participant.rs`,
`crates/dcps/src/topic.rs`, `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`.

**Tests:** `crates/dcps/tests/listener_integration.rs::participant_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::topic_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.1.1.5 get_statuscondition

**Spec:** §2.2.2.1.1.5, p. 25 — "This operation allows access to the
StatusCondition … associated with the Entity. The returned condition can then be
added to a WaitSet". By Figure 2.5 and §2.2.2.1.9 each entity has exactly one
StatusCondition.

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_status_condition`, `EntityState`)

**Tests:** `crates/dcps/tests/status_condition.rs::an_entity_has_one_status_condition`, `crates/dcps/tests/status_condition.rs::get_entity_returns_the_entity_of_each_kind`,
`crates/dcps/tests/entity_lifecycle.rs::status_condition_trigger_on_relevant_status`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done — every call returns the one StatusCondition the entity holds; the `enabled_statuses` set through it is the entity's. It refers to its entity weakly.

### §2.2.2.1.1.6 get_status_changes — triggered statuses

**Spec:** §2.2.2.1.1.6, p. 25 — "This operation retrieves the list of
communication statuses in the Entity that are ‘triggered.’ That is, the list of
statuses whose value has changed since the last time the application read the
status."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_status_changes`,
`EntityState::status_changes`, `EntityState::clear_status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/entity_lifecycle.rs::datareader_status_changes_track`

**Status:** done

### §2.2.2.1.1.6 get_status_changes — empty after creation

**Spec:** §2.2.2.1.1.6, p. 25 — "When the entity is first created or if the
entity is not enabled, all communication statuses are in the “untriggered”
state so the list returned by the get_status_changes operation will be empty."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::with_enabled`:
`status_changes = 0`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datareader_status_changes_track`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — empty after creation and while not enabled (`EntityState::status_changes` returns 0 for an entity that is not enabled).

### §2.2.2.1.1.6 get_status_changes — empty while not enabled

**Spec:** §2.2.2.1.1.6, p. 25 — "… or if the entity is not enabled, all
communication statuses are in the “untriggered” state".

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::status_changes`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the participant and the reader created by a factory that is not enabled report `get_status_changes() == 0`.

### §2.2.2.1.1.6 get_status_changes — only the entity’s own statuses

**Spec:** §2.2.2.1.1.6, p. 25 — "The list of statuses returned by the
get_status_changes operation refers to the status that are triggered on the
Entity itself and does not include statuses that apply to contained entities."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::get_status_changes_reports_the_entity_itself`

**Status:** done — the writer and the reader report PUBLICATION_MATCHED and SUBSCRIPTION_MATCHED, the publisher, the subscriber and the participant do not.

### §2.2.2.1.1.7 enable — enabling, idempotent

**Spec:** §2.2.2.1.1.7, p. 26 — "This operation enables the Entity … The enable
operation is idempotent. Calling enable on an already enabled Entity returns OK
and has no effect."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`,
`Entity::enable`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_is_enabled_by_the_factory_and_enable_is_idempotent`,
`crates/dcps/src/entity.rs::tests::enable_is_idempotent_and_reports_first_transition`

**Status:** done

### §2.2.2.1.1.7 enable — default of ENTITY_FACTORY

**Spec:** §2.2.2.1.1.7, p. 26 — "Entity objects can be created either enabled
or disabled. This is controlled by the value of the ENTITY_FACTORY Qos policy …
by default, it is not necessary to explicitly call enable on newly created
entities".

**Repo:** `crates/dcps/src/participant.rs` (`autoenables_children`),
`crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`),
`crates/dcps/src/factory.rs` (`DomainParticipantFactory::autoenable`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done

### §2.2.2.1.1.7 enable — operations on an entity that is not enabled

**Spec:** §2.2.2.1.1.7, p. 26 — allowed before `enable` are QoS and listener
operations (including default QoS), get_statuscondition, factory operations,
get_status_changes "and other get status operations", lookup operations; "Other
operations may explicitly state that they may be called on disabled entities;
those that do not will return the error NOT_ENABLED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_enabled`); called in
the data operations of `DataWriter` and `DataReader`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — QoS, listener, default QoS, StatusCondition, factory, lookup and get status operations run on an entity that is not enabled; the others (data operations, `assert_liveliness`, `get_discovered_*`, `contains_entity`, `ignore_*`, `get_current_time`, group access) return NOT_ENABLED.

### §2.2.2.1.1.7 enable — get status operations on an entity that is not enabled

**Spec:** §2.2.2.1.1.7, p. 26 — allowed before `enable`: "get_status_changes and
other get status operations (although the status of a disabled entity never
changes)".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs` (`get_*_status`), `crates/dcps/src/topic.rs` (`Topic::get_inconsistent_topic_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — the `get_*_status` operations of DataWriter, DataReader and Topic follow §2.2.2.1.1.7: on a disabled entity they return the untriggered status (all zero), as the C ABI already did; a deleted entity is `AlreadyDeleted`. The class descriptions of Topic, Publisher, DataWriter, Subscriber and DataReader (pp. 49, 56, 62, 78, 88) list only the base operations as exceptions of NOT_ENABLED; the findings file records the contradiction, and the explicit list of §2.2.2.1.1.7 decides. `matched_subscriptions` and the other operations §2.2.2.1.1.7 does not list stay NOT_ENABLED.

### §2.2.2.1.1.7 enable — NOT_ENABLED on DomainParticipant, Publisher, Subscriber, Topic

**Spec:** §2.2.2.1.1.7, p. 26 — "those that do not will return the error
NOT_ENABLED."; §2.2.2.2.1, p. 35 — "Other operations will have the value
NOT_ENABLED if called on a disabled DomainParticipant"; §2.2.2.4.1, p. 56,
and §2.2.2.5.2, p. 78 — the same rule for Publisher and Subscriber.

**Repo:** `crates/dcps/src/participant.rs` (`assert_liveliness`, `try_get_discovered_participants`, `try_get_discovered_topics`, `try_contains_entity`, `get_discovered_*_data`)

**Tests:**
`crates/dcps/tests/entity_lifecycle.rs::group_operations_of_a_disabled_publisher_or_subscriber_are_not_enabled`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the operations that §2.2.2.1.1.7 and §2.2.2.2.1 do not allow return NOT_ENABLED; `get_discovered_participants`, `get_discovered_topics` and `contains_entity` have the `Result` forms (`try_*`) for it.

### §2.2.2.1.1.7 enable — deleting an entity that is not enabled

**Spec:** §2.2.2.1.1.7, p. 26 — "It is legal to delete an Entity that has not
been enabled by calling the proper operation on its factory."

**Repo:** —

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the writer, the reader, the publisher, the subscriber and the topic of a factory that is not enabled are deleted.

### §2.2.2.1.1.7 enable — created by a factory that is not enabled

**Spec:** §2.2.2.1.1.7, p. 26 — "Entities created from a factory that is
disabled, are created disabled regardless of the setting of the ENTITY_FACTORY
Qos policy."

**Repo:** `crates/dcps/src/participant.rs` (`autoenables_children`),
`crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`

**Status:** done

### §2.2.2.1.1.7 enable — PRECONDITION_NOT_MET with a factory that is not enabled

**Spec:** §2.2.2.1.1.7, p. 26 — "Calling enable on an Entity whose factory is
not enabled will fail and return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** done

### §2.2.2.1.1.7 enable — enabling the factory enables what it created

**Spec:** §2.2.2.1.1.7, p. 26 — "If the ENTITY_FACTORY Qos policy has
autoenable_created_entities set to TRUE, the enable operation on the factory
will automatically enable all entities created from the factory."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::enable_entity`, cascade
over `children`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`,
`crates/dcps/tests/enable_after_create.rs::a_participant_created_disabled_is_discovered_after_its_enable`

**Status:** done

### §2.2.2.1.1.7 enable — listeners only after enabling

**Spec:** §2.2.2.1.1.7, p. 26 — "The Listeners associated with an entity are
not called until the entity is enabled."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`), `crates/dcps/src/subscriber.rs` (`notify_data_arrived`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the listener of a reader that is not enabled is not called; after `enable` the next sample calls it.

### §2.2.2.1.1.7 enable — conditions of an entity that is not enabled are inactive

**Spec:** §2.2.2.1.1.7, p. 26 — "Conditions associated with an entity that is
not enabled are “inactive,” that is, have a trigger_value==FALSE".

**Repo:** `crates/dcps/src/subscriber.rs` (`holds_matching`), `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value` via `status_changes`)

**Tests:** `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the ReadCondition and the StatusCondition of a reader that is not enabled have `trigger_value` FALSE; after `enable` the ReadCondition holds.

### §2.2.2.1.1.8 get_instance_handle

**Spec:** §2.2.2.1.1.8, p. 26 — "This operation returns the InstanceHandle_t
that represents the Entity."

**Repo:** `crates/dcps/src/entity.rs` (`Entity::get_instance_handle`,
`EntityState::instance_handle`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_get_instance_handle_unique`,
`crates/dcps/tests/entity_lifecycle.rs::entity_handles_unique_across_types`,
`crates/dcps/src/entity.rs::tests::instance_handles_are_unique_per_entity`

**Status:** done — the handle is a local counter. Which handle appears in
built-in topics and statuses for the same entity is listed under §2.2.5.

### §2.2.2.1.2 DomainEntity

**Spec:** §2.2.2.1.2, p. 26 — "DomainEntity is the abstract base class for all
DCPS entities, except for the DomainParticipant. Its sole purpose is to express
that DomainParticipant is a special kind of Entity, which acts as a container of
all other Entity, but itself cannot contain other DomainParticipant."

**Repo:** —

**Tests:** `crates/dcps/tests/domain_entity.rs::a_domain_participant_contains_no_domain_participant`

**Status:** done — the test creates two participants: neither contains the other, and everything one creates belongs to it and to no other. The Rust API and the C API have no operation that creates a DomainParticipant in another.

### §2.2.2.1.3 QosPolicy — attribute name

**Spec:** §2.2.2.1.3, p. 27 — "It has an attribute name that is used to
identify uniquely each QoS policy. All concrete QosPolicy classes derive from
this root and include a value whose type depends on the concrete QoS policy."

**Repo:** `crates/qos/src/policies/name.rs` (`QosPolicy`, `policy_name`)

**Tests:** `crates/qos/src/policies/name.rs::tests::every_policy_has_its_name`

**Status:** done — `QosPolicy::name` of each of the 22 policies returns the name of the IDL constant; the 22 names are distinct. In the IDL `GROUPDATA_QOS_POLICY_NAME` has the value `"TransportPriority"` (a slip in the spec); here it is `"GroupData"`, and `"TransportPriority"` is the name of TRANSPORT_PRIORITY.

### §2.2.2.1.3 QosPolicy — policies per entity

**Spec:** §2.2.2.1.3, p. 27 — "Each Entity can be configured with a list of
QosPolicy. However, any Entity cannot support any QosPolicy."

**Repo:** `crates/dcps/src/qos.rs` (`DomainParticipantQos`, `TopicQos`,
`PublisherQos`, `SubscriberQos`, `DataWriterQos`, `DataReaderQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::participant_qos_get_set_roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::topic_qos_get_returns_clone`

**Status:** done — which policy belongs to which entity is listed per policy
under §2.2.3.

### §2.2.2.1.3 QosPolicy — consistency check on every change

**Spec:** §2.2.2.1.3, p. 27 — "Consistency checking is performed each time the
policies are modified via the set_qos operation."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — every `set_qos` and `create_*` of Topic, DataWriter and
DataReader checks consistency.

### §2.2.2.1.3 QosPolicy — transition and immutable policies

**Spec:** §2.2.2.1.3, p. 27 — "When a policy is changed after being set to a
given value, it is not required that the new value be applied instantaneously;
the Service is allowed to apply it after a transition phase. In addition, some
QosPolicy have “immutable” semantics".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission without obligation; the immutable
policies are listed under §2.2.2.1.1.1 “IMMUTABLE_POLICY” and per policy under
§2.2.3.

### §2.2.2.1.4 Listener — root of the listener interfaces

**Spec:** §2.2.2.1.4, p. 27 — "Listener is the abstract root for all Listener
interfaces. All the supported kinds of concrete Listener interfaces (one per
concrete Entity: DomainParticipant, Topic, Publisher, DataWriter, Subscriber,
and DataReader) derive from this root".

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener`, `DataWriterListener`,
`PublisherListener`, `DataReaderListener`, `SubscriberListener`,
`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::topic_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — six listener traits, one per entity kind; a common base
trait without methods is missing, it would have no operation. The callback
signatures are listed under §2.2.4.3.

### §2.2.2.1.5 Status — root of the status classes

**Spec:** §2.2.2.1.5, p. 28 — "Status is the abstract root class for all
communication status objects … Each concrete Entity is associated with a set of
Status objects whose value represents the “communication status” of that
entity. These status values can be accessed with corresponding methods on the
Entity."

**Repo:** `crates/dcps/src/status.rs` (one Rust type per status)

**Tests:** `crates/dcps/src/status.rs::tests::inconsistent_topic_default_is_zero`,
`crates/dcps/src/status.rs::tests::data_available_and_data_on_readers_are_zero_sized_markers`

**Status:** done — the statuses and their fields are listed one by one under
§2.2.4.1.

### §2.2.2.1.6 WaitSet — created without a factory

**Spec:** §2.2.2.1.6, p. 28 — "WaitSet has no factory. It is created as an
object directly by the natural means in each language binding … it is not
necessarily associated with a single DomainParticipant".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::new`, `WaitSet::default`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_default_is_empty`,
`crates/dcps/src/condition.rs::tests::waitset_with_multiple_conditions_returns_all_triggered`

**Status:** done

### §2.2.2.1.6.1 attach_condition — attaching

**Spec:** §2.2.2.1.6.1, p. 28 — "Attaches a Condition to the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::read_condition_attaches_to_waitset`

**Status:** done

### §2.2.2.1.6.1 attach_condition — during wait

**Spec:** §2.2.2.1.6.1, p. 28 — "It is possible to attach a Condition on a
WaitSet that is currently being waited upon (via the wait operation). In this
case, if the Condition has a trigger_value of TRUE, then attaching the condition
will unblock the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`:
`signal_data_available`)

**Tests:** `crates/dcps/tests/waitset_attach.rs::attaching_a_triggered_condition_unblocks_a_waiting_waitset`

**Status:** `done` — an attached condition wakes a waiting `wait`; measured 0.10
s instead of 5 s to the deadline.

### §2.2.2.1.6.1 attach_condition — already attached

**Spec:** §2.2.2.1.6.1, p. 29 — "Adding a Condition that is already attached to
the WaitSet has no effect."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::attach_condition`:
`Arc::ptr_eq`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::waitset_attach_idempotent_does_not_count_against_cap`

**Status:** done

### §2.2.2.1.6.1 attach_condition — OUT_OF_RESOURCES

**Spec:** §2.2.2.1.6.1, p. 29 — "Possible error codes returned in addition to
the standard ones: OUT_OF_RESOURCES."

**Repo:** `crates/dcps/src/condition.rs` (`MAX_WAITSET_CONDITIONS`,
`WaitSet::attach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_above_max_returns_out_of_resources`

**Status:** done

### §2.2.2.1.6.2 detach_condition — detaching

**Spec:** §2.2.2.1.6.2, p. 29 — "Detaches a Condition from the WaitSet."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::detach_condition`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`, `crates/dcps/tests/status_condition.rs::detach_condition_wants_an_attached_condition`

**Status:** done — a condition that is not attached returns `PreconditionNotMet`.

### §2.2.2.1.6.3 wait — block until a condition triggers

**Spec:** §2.2.2.1.6.3, p. 29 — "If none of the conditions attached to the
WaitSet have a trigger_value of TRUE, the wait operation will block suspending
the calling thread. The result of the wait operation is the list of all the
attached conditions that have a trigger_value of TRUE".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`,
`WaitSet::poll_active`, `signal_data_available`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_returns_immediately_if_already_triggered`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`,
`crates/dcps/src/condition.rs::tests::waitset_with_multiple_conditions_returns_all_triggered`,
`crates/dcps/src/condition.rs::tests::data_signal_wakes_a_blocked_wait`,
`crates/dcps/tests/waitset_notification.rs::data_arrival_unblocks_a_waitset_waiting_on_a_read_condition`

**Status:** done

### §2.2.2.1.6.3 wait — TIMEOUT

**Spec:** §2.2.2.1.6.3, p. 29 — "It this duration is exceeded and none of the
attached Condition objects is true, wait will return with the return code
TIMEOUT."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_timeout_returns_err`

**Status:** done

### §2.2.2.1.6.3 wait — one waiting thread, otherwise PRECONDITION_NOT_MET

**Spec:** §2.2.2.1.6.3, p. 29 — "If the wait operation is invoked on a WaitSet
that already has a thread blocking on it, the operation will return immediately
with the value PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSetInner::waiting`,
`WaitSet::wait`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_concurrent_wait_returns_precondition_not_met`,
`crates/dcps/src/condition.rs::tests::waitset_sequential_waits_after_first_returns_succeed`

**Status:** done

### §2.2.2.1.6.4 get_conditions

**Spec:** §2.2.2.1.6.4, p. 29 — "This operation retrieves the list of attached
conditions."

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::get_conditions`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_attach_detach_idempotent`,
`crates/dcps/src/condition.rs::tests::waitset_clone_shares_state`

**Status:** done

### §2.2.2.1.7 Condition — trigger_value

**Spec:** §2.2.2.1.7, p. 29 — "A Condition is a root class for all the
conditions that may be attached to a WaitSet … A Condition has a trigger_value
that can be TRUE or FALSE and is set automatically by the Service."

**Repo:** `crates/dcps/src/condition.rs` (`Condition`); implemented for
`GuardCondition`, `StatusCondition`, `ReadCondition`, `QueryCondition`.

**Tests:** `crates/dcps/src/condition.rs::tests::status_condition_implements_condition_trait`,
`crates/dcps/src/condition.rs::tests::read_condition_implements_condition_trait_object_safe`

**Status:** done

### §2.2.2.1.7.1 get_trigger_value

**Spec:** §2.2.2.1.7.1, p. 29 — "This operation retrieves the trigger_value of
the Condition."

**Repo:** `crates/dcps/src/condition.rs` (`Condition::get_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`,
`crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.1.8 GuardCondition — without a factory, initial value FALSE

**Spec:** §2.2.2.1.8, p. 30 — "GuardCondition has no factory. It is created as
an object directly by the natural means in each language binding … When first
created the trigger_value is set to FALSE."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::new`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_starts_false`

**Status:** done

### §2.2.2.1.8.1 set_trigger_value

**Spec:** §2.2.2.1.8.1, p. 30 — "This operation sets the trigger_value of the
GuardCondition. WaitSet objects behavior depends on the changes of the
trigger_value of their attached conditions."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::set_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`

**Status:** done — a waiting WaitSet sees the change after at most
`poll_interval` (1 ms); `set_trigger_value` itself sends no signal.

### §2.2.2.1.9 StatusCondition — trigger_value filtered by enabled_statuses

**Spec:** §2.2.2.1.9, p. 30 — "The trigger_value of the StatusCondition depends
on the communication status of that entity …, ‘filtered’ by the set of
enabled_statuses on the StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/entity_lifecycle.rs::status_condition_trigger_on_relevant_status`

**Status:** done

### §2.2.2.1.9.1 set_enabled_statuses

**Spec:** §2.2.2.1.9.1, pp. 30–31 — "This operation defines the list of
communication statuses that are taken into account to determine the
trigger_value of the StatusCondition. This operation may change the
trigger_value of the StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::set_enabled_statuses`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done — the mask applies to the one `StatusCondition` object; see
§2.2.2.1.1.5.

### §2.2.2.1.9.1 set_enabled_statuses — default: all statuses

**Spec:** §2.2.2.1.9.1, p. 31 — "If this function is not invoked, the default
list of enabled statuses includes all the statuses."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_status_condition_starts_with_every_status_enabled`

**Status:** done — the test covers it.

### §2.2.2.1.9.2 get_enabled_statuses

**Spec:** §2.2.2.1.9.2, p. 31 — "This operation returns the statuses that were
explicitly set on the last call to set_enabled_statuses or, if
set_enabled_statuses was never called, the default list".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_status_condition_starts_with_every_status_enabled`

**Status:** done — the test covers it.

### §2.2.2.1.9.3 get_entity

**Spec:** §2.2.2.1.9.3, p. 31 — "This operation returns the Entity associated
with the StatusCondition. Note that there is exactly one Entity associated with
each StatusCondition."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::get_entity`)

**Tests:** `crates/dcps/tests/status_condition.rs::get_entity_returns_the_entity_of_each_kind`, `crates/dcps/tests/status_condition.rs::get_entity_is_none_once_the_entity_is_gone`

**Status:** done — `get_entity` returns the `EntityRef` of the entity (participant, publisher, subscriber, topic, writer or reader); `None` once the entity is gone.

---

## §2.2.2.2 Domain Module

### §2.2.2.2 Classes of the Domain Module

**Spec:** §2.2.2.2, p. 32 — DomainParticipant, DomainParticipantFactory,
DomainParticipantListener.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; the classes are listed below.

### §2.2.2.2.1 DomainParticipant — container, factory, domain

**Spec:** §2.2.2.2.1, pp. 32–33 — the DomainParticipant is the container of all
other entities, the factory for Publisher, Subscriber, Topic and MultiTopic, and
stands for the participation in a domain: "A domain establishes a “virtual
network” linking all applications that share the same domainId and isolating
them from applications running on different domains."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant`,
`ParticipantInner`); domain separation in `crates/dcps/src/runtime.rs` (SPDP
ports and the domain check of the announcement).

**Tests:** `crates/dcps/src/participant.rs::tests::participant_created_with_domain_id`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_tag_is_not_discovered`

**Status:** done

### §2.2.2.2.1 Operations on a DomainParticipant that is not enabled

**Spec:** §2.2.2.2.1, p. 35 — "The following operations may be called even if
the DomainParticipant is not enabled. Other operations will have the value
NOT_ENABLED if called on a disabled DomainParticipant": set_qos, get_qos,
set_listener, get_listener, enable; create_topic, create_publisher,
create_subscriber, delete_topic, delete_publisher, delete_subscriber;
get_statuscondition.

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic`,
`DomainParticipant::create_publisher`, `DomainParticipant::create_subscriber`
without a state check)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`,
`crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`,
`crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the operations named run on a DomainParticipant that is not enabled, the others return NOT_ENABLED.

### §2.2.2.2.1.1 create_publisher — creating

**Spec:** §2.2.2.2.1.1, pp. 35–36 — "This operation creates a Publisher with the
desired QoS policies … If the specified QoS policies are not consistent, the
operation will fail and no Publisher will be created … The created Publisher
belongs to the DomainParticipant that is its factory. In case of failure, the
operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_publisher`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_publisher`,
`crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_of_another_participant`

**Status:** done — for `PublisherQos` (PRESENTATION, PARTITION, GROUP_DATA,
ENTITY_FACTORY) §2.2.3 names no combination that is inconsistent; the
operation therefore cannot fail and returns `Publisher` instead of `Result`.

### §2.2.2.2.1.1 create_publisher — listener and mask

**Spec:** §2.2.2.2.1.1, p. 35 — parameters `a_listener: PublisherListener`,
`mask: StatusKind []`; "attaches to it the specified PublisherListener".

**Repo:** `create_publisher_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_publisher_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.2.1.1 create_publisher — PUBLISHER_QOS_DEFAULT

**Spec:** §2.2.2.2.1.1, p. 35 — "The special value PUBLISHER_QOS_DEFAULT can be
used to indicate that the Publisher should be created with the default
Publisher QoS set in the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_publisher`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — `PUBLISHER_QOS_DEFAULT` takes `get_default_publisher_qos`;
a QoS value is taken as it is.

### §2.2.2.2.1.2 delete_publisher — deleting

**Spec:** §2.2.2.2.1.2, p. 36 — "This operation deletes an existing Publisher."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`

**Status:** done — the test deletes after removing the DataWriter and checks
`AlreadyDeleted` afterwards.

### §2.2.2.2.1.2 delete_publisher — PRECONDITION_NOT_MET with a DataWriter

**Spec:** §2.2.2.2.1.2, p. 36 — "A Publisher cannot be deleted if it has any
attached DataWriter objects. If delete_publisher is called on a Publisher with
existing DataWriter object, it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`,
`Publisher::has_datawriters`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_with_writers`

**Status:** done

### §2.2.2.2.1.2 delete_publisher — another DomainParticipant

**Spec:** §2.2.2.2.1.2, p. 36 — "If delete_publisher is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_publisher`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_publisher_refuses_a_publisher_of_another_participant`

**Status:** done

### §2.2.2.2.1.3 create_subscriber — creating

**Spec:** §2.2.2.2.1.3, p. 36 — "This operation creates a Subscriber with the
desired QoS policies … The created Subscriber belongs to the DomainParticipant
that is its factory. In case of failure, the operation will return a ‘nil’
value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_subscriber`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_subscriber`,
`crates/dcps/tests/e2e_dcps_api.rs::public_api_create_subscriber_and_reader_offline`

**Status:** done — for `SubscriberQos` §2.2.3 names no inconsistent
combination; the operation returns `Subscriber` instead of `Result`.

### §2.2.2.2.1.3 create_subscriber — listener and mask

**Spec:** §2.2.2.2.1.3, p. 36 — "attaches to it the specified
SubscriberListener"; parameters `a_listener`, `mask`.

**Repo:** `create_subscriber_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_subscriber_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.2.1.3 create_subscriber — SUBSCRIBER_QOS_DEFAULT

**Spec:** §2.2.2.2.1.3, p. 36 — "The special value SUBSCRIBER_QOS_DEFAULT can be
used to indicate that the Subscriber should be created with the default
Subscriber QoS set in the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_subscriber`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — `SUBSCRIBER_QOS_DEFAULT` takes
`get_default_subscriber_qos`.

### §2.2.2.2.1.4 delete_subscriber — deleting

**Spec:** §2.2.2.2.1.4, p. 36 — "This operation deletes an existing Subscriber."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.4 delete_subscriber — PRECONDITION_NOT_MET with a DataReader

**Spec:** §2.2.2.2.1.4, p. 36 — "If the delete_subscriber operation is called
on a Subscriber with existing DataReader objects, it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`,
`Subscriber::has_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.4 delete_subscriber — another DomainParticipant

**Spec:** §2.2.2.2.1.4, p. 36 — "If delete_subscriber is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_subscriber`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant`

**Status:** done

### §2.2.2.2.1.5 create_topic — creating

**Spec:** §2.2.2.2.1.5, pp. 36–37 — "This operation creates a Topic with the
desired QoS policies … The created Topic belongs to the DomainParticipant that
is its factory … In case of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic`,
`DomainParticipant::create_topic_with_support`,
`DomainParticipant::create_serialized_topic`)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_stores_in_registry`,
`crates/dcps/src/participant.rs::tests::create_topic_rejects_empty_name`,
`crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_local_topic`

**Status:** done — a failure is `Err(DdsError)`.

### §2.2.2.2.1.5 create_topic — listener and mask

**Spec:** §2.2.2.2.1.5, p. 36 — "attaches to it the specified TopicListener";
parameters `a_listener`, `mask`.

**Repo:** `create_topic_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_topic_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.2.1.5 create_topic — inconsistent QoS

**Spec:** §2.2.2.2.1.5, p. 36 — "If the specified QoS policies are not
consistent, the operation will fail and no Topic will be created."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — `create_topic` refuses a TopicQos whose HISTORY and
RESOURCE_LIMITS contradict.

### §2.2.2.2.1.5 create_topic — TOPIC_QOS_DEFAULT

**Spec:** §2.2.2.2.1.5, p. 36 — "The special value TOPIC_QOS_DEFAULT can be used
to indicate that the Topic should be created with the default Topic QoS set in
the factory."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_topic`,
`::create_serialized_topic`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — `TOPIC_QOS_DEFAULT` takes `get_default_topic_qos`.

### §2.2.2.2.1.5 create_topic — bound to the registered type

**Spec:** §2.2.2.2.1.5, p. 37 — "The Topic is bound to a type described by the
type_name argument. Prior to creating a Topic the type must have been
registered with the Service. This is done using the register_type operation".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic_of_registered_type`, `create_topic::<T>`)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_takes_a_second_type_and_rejects_an_inconsistent_qos`,
`crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/dcps/tests/type_support.rs::a_topic_is_bound_to_the_registered_type`

**Status:** done — `create_topic_of_registered_type` requires the type to be registered under the name (PRECONDITION_NOT_MET otherwise) and binds the topic to it. `create_topic::<T>` registers `T` under its own name implicitly.

### §2.2.2.2.1.5 create_topic — obtained several times, deleted several times

**Spec:** §2.2.2.2.1.11, p. 38 — "If a Topic is obtained multiple times by means
of find_topic or create_topic, it must also be deleted that same number of times
using delete_topic."

**Repo:** `crates/dcps/src/topic.rs` (`TopicInner::obtained`),
`crates/dcps/src/participant.rs` (`create_topic_with_support`, `delete_topic`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::a_topic_created_twice_goes_with_the_second_delete`

**Status:** `done` — every `create_topic` and every `find_topic` of the same Topic counts; only the last `delete_topic` deletes it (`tests/find_topic.rs::a_topic_obtained_by_find_topic_is_deleted_as_often`).

### §2.2.2.2.1.6 delete_topic — deleting

**Spec:** §2.2.2.2.1.6, p. 37 — "This operation deletes a Topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`,
`DomainParticipant::delete_topic_by_name`,
`DomainParticipant::delete_registered_topic`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_succeeds_when_nothing_uses_it`,
`crates/dcps/tests/delete_topic.rs::a_deleted_topic_is_gone_from_the_registry_and_its_name_is_free_again`,
`crates/dcps/src/participant.rs::tests::delete_topic_removes_the_entry_and_its_handle`

**Status:** done

### §2.2.2.2.1.6 delete_topic — PRECONDITION_NOT_MET with users

**Spec:** §2.2.2.2.1.6, p. 37 — "The deletion of a Topic is not allowed if there
are any existing DataReader, DataWriter, ContentFilteredTopic, or MultiTopic
objects that are using the Topic … it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`ParticipantInner::topic_users`,
`DomainParticipant::delete_registered_topic`); `crates/dcps/src/topic.rs`
(`TopicUserCounts`, `Topic::attach_user`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_datawriter_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_datareader_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_content_filtered_topic_uses_it`,
`crates/dcps/tests/delete_topic.rs::delete_topic_is_refused_while_a_multitopic_uses_it`,
`crates/dcps/tests/delete_topic.rs::every_kind_of_user_holds_the_topic_and_releasing_all_of_them_frees_it`

**Status:** done

### §2.2.2.2.1.6 delete_topic — another DomainParticipant

**Spec:** §2.2.2.2.1.6, p. 37 — "If delete_topic is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_from_another_participant_is_refused_and_changes_nothing`

**Status:** done

### §2.2.2.2.1.7 create_contentfilteredtopic — creating

**Spec:** §2.2.2.2.1.7, p. 37 — "The related Topic being subscribed to is
specified by means of the related_topic parameter … The logical expression is
derived from the filter_expression and expression_parameters arguments … In case
of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::create_contentfilteredtopic`); `crates/dcps/src/topic.rs`
(`ContentFilteredTopic::new`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_name`,
`crates/dcps/src/participant.rs::tests::create_contentfilteredtopic_rejects_empty_expression`

**Status:** done — an unreadable expression or a `%n` without a parameter
returns `BadParameter`. The grammar is listed under Annex B.

### §2.2.2.2.1.8 delete_contentfilteredtopic — deleting

**Spec:** §2.2.2.2.1.8, p. 37 — "This operation deletes a ContentFilteredTopic."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`;
`crates/dcps/src/topic.rs` (`DescriptionLife`)

**Tests:** `crates/dcps/tests/delete_description.rs::a_deleted_content_filtered_topic_is_deleted_for_every_handle`,
`crates/dcps/tests/delete_description.rs::a_reader_cannot_be_created_on_a_deleted_content_filtered_topic`

**Status:** `done` — the delete holds for every handle: the ContentFilteredTopic no longer holds its topic (`delete_topic` works afterwards); a second delete and every further operation return `AlreadyDeleted`.

### §2.2.2.2.1.8 delete_contentfilteredtopic — PRECONDITION_NOT_MET with DataReaders

**Spec:** §2.2.2.2.1.8, p. 37 — "If the delete_contentfilteredtopic operation is
called on a ContentFilteredTopic with existing DataReader objects attached to
it, it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`;
`crates/dcps/src/subscriber.rs::Subscriber::create_datareader_with_cft`;
`crates/zerodds-c-api/src/participant_ffi.rs::zerodds_dp_delete_contentfilteredtopic`

**Tests:** `crates/dcps/tests/delete_description.rs::a_content_filtered_topic_with_a_reader_is_not_deleted`,
`crates/dcps/tests/delete_description.rs::a_dropped_reader_lets_the_content_filtered_topic_go`,
`crates/zerodds-c-api/src/subscriber_ffi.rs::tests::a_cft_with_a_reader_is_not_deleted`

**Status:** `done` — while a DataReader on the ContentFilteredTopic exists, the delete returns `PreconditionNotMet`; once the reader is deleted (also by the end of its last handle) it is allowed. The same in the C API.

### §2.2.2.2.1.8 delete_contentfilteredtopic — another DomainParticipant

**Spec:** §2.2.2.2.1.8, p. 37 — "If delete_contentfilteredtopic is called on a
different DomainParticipant, the operation will have no effect and it will
return PRECONDITION_NOT_MET."

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::delete_contentfilteredtopic`

**Tests:**
`crates/dcps/src/participant.rs::tests::delete_contentfilteredtopic_rejects_foreign`

**Status:** `done` — `PreconditionNotMet`, without effect.

### §2.2.2.2.1.9 create_multitopic — creating

**Spec:** §2.2.2.2.1.9, pp. 37–38 — "a MultiTopic can be used to subscribe to
multiple topics and combine/filter the received data into a resulting type …
The list of topics and the logic used to combine filter and re-arrange the
information from each Topic are specified using the subscription_expression and
expression_parameters arguments."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::create_multitopic`,
`crates/dcps/src/multitopic.rs::MultiTopicPlan::new`

**Tests:** `crates/dcps/tests/multitopic.rs::a_subscription_expression_must_be_a_topic_expression`,
`crates/dcps/tests/multitopic.rs::the_topics_of_the_from_clause_must_exist_and_have_a_type_object`,
`crates/dcps/tests/multitopic.rs::topics_sharing_a_member_name_need_the_same_type`,
`crates/dcps/tests/multitopic.rs::the_result_type_must_hold_the_fields_of_the_select_list`

**Status:** `done` — `create_multitopic` (and `create_dynamic_multitopic` for `DynamicData`) reads the topics from the FROM clause of the expression; `related_topic_names` is gone. An expression that is not a TopicExpression, a topic without a TypeObject or that does not exist, a join member of two types, a field without a member, and a result type that does not hold the fields are `BadParameter` (the reason is in the event `multitopic.refused`).

### §2.2.2.2.1.9 create_multitopic — type registered beforehand

**Spec:** §2.2.2.2.1.9, p. 38 — "The resulting type is specified by the
type_name argument. Prior to creating a MultiTopic the type must have been
registered with the Service."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::the_result_type_must_hold_the_fields_of_the_select_list`

**Status:** `done` — `type_name` is a type registered with `register_type`, or `T` has a `TypeObject` under that name; otherwise `BadParameter`.

### §2.2.2.2.1.10 delete_multitopic — deleting

**Spec:** §2.2.2.2.1.10, p. 38 — "This operation deletes a MultiTopic."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_multitopic`;
`crates/dcps/src/topic.rs` (`DescriptionLife`)

**Tests:** `crates/dcps/tests/delete_description.rs::a_deleted_multitopic_releases_its_related_topics_and_answers_already_deleted`

**Status:** `done` — the delete holds for every handle: the MultiTopic no longer holds its topics; a second delete and `set_expression_parameters` return `AlreadyDeleted`.

### §2.2.2.2.1.10 delete_multitopic — PRECONDITION_NOT_MET with DataReaders

**Spec:** §2.2.2.2.1.10, p. 38 — "If the delete_multitopic operation is called on
a MultiTopic with existing DataReader objects attached to it, it will return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_multitopic_with_a_reader_is_not_deleted_and_its_topic_readers_end_with_it`

**Status:** `done` — while a DataReader on the MultiTopic exists, the delete returns `PreconditionNotMet`; once the reader is deleted (also by the end of its last handle) it is allowed.

### §2.2.2.2.1.10 delete_multitopic — another DomainParticipant

**Spec:** §2.2.2.2.1.10, p. 38 — "If delete_multitopic is called on a different
DomainParticipant, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::delete_multitopic`

**Tests:**
`crates/dcps/tests/multitopic.rs::a_multitopic_of_a_foreign_participant_is_not_deleted`

**Status:** `done` — `PreconditionNotMet`, without effect.

### §2.2.2.2.1.11 find_topic — existing or future topic

**Spec:** §2.2.2.2.1.11, p. 38 — "The operation find_topic gives access to an
existing (or ready to exist) enabled Topic, based on its name … If a Topic of
the same name already exists, it gives access to it, otherwise it waits (blocks
the caller) until another mechanism creates it (or the specified timeout
occurs)."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::find_topic`, `find_enabled_local_topic`, `find_topic_in_sedp`); `crates/dcps/src/topic.rs` (`TopicDescriptionHandle::topic`); `crates/zerodds-c-api/src/participant_ffi.rs` (`zerodds_dp_find_topic_w_timeout`)

**Tests:** `crates/dcps/tests/find_topic.rs::find_topic_gives_a_topic_that_writers_and_readers_are_created_from`, `crates/dcps/tests/find_topic.rs::find_topic_waits_for_a_topic_another_thread_creates`, `crates/dcps/src/participant.rs::tests::find_topic_returns_immediately_for_local`, `crates/dcps/src/participant.rs::tests::find_topic_resolves_after_sedp_publication`, `crates/dcps/src/participant.rs::tests::find_topic_resolves_via_sedp_subscription`, `crates/zerodds-c-api/tests/find_topic_timeout.rs::find_topic_waits_for_a_topic_and_times_out_without_one`

**Status:** done — a local enabled Topic is found at once, one that another thread creates ends the wait, a remote one is found through SEDP. The result is a `TopicDescriptionHandle`; `topic::<T>()` gives the `Topic<T>` that DataWriters and DataReaders are created from (a remote topic is created locally with the default Topic QoS).

### §2.2.2.2.1.11 find_topic — returning a Topic

**Spec:** §2.2.2.2.1.11, p. 38 — return type `Topic`; "the Topic is a local
object that acts as a ‘proxy’ to designate the global concept of topic."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescriptionHandle::topic`)

**Tests:** `crates/dcps/tests/find_topic.rs::find_topic_gives_a_topic_that_writers_and_readers_are_created_from`

**Status:** done — see the item above: the handle names the Topic, `topic::<T>()` is the Topic. The caller names `T`, since the handle is untyped.

### §2.2.2.2.1.11 find_topic — timeout

**Spec:** §2.2.2.2.1.11, p. 38 — "If the operation times-out, a ‘nil’ value (as
specified by the platform) is returned."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::find_topic`)

**Tests:** `crates/dcps/src/participant.rs::tests::find_topic_times_out_when_unknown`

**Status:** done — `Err(DdsError::Timeout)`.

### §2.2.2.2.1.11 find_topic — released with delete_topic, only the local proxy

**Spec:** §2.2.2.2.1.11, p. 38 — "A Topic obtained by means of find_topic, must
also be deleted by means of delete_topic so that the local resources can be
released … the delete_topic operation deletes only the local proxy."

**Repo:** `crates/dcps/src/participant.rs` (`find_enabled_local_topic`, `delete_topic`)

**Tests:** `crates/dcps/tests/find_topic.rs::a_topic_obtained_by_find_topic_is_deleted_as_often`

**Status:** done — `find_topic` counts as an obtaining of the local Topic; `delete_topic` on the `Topic<T>` of the handle deletes the local proxy only, and the topic is deleted by the last of as many `delete_topic` calls as it was obtained. The C ABI (`zerodds_dp_find_topic_w_timeout`) returns the same pointer and one `zerodds_dp_delete_topic` deletes it: no counting there.

### §2.2.2.2.1.12 lookup_topicdescription — finding a local topic

**Spec:** §2.2.2.2.1.12, pp. 38–39 — "The operation lookup_topicdescription gives
access to an existing locally-created TopicDescription, based on its name … If a
TopicDescription of the same name already exists, it gives access to it,
otherwise it returns a ‘nil’ value. The operation never blocks … Unlike
find_topic, the operation lookup_topicdescription searches only among the
locally created topics."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::lookup_topicdescription`,
`DomainParticipant::lookup_topicdescription_at` with an `index` over the topics of a name)

**Tests:** `crates/dcps/src/participant.rs::tests::lookup_topicdescription_returns_local_topics`,
`crates/dcps/src/participant.rs::tests::lookup_topicdescription_none_for_unknown`,
`crates/dcps/tests/topics_of_one_name.rs::lookup_iterates_over_the_topics_of_a_name`

**Status:** done — returns `Option<TopicDescriptionHandle>`; the C ABI has no `index`.

### §2.2.2.2.1.12 lookup_topicdescription — ContentFilteredTopic and MultiTopic

**Spec:** §2.2.2.2.1.12, p. 39 — "The operation lookup_topicdescription may be
used to locate any locally-created Topic, ContentFilteredTopic, and MultiTopic
object."

**Repo:** `crates/dcps/src/participant.rs` (`lookup_at`, `register_description`); `crates/dcps/src/topic.rs` (`DescriptionEntry`, `TopicDescriptionHandle::content_filtered_topic`, `::multitopic`)

**Tests:** `crates/dcps/tests/find_topic.rs::lookup_finds_a_content_filtered_topic_and_not_a_deleted_one`, `crates/dcps/tests/find_topic.rs::lookup_does_not_find_a_description_without_a_handle`, `crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done — ContentFilteredTopics and MultiTopics are listed with the participant (weakly) and found after the topics of the name; `kind()` tells them apart, `content_filtered_topic::<T>()` and `multitopic::<T>()` give one more handle on them. One without a handle, or deleted, is not found. The C ABI finds topics only.

### §2.2.2.2.1.12 lookup_topicdescription — deletion and later lookup

**Spec:** §2.2.2.2.1.12, p. 39 — "It is still possible to delete the
TopicDescription returned by lookup_topicdescription, provided it has no readers
or writers, but then it is really deleted and subsequent lookups will fail."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::delete_topic_by_name`)

**Tests:** `crates/dcps/tests/delete_topic.rs::delete_topic_by_name_follows_the_same_rules`,
`crates/dcps/src/participant.rs::tests::delete_topic_removes_the_entry_and_its_handle`

**Status:** done — the topic behind the name is deleted with
`delete_topic_by_name`; afterwards `lookup_topicdescription` returns `None`.

### §2.2.2.2.1.13 get_builtin_subscriber

**Spec:** §2.2.2.2.1.13, p. 39 — "This operation allows access to the built-in
Subscriber. Each DomainParticipant contains several built-in Topic objects as
well as corresponding DataReader objects to access them. All these DataReader
objects belong to a single built-in Subscriber."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_builtin_subscriber`);
`crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::get_builtin_subscriber_returns_subscriber`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::the_builtin_subscriber_is_a_subscriber`

**Status:** done — see §2.2.5 “the built-in Subscriber as a Subscriber”.

### §2.2.2.2.1.14 ignore_participant — built-in topics and matching

**Spec:** §2.2.2.2.1.14, p. 39 — "From that point onwards the Service will
locally behave as if the remote participant did not exist. This means it will
ignore any Topic, publication, or subscription that originates on that domain
participant."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_participant`, `IgnoreFilter`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::disconnect_ignored`, the match sites
with `IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_drops_ignored_participant_beacon`,
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_publication`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — the announcements of the participant and its endpoints no
longer reach the built-in readers; existing matches with its endpoints are
removed, new ones refused.

### §2.2.2.2.1.14 ignore_participant — no more data

**Spec:** §2.2.2.2.1.14, p. 39 — "the Service will locally behave as if the
remote participant did not exist."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — a DataWriter of the participant matched at the call is
unmatched; samples written afterwards do not arrive.

### §2.2.2.2.1.14 ignore_participant — handle from DCPSParticipant

**Spec:** §2.2.2.2.1.14, p. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples available for the built-in
DataReader to the “DCPSParticipant” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`), `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — the `instance_handle` in the SampleInfo of the
DCPSParticipant reader is `InstanceHandle::from_guid` of the participant GUID,
the same as in `get_discovered_participants` and in the filter.

### §2.2.2.2.1.14 ignore_participant — not reversible

**Spec:** §2.2.2.2.1.14, p. 39 — "The ignore_participant operation is not
required to be reversible. The Service offers no means to reverse it."

**Repo:** `crates/dcps/src/participant.rs` (`IgnoreFilterInner`: insertion
only)

**Tests:** `crates/dcps/src/participant.rs::tests::ignore_is_monotonic_and_idempotent`,
`crates/dcps/src/participant.rs::tests::ignore_participant_records_handle`

**Status:** done — the operation does not return `OUT_OF_RESOURCES`; the list
is unbounded.

### §2.2.2.2.1.15 ignore_topic — built-in topic

**Spec:** §2.2.2.2.1.15, p. 39 — "This operation allows an application to
instruct the Service to locally ignore a Topic … The Topic to ignore is
identified by the handle argument. This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSTopic” topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::ignore_topic`);
`crates/dcps/src/runtime.rs` (`push_sedp_events_to_builtin_readers`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_drops_the_publications_of_an_ignored_topic`,
`crates/dcps/src/participant.rs::tests::get_discovered_topic_data_filters_ignored`,
`crates/dcps/src/participant.rs::tests::ignore_topic_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — an ignored topic no longer reaches the DCPSTopic reader and
`get_discovered_topics`; publications and subscriptions of the topic are
unmatched and no longer matched.

### §2.2.2.2.1.15 ignore_topic — publications and subscriptions of the topic

**Spec:** §2.2.2.2.1.15, p. 39 — "This means it will locally ignore any
publication or subscription to the Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::disconnect_ignored`, the
match sites with `IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — `IgnoreFilter::ignores_endpoint` also asks for the Topic at
the match sites (the handle of the synthetic key of DCPSTopic); a matched writer
of the topic is unmatched.

### §2.2.2.2.1.16 ignore_publication

**Spec:** §2.2.2.2.1.16, p. 40 — "This operation allows an application to
instruct the Service to locally ignore a remote publication … After this call,
any data written related to that publication will be ignored … This handle is
the one that appears in the SampleInfo retrieved when reading the data-samples
from the built-in DataReader to the “DCPSPublication” topic."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_publication`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::disconnect_ignored`, the match sites with
`IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_publication`,
`crates/dcps/src/participant.rs::tests::ignore_publication_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — the publication no longer reaches the DCPSPublication
reader, an existing match is removed, a new one refused.

### §2.2.2.2.1.16 ignore_publication — data of a matched publication

**Spec:** §2.2.2.2.1.16, p. 40 — "After this call, any data written related to
that publication will be ignored."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — samples of the publication written after the call do not
arrive.

### §2.2.2.2.1.17 ignore_subscription

**Spec:** §2.2.2.2.1.17, p. 40 — "This operation allows an application to
instruct the Service to locally ignore a remote subscription … After this call,
any data received related to that subscription will be ignored."

**Repo:** `crates/dcps/src/participant.rs`
(`DomainParticipant::ignore_subscription`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::disconnect_ignored`, the match sites with
`IgnoreFilter::ignores_endpoint`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::sedp_event_push_filters_ignored_subscription`,
`crates/dcps/src/participant.rs::tests::ignore_subscription_records_handle`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — as for `ignore_publication`.

### §2.2.2.2.1.17 ignore_subscription — matched subscription

**Spec:** §2.2.2.2.1.17, p. 40 — "After this call, any data received related to
that subscription will be ignored."

**Repo:** `crates/dcps/src/runtime.rs::DcpsRuntime::disconnect_ignored`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — a matched remote DataReader is unmatched; the writes
afterwards do not reach it.

### §2.2.2.2.1.18 delete_contained_entities — deleting recursively

**Spec:** §2.2.2.2.1.18, p. 40 — "This operation deletes all the entities that
were created by means of the “create” operations on the DomainParticipant. That
is, it deletes all contained Publisher, Subscriber, Topic, ContentFilteredTopic,
and MultiTopic … delete_contained_entities on the DomainParticipant will end up
deleting all the entities recursively contained in the DomainParticipant, that
is also the DataWriter, DataReader, as well as the QueryCondition and
ReadCondition objects belonging to the contained DataReaders."

**Repo:** `crates/dcps/src/participant.rs` (`delete_contained_entities`, `delete_all_descriptions`), `crates/dcps/src/topic.rs` (`DescriptionEntry::delete_handle`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::participant_delete_contained_entities_deletes_everything_it_created`,
`crates/dcps/src/participant.rs::tests::delete_contained_entities_clears_topics_and_groups`,
`crates/dcps/tests/reader_conditions.rs::delete_contained_entities_deletes_every_condition_the_reader_made`,
`crates/dcps/tests/entity_scoping.rs::delete_contained_entities_deletes_a_content_filtered_topic`

**Status:** done — publishers, subscribers, DataWriters, DataReaders, their conditions, the topics and the ContentFilteredTopics and MultiTopics are deleted (the descriptions before the topics they use).

### §2.2.2.2.1.18 delete_contained_entities — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.18, p. 40 — "The operation will return
PRECONDITION_NOT_MET if the any of the contained entities is in a state where
it cannot be deleted."

**Repo:** `crates/dcps/src/participant.rs` (`delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET, with nothing deleted, while a GROUP publisher is inside a coherent set or a subscriber inside a group access.

### §2.2.2.2.1.19 assert_liveliness

**Spec:** §2.2.2.2.1.19, p. 40 — "This operation manually asserts the liveliness
of the DomainParticipant … This operation needs to only be used if the
DomainParticipant contains DataWriter entities with the LIVELINESS set to
MANUAL_BY_PARTICIPANT and it only affects the liveliness of those DataWriter
entities. Otherwise, it has no effect."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_liveliness`),
reachable through `DomainParticipant::runtime`

**Tests:** `crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`,
`crates/dcps/src/runtime.rs::tests::assert_liveliness_enqueues_wlp_pulse_without_panic`,
`crates/dcps/tests/entity_scoping.rs::assert_liveliness_keeps_a_manual_by_participant_writer_alive`

**Status:** done — `DomainParticipant::assert_liveliness` keeps the MANUAL_BY_PARTICIPANT writers alive; without a call the lease runs out (LIVELINESS_LOST).

### §2.2.2.2.1.19 assert_liveliness — operation on the DomainParticipant

**Spec:** §2.2.2.2.1.19, pp. 34 and 40 — operation `assert_liveliness` of the
DomainParticipant class, returning `ReturnCode_t`.

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::assert_liveliness`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::assert_liveliness_keeps_a_manual_by_participant_writer_alive`, `crates/dcps/tests/enable_semantics.rs::a_disabled_entity_is_inert_and_can_be_deleted`

**Status:** done — the operation exists on the `DomainParticipant` (NOT_ENABLED before `enable`).

### §2.2.2.2.1.20 set_default_publisher_qos

**Spec:** §2.2.2.2.1.20, p. 41 — "This operation sets a default value of the
Publisher QoS policies which will be used for newly created Publisher entities
in the case where the QoS policies are defaulted in the create_publisher
operation … if they are not, the operation will have no effect and return
INCONSISTENT_POLICY … The special value PUBLISHER_QOS_DEFAULT may be passed …
to indicate that the default QoS should be reset back to the initial values".

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_publisher_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — stored at the participant; `PUBLISHER_QOS_DEFAULT` resets
it to the specification's default. A PublisherQos cannot be inconsistent (§2.2.3
names no rule for PRESENTATION, PARTITION, GROUP_DATA and ENTITY_FACTORY).

### §2.2.2.2.1.21 get_default_publisher_qos

**Spec:** §2.2.2.2.1.21, p. 41 — "The values retrieved get_default_publisher_qos
will match the set of values specified on the last successful call to
set_default_publisher_qos, or else, if the call was never made, the default
values listed in the QoS table in 2.2.3".

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_publisher_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — the value set last, without a call the specification's
default.

### §2.2.2.2.1.22 set_default_subscriber_qos

**Spec:** §2.2.2.2.1.22, p. 41 — as set_default_publisher_qos for the
Subscriber, with INCONSISTENT_POLICY and SUBSCRIBER_QOS_DEFAULT.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_subscriber_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — as `set_default_publisher_qos`.

### §2.2.2.2.1.23 get_default_subscriber_qos

**Spec:** §2.2.2.2.1.23, p. 41 — as get_default_publisher_qos for the
Subscriber.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_subscriber_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::publisher_and_subscriber_defaults`

**Status:** `done` — as `get_default_publisher_qos`.

### §2.2.2.2.1.24 set_default_topic_qos

**Spec:** §2.2.2.2.1.24, p. 41 — as set_default_publisher_qos for the Topic,
with INCONSISTENT_POLICY and TOPIC_QOS_DEFAULT.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::set_default_topic_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — an inconsistent TopicQos (HISTORY against RESOURCE_LIMITS)
is INCONSISTENT_POLICY and leaves the default unchanged.

### §2.2.2.2.1.25 get_default_topic_qos

**Spec:** §2.2.2.2.1.25, p. 42 — as get_default_publisher_qos for the Topic.

**Repo:**
`crates/dcps/src/participant.rs::DomainParticipant::get_default_topic_qos`

**Tests:** `crates/dcps/tests/default_qos.rs::topic_default_is_checked_and_used`

**Status:** `done` — as `get_default_publisher_qos`.

### §2.2.2.2.1.26 get_domain_id

**Spec:** §2.2.2.2.1.26, p. 42 — "This operation retrieves the domain_id used to
create the DomainParticipant."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::domain_id`)

**Tests:** `crates/dcps/src/participant.rs::tests::participant_created_with_domain_id`,
`crates/dcps/src/factory.rs::tests::factory_creates_participant_with_correct_domain_id`

**Status:** done

### §2.2.2.2.1.27 get_discovered_participants

**Spec:** §2.2.2.2.1.27, p. 42 — "This operation retrieves the list of
DomainParticipants that have been discovered in the domain and that the
application has not indicated should be “ignored” … The operation may fail if
the infrastructure does not locally maintain the connectivity information. In
this case the operation will return UNSUPPORTED."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_participants`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_participants_lists_after_spdp_inject`,
`crates/dcps/src/participant.rs::tests::get_discovered_participants_offline_is_empty`

**Status:** done — without a runtime the operation returns an empty list
instead of UNSUPPORTED.

### §2.2.2.2.1.28 get_discovered_participant_data — data

**Spec:** §2.2.2.2.1.28, p. 42 — "This operation retrieves information on a
DomainParticipant that has been discovered on the network."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_participant_data`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_participants_lists_after_spdp_inject`

**Status:** done

### §2.2.2.2.1.28 get_discovered_participant_data — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.28, p. 42 — "The participant_handle must correspond to such
a DomainParticipant. Otherwise, the operation will fail and return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::a_handle_that_is_no_discovered_entity_is_precondition_not_met`

**Status:** done — an unknown or ignored handle returns PRECONDITION_NOT_MET.

### §2.2.2.2.1.29 get_discovered_topics

**Spec:** §2.2.2.2.1.29, p. 42 — "This operation retrieves the list of Topics
that have been discovered in the domain and that the application has not
indicated should be “ignored” by means of the DomainParticipant ignore_topic
operation."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_topics`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_topics_lists_unique_handles_for_pub_and_sub`,
`crates/dcps/src/participant.rs::tests::get_discovered_topic_data_filters_ignored`,
`crates/dcps/src/participant.rs::tests::get_discovered_topics_offline_is_empty`

**Status:** done

### §2.2.2.2.1.30 get_discovered_topic_data — data

**Spec:** §2.2.2.2.1.30, p. 42 — "This operation retrieves information on a
Topic that has been discovered on the network."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::get_discovered_topic_data`)

**Tests:** `crates/dcps/src/participant.rs::tests::get_discovered_topics_lists_unique_handles_for_pub_and_sub`

**Status:** done

### §2.2.2.2.1.30 get_discovered_topic_data — PRECONDITION_NOT_MET

**Spec:** §2.2.2.2.1.30, p. 42 — "The topic_handle must correspond to such a
topic. Otherwise, the operation will fail and return PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::a_handle_that_is_no_discovered_entity_is_precondition_not_met`

**Status:** done — an unknown or ignored handle returns PRECONDITION_NOT_MET.

### §2.2.2.2.1.31 contains_entity

**Spec:** §2.2.2.2.1.31, p. 43 — "This operation checks whether or not the given
a_handle represents an Entity that was created from the DomainParticipant. The
containment applies recursively."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::contains_entity`);
`crates/dcps/src/publisher.rs` (`Publisher::contains_writer`),
`crates/dcps/src/subscriber.rs` (`Subscriber::contains_reader`)

**Tests:** `crates/dcps/src/participant.rs::tests::contains_entity_returns_true_for_self_handle`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datareader`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_does_not_find_foreign_datawriter`,
`crates/dcps/src/participant.rs::tests::contains_entity_returns_false_for_topic_after_delete`

**Status:** done

### §2.2.2.2.1.32 get_current_time

**Spec:** §2.2.2.2.1.32, p. 43 — "This operation returns the current value of
the time that the service uses to time-stamp data-writes and to set the
reception-timestamp for the data-updates it receives."

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::get_current_time`,
`crates/zerodds-c-api/src/participant_ffi.rs::zerodds_dp_get_current_time`

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::get_current_time_is_the_clock_writes_are_stamped_with`,
`crates/zerodds-c-api/src/participant_ffi.rs::tests::get_current_time_returns_nonzero`

**Status:** `done` — the clock the participant stamps writes with
(`RuntimeConfig::time_source`), offline the system clock; the same in the C API.

### §2.2.2.2.1.32 get_current_time — clock of the participant

**Spec:** §2.2.2.2.1.32, p. 43 — the time "that the service uses to time-stamp
data-writes".

**Repo:** `crates/dcps/src/participant.rs::DomainParticipant::get_current_time`

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::get_current_time_is_the_clock_writes_are_stamped_with`

**Status:** `done` — reads `rt.config.time_source.now()`; the test compares it
with the source timestamp of a write under a set time source.

### §2.2.2.2.2 DomainParticipantFactory — singleton, not an entity

**Spec:** §2.2.2.2.2, p. 43 — "The sole purpose of this class is to allow the
creation and destruction of DomainParticipant objects. DomainParticipantFactory
itself has no factory. It is a pre-existing singleton object".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done

### §2.2.2.2.2.1 create_participant — creating

**Spec:** §2.2.2.2.2.1, p. 44 — "This operation creates a new DomainParticipant
object. The DomainParticipant signifies that the calling application intends to
join the Domain identified by the domain_id argument … In case of failure, the
operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::create_participant`,
`DomainParticipantFactory::create_participant_with_config`,
`ParticipantBuilder::build`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_creates_participant_with_correct_domain_id`,
`crates/dcps/src/runtime.rs::tests::a_participant_of_another_domain_id_is_not_discovered`

**Status:** done — a failure (sockets, configuration, security) is
`Err(DdsError)`. For `DomainParticipantQos` §2.2.3 names no inconsistent
combination.

### §2.2.2.2.2.1 create_participant — listener and mask

**Spec:** §2.2.2.2.2.1, p. 43 — parameters `a_listener:
DomainParticipantListener`, `mask: StatusKind []`.

**Repo:** `create_participant_with_listener`

**Tests:** `crates/dcps/tests/nil_listener.rs::create_participant_takes_a_listener_and_a_mask`

**Status:** done — `create_participant_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.2.2.1 create_participant — PARTICIPANT_QOS_DEFAULT

**Spec:** §2.2.2.2.2.1, p. 44 — "The special value PARTICIPANT_QOS_DEFAULT can
be used to indicate that the DomainParticipant should be created with the
default DomainParticipant QoS set in the factory."

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::create_participant`,
`::create_participant_with_config`, `::create_participant_offline`,
`ParticipantBuilder`

**Tests:** `crates/dcps/tests/default_qos.rs::the_participant_factory_default`

**Status:** `done` — `PARTICIPANT_QOS_DEFAULT` takes
`get_default_participant_qos`; so does `ParticipantBuilder` without `with_qos`.

### §2.2.2.2.2.2 delete_participant — deleting

**Spec:** §2.2.2.2.2.2, p. 44 — "This operation deletes an existing
DomainParticipant."

**Repo:** `crates/dcps/src/factory.rs`
(`DomainParticipantFactory::delete_participant`)

**Tests:**
`crates/dcps/src/factory.rs::tests::delete_participant_removes_from_registry`,
`crates/dcps/src/factory.rs::tests::delete_participant_unknown_returns_precondition_not_met`,
`crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`

**Status:** `done` — the participant leaves the factory, is marked deleted and
its runtime is shut down.

### §2.2.2.2.2.2 delete_participant — participant deleted afterwards

**Spec:** §2.2.2.2.2.2, p. 44 with §2.2.1.1, p. 17 — after deletion "the
operation should fail and return ALREADY_DELETED".

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::delete_participant`
(`mark_deleted`, `DcpsRuntime::shutdown`)

**Tests:** `crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`,
`crates/dcps/src/factory.rs::tests::a_deleted_participant_creates_no_publisher_or_subscriber`

**Status:** `done` — operations with a `Result` return `AlreadyDeleted`; so do `try_create_publisher` and `try_create_subscriber`, while `create_publisher` and `create_subscriber` return a deleted entity.

### §2.2.2.2.2.2 delete_participant — PRECONDITION_NOT_MET with entities

**Spec:** §2.2.2.2.2.2, p. 44 — "This operation can only be invoked if all
domain entities belonging to the participant have already been deleted.
Otherwise the error PRECONDITION_NOT_MET is returned."

**Repo:**
`crates/dcps/src/factory.rs::DomainParticipantFactory::delete_participant`
(`has_contained_entities`)

**Tests:**
`crates/dcps/src/factory.rs::tests::delete_participant_wants_its_entities_deleted_first`

**Status:** `done` — with a Publisher, Subscriber or Topic `PreconditionNotMet`,
without effect.

### §2.2.2.2.2.3 get_instance

**Spec:** §2.2.2.2.2.3, p. 44 — "This operation returns the
DomainParticipantFactory singleton. The operation is idempotent".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::instance`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done

### §2.2.2.2.2.3 get_instance — TheParticipantFactory

**Spec:** §2.2.2.2.2.3, p. 44 — "The pre-defined value TheParticipantFactory can
also be used as an alias for the singleton factory returned by the operation
get_instance."

**Repo:** `crates/dcps/src/factory.rs::THE_PARTICIPANT_FACTORY`

**Tests:**
`crates/dcps/tests/default_qos.rs::the_participant_factory_is_the_instance`

**Status:** `done` — `THE_PARTICIPANT_FACTORY` is the instance of
`DomainParticipantFactory::instance()`.

### §2.2.2.2.2.4 lookup_participant

**Spec:** §2.2.2.2.2.4, p. 44 — "This operation retrieves a previously created
DomainParticipant belonging to specified domain_id. If no such
DomainParticipant exists, the operation will return a ‘nil’ value. If multiple
DomainParticipant entities belonging to that domain_id exist, then the operation
will return one of them."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::lookup_participant`)

**Tests:** `crates/dcps/src/factory.rs::tests::lookup_participant_finds_registered_offline_participant`,
`crates/dcps/src/factory.rs::tests::lookup_participant_returns_none_for_unknown_domain`,
`crates/dcps/src/factory.rs::tests::dropping_participant_releases_it_from_the_factory`

**Status:** done

### §2.2.2.2.2.5 set_default_participant_qos

**Spec:** §2.2.2.2.2.5, p. 44 — "This operation sets a default value of the
DomainParticipant QoS policies which will be used for newly created
DomainParticipant entities in the case where the QoS policies are defaulted in
the create_participant operation … if they are not [self consistent], the
operation will have no effect and return INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::set_default_participant_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::default_participant_qos_roundtrips`

**Status:** done — the value is stored and read back; that `create_participant`
does not use it is listed under §2.2.2.2.2.1 “PARTICIPANT_QOS_DEFAULT”. For
`DomainParticipantQos` there is no inconsistent combination.

### §2.2.2.2.2.6 get_default_participant_qos

**Spec:** §2.2.2.2.2.6, p. 45 — "The values retrieved get_default_participant_qos
will match the set of values specified on the last successful call to
set_default_participant_qos, or else, if the call was never made, the default
values listed in the QoS table in 2.2.3".

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::get_default_participant_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::default_participant_qos_roundtrips`

**Status:** done

### §2.2.2.2.2.7 set_qos (factory)

**Spec:** §2.2.2.2.2.7, p. 45 — "This operation sets the value of the
DomainParticipantFactory QoS policies … Note that despite having QoS, the
DomainParticipantFactory is not an Entity … if they are not [self consistent],
the operation will have no effect and return INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::set_qos`,
`DomainParticipantFactoryQos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_set_get_qos_roundtrip`

**Status:** done — the factory QoS is ENTITY_FACTORY; it cannot be
inconsistent.

### §2.2.2.2.2.8 get_qos (factory)

**Spec:** §2.2.2.2.2.8, p. 45 — "This operation returns the value of the
DomainParticipantFactory QoS policies."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::get_qos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`,
`crates/dcps/src/factory.rs::tests::factory_set_get_qos_roundtrip`

**Status:** done

### §2.2.2.2.3 DomainParticipantListener — all callbacks

**Spec:** §2.2.2.2.3, pp. 45–46 — "The DomainParticipantListener interface
extends all other Listener interfaces and has no additional operation beyond the
ones defined by the more general listeners." Operations: on_inconsistent_topic,
on_liveliness_lost, on_offered_deadline_missed, on_offered_incompatible_qos,
on_data_on_readers, on_sample_lost, on_data_available, on_sample_rejected,
on_liveliness_changed, on_requested_deadline_missed,
on_requested_incompatible_qos, on_publication_matched,
on_subscription_matched.

**Repo:** `crates/dcps/src/listener.rs` (`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — all thirteen callbacks, each with an empty default. The
parameters (handle instead of entity) are listed under §2.2.4.3.

### §2.2.2.2.3 DomainParticipantListener — listener of last resort

**Spec:** §2.2.2.2.3, p. 46 — "When a relevant status change occurs, the DCPS
Service will first attempt to notify the listener attached to the concerned
DomainEntity if one is installed. Otherwise, the DCPS Service will notify the
Listener attached to the DomainParticipant."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (chain reader → subscriber →
participant and writer → publisher → participant)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::data_available_bubbles_all_the_way_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_bubbles_to_participant`,
`crates/dcps/tests/listener_integration.rs::push_raw_bubbles_to_participant_when_others_have_no_listener`

**Status:** done

---

## §2.2.2.3 Topic-Definition Module

### §2.2.2.3 Classes of the Topic-Definition Module

**Spec:** §2.2.2.3, p. 47 — TopicDescription, Topic, ContentFilteredTopic,
MultiTopic, TopicListener, TypeSupport.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; the classes are listed below.

### §2.2.2.3.1 TopicDescription — base of Topic, ContentFilteredTopic, MultiTopic

**Spec:** §2.2.2.3.1, pp. 47–48 — "This class is an abstract class. It is the
base class for Topic, ContentFilteredTopic, and MultiTopic … Its attribute
type_name defines a unique resulting type for the publication or the
subscription and therefore creates an implicit association with a TypeSupport.
TopicDescription has also a name that allows it to be retrieved locally."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription`, implemented for
`Topic<T>`, `ContentFilteredTopic<T>`, `MultiTopic<T>`,
`TopicDescriptionHandle`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`,
`crates/dcps/src/topic.rs::tests::cft_compiles_and_evaluates_filter`,
`crates/dcps/src/topic.rs::tests::topic_description_trait_is_object_safe`

**Status:** done

### §2.2.2.3.1.1 get_participant

**Spec:** §2.2.2.3.1.1, p. 48 — "This operation returns the DomainParticipant to
which the TopicDescription belongs."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_participant`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_description_get_participant_returns_owning_participant`

**Status:** done

### §2.2.2.3.1.2 type_name

**Spec:** §2.2.2.3.1.2, p. 48 — "The type_name used to create the
TopicDescription."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_type_name`,
`Topic::type_name`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done

### §2.2.2.3.1.3 name

**Spec:** §2.2.2.3.1.3, p. 48 — "The name used to create the TopicDescription."

**Repo:** `crates/dcps/src/topic.rs` (`TopicDescription::get_name`,
`Topic::name`)

**Tests:** `crates/dcps/src/topic.rs::tests::topic_implements_topic_description`,
`crates/dcps/src/topic.rs::tests::cft_compiles_and_evaluates_filter`

**Status:** done

### §2.2.2.3.2 Topic — name unique in the domain

**Spec:** §2.2.2.3.2, p. 49 — "A Topic is identified by its name, which must be
unique in the whole Domain. In addition (by virtue of extending
TopicDescription) it fully specifies the type of the data that can be
communicated when publishing or subscribing to the Topic."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::create_topic_with_support`:
same name with another QoS → `InconsistentPolicy`, another type with the same
QoS → a further topic);
`crates/dcps/src/topic.rs` (`inconsistent_topic_total`: counter from
discovery)

**Tests:** `crates/dcps/src/participant.rs::tests::create_topic_takes_a_second_type_and_rejects_an_inconsistent_qos`,
`crates/dcps/tests/topics_of_one_name.rs::a_second_type_under_one_name_is_a_second_topic`,
`crates/dcps/tests/topics_of_one_name.rs::an_inconsistent_qos_is_refused_and_counted`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`

**Status:** done — a further type under a name in use is a further topic if the
QoS is equal; on a different QoS it is refused and INCONSISTENT_TOPIC is
counted. Across the domain a contradiction raises INCONSISTENT_TOPIC (§2.2.4.1).

### §2.2.2.3.2 Topic — the only TopicDescription for DataWriters

**Spec:** §2.2.2.3.2, p. 49 — "Topic is the only TopicDescription that can be
used for publications and therefore associated to a DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter` takes
`&Topic<T>`)

**Tests:** `crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`

**Status:** done — by the signature.

### §2.2.2.3.2 Topic — NOT_ENABLED

**Spec:** §2.2.2.3.2, p. 49 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable and
get_status_condition may return the value NOT_ENABLED."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`, `crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — the test covers it.

### §2.2.2.3.2.1 get_inconsistent_topic_status

**Spec:** §2.2.2.3.2.1, p. 49 — "This method allows the application to retrieve
the INCONSISTENT_TOPIC status of the Topic."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`

**Status:** done — the test covers it.

### §2.2.2.3.3 ContentFilteredTopic — filter expression and parameters

**Spec:** §2.2.2.3.3, p. 49 — "The filter_expression attribute is a string that
specifies the criteria to select the data samples of interest. It is similar to
the WHERE part of an SQL clause. The expression_parameters attribute is a
sequence of strings that give values to the ‘parameters’ (i.e., "%n" tokens) in
the filter_expression. The number of supplied parameters must fit with the
requested values in the filter_expression".

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::new`,
`ContentFilteredTopic::evaluate`); `crates/sql-filter/src/parser.rs`

**Tests:** `crates/dcps/src/topic.rs::tests::cft_param_index_out_of_range_rejected`,
`crates/dcps/src/topic.rs::tests::cft_filter_with_string_param`,
`crates/dcps/src/topic.rs::tests::cft_invalid_expression_rejected`

**Status:** done — too few parameters for the `%n` used are refused; surplus
parameters are accepted.

### §2.2.2.3.3 ContentFilteredTopic — content-based subscription

**Spec:** §2.2.2.3.3, p. 49 — "ContentFilteredTopic describes a more
sophisticated subscription that indicates the subscriber does not want to
necessarily see all values of each instance published under the Topic. Rather,
it wants to see only the values whose contents satisfy certain criteria."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader_with_cft`);
`crates/dcps/src/topic.rs` (`CftEvaluator`)

**Tests:** `crates/dcps/tests/content_filtered_reader.rs::a_reader_on_a_content_filtered_topic_delivers_what_the_filter_accepts`,
`crates/dcps/tests/content_filtered_reader.rs::a_field_the_type_does_not_answer_delivers_nothing`,
`crates/dcps/tests/content_filter.rs::filter_drops_samples_that_return_false`

**Status:** done

### §2.2.2.3.3.1 get_related_topic

**Spec:** §2.2.2.3.3.1, p. 50 — "This operation returns the Topic associated
with the ContentFilteredTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_related_topic`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_get_related_topic`

**Status:** done

### §2.2.2.3.3.2 filter_expression

**Spec:** §2.2.2.3.3.2, p. 50 — "The filter_expression associated with the
ContentFilteredTopic. That is, the expression specified when the
ContentFilteredTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_filter_expression`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`

**Status:** done

### §2.2.2.3.3.3 get_expression_parameters

**Spec:** §2.2.2.3.3.3, p. 50 — "This operation returns the expression_parameters
associated with the ContentFilteredTopic. That is, the parameters specified on
the last successful call to set_expression_parameters, or if
set_expression_parameters was never called, the parameters specified when the
ContentFilteredTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::get_filter_parameters`)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`,
`crates/dcps/src/topic.rs::tests::cft_clone_shares_params`

**Status:** done — the operation is called `get_filter_parameters`.

### §2.2.2.3.3.4 set_expression_parameters

**Spec:** §2.2.2.3.3.4, p. 50 — "This operation changes the
expression_parameters associated with the ContentFilteredTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic::set_filter_parameters`:
new parameters, re-announcement per reader)

**Tests:** `crates/dcps/src/topic.rs::tests::cft_with_params_can_be_updated`,
`crates/dcps/src/topic.rs::tests::cft_set_filter_parameters_validates_count`,
`crates/dcps/src/runtime.rs::tests::the_content_filter_travels_in_the_subscription_and_is_reannounced`

**Status:** done — the operation is called `set_filter_parameters`.

### §2.2.2.3.4 MultiTopic [optional] — combining data of several topics

**Spec:** §2.2.2.3.4, pp. 50–51 — "MultiTopic allows a more sophisticated
subscription that can select and combine data received from multiple topics
into a single resulting type … The subscription_expression … is similar to an
SQL clause where the SELECT part provides the fields to be kept, the FROM part
provides the names of the topics that are searched for those fields, and the
WHERE clause gives the content filter. The Topics combined may have different
types but they are restricted in that the type of the fields used for the
NATURAL JOIN operation must be the same."

**Repo:** `crates/dcps/src/multitopic.rs`;
`crates/dcps/src/topic.rs::MultiTopic`;
`crates/dcps/src/subscriber.rs::Subscriber::create_datareader_with_multitopic`

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`,
`crates/dcps/tests/multitopic.rs::live::projection_alias_where_and_parameter`,
`crates/dcps/tests/multitopic.rs::live::the_join_runs_over_an_inherited_member`,
`crates/dcps/tests/multitopic.rs::inherited_members_are_fields_of_the_structure`,
`crates/dcps/tests/multitopic.rs::live::the_readers_of_the_topics_are_not_visible_to_the_application`

**Status:** `done` — the topics of the FROM clause are joined over all members of one name (the SQL natural join that Annex B.2 calls “NATURAL JOIN”; a cross product with no shared member), SELECT keeps and names fields, WHERE evaluates the joined row with the aliases. Members inherited from a base structure are fields of the structure. The readers of the topics belong to the reader of the MultiTopic and are not visible to the application. Model: `crates/dcps/src/multitopic.rs`.

### §2.2.2.3.4 MultiTopic — number of parameters

**Spec:** §2.2.2.3.4, p. 51 — "The number of supplied parameters must fit with
the requested values in the subscription_expression (i.e., the number of %n
tokens)."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::new`,
`MultiTopic::set_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.4 MultiTopic — DataReaders are notified

**Spec:** §2.2.2.3.4, p. 51 — "DataReader entities associated with a MultiTopic
are alerted of data modifications by the usual Listener or Condition mechanisms
… whenever modifications occur to the data associated with any of the topics
relevant to the MultiTopic."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`

**Status:** `done` — the readers of the topics of the FROM clause tell the reader of the MultiTopic about arrivals (`on_data_available`, conditions); a DataReader on a MultiTopic: `Subscriber::create_datareader_with_multitopic`.

### §2.2.2.3.4 MultiTopic — constructed instances

**Spec:** §2.2.2.3.4, p. 51 — "DataReader entities associated with a MultiTopic
access instances that are “constructed” at the DataReader side from the
instances written by multiple DataWriter entities. The MultiTopic access
instance will begin to exist as soon as all the constituting Topic instances are
in existence."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_reader_delivers_the_natural_join_of_two_topics`

**Status:** `done` — a result arises as soon as instances of all the other topics exist for an instance of one topic; an order with no customer waits.

### §2.2.2.3.4 MultiTopic — view_state of the constructed instance

**Spec:** §2.2.2.3.4, p. 51 — "The view_state … of the MultiTopic instance is
NEW if at least one of the constituting instances has view_state = NEW,
otherwise it will be NOT_NEW."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_new_constituent_makes_the_result_instance_new`

**Status:** `done` — NEW if at least one constituting instance is NEW, otherwise NOT_NEW. A constituting instance is NEW from its first sample and again from the first one after it was not alive, until a result sample has been made of it; a result sample made of a NEW constituent makes the result instance NEW again for the next read or take (`InstanceTracker::mark_view_new`).

### §2.2.2.3.4 MultiTopic — instance_state of the constructed instance

**Spec:** §2.2.2.3.4, p. 51 — "The instance_state … of the MultiTopic instance
is “ALIVE” if the instance_state of all the constituting Topic instances is
ALIVE. It is “NOT_ALIVE_DISPOSED” if at least one of the constituting Topic
instances is NOT_ALIVE_DISPOSED. Otherwise it is NOT_ALIVE_NO_WRITERS."

**Repo:** —

**Tests:** `crates/dcps/tests/multitopic.rs::live::a_disposed_constituent_disposes_the_result_instance`,
`crates/dcps/tests/multitopic.rs::live::an_unregistered_constituent_leaves_the_result_instance_without_writers`

**Status:** `done` — NOT_ALIVE_DISPOSED once a constituent is disposed, otherwise NOT_ALIVE_NO_WRITERS once one has no writers; the result instance learns of it as a marker.

### §2.2.2.3.4.1 subscription_expression

**Spec:** §2.2.2.3.4.1, p. 51 — "The subscription_expression associated with the
MultiTopic. That is, the expression specified when the MultiTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::get_subscription_expression`)

**Tests:** `crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`

**Status:** done

### §2.2.2.3.4.2 get_expression_parameters (MultiTopic)

**Spec:** §2.2.2.3.4.2, p. 51 — "This operation returns the expression_parameters
associated with the MultiTopic. That is, the parameters specified on the last
successful call to set_expression_parameters, or if set_expression_parameters
was never called, the parameters specified when the MultiTopic was created."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::get_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.4.3 set_expression_parameters (MultiTopic)

**Spec:** §2.2.2.3.4.3, p. 51 — "This operation changes the
expression_parameters associated with the MultiTopic."

**Repo:** `crates/dcps/src/topic.rs` (`MultiTopic::set_expression_parameters`)

**Tests:** `crates/dcps/tests/multitopic.rs::the_expression_parameters_are_checked_and_can_be_set`

**Status:** done

### §2.2.2.3.5 TopicListener — on_inconsistent_topic

**Spec:** §2.2.2.3.5, p. 52 — "Since Topic is a kind of Entity, it has the
ability to have an associated listener. In this case, the associated listener
should be of concrete type TopicListener." Operation `on_inconsistent_topic(the_topic:
Topic, status: InconsistentTopicStatus)`.

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener::on_inconsistent_topic`);
`crates/dcps/src/topic.rs` (`poll_inconsistent_topic`)

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_no_delta_no_listener`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_consumed_at_topic`

**Status:** done — the callback receives the handle of the topic instead of the
topic (see §2.2.4.3).

### §2.2.2.3.6 TypeSupport — generated automatically

**Spec:** §2.2.2.3.6, p. 52 — "It is required that each implementation of the
Service provides an automatic means to generate this type-specific class from a
description of the type (using IDL for example in the OMG IDL mapping)."

**Repo:** `crates/zerodds-build/src/dds_type.rs` (`dds_type_impls`: an
`impl zerodds_dcps::DdsType` generated from IDL); `crates/dcps/src/dds_type.rs`
(`DdsType`)

**Tests:** `crates/zerodds-build/tests/compile.rs::a_struct_becomes_a_type_with_a_codec`,
`crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/zerodds-build/tests/dds_type.rs::a_keyed_type_carries_its_holder_and_the_step_five_branch`

**Status:** done

### §2.2.2.3.6.1 register_type

**Spec:** §2.2.2.3.6, p. 52 — "A TypeSupport must be registered using the
register_type operation on this type-specific class before it can be used to
create Topic objects."; §2.2.2.3.6.1, p. 52 — "This operation allows an
application to communicate to the Service the existence of a data type … This
includes in particular the key definition".

**Repo:** `crates/dcps/src/type_support.rs` (`TypeSupport::register_type`), `crates/dcps/src/participant.rs` (`register_type_support`)

**Tests:** `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`, `crates/dcps/tests/type_support.rs::the_type_support_names_its_type`

**Status:** done — `TypeSupport::<T>::register_type(&participant, Option<&str>)`; the key and the encoding are in the `DdsType` impl.

### §2.2.2.3.6.1 register_type — the same name for two TypeSupports

**Spec:** §2.2.2.3.6.1, p. 52 — "It is a pre-condition error to use the same
type_name to register two different TypeSupport with the same
DomainParticipant. If an application attempts this, the operation will fail and
return PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/type_support.rs::one_type_name_is_not_two_type_supports`

**Status:** done — the same name for two different TypeSupports at the same participant returns PRECONDITION_NOT_MET; at another participant the name is free.

### §2.2.2.3.6.1 register_type — repeated, other name, nil

**Spec:** §2.2.2.3.6.1, p. 52 — "it is allowed to register the same TypeSupport
multiple times with a DomainParticipant using the same or different values for
the type_name … the second (and subsequent) registrations are ignored but the
operation returns OK. The application may pass nil as the value for the
type_name. In this case the default type-name … will be used."

**Repo:** —

**Tests:** `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`

**Status:** done — the same TypeSupport several times under the same or another name is ignored and returns OK; `None` (nil) uses the default name.

### §2.2.2.3.6.2 get_type_name

**Spec:** §2.2.2.3.6.2, p. 53 — "This operation returns the default name for the
data-type represented by the TypeSupport."

**Repo:** `crates/dcps/src/dds_type.rs` (`DdsType::TYPE_NAME`,
`DdsType::type_name`)

**Tests:** `crates/dcps/src/dds_type.rs::tests::raw_bytes_type_name_is_namespaced`,
`crates/dcps/src/publisher.rs::tests::datawriter_type_name_follows_the_topic_type`

**Status:** done

### §2.2.2.3.7 Derived classes per application type

**Spec:** §2.2.2.3.7, p. 53 — "For each data class defined by the application,
there is a number of specialized classes that are required to facilitate the
type-safe interaction of the application with the Service. It is required that
each implementation of the Service provides an automatic means to generate all
these type-specific classes." (FooTypeSupport, FooDataWriter, FooDataReader)

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`),
`crates/dcps/src/subscriber.rs` (`DataReader<T>`),
`crates/zerodds-build/src/dds_type.rs` (`dds_type_impls`)

**Tests:** `crates/zerodds-build/tests/dds_type.rs::issue_41_a_cross_module_type_is_a_topic_type`,
`crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`

**Status:** done — the typed classes are generic types over `T: DdsType`; only
the `DdsType` impl is generated.

---

## §2.2.2.4 Publication Module

### §2.2.2.4 Classes of the Publication Module

**Spec:** §2.2.2.4, p. 54 — Publisher, DataWriter, PublisherListener,
DataWriterListener.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; the classes are listed below.

### §2.2.2.4.1 Publisher — dissemination of publications

**Spec:** §2.2.2.4.1, pp. 55–56 — "A Publisher is the object responsible for the
actual dissemination of publications … When it is informed of a change to the
data associated with one of its DataWriter objects, it decides when it is
appropriate to actually send the data-update message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description of the role; the operations are
listed below. The rule on NOT_ENABLED (p. 56) is under §2.2.2.1.1.7
“NOT_ENABLED on DomainParticipant, Publisher, Subscriber, Topic”.

### §2.2.2.4.1.1 set_listener (Publisher)

**Spec:** §2.2.2.4.1.1, p. 56 — "a Publisher can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend PublisherListener."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`

**Status:** done — afterwards; not at creation (see §2.2.2.2.1.1 “listener and
mask”).

### §2.2.2.4.1.2 get_listener (Publisher)

**Spec:** §2.2.2.4.1.2, p. 56 — "Retrieves the attached PublisherListener."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::publisher_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.4.1.3 set_qos (Publisher)

**Spec:** §2.2.2.4.1.3, p. 56 — "a Publisher can be given QoS at creation time
or later by using the set_qos operation … Possible error codes returned in
addition to the standard ones: IMMUTABLE_POLICY, INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`

**Status:** done — PRESENTATION after `enable` → `ImmutablePolicy`; a new
PARTITION or GROUP_DATA reaches the DataWriters of the publisher.

### §2.2.2.4.1.4 get_qos (Publisher)

**Spec:** §2.2.2.4.1.4, p. 56 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::get_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_set_qos_pre_and_post_enable_works`

**Status:** done

### §2.2.2.4.1.5 create_datawriter — creating

**Spec:** §2.2.2.4.1.5, p. 56 — "This operation creates a DataWriter. The
returned DataWriter will be attached and belongs to the Publisher. The
DataWriter returned by the create_datawriter operation will in fact be a
derived class, specific to the data-type associated with the Topic … In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::create_datawriter`,
`WriterRegistration`)

**Tests:** `crates/dcps/src/publisher.rs::tests::publisher_creates_datawriter_for_matching_type`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datawriter`,
`crates/dcps/tests/enable_after_create.rs::a_writer_created_disabled_is_announced_by_its_enable_with_the_qos_it_has_then`

**Status:** done — returns `Result<DataWriter<T>>`; refused are, among others, a
deleted publisher, a DATA_REPRESENTATION that is not carried out and a
PERSISTENT writer without a store.

### §2.2.2.4.1.5 create_datawriter — listener and mask

**Spec:** §2.2.2.4.1.5, p. 55 — parameters `a_listener: DataWriterListener`,
`mask: StatusKind []`.

**Repo:** `create_datawriter_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_datawriter_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.4.1.5 create_datawriter — DATAWRITER_QOS_DEFAULT

**Spec:** §2.2.2.4.1.5, p. 56 — "The special value DATAWRITER_QOS_DEFAULT can be
used to indicate that the DataWriter should be created with the default
DataWriter QoS set in the factory."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — `DATAWRITER_QOS_DEFAULT` takes
`get_default_datawriter_qos`.

### §2.2.2.4.1.5 create_datawriter — DATAWRITER_QOS_USE_TOPIC_QOS

**Spec:** §2.2.2.4.1.5, p. 57 — "The special value DATAWRITER_QOS_USE_TOPIC_QOS
can be used to indicate that the DataWriter should be created with a combination
of the default DataWriter QoS and the Topic QoS."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`
(`EndpointQosArg::UseTopicQos`)

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — the publisher's default with the Topic QoS copied over it
by `copy_from_topic_qos`.

### §2.2.2.4.1.5 create_datawriter — topic of another DomainParticipant

**Spec:** §2.2.2.4.1.5, p. 57 — "The Topic passed to this operation must have
been created from the same DomainParticipant that was used to create this
Publisher. If the Topic was created from a different DomainParticipant, the
operation will fail and return a nil result."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::create_datawriter`
(`Topic::created_by_another_than`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::an_endpoint_cannot_use_the_topic_of_another_participant`

**Status:** `done` — `PreconditionNotMet`, no writer.

### §2.2.2.4.1.6 delete_datawriter — deleting

**Spec:** §2.2.2.4.1.6, p. 57 — "This operation deletes a DataWriter that
belongs to the Publisher."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`,
`WriterInner::delete`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_deletes_the_writer_behind_every_handle`,
`crates/dcps/tests/entity_deletion.rs::the_last_handle_deletes_the_writer`

**Status:** done

### §2.2.2.4.1.6 delete_datawriter — another Publisher

**Spec:** §2.2.2.4.1.6, p. 57 — "If delete_datawriter is called on a different
Publisher, the operation will have no effect and it will return
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_writer_of_another_publisher_is_left_as_it_was`

**Status:** done

### §2.2.2.4.1.6 delete_datawriter — unregistering and disposing instances

**Spec:** §2.2.2.4.1.6, p. 57 — "The deletion of the DataWriter will
automatically unregister all instances. Depending on the settings of the
WRITER_DATA_LIFECYCLE QosPolicy, the deletion of the DataWriter may also dispose
all instances."

**Repo:** `crates/dcps/src/publisher.rs` (`WriterInner::delete`,
`WriterInner::unregister_all_instances`); switch
`crates/dcps/src/runtime.rs` (`RuntimeConfig::writer_unregister_on_delete`,
default `true`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_disposes_and_unregisters_its_instances`

**Status:** done — with `writer.unregister_on_delete = false` ZeroDDS does not
send the unregistrations (for `Test_FinalInstanceState_2` of the OMG suite); the
default follows the specification.

### §2.2.2.4.1.7 lookup_datawriter

**Spec:** §2.2.2.4.1.7, p. 57 — "This operation retrieves a previously created
DataWriter belonging to the Publisher that is attached to a Topic with a
matching topic_name. If no such DataWriter exists, the operation will return
’nil.’ If multiple DataWriter attached to the Publisher satisfy this condition,
then the operation will return one of them."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::lookup_datawriter`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::lookup_datawriter_hands_out_a_handle_to_the_same_writer`

**Status:** done — the Rust operation is additionally filtered by the type `T`.

### §2.2.2.4.1.8 suspend_publications

**Spec:** §2.2.2.4.1.8, p. 57 — "It is a hint to the Service so it can optimize
its performance by e.g., holding the dissemination of the modifications and then
batching them. It is not required that the Service use this hint in any way …
If the Publisher is deleted before resume_publications is called, any suspended
updates yet to be published will be discarded."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::suspend_publications`,
`Publisher::is_suspended`)

**Tests:**
`crates/dcps/src/publisher.rs::tests::suspend_publications_sets_flag`,
`crates/dcps/src/publisher.rs::tests::two_suspends_want_two_resumes`

**Status:** `done` — a counter; the send path does not evaluate it, no changes
are held back.

### §2.2.2.4.1.9 resume_publications

**Spec:** §2.2.2.4.1.9, p. 57 — "This operation indicates to the Service that
the application has completed the multiple changes initiated by the previous
suspend_publications."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::resume_publications`)

**Tests:** `crates/dcps/src/publisher.rs::tests::resume_publications_clears_flag`

**Status:** done

### §2.2.2.4.1.9 resume_publications — PRECONDITION_NOT_MET without suspend

**Spec:** §2.2.2.4.1.9, pp. 57–58 — "The call to resume_publications must match a
previous call to suspend_publications. Otherwise the operation will return the
error PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::resume_publications`,
`crates/zerodds-c-api/src/publisher_ffi.rs::zerodds_pub_resume_publications`

**Tests:**
`crates/dcps/src/publisher.rs::tests::resume_without_suspend_is_precondition_not_met`

**Status:** `done` — `resume_publications` without an open
`suspend_publications` is `PreconditionNotMet`, also in the C API.

### §2.2.2.4.1.10 begin_coherent_changes — coherent set

**Spec:** §2.2.2.4.1.10, p. 58 — "This operation requests that the application
will begin a ‘coherent set’ of modifications using DataWriter objects attached
to the Publisher … the receiver will only be able to access the data after all
the modifications in the set are available at the receiver end."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::begin_coherent_changes`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::begin_user_writer_coherent_set`);
`crates/dcps/src/group_order.rs`

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/tests/coherent_changes_e2e.rs::two_coherent_sets_are_released_one_at_a_time`,
`crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_coherent_reader_still_waits_for_end_coherent_changes`

**Status:** done — the end of the writer’s set and dispose/unregister in the set
follow DDSI-RTPS 2.5 Table 9.22 since `a5cdaa56247` and `c35d2137e80`; the GROUP
path is described in `internal/rtps/group-access-followup.md`.

### §2.2.2.4.1.10 begin_coherent_changes — incomplete set

**Spec:** §2.2.2.4.1.10, p. 58 — "In the event that such a change prevents an
entity from receiving the entire set of coherent changes, that entity must
behave as if it had received none of the set."

**Repo:** `crates/dcps/src/subscriber.rs` (discarding incomplete sets);
`crates/dcps/src/group_order.rs`

**Tests:** `crates/dcps/src/subscriber.rs::tests::an_incomplete_set_is_discarded`,
`crates/dcps/src/group_order.rs::tests::a_set_short_of_one_member_is_discarded_and_counted`

**Status:** done — since `402f4fd66e2`.

### §2.2.2.4.1.10 begin_coherent_changes — nested

**Spec:** §2.2.2.4.1.10, p. 58 — "These calls can be nested. In that case, the
coherent set terminates only with the last call to end_coherent_ changes."

**Repo:** `crates/dcps/src/publisher.rs` (`PublisherInner::coherent_depth`,
`begin_coherent_changes`, `end_coherent_changes`)

**Tests:**
`crates/dcps/src/publisher.rs::tests::nested_coherent_changes_end_with_the_last_end`

**Status:** `done` — the calls count; only the last `end_coherent_changes`
closes the set.

### §2.2.2.4.1.11 end_coherent_changes

**Spec:** §2.2.2.4.1.11, p. 58 — "This operation terminates the ‘coherent set’
initiated by the matching call to begin_coherent_ changes. If there is no
matching call to begin_coherent_ changes, the operation will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::end_coherent_changes`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::end_coherent_changes_without_begin_is_precondition_not_met`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`

**Status:** done

### §2.2.2.4.1.12 wait_for_acknowledgments (Publisher)

**Spec:** §2.2.2.4.1.12, p. 58 — "This operation blocks the calling thread until
either all data written by the reliable DataWriter entities is acknowledged by
all matched reliable DataReader entities, or else the duration specified by the
max_wait parameter elapses".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::wait_for_acknowledgments`)

**Tests:** `crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — the publisher waits for all its writers; a reader that does not acknowledge gives TIMEOUT after the time given.

### §2.2.2.4.1.13 get_participant (Publisher)

**Spec:** §2.2.2.4.1.13, p. 58 — "This operation returns the DomainParticipant
to which the Publisher belongs."

**Repo:** `crates/dcps/src/publisher.rs::Publisher::get_participant`

**Tests:**
`crates/dcps/tests/default_qos.rs::publisher_and_subscriber_know_their_participant`

**Status:** `done` — `None` once no handle of the participant remains.

### §2.2.2.4.1.14 delete_contained_entities (Publisher)

**Spec:** §2.2.2.4.1.14, pp. 58–59 — "This operation deletes all the entities
that were created by means of the “create” operations on the Publisher. That is,
it deletes all contained DataWriter objects."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::publisher_delete_contained_entities_deletes_every_writer`

**Status:** done

### §2.2.2.4.1.14 delete_contained_entities (Publisher) — PRECONDITION_NOT_MET

**Spec:** §2.2.2.4.1.14, p. 58 — "The operation will return
PRECONDITION_NOT_MET if the any of the contained entities is in a state where
it cannot be deleted."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET while a GROUP publisher is inside a coherent set.

### §2.2.2.4.1.15 set_default_datawriter_qos

**Spec:** §2.2.2.4.1.15, p. 59 — "This operation sets a default value of the
DataWriter QoS policies which will be used for newly created DataWriter entities
in the case where the QoS policies are defaulted in the create_datawriter
operation … if they are not, the operation will have no effect and return
INCONSISTENT_POLICY … DATAWRITER_QOS_DEFAULT may be passed".

**Repo:** `crates/dcps/src/publisher.rs::Publisher::set_default_datawriter_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — checks the consistency as `create_datawriter` does;
`DATAWRITER_QOS_DEFAULT` resets it.

### §2.2.2.4.1.16 get_default_datawriter_qos

**Spec:** §2.2.2.4.1.16, p. 59 — "The values retrieved by
get_default_datawriter_qos will match the set of values specified on the last
successful call to set_default_datawriter_qos, or else, if the call was never
made, the default values listed in the QoS table in 2.2.3".

**Repo:** `crates/dcps/src/publisher.rs::Publisher::get_default_datawriter_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`

**Status:** `done` — as `get_default_publisher_qos`.

### §2.2.2.4.1.17 copy_from_topic_qos (Publisher)

**Spec:** §2.2.2.4.1.17, p. 59 — "This operation copies the policies in the
a_topic_qos to the corresponding policies in the a_datawriter_qos (replacing
values in the a_datawriter_qos, if present) … This operation does not check the
resulting a_datawriter_qos for consistency."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::copy_from_topic_qos`)

**Tests:** `crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_copies_every_policy_both_carry`,
`crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_does_not_touch_writer_only_policies`,
`crates/dcps/src/publisher.rs::tests::copy_from_topic_qos_copies_durability_and_reliability`

**Status:** done — the twelve policies both QoS sets carry by §2.2.3;
TOPIC_DATA is not a DataWriter policy and is not copied.

### §2.2.2.4.2 DataWriter — one publisher, one topic, one type

**Spec:** §2.2.2.4.2, p. 61 — "A DataWriter is attached to exactly one Publisher
that acts as a factory for it. A DataWriter is bound to exactly one Topic and
therefore to exactly one data type. The Topic must exist prior to the
DataWriter’s creation. DataWriter is an abstract class. It must be specialized
for each particular application data-type".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`, `DataWriter::get_publisher`,
`DataWriter::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`,
`crates/dcps/src/publisher.rs::tests::datawriter_type_name_follows_the_topic_type`

**Status:** done

### §2.2.2.4.2 DataWriter — NOT_ENABLED

**Spec:** §2.2.2.4.2, p. 62 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable, and
get_statuscondition may return the value NOT_ENABLED."

**Repo:** `crates/dcps/src/publisher.rs` (`EntityState::check_enabled` in
`write`, `register_instance_w_timestamp`, `unregister_instance_w_timestamp`,
`dispose_w_timestamp`, `get_key_value`, `wait_for_acknowledgments`, the get
status operations)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`,
`crates/dcps/tests/status_reads.rs::a_disabled_endpoint_has_an_untriggered_status`

**Status:** done — for the contradiction of this rule with §2.2.2.1.1.7 see
there.

### §2.2.2.4.2.1 set_listener (DataWriter)

**Spec:** §2.2.2.4.2.1, p. 62 — "a DataWriter can be attached to a Listener at
creation time or later by using the set_listener operation. The attached
Listener must extend DataWriterListener."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_consumed_at_writer`

**Status:** done — afterwards; not at creation (see §2.2.2.4.1.5).

### §2.2.2.4.2.2 get_listener (DataWriter)

**Spec:** §2.2.2.4.2.2, p. 62 — "Allows access to the attached
DataWriterListener."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datawriter_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.4.2.3 set_qos (DataWriter)

**Spec:** §2.2.2.4.2.3, p. 62 — "a DataWriter can be given QoS at creation time
or later by using the set_qos operation … Possible error codes returned in
addition to the standard ones: IMMUTABLE_POLICY, INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::set_qos`,
`runtime_qos_update`); `crates/dcps/src/qos.rs` (`immutable_writer_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`,
`crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done — INCONSISTENT_POLICY is missing (§2.2.2.1.1.1).

### §2.2.2.4.2.4 get_qos (DataWriter)

**Spec:** §2.2.2.4.2.4, p. 62 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_qos`, `DataWriter::qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`

**Status:** done

### §2.2.2.4.2.5 register_instance — handle, idempotent

**Spec:** §2.2.2.4.2.5, pp. 62–63 — "It takes as a parameter an instance (to get
the key value) and returns a handle that can be used in successive write or
dispose operations … The special value HANDLE_NIL may be returned by the Service
if it does not want to allocate any handle for that instance … The operation
register_instance is idempotent. If it is called for an already registered
instance, it just returns the already allocated handle." The operation may
return TIMEOUT and OUT_OF_RESOURCES like `write`.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_instance`,
`DataWriter::register_instance_w_timestamp`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::register`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_instance_returns_stable_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_finds_registered_handle`

**Status:** done — for a type without a key the operation returns
`HANDLE_NIL`; it does not block and does not check `max_instances` (the
specification allows TIMEOUT/OUT_OF_RESOURCES, it does not require them).

### §2.2.2.4.2.5 register_instance — optional, write with HANDLE_NIL

**Spec:** §2.2.2.4.2.5, p. 63 — "The explicit use of this operation is optional
as the application may call directly the write operation and specify a
HANDLE_NIL to indicate that the ‘key’ should be examined to identify the
instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_on_write`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::write_w_timestamp_auto_registers_unknown_instance`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`

**Status:** done — `write` has no handle parameter and registers the instance
from the key (see §2.2.2.4.2.11 “handle parameter”).

### §2.2.2.4.2.6 register_instance_w_timestamp

**Spec:** §2.2.2.4.2.6, p. 63 — "This operation performs the same function as
register_instance and can be used instead of register_instance in the cases
where the application desires to specify the value for the source_timestamp."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_instance_w_timestamp`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_with_timestamp_records_timestamp`

**Status:** done — the timestamp is recorded in the instance state; a
registration does not go on the wire.

### §2.2.2.4.2.7 unregister_instance — unregistering

**Spec:** §2.2.2.4.2.7, pp. 63–64 — "This operation reverses the action of
register_instance … This operation informs the Service that the DataWriter is
not intending to modify any more of that data instance … DataReader entities
that are reading the instance will eventually receive a sample with a
NOT_ALIVE_NO_WRITERS instance state if no other DataWriter entities are writing
the instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::unregister_instance`,
`DataWriter::unregister_instance_w_timestamp`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::write_user_lifecycle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_unregister_drops_to_no_writers_when_count_zero`,
`crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** done — with `autodispose_unregistered_instances = TRUE` the
instance is also disposed (§2.2.3.21).

### §2.2.2.4.2.7 unregister_instance — HANDLE_NIL and unknown handle

**Spec:** §2.2.2.4.2.7, p. 63 — "The special value HANDLE_NIL can be used for the
parameter handle. This indicates that the identity of the instance should be
automatically deduced from the instance … If the handle does not correspond to
an existing instance the behavior is in general unspecified, but if detectable
by the Service implementation, the operation shall fail and return the
error-code 'BAD_PARAMETER.'"

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::resolve_handle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_unknown_handle_errors`

**Status:** done — `unregister_instance` and `dispose` share `resolve_handle`;
the tests go through `dispose`.

### §2.2.2.4.2.7 unregister_instance — handle of another instance

**Spec:** §2.2.2.4.2.7, p. 63 — "If the handle corresponds to an existing
instance but does not correspond to the same instance referred by the
'instance' parameter, the behavior is in general unspecified, but if detectable
by the Service implementation, the operation shall fail and return the
error-code PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/publisher.rs::DataWriter::resolve_handle`

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_mismatch_errors`

**Status:** `done` — the handle of another existing instance is
`PreconditionNotMet`, a handle of no instance `BadParameter`, for
`unregister_instance` and `dispose`.

### §2.2.2.4.2.7 unregister_instance — registering again

**Spec:** §2.2.2.4.2.7, p. 63 — "If after that, the application wants to modify
(write or dispose) the instance, it has to register it again, or else use the
special handle value HANDLE_NIL."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::register_on_write`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::register`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.4.2.7 unregister_instance — relinquishing ownership

**Spec:** §2.2.2.4.2.7, p. 64 — "If the DataWriter was the exclusive owner of
the instance, then calling unregister_instance will relinquish that ownership."

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`: markers with their
writer, `release_owner_of` on unregister); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::release_owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`

**Status:** `done` — a marker carries the GUID of its writer; when the owner
unregisters the instance, it has no owner, and the next writer takes it.

### §2.2.2.4.2.7 unregister_instance — TIMEOUT

**Spec:** §2.2.2.4.2.7, p. 64 — "This operation may block and return TIMEOUT
under the same circumstances described for the write operation … Possible error
codes returned in addition to the standard ones: TIMEOUT,
PRECONDITION_NOT_MET."

**Repo:**
`crates/dcps/src/publisher.rs::DataWriter::unregister_instance_w_timestamp`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::an_unregister_a_full_reader_holds_back_times_out`

**Status:** `done` — first the marker, then the state: if sending runs into the
limit, the result is TIMEOUT and the instance stays registered.

### §2.2.2.4.2.8 unregister_instance_w_timestamp

**Spec:** §2.2.2.4.2.8, p. 64 — "This operation performs the same function as
unregister_instance and can be used instead of unregister_instance in the cases
where the application desires to specify the value for the source_timestamp."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_timestamps_of_dispose_and_unregister_reach_the_reader`

**Status:** done — the test covers it.

### §2.2.2.4.2.9 get_key_value (DataWriter)

**Spec:** §2.2.2.4.2.9, p. 64 — "This operation can be used to retrieve the
instance key that corresponds to an instance_handle. The operation will only
fill the fields that form the key inside the key_holder instance. This operation
may return BAD_PARAMETER if the InstanceHandle_t a_handle does not correspond to
an existing data-object known to the DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_key_value`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_get_key_value_extracts_only_key`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_get_key_value_unknown_handle_errors`,
`crates/dcps/src/subscriber.rs::tests::get_key_value_uses_holder_decoder_for_writer_and_reader`

**Status:** done — returns `Result<T>` instead of an inout parameter.

### §2.2.2.4.2.9 get_key_value (DataWriter) — UNSUPPORTED

**Spec:** §2.2.2.1.1, p. 17 — UNSUPPORTED "Can only be returned by operations
that are optional."; §2.2.2.4.2.9, p. 64 names only BAD_PARAMETER.

**Repo:** —

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::get_key_value_of_an_instance_known_by_its_hash`,
`crates/dcps/tests/keyless_instance.rs::get_key_value_answers_for_the_one_instance_of_a_keyless_topic`,
`crates/zerodds-c-api/tests/get_key_value_from_hash.rs::the_key_comes_out_of_a_hash_that_spells_it_and_nothing_else_does`

**Status:** `done` — if only the KeyHash is known for the handle, the key is rebuilt from it where the hash spells it out (DDSI-RTPS 2.5 §9.6.4.8 Step 5.1), otherwise `BadParameter`; the writer of a topic without a key answers with its first sample. The same in the C API (`zerodds_dw_get_key_value`, `zerodds_dr_get_key_value`), through the type of a typed endpoint; an endpoint without a type returns `BadParameter`.

### §2.2.2.4.2.10 lookup_instance (DataWriter)

**Spec:** §2.2.2.4.2.10, p. 64 — "This operation takes as a parameter an
instance and returns a handle that can be used in subsequent operations … This
operation does not register the instance in question. If the instance has not
been previously registered, or if for any other reason the Service is unable to
provide an instance handle, the Service will return the special value
HANDLE_NIL."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::lookup_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_finds_registered_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_lookup_instance_returns_nil_for_unknown`

**Status:** done

### §2.2.2.4.2.11 write — value and source time

**Spec:** §2.2.2.4.2.11, p. 64 — "This operation modifies the value of a data
instance. When this operation is used, the Service will automatically supply the
value of the source_timestamp that is made available to DataReader objects by
means of the source_timestamp attribute inside the SampleInfo … This operation
must be provided on the specialized class that is generated for the particular
application data-type".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`,
`DataWriter::write_with_source_ts_override`)

**Tests:** `crates/dcps/src/publisher.rs::tests::datawriter_write_queues_encoded_sample`,
`crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`,
`crates/dcps/tests/time_source_destination_order.rs::test_clock_drives_destination_order_deterministically`

**Status:** done — the source time comes from `RuntimeConfig::time_source`.

### §2.2.2.4.2.11 write — liveliness

**Spec:** §2.2.2.4.2.11, p. 65 — "As a side effect, this operation asserts
liveliness on the DataWriter itself, the Publisher and the DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (write path: `last_liveliness_assert`,
`DcpsRuntime::note_participant_liveliness`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_writer_goes_not_alive_when_silent_and_back_when_it_writes`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_sees_writer_alive_again_after_resumed_publishing`

**Status:** done — the publisher has no liveliness state of its own.

### §2.2.2.4.2.11 write — handle parameter

**Spec:** §2.2.2.4.2.11, p. 65 — "The special value HANDLE_NIL can be used for
the parameter handle … If the handle corresponds to an existing instance but
does not correspond to the same instance referred by the ‘data’ parameter … the
operation shall fail and return the error-code PRECONDITION_NOT_MET. If the
handle does not correspond to an existing instance … BAD_PARAMETER."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write_with_handle`, `write_w_timestamp_with_handle`, `check_write_handle`); `crates/zerodds-c-api/src/publisher_ffi.rs` (`zerodds_dw_write`, `zerodds_dw_write_w_timestamp`, `write_handle_refused`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::write_with_a_handle_follows_the_handle_rules`, `crates/zerodds-c-api/tests/write_handle.rs::a_write_handle_must_be_the_one_of_the_key`

**Status:** done — `write` stays HANDLE_NIL; `write_with_handle` and `write_w_timestamp_with_handle` take the handle: the handle of the key goes through, that of another existing instance is `PreconditionNotMet`, one without an instance `BadParameter`. The C ABI did not read `handle` (`_handle`) and checks it the same way now.

### §2.2.2.4.2.11 write — blocking up to max_blocking_time, TIMEOUT

**Spec:** §2.2.2.4.2.11, p. 65 — "If the RELIABILITY kind is set to RELIABLE, the
write operation may block if the modification would cause data to be lost or
else cause one of the limits specified in the RESOURCE_LIMITS to be exceeded …
If max_blocking_time elapses before the DataWriter is able to store the
modification without exceeding the limits, the write operation will fail and
return TIMEOUT."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_writer_resource_limits`)

**Tests:** `crates/dcps/src/publisher.rs::tests::write_blocks_until_drain_when_reliable_max_samples_reached`,
`crates/dcps/src/runtime.rs::tests::a_limited_writer_with_an_acknowledging_reader_writes_everything`,
`crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** done

### §2.2.2.4.2.11 write — OUT_OF_RESOURCES instead of blocking

**Spec:** §2.2.2.4.2.11, p. 65 — "Instead of blocking, the write operation is
allowed to return immediately with the error code OUT_OF_RESOURCES provided the
following two conditions are met: 1. The reason for blocking would be that the
RESOURCE_LIMITS are exceeded. 2. The service determines that waiting the
max_waiting_time has no chance of freeing the necessary resources."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_write_no_acknowledgement_can_make_room_for_is_refused_at_once`,
`crates/dcps/src/publisher.rs::tests::write_returns_oor_when_best_effort_queue_full`

**Status:** done

### §2.2.2.4.2.12 write_w_timestamp

**Spec:** §2.2.2.4.2.12, pp. 65–66 — "This operation performs the same function as
write except that it also provides the value for the source_timestamp that is
made available to DataReader objects by means of the source_timestamp attribute
inside the SampleInfo." Handle rules, TIMEOUT, OUT_OF_RESOURCES as for write.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write_w_timestamp`)

**Tests:** `crates/dcps/tests/intra_runtime_timestamp_pairing.rs::write_w_timestamp_stamps_the_sample_with_the_given_timestamp_not_the_clock`,
`crates/dcps/tests/sample_info_lifecycle.rs::write_w_timestamp_auto_registers_unknown_instance`

**Status:** done — without a handle parameter, like `write` (see
§2.2.2.4.2.11).

### §2.2.2.4.2.13 dispose — disposing an instance

**Spec:** §2.2.2.4.2.13, p. 66 — "This operation requests the middleware to
delete the data … applications are made aware of the deletion by means of
operations on the DataReader objects that already knew that instance … This
operation does not modify the value of the instance. The instance parameter is
passed just for the purposes of identifying the instance. When this operation is
used, the Service will automatically supply the value of the source_timestamp".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose`,
`DataWriter::dispose_w_timestamp`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::write_user_lifecycle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`,
`crates/dcps/src/runtime.rs::tests::qr_dispose_delivers_lifecycle_to_intra_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::dispose_propagates_over_wire_to_reader`

**Status:** done

### §2.2.2.4.2.13 dispose — handle rules

**Spec:** §2.2.2.4.2.13, p. 66 — "The constraints on the values of the handle
parameter and the corresponding error behavior are the same specified for the
unregister_instance operation".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::resolve_handle`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_resolves_via_lookup`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_unknown_handle_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_nil_unregistered_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_handle_mismatch_errors`

**Status:** `done` — the rules of `unregister_instance`: HANDLE_NIL, an unknown
handle `BadParameter`, the handle of another existing instance
`PreconditionNotMet`.

### §2.2.2.4.2.13 dispose — TIMEOUT and OUT_OF_RESOURCES

**Spec:** §2.2.2.4.2.13, p. 66 — "This operation may block and return TIMEOUT
under the same circumstances described for the write operation … This operation
may return OUT_OF_RESOURCES under the same circumstances described for the write
operation".

**Repo:** `crates/dcps/src/publisher.rs::DataWriter::dispose_w_timestamp`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_a_full_reader_holds_back_times_out`

**Status:** `done` — first the marker, then the state; an error of sending
(TIMEOUT, OUT_OF_RESOURCES) goes to the caller, and the instance stays ALIVE.

### §2.2.2.4.2.13 dispose — topic without a key

**Spec:** §2.2.1.2.2, p. 21 — "If no key is provided, the data set associated
with the Topic is restricted to a single instance."; §2.2.2.4.2.13, p. 66 —
dispose disposes the instance.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::instance_key`);
`crates/dcps/src/subscriber.rs` (`DataReader::instance_key_hash`);
`crates/zerodds-c-api/src/subscriber_ffi.rs` (`resolve_instances_and_ownership`)

**Tests:** `crates/dcps/tests/keyless_instance.rs` (five),
`crates/zerodds-c-api/src/subscriber_ffi.rs::tests::a_keyless_instance_can_be_disposed`,
`crates/zerodds-c-api/src/publisher_ffi.rs::tests::a_keyless_writer_names_its_one_instance_by_the_zero_key`

**Status:** `done` — the writer registers, looks up, disposes and unregisters the one instance of a topic without a key (KeyHash of 16 zeros, not sent); the reader tracks it in the `InstanceTracker` with a handle, NEW at the first sample, NOT_ALIVE_DISPOSED and NOT_ALIVE_NO_WRITERS, in Rust and in the C API. The invalid-data sample carries the last value received; a marker before any sample is reported (`reader.lifecycle.unknown_instance`) and not taken in. `DataWriter::get_key_value` returns `Unsupported` for a topic without a key.

### §2.2.2.4.2.14 dispose_w_timestamp

**Spec:** §2.2.2.4.2.14, p. 66 — "This operation performs the same functions as
dispose except that the application provides the value for the
source_timestamp that is made available to DataReader objects by means of the
source_timestamp attribute inside the SampleInfo".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_timestamps_of_dispose_and_unregister_reach_the_reader`

**Status:** done — the test covers it.

### §2.2.2.4.2.15 wait_for_acknowledgments (DataWriter)

**Spec:** §2.2.2.4.2.15, p. 67 — "This operation is intended to be used only if
the DataWriter has RELIABILITY QoS kind set to RELIABLE. Otherwise the operation
will return immediately with RETCODE_OK. The operation wait_for_acknowledgments
blocks the calling thread until either all data written by the DataWriter is
acknowledged by all matched DataReader entities that have RELIABILITY QoS kind
RELIABLE, or else the duration specified by the max_wait parameter elapses …
TIMEOUT indicates that max_wait elapsed".

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::wait_for_acknowledgments`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_writer_all_acknowledged`,
`DcpsRuntime::wait_ack_event`)

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`,
`crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — "all acknowledged → OK" (`e2e_dcps_api`), TIMEOUT and the immediate return of a BEST_EFFORT writer are tested.

### §2.2.2.4.2.15 wait_for_acknowledgments — TIMEOUT and BEST_EFFORT

**Spec:** §2.2.2.4.2.15, p. 67 — "Otherwise the operation will return immediately
with RETCODE_OK … a return value of TIMEOUT indicates that max_wait elapsed
before all the data was acknowledged."

**Repo:** `crates/rtps/src/reliable_writer.rs` (`ReliableWriter::all_samples_acknowledged`,
counts only reader proxies with `is_reliable`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::await_user_writer_acknowledged`); `crates/dcps/src/publisher.rs`
(`DataWriter::wait_for_acknowledgments`, `Err(Timeout)` after `timeout`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::all_samples_acknowledged_asks_the_reliable_readers_only`,
`crates/dcps/src/runtime.rs::tests::wait_for_acknowledgments_times_out_for_a_reader_that_does_not_acknowledge`

**Status:** done — the writer with a hand-built RELIABLE reader that never acknowledges returns TIMEOUT after at least the time given; the BEST_EFFORT writer returns OK at once.

### §2.2.2.4.2.16 get_liveliness_lost_status

**Spec:** §2.2.2.4.2.16, p. 67 — "This operation allows access to the
LIVELINESS_LOST communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_liveliness_lost_status`)

**Tests:** `crates/dcps/src/publisher.rs::tests::an_offline_writer_polls_nothing_and_reads_the_default_status`

**Status:** done — content and counting of the status under §2.2.4.1.

### §2.2.2.4.2.17 get_offered_deadline_missed_status

**Spec:** §2.2.2.4.2.17, p. 67 — "This operation allows access to the
OFFERED_DEADLINE_MISSED communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_offered_deadline_missed_status`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`

**Status:** done

### §2.2.2.4.2.18 get_offered_incompatible_qos_status

**Spec:** §2.2.2.4.2.18, p. 67 — "This operation allows access to the
OFFERED_INCOMPATIBLE_QOS communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_offered_incompatible_qos_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.2.4.2.19 get_publication_matched_status

**Spec:** §2.2.2.4.2.19, p. 67 — "This operation allows access to the
PUBLICATION_MATCHED communication status."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_publication_matched_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done

### §2.2.2.4.2.20 get_topic

**Spec:** §2.2.2.4.2.20, p. 67 — "This operation returns the Topic associated
with the DataWriter. This is the same Topic that was used to create the
DataWriter."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done — the operation is called `topic`.

### §2.2.2.4.2.21 get_publisher

**Spec:** §2.2.2.4.2.21, p. 67 — "This operation returns the Publisher to which
the DataWriter belongs."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::get_publisher`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done

### §2.2.2.4.2.22 assert_liveliness (DataWriter)

**Spec:** §2.2.2.4.2.22, p. 67 — "This operation manually asserts the liveliness
of the DataWriter … This operation need only be used if the LIVELINESS setting
is either MANUAL_BY_PARTICIPANT or MANUAL_BY_TOPIC. Otherwise, it has no
effect."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::assert_liveliness`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_writer_liveliness_eid`)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`,
`crates/dcps/tests/listener_trigger_c22c.rs::assert_liveliness_offline_no_op`

**Status:** done — without a return value.

### §2.2.2.4.2.23 get_matched_subscription_data

**Spec:** §2.2.2.4.2.23, p. 68 — "This operation retrieves information on a
subscription that is currently “associated” with the DataWriter … The
subscription_handle must correspond to a subscription currently associated with
the DataWriter, otherwise the operation will fail and return BAD_PARAMETER."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::matched_subscription_data`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`,
`crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — the sub-items below it are done; see their tests.

### §2.2.2.4.2.23 get_matched_subscription_data — BAD_PARAMETER

**Spec:** §2.2.2.4.2.23, p. 68 — "otherwise the operation will fail and return
BAD_PARAMETER."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — the test covers it.

### §2.2.2.4.2.24 get_matched_subscriptions

**Spec:** §2.2.2.4.2.24, p. 68 — "This operation retrieves the list of
subscriptions currently “associated” with the DataWriter; that is, subscriptions
that have a matching Topic and compatible QoS that the application has not
indicated should be “ignored” … These handles match the ones that appear in the
‘instance_handle’ field of the SampleInfo when reading the “DCPSSubscriptions”
builtin topic."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::matched_subscriptions`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_writer_matched_subscription_handles`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`, `crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches_over_the_wire`

**Status:** done — the handles of the list are the `instance_handle` in the SampleInfo of the DCPSSubscription reader (in one process and over the wire).

### §2.2.2.4.3 PublisherListener

**Spec:** §2.2.2.4.3, p. 68 — "Since a Publisher is a kind of Entity, it has the
ability to have a listener associated with it. In this case, the associated
listener should be of concrete type PublisherListener."

**Repo:** `crates/dcps/src/listener.rs` (`PublisherListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::publisher_listener_is_object_safe`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`

**Status:** done — with the four DataWriter callbacks (§2.2.4.3).

### §2.2.2.4.4 DataWriterListener

**Spec:** §2.2.2.4.4, p. 69 — operations on_liveliness_lost,
on_offered_deadline_missed, on_offered_incompatible_qos,
on_publication_matched, each with `the_writer: DataWriter` and the status.

**Repo:** `crates/dcps/src/listener.rs` (`DataWriterListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datawriter_listener_is_object_safe`,
`crates/dcps/tests/listener_trigger_c22c.rs::offered_deadline_missed_listener_fires_on_delta`,
`crates/dcps/tests/listener_trigger_c22c.rs::liveliness_lost_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::offered_incompatible_qos_dispatcher_fires_listener`,
`crates/dcps/tests/listeners_fire_without_polling.rs::matched_listeners_fire_on_match_and_unmatch_without_a_poll`

**Status:** done — the callbacks receive the handle of the writer instead of the
writer (§2.2.4.3).

### §2.2.2.4.5 Concurrency Behavior

**Spec:** §2.2.2.4.5, p. 69 — "several DataWriter may operate in different
threads. If they share the same Publisher, the middleware guarantees that its
operations are thread-safe. However, it is not required that each requesting
thread be treated in isolation from the others (leading e.g., to several
isolated sets of coherent changes)."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher`, `DataWriter<T>`: `Send`
and `Sync` through `Arc`/`Mutex`)

**Tests:** `crates/dcps/src/publisher.rs::tests::two_writers_of_one_publisher_write_concurrently`,
`crates/dcps/src/publisher.rs::tests::two_datawriters_on_one_publisher_write_concurrently`

**Status:** done

---

## §2.2.2.5 Subscription Module

### §2.2.2.5 Classes of the Subscription Module

**Spec:** §2.2.2.5, pp. 70–71 — Subscriber, DataReader, DataSample, SampleInfo,
SubscriberListener, DataReaderListener, ReadCondition, QueryCondition.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; the classes are listed below.

### §2.2.2.5.1 Access to the data — read and take

**Spec:** §2.2.2.5.1, p. 71 — "The general semantics of the “read” operations
is that the application only gets access to the corresponding data; the data
remains the middleware’s responsibility and can be read again. The semantics of
the “take” operation is that the application takes full responsibility for the
data; that data will no longer be accessible to the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read`, `DataReader::take`,
`DataReader::read_with_info`, `DataReader::take_with_info`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_read_preserves_samples`,
`crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/query_condition.rs::read_w_condition_preserves_cache_state`

**Status:** done

### §2.2.2.5.1 Access to the data — ordered collection of Data and SampleInfo

**Spec:** §2.2.2.5.1, p. 71 — "Each of these operations returns an ordered
collection of Data values and associated SampleInfo objects … Multiple samples
can refer to the same instance if the settings of the HISTORY QoS allow for
it."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_with_info`,
`DataReader::read_with_info`); `crates/dcps/src/sample.rs` (`Sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_n_keeps_the_newest_n_per_instance`

**Status:** done — plain `read` and `take` return only the data, `*_with_info`
the pairs.

### §2.2.2.5.1.1 Interpretation of the SampleInfo — content

**Spec:** §2.2.2.5.1.1, pp. 71–72 — the SampleInfo holds sample_state,
view_state, instance_state, valid_data, disposed_generation_count,
no_writers_generation_count, sample_rank, generation_rank,
absolute_generation_rank, source_timestamp.

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — the meaning of the fields is listed per field below.

### §2.2.2.5.1.2 sample_state READ / NOT_READ

**Spec:** §2.2.2.5.1.2, p. 72 — "For each sample received, the middleware
internally maintains a sample_state relative to each DataReader. The
sample_state can either be READ or NOT_READ. READ indicates that the DataReader
has already accessed that sample by means of read. NOT_READ indicates that the
DataReader has not accessed that sample before."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleStateKind`);
`crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/src/sample_info.rs::tests::sample_state_predicate`

**Status:** done

### §2.2.2.5.1.3 instance_state ALIVE

**Spec:** §2.2.2.5.1.3, p. 72 — "ALIVE indicates that (a) samples have been
received for the instance, (b) there are live DataWriter entities writing the
instance, and (c) the instance has not been explicitly disposed (or else more
samples have been received after it was disposed)."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::note_admitted`,
`InstanceTracker::observe_sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`,
`crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`

**Status:** done

### §2.2.2.5.1.3 instance_state NOT_ALIVE_DISPOSED

**Spec:** §2.2.2.5.1.3, p. 72 — "NOT_ALIVE_DISPOSED indicates the instance was
explicitly disposed by a DataWriter by means of the dispose operation."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::dispose`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`

**Status:** done

### §2.2.2.5.1.3 instance_state NOT_ALIVE_NO_WRITERS

**Spec:** §2.2.2.5.1.3, p. 72 — "NOT_ALIVE_NO_WRITERS indicates the instance has
been declared as not-alive by the DataReader because it detected that there are
no live DataWriter entities writing that instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_from`,
`DataReader::apply_unmatched_writers`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::unregister_writer`, `InstanceTracker::drop_writer`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_of_one_of_two_writers_leaves_the_instance_alive`,
`crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`

**Status:** `done` — reasons: the unregister of the instance's last writer, the
end of the match, the loss of liveliness. The unregister of one of several
writers does not change the state.

### §2.2.2.5.1.3 instance_state NOT_ALIVE_NO_WRITERS on loss of liveliness

**Spec:** §2.2.2.5.1.3, p. 72 — "because it detected that there are no live
DataWriter entities writing that instance"; §2.2.4.2.2, p. 132 — "The loss of
liveliness of the DataWriter of an instance for which there is no other
DataWriter."

**Repo:** `crates/dcps/src/liveliness_tracker.rs`
(`LivelinessTracker::take_lost`); `crates/dcps/src/runtime.rs`
(`check_liveliness`, `DcpsRuntime::take_reader_lost_liveliness_writers`);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`

**Status:** `done` — when a writer's lease runs out, the DataReader takes it out
of its instances' writers; an instance left without one gets the
NOT_ALIVE_NO_WRITERS marker. When the writer writes again, the instance is ALIVE
(`no_writers_generation_count` 1).

### §2.2.2.5.1.3 instance_state under OWNERSHIP EXCLUSIVE

**Spec:** §2.2.2.5.1.3, p. 72 — "If OWNERSHIP is set to EXCLUSIVE, then the
instance_state becomes NOT_ALIVE_DISPOSED only if the DataWriter that “owns”
the instance explicitly disposes it. The instance_state becomes ALIVE again only
if the DataWriter that owns the instance writes it."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::passes_exclusive_ownership` for data samples, `ingest_pending` for
markers); `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** `done` — a `dispose` of a writer that does not own the instance is
not taken; a data sample makes the instance ALIVE only from the owner.

### §2.2.2.5.1.3 instance_state under OWNERSHIP SHARED

**Spec:** §2.2.2.5.1.3, p. 72 — "If OWNERSHIP is set to SHARED, then the
instance_state becomes NOT_ALIVE_DISPOSED if any DataWriter explicitly disposes
the instance. The instance_state becomes ALIVE as soon as any DataWriter writes
the instance again."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::dispose`,
`InstanceTracker::observe_sample`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`,
`crates/dcps/src/instance_tracker.rs::tests::dispose_transitions_to_disposed`

**Status:** done

### §2.2.2.5.1.3 instance_state as a snapshot at read time

**Spec:** §2.2.2.5.1.3, p. 73 — "The instance_state available in the SampleInfo
is a snapshot of the instance_state of the instance at the time the collection
was obtained (i.e., at the time read or take was called). The instance_state is
therefore be the same for all samples in the returned collection that refer to
the same instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::refresh_states`, called
by `take_filtered`, `read_filtered`, `read_w_condition`, `take_w_condition`,
`read_instance`, `take_instance`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::current_states`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::the_instance_state_of_a_sample_is_that_of_its_instance_when_read`

**Status:** `done` — before every `read`/`take` with SampleInfo each sample
carries the state of its instance at that moment; the state masks select by it.
The ReadCondition evaluates the values of the last `read`/`take` (read from the
code).

### §2.2.2.5.1.4 valid_data

**Spec:** §2.2.2.5.1.4, p. 73 — "there are situations where a DataSample
contains only the SampleInfo and does not have any associated data … The
application can distinguish wether a particular DataSample has data by
examining the value of the valid_data flag."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::valid_data`);
`crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`:
`valid_data = false`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_dispose_marker_has_invalid_data`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`

**Status:** done — plain `read` and `take` skip such samples, the
`*_with_info` variants return them.

### §2.2.2.5.1.5 Generation counters — initial value

**Spec:** §2.2.2.5.1.5, p. 73 — "The disposed_generation_count and
no_writers_generation_count are initialized to zero when the DataReader first
detects the presence of a never-seen-before instance."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceState` initial
values)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_sample_creates_new_instance_on_first_call`,
`crates/dcps/src/sample_info.rs::tests::defaults_are_alive_new_not_read`

**Status:** done

### §2.2.2.5.1.5 disposed_generation_count

**Spec:** §2.2.2.5.1.5, p. 73 — "The disposed_generation_count is incremented
each time the instance_state of the corresponding instance changes from
NOT_ALIVE_DISPOSED to ALIVE."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker::observe_sample`,
`InstanceTracker::register`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::observe_after_dispose_bumps_disposed_generation`,
`crates/dcps/src/instance_tracker.rs::tests::re_register_after_dispose_bumps_disposed_generation`

**Status:** done

### §2.2.2.5.1.5 no_writers_generation_count

**Spec:** §2.2.2.5.1.5, p. 73 — "The no_writers_generation_count is incremented
each time the instance_state of the corresponding instance changes from
NOT_ALIVE_NO_WRITERS to ALIVE."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — the test covers it.

### §2.2.2.5.1.5 Generation counters — value at reception

**Spec:** §2.2.2.5.1.5, p. 73 — "The disposed_generation_count and
no_writers_generation_count available in the SampleInfo capture a snapshot of
the corresponding counters at the time the sample was received."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`:
counters from `note_admitted`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.5.1.6 sample_rank

**Spec:** §2.2.2.5.1.6, p. 73 — "The sample_rank indicates the number of samples
of the same instance that follow the current one in the collection."

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_single_instance_three_samples`,
`crates/dcps/src/sample_info.rs::tests::compute_ranks_two_instances_are_independent`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance and the filtered variants fill the ranks with `compute_ranks`.

### §2.2.2.5.1.6 generation_rank

**Spec:** §2.2.2.5.1.6, p. 73 — "generation_rank = (MRSIC.disposed_generation_count
+ MRSIC.no_writers_generation_count) - (S.disposed_generation_count +
S.no_writers_generation_count)".

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_generation_rank_against_last_sample_in_batch`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance and the filtered variants fill the ranks with `compute_ranks`.

### §2.2.2.5.1.6 absolute_generation_rank

**Spec:** §2.2.2.5.1.6, p. 74 — "absolute_generation_rank =
(MRS.disposed_generation_count + MRS.no_writers_generation_count) -
(S.disposed_generation_count + S.no_writers_generation_count)".

**Repo:** `crates/dcps/src/sample_info.rs` (`compute_ranks`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::compute_ranks_absolute_generation_rank_uses_current_not_batch`,
`crates/dcps/tests/sample_info_lifecycle.rs::rust_read_and_take_fill_the_ranks`

**Status:** done — read, take, read_instance, take_instance, next_instance and the filtered variants fill the ranks with `compute_ranks`.

### §2.2.2.5.1.7 Interpretation of the SampleInfo counters and ranks

**Spec:** §2.2.2.5.1.7, p. 74 — explanation of how an application uses counters
and ranks (sample_rank==0, generation_rank==0, absolute_generation_rank!=0).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — application guidance without a requirement.

### §2.2.2.5.1.8 view_state NEW / NOT_NEW

**Spec:** §2.2.2.5.1.8, p. 74 — "NEW indicates that either this is the first
time that the DataReader has ever accessed samples of that instance, or else
that the DataReader has accessed previous samples of the instance, but the
instance has since been reborn … NOT_NEW indicates that the DataReader has
already accessed samples of the same instance and that the instance has not
been reborn since."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`reader_view_new`,
`InstanceTracker::mark_view_seen`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`,
`crates/dcps/src/instance_tracker.rs::tests::mark_view_seen_clears_new_flag`,
`crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — the sub-items below it are done; see their tests.

### §2.2.2.5.1.8 view_state after rebirth

**Spec:** §2.2.2.5.1.8, p. 74 — "… or else that the DataReader has accessed
previous samples of the instance, but the instance has since been reborn (i.e.,
become not-alive and then alive again)."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_reborn_instance_counts_a_generation_and_is_new_again`

**Status:** done — the test covers it.

### §2.2.2.5.1.8 view_state as a snapshot at read time

**Spec:** §2.2.2.5.1.8, p. 74 — "The view_state available in the SampleInfo is
a snapshot of the view_state of the instance relative to the DataReader used to
access the samples at the time the collection was obtained … The view_state is
therefore the same for all samples in the returned collection that refer to the
same instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::refresh_states`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::current_states`,
`mark_view_seen`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::the_view_state_of_a_sample_is_that_of_its_instance_when_read`

**Status:** `done` — the first `read` gives NEW, a further `read` of the same
samples NOT_NEW; all samples of an instance in one collection carry the same
value.

### §2.2.2.5.1.8 Releasing an instance without writers

**Spec:** §2.2.2.5.1.8, p. 74 — "Once an instance has been detected as not
having any “live” writers and all the samples associated with the instance are
‘taken’ from the DataReader, the middleware can reclaim all local resources
regarding the instance. Future samples will be treated as ‘never seen’."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`);
`crates/dcps/src/instance_tracker.rs` (autopurge)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::autopurge_no_writers_after_delay`,
`crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`

**Status:** done — the specification allows the release; ZeroDDS releases an
instance by READER_DATA_LIFECYCLE (§2.2.3.22), not with the default INFINITE.

### §2.2.2.5.1 Figure 2.11 — statechart

**Spec:** §2.2.2.5.1, p. 75 — statechart of instance_state and view_state of an
instance.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the transitions are listed in the items on
§2.2.2.5.1.3, §2.2.2.5.1.5 and §2.2.2.5.1.8.

### §2.2.2.5.1.9 Data access patterns — selection by states, non-blocking

**Spec:** §2.2.2.5.1.9, p. 75 — the collection depends on the desired sample,
view and instance states; "The read and take operations are non-blocking and
just deliver what is currently available that matches the specified states."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`,
`DataReader::take_filtered`); `crates/dcps/src/sample_info.rs`
(`SampleInfo::matches_states`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`,
`crates/dcps/src/sample_info.rs::tests::matches_states_filter`

**Status:** done

### §2.2.2.5.1.9 Data access patterns — values of the state masks

**Spec:** §2.2.2.5.1.9, p. 75, with §2.3.3, p. 155 — the masks of the read/take
operations are `SampleStateMask`, `ViewStateMask`, `InstanceStateMask` with
`READ_SAMPLE_STATE = 0x0001 << 0`, `NOT_READ_SAMPLE_STATE = 0x0001 << 1`.

**Repo:** `crates/dcps/src/sample_info.rs` (`sample_state_mask`,
`view_state_mask`, `instance_state_mask`), `crates/dcps/src/psm_constants.rs`
(`state`)

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — `READ = 0x0001`, `NOT_READ = 0x0002`; the masks of the read
operations are those of the IDL PSM.

### §2.2.2.5.1.9 Data access patterns — read_w_condition and take_w_condition

**Spec:** §2.2.2.5.1.9, p. 76 — "The read_w_condition and take_w_condition
operations take a ReadCondition object as a parameter instead of sample, view,
and instance states. The behavior is that the samples returned will only be
those for which the condition is TRUE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`,
`DataReader::take_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/tests/query_condition.rs::take_w_condition_consumes_only_matching_samples`,
`crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`

**Status:** done — the operations take a `ReadCondition` or a `QueryCondition`.

### §2.2.2.5.1.9 Pattern 1 — GROUP: begin_access, get_datareaders, read/take, end_access

**Spec:** §2.2.2.5.1.9, p. 76 — "General pattern to access samples as a coherent
set and/or in order across DataReader entities. This case applies when
PRESENTATION QoS specifies “access_scope=GROUP.” … if the PRESENTATION QoS
policy specifies ordered_access=TRUE, then the list of DataReaders may return
the same reader several times."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::begin_access`,
`Subscriber::get_datareaders`, `Subscriber::end_access`,
`DataReader::check_group_access`); `crates/dcps/src/group_order.rs`

**Tests:**
`crates/dcps/tests/entity_deletion.rs::under_group_scope_get_datareaders_needs_begin_access`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_lists_a_reader_once_per_sample_in_write_order`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_in_a_group_coherent_set_waits_for_its_end_same_runtime`,
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — the pattern runs; under `ordered_access`, `get_datareaders` orders the list by the GSN (§2.2.2.5.2.10).

### §2.2.2.5.1.9 Pattern 2 — without GROUP

**Spec:** §2.2.2.5.1.9, p. 76 — "it is not required for the application to call
begin_access and end_access. However, doing so is not an error and it will have
no effect … the return of get_datareaders will be logically a “set” in the sense
that the same reader will not appear twice".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`,
`Subscriber::begin_access`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::get_datareaders_names_the_readers_that_hold_samples_in_the_states`,
`crates/dcps/src/subscriber.rs::tests::subscriber_begin_end_access_roundtrip`

**Status:** done

### §2.2.2.5.1.9 Pattern 3 — inside the SubscriberListener

**Spec:** §2.2.2.5.1.9, p. 77 — access inside `on_data_on_readers`; "The
application can also delegate the accessing of the data to the
DataReaderListener objects installed on each DataReader by calling
notify_datareaders."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::notify_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`,
`crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`

**Status:** done

### §2.2.2.5.2 Subscriber — reception of the subscriptions

**Spec:** §2.2.2.5.2, pp. 77–78 — "A Subscriber is the object responsible for the
actual reception of the data resulting from its subscriptions … it builds the
list of concerned DataReader objects, and then indicates to the application that
data is available, through its listener or by enabling related conditions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description of the role; the operations are
listed below, the rule on NOT_ENABLED (p. 78) under §2.2.2.1.1.7.

### §2.2.2.5.2.1 set_listener (Subscriber)

**Spec:** §2.2.2.5.2.1, p. 78 — "a Subscriber can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend SubscriberListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`

**Status:** done — afterwards; not at creation (§2.2.2.2.1.3).

### §2.2.2.5.2.2 get_listener (Subscriber)

**Spec:** §2.2.2.5.2.2, p. 78 — "Allows access to the attached
SubscriberListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::subscriber_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.5.2.3 set_qos (Subscriber)

**Spec:** §2.2.2.5.2.3, p. 78 — "a Subscriber can be given QoS at creation time
or later by using the set_qos operation … IMMUTABLE_POLICY,
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`,
`crates/dcps/tests/entity_lifecycle.rs::subscriber_entity_lifecycle`

**Status:** done

### §2.2.2.5.2.4 get_qos (Subscriber)

**Spec:** §2.2.2.5.2.4, p. 78 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::subscriber_entity_lifecycle`

**Status:** done

### §2.2.2.5.2.5 create_datareader — creating

**Spec:** §2.2.2.5.2.5, pp. 78–79 — "This operation creates a DataReader. The
returned DataReader will be attached and belong to the Subscriber. The
DataReader returned by the create_datareader operation will in fact be a
derived class, specific to the data-type associated with the Topic … In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::create_datareader`,
`Subscriber::create_datareader_with_cft`, `Subscriber::create_reader`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_creates_datareader_for_matching_type`,
`crates/dcps/tests/e2e_dcps_api.rs::public_api_create_subscriber_and_reader_offline`,
`crates/dcps/src/participant.rs::tests::contains_entity_recursive_finds_local_datareader`

**Status:** done — an operation of its own for a ContentFilteredTopic; for
MultiTopic see §2.2.2.3.4.

### §2.2.2.5.2.5 create_datareader — listener and mask

**Spec:** §2.2.2.5.2.5, p. 77 — parameters `a_listener: DataReaderListener`,
`mask: StatusKind []`.

**Repo:** `create_datareader_with_listener`

**Tests:** `crates/dcps/tests/listener_integration.rs::every_create_has_a_listener_and_a_mask`

**Status:** done — `create_datareader_with_listener` takes the listener and the mask; `None` with a mask is the nil listener. It creates the entity and sets the listener; what happened before is heard at the first look of the listener (the status flags stay until read or taken by a listener).

### §2.2.2.5.2.5 create_datareader — DATAREADER_QOS_DEFAULT

**Spec:** §2.2.2.5.2.5, p. 79 — "The special value DATAREADER_QOS_DEFAULT can be
used to indicate that the DataReader should be created with the default
DataReader QoS set in the factory."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::create_datareader`,
`::create_datareader_with_cft`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — `DATAREADER_QOS_DEFAULT` takes
`get_default_datareader_qos`.

### §2.2.2.5.2.5 create_datareader — DATAREADER_QOS_USE_TOPIC_QOS

**Spec:** §2.2.2.5.2.5, p. 79 — "Provided that the TopicDescription passed to
this method is a Topic or a ContentFilteredTopic, the special value
DATAREADER_QOS_USE_TOPIC_QOS can be used … It is an error to use
DATAREADER_QOS_USE_TOPIC_QOS when creating a DataReader with a MultiTopic".

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::resolve_reader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — the subscriber's default with the QoS of the Topic copied
over it, for a ContentFilteredTopic that of the related Topic. The Rust API
creates no DataReader on a MultiTopic (§2.2.2.3.4 is `open`).

### §2.2.2.5.2.5 create_datareader — TopicDescription of another DomainParticipant

**Spec:** §2.2.2.5.2.5, p. 79 — "The TopicDescription passed to this operation
must have been created from the same DomainParticipant that was used to create
this Subscriber. If the TopicDescription was created from a different
DomainParticipant, the operation will fail and return a nil result."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::create_reader`
(`Topic::created_by_another_than`)

**Tests:**
`crates/dcps/tests/delete_topic.rs::an_endpoint_cannot_use_the_topic_of_another_participant`

**Status:** `done` — `PreconditionNotMet`, no reader; for a ContentFilteredTopic
the related Topic counts.

### §2.2.2.5.2.6 delete_datareader — deleting

**Spec:** §2.2.2.5.2.6, p. 79 — "This operation deletes a DataReader that
belongs to the Subscriber … The deletion of a DataReader is not allowed if it
has any outstanding loans as a result of a call to read, take, or one of the
variants thereof."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::subscriber_delete_contained_entities_deletes_readers_and_their_conditions`,
`crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`

**Status:** done — `read` and `take` return values of their own, there are no
loans that could prevent the deletion.

### §2.2.2.5.2.6 delete_datareader — another Subscriber

**Spec:** §2.2.2.5.2.6, p. 79 — "If the DataReader does not belong to the
Subscriber, the operation returns the error PRECONDITION_NOT_MET … If
delete_datareader is called on a different Subscriber, the operation will have
no effect and it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::a_reader_of_another_subscriber_is_left_as_it_was`

**Status:** done

### §2.2.2.5.2.6 delete_datareader — PRECONDITION_NOT_MET with conditions

**Spec:** §2.2.2.5.2.6, p. 79 — "The deletion of a DataReader is not allowed if
there are any existing ReadCondition or QueryCondition objects that are attached
to the DataReader … it will return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_datareader`,
`ReaderInner::has_conditions`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datareader_refuses_a_reader_that_holds_conditions`

**Status:** done

### §2.2.2.5.2.7 lookup_datareader

**Spec:** §2.2.2.5.2.7, p. 80 — "This operation retrieves a previously-created
DataReader belonging to the Subscriber that is attached to a Topic with a
matching topic_name. If no such DataReader exists, the operation will return
’nil.’ … The use of this operation on the built-in Subscriber allows access to
the built-in DataReader entities for the built-in topics."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::lookup_datareader`);
`crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::lookup_datareader`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::lookup_datareader_hands_out_a_handle_to_the_same_reader`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — `None` for an unknown name, also at the built-in Subscriber.

### §2.2.2.5.2.8 begin_access — needed only under GROUP, nestable

**Spec:** §2.2.2.5.2.8, p. 80 — "The application is required to use this
operation only if PRESENTATION QosPolicy of the Subscriber … has the
access_scope set to ‘GROUP.’ … Calling begin_access/end_access in this case is
not considered an error and has no effect. The calls to begin_access/end_access
may be nested."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::begin_access`);
`crates/dcps/src/coherent_set.rs` (`GroupAccessScope`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_begin_access_is_nestable`,
`crates/dcps/src/coherent_set.rs::tests::group_access_nesting`

**Status:** done

### §2.2.2.5.2.8 begin_access — before the reading operations under GROUP

**Spec:** §2.2.2.5.2.8, p. 80 — "the operation begin_access must be called prior
to calling any of the sample-accessing operations, namely: get_datareaders on
the Subscriber and read, take, read_w_condition, take_w_condition on any
DataReader. Otherwise the sample-accessing operations will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`,
`DataReader::check_group_access`)

**Tests:**
`crates/dcps/tests/entity_deletion.rs::under_group_scope_get_datareaders_needs_begin_access`,
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`

**Status:** `done` — `get_datareaders`, `read`, `take`, `read_w_condition` and
`take_w_condition` are PRECONDITION_NOT_MET under GROUP without `begin_access`;
inside a data listener no `begin_access` is needed.

### §2.2.2.5.2.9 end_access

**Spec:** §2.2.2.5.2.9, p. 80 — "This call must close a previous call to
begin_access otherwise the operation will return the error
PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::end_access`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_end_access_without_begin_returns_precondition_not_met`,
`crates/dcps/src/subscriber.rs::tests::subscriber_too_many_ends_after_balanced_returns_error`

**Status:** done

### §2.2.2.5.2.10 get_datareaders — readers with samples in the states

**Spec:** §2.2.2.5.2.10, pp. 80–81 — "This operation allows the application to
access the DataReader objects that contain samples with the specified
sample_states, view_states, and instance_states … If PRESENTATION access_scope
is INSTANCE or TOPIC the returned collection is a ‘set.’"

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::get_datareaders_names_the_readers_that_hold_samples_in_the_states`

**Status:** done — the masks follow `sample_info` (see §2.2.2.5.1.9 “values of
the state masks”).

### §2.2.2.5.2.10 get_datareaders — GROUP with ordered_access as a list

**Spec:** §2.2.2.5.2.10, p. 81 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to TRUE, then the returned collection is a ‘list.’ … the
application should process each DataReader in the same order it appears in the
‘list’ and read or take exactly one sample from each DataReader."

**Repo:** —

**Tests:** `crates/dcps/tests/entity_deletion.rs::group_ordered_access_lists_a_reader_once_per_sample_in_write_order`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** `done` — under GROUP with `ordered_access` one entry per sample; samples of one group by GSN (`SampleWire::group`), otherwise by source time.

### §2.2.2.5.2.11 notify_datareaders

**Spec:** §2.2.2.5.2.11, p. 81 — "This operation invokes the operation
on_data_available on the DataReaderListener objects attached to contained
DataReader entities with a DATA_AVAILABLE status that is considered changed".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::notify_datareaders`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`

**Status:** done

### §2.2.2.5.2.12 get_sample_lost_status (Subscriber)

**Spec:** §2.2.2.5.2.12, p. 81 — "This operation allows access to the
SAMPLE_LOST communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::get_sample_lost_status`)

**Tests:** `crates/dcps/tests/subscriber_sample_lost.rs::the_subscriber_sums_the_sample_lost_status_of_its_readers`

**Status:** done — the SAMPLE_LOST status of the Subscriber is the sum over its readers; reading resets the change of each reader. The class table of the Subscriber does not list the operation, the section §2.2.2.5.2.12 describes it.

### §2.2.2.5.2.13 get_participant (Subscriber)

**Spec:** §2.2.2.5.2.13, p. 81 — "This operation returns the DomainParticipant
to which the Subscriber belongs."

**Repo:** `crates/dcps/src/subscriber.rs::Subscriber::get_participant`

**Tests:**
`crates/dcps/tests/default_qos.rs::publisher_and_subscriber_know_their_participant`

**Status:** `done` — as at the Publisher.

### §2.2.2.5.2.14 delete_contained_entities (Subscriber)

**Spec:** §2.2.2.5.2.14, p. 81 — "it deletes all contained DataReader objects.
This pattern is applied recursively … that is also the QueryCondition and
ReadCondition objects belonging to the contained DataReaders."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::subscriber_delete_contained_entities_deletes_readers_and_their_conditions`

**Status:** done

### §2.2.2.5.2.14 delete_contained_entities (Subscriber) — PRECONDITION_NOT_MET

**Spec:** §2.2.2.5.2.14, p. 81 — "The operation will return
PRECONDITION_NOT_MET if any of the contained entities is in a state where it
cannot be deleted. This will occur, for example, if a contained DataReader
cannot be deleted because the application has called a read or take operation
and has not called the corresponding return_loan operation".

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::delete_contained_entities`)

**Tests:** `crates/dcps/tests/entity_scoping.rs::delete_contained_entities_refuses_an_entity_that_cannot_be_deleted`

**Status:** done — PRECONDITION_NOT_MET while a group access (`begin_access`) is open.

### §2.2.2.5.2.15 set_default_datareader_qos

**Spec:** §2.2.2.5.2.15, pp. 81–82 — "This operation sets a default value of the
DataReader QoS policies … This operation will check that the resulting policies
are self consistent … DATAREADER_QOS_DEFAULT may be passed".

**Repo:**
`crates/dcps/src/subscriber.rs::Subscriber::set_default_datareader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — as `set_default_datawriter_qos`; the consistency includes
DEADLINE against TIME_BASED_FILTER.

### §2.2.2.5.2.16 get_default_datareader_qos

**Spec:** §2.2.2.5.2.16, p. 82 — "This operation retrieves the default value of
the DataReader QoS".

**Repo:**
`crates/dcps/src/subscriber.rs::Subscriber::get_default_datareader_qos`

**Tests:**
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — as `get_default_publisher_qos`.

### §2.2.2.5.2.17 copy_from_topic_qos (Subscriber)

**Spec:** §2.2.2.5.2.17, p. 82 — "This operation copies the policies in the
a_topic_qos to the corresponding policies in the a_datareader_qos … This
operation does not check the resulting a_datareader_qos for consistency."

**Repo:** `crates/dcps/src/subscriber.rs` (`Subscriber::copy_from_topic_qos`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::copy_from_topic_qos_copies_every_policy_both_carry`

**Status:** done — the nine policies both QoS sets carry by §2.2.3.

### §2.2.2.5.3 DataReader — subscription and data access

**Spec:** §2.2.2.5.3, pp. 82, 86 — "A DataReader allows the application (1) to
declare the data it wishes to receive (i.e., make a subscription) and (2) to
access the data received by the attached Subscriber … DataReader is an abstract
class. It must be specialized for each particular application data-type".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader<T>`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::subscriber_creates_datareader_for_matching_type`,
`crates/dcps/tests/e2e_dcps_api.rs::linux_e2e::two_participants_exchange_samples_via_public_api`

**Status:** done

### §2.2.2.5.3 DataReader — NOT_ENABLED

**Spec:** §2.2.2.5.3, p. 88 — "All operations except for the base-class
operations set_qos, get_qos, set_listener, get_listener, enable, and
get_statuscondition may return the error NOT_ENABLED."

**Repo:** `crates/dcps/src/entity.rs` (`EntityState::check_enabled`); in
`crates/dcps/src/subscriber.rs` among others `DataReader::take`,
`DataReader::read`, the get status operations

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::a_disabled_writer_and_reader_refuse_data_operations`

**Status:** done

### §2.2.2.5.3 DataReader — PRECONDITION_NOT_MET of the reading operations under GROUP

**Spec:** §2.2.2.5.3, p. 88 — "All sample-accessing operations, namely all
variants of read, take may return the error PRECONDITION_NOT_MET. The
circumstances that result on this are described in 2.2.2.5.2.8."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::check_group_access`,
called from `ingest_into_cache`)

**Tests:**
`crates/dcps/tests/group_access_e2e.rs::a_take_under_group_scope_wants_begin_access`

**Status:** `done` — every read operation checks before reading whether a
`begin_access` is open.

### §2.2.2.5.3.1 set_listener (DataReader)

**Spec:** §2.2.2.5.3.1, p. 88 — "a DataReader can be attached to a Listener at
creation time or later by using the set_listener operation. The Listener
attached must extend DataReaderListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::set_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`,
`crates/dcps/src/subscriber.rs::tests::datareader_set_listener_stores_arc_and_mask`

**Status:** done — afterwards; not at creation (§2.2.2.5.2.5).

### §2.2.2.5.3.2 get_listener (DataReader)

**Spec:** §2.2.2.5.3.2, p. 88 — "Allows access to the attached
DataReaderListener."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_listener`)

**Tests:** `crates/dcps/tests/listener_integration.rs::datareader_set_get_listener_roundtrip`

**Status:** done

### §2.2.2.5.3.3 set_qos (DataReader)

**Spec:** §2.2.2.5.3.3, p. 88 — "a DataReader can be given QoS at creation time
or later by using the set_qos operation … IMMUTABLE_POLICY,
INCONSISTENT_POLICY."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::set_qos`);
`crates/dcps/src/qos.rs` (`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done — INCONSISTENT_POLICY is missing (§2.2.2.1.1.1).

### §2.2.2.5.3.4 get_qos (DataReader)

**Spec:** §2.2.2.5.3.4, p. 88 — "Allows access to the values of the QoS."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_qos`, `DataReader::qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`

**Status:** done

### §2.2.2.5.3.5 create_readcondition

**Spec:** §2.2.2.5.3.5, p. 88 — "This operation creates a ReadCondition. The
returned ReadCondition will be attached and belong to the DataReader. In case of
failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`,
`DataReader::holds_matching`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::a_waitset_returns_a_read_condition_of_a_reader_with_data`

**Status:** done

### §2.2.2.5.3.6 create_querycondition

**Spec:** §2.2.2.5.3.6, pp. 88–89 — "This operation creates a QueryCondition. The
returned QueryCondition will be attached and belong to the DataReader. The
syntax of the query_expression and query_parameters parameters is described in
Annex B. In case of failure, the operation will return a ‘nil’ value".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_querycondition`);
`crates/dcps/src/condition.rs` (`QueryCondition::new`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`,
`crates/dcps/src/condition.rs::tests::query_condition_rejects_invalid_sql`

**Status:** done

### §2.2.2.5.3.7 delete_readcondition

**Spec:** §2.2.2.5.3.7, p. 89 — "This operation deletes a ReadCondition attached
to the DataReader. Since QueryCondition specializes ReadCondition it can also be
used to delete a QueryCondition. If the ReadCondition is not attached to the
DataReader, the operation will return the error PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::delete_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`

**Status:** done — a QueryCondition is deleted through `QueryCondition::base`.

### §2.2.2.5.3.8 read — max_samples

**Spec:** §2.2.2.5.3.8, p. 89 — "This operation accesses a collection of Data
values from the DataReader. The size of the returned collection will be limited
to the specified max_samples."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::max_samples_limits_read_and_take`, `crates/dcps/tests/read_selection.rs::no_matching_sample_is_no_data`,
`crates/dcps/tests/query_condition.rs::max_samples_limits_what_a_condition_read_or_take_returns`

**Status:** done — the operations in the form of the specification (`read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`, `read_w_condition_selected`, `take_w_condition_selected`) take `max_samples` (`LENGTH_UNLIMITED` = -1; below that `BadParameter`); what is not returned keeps its state, and with an `ORDER BY` the limit comes after the order. The older convenience forms (`read`, `take`, `read_with_info`, ...) return all.

### §2.2.2.5.3.8 read — PRESENTATION INSTANCE: samples of an instance together

**Spec:** §2.2.2.5.3.8, p. 89 — "If PRESENTATION access_scope is INSTANCE, then
the returned collection is a ‘list’ where samples belonging to the same
data-instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::the_default_presentation_groups_the_plain_take_by_instance`,
`crates/dcps/tests/ordered_access_e2e.rs::instance_scope_ordered_access_groups_the_plain_take_by_instance`

**Status:** `done` — the samples of an instance are consecutive, also with the
default.

### §2.2.2.5.3.8 read — TOPIC without ordered_access: samples of an instance together

**Spec:** §2.2.2.5.3.8, p. 89 — "If PRESENTATION access_scope is TOPIC and
ordered_access is set to FALSE, then the returned collection is a ‘list’ where
samples belonging to the same data-instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_without_ordered_access_groups_the_plain_take`

**Status:** `done` — as for INSTANCE.

### §2.2.2.5.3.8 read — TOPIC with ordered_access: write order across instances

**Spec:** §2.2.2.5.3.8, p. 89 — "If PRESENTATION access_scope is TOPIC and
ordered_access is set to TRUE, then the returned collection is a ‘list’ were
samples belonging to the same instance may or may not be consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_ordered_access_keeps_write_order_on_the_plain_take`

**Status:** `done`

### §2.2.2.5.3.8 read — GROUP without ordered_access: samples of an instance together

**Spec:** §2.2.2.5.3.8, p. 89 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to FALSE, then the returned collection is a ‘list’ where
samples belonging to the same data instance are consecutive."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`, `group_by_instance`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_without_ordered_access_groups_the_plain_take`

**Status:** `done` — as for INSTANCE.

### §2.2.2.5.3.8 read — GROUP with ordered_access: at most one sample

**Spec:** §2.2.2.5.3.8, p. 89 — "If PRESENTATION access_scope is GROUP and
ordered_access is set to TRUE, then the returned collection contains at most one
sample."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::presentation_limit`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`

**Status:** `done` — `read`, `take`, the `_w_condition` and `_instance` variants
return at most one sample, in write order across the instances.

### §2.2.2.5.3.8 read — order by DESTINATION_ORDER

**Spec:** §2.2.2.5.3.8, p. 89 — "If DESTINATION_ORDER is BY_RECEPTION_TIMESTAMP,
samples belonging to the same instances will appear in the relative order in
which there were received … If DESTINATION_ORDER is BY_SOURCE_TIMESTAMP,
samples belonging to the same instances will appear in the relative order
implied by the source_timestamp".

**Repo:** `crates/dcps/src/subscriber.rs` (admission by DESTINATION_ORDER in
`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/time_source_destination_order.rs::plain_take_enforces_destination_order`,
`crates/dcps/tests/time_source_destination_order.rs::plain_read_enforces_destination_order`,
`crates/dcps/tests/destination_order_qos.rs::by_source_timestamp_drops_older_samples`

**Status:** done — the policy itself is listed under §2.2.3.17.

### §2.2.2.5.3.8 read — collections data_values and sample_infos (len, max_len, owns, loan)

**Spec:** §2.2.2.5.3.8, pp. 89–90 — rules 1–5 for the input collections (equal
properties, loan for `max_len == 0`, copy for `owns == TRUE`,
`max_samples > max_len` → PRECONDITION_NOT_MET); "PSM mappings that do not
provide these facilities may need to change the signature of the read operation
slightly to compensate for it."

**Repo:** `crates/dcps/src/subscriber.rs` (returns `Result<Vec<Sample<T>>>`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done — the Rust API returns a `Vec` of its own; there is no input
collection and no loan, rules 1–5 do not apply by the sentence quoted.

### §2.2.2.5.3.8 read — Data and SampleInfo one to one

**Spec:** §2.2.2.5.3.8, p. 90 — "On output, the collection of Data values and
the collection of SampleInfo structures are of the same length and are in a
one-to-one correspondence."

**Repo:** `crates/dcps/src/sample.rs` (`Sample { data, info }`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done

### §2.2.2.5.3.8 read — last sample of an instance that is not alive has no data

**Spec:** §2.2.2.5.3.8, pp. 90–91 — "If the instance_state in the SampleInfo is
NOT_ALIVE_DISPOSED or NOT_ALIVE_NO_WRITERS, then the last sample for that
instance in the collection, that is, the one whose SampleInfo has sample_rank==0
does not contain valid data."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_after_the_last_sample_survives_keep_last_1`

**Status:** done

### §2.2.2.5.3.8 read — samples without data do not count for RESOURCE_LIMITS

**Spec:** §2.2.2.5.3.8, p. 91 — "Samples that contain no data do not count
towards the limits imposed by the RESOURCE_LIMITS QoS policy."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::a_dispose_marker_does_not_count_against_the_resource_limits`

**Status:** done — the test covers it.

### §2.2.2.5.3.8 read — sample_state READ, view_state NOT_NEW, instance_state unchanged

**Spec:** §2.2.2.5.3.8, p. 91 — "The act of reading a sample sets its
sample_state to READ. If the sample belongs to the most recent generation of
the instance, it will also set the view_state of the instance to NOT_NEW. It
will not affect the instance_state of the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_filtered`,
`DataReader::read`); `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::mark_view_seen`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** done

### §2.2.2.5.3.8 read — NO_DATA

**Spec:** §2.2.2.5.3.8–§2.2.2.5.3.19, pp. 91–95 — for read, take,
read_w_condition, take_w_condition, read_next_sample, take_next_sample,
read_instance, take_instance, read_next_instance, take_next_instance and the
`*_w_condition` variants: "If the DataReader has no samples that meet the
constraints, the return value will be NO_DATA."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::no_matching_sample_is_no_data`, `crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`,
`crates/dcps/tests/query_condition.rs::max_samples_limits_what_a_condition_read_or_take_returns`

**Status:** done — the same operations return `NoData` when no sample meets the constraints; the older convenience forms return `Ok` with an empty list.

### §2.2.2.5.3.9 take

**Spec:** §2.2.2.5.3.9, p. 91 — "The act of taking a sample removes it from the
DataReader so it cannot be ‘read’ or ‘taken’ again. If the sample belongs to the
most recent generation of the instance, it will also set the view_state of the
instance to NOT_NEW. It will not affect the instance_state of the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take`,
`DataReader::take_with_info`, `DataReader::take_filtered`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::datareader_take_returns_decoded_samples`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_filtered_with_state_mask_filters`

**Status:** done — limit and PRESENTATION as for `read` (see there).

### §2.2.2.5.3.10 read_w_condition — with a QueryCondition

**Spec:** §2.2.2.5.3.10, p. 91 — "This operation accesses via ‘read’ the samples
that match the criteria specified in the ReadCondition. This operation is
especially useful in combination with QueryCondition to filter data samples
based on the content."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/tests/query_condition.rs::read_w_condition_preserves_cache_state`,
`crates/dcps/tests/query_condition.rs::read_w_condition_unknown_field_drops_sample`

**Status:** done

### §2.2.2.5.3.10 read_w_condition — with a plain ReadCondition

**Spec:** §2.2.2.5.3.10, p. 91 — "In case the ReadCondition is a ‘plain’
ReadCondition and not the specialized QueryCondition, the operation is
equivalent to calling read and passing as sample_states, view_states and
instance_states the value of the corresponding attributes in the
read_condition."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_w_condition`, `ReadConditionArg` in `crates/dcps/src/condition.rs`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`

**Status:** done — a `ReadCondition` from `create_readcondition` selects by its three masks; the samples become READ (`take_w_condition` removes them).

### §2.2.2.5.3.10 read_w_condition — condition of the reader, otherwise PRECONDITION_NOT_MET

**Spec:** §2.2.2.5.3.10, p. 91 — "The specified ReadCondition must be attached
to the DataReader; otherwise the operation will fail and return
PRECONDITION_NOT_MET."

**Repo:** —

**Tests:** `crates/dcps/tests/read_api_spec.rs::a_condition_of_another_reader_is_precondition_not_met`

**Status:** done — a condition of another reader or of none returns PRECONDITION_NOT_MET, for ReadCondition and QueryCondition, `read_w_condition`, `take_w_condition` and `read_next_instance_w_condition`; nothing is read.

### §2.2.2.5.3.11 take_w_condition

**Spec:** §2.2.2.5.3.11, pp. 91–92 — "This operation is analogous to
read_w_condition except it accesses samples via the ‘take’ operation … The
specified ReadCondition must be attached to the DataReader; otherwise the
operation will fail and return PRECONDITION_NOT_MET."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_w_condition`)

**Tests:** `crates/dcps/tests/query_condition.rs::take_w_condition_consumes_only_matching_samples`,
`crates/dcps/tests/query_condition.rs::take_w_condition_empty_when_no_match`,
`crates/dcps/tests/read_api_spec.rs::a_plain_read_condition_selects_by_its_state_masks`, `crates/dcps/tests/read_api_spec.rs::a_condition_of_another_reader_is_precondition_not_met`

**Status:** done — with a QueryCondition and a plain ReadCondition; a foreign condition returns PRECONDITION_NOT_MET.

### §2.2.2.5.3.12 read_next_sample

**Spec:** §2.2.2.5.3.12, p. 92 — "This operation copies the next,
non-previously accessed Data value from the DataReader; the operation also
copies the corresponding SampleInfo … semantically equivalent to the read
operation where the input Data sequence has max_len=1, the
sample_states=NOT_READ, the view_states=ANY_VIEW_STATE, and the
instance_states=ANY_INSTANCE_STATE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_sample`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::read_next_sample_and_take_next_sample_give_one_unaccessed_sample`

**Status:** done — one sample not yet accessed (NOT_READ), in the order of arrival; NO_DATA when there is none.

### §2.2.2.5.3.13 take_next_sample

**Spec:** §2.2.2.5.3.13, p. 92 — "This operation copies the next,
non-previously accessed Data value from the DataReader and ‘removes’ it from
the DataReader so it is no longer accessible."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_sample`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::read_next_sample_and_take_next_sample_give_one_unaccessed_sample`

**Status:** done — as `read_next_sample`, and the sample leaves the reader; samples already read stay.

### §2.2.2.5.3.14 read_instance

**Spec:** §2.2.2.5.3.14, pp. 92–93 — "The behavior is identical to read except
that all samples returned belong to the single specified instance whose handle
is a_handle … This operation may return BAD_PARAMETER if the InstanceHandle_t
a_handle does not correspond to an existing data-object known to the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_instance`, `read_instance_selected`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_filters_to_one_handle`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_with_handle_nil_errors`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — `read_instance_selected` takes `max_samples` and the three state masks; `read_instance` reads all samples of the instance. `HANDLE_NIL` returns `BadParameter`, an unknown handle an empty list or NO_DATA.

### §2.2.2.5.3.14 read_instance and take_instance — state masks

**Spec:** §2.2.2.5.3.14–§2.2.2.5.3.15, pp. 87 and 92–93 — parameters
`sample_states`, `view_states`, `instance_states`, `max_samples`.

**Repo:** `crates/dcps/src/subscriber.rs` (`ReadSelection`, `DataReader::read_selected`, `take_selected`, `read_instance_selected`, `take_instance_selected`, `read_next_instance_selected`, `take_next_instance_selected`)

**Tests:** `crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`, `crates/dcps/tests/read_selection.rs::next_instance_looks_at_the_states`

**Status:** done — `read_instance_selected` and `take_instance_selected` take the three masks and `max_samples`; so do the `*_next_instance_selected`, whose next instance is the smallest handle above `previous` with samples in those states.

### §2.2.2.5.3.15 take_instance

**Spec:** §2.2.2.5.3.15, p. 93 — "The behavior is identical to take except for
that all samples returned belong to the single specified instance whose handle
is a_handle."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_instance`, `take_instance_selected`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_instance_with_handle_nil_errors`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — as `read_instance`: `take_instance_selected` with `max_samples` and the masks.

### §2.2.2.5.3.16 read_next_instance — next instance with samples

**Spec:** §2.2.2.5.3.16, p. 93 — "The behavior of read_next_instance is ‘as if’
the DataReader invoked read_instance passing the smallest instance_handle among
all the ones that (a) are greater than previous_handle and (b) have available
samples … previous_handle==HANDLE_NIL will return the samples for the instance
which has the smallest instance_handle among all the instances that contain
available samples."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_instance`,
`next_instance_with_samples`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_next_instance_iterates_all`,
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`

**Status:** `done` — the instance with the smallest handle above
`previous_handle` that has samples in the cache; HANDLE_NIL starts at the
smallest.

### §2.2.2.5.3.16 read_next_instance — instances without samples, unknown handle, ordered_access

**Spec:** §2.2.2.5.3.16, pp. 93–94 — "(b) have available samples (i.e., samples
that meet the constraints imposed by the specified states) … it is possible to
call the ‘read_next_instance’ operation with a previous_handle that does not
correspond to an instance currently managed by the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::next_instance_with_samples`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`,
`crates/dcps/tests/ordered_access_e2e.rs::take_next_instance_returns_one_instance_under_ordered_access`,
`crates/dcps/tests/read_selection.rs::instance_reads_take_masks_and_a_limit`

**Status:** done — an instance without samples is passed over, an unmanaged `previous_handle` orders by its value, under `ordered_access` one instance comes; `read_next_instance_selected` takes the masks and `max_samples`.

### §2.2.2.5.3.17 take_next_instance

**Spec:** §2.2.2.5.3.17, p. 94 — "This operation has the same behavior as
read_next_instance except that the samples are ‘taken’ from the DataReader".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_instance`)

**Tests:**
`crates/dcps/tests/sample_info_lifecycle.rs::reader_take_next_instance_consumes`,
`crates/dcps/tests/sample_info_lifecycle.rs::next_instance_passes_over_an_instance_without_samples`

**Status:** `done` — as `read_next_instance`, with `take`.

### §2.2.2.5.3.18 read_next_instance_w_condition

**Spec:** §2.2.2.5.3.18, pp. 94–95 — "The behavior is identical to
read_next_instance except that all samples returned satisfy the specified
condition."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::read_next_instance_w_condition`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::next_instance_w_condition_walks_the_instances_the_condition_accepts`

**Status:** done — the samples of the instance with the smallest handle above `previous_handle` that has samples the condition accepts; NO_DATA at the end. The spec text says ">=" here and "greater" for `read_next_instance`; the iteration to NO_DATA needs "greater", which the implementation follows.

### §2.2.2.5.3.19 take_next_instance_w_condition

**Spec:** §2.2.2.5.3.19, p. 95 — "This operation has the same behavior as
read_next_instance_w_condition except that the samples are ‘taken’ from the
DataReader".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::take_next_instance_w_condition`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::next_instance_w_condition_walks_the_instances_the_condition_accepts`

**Status:** done — as `read_next_instance_w_condition`; the accepted samples leave the reader, the other samples of the instance stay.

### §2.2.2.5.3.20 return_loan

**Spec:** §2.2.2.5.3.20, p. 95 — "This operation indicates to the DataReader
that the application is done accessing the collection of data_values and
sample_infos obtained by some earlier invocation of read or take … calling
return_loan on a collection that does not have a loan is safe and has no side
effects."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::return_loan`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::return_loan_leaves_the_reader_as_it_is`

**Status:** done — `read` and `take` of this API return the samples owned (`Vec<Sample<T>>`) and loan nothing from the reader; `return_loan` has no loan to give back and leaves the reader as it is ("calling return_loan on a collection that does not have a loan is safe and has no side effects"). After the reader is deleted it returns `AlreadyDeleted`. The C API has `zerodds_dr_return_loan`.

### §2.2.2.5.3.21 get_liveliness_changed_status

**Spec:** §2.2.2.5.3.21, p. 96 — "This operation allows access to the
LIVELINESS_CHANGED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_liveliness_changed_status`)

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.2.5.3.22 get_requested_deadline_missed_status

**Spec:** §2.2.2.5.3.22, p. 96 — "This operation allows access to the
REQUESTED_DEADLINE_MISSED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_requested_deadline_missed_status`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done

### §2.2.2.5.3.23 get_requested_incompatible_qos_status

**Spec:** §2.2.2.5.3.23, p. 96 — "This operation allows access to the
REQUESTED_INCOMPATIBLE_QOS communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_requested_incompatible_qos_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.2.5.3.24 get_sample_lost_status

**Spec:** §2.2.2.5.3.24, p. 96 — "This operation allows access to the
SAMPLE_LOST communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_sample_lost_status`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::an_offline_reader_polls_nothing_and_reads_the_default_status`

**Status:** done — content and counting under §2.2.4.1.

### §2.2.2.5.3.25 get_sample_rejected_status

**Spec:** §2.2.2.5.3.25, p. 96 — "This operation allows access to the
SAMPLE_REJECTED communication status."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::get_sample_rejected_status_reports_the_change_once`

**Status:** done — the test covers it.

### §2.2.2.5.3.26 get_subscription_matched_status

**Spec:** §2.2.2.5.3.26, p. 96 — "This operation allows access to the
SUBSCRIPTION_MATCHED communication status."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_subscription_matched_status`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.2.5.3.27 get_topicdescription

**Spec:** §2.2.2.5.3.27, p. 96 — "This operation returns the TopicDescription
associated with the DataReader. This is the same TopicDescription that was used
to create the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::topic`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`,
`crates/dcps/tests/read_api_spec.rs::get_topicdescription_is_the_description_the_reader_was_created_with`

**Status:** done — for a reader on a Topic the Topic, for a reader from `create_datareader_with_cft` the ContentFilteredTopic.

### §2.2.2.5.3.27 get_topicdescription — ContentFilteredTopic

**Spec:** §2.2.2.5.3.27, p. 96 — "This is the same TopicDescription that was
used to create the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_topicdescription`)

**Tests:** `crates/dcps/tests/read_api_spec.rs::get_topicdescription_is_the_description_the_reader_was_created_with`

**Status:** done — `DataReader::get_topicdescription` returns the ContentFilteredTopic or the MultiTopic of the reader; `DataReader::topic` stays the Topic the reader reads from (for a ContentFilteredTopic its related Topic).

### §2.2.2.5.3.28 get_subscriber

**Spec:** §2.2.2.5.3.28, p. 96 — "This operation returns the Subscriber to which
the DataReader belongs."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_subscriber`)

**Tests:** `crates/dcps/tests/status_reads.rs::an_endpoint_names_the_entity_that_created_it`

**Status:** done

### §2.2.2.5.3.29 get_key_value (DataReader)

**Spec:** §2.2.2.5.3.29, p. 96 — "This operation can be used to retrieve the
instance key that corresponds to an instance_handle. The operation will only
fill the fields that form the key inside the key_holder instance. This operation
may return BAD_PARAMETER if the InstanceHandle_t a_handle does not correspond to
an existing data-object known to the DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::get_key_value`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_get_key_value_returns_keyonly_sample`,
`crates/dcps/src/subscriber.rs::tests::get_key_value_uses_holder_decoder_for_writer_and_reader`

**Status:** done — an unknown handle returns `BadParameter`.

### §2.2.2.5.3.29 get_key_value (DataReader) — UNSUPPORTED

**Spec:** §2.2.2.1.1, p. 17 — UNSUPPORTED "Can only be returned by operations
that are optional."; §2.2.2.5.3.29, p. 96 names only BAD_PARAMETER.

**Repo:** —

**Tests:** `crates/dcps/tests/keyless_instance.rs::get_key_value_answers_for_the_one_instance_of_a_keyless_topic`

**Status:** `done` — as for the DataWriter, also in the C API; the reader of a topic without a key answers with the last sample received.

### §2.2.2.5.3.30 lookup_instance (DataReader)

**Spec:** §2.2.2.5.3.30, pp. 96–97 — "This operation takes as a parameter an
instance and returns a handle that can be used in subsequent operations … If the
instance has not been previously registered, or if for any other reason the
Service is unable to provide an instance handle, the Service will return the
special value HANDLE_NIL."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::lookup_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lookup_instance_after_take_with_info`

**Status:** done

### §2.2.2.5.3.31 delete_contained_entities (DataReader)

**Spec:** §2.2.2.5.3.31, p. 97 — "it deletes all contained ReadCondition and
QueryCondition objects. The operation will return PRECONDITION_NOT_MET if the
any of the contained entities is in a state where it cannot be deleted."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::delete_contained_entities`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_contained_entities_deletes_every_condition_the_reader_made`

**Status:** done — a condition has no state that prevents its deletion.

### §2.2.2.5.3.32 wait_for_historical_data

**Spec:** §2.2.2.5.3.32, p. 97 — "This operation is intended only for DataReader
entities that have a non-VOLATILE PERSISTENCE QoS kind … The operation
wait_for_historical_data blocks the calling thread until either all
“historical” data is received, or else the duration specified by the max_wait
parameter elapses … TIMEOUT indicates that max_wait elapsed before all the data
was received."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::wait_for_historical_data`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_reader_historical_data_received`)

**Tests:** `crates/dcps/tests/historical_data.rs::a_late_transient_local_reader_waits_for_the_history_and_then_holds_it`,
`crates/dcps/tests/historical_data.rs::a_volatile_reader_expects_no_history`,
`crates/dcps/src/runtime.rs::tests::wait_for_historical_data_times_out_for_a_writer_without_a_heartbeat`

**Status:** done — waiting for the history, the immediate return of a VOLATILE reader and of one without a matched writer, and the TIMEOUT (next item) are tested.

### §2.2.2.5.3.32 wait_for_historical_data — TIMEOUT

**Spec:** §2.2.2.5.3.32, p. 97 — "a return value of TIMEOUT indicates that
max_wait elapsed before all the data was received."

**Repo:** —

**Tests:** `crates/dcps/src/runtime.rs::tests::wait_for_historical_data_times_out_for_a_writer_without_a_heartbeat`

**Status:** done — a remote TRANSIENT_LOCAL, RELIABLE writer announced by SPDP and SEDP (datagrams of a hand-built SEDP stack) that sends no HEARTBEAT keeps the wait open; `Err(Timeout)` after `max_wait`.

### §2.2.2.5.3.33 get_matched_publication_data

**Spec:** §2.2.2.5.3.33, p. 97 — "This operation retrieves information on a
publication that is currently “associated” with the DataReader … The
publication_handle must correspond to a publication currently associated with
the DataReader otherwise the operation will fail and return BAD_PARAMETER."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::matched_publication_data`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::matched_data_of_a_same_participant_pair_names_the_other_endpoint`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`,
`crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — the sub-items below it are done; see their tests.

### §2.2.2.5.3.33 get_matched_publication_data — BAD_PARAMETER

**Spec:** §2.2.2.5.3.33, p. 97 — "otherwise the operation will fail and return
BAD_PARAMETER."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::a_handle_that_matches_nothing_is_bad_parameter`

**Status:** done — the test covers it.

### §2.2.2.5.3.34 get_matched_publications

**Spec:** §2.2.2.5.3.34, p. 97 — "This operation retrieves the list of
publications currently “associated” with the DataReader … These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSPublications” builtin topic".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::matched_publications`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::user_reader_matched_publication_handles`)

**Tests:** `crates/dcps/tests/publication_handle_live.rs::publication_handle_matches_get_matched_publications`,
`crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`,
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`, `crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches_over_the_wire`

**Status:** done — the handles of the list match `SampleInfo::publication_handle` of the data (test) and the `instance_handle` in the SampleInfo of the DCPSPublication reader (in one process and over the wire).

### §2.2.2.5.4 DataSample

**Spec:** §2.2.2.5.4, p. 98 — "A DataSample represents an atom of data
information (i.e., one value for one instance) as returned by DataReader’s
read/take operations. It consists of two parts: A SampleInfo and the Data."

**Repo:** `crates/dcps/src/sample.rs` (`Sample`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_take_with_info_first_sample_is_new_and_not_read`

**Status:** done

### §2.2.2.5.5 SampleInfo.sample_state

**Spec:** §2.2.2.5.5, p. 98 — "The sample_state (READ or NOT_READ) - indicates
whether or not the corresponding data sample has already been read."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::sample_state`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_read_marks_sample_state_read`

**Status:** done

### §2.2.2.5.5 SampleInfo.view_state

**Spec:** §2.2.2.5.5, p. 98 — "The view_state, (NEW, or NOT_NEW) - indicates
whether the DataReader has already seen samples for the most-current generation
of the related instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::view_state`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`

**Status:** done — time of the snapshot see §2.2.2.5.1.8.

### §2.2.2.5.5 SampleInfo.instance_state

**Spec:** §2.2.2.5.5, p. 98 — "The instance_state (ALIVE, NOT_ALIVE_DISPOSED, or
NOT_ALIVE_NO_WRITERS) - indicates whether the instance is currently in
existence or, if it has been disposed, the reason why it was disposed."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::instance_state`)

**Tests:** `crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`

**Status:** done — time of the snapshot see §2.2.2.5.1.3.

### §2.2.2.5.5 SampleInfo.disposed_generation_count

**Spec:** §2.2.2.5.5, p. 98 — "The disposed_generation_count that indicates the
number of times the instance had become alive after it was disposed explicitly
by a DataWriter, at the time the sample was received."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::disposed_generation_count`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_register_then_unregister_then_re_register_bumps_generation`

**Status:** done

### §2.2.2.5.5 SampleInfo.no_writers_generation_count

**Spec:** §2.2.2.5.5, p. 98 — "The no_writers_generation_count that indicates
the number of times the instance had become alive after it was disposed because
there were no writers, at the time the sample was received."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::no_writers_generation_count`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — the field is present; that the counter rises is checked by
no test (§2.2.2.5.1.5).

### §2.2.2.5.5 SampleInfo.sample_rank, generation_rank, absolute_generation_rank

**Spec:** §2.2.2.5.5, p. 99 — "The sample_rank that indicates the number of
samples related to the same instance that follow in the collection returned by
read or take. The generation_rank … The absolute_generation_rank …".

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::sample_rank`,
`SampleInfo::generation_rank`, `SampleInfo::absolute_generation_rank`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_generation_rank_starts_zero`,
`crates/dcps/src/sample_info.rs::tests::sample_info_all_spec_fields_accessible`

**Status:** done — the ranks are filled in the Rust API (§2.2.2.5.1.6).

### §2.2.2.5.5 SampleInfo.source_timestamp

**Spec:** §2.2.2.5.5, p. 99 — "the source_timestamp that indicates the time
provided by the DataWriter when the sample was written."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::source_timestamp`);
`crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/intra_runtime_timestamp_pairing.rs::write_w_timestamp_stamps_the_sample_with_the_given_timestamp_not_the_clock`,
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`

**Status:** `done` — the writer's time, for data samples and lifecycle markers.

### §2.2.2.5.5 SampleInfo.source_timestamp — dispose and unregister

**Spec:** §2.2.2.4.2.13–§2.2.2.4.2.14, p. 66 — dispose and dispose_w_timestamp
provide the source_timestamp "that is made available to DataReader objects by
means of the source_timestamp attribute inside the SampleInfo".

**Repo:** `crates/dcps/src/subscriber.rs` (`StagedIngest::Lifecycle`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_marker_names_its_writer_and_its_source_time`

**Status:** `done` — the marker's source time from `UserSample::Lifecycle`
reaches the SampleInfo.

### §2.2.2.5.5 SampleInfo.instance_handle

**Spec:** §2.2.2.5.5, p. 99 — "the instance_handle that identifies locally the
corresponding instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::instance_handle`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lookup_instance_after_take_with_info`,
`crates/dcps/tests/sample_info_lifecycle.rs::reader_read_instance_filters_to_one_handle`

**Status:** done

### §2.2.2.5.5 SampleInfo.publication_handle

**Spec:** §2.2.2.5.5, p. 99 — "the publication_handle that identifies locally
the DataWriter that modified the instance. The publication_handle is the same
InstanceHandle_t that is returned by the operation get_matched_publications on
the DataReader and can also be used as a parameter to the DataReader operation
get_matched_publication_data."

**Repo:** `crates/dcps/src/subscriber.rs` (`writer_publication_handle`,
`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/publication_handle_live.rs::publication_handle_matches_get_matched_publications`,
`crates/dcps/tests/publication_handle_live.rs::publication_handle_distinguishes_two_writers`,
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`

**Status:** `done` — for data samples and lifecycle markers the handle of the
writer.

### §2.2.2.5.5 SampleInfo.publication_handle — dispose and unregister

**Spec:** §2.2.2.5.5, p. 99 — "the DataWriter that modified the instance".

**Repo:** `crates/dcps/src/subscriber.rs` (`push_lifecycle_from`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::a_dispose_marker_names_its_writer_and_its_source_time`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_unregister_marker_names_its_writer_and_its_source_time`

**Status:** `done` — the marker names the writer that sent it.

### §2.2.2.5.5 SampleInfo.valid_data

**Spec:** §2.2.2.5.5, p. 99 — "the valid_data flag that indicates whether the
DataSample contains data or else it is only used to communicate of a change in
the instance_state of the instance."

**Repo:** `crates/dcps/src/sample_info.rs` (`SampleInfo::valid_data`)

**Tests:** `crates/dcps/src/sample_info.rs::tests::sample_info_dispose_marker_has_invalid_data`

**Status:** done

### §2.2.2.5.6 SubscriberListener — on_data_on_readers

**Spec:** §2.2.2.5.6, p. 99 — operation `on_data_on_readers(the_subscriber:
Subscriber)`; "the associated listener should be of concrete type
SubscriberListener."

**Repo:** `crates/dcps/src/listener.rs` (`SubscriberListener::on_data_on_readers`)

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`,
`crates/dcps/tests/data_available_live.rs::a_subscriber_listener_hears_data_on_its_readers`

**Status:** done — the callback receives the handle of the subscriber
(§2.2.4.3).

### §2.2.2.5.7 DataReaderListener

**Spec:** §2.2.2.5.7, p. 100 — operations on_data_available,
on_sample_rejected, on_liveliness_changed, on_requested_deadline_missed,
on_requested_incompatible_qos, on_subscription_matched, on_sample_lost; "Some
implementations of the service may not propagate this information. In that case
the DDS specification does not require this listener operation
[on_subscription_matched] to be called."

**Repo:** `crates/dcps/src/listener.rs` (`DataReaderListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datareader_listener_is_object_safe`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_available_at_reader`,
`crates/dcps/tests/listener_trigger_c22c.rs::sample_rejected_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::sample_lost_dispatcher_fires_listener`,
`crates/dcps/tests/listeners_fire_without_polling.rs::matched_listeners_fire_on_match_and_unmatch_without_a_poll`

**Status:** done — all seven, `on_subscription_matched` included; the callbacks
receive the handle of the reader (§2.2.4.3).

### §2.2.2.5.8 ReadCondition — bound to a DataReader, masks

**Spec:** §2.2.2.5.8, pp. 100–101 — "ReadCondition objects are conditions
specifically dedicated to read operations and attached to one DataReader …
ReadCondition objects allow an application to specify the data samples it is
interested in (by specifying the desired sample-states, view-states, and
instance-states) … More than one ReadCondition may be attached to the same
DataReader."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition`);
`crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`

**Status:** done

### §2.2.2.5.8.1 get_datareader

**Spec:** §2.2.2.5.8.1, p. 101 — "This operation returns the DataReader
associated with the ReadCondition. Note that there is exactly one DataReader
associated with each ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_datareader`)

**Tests:** `crates/dcps/tests/status_condition.rs::a_read_condition_names_its_reader`

**Status:** done — `get_datareader` returns the reader of the condition.

### §2.2.2.5.8.2 get_sample_state_mask

**Spec:** §2.2.2.5.8.2, p. 101 — "This operation returns the set of
sample-states that are taken into account to determine the trigger_value of the
ReadCondition. These are the sample-states specified when the ReadCondition was
created."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_sample_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done — the bits follow `sample_info::sample_state_mask` (see
§2.2.2.5.1.9).

### §2.2.2.5.8.3 get_view_state_mask

**Spec:** §2.2.2.5.8.3, p. 101 — "This operation returns the set of view-states
that are taken into account to determine the trigger_value of the
ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_view_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.5.8.4 get_instance_state_mask

**Spec:** §2.2.2.5.8.4, p. 101 — "This operation returns the set of
instance-states that are taken into account to determine the trigger_value of
the ReadCondition."

**Repo:** `crates/dcps/src/condition.rs` (`ReadCondition::get_instance_state_mask`)

**Tests:** `crates/dcps/src/condition.rs::tests::read_condition_returns_trigger_from_closure`

**Status:** done

### §2.2.2.5.9 QueryCondition — filter on local data

**Spec:** §2.2.2.5.9, pp. 101–102 — "QueryCondition objects are specialized
ReadCondition objects that allow the application to also specify a filter on the
locally available data. The query (query_expression) is similar to an SQL WHERE
clause and can be parameterized by arguments that are dynamically changeable by
the set_query_parameters operation … This feature is optional."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition`,
`QueryCondition::evaluate`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_evaluate_filters_per_sample`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`

**Status:** done

### §2.2.2.5.9.1 get_query_expression

**Spec:** §2.2.2.5.9.1, p. 102 — "This operation returns the query_expression
associated with the QueryCondition. That is, the expression specified when the
QueryCondition was created."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::get_query_expression`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_inherits_trigger_from_base`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`

**Status:** done

### §2.2.2.5.9.2 get_query_parameters

**Spec:** §2.2.2.5.9.2, p. 102 — "the parameters specified on the last
successful call to set_query_parameters, or if set_query_parameters was never
called, the arguments specified when the QueryCondition was created."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::get_query_parameters`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_set_query_parameters_roundtrip`

**Status:** done

### §2.2.2.5.9.3 set_query_parameters

**Spec:** §2.2.2.5.9.3, p. 102 — "This operation changes the query_parameters
associated with the QueryCondition."

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition::set_query_parameters`)

**Tests:** `crates/dcps/src/condition.rs::tests::query_condition_set_query_parameters_roundtrip`

**Status:** done

---

## §2.2.3 Supported QoS

### §2.2.3 QoS as a list of QosPolicy objects

**Spec:** §2.2.3, pp. 102–103 — "QoS is comprised of individual QoS policies
(objects of type deriving from QosPolicy) … QoS (i.e., a list of QosPolicy
objects) may be associated with all Entity objects in the system such as Topic,
DataWriter, DataReader, Publisher, Subscriber, and DomainParticipant." Figure
2.12 shows the 22 policies with their fields.

**Repo:** `crates/qos/src/policies/` (one module per policy);
`crates/dcps/src/qos.rs` (QoS sets per entity)

**Tests:** `crates/qos/src/pid.rs::tests::every_qos_policy_has_a_distinct_identifier`,
`crates/qos/src/policies/qos_set.rs::tests::defaults_are_compatible`

**Status:** done — the policies and their fields are listed per policy below.

### §2.2.3 Consistency on set_qos

**Spec:** §2.2.3, p. 104 — "Some QosPolicy values may not be consistent with
other ones … When a set of QosPolicy is passed (set_qos operations), the set
resulting from adding the new policies on top of the previous is checked for
consistency. If the resulting QoS is inconsistent, the change of QoS operation
fails and the previous values are retained."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — `set_qos` and `create_*` check consistency; a refused
`set_qos` keeps the previous values.

### §2.2.3 Requested/offered — incompatibility prevents communication

**Spec:** §2.2.3, p. 104 — "If the two policies are not compatible, the Service
will not establish communications between the two Entity objects and will record
this fact by means of the OFFERED_INCOMPATIBLE_QOS on the publisher end and
REQUESTED_INCOMPATIBLE_QOS on the subscriber end".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`RxoView`, matching points with `record_match_refusal`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_same_runtime_pair_is_refused_for_an_rxo_policy_and_counted_once`,
`crates/dcps/src/runtime.rs::tests::an_incompatible_remote_endpoint_is_counted_once`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.3 RxO property Yes / No / N/A

**Spec:** §2.2.3, p. 104 — "An ‘RxO’ setting of “Yes” indicates that the policy
can be set both at the publishing and subscribing ends and the values must be
set in a compatible manner … “No” … all combinations of values are compatible …
“N/A” indicates that the policy can only be specified at either the publishing
or the subscribing end".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView`: DURABILITY, RELIABILITY,
DEADLINE, LIVELINESS, LATENCY_BUDGET, DESTINATION_ORDER, OWNERSHIP,
PRESENTATION)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_requested_both_compatible_matches`,
`crates/qos/src/compatibility.rs::tests::compute_compatibility_reports_multiple_mismatches`

**Status:** done — the assignment per policy is listed below.

### §2.2.3 Changeable property

**Spec:** §2.2.3, p. 104 — "a policy with ‘changeable’ setting of ‘NO” is
considered “immutable” and can only be specified either at Entity creation time
or else prior to calling the enable operation on the Entity."

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`,
`crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`

**Status:** done — the assignment per policy is listed below.

### §2.2.3.1 USER_DATA — field value

**Spec:** §2.2.3, p. 104 — USER_DATA: "A sequence of octets: “value”" for
DomainParticipant, DataReader, DataWriter; "User data not known by the
middleware, but distributed by means of built-in topics".

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy::value`);
`crates/dcps/src/qos.rs` (`DomainParticipantQos::user_data`,
`DataWriterQos::user_data`, `DataReaderQos::user_data`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::user_data_roundtrip`,
`crates/dcps/tests/participant_config_user_data.rs::an_explicit_config_keeps_the_user_data_of_the_qos`,
`crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done — a value above 64 KiB (`MAX_OPAQUE_LEN`) is refused when
read; the specification sets no limit.

### §2.2.3.1 USER_DATA — default

**Spec:** §2.2.3, p. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::user_data_default_is_empty`

**Status:** done

### §2.2.3.1 USER_DATA — RxO No

**Spec:** §2.2.3, p. 104 — USER_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy` does not
compare USER_DATA)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.1 USER_DATA — Changeable Yes

**Spec:** §2.2.3, p. 104 — USER_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_qos`:
`update_participant_user_data`); `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs` (`runtime_qos_update`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`,
`crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.1 USER_DATA — purpose

**Spec:** §2.2.3.1, p. 113 — "The purpose of this QoS is to allow the
application to attach additional information to the created Entity objects such
that when a remote application discovers their existence it can access that
information".

**Repo:** `crates/dcps/src/builtin_topics.rs` (`user_data` in
`ParticipantBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done

### §2.2.3.2 TOPIC_DATA — field value

**Spec:** §2.2.3, p. 104 — TOPIC_DATA: "A sequence of octets: “value”" for
Topic.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy`);
`crates/dcps/src/qos.rs` (`TopicQos::topic_data`; additionally
`DataWriterQos::topic_data` and `DataReaderQos::topic_data` as an override per
endpoint)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`,
`crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done — DataWriter and DataReader carry the field additionally, the
specification assigns it only to the Topic.

### §2.2.3.2 TOPIC_DATA — default

**Spec:** §2.2.3, p. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::topic_data_default_is_empty`

**Status:** done

### §2.2.3.2 TOPIC_DATA — RxO No

**Spec:** §2.2.3, p. 104 — TOPIC_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.2 TOPIC_DATA — Changeable Yes

**Spec:** §2.2.3, p. 104 — TOPIC_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/topic.rs` (`Topic::set_qos`:
`update_user_topic_data`, re-announcement)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_topic_data_change_reaches_the_writers_announcement`

**Status:** done

### §2.2.3.2 TOPIC_DATA — purpose

**Spec:** §2.2.3.2, p. 114 — "The purpose of this QoS is to allow the
application to attach additional information to the created Topic such that when
a remote application discovers their existence it can examine the information".

**Repo:** `crates/dcps/src/builtin_topics.rs` (`topic_data` in
`TopicBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done

### §2.2.3.3 GROUP_DATA — field value

**Spec:** §2.2.3, p. 104 — GROUP_DATA: "A sequence of octets: “value”" for
Publisher, Subscriber.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy`);
`crates/dcps/src/qos.rs` (`PublisherQos::group_data`, `SubscriberQos::group_data`;
additionally per endpoint, `effective_group_data`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::group_data_large_payload`,
`crates/dcps/src/qos.rs::group_policy_tests::empty_endpoint_group_data_inherits_the_group_octets`

**Status:** done

### §2.2.3.3 GROUP_DATA — default

**Spec:** §2.2.3, p. 104 — "The default value is an empty (zero-sized)
sequence."

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy::default`)

**Tests:** `crates/qos/src/defaults.rs::tests::group_data_default_is_empty`

**Status:** done

### §2.2.3.3 GROUP_DATA — RxO No

**Spec:** §2.2.3, p. 104 — GROUP_DATA: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.3 GROUP_DATA — Changeable Yes

**Spec:** §2.2.3, p. 104 — GROUP_DATA: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`:
`update_user_group_qos`), `crates/dcps/src/subscriber.rs`
(`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`

**Status:** done — the test changes PARTITION; GROUP_DATA takes the same path
(`update_user_group_qos`), there is no check of its own of the new octets.

### §2.2.3.3 GROUP_DATA — purpose

**Spec:** §2.2.3.3, p. 114 — "The value of the GROUP_DATA is available to the
application on the DataReader and DataWriter entities and is propagated by
means of the built-in topics."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`group_data` in
`PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`)

**Tests:** `crates/dcps/src/runtime.rs::tests::group_and_topic_values_reach_a_writer_that_sets_none_of_its_own`

**Status:** done

### §2.2.3.4 DURABILITY — field kind

**Spec:** §2.2.3, p. 105 — DURABILITY: "A “kind”: VOLATILE, TRANSIENT_LOCAL,
TRANSIENT, or PERSISTENT" for Topic, DataReader, DataWriter.

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityKind`,
`DurabilityQosPolicy`); `crates/dcps/src/qos.rs`

**Tests:** `crates/qos/src/policies/durability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/durability.rs::tests::try_from_u32_is_strict`

**Status:** done

### §2.2.3.4 DURABILITY — default VOLATILE

**Spec:** §2.2.3, p. 105 — VOLATILE "This is the default kind."

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityKind::default`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::default_is_volatile`,
`crates/qos/src/defaults.rs::tests::durability_default_volatile`

**Status:** done

### §2.2.3.4 DURABILITY — RxO: offered kind >= requested kind

**Spec:** §2.2.3.4, p. 114 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … VOLATILE < TRANSIENT_LOCAL < TRANSIENT <
PERSISTENT."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`);
`crates/qos/src/policies/durability.rs` (`DurabilityQosPolicy::is_compatible_with`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::compatibility_offered_ge_requested`,
`crates/qos/src/compatibility.rs::tests::durability_offered_ge_requested`,
`crates/dcps/tests/transient_local_durability.rs::linux::transient_local_reader_rejects_volatile_writer`

**Status:** done

### §2.2.3.4 DURABILITY — Changeable No

**Spec:** §2.2.3, p. 105 — DURABILITY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs` (`Topic::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_immutable_durability_post_enable`

**Status:** done

### §2.2.3.4 DURABILITY — VOLATILE

**Spec:** §2.2.3, p. 105 — "The Service does not need to keep any samples of
data-instances on behalf of any DataReader that is not known by the DataWriter
at the time the instance is written."

**Repo:** `crates/rtps/src/reader_proxy.rs` (`ReaderProxy::skip_samples_up_to`);
`crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::volatile_writer_does_not_deliver_history_to_late_joiner`,
`crates/dcps/src/runtime.rs::tests::qr_transient_local_late_join_replay_vs_volatile`

**Status:** done

### §2.2.3.4 DURABILITY — TRANSIENT_LOCAL

**Spec:** §2.2.3, p. 105 — "The Service will attempt to keep some samples so
that they can be delivered to any potential late-joining DataReader … For
TRANSIENT_LOCAL, the service is only required to keep the data in the memory of
the DataWriter that wrote the data".

**Repo:** `crates/dcps/src/runtime.rs` (the writer’s history for late readers,
`enforce_retained_limits`)

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::transient_local_writer_delivers_history_to_late_joiner`,
`crates/dcps/tests/transient_local_history_depth.rs::keep_last_one_replays_only_the_newest_sample`,
`crates/dcps/src/runtime.rs::tests::transient_local_reader_acknacked_on_metatraffic_socket_gets_history_and_live_samples`

**Status:** done

### §2.2.3.4 DURABILITY — TRANSIENT [optional]

**Spec:** §2.2.3, p. 105 — "For TRANSIENT, the service is only required to keep
the data in memory and not in permanent storage; but the data is not tied to the
lifecycle of the DataWriter and will, in general, survive it. Support for
TRANSIENT kind is optional."

**Repo:** `crates/dcps/src/durability_service.rs` (`InMemoryDurabilityBackend`);
`crates/dcps/src/publisher.rs` (`DataWriter::build_durability_backend`)

**Tests:** `crates/dcps/tests/durability_service_qos.rs::transient_writer_has_in_memory_backend`,
`crates/dcps/tests/durability_service_qos.rs::transient_backend_outlives_writer_cache_keep_last_eviction`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_store.rs::the_store_outlives_the_writer_that_filled_it`, `crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`

**Status:** done — the store survives the eviction from the cache of the writer and the deletion of the writer, and a reader that comes after the last application DataWriter is gone gets the samples (the fictitious DataWriter, see the next item). A stand-alone service is `crates/durability-service/`.

### §2.2.3.4 DURABILITY — TRANSIENT outlives the DataWriter

**Spec:** §2.2.3, p. 105 — "the data is not tied to the lifecycle of the
DataWriter and will, in general, survive it."

**Repo:** `crates/dcps/src/participant.rs` (`ParticipantInner::durability_store`);
`crates/dcps/src/publisher.rs` (`Publisher::durability_store_for`)

**Tests:** `crates/dcps/tests/durability_store.rs::the_store_outlives_the_writer_that_filled_it`

**Status:** `done` — the store belongs to the participant's topic; a writer created later finds what the deleted one wrote and continues the store's sequence numbers (`next_sequence`, above the highest on disk for a PERSISTENT store). A reader created only after all writers are deleted gets nothing while no writer serves the store again: the fictitious entities (next item) are missing for that.

### §2.2.3.4 DURABILITY — PERSISTENT [optional]

**Spec:** §2.2.3, p. 105 — "[optional] Data is kept on permanent storage, so
that they can outlive a system session."

**Repo:** `crates/dcps/src/durability_service.rs` (`OnDiskDurabilityBackend`)

**Tests:** `crates/dcps/src/durability_service.rs::tests::on_disk_persistent_survives_backend_drop`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::persistent_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::a_persistent_writer_without_a_store_is_refused`

**Status:** done

### §2.2.3.4 DURABILITY — control of the data for late readers

**Spec:** §2.2.3.4, p. 114 — "This QoS policy controls whether the Service will
actually make data available to late-joining readers … the Service may choose
to maintain some data for its own purposes (e.g., flow control) and yet not make
it available to late-joining readers if the DURABILITY QoS policy is set to
VOLATILE."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/transient_local_durability.rs::linux::a_transient_local_writer_serves_a_volatile_late_joiner_nothing`,
`crates/dcps/tests/transient_local_durability.rs::linux::volatile_writer_does_not_deliver_history_to_late_joiner`

**Status:** done

### §2.2.3.4 DURABILITY — fictitious durability DataReader and DataWriter per topic

**Spec:** §2.2.3.4, p. 114 — "the service behaves “as if” for each Topic that has
TRANSIENT or PERSISTENT DURABILITY kind there was a corresponding “built-in”
DataReader and DataWriter … For each Topic, the built-in fictitious
“persistence service” DataReader and DataWriter has its QoS configured from the
Topic QoS of the corresponding Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`retain_user_writer_as_service`, `release_service_writer`); `crates/dcps/src/publisher.rs` (`WriterInner::delete`)

**Tests:** `crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`, `crates/dcps/tests/durability_service_entities.rs::the_service_writer_ends_with_the_content_of_the_store`,
`crates/dcps/tests/durability_store.rs::the_store_follows_the_topics_durability_service`, `crates/dcps/tests/durability_store.rs::a_default_topic_leaves_the_setting_to_the_writer`, `crates/dcps/tests/durability_store.rs::a_writer_that_differs_from_the_topic_is_overridden`, `crates/durability-service/tests/keyed_topic_end_to_end.rs`, `crates/durability-service/tests/writer_dies.rs::late_joiner_gets_history_after_writer_dies`, `crates/durability-service/tests/restart.rs`, `crates/durability-service/tests/cross_vendor_ingest.rs`, `crates/durability-service/tests/discovered_durability_service_qos.rs`

**Status:** done — the fictitious entities are realised in two places that together cover the clause. In the participant: the store of a topic, the fictitious DataWriter (the endpoint of the last application DataWriter, kept while the store holds samples) and the checks against the Topic QoS. The store follows the Topic's DURABILITY_SERVICE (§2.2.3.5: it configures the fictitious entities, which are per topic); a Topic that leaves it at the default leaves it to the first writer that creates the store; a writer that differs from a non-default Topic is overridden and `writer.durability_service.overridden_by_topic` is recorded. The samples of remote writers are not taken in by the participant; that is the domain-wide service `crates/durability-service/` (ADR 0009): it ingests with a RELIABLE, TRANSIENT_LOCAL, KEEP_ALL reader every sample of every discovered writer, local, remote or of another vendor, stores it (sqlite, files or memory), and replays it with a TRANSIENT_LOCAL writer, also after the original writer is gone and after its own restart. It takes the retention contract from the DURABILITY_SERVICE the writer announces.

### §2.2.3.4 DURABILITY — incompatibility with the fictitious entities

**Spec:** §2.2.3.4, pp. 114–115 — "a DataWriter that has an incompatible QoS with
respect to what the Topic specified will not send its data to the
transient/persistent service … Incompatibilities between local DataReader/
DataWriter entities and the corresponding fictitious “built-in
transient/persistent entities” cause the REQUESTED_INCOMPATIBLE_QOS/
OFFERED_INCOMPATIBLE_QOS status to change".

**Repo:** `crates/dcps/src/runtime.rs` (`check_writer_against_fictitious_reader`, `check_reader_against_fictitious_writer`, `RxoView::of_topic`)

**Tests:** `crates/dcps/tests/durability_service_entities.rs::an_endpoint_incompatible_with_the_topic_changes_its_status`, `crates/dcps/tests/durability_service_entities.rs::an_incompatible_writer_does_not_feed_the_store`

**Status:** done — `check_writer_against_fictitious_reader` and `check_reader_against_fictitious_writer` compare an endpoint with the Topic QoS of a TRANSIENT or PERSISTENT topic at its enable: an incompatible writer counts in OFFERED_INCOMPATIBLE_QOS and sends nothing to the store, an incompatible reader counts in REQUESTED_INCOMPATIBLE_QOS. A VOLATILE writer on such a topic is incompatible (DURABILITY), as the clause reads. The comparison is at enable; a later change of a mutable Topic policy is not compared again.

### §2.2.3.4 DURABILITY — service_cleanup_delay: when the service removes an instance

**Spec:** §2.2.3.4, p. 115 — "Information on a data-instances is maintained
until the following conditions are met: 1. the instance has been explicitly
disposed … 2. … there are no more “live” DataWriter entities writing the
instance … 3. a time interval longer that service_cleanup_delay has elapsed".

**Repo:** `crates/dcps/src/durability_service.rs` (`LiveStoreWriters`); `crates/dcps/src/publisher.rs` (`WriterInner::leave_store`, `WriterStatus::leave_store_on_lost_liveliness`)

**Tests:** `crates/dcps/tests/durability_store.rs::a_disposed_unregistered_instance_stays_for_the_service_cleanup_delay`,
`crates/dcps/tests/durability_store.rs::an_instance_that_is_not_disposed_stays`,
`crates/dcps/src/durability_service.rs::tests::replay_applies_the_service_cleanup_delay`,
`crates/dcps/tests/durability_store.rs::the_cleanup_delay_waits_for_the_last_writer_of_the_instance`, `crates/dcps/tests/durability_store.rs::a_deleted_writer_is_no_live_writer`, `crates/dcps/tests/durability_service_entities.rs::a_writer_that_lost_its_liveliness_is_no_live_writer`

**Status:** done — the three conditions: the instance is disposed, no live writer of the participant writes it (`LiveStoreWriters`: a writer that unregisters, is deleted, or loses its liveliness stops being one; a later write makes it one again), and `service_cleanup_delay` has run out. Writers of other participants are not seen by the store.

### §2.2.3.5 DURABILITY_SERVICE — fields

**Spec:** §2.2.3, pp. 105–106 — DURABILITY_SERVICE for Topic, DataWriter:
`service_cleanup_delay` (duration), `history_kind`, `history_depth`,
`max_samples`, `max_instances`, `max_samples_per_instance`.

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy`); `crates/dcps/src/qos.rs`
(`TopicQos::durability_service`, `DataWriterQos::durability_service`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::roundtrip`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — defaults

**Spec:** §2.2.3, p. 106 — "service_cleanup_delay … By default, zero …
history_kind=KEEP_LAST history_depth=1 … max_samples, max_instances,
max_samples_per_instance … By default they are all LENGTH_UNLIMITED."

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy::default`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::default_matches_spec`,
`crates/qos/src/defaults.rs::tests::durability_service_defaults_match_spec`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — RxO No

**Spec:** §2.2.3, p. 105 — DURABILITY_SERVICE: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` without DURABILITY_SERVICE)

**Tests:** `crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`

**Status:** done

### §2.2.3.5 DURABILITY_SERVICE — Changeable No

**Spec:** §2.2.3, p. 105 — DURABILITY_SERVICE: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.5 DURABILITY_SERVICE — HISTORY and RESOURCE_LIMITS of the service

**Spec:** §2.2.3.5, p. 115 — "This policy is used to configure the HISTORY QoS
and the RESOURCE_LIMITS QoS used by the fictitious DataReader and DataWriter used
by the “persistence service.”"

**Repo:** `crates/dcps/src/durability_service.rs` (`InMemoryDurabilityBackend`,
`OnDiskDurabilityBackend`: depth and limits from DURABILITY_SERVICE)

**Tests:** `crates/dcps/src/durability_service.rs::tests::in_memory_keeplast_caps_history_at_depth`,
`crates/dcps/src/durability_service.rs::tests::in_memory_max_instances_returns_oor`,
`crates/dcps/src/durability_service.rs::tests::on_disk_max_samples_per_instance_returns_oor`

**Status:** done — applied to the writer’s store (see §2.2.3.4).

### §2.2.3.6 PRESENTATION — field access_scope

**Spec:** §2.2.3, p. 106 — PRESENTATION for Publisher, Subscriber: "An
“access_scope”: INSTANCE, TOPIC, GROUP" — INSTANCE "Scope spans only a single
instance", TOPIC "Scope spans to all instances within the same DataWriter (or
DataReader)", GROUP "[optional] Scope spans to all instances belonging to
DataWriter (or DataReader) entities within the same Publisher (or
Subscriber)".

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationAccessScope`);
`crates/dcps/src/qos.rs` (`PublisherQos::presentation`,
`SubscriberQos::presentation`; override per endpoint,
`effective_presentation`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/dcps/src/qos.rs::group_policy_tests::endpoint_default_presentation_inherits_the_group_value`

**Status:** done

### §2.2.3.6 PRESENTATION — fields coherent_access and ordered_access

**Spec:** §2.2.3, pp. 106–107 — "coherent_access … the ability to group a set of
changes as a unit on the publishing end such that they are received as a unit at
the subscribing end … ordered_access … the ability of the subscriber to see
changes in the same order as they occurred on the publishing end."

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/dcps/src/runtime.rs::tests::publisher_coherent_and_ordered_flags_reach_the_writer_announcement`

**Status:** done

### §2.2.3.6 PRESENTATION — defaults

**Spec:** §2.2.3, pp. 106–107 — INSTANCE "This is the default access_scope …
The default setting of coherent_access is FALSE … The default setting of
ordered_access is FALSE."

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy::default`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::default_instance_no_flags`,
`crates/qos/src/defaults.rs::tests::presentation_default_instance_no_flags`

**Status:** done

### §2.2.3.6 PRESENTATION — RxO

**Spec:** §2.2.3.6, p. 116 — "1. The inequality “offered access_scope >=
requested access_scope” … INSTANCE < TOPIC < GROUP. 2. Requested
coherent_access is FALSE, or else both offered and requested coherent_access are
TRUE. 3. Requested ordered_access is FALSE, or else both offered and requested
ordered _access are TRUE."

**Repo:** `crates/qos/src/policies/presentation.rs` (`is_compatible_with`);
`crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_coherent_access_unoffered_rejects_on_both_sides`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_ordered_access_unoffered_rejects_on_both_sides`,
`crates/qos/src/compatibility.rs::tests::presentation_scope_and_flags`

**Status:** done

### §2.2.3.6 PRESENTATION — Changeable No

**Spec:** §2.2.3, p. 106 — PRESENTATION: Changeable "No".

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`),
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`

**Status:** done

### §2.2.3.6 PRESENTATION — coherent_access with INSTANCE

**Spec:** §2.2.3.6, p. 115 — "If access_scope is set to INSTANCE, the use of
begin_coherent_change and end_coherent_change has no effect on how the
subscriber can access the data because with the scope limited to each instance,
changes to separate instances are considered independent and thus cannot be
grouped by a coherent change."

**Repo:** `crates/dcps/src/subscriber.rs::DataReader::coherent_access_active`;
`crates/dcps/src/runtime.rs` (`PresentationInstanceCoherence`, `dcps.presentation_instance_coherence`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_coherent_reader_still_waits_for_end_coherent_changes`,
`crates/dcps/tests/coherent_changes_e2e.rs::instance_scope_with_no_coherence_presents_the_samples_as_they_arrive`,
`crates/dcps/tests/runtime_config_layering.rs::the_presentation_instance_coherence_setting_takes_both_values_and_refuses_others`

**Status:** `done` — the specification contradicts itself: the table (p. 106) “order and coherent changes apply to each instance separately” against §2.2.3.6 (p. 115) “has no effect on how the subscriber can access the data”; both say there is no coherence across instances, they differ for a coherent set within one instance. Decision: the default is the table reading (`per_instance`, the set is held until its end), because the OMG dds-rtps suite (`Test_CoherentSets_10`, `_19`) checks exactly that against the other vendors. The reading of the text is the setting `dcps.presentation_instance_coherence: none` (environment `ZERODDS_PRESENTATION_INSTANCE_COHERENCE`): under INSTANCE the set is not held back.

### §2.2.3.6 PRESENTATION — coherent_access with TOPIC

**Spec:** §2.2.3.6, p. 115 — "If access_scope is set to TOPIC, then coherent
changes … will be made available as such to each remote DataReader
independently. That is, changes made to instances within each individual
DataWriter will be available as coherent with respect to other changes to
instances in that same DataWriter".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::coherent_gate`,
`DataReader::release_coherent_set`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::two_coherent_sets_are_released_one_at_a_time`,
`crates/dcps/src/subscriber.rs::tests::every_end_of_coherent_set_form_releases_the_writers_set`,
`crates/dcps/src/subscriber.rs::tests::another_writers_end_leaves_a_set_held`

**Status:** done

### §2.2.3.6 PRESENTATION — coherent_access with GROUP

**Spec:** §2.2.3.6, p. 115 — "If access_scope is set to GROUP, then coherent
changes made to instances through a DataWriter attached to a common Publisher are
made available as a unit to remote subscribers."

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/publisher.rs`
(`Publisher::end_coherent_changes`)

**Tests:** `crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/src/group_order.rs::tests::the_group_coherent_set_of_table_9_23_goes_out_whole_after_its_end`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_in_a_group_coherent_set_waits_for_its_end_over_udp`

**Status:** done — built on `wip/omg-rc7`, state in
`internal/rtps/group-access-followup.md`.

### §2.2.3.6 PRESENTATION — ordered_access with INSTANCE

**Spec:** §2.2.3.6, p. 116 — "If access_scope is set to INSTANCE (the lowest
level), then changes to each instance are considered unordered relative to
changes to any other instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::instance_scope_ordering_active`,
`DataReader::present_by_access_scope`)

**Tests:** `crates/dcps/tests/ordered_access_e2e.rs::ordered_access_under_instance_scope_is_inert`,
`crates/dcps/tests/ordered_access_e2e.rs::instance_scope_ordered_access_groups_the_plain_take_by_instance`

**Status:** done

### §2.2.3.6 PRESENTATION — ordered_access with TOPIC

**Spec:** §2.2.3.6, p. 116 — "If access_scope is set to TOPIC, changes …
made by a single DataWriter are made available to subscribers in the same order
they occur."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::present_by_access_scope`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::topic_scope_ordered_access_keeps_write_order_on_the_plain_take`

**Status:** `done`

### §2.2.3.6 PRESENTATION — ordered_access with GROUP

**Spec:** §2.2.3.6, p. 116 — "if access_scope is set to GROUP, changes made to
instances via DataWriter entities attached to the same Publisher object are made
available to subscribers on the same order they occur."

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/subscriber.rs`
(`Subscriber::get_datareaders`, `DataReader::presentation_limit`)

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`,
`crates/dcps/tests/group_access_e2e.rs::a_dispose_takes_its_place_in_the_group_order_same_runtime`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — hand-over in GSN order per pair of writer and reader, and `get_datareaders` orders by the GSN.

### §2.2.3.7 DEADLINE — field period

**Spec:** §2.2.3, p. 107 — DEADLINE for Topic, DataReader, DataWriter: "A
duration “period”".

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy::period`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::roundtrip`

**Status:** done

### §2.2.3.7 DEADLINE — default infinite

**Spec:** §2.2.3, p. 107 — "The default value of the deadline period is
infinite."

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy::default`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::default_is_infinite`,
`crates/dcps/tests/deadline_qos.rs::deadline_default_is_infinite_so_counter_stays_zero`

**Status:** done

### §2.2.3.7 DEADLINE — RxO: offered period <= requested period

**Spec:** §2.2.3.7, p. 116 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered deadline period <=
requested deadline period” evaluates to ‘TRUE.’"

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`deadline_compat`)

**Tests:** `crates/qos/src/compatibility.rs::tests::deadline_offered_le_requested`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done — `qos_duration_to_nanos` maps INFINITE and the duration 0 to
the same value 0; a requested period of 0 therefore counts as infinite. Read,
not measured; see the findings file.

### §2.2.3.7 DEADLINE — Changeable Yes

**Spec:** §2.2.3, p. 107 — DEADLINE: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`
(`runtime_qos_update`: `deadline`); `crates/dcps/src/topic.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`,
`crates/dcps/tests/set_qos_after_enable.rs::a_reader_deadline_change_makes_the_pair_incompatible`

**Status:** done

### §2.2.3.7 DEADLINE — contract and monitoring

**Spec:** §2.2.3, p. 107; §2.2.3.7, p. 116 — "DataReader expects a new sample
updating the value of each instance at least once every deadline period.
DataWriter indicates that the application commits to write a new value … for
each instance … the fulfillment of this contract is monitored by the Service and
the application is informed of any violations".

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::writes_within_deadline_keep_counter_at_zero`

**Status:** done

### §2.2.3.7 DEADLINE — consistency with TIME_BASED_FILTER

**Spec:** §2.2.3, p. 107; §2.2.3.7, p. 116 — "It is inconsistent for a DataReader
to have a DEADLINE period less than its TIME_BASED_FILTER’s minimum_separation …
the settings must be such that “deadline period>= minimum_separation.”"

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::a_time_based_filter_longer_than_the_deadline_refuses_the_reader`

**Status:** `done` — `create_datareader` and `DataReader::set_qos` refuse a
`minimum_separation` above the DEADLINE period.

### §2.2.3.8 LATENCY_BUDGET — field duration

**Spec:** §2.2.3, p. 107 — LATENCY_BUDGET for Topic, DataReader, DataWriter: "A
duration “duration”" — "Specifies the maximum acceptable delay from the time the
data is written until the data is inserted in the receiver's application-cache
and the receiving application is notified of the fact."

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::roundtrip`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — default 0

**Spec:** §2.2.3, p. 107 — "The default value of the duration is zero
indicating that the delay should be minimized."

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy::default`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::default_is_zero`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — RxO: offered duration <= requested duration

**Spec:** §2.2.3.8, p. 117 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered duration <= requested
duration” evaluates to ‘TRUE.’"

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`is_compatible_with`);
`crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_offered_latency_budget_incompatible_rejects_match`,
`crates/dcps/src/runtime.rs::tests::rxo_requested_latency_budget_incompatible_rejects_match`,
`crates/qos/src/compatibility.rs::tests::latency_budget_offered_le_requested`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — Changeable Yes

**Spec:** §2.2.3, p. 107 — LATENCY_BUDGET: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs`, `crates/dcps/src/subscriber.rs`
(`runtime_qos_update`: `latency_budget`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_writer_qos_change_is_decided_again_against_a_matched_reader`

**Status:** done

### §2.2.3.8 LATENCY_BUDGET — hint

**Spec:** §2.2.3, p. 107; §2.2.3.8, pp. 116–117 — "This policy is a hint to the
Service, not something that must be monitored or enforced. The Service is not
required to track or alert the user of any violation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission; ZeroDDS uses the value only in
the comparison (RxO).

### §2.2.3.9 OWNERSHIP — field kind

**Spec:** §2.2.3, p. 107 — OWNERSHIP for Topic, DataReader, DataWriter: "A
“kind” SHARED EXCLUSIVE" — "[optional] Specifies whether it is allowed for
multiple DataWriters to write the same instance of the data and if so, how these
modifications should be arbitrated".

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipKind`,
`OwnershipQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::roundtrip`,
`crates/qos/src/policies/ownership.rs::tests::try_from_u32_valid_variants`

**Status:** done

### §2.2.3.9 OWNERSHIP — default SHARED

**Spec:** §2.2.3, p. 107 — SHARED "This is the default behavior if the OWNERSHIP
QoS policy is not specified or supported."

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipKind::default`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::default_shared`,
`crates/qos/src/defaults.rs::tests::ownership_default_shared`

**Status:** done

### §2.2.3.9 OWNERSHIP — RxO: the same kind

**Spec:** §2.2.3.9.2, p. 118 — "The value of the OWNERSHIP kind offered must
exactly match the one requested or else they are considered incompatible."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::ownership_must_match_exactly`,
`crates/qos/src/policies/qos_set.rs::tests::ownership_mismatch_reported`

**Status:** done — the tests check `OwnershipQosPolicy::is_compatible_with`;
`first_incompatible_policy` compares the kind itself (`offered.ownership !=
requested.ownership`), a runtime test for it is missing.

### §2.2.3.9 OWNERSHIP — Changeable No

**Spec:** §2.2.3, p. 107 — OWNERSHIP: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.9.1 SHARED

**Spec:** §2.2.3.9.1, p. 117 — "The subscriber to the Topic will be able to
access modifications from all DataWriter objects … In any case there is no
“filtering” of modifications made based on the identity of the DataWriter that
causes the modification."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`:
SHARED lets everything through)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::shared_ownership_passes_all_samples`

**Status:** done

### §2.2.3.9.2 EXCLUSIVE — one owner per instance, highest strength

**Spec:** §2.2.3.9.2, p. 117 — "at any point in time a single DataWriter “owns”
each instance and is the only one whose modifications will be visible to the
DataReader objects. The owner is determined by selecting the DataWriter with the
highest value of the strength that is both “alive” as defined by the LIVELINESS
QoS policy and has not violated its DEADLINE contract with regards to the
data-instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::accept_under_exclusive_ownership`);
`crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`)

**Tests:**
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_filters_weaker_writer`,
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`,
`crates/dcps/src/instance_tracker.rs::tests::exclusive_higher_strength_wins`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — the highest strength among the writers that are alive and
keep their DEADLINE for the instance.

### §2.2.3.9.2 EXCLUSIVE — change on liveliness, strength, DEADLINE

**Spec:** §2.2.3.9.2, p. 117 — "Ownership can therefore change as a result of
(a) a DataWriter in the system with a higher value of the strength that modifies
the instance, (b) a change in the strength value of the DataWriter that owns the
instance, (c) a change in the liveliness of the DataWriter that owns the
instance, and (d) a deadline with regards to the instance that is missed by the
DataWriter that owns the instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`accept_under_exclusive_ownership`, `update_owner_strength`,
`clear_owner_for_writer`); `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`)

**Tests:**
`crates/dcps/tests/ownership_failover.rs::notify_writer_liveliness_lost_clears_owner`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_lowers_its_strength_keeps_the_instance_until_a_stronger_one`,
`crates/dcps/src/instance_tracker.rs::tests::an_announced_strength_holds_the_owned_instances`

**Status:** `done` — (a) a stronger writer takes over; (b) the owner's announced
strength applies from the announcement; (c) loss of liveliness or of the match
releases ownership; (d) an owner that misses its DEADLINE for the instance loses
it to another writer.

### §2.2.3.9.2 EXCLUSIVE — equal strength, the same choice in all readers

**Spec:** §2.2.3.9.2, p. 117 — "If this occurs the Service will pick one of the
DataWriter objects as the “owner.” It is not specified how the owner is
selected. However, it is required that the policy used to select the owner is
such that all DataReader objects will make the same choice".

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::accept_under_exclusive_ownership`: lower GUID);
`crates/qos/src/exclusive_ownership.rs`

**Tests:**
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_tie_break_by_lower_guid`,
`crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_higher_guid_at_tie_rejected`

**Status:** `done` — on equal strength the lower GUID wins, the same rule in the
DataReader and in the resolver of `zerodds-qos`; Cyclone 11 chooses alike
(`inst_accepts_sample_by_writer_guid`).

### §2.2.3.9.2 EXCLUSIVE — per instance

**Spec:** §2.2.3.9.2, p. 118 — "Exclusive ownership is on an instance-by-instance
basis. That is, a subscriber can receive values written by a lower strength
DataWriter as long as they affect instances whose values have not been set by
the higher-strength DataWriter."

**Repo:** `crates/dcps/src/instance_tracker.rs` (owner per `InstanceState`)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::exclusive_first_writer_wins`,
`crates/dcps/tests/ownership_failover.rs::notify_participant_liveliness_lost_clears_all_writers_with_prefix`

**Status:** done

### §2.2.3.9.2 EXCLUSIVE — no feedback to the writer

**Spec:** §2.2.3.9.2, p. 117 — "It is also not a requirement that the DataWriter
objects are aware of whether they own a particular instance. There is no error
or notification given to a DataWriter that modifies an instance it does not
currently own."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an exemption; the DataWriter of ZeroDDS
learns nothing about ownership.

### §2.2.3.10 OWNERSHIP_STRENGTH — field value

**Spec:** §2.2.3, p. 108 — OWNERSHIP_STRENGTH for DataWriter: "An integer
“value”" — "[optional] Specifies the value of the “strength” used to arbitrate
among multiple DataWriter objects that attempt to modify the same instance".

**Repo:** `crates/qos/src/policies/ownership_strength.rs`
(`OwnershipStrengthQosPolicy`); `crates/dcps/src/qos.rs`
(`DataWriterQos::ownership_strength`)

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_negative`,
`crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_large`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — default 0

**Spec:** §2.2.3, p. 108 — "The default value of the ownership_strength is
zero."

**Repo:** `crates/qos/src/policies/ownership_strength.rs`

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::default_is_zero`,
`crates/qos/src/defaults.rs::tests::ownership_strength_default_zero`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — RxO N/A

**Spec:** §2.2.3, p. 108 — OWNERSHIP_STRENGTH: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `DataWriterQos::ownership_strength`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — Changeable Yes

**Spec:** §2.2.3, p. 108 — OWNERSHIP_STRENGTH: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`:
`ownership_strength`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_matched_writer_announcing_new_strength_and_lifespan_is_taken_up`

**Status:** done

### §2.2.3.10 OWNERSHIP_STRENGTH — arbitration by the DataReader

**Spec:** §2.2.3.10, p. 118 — "It only applies to the situation case where
OWNERSHIP kind is set to EXCLUSIVE … The arbitration is performed by the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::passes_exclusive_ownership`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_filters_weaker_writer`

**Status:** done

### §2.2.3.11 LIVELINESS — fields kind and lease_duration

**Spec:** §2.2.3, p. 108 — LIVELINESS for Topic, DataReader, DataWriter: "A
“kind”: AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC and a duration
“lease_duration”".

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessKind`,
`LivelinessQosPolicy`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::roundtrip`,
`crates/qos/src/policies/liveliness.rs::tests::try_from_u32_strict`

**Status:** done

### §2.2.3.11 LIVELINESS — defaults

**Spec:** §2.2.3, p. 108 — "The default kind is AUTOMATIC and the default value
of the lease_duration is infinite."

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessQosPolicy::default`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::default_automatic_infinite`,
`crates/dcps/tests/liveliness_qos.rs::liveliness_default_infinite_keeps_counters_zero`

**Status:** done

### §2.2.3.11 LIVELINESS — RxO

**Spec:** §2.2.3.11, pp. 118–119 — "1. the inequality “offered kind >= requested
kind” … AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC. 2. the inequality
“offered lease_duration <= requested lease_duration” evaluates to TRUE."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::liveliness_kind_and_lease_checked`,
`crates/qos/src/policies/qos_set.rs::tests::liveliness_lease_mismatch_reported`

**Status:** done — as with OWNERSHIP the tests check the comparison in
`zerodds-qos`; `first_incompatible_policy` computes it itself. Duration 0 and
INFINITE coincide as with DEADLINE.

### §2.2.3.11 LIVELINESS — Changeable No

**Spec:** §2.2.3, p. 108 — LIVELINESS: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.11 LIVELINESS — AUTOMATIC

**Spec:** §2.2.3, p. 108 — "The infrastructure will automatically signal
liveliness for the DataWriters at least as often as required by the
lease_duration."; §2.2.3.11, p. 118 — "as long as the local process where a
DomainParticipant is running and the link connecting it to remote participants
remains connected, the entities within the DomainParticipant will be considered
alive."

**Repo:** `crates/dcps/src/wlp.rs`; `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::an_automatic_writer_that_never_writes_stays_alive_at_a_remote_reader`,
`crates/dcps/src/liveliness_tracker.rs::tests::an_automatic_writer_is_alive_from_its_match_without_writing`

**Status:** done

### §2.2.3.11 LIVELINESS — MANUAL: assertion within lease_duration

**Spec:** §2.2.3, p. 109 — "The user application takes responsibility to signal
liveliness to the Service … Liveliness must be asserted at least once every
lease_duration otherwise the Service will assume the corresponding Entity is no
longer “active/alive.”"; §2.2.3.11, p. 118 — "The action can be explicit by
calling the assert_liveliness operations, or implicit by writing some data."

**Repo:** `crates/dcps/src/liveliness_tracker.rs`; `crates/dcps/src/runtime.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_manual_writer_goes_not_alive_when_silent_and_back_when_it_writes`,
`crates/dcps/src/runtime.rs::tests::liveliness_lost_counts_a_silent_manual_writer_once_and_no_automatic_one`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`

**Status:** done

### §2.2.3.11 LIVELINESS — MANUAL_BY_PARTICIPANT

**Spec:** §2.2.3, p. 109 — "The Service will assume that as long as at least one
Entity within the DomainParticipant has asserted its liveliness the other
Entities in that same DomainParticipant are also alive."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`participant_asserted`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_liveliness`,
`DcpsRuntime::note_participant_liveliness`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_participant_message_asserts_only_what_its_kind_covers`,
`crates/dcps/src/runtime.rs::tests::two_runtimes_assert_liveliness_reaches_peer`

**Status:** done — the paths to the remote reader are listed in
`ddsi-rtps-2.5.en.md` (§8.4.13).

### §2.2.3.11 LIVELINESS — MANUAL_BY_TOPIC

**Spec:** §2.2.3, p. 109 — "The Service will only assume liveliness of the
DataWriter if the application has asserted liveliness of that DataWriter
itself."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::assert_writer_liveliness_eid`)

**Tests:** `crates/dcps/src/runtime.rs::tests::assert_liveliness_keeps_a_manual_writer_alive_without_writing`,
`crates/dcps/tests/liveliness_qos.rs::linux::assert_liveliness_keeps_a_manual_writer_alive_at_a_remote_reader`

**Status:** done

### §2.2.3.11 LIVELINESS — detection within lease_duration

**Spec:** §2.2.3.11, p. 119 — "Changes in LIVELINESS must be detected by the
Service with a time-granularity greater or equal to the lease_duration. This
ensures that the value of the LivelinessChangedStatus is updated at least once
during each lease_duration and the related Listeners and WaitSets are notified
within a lease_duration from the time the LIVELINESS changed."

**Repo:** `crates/dcps/src/runtime.rs` (tick of the lease check);
`crates/dcps/src/liveliness_tracker.rs`

**Tests:** `crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — field minimum_separation

**Spec:** §2.2.3, p. 109 — TIME_BASED_FILTER for DataReader: "A duration
"minimum_separation"" — "The filter states that the DataReader does not want to
receive more than one value each minimum_separation, regardless of how fast the
changes occur."

**Repo:** `crates/qos/src/policies/time_based_filter.rs`
(`TimeBasedFilterQosPolicy`); `crates/dcps/src/qos.rs`
(`DataReaderQos::time_based_filter`); `crates/dcps/src/runtime.rs`
(`TimeBasedFilterClock`)

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::roundtrip`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::the_sender_clock_filters_on_the_source_timestamps`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::the_receiver_clock_filters_on_the_arrival`

**Status:** `done` — The specification leaves open the clock
`minimum_separation` is measured on. The setting
`reader.time_based_filter_clock` chooses `sender` (the source timestamp from
INFO_TS, the reception time without one; the default) or `receiver` (the
reception time). It measured with the time of the read.

### §2.2.3.12 TIME_BASED_FILTER — default 0

**Spec:** §2.2.3, p. 109 — "By default minimum_separation=0 indicating
DataReader is potentially interested in all values."

**Repo:** `crates/qos/src/policies/time_based_filter.rs`

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::default_is_zero`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_zero_passes_all_samples`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — RxO N/A

**Spec:** §2.2.3, p. 109 — TIME_BASED_FILTER: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `DataReaderQos::time_based_filter`)

**Tests:** `crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_filters_close_samples_per_instance`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — Changeable Yes

**Spec:** §2.2.3, p. 109 — TIME_BASED_FILTER: Changeable "Yes".

**Repo:** `crates/dcps/src/subscriber.rs` (`runtime_qos_update`:
`time_based_filter`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::matched_data_across_participants_follows_a_set_qos`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — per instance

**Spec:** §2.2.3.12, p. 119 — "The TIME_BASED_FILTER applies to each instance
separately, that is, the constraint is that the DataReader does not want to see
more than one sample of each instance per minimum_separation period."

**Repo:** `crates/dcps/src/instance_tracker.rs` (filter per instance)

**Tests:** `crates/dcps/tests/time_based_filter_qos.rs::linux::min_separation_is_per_instance`,
`crates/dcps/src/instance_tracker.rs::tests::time_based_filter_per_instance_isolation`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — before HISTORY and RELIABILITY

**Spec:** §2.2.3.12, p. 119 — "The HISTORY and RELIABILITY QoS affect the
behavior of the middleware with respect to the samples that have been determined
to be of interest to the DataReader, that is, they apply after the
TIME_BASED_FILTER has been applied."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`: filter
before admission to the cache)

**Tests:** `crates/dcps/src/instance_tracker.rs::tests::time_based_filter_too_close_drops`

**Status:** done

### §2.2.3.12 TIME_BASED_FILTER — RELIABLE: last sample in steady state

**Spec:** §2.2.3.12, p. 119 — "In the case where the reliability QoS kind is
RELIABLE then in steady-state, defined as the situation where the DataWriter does
not write new samples for a period “long” compared to the minimum_separation,
the system should guarantee delivery the last sample to the DataReader."

**Repo:** `crates/dcps/src/presentation.rs` (`TbfHold`);
`crates/dcps/src/subscriber.rs` (`DataReader::release_due_tbf`,
`DataReader::arm_tbf_hook`); `crates/dcps/src/runtime.rs`
(`UserReaderSlot::tbf_gate`, `release_due_tbf`)

**Tests:**
`crates/dcps/src/presentation.rs::tests::the_last_filtered_sample_of_an_instance_is_due_a_separation_after_it_arrived`,
`crates/dcps/src/presentation.rs::tests::a_sample_that_passes_the_filter_supersedes_the_held_one`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::a_reliable_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet`,
`crates/dcps/tests/time_based_filter_qos.rs::linux::a_best_effort_reader_gets_no_filtered_sample`,
`crates/zerodds-c-api/tests/time_based_filter.rs::a_reliable_c_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet`

**Status:** `done` — A RELIABLE reader holds the last filtered sample of each
instance and delivers it `minimum_separation` after it arrived when none passed
until then; a tick hook raises DATA_AVAILABLE for it. Typed reader and runtime
path (C API, rmw).

### §2.2.3.12 TIME_BASED_FILTER — consistency with DEADLINE

**Spec:** §2.2.3.12, p. 119 — "For these two QoS policies to be consistent they
must verify that "period >= minimum_separation." An attempt to set these
policies in an inconsistent manner when an entity is created of via a set_qos
operation will cause the operation to fail."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::a_time_based_filter_longer_than_the_deadline_refuses_the_reader`

**Status:** `done` — as under §2.2.3.7.

### §2.2.3.13 PARTITION — field name

**Spec:** §2.2.3, p. 109 — PARTITION for Publisher, Subscriber: "A list of
strings “name”" — "A DataWriter within a Publisher only communicates with a
DataReader in a Subscriber if … the Publisher and Subscriber have a common
partition name string."

**Repo:** `crates/qos/src/policies/partition.rs` (`PartitionQosPolicy::names`);
`crates/dcps/src/qos.rs` (`PublisherQos::partition`, `SubscriberQos::partition`;
override per endpoint, `effective_partition`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::roundtrip_two_names`,
`crates/dcps/src/runtime.rs::tests::partition_from_publisher_reaches_the_writer_announcement`

**Status:** done

### §2.2.3.13 PARTITION — default and the empty string

**Spec:** §2.2.3, p. 109 — "The empty string ("") is considered a valid
partition … The default value for the PARTITION QoS is a zero-length sequence.
The zero-length sequence is treated as a special value equivalent to a sequence
containing a single element consisting of the empty string."

**Repo:** `crates/qos/src/policies/partition.rs` (`partitions_compatible`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::default_is_empty`,
`crates/qos/src/policies/partition.rs::tests::partitions_default_and_empty_string_are_equivalent`,
`crates/qos/src/policies/partition.rs::tests::partitions_named_does_not_match_default`

**Status:** done

### §2.2.3.13 PARTITION — RxO No, no incompatibility status

**Spec:** §2.2.3, p. 109 — PARTITION: RxO "No"; §2.2.3.13, p. 119 — "Failure to
match partitions is not considered an “incompatible” QoS and does not trigger any
listeners nor conditions."

**Repo:** `crates/dcps/src/runtime.rs` (partition check separate from
`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::partition_mismatch_blocks_match_without_any_incompatible_qos_event`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_unmatches_without_a_status_and_is_announced`

**Status:** done

### §2.2.3.13 PARTITION — Changeable Yes, new and broken matches

**Spec:** §2.2.3, p. 109 — PARTITION: Changeable "Yes"; §2.2.3.13, p. 119 — "A
change of this policy can potentially modify the “match” of existing DataReader
and DataWriter entities. It may establish new “matchs” that did not exist before,
or break existing matchs."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::set_qos`),
`crates/dcps/src/subscriber.rs` (`Subscriber::set_qos`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::update_user_group_qos`)

**Tests:** `crates/dcps/tests/set_qos_after_enable.rs::a_group_partition_change_unmatches_and_matches_again`,
`crates/dcps/src/runtime.rs::tests::a_partition_change_moves_a_same_runtime_pair_apart_and_together`

**Status:** done

### §2.2.3.13 PARTITION — wildcards after fnmatch

**Spec:** §2.2.3.13, p. 119 — "A partition name may contain wildcards. Sharing a
common partition means that one of the partition names matches … PARTITION names
can be regular expressions and include wildcards as defined by the POSIX fnmatch
API (1003.2-1992 section B.6). Either Publisher or Subscriber may include
regular expressions in partition names".

**Repo:** `crates/qos/src/policies/partition.rs` (`fnmatch`,
`partitions_compatible`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::fnmatch_star_matches_any`,
`crates/qos/src/policies/partition.rs::tests::fnmatch_char_class`,
`crates/qos/src/policies/partition.rs::tests::partitions_wildcard_matches_symmetrically`,
`crates/dcps/src/runtime.rs::tests::partitions_overlap_matches_canonical_fnmatch`

**Status:** done

### §2.2.3.13 PARTITION — two wildcard names never match

**Spec:** §2.2.3.13, p. 119 — "but no two names that both contain wildcards will
ever be considered to match. This means that although regular expressions may be
used both at publisher as well as subscriber side, the service will not try to
match two regular expressions".

**Repo:** `crates/qos/src/policies/partition.rs` (`partition_names_match`,
`has_wildcard`)

**Tests:**
`crates/qos/src/policies/partition.rs::tests::two_wildcard_names_never_match`

**Status:** `done` — two names that both hold wildcards do not match; a wildcard
against a literal name as before.

### §2.2.3.13 PARTITION — difference from domains

**Spec:** §2.2.3.13, p. 120 — "Partitions are different from creating Entity
objects in different domains in several ways … each unique data instance is
identified by the tuple (domainId, Topic, key)."

**Repo:** —

**Tests:** `crates/dcps/tests/partition_instances.rs::one_key_in_two_partitions_is_one_instance`

**Status:** done — a reader in the partitions A and B that receives key 7 from a writer in A and from one in B keeps one instance (one `InstanceHandle`).

### §2.2.3.14 RELIABILITY — fields kind and max_blocking_time

**Spec:** §2.2.3, p. 110 — RELIABILITY for Topic, DataReader, DataWriter: "A
“kind”: RELIABLE, BEST_EFFORT and a duration “max_blocking_time”".

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityKind`,
`ReliabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/reliability.rs::tests::best_effort_roundtrip_with_custom_blocking`

**Status:** done

### §2.2.3.14 RELIABILITY — defaults

**Spec:** §2.2.3, p. 110 — RELIABLE "This is the default value for
DataWriters"; BEST_EFFORT "This is the default value for DataReaders and Topics";
"The default max_blocking_time=100ms."

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityQosPolicy::default`);
`crates/dcps/src/qos.rs` (`DataWriterQos::default`, `DataReaderQos::default`,
`TopicQos::default`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::default_is_best_effort_100ms`,
`crates/qos/src/review_tests.rs::writer_qos_default_is_reliable`,
`crates/qos/src/review_tests.rs::reader_qos_default_is_best_effort`

**Status:** done

### §2.2.3.14 RELIABILITY — RxO: offered kind >= requested kind

**Spec:** §2.2.3.14, p. 120 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … BEST_EFFORT < RELIABLE."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/qos/src/compatibility.rs::tests::all_four_reliability_combinations_follow_rtps_8_4_4`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.3.14 RELIABILITY — Changeable No

**Spec:** §2.2.3, p. 110 — RELIABILITY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_immutable_reliability_post_enable`

**Status:** done

### §2.2.3.14 RELIABILITY — RELIABLE: repair and order

**Spec:** §2.2.3, p. 110 — "Specifies the Service will attempt to deliver all
samples in its history. Missed samples may be retried."; §2.2.3.14, p. 120 —
"data-samples originating from a single DataWriter cannot be made available to
the DataReader if there are previous data-samples that have not been received
yet due to a communication error."

**Repo:** `crates/rtps/src/reliable_reader.rs`; `crates/rtps/src/reliable_writer.rs`

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::out_of_order_data_buffered_until_gap_filled`,
`crates/rtps/src/reliable_reader.rs::tests::in_order_data_delivered_immediately`

**Status:** done — the protocol side is listed in `ddsi-rtps-2.5.en.md`.

### §2.2.3.14 RELIABILITY — BEST_EFFORT: no retransmission, no older version

**Spec:** §2.2.3.14, p. 120 — "If the RELIABILITY kind is set to BEST_EFFORT,
the service will not re-transmit missing data-samples. However for data-samples
originating from any one DataWriter the service will ensure they are stored in
the DataReader history in the same order they originated in the DataWriter."

**Repo:** `crates/rtps/src/reliable_reader.rs` (BEST_EFFORT path)

**Tests:** `crates/rtps/src/reliable_reader.rs::tests::best_effort_reader_skips_leading_gap`,
`crates/rtps/src/reliable_reader.rs::tests::duplicate_data_is_rejected`

**Status:** done

### §2.2.3.14 RELIABILITY — max_blocking_time and RESOURCE_LIMITS

**Spec:** §2.2.3, p. 110 — "The value of the max_blocking_time indicates the
maximum time the operation DataWriter::write is allowed to block if the
DataWriter does not have space to store the value written."; §2.2.3.14, p. 120.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`)

**Tests:** `crates/dcps/src/publisher.rs::tests::write_blocks_until_drain_when_reliable_max_samples_reached`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — field value

**Spec:** §2.2.3, p. 110 — TRANSPORT_PRIORITY for Topic, DataWriter: "An integer
“value”"; §2.2.3.15, p. 120 — "Any value within the range of a 32-bit signed
integer may be chosen; higher values indicate higher priority."

**Repo:** `crates/qos/src/policies/transport_priority.rs`
(`TransportPriorityQosPolicy`); `crates/dcps/src/runtime.rs`
(`send_on_best_interface` with priority)

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::roundtrip`,
`crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — default 0

**Spec:** §2.2.3, p. 110 — "The default value of the transport_priority is
zero."

**Repo:** `crates/qos/src/policies/transport_priority.rs`

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::default_is_zero`,
`crates/qos/src/defaults.rs::tests::transport_priority_default_zero`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — RxO N/A

**Spec:** §2.2.3, p. 110 — TRANSPORT_PRIORITY: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `TopicQos` and `DataWriterQos`)

**Tests:** `crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — Changeable Yes

**Spec:** §2.2.3, p. 110 — TRANSPORT_PRIORITY: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`:
`transport_priority`)

**Tests:** `crates/dcps/tests/transport_priority.rs::a_writers_transport_priority_reaches_the_runtime_and_follows_a_change`

**Status:** done

### §2.2.3.15 TRANSPORT_PRIORITY — hint to the transport

**Spec:** §2.2.3.15, p. 120 — "This policy is considered a hint. The policy
depends on the ability of the underlying transports to set a priority on the
messages they send … any further interpretation of this policy is specific to a
particular transport and a particular implementation of the Service."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — freedom of interpretation; the path of the
value to the transport is evidenced in the item “field value”.

### §2.2.3.16 LIFESPAN — field duration

**Spec:** §2.2.3, p. 110 — LIFESPAN for Topic, DataWriter: "A duration
“duration”" — "Specifies the maximum duration of validity of the data written by
the DataWriter".

**Repo:** `crates/qos/src/policies/lifespan.rs` (`LifespanQosPolicy`)

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::roundtrip`

**Status:** done

### §2.2.3.16 LIFESPAN — default infinite

**Spec:** §2.2.3, p. 110 — "The default value of the lifespan duration is
infinite."

**Repo:** `crates/qos/src/policies/lifespan.rs`

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::default_is_infinite`,
`crates/qos/src/defaults.rs::tests::lifespan_default_infinite`

**Status:** done

### §2.2.3.16 LIFESPAN — RxO N/A

**Spec:** §2.2.3, p. 110 — LIFESPAN: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `TopicQos` and `DataWriterQos`)

**Tests:** `crates/dcps/tests/lifespan_qos.rs::linux::lifespan_removes_expired_samples_from_the_reader_cache`

**Status:** done

### §2.2.3.16 LIFESPAN — Changeable Yes

**Spec:** §2.2.3, p. 110 — LIFESPAN: Changeable "Yes".

**Repo:** `crates/dcps/src/publisher.rs` (`runtime_qos_update`: `lifespan`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_matched_writer_announcing_new_strength_and_lifespan_is_taken_up`

**Status:** done

### §2.2.3.16 LIFESPAN — expiry from source time plus duration, removal from the reader cache

**Spec:** §2.2.3.16, pp. 120–121 — "Once the sample expires, the data will be
removed from the DataReader caches … The ‘expiration time’ of each sample is
computed by adding the duration specified by the LIFESPAN QoS to the source
timestamp."

**Repo:** `crates/dcps/src/subscriber.rs` (`lifespan_expiry`,
`purge_expired_by_lifespan`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::lifespan_expiry_adds_the_writer_duration_to_the_source_timestamp`,
`crates/dcps/src/subscriber.rs::tests::purge_expired_by_lifespan_drops_only_samples_past_their_expiration`,
`crates/dcps/tests/lifespan_qos.rs::linux::lifespan_removes_expired_samples_from_the_reader_cache`

**Status:** done

### §2.2.3.16 LIFESPAN — removal from TRANSIENT and PERSISTENT stores

**Spec:** §2.2.3.16, p. 120 — "Once the sample expires, the data will be removed
from the DataReader caches as well as from the transient and persistent
information caches."

**Repo:** —

**Tests:** `crates/dcps/tests/durability_store.rs::an_expired_sample_leaves_the_store`,
`crates/dcps/src/durability_service.rs::tests::in_memory_replay_drops_the_expired_sample`,
`crates/dcps/src/durability_service.rs::tests::on_disk_replay_drops_the_expired_sample`

**Status:** `done` — the sample carries `expires_at` (source time plus the writer's LIFESPAN, `<sequence>.exp` on disk); `replay_for_topic_at` and `cleanup` remove expired samples and instances left without a sample.

### §2.2.3.16 LIFESPAN — reception time instead of source time

**Spec:** §2.2.3.16, p. 121 — "If this is not the case and the Service can
detect it, the DataReader is allowed to use the reception timestamp instead of
the source timestamp in its computation of the ‘expiration time.’"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission; ZeroDDS computes with the source
time.

### §2.2.3.17 DESTINATION_ORDER — field kind

**Spec:** §2.2.3, pp. 110–111 — DESTINATION_ORDER for Topic, DataReader,
DataWriter: "A “kind”: BY_RECEPTION_TIMESTAMP, BY_SOURCE_TIMESTAMP".

**Repo:** `crates/qos/src/policies/destination_order.rs`
(`DestinationOrderKind`, `DestinationOrderQosPolicy`)

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::roundtrip`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — default BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.3, p. 110 — "The default kind is BY_RECEPTION_TIMESTAMP."

**Repo:** `crates/qos/src/policies/destination_order.rs`

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::default_by_reception`,
`crates/qos/src/defaults.rs::tests::destination_order_default_by_reception`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — RxO

**Spec:** §2.2.3.17, p. 121 — "The value offered is considered compatible with
the value requested if and only if the inequality “offered kind >= requested
kind” evaluates to ‘TRUE.’ … BY_RECEPTION_TIMESTAMP < BY_SOURCE_TIMESTAMP."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_offered_destination_order_incompatible_rejects_match`,
`crates/dcps/src/runtime.rs::tests::rxo_requested_destination_order_incompatible_rejects_match`,
`crates/qos/src/compatibility.rs::tests::destination_order_offered_ge_requested`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — Changeable No

**Spec:** §2.2.3, p. 110 — DESTINATION_ORDER: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::endpoints_refuse_a_destination_order_change_once_enabled`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.3.17, p. 121 — "The setting BY_RECEPTION_TIMESTAMP indicates that,
assuming the OWNERSHIP policy allows it, the latest received value for the
instance should be the one whose value is kept."

**Repo:** `crates/dcps/src/instance_tracker.rs`

**Tests:** `crates/dcps/tests/destination_order_qos.rs::by_reception_timestamp_always_delivers`

**Status:** done

### §2.2.3.17 DESTINATION_ORDER — BY_SOURCE_TIMESTAMP

**Spec:** §2.2.3.17, p. 121 — "The setting BY_SOURCE_TIMESTAMP indicates that,
assuming the OWNERSHIP policy allows it, a timestamp placed at the source should
be used. This is the only setting that, in the case of concurrent same-strength
DataWriter objects updating the same instance, ensures all subscribers will end
up with the same final value for the instance."

**Repo:** `crates/dcps/src/instance_tracker.rs`; `crates/dcps/src/subscriber.rs`
(`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/destination_order_qos.rs::by_source_timestamp_drops_older_samples`,
`crates/dcps/tests/destination_order_qos.rs::destination_order_is_per_instance`,
`crates/dcps/tests/time_source_destination_order.rs::test_clock_drives_destination_order_deterministically`

**Status:** done

### §2.2.3.18 HISTORY — fields kind and depth

**Spec:** §2.2.3, p. 111 — HISTORY for Topic, DataReader, DataWriter: "A “kind”:
KEEP_LAST, KEEP_ALL And an optional integer “depth”".

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryKind`,
`HistoryQosPolicy`)

**Tests:** `crates/qos/src/policies/history.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/history.rs::tests::encode_decode_keep_all`

**Status:** done

### §2.2.3.18 HISTORY — default KEEP_LAST, depth 1

**Spec:** §2.2.3, p. 111 — "KEEP_LAST is the default kind. The default value of
depth is 1."

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryQosPolicy::default`)

**Tests:** `crates/qos/src/policies/history.rs::tests::default_is_keep_last_1`,
`crates/qos/src/defaults.rs::tests::history_default_keep_last_1`

**Status:** done

### §2.2.3.18 HISTORY — RxO No

**Spec:** §2.2.3, p. 111 — HISTORY: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` without HISTORY)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** done

### §2.2.3.18 HISTORY — Changeable No

**Spec:** §2.2.3, p. 111 — HISTORY: Changeable "No".

**Repo:** `crates/dcps/src/qos.rs` (`immutable_writer_change`,
`immutable_reader_change`); `crates/dcps/src/topic.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_refuses_a_history_change_and_takes_a_deadline_change_once_enabled`

**Status:** done

### §2.2.3.18 HISTORY — KEEP_LAST

**Spec:** §2.2.3, p. 111; §2.2.3.18, p. 121 — "On the publishing side, the
Service will only attempt to keep the most recent “depth” samples of each
instance of data … On the subscribing side, the DataReader will only attempt to
keep the most recent “depth” samples received for each instance … until the
application “takes” them".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::evict_oldest_of_instance`);
`crates/dcps/src/publisher.rs` (`create_datawriter`: depth to the runtime)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_last_1_keeps_only_the_newest_per_instance`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_is_independent_per_instance`,
`crates/dcps/src/publisher.rs::tests::create_datawriter_wires_history_keep_last_depth`

**Status:** done

### §2.2.3.18 HISTORY — KEEP_ALL

**Spec:** §2.2.3, p. 112; §2.2.3.18, p. 121 — "the Service will attempt to keep
all samples … If the limit is reached, then the behavior of the Service will
depend on the RELIABILITY QoS. If the reliability kind is BEST_EFFORT, then the
old values will be discarded. If reliability is RELIABLE, then the Service will
block the DataWriter until it can deliver the necessary old values to all
subscribers."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::write`);
`crates/dcps/src/subscriber.rs` (intake under KEEP_ALL)

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`,
`crates/dcps/src/runtime.rs::tests::a_keep_all_writer_keeps_what_a_matched_reader_has_not_acknowledged`

**Status:** `done`

### §2.2.3.18 HISTORY — consistency depth <= max_samples_per_instance

**Spec:** §2.2.3, p. 111; §2.2.3.18, p. 121 — "The setting of HISTORY depth must
be consistent with the RESOURCE_LIMITS max_samples_per_instance. For these two
QoS to be consistent, they must verify that depth <= max_samples_per_instance."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`

**Status:** `done` — a KEEP_LAST depth above `max_samples_per_instance` and a
depth below 1 are refused.

### §2.2.3.19 RESOURCE_LIMITS — fields

**Spec:** §2.2.3, p. 112 — RESOURCE_LIMITS for Topic, DataReader, DataWriter:
"Three integers: max_samples, max_instances, max_samples_per_instance".

**Repo:** `crates/qos/src/policies/resource_limits.rs`
(`ResourceLimitsQosPolicy`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/resource_limits.rs::tests::encode_decode_negative_one`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — defaults LENGTH_UNLIMITED

**Spec:** §2.2.3, p. 112 — max_samples, max_instances,
max_samples_per_instance: "By default, LENGTH_UNLIMITED."

**Repo:** `crates/qos/src/policies/resource_limits.rs` (`LENGTH_UNLIMITED`,
`ResourceLimitsQosPolicy::default`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::default_is_unlimited`,
`crates/dcps/src/qos.rs::data_representation_tests::resource_limits_default_to_unlimited`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — RxO No

**Spec:** §2.2.3, p. 112 — RESOURCE_LIMITS: RxO "No".

**Repo:** `crates/dcps/src/runtime.rs` (`RxoView` without RESOURCE_LIMITS)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_limited_writer_with_an_acknowledging_reader_writes_everything`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — Changeable No

**Spec:** §2.2.3, p. 112 — RESOURCE_LIMITS: Changeable "No".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.19 RESOURCE_LIMITS — max_samples, max_instances, max_samples_per_instance

**Spec:** §2.2.3, p. 112 — "max_samples Specifies the maximum number of
data-samples the DataWriter (or DataReader) can manage across all the instances
… max_instances Represents the maximum number of instances DataWriter (or
DataReader) can manage … max_samples_per_instance Represents the maximum number
of samples of any one instance".

**Repo:** `crates/dcps/src/subscriber.rs` (admission check);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_writer_resource_limits`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_writer_refuses_an_instance_beyond_its_own_max_instances`

**Status:** done

### §2.2.3.19 RESOURCE_LIMITS — BEST_EFFORT drops, RELIABLE blocks

**Spec:** §2.2.3.19, p. 122 — "If reliability is BEST_EFFORT then the Service is
allowed to drop samples. If the reliability is RELIABLE, the Service will block
the DataWriter or discard the sample at the DataReader in order not to lose
existing samples."

**Repo:** `crates/dcps/src/subscriber.rs`; `crates/dcps/src/publisher.rs`

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_discards_the_oldest_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_reliable_sample_refused_for_room_arrives_once_there_is_room`,
`crates/dcps/tests/reader_room_backpressure.rs::a_full_keep_all_reader_blocks_its_writer_and_loses_nothing`

**Status:** `done`

### §2.2.3.19 RESOURCE_LIMITS — consistency

**Spec:** §2.2.3, p. 112; §2.2.3.19, p. 122 — "max_samples >=
max_samples_per_instance … depth <= max_samples_per_instance … An attempt to set
this policy to inconsistent values when an entity is created of via a set_qos
operation will cause the operation to fail."

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos::check_consistency`,
`DataReaderQos::check_consistency`, `TopicQos::check_consistency`), called by
`create_topic`, `create_datawriter`, `create_datareader` and the `set_qos` of
Topic, DataWriter and DataReader

**Tests:** `crates/dcps/tests/qos_consistency.rs::an_inconsistent_history_or_resource_limit_refuses_the_entity`,
`crates/dcps/tests/qos_consistency.rs::an_inconsistent_set_qos_keeps_the_previous_values`

**Status:** `done` — `max_samples` below `max_samples_per_instance` is refused
on creation and in `set_qos`; the C API also checks `is_consistent` when
converting the C QoS.

### §2.2.3.20 ENTITY_FACTORY — field autoenable_created_entities

**Spec:** §2.2.3, p. 112 — ENTITY_FACTORY for DomainParticipantFactory,
DomainParticipant, Publisher, Subscriber: "A boolean:
“autoenable_created_entities”" — "If autoenable_created_entities==TRUE the
factory will automatically enable each created Entity otherwise it will not."

**Repo:** `crates/qos/src/policies/entity_factory.rs`
(`EntityFactoryQosPolicy`); `crates/dcps/src/factory.rs`
(`DomainParticipantFactoryQos`); `crates/dcps/src/qos.rs`

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::roundtrip`,
`crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done

### §2.2.3.20 ENTITY_FACTORY — default TRUE

**Spec:** §2.2.3, p. 112 — "By default, TRUE."

**Repo:** `crates/qos/src/policies/entity_factory.rs`

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::default_is_true`,
`crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`

**Status:** done

### §2.2.3.20 ENTITY_FACTORY — RxO No

**Spec:** §2.2.3, p. 112 — ENTITY_FACTORY: RxO "No".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the policy concerns no match between writer
and reader; it is not announced.

### §2.2.3.20 ENTITY_FACTORY — Changeable Yes, affects later creations

**Spec:** §2.2.3, p. 112 — ENTITY_FACTORY: Changeable "Yes"; §2.2.3.20, p. 122
— "This policy is mutable. A change in the policy affects only the entities
created after the change; not the previously created entities."

**Repo:** `crates/dcps/src/participant.rs` (`DomainParticipant::set_qos`:
`set_autoenable_children`), `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::enabling_a_factory_enables_what_it_created_when_its_entity_factory_says_so`

**Status:** done — the value takes effect at creation and in the cascade of
`enable`; entities already created are not enabled by the change.

### §2.2.3.20 ENTITY_FACTORY — create_* calls enable

**Spec:** §2.2.3.20, p. 122 — "The setting of autoenable_created_entities to TRUE
indicates that the factory create_<entity> operation will automatically invoke
the enable operation each time a new Entity is created. Therefore, the Entity
returned by create_<entity> will already be enabled."

**Repo:** `crates/dcps/src/publisher.rs` (`Publisher::autoenable_child`),
`crates/dcps/src/subscriber.rs` (`Subscriber::autoenable_child`),
`crates/dcps/src/participant.rs` (`autoenables_children`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::datawriter_and_datareader_follow_their_factories_entity_factory_policy`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — field autodispose_unregistered_instances

**Spec:** §2.2.3, p. 113 — WRITER_DATA_LIFECYCLE for DataWriter: "A boolean:
“autodispose_unregistered_instances”" — "Controls whether a DataWriter will
automatically dispose instances each time they are unregistered."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`WriterDataLifecycleQosPolicy`); `crates/dcps/src/qos.rs`
(`DataWriterQos::writer_data_lifecycle`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_roundtrip`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — default TRUE

**Spec:** §2.2.3, p. 113 — "By default, TRUE."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_default_is_autodispose_true`,
`crates/qos/src/defaults.rs::tests::writer_data_lifecycle_default_autodispose`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — RxO N/A

**Spec:** §2.2.3, p. 113 — WRITER_DATA_LIFECYCLE: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `DataWriterQos`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — Changeable Yes

**Spec:** §2.2.3, p. 113 — WRITER_DATA_LIFECYCLE: Changeable "Yes".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.21 WRITER_DATA_LIFECYCLE — TRUE and FALSE

**Spec:** §2.2.3.21, pp. 122–123 — "The setting ‘autodispose_unregistered_instances
= TRUE’ causes the DataWriter to dispose the instance each time it is
unregistered … The setting ‘autodispose_unregistered_instances = FALSE’ will not
cause this automatic disposition upon unregistering."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::unregister_instance_w_timestamp`)

**Tests:** `crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_default_true_unregister_disposes`,
`crates/dcps/tests/writer_data_lifecycle_qos.rs::autodispose_false_unregister_only_decrements_writer_count`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`

**Status:** done

### §2.2.3.21 WRITER_DATA_LIFECYCLE — deleting the DataWriter

**Spec:** §2.2.3.21, p. 123 — "the setting of the
autodispose_unregistered_instances flag will determine whether instances are
ultimately disposed when the DataWriter is deleted either directly by means of
the Publisher::delete_datawriter operation or indirectly as a consequence of
calling delete_contained_entities".

**Repo:** `crates/dcps/src/publisher.rs` (`WriterInner::unregister_all_instances`)

**Tests:** `crates/dcps/tests/entity_deletion.rs::delete_datawriter_disposes_and_unregisters_its_instances`

**Status:** done — `Test_FinalInstanceState_2` of the OMG suite is
specification-conformant with this behaviour (`internal/release/rc7-omg-findings.md`).

### §2.2.3.22 READER_DATA_LIFECYCLE — fields

**Spec:** §2.2.3, p. 113 — READER_DATA_LIFECYCLE for DataReader: "Two durations
“autopurge_nowriter_samples_delay” and “autopurge_disposed_samples_delay”".

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`ReaderDataLifecycleQosPolicy`); `crates/dcps/src/qos.rs`
(`DataReaderQos::reader_data_lifecycle`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_roundtrip`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — defaults infinite

**Spec:** §2.2.3, p. 113 — both durations "By default, infinite."

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_default_is_infinite`,
`crates/dcps/tests/reader_data_lifecycle_qos.rs::default_infinite_delay_keeps_disposed_instance`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — RxO N/A

**Spec:** §2.2.3, p. 113 — READER_DATA_LIFECYCLE: RxO "N/A".

**Repo:** `crates/dcps/src/qos.rs` (only `DataReaderQos`)

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — Changeable Yes

**Spec:** §2.2.3, p. 113 — READER_DATA_LIFECYCLE: Changeable "Yes".

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::the_changeable_column_holds_after_enable`

**Status:** done — the test covers it.

### §2.2.3.22 READER_DATA_LIFECYCLE — autopurge_nowriter_samples_delay

**Spec:** §2.2.3.22, p. 123 — "The autopurge_nowriter_samples_delay defines the
maximum duration for which the DataReader will maintain information regarding an
instance once its instance_state becomes NOT_ALIVE_NO_WRITERS. After this time
elapses, the DataReader will purge all internal information regarding the
instance, any untaken samples will also be lost."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`);
`crates/dcps/src/instance_tracker.rs`

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_nowriter_delay_purges_after_window`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_no_writers_after_delay`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — autopurge_disposed_samples_delay

**Spec:** §2.2.3.22, p. 123 — "The autopurge_disposed_samples_delay defines the
maximum duration for which the DataReader will maintain samples for an instance
once its instance_state becomes NOT_ALIVE_DISPOSED. After this time elapses, the
DataReader will purge all samples for the instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::run_reader_autopurge`)

**Tests:** `crates/dcps/tests/reader_data_lifecycle_qos.rs::finite_disposed_delay_purges_after_window`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_disposed_after_delay`,
`crates/dcps/src/instance_tracker.rs::tests::autopurge_disposed_before_delay_keeps_instance`

**Status:** done

### §2.2.3.22 READER_DATA_LIFECYCLE — state after the last take

**Spec:** §2.2.3.22, p. 123 — "The DataReader also maintains information
regarding the identity, view_state and instance_state of data-instances even
after all samples have been ‘taken.’ This is needed to properly compute the
states when future samples arrive."

**Repo:** `crates/dcps/src/instance_tracker.rs` (`InstanceTracker` remains after
`drain_samples`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_view_state_transitions_to_not_new_on_second_sample`,
`crates/dcps/src/instance_tracker.rs::tests::drain_samples_decrements_count`

**Status:** done

### §2.2.3.23 Registration, LIVELINESS and OWNERSHIP — use cases

**Spec:** §2.2.3.23, p. 123 — registration serves ownership resolution in
redundant systems and the detection of lost connectivity.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale; the requirements are in the
following items and under §2.2.3.9.

### §2.2.3.23.1 Ownership resolution on redundant systems

**Spec:** §2.2.3.23.1, pp. 123–124 — causes [crash], [connectivity loss],
[application fault]; "Switching ownership from a higher strength DataWriter to
one of a lower strength DataWriter requires that the DataReader can make a
determination that the stronger DataWriter is “no longer writing the
instance.”"

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`, `passes_exclusive_ownership`);
`crates/dcps/src/instance_tracker.rs` (`accept_under_exclusive_ownership`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`,
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — ownership is released on loss of liveliness, loss of the
match, `unregister_instance` and a missed DEADLINE.

### §2.2.3.23.1 unregister_instance of the owner hands over ownership

**Spec:** §2.2.3.23.3, p. 125 — "For a Topic with EXCLUSIVE OWNERSHIP if the
current owner of an instance unregisters it, then it will relinquish ownership
of the instance and thus the readers may see the value updated by another
writer"; §2.2.2.4.2.7, p. 64.

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::release_owner_of`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_unregisters_gives_up_the_instance`

**Status:** `done` — when the owner unregisters the instance, the weaker writer
takes it with its next sample.

### §2.2.3.23.1.1 Periodically written data — DEADLINE

**Spec:** §2.2.3.23.1.1, p. 124 — "If the deadline is missed, the DataReader
considers the DataWriter “not alive” and automatically gives ownership to the
next highest-strength DataWriter that is alive."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`accept_under_exclusive_ownership` with `deadline_nanos`)

**Tests:**
`crates/dcps/tests/instance_writers_lifecycle.rs::an_owner_that_missed_its_deadline_gives_the_instance_up`

**Status:** `done` — an owner whose last sample of the instance is older than
the DEADLINE counts as not alive for the choice; the next sample of another
writer takes over.

### §2.2.3.23.1.2 Data not written periodically — LIVELINESS

**Spec:** §2.2.3.23.1.2, p. 124 — "Ownership is maintained while the DataWriter
is “alive” and for the DataWriter to be alive it must fulfill its “LIVELINESS”
QoS contract … Hence the need for the writer to “register” and “unregister”
instances … unregistration will also require a message to be sent to the
readers."

**Repo:** `crates/dcps/src/publisher.rs`
(`DataWriter::unregister_instance_w_timestamp`: unregistration on the wire);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:**
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::unregister_with_autodispose_false_propagates_unregister_only`,
`crates/dcps/tests/ownership_failover.rs::notify_writer_liveliness_lost_clears_owner`

**Status:** `done` — the unregistration goes to the readers as a message; a loss
of the owner's liveliness releases ownership.

### §2.2.3.23.2 Detection of loss in topological connectivity

**Spec:** §2.2.3.23.2, p. 125 — "For the case where data is being written
periodically, the DEADLINE QoS and the on_deadline_missed listener provides the
notification. The case where data is not periodically updated requires the use
of the LIVELINESS in combination with register/unregister instance to detect
whether the “connectivity” has been lost, and the notification is provided by
means of the “NO_WRITERS” view state."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, liveliness check);
`crates/dcps/src/subscriber.rs` (`DataReader::apply_unmatched_writers`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/endpoint_delete_unmatch_e2e.rs::linux_e2e::dropping_a_writer_unmatches_the_remote_reader`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::instance_goes_no_writers_when_its_last_writer_disappears`

**Status:** `done` — DEADLINE reports by REQUESTED_DEADLINE_MISSED, the loss of
liveliness by LIVELINESS_CHANGED and NOT_ALIVE_NO_WRITERS, the deletion of a
writer by NOT_ALIVE_NO_WRITERS.

### §2.2.3.23.3 dispose compared with unregister_instance

**Spec:** §2.2.3.23.3, p. 125 — "The dispose operation indicates that the
data-instance no longer exists … whereas the unregister_instance operation
indicates that the writer is no longer taking responsibility for updating the
value of the instance."

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter::dispose`,
`DataWriter::unregister_instance`)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_unregister_drops_to_no_writers_when_count_zero`

**Status:** done

### §2.2.3.23.3 EXCLUSIVE: dispose by the owner

**Spec:** §2.2.3.23.3, p. 125 — "For a Topic with EXCLUSIVE OWNERSHIP if the
current owner of an instance disposes it, the readers accessing the instance
will see the instance_state as being “DISPOSED” and not see the values being
written by the weaker writer (even after the stronger one has disposed the
instance)."

**Repo:** `crates/dcps/src/subscriber.rs` (`ingest_pending`: owner check for
`dispose`)

**Tests:** `crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** `done` — the owner disposes; a weaker writer can neither dispose nor
write after it.

### §2.2.4 Two mechanisms

**Spec:** §2.2.4, p. 125 — "Listeners and conditions (in conjunction with
wait-sets) are two alternative mechanisms that allow the application to be made
aware of changes in the DCPS communication status."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the requirements are in the
following subclauses.

### §2.2.4.1 Statuses per entity

**Spec:** §2.2.4.1, p. 126 — table: Topic INCONSISTENT_TOPIC; Subscriber
DATA_ON_READERS; DataReader SAMPLE_REJECTED, LIVELINESS_CHANGED,
REQUESTED_DEADLINE_MISSED, REQUESTED_INCOMPATIBLE_QOS, DATA_AVAILABLE,
SAMPLE_LOST, SUBSCRIPTION_MATCHED; DataWriter LIVELINESS_LOST,
OFFERED_DEADLINE_MISSED, OFFERED_INCOMPATIBLE_QOS, PUBLICATION_MATCHED.

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`);
`crates/dcps/src/status.rs`; `get_*_status` per entity in
`crates/dcps/src/topic.rs`, `crates/dcps/src/publisher.rs`,
`crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done — the assignment to the entity is evidenced per status below
and at the `get_*_status` operations (§2.2.2.3 to §2.2.2.5).

### §2.2.4.1 Read statuses and plain statuses

**Spec:** §2.2.4.1, p. 126 — "Read communication statuses: i.e., those that are
related to arrival of data, namely DATA_ON_READERS and DATA_AVAILABLE. Plain
communication statuses: i.e., all the others. … This ‘grouping’ has to be
communicated to the application. How this is done is discussed in each of the
two following sub clauses."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — classification; the rules are under
§2.2.4.2 and §2.2.4.3.

### §2.2.4.1 One structure per plain status

**Spec:** §2.2.4.1, p. 127 — "For each plain communication status, there is a
corresponding structure to hold the status value. These values contain the
information related to the change of status, as well as information related to
the statuses themselves (e.g., contains cumulative counts)." Figure 2.13.

**Repo:** `crates/dcps/src/status.rs` (`InconsistentTopicStatus`,
`SampleLostStatus`, `SampleRejectedStatus`, `LivelinessLostStatus`,
`LivelinessChangedStatus`, `OfferedDeadlineMissedStatus`,
`RequestedDeadlineMissedStatus`, `OfferedIncompatibleQosStatus`,
`RequestedIncompatibleQosStatus`, `PublicationMatchedStatus`,
`SubscriptionMatchedStatus`, `QosPolicyCount`)

**Tests:** `crates/dcps/src/status.rs::tests::inconsistent_topic_default_is_zero`,
`crates/dcps/src/status.rs::tests::publication_matched_with_handle_roundtrip`,
`crates/dcps/src/status.rs::tests::offered_incompatible_qos_clone_roundtrip`

**Status:** done — the fields have the names of Figure 2.13; `long` is `i32`,
`QosPolicyId_t` is `u32`. DATA_AVAILABLE and DATA_ON_READERS have empty
structures (`DataAvailableStatus`, `DataOnReadersStatus`).

### §2.2.4.1 InconsistentTopicStatus.total_count

**Spec:** §2.2.4.1, p. 128 — "Total cumulative count of the Topics discovered
whose name matches the Topic to which this status is attached and whose type is
inconsistent with the Topic."

**Repo:** `crates/dcps/src/topic.rs` (`inconsistent_topic_total`);
`crates/dcps/src/runtime.rs` (`DcpsRuntime::record_inconsistent_topic`,
`inconsistent_topics_counted`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::xtypes_step1_refuses_a_differently_named_non_assignable_type`,
`crates/dcps/tests/listener_trigger_c22c.rs::inconsistent_topic_listener_fires_on_qos_mismatch_of_a_name`,
`crates/dcps/src/runtime.rs::tests::an_inconsistent_topic_counts_once_per_remote_participant`

**Status:** `done` — counted are a local `create_topic` with the same name and
a different QoS and each discovered remote Topic with an incompatible type once.

### §2.2.4.1 InconsistentTopicStatus.total_count — per discovered Topic

**Spec:** §2.2.4.1, p. 128 — "Total cumulative count of the Topics discovered
whose name matches the Topic".

**Repo:** `crates/dcps/src/runtime.rs`
(`DcpsRuntime::record_inconsistent_topic`, `forget_inconsistent_topics_of`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::an_inconsistent_topic_counts_once_per_remote_participant`

**Status:** `done` — a remote Topic counts once per remote participant until the
participant leaves, whatever number of endpoints it shows; the local
`create_topic` wakes the listener.

### §2.2.4.1 InconsistentTopicStatus.total_count_change

**Spec:** §2.2.4.1, p. 128 — "The incremental number of inconsistent topics
discovered since the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/tests/spec_misc.rs::get_inconsistent_topic_status_reports_the_change_once`

**Status:** done — the test covers it.

### §2.2.4.1 SampleLostStatus.total_count

**Spec:** §2.2.4.1, pp. 126, 128 — SAMPLE_LOST "A sample has been lost (never
received)"; total_count "Total cumulative count of all samples lost across of
instances of data published under the Topic."

**Repo:** `crates/dcps/src/runtime.rs` (`note_lost`,
`DcpsRuntime::record_sample_lost`); `crates/dcps/src/subscriber.rs`
(`DataReader::note_sample_lost`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::samples_a_best_effort_reader_steps_over_are_sample_lost`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`

**Status:** `done` — counted are the gaps a BEST_EFFORT reader steps over and
what a writer that left was missing before held samples; displaced received
samples do not count.

### §2.2.4.1 SampleLostStatus.total_count — only samples never received

**Spec:** §2.2.4.1, p. 126 — "SAMPLE_LOST A sample has been lost (never
received)."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`)

**Tests:**
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_discards_the_oldest_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_best_effort`,
`crates/dcps/tests/reader_history_resource_limits.rs::on_sample_rejected_listener_fires_for_a_best_effort_reader`

**Status:** `done` — a sample displaced under BEST_EFFORT changes no status
(§2.2.3.18: “the old values will be discarded”); the one refused for
`max_instances` is SAMPLE_REJECTED with `REJECTED_BY_INSTANCES_LIMIT`, also in
the C API.

### §2.2.4.1 SampleLostStatus.total_count_change

**Spec:** §2.2.4.1, p. 128 — "The incremental number of samples lost since the
last time the listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger`, `StatusRecord for
SampleLostStatus`)

**Tests:** `crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`,
`crates/dcps/src/status.rs::tests::a_change_after_the_look_survives_the_acknowledgement`

**Status:** done

### §2.2.4.1 SampleRejectedStatus.total_count

**Spec:** §2.2.4.1, p. 128 — "Total cumulative count of samples rejected by the
DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`,
`DataReader::note_sample_rejected`); `crates/dcps/src/runtime.rs`
(`DcpsRuntime::record_sample_rejected`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::a_reliable_sample_refused_for_room_arrives_once_there_is_room`

**Status:** done — a held-back RELIABLE sample counts once, even when it is
checked several times.

### §2.2.4.1 SampleRejectedStatus.total_count_change

**Spec:** §2.2.4.1, p. 128 — "The incremental number of samples rejected since
the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::get_sample_rejected_status_reports_the_change_once`

**Status:** done — the test covers it.

### §2.2.4.1 SampleRejectedStatus.last_reason

**Spec:** §2.2.4.1, p. 128 — "Reason for rejecting the last sample rejected. If
no samples have been rejected, the reason is the special value NOT_REJECTED."

**Repo:** `crates/dcps/src/status.rs` (`SampleRejectedStatusKind`)

**Tests:** `crates/dcps/src/status.rs::tests::sample_rejected_status_default_kind_is_not_rejected`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_total_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`

**Status:** done

### §2.2.4.1 SampleRejectedStatus.last_instance_handle

**Spec:** §2.2.4.1, p. 128 — "Handle to the instance being updated by the last
sample that was rejected."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::ingest_alive`)

**Tests:** `crates/dcps/tests/reader_history_resource_limits.rs::keep_all_max_samples_per_instance_rejects_when_reliable`,
`crates/dcps/tests/reader_history_resource_limits.rs::max_instances_rejects_when_reliable`

**Status:** done — for `RejectedByInstancesLimit` the handle is NIL, because no
instance was created for the refused sample; likewise for a Topic without a key.

### §2.2.4.1 LivelinessChangedStatus.alive_count

**Spec:** §2.2.4.1, p. 128 — "The total number of currently active DataWriters
that write the Topic read by the DataReader. This count increases when a newly
matched DataWriter asserts its liveliness for the first time or when a
DataWriter previously considered to be not alive reasserts its liveliness. The
count decreases when a DataWriter considered alive fails to assert its liveliness
and becomes not alive, whether because it was deleted normally or for some other
reason."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::two_automatic_writers_are_two_alive_writers`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_manual_writer_counts_from_its_first_assertion`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/src/liveliness_tracker.rs::tests::unmatching_takes_a_writer_out_of_the_number_it_was_in`

**Status:** done — an AUTOMATIC writer counts as alive from its match.

### §2.2.4.1 LivelinessChangedStatus.not_alive_count

**Spec:** §2.2.4.1, p. 128 — "The total count of currently DataWriters that
write the Topic read by the DataReader that are no longer asserting their
liveliness. This count increases when a DataWriter considered alive fails to
assert its liveliness and becomes not alive for some reason other than the normal
deletion of that DataWriter. It decreases when a previously not alive DataWriter
either reasserts its liveliness or is deleted normally."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::expire`,
`LivelinessTracker::unmatched`, `lost_with_participant`)

**Tests:**
`crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/src/liveliness_tracker.rs::tests::unmatching_takes_a_writer_out_of_the_number_it_was_in`,
`crates/dcps/tests/liveliness_qos.rs::linux::reader_with_short_lease_marks_writer_not_alive_when_silent`,
`crates/dcps/src/liveliness_tracker.rs::tests::a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts`

**Status:** `done` — expiry of the lease, loss with the participant, reassertion
and deletion.

### §2.2.4.1 LivelinessChangedStatus.not_alive_count — loss without deletion

**Spec:** §2.2.4.1, p. 128 — "This count increases when a DataWriter considered
alive fails to assert its liveliness and becomes not alive for some reason other
than the normal deletion of that DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`Departure::Lost`);
`crates/dcps/src/liveliness_tracker.rs` (`lost_with_participant`)

**Tests:**
`crates/dcps/src/liveliness_tracker.rs::tests::a_writer_lost_with_its_participant_is_not_alive_until_it_reasserts`,
`crates/dcps/tests/liveliness_qos.rs::linux::a_writer_lost_with_its_participant_counts_as_not_alive`

**Status:** `done` — when a participant's SPDP lease expires, its writers move
from `alive_count` to `not_alive_count` and stay there until they are matched
again under their GUID and assert; SPDP and SEDP dispose stay deletion. Cyclone
11 (`ddsi_delete_proxy_writer`) and OpenDDS (`DataReaderImpl::writer_removed`)
count both as deletion.

### §2.2.4.1 LivelinessChangedStatus.alive_count_change and not_alive_count_change

**Spec:** §2.2.4.1, p. 128 — "The change in the alive_count since the last time
the listener was called or the status was read … The change in the
not_alive_count since the last time the listener was called or the status was
read."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::take_status`,
`LivelinessTracker::acknowledge`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_listener_acknowledges_what_it_was_handed_and_no_more`,
`crates/dcps/src/status.rs::tests::liveliness_changed_acknowledges_both_changes`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.4.1 LivelinessChangedStatus.last_publication_handle

**Spec:** §2.2.4.1, p. 128 — "Handle to the last DataWriter whose change in
liveliness caused this status to change."

**Repo:** `crates/dcps/src/liveliness_tracker.rs` (`LivelinessTracker::status`:
`InstanceHandle::from_guid`)

**Tests:** `crates/dcps/src/liveliness_tracker.rs::tests::a_silent_writer_moves_from_alive_to_not_alive_and_back`,
`crates/dcps/tests/liveliness_qos.rs::linux::get_liveliness_changed_status_reports_and_resets_the_changes`

**Status:** done

### §2.2.4.1 RequestedDeadlineMissedStatus.total_count

**Spec:** §2.2.4.1, p. 129 — "Total cumulative number of missed deadlines
detected for any instance read by the DataReader. Missed deadlines accumulate;
that is, each deadline period the total_count will be incremented by one for
each instance for which data was not received."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, `missed_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done — counted per instance that has received a sample; before the
first sample of an instance no period runs.

### §2.2.4.1 RequestedDeadlineMissedStatus.total_count_change

**Spec:** §2.2.4.1, p. 129 — "The incremental number of deadlines detected since
the last time the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::requested_deadline_missed_total_count_change_is_read_and_reset`

**Status:** done — the test reads `total_count` and `total_count_change` before and after the read and after a further change.

### §2.2.4.1 RequestedDeadlineMissedStatus.last_instance_handle

**Spec:** §2.2.4.1, p. 129 — "Handle to the last instance in the DataReader for
which a deadline was detected."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::with_missed_instance`);
`crates/dcps/src/runtime.rs` (`missed_deadline_instance`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::requested_deadline_is_missed_per_received_instance`

**Status:** done

### §2.2.4.1 RequestedIncompatibleQosStatus.total_count

**Spec:** §2.2.4.1, p. 129 — "Total cumulative number of times the concerned
DataReader discovered a DataWriter for the same Topic with an offered QoS that
was incompatible with that requested by the DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_requested_incompatible`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`,
`crates/dcps/src/runtime.rs::tests::an_incompatible_remote_endpoint_is_counted_once`

**Status:** done — a writer counts once per policy until the pair matches or
the writer leaves.

### §2.2.4.1 RequestedIncompatibleQosStatus.total_count_change

**Spec:** §2.2.4.1, p. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for
RequestedIncompatibleQosStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 RequestedIncompatibleQosStatus.last_policy_id

**Spec:** §2.2.4.1, p. 129 — "The QosPolicyId_t of one of the policies that was
found to be incompatible the last time an incompatibility was detected."

**Repo:** `crates/dcps/src/runtime.rs` (`first_incompatible_policy`,
`DcpsRuntime::note_requested_incompatible`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done — it is the first incompatible policy in the order of
`first_incompatible_policy`.

### §2.2.4.1 RequestedIncompatibleQosStatus.policies

**Spec:** §2.2.4.1, p. 129 — "A list containing for each policy the total number
of times that the concerned DataReader discovered a DataWriter for the same Topic
with an offered QoS that is incompatible with that requested by the DataReader."

**Repo:** `crates/dcps/src/status.rs` (`bump_policy_count`)

**Tests:** `crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`,
`crates/dcps/src/runtime.rs::tests::rxo_presentation_coherent_access_unoffered_rejects_on_both_sides`

**Status:** done — per refused writer the one policy named by
`first_incompatible_policy` is counted; further incompatible policies of the same
pair do not count.

### §2.2.4.1 LivelinessLostStatus.total_count

**Spec:** §2.2.4.1, p. 129 — "Total cumulative number of times that a
previously-alive DataWriter became not alive due to a failure to actively signal
its liveliness within its offered liveliness period. This count does not change
when an already not alive DataWriter simply remains not alive for another
liveliness period."

**Repo:** `crates/dcps/src/runtime.rs` (`check_writer_liveliness`,
`liveliness_lost`)

**Tests:** `crates/dcps/src/runtime.rs::tests::liveliness_lost_counts_a_silent_manual_writer_once_and_no_automatic_one`

**Status:** done

### §2.2.4.1 LivelinessLostStatus.total_count_change

**Spec:** §2.2.4.1, p. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::liveliness_lost_total_count_change_is_read_and_reset`

**Status:** done — as above.

### §2.2.4.1 OfferedDeadlineMissedStatus.total_count

**Spec:** §2.2.4.1, p. 129 — "Total cumulative number of offered deadline
periods elapsed during which a DataWriter failed to provide data. Missed
deadlines accumulate; that is, each deadline period the total_count will be
incremented by one."

**Repo:** `crates/dcps/src/runtime.rs` (`check_deadlines`, `missed_deadlines`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`,
`crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_increments_when_writer_misses_period`

**Status:** done — counted per period and instance, as §2.2.3.7 frames the
contract per instance; the difference from the wording for the reader is
recorded in the findings file.

### §2.2.4.1 OfferedDeadlineMissedStatus.total_count_change

**Spec:** §2.2.4.1, p. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::offered_deadline_missed_total_count_change_is_read_and_reset`

**Status:** done — as above.

### §2.2.4.1 OfferedDeadlineMissedStatus.last_instance_handle

**Spec:** §2.2.4.1, p. 129 — "Handle to the last instance in the DataWriter for
which an offered deadline was missed."

**Repo:** `crates/dcps/src/publisher.rs` (`WriterStatus`: `last_instance_handle`
from `missed_deadline_instance`)

**Tests:** `crates/dcps/tests/deadline_qos.rs::linux::offered_deadline_is_missed_per_instance`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.total_count

**Spec:** §2.2.4.1, p. 129 — "Total cumulative number of times the concerned
DataWriter discovered a DataReader for the same Topic with a requested QoS that
is incompatible with that offered by the DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_offered_incompatible`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`,
`crates/dcps/src/runtime.rs::tests::a_same_runtime_pair_is_refused_for_an_rxo_policy_and_counted_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.total_count_change

**Spec:** §2.2.4.1, p. 129 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for
OfferedIncompatibleQosStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.last_policy_id

**Spec:** §2.2.4.1, p. 130 — "The PolicyId_t of one of the policies that was
found to be incompatible the last time an incompatibility was detected."

**Repo:** `crates/dcps/src/runtime.rs` (`DcpsRuntime::note_offered_incompatible`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_access_scope_below_request_rejects_on_both_sides`,
`crates/dcps/tests/status_reads.rs::incompatible_qos_is_counted_once_and_read_once`

**Status:** done

### §2.2.4.1 OfferedIncompatibleQosStatus.policies

**Spec:** §2.2.4.1, p. 130 — "A list containing for each policy the total number
of times that the concerned DataWriter discovered a DataReader for the same Topic
with a requested QoS that is incompatible with that offered by the DataWriter."

**Repo:** `crates/dcps/src/status.rs` (`bump_policy_count`)

**Tests:** `crates/dcps/src/runtime.rs::tests::rxo_presentation_ordered_access_unoffered_rejects_on_both_sides`,
`crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`

**Status:** done — as with the reader, only the first incompatible policy per
pair.

### §2.2.4.1 PublicationMatchedStatus.total_count

**Spec:** §2.2.4.1, p. 130 — "Total cumulative count the concerned DataWriter
discovered a “match” with a DataReader. That is, it found a DataReader for the
same Topic with a requested QoS that is compatible with that offered by the
DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.total_count_change

**Spec:** §2.2.4.1, p. 130 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for PublicationMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.last_subscription_handle

**Spec:** §2.2.4.1, p. 130 — "Handle to the last DataReader that matched the
DataWriter causing the status to change."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done — after an unmatch the field names the reader that left.

### §2.2.4.1 PublicationMatchedStatus.current_count

**Spec:** §2.2.4.1, p. 130 — "The number of DataReaders currently matched to the
concerned DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`publication_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`

**Status:** done

### §2.2.4.1 PublicationMatchedStatus.current_count_change

**Spec:** §2.2.4.1, p. 130 — "The change in current_count since the last time
the listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for PublicationMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::publication_matched_counts_every_reader_that_ever_matched`,
`crates/dcps/src/status.rs::tests::an_acknowledgement_keeps_what_came_after_the_report`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.total_count

**Spec:** §2.2.4.1, p. 130 — "Total cumulative count the concerned DataReader
discovered a “match” with a DataWriter."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.total_count_change

**Spec:** §2.2.4.1, p. 130 — "The change in total_count since the last time the
listener was called or the status was read."

**Repo:** `crates/dcps/src/status.rs` (`StatusRecord for SubscriptionMatchedStatus`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.last_publication_handle

**Spec:** §2.2.4.1, p. 130 — "Handle to the last DataWriter that matched the
DataReader causing the status to change."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.current_count

**Spec:** §2.2.4.1, p. 130 — "The number of DataWriters currently matched to the
concerned DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`subscription_matched`)

**Tests:** `crates/dcps/tests/status_reads.rs::subscription_matched_names_the_writer`

**Status:** done

### §2.2.4.1 SubscriptionMatchedStatus.current_count_change

**Spec:** §2.2.4.1, p. 130 — "The change in current_count since the last time
the listener was called or the status was read."

**Repo:** —

**Tests:** `crates/dcps/src/status.rs::tests::subscription_matched_current_count_change_is_read_and_reset`

**Status:** done — the test reads `current_count_change` after a match (1), after the loss (-1) and for a match with its loss before the read (0).

### §2.2.4.1 QosPolicyCount — policy_id and count

**Spec:** §2.2.4.1, p. 127 — Figure 2.13: QosPolicyCount with
`policy_id : QosPolicyId_t` and `count : long`.

**Repo:** `crates/dcps/src/status.rs` (`QosPolicyCount`)

**Tests:** `crates/dcps/src/status.rs::tests::bump_policy_count_inserts_then_increments`

**Status:** done

### §2.2.4.2 StatusChangedFlag per status

**Spec:** §2.2.4.2, pp. 130–131 — "Associated with each one of an Entity’s
communication status is a logical StatusChangedFlag. This flag indicates whether
that particular communication status has changed since the last time the status
was ‘read’ by the application … Note that Figure 2.14 is only conceptual".

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::unread`: the flag from the
change fields); `crates/dcps/src/entity.rs` (`EntityState::status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`

**Status:** done

### §2.2.4.2.1 Plain statuses — initially FALSE, TRUE on a change

**Spec:** §2.2.4.2.1, p. 131 — "For the plain communication status, the
StatusChangedFlag flag is initially set to FALSE. It becomes TRUE whenever the
plain communication status changes".

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::update`,
`StatusLedger::unread`); `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`:
`set_status_bits`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`

**Status:** done

### §2.2.4.2.1 Plain statuses — FALSE after get_<status>

**Spec:** §2.2.4.2.1, p. 131 — "it is reset to FALSE each time the application
accesses the plain communication status via the proper get_<plain communication
status> operation on the Entity."

**Repo:** `crates/dcps/src/status.rs` (`StatusLedger::take`); the
`get_*_status` operations (`clear_status_changes`)

**Tests:** `crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/src/status.rs::tests::a_read_takes_the_changes_and_keeps_the_total`

**Status:** done

### §2.2.4.2.1 Plain statuses — FALSE when the listener is called

**Spec:** §2.2.4.2.1, p. 131 — "The communication status is also reset to FALSE
whenever the associated listener operation is called as the listener implicitly
accesses the status which is passed as a parameter to the operation."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`: `acknowledge`,
`clear_status_changes` after the call)

**Tests:** `crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`,
`crates/dcps/src/status.rs::tests::a_read_between_the_look_and_the_acknowledgement_is_not_taken_off_twice`

**Status:** done

### §2.2.4.2.1 Plain statuses — reset before the listener is called

**Spec:** §2.2.4.2.1, p. 131 — "The fact that the status is reset prior to
calling the listener means that if the application calls the get_<plain
communication status> from inside the listener it will see the status already
reset."

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`poll_ledger`,
`WriterListenerChain::takes`, `ReaderListenerChain::takes`)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::the_status_is_read_before_the_listener_runs`

**Status:** `done` — when a listener takes the status, it is acknowledged and
its flag lowered before the call; a `get_*_status` inside the listener sees it
reset.

### §2.2.4.2.1 Plain statuses — the nil listener does not reset

**Spec:** §2.2.4.2.1, p. 131 — "An exception to this rule is when the associated
listener is the ‘nil’ listener. As described in 2.2.4.3.1 the ‘nil’ listener is
treaded as a NOOP and the act of calling the ‘nil’ listener does not reset the
communication status."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` with a non-empty mask installs `NilListener`: the statuses of the mask are handled by doing nothing, do not bubble up (for DATA_ON_READERS no `on_data_available` follows) and are not reset. `get_listener` returns none for it. An empty mask is no listener. The C ABI (polled listeners) takes NULL with a mask as the nil listener too: `zerodds_poll_listeners` leaves out of the publisher, subscriber and participant levels the statuses a nil listener of the writer, reader, subscriber or topic below has in its mask (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.4.2.1 Example REQUESTED_DEADLINE_MISSED

**Spec:** §2.2.4.2.1, pp. 131–132 — "For example, the value of the
StatusChangedFlag associated with the REQUESTED_DEADLINE_MISSED status will
become TRUE each time new deadline occurs … The value changes to FALSE when the
application accesses the status via the corresponding
get_requested_deadline_missed_status method".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example of the rule above.

### §2.2.4.2.2 Read statuses — initially FALSE

**Spec:** §2.2.4.2.2, p. 132 — "For the read communication status, the
StatusChangedFlag flag is initially set to FALSE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::install_data_available_probe`,
`Subscriber::new`: probe for DATA_ON_READERS)

**Tests:** `crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done

### §2.2.4.2.2 Read statuses — TRUE on new data

**Spec:** §2.2.4.2.2, p. 132 — "The StatusChangedFlag becomes TRUE when either a
data-sample arrives … The arrival of new data."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::install_data_available_probe`:
channel, inbox, NOT_READ samples in the cache)

**Tests:** `crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/data_available_live.rs::a_live_readers_listener_hears_samples_nobody_waited_for`

**Status:** done

### §2.2.4.2.2 Read statuses — TRUE on dispose

**Spec:** §2.2.4.2.2, p. 132 — "The arrival of the notification that an instance
has been disposed by: the DataWriter that owns it if OWNERSHIP QoS
kind=EXLUSIVE, or by any DataWriter if OWNERSHIP QoS kind=SHARED."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`:
marker without data, NOT_READ)

**Tests:** `crates/dcps/tests/sample_info_lifecycle.rs::reader_lifecycle_dispose_marker_yields_invalid_data_sample`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_disposed_sets_instance_state`,
`crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** done — under SHARED each dispose puts a NOT_READ marker into the cache, from which the probe reports DATA_AVAILABLE; under EXCLUSIVE a dispose by a writer that does not own the instance is not taken (§2.2.2.5.1.3).

### §2.2.4.2.2 Read statuses — TRUE on loss of liveliness of the only writer

**Spec:** §2.2.4.2.2, p. 132 — "The loss of liveliness of the DataWriter of an
instance for which there is no other DataWriter."

**Repo:** `crates/dcps/src/subscriber.rs`
(`DataReader::apply_unmatched_writers`, `ReaderNews`);
`crates/dcps/src/instance_tracker.rs` (`would_leave_writerless`)

**Tests:**
`crates/dcps/tests/liveliness_qos.rs::linux::an_instance_whose_writer_lost_its_liveliness_has_no_writers`,
`crates/dcps/tests/liveliness_qos.rs::linux::a_lost_writer_raises_data_available_before_any_read`,
`crates/dcps/tests/lifecycle_wire_e2e.rs::linux::a_writer_that_leaves_raises_data_available_before_any_read`

**Status:** `done` — the NOT_ALIVE_NO_WRITERS marker raises DATA_AVAILABLE
before the next `read`/`take`, also after the loss of the match.

### §2.2.4.2.2 Read statuses — TRUE on unregister by the only writer

**Spec:** §2.2.4.2.2, p. 132 — "The arrival of the notification that an instance
has been unregistered by the only DataWriter that is known to be writing the
instance."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::push_lifecycle_with_holder`);
`crates/dcps/src/instance_tracker.rs` (`InstanceTracker::unregister`)

**Tests:** `crates/dcps/src/runtime.rs::tests::qr_unregister_delivers_no_writers_lifecycle`,
`crates/dcps/tests/lifecycle_wire_decode.rs::reader_push_lifecycle_unregistered_sets_instance_state`

**Status:** done

### §2.2.4.2.2 DATA_AVAILABLE — FALSE after on_data_available or read/take

**Spec:** §2.2.4.2.2, p. 132 — "The DATA_AVAILABLE StatusChangedFlag becomes
FALSE when either the corresponding listener operation (on_data_available) is
called or the read or take operation (or their variants) is called on the
associated DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderNews`,
`ReaderStatus::note_read_or_take`, `poll_data_available`)

**Tests:**
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/status_reads.rs::a_read_of_one_instance_lowers_data_available`,
`crates/dcps/tests/status_reads.rs::an_on_data_available_call_lowers_data_available`

**Status:** `done` — a flag: a call of `on_data_available` or a `read`/`take` of
any variant lowers it, new data raises it.

### §2.2.4.2.2 DATA_AVAILABLE — listener call and variants of read/take

**Spec:** §2.2.4.2.2, p. 132 — "becomes FALSE when either the corresponding
listener operation (on_data_available) is called or the read or take operation
(or their variants) is called".

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderNews`,
`ReaderStatus::note_read_or_take`)

**Tests:**
`crates/dcps/tests/status_reads.rs::a_read_of_one_instance_lowers_data_available`,
`crates/dcps/tests/status_reads.rs::an_on_data_available_call_lowers_data_available`

**Status:** `done` — after `on_data_available` and after `read_instance` the
flag is FALSE, also while other instances have NOT_READ samples.

### §2.2.4.2.2 DATA_ON_READERS — FALSE after on_data_on_readers, on_data_available or read/take

**Spec:** §2.2.4.2.2, p. 132 — "The DATA_ON_READERS StatusChangedFlag becomes
FALSE when any of the following events occurs: The corresponding listener
operation (on_data_on_readers) is called. The on_data_available listener
operation is called on any DataReader belonging to the Subscriber. The read or
take operation (or their variants) is called on any DataReader belonging to the
Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`SubscriberInner::data_on_readers`,
`cover_data_on_readers`)

**Tests:**
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`,
`crates/dcps/tests/status_reads.rs::a_take_on_one_reader_lowers_data_on_readers`

**Status:** done — the three events reset the flag. The GROUP release after a listener call is its own item below.

### §2.2.4.2.2 DATA_ON_READERS — GROUP release after a listener call

**Spec:** §2.2.4.2.2, p. 132 — "The DATA_ON_READERS StatusChangedFlag becomes
FALSE when any of the following events occurs: … The on_data_available listener
operation is called on any DataReader belonging to the Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs` (`NewsMarks::cover_held`, `NewsMarks::released`,
`ReaderInner::accept_group_released`)

**Tests:** `crates/dcps/src/subscriber.rs::tests::a_group_release_after_a_listener_call_is_no_news_for_what_it_covered`

**Status:** `done` — a listener call remembers how many samples the Subscriber's group order held for the reader; their release into the inbox counts as covered, and a sample it did not cover raises both flags. The test checks this on the marks of an offline reader; the path through a running GROUP publisher's order is not measured.

### §2.2.4.2.2 DATA_ON_READERS — reset by listeners and other readers

**Spec:** §2.2.4.2.2, p. 132 — "The read or take operation (or their variants) is
called on any DataReader belonging to the Subscriber."

**Repo:** `crates/dcps/src/subscriber.rs`
(`SubscriberInner::cover_data_on_readers`)

**Tests:**
`crates/dcps/tests/status_reads.rs::a_take_on_one_reader_lowers_data_on_readers`

**Status:** `done` — `on_data_on_readers`, `on_data_available` and `read`/`take`
on one reader of the subscriber lower it, also while another reader holds
NOT_READ samples.

### §2.2.4.3 A listener per kind of entity

**Spec:** §2.2.4.3, p. 133 — "All Entity support a listener, which type of which
is specialized to the specific type of the related Entity (e.g.,
DataReaderListener for the DataReader). Listeners are interfaces that the
application must implement." Figure 2.17.

**Repo:** `crates/dcps/src/listener.rs` (`TopicListener`, `DataWriterListener`,
`PublisherListener`, `DataReaderListener`, `SubscriberListener`,
`DomainParticipantListener`)

**Tests:** `crates/dcps/src/listener.rs::tests::datareader_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::all_listener_traits_default_methods_invoke_safely`

**Status:** done — the methods have empty default bodies; an application
implements only what it needs.

### §2.2.4.3 Listeners stateless, the parameter refers to the entity

**Spec:** §2.2.4.3, p. 134 — "Listeners are stateless. It is thus possible to
share the same DataReaderListener instance among all the DataReader objects …
Consequently, the provided parameter contains a reference to the actual
concerned Entity."

**Repo:** `crates/dcps/src/listener.rs` (`*_entity`), `crates/dcps/src/listener_dispatch.rs`

**Tests:** `crates/dcps/tests/listener_entity.rs::a_shared_listener_is_called_with_the_entity`, `crates/dcps/tests/listener_entity.rs::the_bubbled_up_call_gets_the_entity`, `crates/dcps/tests/listener_entity.rs::a_listener_of_the_handle_form_is_still_called`

**Status:** done — each operation of the six listener traits has an entity form (`on_data_available_entity(&AnyDataReader)` and the others); the dispatcher calls it with the entity whose handle it has, and its default calls the handle form, so existing listeners are called as before. A shared listener reaches the reader through the entity (`typed::<T>()`).

### §2.2.4.3.1 on_<status> with the status as parameter

**Spec:** §2.2.4.3.1, p. 134 — "For each communication status, there is a
corresponding operation whose name is on_<communication_status>, which takes a
parameter of type <communication_status> as listed in 2.2.4.1".

**Repo:** `crates/dcps/src/listener.rs`

**Tests:** `crates/dcps/tests/listener_trigger_c22c.rs::liveliness_changed_dispatcher_fires_listener`,
`crates/dcps/tests/listener_trigger_c22c.rs::requested_incompatible_qos_dispatcher_fires_listener`,
`crates/dcps/tests/status_reads.rs::a_listener_takes_the_changes_it_was_handed`

**Status:** done

### §2.2.4.3.1 on_<status> also on the enclosing entities

**Spec:** §2.2.4.3.1, pp. 134–135 — "on_<communication_status> is available on
the relevant Entity as well as those that embed it, as expressed in the following
figure" (Figure 2.18: Domain; Publisher, Topic, Subscriber; DataWriter,
DataReader).

**Repo:** `crates/dcps/src/listener.rs` (`PublisherListener` with the
DataWriter callbacks, `SubscriberListener` with the DataReader callbacks,
`DomainParticipantListener` with all of them)

**Tests:** `crates/dcps/src/listener.rs::tests::publisher_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::subscriber_listener_is_object_safe`,
`crates/dcps/src/listener.rs::tests::participant_listener_is_object_safe`

**Status:** done

### §2.2.4.3.1 Mask when attaching

**Spec:** §2.2.4.3.1, p. 135 — "When the application attaches a listener on an
entity, it must set a mask that indicates to the middleware which operations are
enabled within this listener (see operation Entity::set_listener)."

**Repo:** `set_listener(listener, mask)` per entity

**Tests:** `crates/dcps/tests/listener_integration.rs::push_raw_with_only_data_available_bit_does_not_call_reader_for_others`,
`crates/dcps/src/listener_dispatch.rs::tests::writer_with_only_publication_matched_bit_passes_other_events_up`

**Status:** done

### §2.2.4.3.1 The most specific enabled listener

**Spec:** §2.2.4.3.1, p. 135 — "When a plain communication status changes, the
middleware triggers the most ‘specific’ relevant listener operation that is
enabled." Footnote 30: DataWriter, otherwise Publisher, otherwise
DomainParticipant.

**Repo:** `crates/dcps/src/listener_dispatch.rs` (`dispatch_*`,
`WriterListenerChain`, `ReaderListenerChain`, `TopicListenerChain`)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::publication_matched_consumed_at_writer`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_publisher`,
`crates/dcps/src/listener_dispatch.rs::tests::publication_matched_bubbles_to_participant`,
`crates/dcps/src/listener_dispatch.rs::tests::sample_lost_bubbles_to_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::inconsistent_topic_bubbles_to_participant`

**Status:** done

### §2.2.4.3.1 The nil listener as NO-OP

**Spec:** §2.2.4.3.1, p. 135 — "In case the most specific relevant listener
operation corresponds to an application-installed ‘nil’ listener the operation
will be considered handled by a NO-OP operation."

**Repo:** `crates/dcps/src/listener.rs` (`NilListener`, `is_nil`); `crates/dcps/src/listener_dispatch.rs` (`nil_takes`); `set_listener` of the six entities

**Tests:** `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_a_mask_handles_the_status_by_doing_nothing`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_with_an_empty_mask_is_no_listener`, `crates/dcps/tests/listener_integration.rs::a_nil_listener_for_data_on_readers_leaves_on_data_available_uncalled`, `crates/dcps/tests/nil_listener.rs::a_nil_listener_does_not_pass_the_status_up_and_does_not_reset_it`

**Status:** done — `set_listener(None, mask)` with a non-empty mask installs `NilListener`: the statuses of the mask are handled by doing nothing, do not bubble up (for DATA_ON_READERS no `on_data_available` follows) and are not reset. `get_listener` returns none for it. An empty mask is no listener. The C ABI (polled listeners) takes NULL with a mask as the nil listener too: `zerodds_poll_listeners` leaves out of the publisher, subscriber and participant levels the statuses a nil listener of the writer, reader, subscriber or topic below has in its mask (`crates/zerodds-c-api/src/listener_ffi.rs::tests::poll_a_nil_listener_stops_the_level_above`).

### §2.2.4.3.1 Default behaviour at the DomainParticipant

**Spec:** §2.2.4.3.1, p. 135 — "This behavior allows the application to set a
default behavior (e.g., in the listener associated with the DomainParticipant)
and setting dedicated behaviors only where needed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale of the rule above.

### §2.2.4.3.2 Read statuses — on_data_on_readers first

**Spec:** §2.2.4.3.2, p. 135 — "first, the middleware tries to trigger the
SubscriberListener operation on_data_on_readers with a parameter of the related
Subscriber; if this does not succeed (no listener or operation non-enabled), it
tries to trigger on_data_available on all the related DataReaderListener
objects, with as parameter the related DataReader."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::poll_data_available`);
`crates/dcps/src/listener_dispatch.rs` (`dispatch_data_on_readers`,
`dispatch_data_available`)

**Tests:** `crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`,
`crates/dcps/tests/listener_integration.rs::push_raw_fires_on_data_on_readers_at_subscriber`,
`crates/dcps/src/listener_dispatch.rs::tests::data_on_readers_does_not_call_reader_stage`

**Status:** done — `on_data_on_readers` passes from the Subscriber on to the
DomainParticipant, `on_data_available` from the reader to the Subscriber and the
DomainParticipant (`data_on_readers_bubbles_to_participant`,
`data_available_bubbles_all_the_way_to_participant`). The specification names
only SubscriberListener and DataReaderListener here; the contradiction is
recorded in the findings file.

### §2.2.4.3.2 No on_data_available after on_data_on_readers, notify_datareaders

**Spec:** §2.2.4.3.2, p. 136 — "Note that if on_data_on_readers is called, then
the middleware will not try to call on_data_available, however, the application
can force a call to the DataReader objects that have data by means of the
notify_datareaders operation."

**Repo:** `crates/dcps/src/subscriber.rs` (`ReaderStatus::poll_data_available`,
`ReaderStatus::notify_data_available`, `data_available_pending`)

**Tests:** `crates/dcps/tests/listener_integration.rs::on_data_on_readers_takes_the_arrival_from_on_data_available`,
`crates/dcps/tests/entity_deletion.rs::notify_datareaders_calls_the_readers_listeners_once_per_change`

**Status:** done

### §2.2.4.3.2 Rationale of the two ways

**Spec:** §2.2.4.3.2, p. 136 — "The rationale is that either the application is
interested in relations among data arrivals and it must use the first option …
or it wants to treat each DataReader fully independently and it may choose the
second option".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale.

### §2.2.4.3.2 No event queue for listeners

**Spec:** §2.2.4.3.2, p. 136 — "There is no implied “event queuing” in the
invocation of the listeners in the sense that, if several changes of status of
the same kind occur in sequence, it is not necessary that the DCPS
implementation delivers one listener callback per “unit” change … as long as the
LivelinessChangedStatus provided to the listener corresponds to the most current
one."

**Repo:** `crates/dcps/src/listener_dispatcher.rs` (one pass for a sequence of
changes)

**Tests:** `crates/dcps/src/listener_dispatch.rs::tests::a_look_asked_for_during_a_look_runs_after_it_not_inside_it`,
`crates/dcps/src/listener_dispatch.rs::tests::a_run_of_changes_is_handed_over_once_with_the_most_current_status`

**Status:** done — a run of four changes reaches the listener in one call with the most current LivelinessChangedStatus; a change after it is the next call with its own change counts.

### §2.2.4.4 Use pattern with conditions and wait-sets

**Spec:** §2.2.4.4, pp. 136–138 — "The application indicates which relevant
information it wants to get, by creating Condition objects … and attaching them
to a WaitSet. It then waits on that WaitSet until the trigger_value of one or
several Condition objects become TRUE. It then uses the result of the wait …
Usually the first step is done in an initialization phase".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — use pattern; the operations are listed under
§2.2.2.1.6 to §2.2.2.1.9 and §2.2.2.5.8 to §2.2.2.5.9.

### §2.2.4.4 Conditions of an entity belong to exactly one entity

**Spec:** §2.2.4.4, p. 138 — "Condition objects are meant to embed all that is
needed to react properly when enabled. In particular, Entity-related conditions
are related to exactly one Entity and cannot be shared."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition`: one `EntityState`);
`crates/dcps/src/condition.rs` (`ReadCondition`: one reader)

**Tests:** `crates/dcps/tests/reader_conditions.rs::delete_readcondition_takes_only_the_readers_own_conditions`,
`crates/dcps/src/entity.rs::tests::status_condition_trigger_value`

**Status:** done

### §2.2.4.4 WaitSet — BLOCKED and UNBLOCKED

**Spec:** §2.2.4.4, p. 138 — "If the wait operation is called on WaitSet with
state BLOCKED, it will block the calling thread. If wait is called on a WaitSet
with state UNBLOCKED, it will return immediately. In addition, when the WaitSet
transitions from BLOCKED to UNBLOCKED it wakes up any threads that had called
wait on it." Figure 2.20.

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::wait`, `WaitSet::poll_active`,
`signal_data_available`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_wait_returns_immediately_if_already_triggered`,
`crates/dcps/src/condition.rs::tests::waitset_wakes_when_guard_triggers`,
`crates/dcps/src/condition.rs::tests::data_signal_wakes_a_blocked_wait`,
`crates/dcps/src/condition.rs::tests::a_signal_between_check_and_block_is_not_lost`

**Status:** done — data arrival wakes at once; a GuardCondition or
StatusCondition is seen after at most `poll_interval` (1 ms) (see §2.2.2.1.8.1).
At most one thread waits (§2.2.2.1.6.3).

### §2.2.4.4 No event queue for wait-sets

**Spec:** §2.2.4.4, p. 138 — "there is no implied “event queuing” in the
awakening of a WaitSet in the sense that, if several Conditions attached to the
WaitSet have their trigger_value transition to TRUE in sequence the DCPS
implementation needs to only unblock the WaitSet once."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission.

### §2.2.4.4.1 trigger_value of the StatusCondition

**Spec:** §2.2.4.4.1, p. 139 — "The trigger_value of a StatusCondition is the
Boolean OR of the ChangedStatusFlag of all the communication statuses to which it
is sensitive. That is, trigger_value==FALSE only if all the values of the
ChangedStatusFlags are FALSE. The sensitivity … is controlled by the list of
enabled_statuses set on the condition by means of the set_enabled_statuses
operation."

**Repo:** `crates/dcps/src/entity.rs` (`StatusCondition::trigger_value`:
`status_changes() & enabled_statuses`)

**Tests:** `crates/dcps/src/entity.rs::tests::status_condition_trigger_value`,
`crates/dcps/tests/status_reads.rs::a_status_change_raises_the_flag_and_the_read_lowers_it`,
`crates/dcps/tests/status_reads.rs::data_available_raises_the_readers_and_its_subscribers_flags`

**Status:** done — for DATA_AVAILABLE and DATA_ON_READERS the deviations under
§2.2.4.2.2 apply.

### §2.2.4.5 trigger_value of the ReadCondition

**Spec:** §2.2.4.5, p. 139 — "the trigger_value of the ReadCondition is tied to
the presence of at least a sample managed by the Service with SampleState,
ViewState, and InstanceState matching those of the ReadCondition. Furthermore,
for the QueryCondition to have a trigger_value==TRUE, the data associated with
the sample must be such that the query_expression evaluates to TRUE."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`,
`DataReader::create_querycondition`: trigger over the samples in the cache)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`,
`crates/dcps/tests/reader_conditions.rs::a_query_condition_triggers_for_the_samples_its_expression_holds_for`,
`crates/dcps/tests/reader_conditions.rs::a_waitset_returns_a_read_condition_of_a_reader_with_data`

**Status:** done — ViewState and InstanceState are the values recorded at
admission (see §2.2.2.5.1).

### §2.2.4.5 One take changes several conditions

**Spec:** §2.2.4.5, p. 139 — "a single take operation can potentially change the
trigger_value of several ReadCondition or QueryCondition conditions. For
example, if all samples are taken, any ReadCondition and QueryCondition
conditions associated with the DataReader that had their trigger_value==TRUE
before will see the trigger_value change to FALSE."

**Repo:** `crates/dcps/src/subscriber.rs` (the triggers read the cache on every
query)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`

**Status:** done

### §2.2.4.5 No taking back of a wake-up

**Spec:** §2.2.4.5, p. 139 — "Once we have trigger_value==TRUE on a condition it
may wake up the attached WaitSet, the condition transitioning to
trigger_value==FALSE does not necessarily ‘unwakeup’ the WaitSet … an
application blocked on a WaitSet may return from the wait with a list of
conditions some of which are no longer “active.”"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission.

### §2.2.4.5 Example sample_state_mask {NOT_READ} and {READ, NOT_READ}

**Spec:** §2.2.4.5, p. 139 — "A ReadCondition that has a sample_state_mask =
{NOT_READ} will have trigger_value of TRUE whenever a new sample arrives and will
transition to FALSE as soon as all the newly-arrived samples are either read …
or taken … However if the same ReadCondition had a sample_state_mask = {READ,
NOT_READ}, then the trigger_value would only become FALSE once all the
newly-arrived samples are taken".

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::create_readcondition`)

**Tests:** `crates/dcps/tests/reader_conditions.rs::a_read_condition_follows_the_states_of_the_readers_samples`

**Status:** done

### §2.2.4.5.1 trigger_value of the GuardCondition

**Spec:** §2.2.4.5.1, p. 139 — "The trigger_value of a GuardCondition is
completely controlled by the application via operation set_trigger_value."

**Repo:** `crates/dcps/src/condition.rs` (`GuardCondition::set_trigger_value`)

**Tests:** `crates/dcps/src/condition.rs::tests::guard_condition_set_trigger`

**Status:** done

### §2.2.4.6 Combination — listener first, then the WaitSet

**Spec:** §2.2.4.6, p. 139 — "Those two mechanisms may be combined in the
application … if both mechanisms are enabled, then the listener mechanism is used
first and then the WaitSet objects are signalled."

**Repo:** `crates/dcps/src/subscriber.rs` (`DataReader::notify_data_arrived`);
`crates/dcps/src/runtime.rs` (`signal_reader_data`)

**Tests:**
`crates/dcps/tests/waitset_notification.rs::a_listener_and_a_waitset_both_see_the_same_arrival`,
`crates/dcps/tests/listener_before_waitset.rs::the_listener_runs_before_the_waitset_is_signalled`

**Status:** `done` — on both paths the listener runs first. For a plain status
the listener call lowers the flag (§2.2.4.2.1); a StatusCondition on that status
does not trigger afterwards. The contradiction is kept in the findings file.

### §2.2.4.6 Combination — order on the receive path

**Spec:** §2.2.4.6, p. 139 — "the listener mechanism is used first and then the
WaitSet objects are signalled."

**Repo:** `crates/dcps/src/runtime.rs::signal_reader_data`

**Tests:**
`crates/dcps/tests/listener_before_waitset.rs::the_listener_runs_before_the_waitset_is_signalled`

**Status:** `done` — `signal_reader_data` calls the listener first and wakes the
WaitSets afterwards.

---

## §2.2.5 Built-in Topics

### §2.2.5 Purpose of the built-in topics

**Spec:** §2.2.5, p. 140 — "the DCPS specification introduces a set of built-in
topics and corresponding DataReader objects that can then be used by the
application. The information is then accessed as if it was normal application
data. This approach avoids introducing a new API to access this information".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale; the requirements are in the
following items.

### §2.2.5 Built-in readers in one built-in Subscriber

**Spec:** §2.2.5, p. 140 — "The built-in data-readers all belong to a built-in
Subscriber. This subscriber can be retrieved by using the method
get_builtin_subscriber provided by the DomainParticipant."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::new`:
the four readers); `crates/dcps/src/participant.rs`
(`DomainParticipant::get_builtin_subscriber`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::get_builtin_subscriber_returns_subscriber`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/src/builtin_subscriber.rs::tests::builtin_subscriber_has_four_readers`

**Status:** done — one built-in subscriber per DomainParticipant, both calls
return the same object.

### §2.2.5 The built-in Subscriber as a Subscriber

**Spec:** §2.2.5, p. 140 — "The built-in data-readers all belong to a built-in
Subscriber."; §2.2.2.2.1.13, p. 39 — "This operation allows access to the
built-in Subscriber."

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::the_builtin_subscriber_is_a_subscriber`

**Status:** done — `BuiltinSubscriber` derefs to its `Subscriber`: QoS, listener, StatusCondition and the other operations are its own, its readers are the four built-in ones (`lookup_datareader`, `get_datareaders`), it is enabled, and the application cannot delete them (`PreconditionNotMet`).

### §2.2.5 lookup_datareader with the topic name

**Spec:** §2.2.5, p. 140 — "The built-in DataReader objects can be retrieved by
using the operation lookup_datareader, with the Subscriber and the topic name as
parameters."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::lookup_datareader`,
`BuiltinTopic`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::lookup_datareader_routes_by_type`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_subscriber_has_all_four_readers_pre_created`,
`crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — the four names give their reader; an unknown name, or a type that is not that of the topic, gives nil (`None`).

### §2.2.5 lookup_datareader — unknown name

**Spec:** §2.2.2.5.2.7, p. 80 — lookup_datareader: "If no such DataReader exists,
the operation will return ’nil.’"

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::lookup_with_wrong_type_for_topic_is_nil`

**Status:** done — nil (`None`) instead of `BadParameter`.

### §2.2.5 QoS of the built-in entities — values not specified

**Spec:** §2.2.5, p. 140 — USER_DATA, TOPIC_DATA, GROUP_DATA, PARTITION
"<unspecified>"; LATENCY_BUDGET "duration = <unspecified>"; LIVELINESS
"lease_duration = <unspecified>".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §2.2.5 QoS of the built-in readers — DURABILITY TRANSIENT_LOCAL

**Spec:** §2.2.5, p. 140 — "DURABILITY TRANSIENT_LOCAL".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS of the built-in readers — DURABILITY_SERVICE

**Spec:** §2.2.5, p. 140 — "DURABILITY_SERVICE Does not apply as DURABILITY is
TRANSIENT_LOCAL".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no requirement.

### §2.2.5 QoS of the built-in Subscriber — PRESENTATION

**Spec:** §2.2.5, p. 140 — "PRESENTATION access_scope = TOPIC coherent_access =
FALSE ordered_access = FALSE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_subscriber_qos`)

**Tests:**
`crates/dcps/src/builtin_subscriber.rs::tests::the_builtin_subscriber_presents_by_topic`

**Status:** `done` — `access_scope` TOPIC, `coherent_access` and
`ordered_access` FALSE.

### §2.2.5 QoS of the built-in readers — DEADLINE infinite

**Spec:** §2.2.5, p. 140 — "DEADLINE Period = infinite".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — OWNERSHIP SHARED

**Spec:** §2.2.5, p. 140 — "OWNERSHIP SHARED".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — LIVELINESS AUTOMATIC

**Spec:** §2.2.5, p. 140 — "LIVELINESS kind = AUTOMATIC".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — TIME_BASED_FILTER 0

**Spec:** §2.2.5, p. 140 — "TIME_BASED_FILTER minimum_separation = 0".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — RELIABILITY RELIABLE

**Spec:** §2.2.5, p. 140 — "RELIABILITY kind = RELIABLE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS of the built-in readers — max_blocking_time 100 ms

**Spec:** §2.2.5, p. 140 — "RELIABILITY … max_blocking_time = 100 milliseconds".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — DESTINATION_ORDER BY_RECEPTION_TIMESTAMP

**Spec:** §2.2.5, p. 140 — "DESTINATION_ORDER BY_RECEPTION_TIMESTAMP".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — HISTORY KEEP_LAST 1

**Spec:** §2.2.5, p. 140 — "HISTORY kind = KEEP_LAST depth = 1".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::builtin_reader_qos_is_spec_default`,
`crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done

### §2.2.5 QoS of the built-in readers — RESOURCE_LIMITS unlimited

**Spec:** §2.2.5, p. 140 — "RESOURCE_LIMITS All LENGTH_UNLIMITED."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in readers — READER_DATA_LIFECYCLE infinite

**Spec:** §2.2.5, p. 140 — "READER_DATA_LIFECYCLE
autopurge_nowriter_samples_delay = infinite autopurge_disposed_samples_delay =
infinite".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 QoS of the built-in Subscriber — ENTITY_FACTORY TRUE

**Spec:** §2.2.5, p. 140 — "ENTITY_FACTORY autoenable_created_entities = TRUE".

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`builtin_reader_qos`, `builtin_subscriber_qos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_entities_have_the_qos_of_the_table`

**Status:** done — the value is asserted on all four built-in readers (resp. on the built-in Subscriber).

### §2.2.5 Listeners of the built-in Subscriber and the built-in topics

**Spec:** §2.2.5, p. 141 — "The built-in Subscriber and all of its built-in
Topics have nil listeners with all statuses appearing in their listener masks."

**Repo:** `crates/dcps/src/builtin_subscriber.rs` (`BuiltinSubscriber::new`); `crates/dcps/src/participant.rs` (`attach_participant` for the built-in Subscriber)

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::a_builtin_status_does_not_reach_the_participant_listener`

**Status:** done — `BuiltinSubscriber::new` sets the nil listener with all statuses on the built-in Subscriber and attaches it to its participant; a status of a built-in reader ends there and does not reach the participant listener. The built-in topics are standalone objects without a participant chain.

### §2.2.5 Listeners of the built-in readers

**Spec:** §2.2.5, p. 141 — "The built-in DataReaders have nil listeners with no
statuses in their masks."

**Repo:** `crates/dcps/src/builtin_subscriber.rs`

**Tests:** `crates/dcps/src/builtin_subscriber.rs::tests::a_builtin_status_does_not_reach_the_participant_listener`

**Status:** done — the built-in readers have no listener (a nil listener with an empty mask is none); `get_listener` returns none.

### §2.2.5 QoS of the remote entity as data fields

**Spec:** §2.2.5, p. 141 — "The information that is accessible about the remote
entities by means of the built-in topics includes all the QoS policies that
apply to the corresponding remote Entity. This QoS policies appear as normal
‘data’ fields inside the data read by means of the built-in Topic."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the fields are listed per built-in topic
below.

### §2.2.5 No data about entities of the own DomainParticipant

**Spec:** §2.2.5, p. 141 — "A built-in DataReader obtained from a given
Participant will not provide data pertaining to Entities created from that same
Participant under the assumption that such entities are already known to the
application that created them."

**Repo:** `crates/dcps/src/runtime.rs` (`handle_spdp_datagram`: own beacon
discarded)

**Tests:** `crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_skips_self_beacon`,
`crates/dcps/tests/builtin_no_own_data.rs::the_own_entities_are_in_no_built_in_reader`

**Status:** done — neither the participant, nor its topics, writers and readers appear in its own built-in readers.

### §2.2.5 No data about own Topics, DataWriters and DataReaders

**Spec:** §2.2.5, p. 141 — "will not provide data pertaining to Entities created
from that same Participant".

**Repo:** —

**Tests:** `crates/dcps/tests/builtin_no_own_data.rs::the_own_entities_are_in_no_built_in_reader`

**Status:** done — see the item above.

### §2.2.5 DCPSParticipant — key

**Spec:** §2.2.5, p. 141 — DCPSParticipant "(entry created when a
DomainParticipant object is created)": "key BuiltinTopicKey_t DCPS key to
distinguish entries."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`ParticipantBuiltinTopicData::key`,
`ParticipantBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::participant_from_wire_strips_to_guid`,
`crates/dcps/src/runtime.rs::tests::handle_spdp_datagram_pushes_into_builtin_participant_reader`

**Status:** done — the key is the GUID of the participant.

### §2.2.5 DCPSParticipant — user_data

**Spec:** §2.2.5, p. 141 — "user_data UserDataQosPolicy Policy of the
corresponding DomainParticipant."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::participant_sample_carries_user_data`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSParticipant — instance_handle in the SampleInfo

**Spec:** §2.2.2.2.1.14, p. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples available for the built-in
DataReader to the “DCPSParticipant” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`), `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_participant_with_the_builtin_handle_unmatches`

**Status:** `done` — the built-in readers assign `InstanceHandle::from_guid` of
the key instead of a counter.

### §2.2.5 DCPSTopic — key

**Spec:** §2.2.5, p. 141 — DCPSTopic "(entry created when a Topic object is
created)": "key BuiltinTopicKey_t DCPS key to distinguish entries."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::synthesize_key`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_synthesize_key_is_stable_and_distinct`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::two_participants_creating_one_topic_are_two_dcps_topic_entries`

**Status:** done — the key is formed from the participant, the topic name and the type name and distinguishes the entries; two participants that create the same topic give two entries.

### §2.2.5 DCPSTopic — entry per Topic object and values of the Topic

**Spec:** §2.2.5, p. 141 — "(entry created when a Topic object is created)";
the policies are each "Policy of the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::synthesize_key`), `crates/dcps/src/runtime.rs` (`push_sedp_events_to_builtin_readers`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::two_participants_creating_one_topic_are_two_dcps_topic_entries`, `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — one entry per participant, topic and type; the announcement over the SEDP topics writer carries all policies of the topic. When a participant announces no topic over SEDP (a peer of another vendor), the entry is derived from its endpoints (`from_publication`, `from_subscription`): the policies are then those of the DataWriter or DataReader, HISTORY, RESOURCE_LIMITS and TRANSPORT_PRIORITY have their default because no endpoint announces them (DDSI-RTPS 2.5 Tables 9.18, 9.19).

### §2.2.5 DCPSTopic — name

**Spec:** §2.2.5, p. 141 — "name string Name of the Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::name`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`,
`crates/dcps/src/builtin_topics.rs::tests::topic_roundtrip`

**Status:** done

### §2.2.5 DCPSTopic — type_name

**Spec:** §2.2.5, p. 141 — "type_name string Name of the type attached to the
Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — durability

**Spec:** §2.2.5, p. 141 — "durability DurabilityQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — durability_service

**Spec:** §2.2.5, p. 141 — "durability_service DurabilityServiceQosPolicy Policy
of the corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — deadline

**Spec:** §2.2.5, p. 141 — "deadline DeadlineQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — latency_budget

**Spec:** §2.2.5, p. 141 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — liveliness

**Spec:** §2.2.5, p. 141 — "liveliness LivelinessQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — reliability

**Spec:** §2.2.5, p. 141 — "reliability ReliabilityQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — transport_priority

**Spec:** §2.2.5, p. 141 — "transport_priority TransportPriorityQosPolicy Policy
of the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TopicBuiltinTopicData::transport_priority`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — for an entry derived from endpoints see the `open` item
“entry per Topic object and values of the Topic”.

### §2.2.5 DCPSTopic — lifespan

**Spec:** §2.2.5, p. 141 — "lifespan LifespanQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — destination_order

**Spec:** §2.2.5, p. 141 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — history

**Spec:** §2.2.5, p. 141 — "history HistoryQosPolicy Policy of the corresponding
Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs` (`TopicBuiltinTopicData::history`,
`TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — for an entry derived from endpoints see the `open` item
“entry per Topic object and values of the Topic”.

### §2.2.5 DCPSTopic — resource_limits

**Spec:** §2.2.5, p. 141 — "resource_limits ResourceLimitsQosPolicy Policy of
the corresponding Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TopicBuiltinTopicData::resource_limits`, `TopicBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::a_created_topic_reaches_dcps_topic_with_every_member_and_its_deletion_disposes_it`

**Status:** done — for an entry derived from endpoints see the `open` item
“entry per Topic object and values of the Topic”.

### §2.2.5 DCPSTopic — ownership

**Spec:** §2.2.5, p. 141 — "ownership OwnershipQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — topic_data

**Spec:** §2.2.5, p. 141 — "topic_data TopicDataQosPolicy Policy of the
corresponding Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::topic_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSTopic — instance_handle in the SampleInfo

**Spec:** §2.2.2.2.1.15, p. 39 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSTopic” topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_topic_unmatches_the_endpoints_of_the_topic`

**Status:** `done` — the handle is `InstanceHandle::from_guid` of the synthetic
key; `ignore_topic` with the handle from the SampleInfo unmatches the endpoints
of the topic.

### §2.2.5 DCPSPublication — key

**Spec:** §2.2.5, p. 142 — "key BuiltinTopicKey_t DCPS key to distinguish
entries."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::key`, `PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done — the key is the GUID of the DataWriter.

### §2.2.5 DCPSPublication — participant_key

**Spec:** §2.2.5, p. 142 — "participant_key BuiltinTopicKey_t DCPS key of the
participant to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::participant_key`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — topic_name

**Spec:** §2.2.5, p. 142 — "topic_name string Name of the related Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::topic_name`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`,
`crates/dcps/tests/builtin_topic_readers.rs::linux_e2e::sedp_pub_sub_populates_dcps_publication_and_subscription_readers`

**Status:** done

### §2.2.5 DCPSPublication — type_name

**Spec:** §2.2.5, p. 142 — "type_name string Name of the type attached to the
related Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::type_name`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — durability

**Spec:** §2.2.5, p. 142 — "durability DurabilityQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::durability`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — durability_service

**Spec:** §2.2.5, p. 142 — "durability_service DurabilityServiceQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — deadline

**Spec:** §2.2.5, p. 142 — "deadline DeadlineQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::deadline`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — latency_budget

**Spec:** §2.2.5, p. 142 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::latency_budget`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — liveliness

**Spec:** §2.2.5, p. 142 — "liveliness LivelinessQosPolicy Policy of the
corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — reliability

**Spec:** §2.2.5, p. 142 — "reliability ReliabilityQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::reliability`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — lifespan

**Spec:** §2.2.5, p. 142 — "lifespan LifespanQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::lifespan`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — user_data

**Spec:** §2.2.5, p. 142 — "user_data UserDataQosPolicy Policy of the
corresponding DataWriter."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::user_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`,
`crates/dcps/tests/set_qos_after_enable.rs::a_user_data_change_reaches_the_peers_builtin_reader`

**Status:** done

### §2.2.5 DCPSPublication — ownership

**Spec:** §2.2.5, p. 142 — "ownership OwnershipQosPolicy Policy of the
corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — ownership_strength

**Spec:** §2.2.5, p. 142 — "ownership_strength OwnershipStrengthQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — destination_order

**Spec:** §2.2.5, p. 142 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding DataWriter."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — presentation

**Spec:** §2.2.5, p. 142 — "presentation PresentationQosPolicy Policy of the
Publisher to which the DataWriter belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_sample_carries_ownership_liveliness_and_presentation`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSPublication — partition

**Spec:** §2.2.5, p. 142 — "partition PartitionQosPolicy Policy of the Publisher
to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::partition`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — topic_data

**Spec:** §2.2.5, p. 142 — "topic_data TopicDataQosPolicy Policy of the related
Topic."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::topic_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done — a DataWriter of ZeroDDS announces its own value if set,
otherwise that of the Topic (§2.2.3.2).

### §2.2.5 DCPSPublication — group_data

**Spec:** §2.2.5, p. 142 — "group_data GroupDataQosPolicy Policy of the
Publisher to which the DataWriter belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicData::group_data`,
`PublicationBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::publication_from_wire_carries_every_policy`

**Status:** done

### §2.2.5 DCPSPublication — instance_handle in the SampleInfo

**Spec:** §2.2.2.2.1.16, p. 40 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSPublication” topic."; §2.2.2.5.3.34, p. 97 — "These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSPublications” builtin topic".

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_publication_with_the_builtin_handle_unmatches`

**Status:** `done` — the handle is the one of
`DataReader::matched_publications`.

### §2.2.5 DCPSSubscription — key

**Spec:** §2.2.5, p. 143 — "key BuiltinTopicKey_t DCPS key to distinguish
entries."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::key`, `SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done — the key is the GUID of the DataReader.

### §2.2.5 DCPSSubscription — participant_key

**Spec:** §2.2.5, p. 143 — "participant_key BuiltinTopicKey_t DCPS key of the
participant to which the DataReader belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::participant_key`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — topic_name

**Spec:** §2.2.5, p. 143 — "topic_name string Name of the related Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — type_name

**Spec:** §2.2.5, p. 143 — "type_name string Name of the type attached to the
related Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — durability

**Spec:** §2.2.5, p. 143 — "durability DurabilityQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — deadline

**Spec:** §2.2.5, p. 143 — "deadline DeadlineQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — latency_budget

**Spec:** §2.2.5, p. 143 — "latency_budget LatencyBudgetQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — liveliness

**Spec:** §2.2.5, p. 143 — "liveliness LivelinessQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — reliability

**Spec:** §2.2.5, p. 143 — "reliability ReliabilityQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — ownership

**Spec:** §2.2.5, p. 143 — "ownership OwnershipQosPolicy Policy of the
corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — destination_order

**Spec:** §2.2.5, p. 143 — "destination_order DestinationOrderQosPolicy Policy
of the corresponding DataReader."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — user_data

**Spec:** §2.2.5, p. 143 — "user_data UserDataQosPolicy Policy of the
corresponding DataReader."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::user_data`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — time_based_filter

**Spec:** §2.2.5, p. 143 — "time_based_filter TimeBasedFilterQosPolicy Policy of
the corresponding DataReader."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::time_based_filter`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — presentation

**Spec:** §2.2.5, p. 143 — "presentation PresentationQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — partition

**Spec:** §2.2.5, p. 143 — "partition PartitionQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`SubscriptionBuiltinTopicData::partition`,
`SubscriptionBuiltinTopicData::from_wire`)

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_from_wire_copies_topic_and_type`

**Status:** done

### §2.2.5 DCPSSubscription — topic_data

**Spec:** §2.2.5, p. 143 — "topic_data TopicDataQosPolicy Policy of the related
Topic."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — group_data

**Spec:** §2.2.5, p. 143 — "group_data GroupDataQosPolicy Policy of the
Subscriber to which the DataReader belongs."

**Repo:** —

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::subscription_sample_carries_every_member`

**Status:** done — the sample carries the value; the test sets it away from the default and reads it back.

### §2.2.5 DCPSSubscription — instance_handle in the SampleInfo

**Spec:** §2.2.2.2.1.17, p. 40 — "This handle is the one that appears in the
SampleInfo retrieved when reading the data-samples from the built-in DataReader
to the “DCPSSubscription” topic."; §2.2.2.4.2.24, p. 68 — "These handles match
the ones that appear in the ‘instance_handle’ field of the SampleInfo when
reading the “DCPSSubscriptions” builtin topic."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`InstanceTracker::with_key_handles`)

**Tests:**
`crates/dcps/tests/ignore_entities.rs::linux::ignore_subscription_unmatches`

**Status:** `done` — the handle is the one of
`DataWriter::matched_subscriptions`.

## §2.2.6 Interaction Model

### §2.2.6 Purpose of the interaction models

**Spec:** §2.2.6, p. 143 — "these models are not intended to explain how the
Service is implemented … should be understood as how it may work rather than how
it actually does work".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — illustration.

### §2.2.6.1 Publication View

**Spec:** §2.2.6.1, pp. 144–145 — Figure 2.21; "It is important to take into
account that the corresponding notifications are now delayed until the last
resume_publications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — sequence diagram; `suspend_publications` and
`resume_publications` are listed under §2.2.2.4.1.8 and §2.2.2.4.1.9.

### §2.2.6.2.1 Notification via Listeners

**Spec:** §2.2.6.2.1, p. 145 — Figure 2.22; "NOTE: When the two kinds of
listeners are set, the SubscriberListener supersedes the DataReaderListener
ones."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — sequence diagram; the rule is listed under
§2.2.4.3.2.

### §2.2.6.2.2 Notifications via Conditions and Wait-Sets

**Spec:** §2.2.6.2.2, p. 147 — Figure 2.23; "Note that the wait returns the
list of all the enabled conditions, in an arrival cycle".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — sequence diagram; `wait` is listed under
§2.2.2.1.6.3.

---

## §2.3 OMG IDL Platform Specific Model (PSM)

### §2.3.1 Introduction

**Spec:** §2.3.1, p. 149 — "The OMG IDL PSM is provided by means of the IDL that
defines the interface an application can use to interact with the Service."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction. The Rust API of `zerodds-dcps`
is not a mapping of this IDL (see the head); the following items check the
counterparts of the IDL constants, types and interfaces.

### §2.3.2 out parameters as inout

**Spec:** §2.3.2, p. 149 — "‘Out’ parameters in the PIM are conventionally
mapped to ‘inout’ parameters in the PSM in order to minimize the memory
allocation performed by the Service".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a rule of the IDL mapping; the Rust API
returns results as a return value (`Result<T>`).

### §2.3.2 Interfaces and data types

**Spec:** §2.3.2, p. 149 — "The PIM to PSM mapping maps the UML interfaces and
classes interfaces into IDL interfaces. Plain data types are mapped into
structures. IDL interfaces do not support overloading … the abstract operation
appears inside comments."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a rule of the IDL mapping; the Rust API maps
classes to types and traits, typed operations to generics (`DataWriter<T>`,
`DataReader<T>`).

### §2.3.2 Masks for StatusKind, SampleStateKind, ViewStateKind, InstanceStateKind

**Spec:** §2.3.2, p. 149 — "Enumerations have been mapped into either IDL ’enum’
or hand-picked IDL ‘long’ values that correspond to increasing powers of 2 …
This selection affected only the PIM ‘status kind’ values, namely: StatusKind,
SampleStateKind, ViewStateKind, and InstanceStateKind. The name of the mask type
is formed by replacing the word ‘Kind’ with the word ‘Mask’".

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`, `state`);
`crates/dcps/src/entity.rs` (`StatusMask`); `crates/dcps/src/sample_info.rs`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`,
`crates/dcps/src/psm_constants.rs::tests::instance_state_not_alive_is_or_of_disposed_and_no_writers`

**Status:** `done` — the bits are powers of two by the IDL, the same in
`psm_constants` and in `sample_info`.

### §2.3.2 Collections as sequences

**Spec:** §2.3.2, p. 149 — "Collection parameters have been mapped into IDL
sequences. The only exception applies to the case where the collection elements
are hand-picked IDL ‘long.’ In this case the collection is mapped into an IDL
‘long’ interpreted as a mask."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a rule of the IDL mapping; the Rust API uses
`Vec<T>` and `u32` masks.

### §2.3.2 QosPolicy as a struct, the QoS per entity as a struct

**Spec:** §2.3.2, p. 149 — "Each QosPolicy has been mapped as an IDL struct. The
collection of policies suitable for each Entity has been modeled as another IDL
struct that contains attributes corresponding to the policies that apply to this
Entity."

**Repo:** `crates/qos/src/policies/` (one struct per policy);
`crates/dcps/src/qos.rs` (`DomainParticipantQos`, `TopicQos`, `PublisherQos`,
`SubscriberQos`, `DataWriterQos`, `DataReaderQos`); `crates/dcps/src/factory.rs`
(`DomainParticipantFactoryQos`)

**Tests:** `crates/qos/src/policies/qos_set.rs::tests::writer_qos_clone_and_eq`,
`crates/dcps/src/qos.rs::group_policy_tests::endpoint_default_presentation_inherits_the_group_value`

**Status:** done — the contents per struct are under §2.3.3.

### §2.3.2 Errors as return codes

**Spec:** §2.3.2, p. 149 — "Error-return values have been mapped to plain return
codes of the corresponding functions."

**Repo:** `crates/dcps/src/error.rs` (`DdsError::as_return_code`, `return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`,
`crates/dcps/src/error.rs::tests::rc_not_enabled_maps`

**Status:** done — the Rust API returns `Result<T, DdsError>`; `as_return_code`
maps every error to the value of the `ReturnCode_t`.

### §2.3.2 DataSample not mapped

**Spec:** §2.3.2, pp. 149–150 — "The DataSample class that associates the
SampleInfo and Data collections returned from the data-accessing operations
(read and take) have not been explicitly mapped into IDL … It is anticipated
that additional data-accessing API’s may be provided on each target language".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission; the Rust API returns pairs of
data and SampleInfo (`Sample<T>`).

### §2.3.2 WaitSet and GuardCondition with a constructor without arguments

**Spec:** §2.3.2, p. 150 — "The classes that do not have factory operations,
namely WaitSet and GuardCondition are mapped to IDL interfaces … the
implementation language mapping should offer at least a constructor that takes
no arguments".

**Repo:** `crates/dcps/src/condition.rs` (`WaitSet::new`, `GuardCondition::new`)

**Tests:** `crates/dcps/src/condition.rs::tests::waitset_default_is_empty`,
`crates/dcps/src/condition.rs::tests::guard_condition_starts_false`

**Status:** done

### §2.3.2 get_instance as a static operation

**Spec:** §2.3.2, p. 150 — "The language implementations of the
DomainParticipantFactory interface should have the static operation get_instance
described in 2.2.2.2.2."

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactory::instance`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_is_singleton`

**Status:** done — the operation is called `instance` (§2.2.2.2.2.3).

### §2.3.2 Duration_t and Time_t with sec and nanosec

**Spec:** §2.3.2, p. 150 — "The two types used to represent time: Duration_t and
Time_t are mapped into structures that contain fields for the second and the
nanosecond parts."

**Repo:** `crates/dcps/src/time.rs` (`Time`, `Duration`: `sec`, `nanosec`)

**Tests:** `crates/dcps/src/time.rs::tests::time_seconds_and_nanoseconds_accessors`,
`crates/dcps/src/time.rs::tests::duration_seconds_and_nanoseconds_accessors`,
`crates/dcps/src/time.rs::tests::a_qos_duration_converts_to_sec_and_nanosec_and_back`

**Status:** done — the sub-items below it are done; see their tests.

### §2.3.2 Durations of the QoS policies as Duration_t

**Spec:** §2.3.3, pp. 157–159 — `LifespanQosPolicy { Duration_t duration; }`,
`DeadlineQosPolicy { Duration_t period; }` and others; `struct Duration_t { long
sec; unsigned long nanosec; }` (p. 151).

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::a_qos_duration_converts_to_sec_and_nanosec_and_back`

**Status:** done — the QoS durations keep the RTPS layout (`seconds`, `fraction`) and convert to and from the `sec`/`nanosec` `Duration` (`From`), the infinite one stays infinite.

### §2.3.2 normalized representation 0 <= nanosec < 1000000000

**Spec:** §2.3.2, p. 150 — "These types are further constrained to always use a
‘normalized’ representation for the time, that is, the nanosec field must verify
0 <= nanosec < 1000000000."

**Repo:** `crates/dcps/src/time.rs` (`Time::new`, `Duration::new`, `is_normalized`, `from_millis`);
`crates/dcps/src/publisher.rs` (`check_timestamp`);
`crates/zerodds-c-api/src/extra_ffi.rs` (`*_w_timestamp`);
`crates/qos/src/duration.rs`

**Tests:** `crates/dcps/src/time.rs::tests::the_constructors_keep_nanosec_below_a_second`,
`crates/dcps/src/time.rs::tests::from_millis_of_a_negative_value_is_normalized`,
`crates/dcps/tests/sample_info_lifecycle.rs::a_timestamp_out_of_range_is_bad_parameter`,
`crates/zerodds-c-api/src/extra_ffi.rs::tests::dw_w_timestamp_calls_refuse_a_nanosec_out_of_range`

**Status:** `done` — `Time::new` and `Duration::new` carry whole seconds of `nanosec` into `sec` (`TIME_INVALID`, `TIME_INFINITE` and `DURATION_INFINITE` are kept); `from_millis` floors, also for negative values. `is_normalized` reports a value built by hand through the public fields; the four `*_w_timestamp` refuse `nanosec` of 10^9 or more with BAD_PARAMETER, also in the C API. Durations of the QoS policies are stored in fractions of 2^-32 s and so normalized.

### §2.3.2 Native types

**Spec:** §2.3.2, p. 150 — "The IDL PSM introduces a number of types that are
intended to be defined in a native way … The type used in the IDL by this means
is not normative and an implementation is allowed to use any other type,
including non-scalar (i.e., structured types)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a permission; `DomainId_t` is `i32`,
`InstanceHandle_t` is `InstanceHandle` (`u64`), `BuiltinTopicKey_t` is the
16-byte GUID.

### §2.3.3 HANDLE_NIL

**Spec:** §2.3.3, pp. 150–151 — "#define HANDLE_NIL_NATIVE 0 … const
InstanceHandle_t HANDLE_NIL = HANDLE_NIL_NATIVE;"

**Repo:** `crates/dcps/src/instance_handle.rs` (`HANDLE_NIL`)

**Tests:** `crates/dcps/src/instance_handle.rs::tests::handle_nil_is_zero`

**Status:** done

### §2.3.3 LENGTH_UNLIMITED

**Spec:** §2.3.3, p. 151 — "const long LENGTH_UNLIMITED = -1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`LENGTH_UNLIMITED`);
`crates/qos/src/policies/resource_limits.rs` (`LENGTH_UNLIMITED`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::length_unlimited_is_minus_one`,
`crates/qos/src/policies/resource_limits.rs::tests::encode_decode_negative_one`

**Status:** done

### §2.3.3 DURATION_INFINITE

**Spec:** §2.3.3, p. 151 — "const long DURATION_INFINITE_SEC = 0x7fffffff; const
unsigned long DURATION_INFINITE_NSEC = 0x7fffffff;"

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_values_are_those_of_the_idl`

**Status:** done — `Duration::INFINITE` is `{ sec: 0x7fffffff, nanosec: 0x7fffffff }` as the IDL has it (the nanosec was `0xffffffff`).

### §2.3.3 DURATION_ZERO

**Spec:** §2.3.3, p. 151 — "const long DURATION_ZERO_SEC = 0; const unsigned
long DURATION_ZERO_NSEC = 0;"

**Repo:** `crates/dcps/src/time.rs` (`Duration::ZERO`);
`crates/qos/src/duration.rs` (`Duration::ZERO`)

**Tests:** `crates/dcps/src/time.rs::tests::duration_zero_and_infinite_sentinels`,
`crates/qos/src/duration.rs::tests::zero_is_default_and_zero`

**Status:** done

### §2.3.3 TIME_INVALID

**Spec:** §2.3.3, p. 151 — "const long TIME_INVALID_SEC = -1; const unsigned
long TIME_INVALID_NSEC = 0xffffffff;"

**Repo:** —

**Tests:** `crates/dcps/src/time.rs::tests::the_reserved_values_are_those_of_the_idl`

**Status:** done — the test covers it.

### §2.3.3 Return codes RETCODE_OK to RETCODE_ILLEGAL_OPERATION

**Spec:** §2.3.3, p. 151 — "const ReturnCode_t RETCODE_OK = 0; … RETCODE_ERROR =
1; … RETCODE_ILLEGAL_OPERATION = 12;"

**Repo:** `crates/dcps/src/error.rs` (`return_code`)

**Tests:** `crates/dcps/src/error.rs::tests::rc_constants_have_spec_values`

**Status:** done — in addition `NOT_ALLOWED_BY_SECURITY = 13` from DDS-Security.

### §2.3.3 StatusKind bits

**Spec:** §2.3.3, pp. 151–152 — "const StatusKind INCONSISTENT_TOPIC_STATUS =
0x0001 << 0; … SUBSCRIPTION_MATCHED_STATUS = 0x0001 << 14;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`status`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::status_mask_any_covers_13_bits`,
`crates/dcps/src/listener.rs::tests::status_bit_for_inconsistent_topic_matches_psm`

**Status:** done — the values agree with the IDL (bits 0, 1, 2, 5 to 14); the
names have no suffix `_STATUS`.

### §2.3.3 Status structures

**Spec:** §2.3.3, pp. 152–153 — `InconsistentTopicStatus`, `SampleLostStatus`,
`SampleRejectedStatus`, `LivelinessLostStatus`, `LivelinessChangedStatus`,
`OfferedDeadlineMissedStatus`, `RequestedDeadlineMissedStatus`,
`QosPolicyCount`, `OfferedIncompatibleQosStatus`,
`RequestedIncompatibleQosStatus`, `PublicationMatchedStatus`,
`SubscriptionMatchedStatus`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the same structures as in the PIM; their
fields are listed one by one under §2.2.4.1.

### §2.3.3 SampleRejectedStatusKind

**Spec:** §2.3.3, p. 152 — "enum SampleRejectedStatusKind { NOT_REJECTED,
REJECTED_BY_INSTANCES_LIMIT, REJECTED_BY_SAMPLES_LIMIT,
REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT };"

**Repo:** `crates/dcps/src/status.rs` (`SampleRejectedStatusKind`)

**Tests:** `crates/dcps/src/status.rs::tests::sample_rejected_kind_variants_are_distinct`,
`crates/dcps/src/status.rs::tests::sample_rejected_status_default_kind_is_not_rejected`

**Status:** done — four variants in the order of the IDL.

### §2.3.3 Listener interfaces

**Spec:** §2.3.3, pp. 154–155 — `TopicListener`, `DataWriterListener`,
`PublisherListener : DataWriterListener`, `DataReaderListener`,
`SubscriberListener : DataReaderListener` with `on_data_on_readers(in Subscriber
the_subscriber)`, `DomainParticipantListener : TopicListener, PublisherListener,
SubscriberListener`; every operation with `in DataWriter writer`, `in DataReader
the_reader` or `in Topic the_topic`.

**Repo:** —

**Tests:** `crates/dcps/tests/listener_entity.rs::a_shared_listener_is_called_with_the_entity`

**Status:** done — the inheritance and the operations correspond to the IDL (`crates/dcps/src/listener.rs`); the first parameter of the entity form is the entity (see §2.2.4.3).

### §2.3.3 Condition, WaitSet, GuardCondition, StatusCondition

**Spec:** §2.3.3, p. 155 — `Condition::get_trigger_value`, `WaitSet::wait(inout
ConditionSeq active_conditions, in Duration_t timeout)`, `attach_condition`,
`detach_condition`, `get_conditions`, `GuardCondition::set_trigger_value`,
`StatusCondition::get_enabled_statuses`, `set_enabled_statuses`, `get_entity`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the same operations as in the PIM; they are
listed one by one under §2.2.2.1.6 to §2.2.2.1.9.

### §2.3.3 SampleStateKind bits

**Spec:** §2.3.3, p. 155 — "const SampleStateKind READ_SAMPLE_STATE = 0x0001 <<
0; const SampleStateKind NOT_READ_SAMPLE_STATE = 0x0001 << 1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state::READ_SAMPLE_STATE`,
`state::NOT_READ_SAMPLE_STATE`),
`crates/dcps/src/sample_info.rs::sample_state_mask`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — the values of the IDL in both modules.

### §2.3.3 ViewStateKind bits

**Spec:** §2.3.3, p. 156 — "const ViewStateKind NEW_VIEW_STATE = 0x0001 << 0;
const ViewStateKind NOT_NEW_VIEW_STATE = 0x0001 << 1;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state::NEW_VIEW_STATE`,
`state::NOT_NEW_VIEW_STATE`), `crates/dcps/src/sample_info.rs::view_state_mask`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — the values of the IDL in both modules.

### §2.3.3 InstanceStateKind bits and NOT_ALIVE_INSTANCE_STATE

**Spec:** §2.3.3, p. 156 — "const InstanceStateKind ALIVE_INSTANCE_STATE =
0x0001 << 0; … NOT_ALIVE_DISPOSED_INSTANCE_STATE = 0x0001 << 1; …
NOT_ALIVE_NO_WRITERS_INSTANCE_STATE = 0x0001 << 2; … const InstanceStateMask
NOT_ALIVE_INSTANCE_STATE = 0x006;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state`);
`crates/dcps/src/sample_info.rs` (`instance_state_mask`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::instance_state_not_alive_is_or_of_disposed_and_no_writers`

**Status:** done

### §2.3.3 ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE = 0xffff

**Spec:** §2.3.3, pp. 155–156 — "const SampleStateMask ANY_SAMPLE_STATE =
0xffff; … const ViewStateMask ANY_VIEW_STATE = 0xffff; … const InstanceStateMask
ANY_INSTANCE_STATE = 0xffff;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`state`),
`crates/dcps/src/sample_info.rs`

**Tests:**
`crates/dcps/src/psm_constants.rs::tests::the_state_masks_are_those_of_the_idl`

**Status:** `done` — the three masks are 0xffff.

### §2.3.3 ReadCondition and QueryCondition

**Spec:** §2.3.3, p. 156 — `ReadCondition::get_sample_state_mask`,
`get_view_state_mask`, `get_instance_state_mask`, `get_datareader`;
`QueryCondition::get_query_expression`, `get_query_parameters`,
`set_query_parameters`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the same operations as in the PIM; they are
listed one by one under §2.2.2.5.8 and §2.2.2.5.9.

### §2.3.3 Names of the QoS policies

**Spec:** §2.3.3, pp. 156–157 — "const string USERDATA_QOS_POLICY_NAME =
"UserData"; … const string DURABILITYSERVICE_POLICY_NAME = "DurabilityService";"

**Repo:** `crates/qos/src/policies/name.rs`, `crates/dcps/src/psm_constants.rs` (`qos_policy_name`)

**Tests:** `crates/qos/src/policies/name.rs::tests::every_policy_has_its_name`

**Status:** done — the name constants of the 22 policies are in `zerodds_qos::policy_name` and `zerodds_dcps::psm_constants::qos_policy_name`; the policies have `QosPolicy::name` (§2.2.2.1.3). For GROUPDATA see there.

### §2.3.3 QosPolicyId_t values

**Spec:** §2.3.3, p. 157 — "const QosPolicyId_t INVALID_QOS_POLICY_ID = 0; const
QosPolicyId_t USERDATA_QOS_POLICY_ID = 1; … DURABILITYSERVICE_QOS_POLICY_ID =
22;"

**Repo:** `crates/dcps/src/psm_constants.rs` (`qos_policy_id`)

**Tests:** `crates/dcps/src/psm_constants.rs::tests::qos_policy_ids_are_distinct`

**Status:** done — the values 0 to 22 agree with the IDL; the names read
`qos_policy_id::USER_DATA` instead of `USERDATA_QOS_POLICY_ID`. In addition 23
and 24 from XTypes 1.3.

### §2.3.3 UserDataQosPolicy

**Spec:** §2.3.3, p. 157 — `struct UserDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`UserDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::user_data_roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 TopicDataQosPolicy

**Spec:** §2.3.3, p. 157 — `struct TopicDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`TopicDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::empty_data_roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 GroupDataQosPolicy

**Spec:** §2.3.3, p. 157 — `struct GroupDataQosPolicy`: sequence<octet> value.

**Repo:** `crates/qos/src/policies/generic_data.rs` (`GroupDataQosPolicy`)

**Tests:** `crates/qos/src/policies/generic_data.rs::tests::group_data_large_payload`

**Status:** done — fields as in the IDL.

### §2.3.3 TransportPriorityQosPolicy

**Spec:** §2.3.3, p. 157 — `struct TransportPriorityQosPolicy`: long value.

**Repo:** `crates/qos/src/policies/transport_priority.rs`
(`TransportPriorityQosPolicy`)

**Tests:** `crates/qos/src/policies/transport_priority.rs::tests::roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 LifespanQosPolicy

**Spec:** §2.3.3, p. 157 — `struct LifespanQosPolicy`: Duration_t duration.

**Repo:** `crates/qos/src/policies/lifespan.rs` (`LifespanQosPolicy`)

**Tests:** `crates/qos/src/policies/lifespan.rs::tests::roundtrip`

**Status:** done — fields as in the IDL. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 DurabilityQosPolicy

**Spec:** §2.3.3, p. 157 — `struct DurabilityQosPolicy`: DurabilityQosPolicyKind
kind (VOLATILE_DURABILITY_QOS, TRANSIENT_LOCAL_DURABILITY_QOS,
TRANSIENT_DURABILITY_QOS, PERSISTENT_DURABILITY_QOS).

**Repo:** `crates/qos/src/policies/durability.rs` (`DurabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/durability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/durability.rs::tests::ordering_matches_spec`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants.

### §2.3.3 PresentationQosPolicy

**Spec:** §2.3.3, p. 158 — `struct PresentationQosPolicy`:
PresentationQosPolicyAccessScopeKind access_scope (INSTANCE_PRESENTATION_QOS,
TOPIC_PRESENTATION_QOS, GROUP_PRESENTATION_QOS); boolean coherent_access;
boolean ordered_access.

**Repo:** `crates/qos/src/policies/presentation.rs` (`PresentationQosPolicy`)

**Tests:** `crates/qos/src/policies/presentation.rs::tests::roundtrip_all_flags`,
`crates/qos/src/policies/presentation.rs::tests::scope_ordering`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants.

### §2.3.3 DeadlineQosPolicy

**Spec:** §2.3.3, p. 158 — `struct DeadlineQosPolicy`: Duration_t period.

**Repo:** `crates/qos/src/policies/deadline.rs` (`DeadlineQosPolicy`)

**Tests:** `crates/qos/src/policies/deadline.rs::tests::roundtrip`

**Status:** done — fields as in the IDL. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 LatencyBudgetQosPolicy

**Spec:** §2.3.3, p. 158 — `struct LatencyBudgetQosPolicy`: Duration_t duration.

**Repo:** `crates/qos/src/policies/latency_budget.rs` (`LatencyBudgetQosPolicy`)

**Tests:** `crates/qos/src/policies/latency_budget.rs::tests::roundtrip`

**Status:** done — fields as in the IDL. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 OwnershipQosPolicy

**Spec:** §2.3.3, p. 158 — `struct OwnershipQosPolicy`: OwnershipQosPolicyKind
kind (SHARED_OWNERSHIP_QOS, EXCLUSIVE_OWNERSHIP_QOS).

**Repo:** `crates/qos/src/policies/ownership.rs` (`OwnershipQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership.rs::tests::roundtrip`,
`crates/qos/src/policies/ownership.rs::tests::try_from_u32_valid_variants`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants.

### §2.3.3 OwnershipStrengthQosPolicy

**Spec:** §2.3.3, p. 158 — `struct OwnershipStrengthQosPolicy`: long value.

**Repo:** `crates/qos/src/policies/ownership_strength.rs`
(`OwnershipStrengthQosPolicy`)

**Tests:** `crates/qos/src/policies/ownership_strength.rs::tests::roundtrip_negative`

**Status:** done — fields as in the IDL.

### §2.3.3 LivelinessQosPolicy

**Spec:** §2.3.3, p. 158 — `struct LivelinessQosPolicy`: LivelinessQosPolicyKind
kind (AUTOMATIC_LIVELINESS_QOS, MANUAL_BY_PARTICIPANT_LIVELINESS_QOS,
MANUAL_BY_TOPIC_LIVELINESS_QOS); Duration_t lease_duration.

**Repo:** `crates/qos/src/policies/liveliness.rs` (`LivelinessQosPolicy`)

**Tests:** `crates/qos/src/policies/liveliness.rs::tests::roundtrip`,
`crates/qos/src/policies/liveliness.rs::tests::kind_ordering`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 TimeBasedFilterQosPolicy

**Spec:** §2.3.3, p. 158 — `struct TimeBasedFilterQosPolicy`: Duration_t
minimum_separation.

**Repo:** `crates/qos/src/policies/time_based_filter.rs`
(`TimeBasedFilterQosPolicy`)

**Tests:** `crates/qos/src/policies/time_based_filter.rs::tests::roundtrip`

**Status:** done — fields as in the IDL. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 PartitionQosPolicy

**Spec:** §2.3.3, p. 158 — `struct PartitionQosPolicy`: StringSeq name.

**Repo:** `crates/qos/src/policies/partition.rs` (`PartitionQosPolicy`)

**Tests:** `crates/qos/src/policies/partition.rs::tests::roundtrip_two_names`

**Status:** done — the field is called `names` instead of `name`.

### §2.3.3 ReliabilityQosPolicy

**Spec:** §2.3.3, p. 158 — `struct ReliabilityQosPolicy`:
ReliabilityQosPolicyKind kind (BEST_EFFORT_RELIABILITY_QOS,
RELIABLE_RELIABILITY_QOS); Duration_t max_blocking_time.

**Repo:** `crates/qos/src/policies/reliability.rs` (`ReliabilityQosPolicy`)

**Tests:** `crates/qos/src/policies/reliability.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/reliability.rs::tests::ordering_reliable_higher_than_besteffort`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants. The discriminants of
`ReliabilityKind` are the RTPS values 1 and 2, not the ordinals 0 and 1 of the
IDL.

### §2.3.3 DestinationOrderQosPolicy

**Spec:** §2.3.3, p. 159 — `struct DestinationOrderQosPolicy`:
DestinationOrderQosPolicyKind kind (BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS,
BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS).

**Repo:** `crates/qos/src/policies/destination_order.rs`
(`DestinationOrderQosPolicy`)

**Tests:** `crates/qos/src/policies/destination_order.rs::tests::roundtrip`,
`crates/qos/src/policies/destination_order.rs::tests::kind_ordering`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants.

### §2.3.3 HistoryQosPolicy

**Spec:** §2.3.3, p. 159 — `struct HistoryQosPolicy`: HistoryQosPolicyKind kind
(KEEP_LAST_HISTORY_QOS, KEEP_ALL_HISTORY_QOS); long depth.

**Repo:** `crates/qos/src/policies/history.rs` (`HistoryQosPolicy`)

**Tests:** `crates/qos/src/policies/history.rs::tests::encode_decode_roundtrip`,
`crates/qos/src/policies/history.rs::tests::encode_decode_keep_all`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants.

### §2.3.3 ResourceLimitsQosPolicy

**Spec:** §2.3.3, p. 159 — `struct ResourceLimitsQosPolicy`: long max_samples;
long max_instances; long max_samples_per_instance.

**Repo:** `crates/qos/src/policies/resource_limits.rs`
(`ResourceLimitsQosPolicy`)

**Tests:** `crates/qos/src/policies/resource_limits.rs::tests::encode_decode_roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 EntityFactoryQosPolicy

**Spec:** §2.3.3, p. 159 — `struct EntityFactoryQosPolicy`: boolean
autoenable_created_entities.

**Repo:** `crates/qos/src/policies/entity_factory.rs` (`EntityFactoryQosPolicy`)

**Tests:** `crates/qos/src/policies/entity_factory.rs::tests::roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 WriterDataLifecycleQosPolicy

**Spec:** §2.3.3, p. 159 — `struct WriterDataLifecycleQosPolicy`: boolean
autodispose_unregistered_instances.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`WriterDataLifecycleQosPolicy`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::writer_roundtrip`

**Status:** done — fields as in the IDL.

### §2.3.3 ReaderDataLifecycleQosPolicy

**Spec:** §2.3.3, p. 159 — `struct ReaderDataLifecycleQosPolicy`: Duration_t
autopurge_nowriter_samples_delay; Duration_t autopurge_disposed_samples_delay.

**Repo:** `crates/qos/src/policies/data_lifecycle.rs`
(`ReaderDataLifecycleQosPolicy`)

**Tests:** `crates/qos/src/policies/data_lifecycle.rs::tests::reader_roundtrip`

**Status:** done — fields as in the IDL. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 DurabilityServiceQosPolicy

**Spec:** §2.3.3, p. 159 — `struct DurabilityServiceQosPolicy`: Duration_t
service_cleanup_delay; HistoryQosPolicyKind history_kind; long history_depth;
long max_samples; long max_instances; long max_samples_per_instance.

**Repo:** `crates/qos/src/policies/durability_service.rs`
(`DurabilityServiceQosPolicy`)

**Tests:** `crates/qos/src/policies/durability_service.rs::tests::roundtrip`

**Status:** done — fields as in the IDL; the kind is a Rust `enum`, without the
suffix `_QOS` in the names of the variants. The durations are
`zerodds_qos::Duration` (item “durations of the QoS policies as Duration_t”).

### §2.3.3 DomainParticipantFactoryQos

**Spec:** §2.3.3, pp. 159–160 — `struct DomainParticipantFactoryQos`:
entity_factory.

**Repo:** `crates/dcps/src/factory.rs` (`DomainParticipantFactoryQos`)

**Tests:** `crates/dcps/src/factory.rs::tests::factory_qos_default_is_autoenable_true`

**Status:** done — fields as in the IDL.

### §2.3.3 DomainParticipantQos

**Spec:** §2.3.3, p. 160 — `struct DomainParticipantQos`: user_data,
entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`DomainParticipantQos`)

**Tests:** `crates/dcps/tests/participant_config_user_data.rs::an_explicit_config_keeps_the_user_data_of_the_qos`

**Status:** done — fields as in the IDL.

### §2.3.3 TopicQos

**Spec:** §2.3.3, p. 160 — `struct TopicQos`: topic_data, durability,
durability_service, deadline, latency_budget, liveliness, reliability,
destination_order, history, resource_limits, transport_priority, lifespan,
ownership.

**Repo:** `crates/dcps/src/qos.rs` (`TopicQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::topic_set_qos_pre_enable_allows_durability_change`

**Status:** done — fields as in the IDL.

### §2.3.3 DataWriterQos

**Spec:** §2.3.3, p. 160 — `struct DataWriterQos`: durability,
durability_service, deadline, latency_budget, liveliness, reliability,
destination_order, history, resource_limits, transport_priority, lifespan,
user_data, ownership, ownership_strength, writer_data_lifecycle.

**Repo:** `crates/dcps/src/qos.rs` (`DataWriterQos`)

**Tests:** `crates/dcps/src/publisher.rs::tests::create_datawriter_wires_history_keep_last_depth`

**Status:** done — the fields of the IDL and in addition `partition`,
`presentation`, `topic_data`, `group_data` as an override per endpoint, and
`data_representation` from XTypes 1.3.

### §2.3.3 PublisherQos

**Spec:** §2.3.3, p. 160 — `struct PublisherQos`: presentation, partition,
group_data, entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`PublisherQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_follows_the_participants_entity_factory_policy`

**Status:** done — fields as in the IDL.

### §2.3.3 DataReaderQos

**Spec:** §2.3.3, pp. 160–161 — `struct DataReaderQos`: durability, deadline,
latency_budget, liveliness, reliability, destination_order, history,
resource_limits, user_data, ownership, time_based_filter, reader_data_lifecycle.

**Repo:** `crates/dcps/src/qos.rs` (`DataReaderQos`)

**Tests:** `crates/dcps/tests/builtin_topic_readers.rs::builtin_readers_have_spec_default_qos`

**Status:** done — the fields of the IDL and in addition `partition`,
`presentation`, `topic_data`, `group_data` as an override per endpoint, and
`data_representation` and `type_consistency` from XTypes 1.3.

### §2.3.3 SubscriberQos

**Spec:** §2.3.3, p. 161 — `struct SubscriberQos`: presentation, partition,
group_data, entity_factory.

**Repo:** `crates/dcps/src/qos.rs` (`SubscriberQos`)

**Tests:** `crates/dcps/tests/entity_lifecycle.rs::publisher_and_subscriber_refuse_a_presentation_change_once_enabled`

**Status:** done — fields as in the IDL.

### §2.3.3 Structures of the built-in topics

**Spec:** §2.3.3, pp. 161–162 — `ParticipantBuiltinTopicData`,
`TopicBuiltinTopicData`, `PublicationBuiltinTopicData`,
`SubscriptionBuiltinTopicData`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the same fields as the table in §2.2.5; they
are listed one by one there.

### §2.3.3 Entity

**Spec:** §2.3.3, p. 162 — `interface Entity` with the operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.1.1.

### §2.3.3 DomainParticipant

**Spec:** §2.3.3, pp. 163–164 — `interface DomainParticipant` with the
operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.2.1.

### §2.3.3 DomainParticipantFactory

**Spec:** §2.3.3, p. 165 — `interface DomainParticipantFactory` with the
operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.2.2.

### §2.3.3 TopicDescription, Topic, ContentFilteredTopic, MultiTopic

**Spec:** §2.3.3, pp. 165–166 — `interface TopicDescription, Topic,
ContentFilteredTopic, MultiTopic` with the operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.3.

### §2.3.3 Publisher

**Spec:** §2.3.3, p. 166 — `interface Publisher` with the operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.4.1.

### §2.3.3 DataWriter

**Spec:** §2.3.3, pp. 167–168 — `interface DataWriter` with the operations of
the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.4.2.

### §2.3.3 Subscriber

**Spec:** §2.3.3, pp. 168–169 — `interface Subscriber` with the operations of
the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.5.2.

### §2.3.3 DataReader

**Spec:** §2.3.3, pp. 169–171 — `interface DataReader` with the operations of
the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.5.3.

### §2.3.3 SampleInfo

**Spec:** §2.3.3, pp. 171–172 — `interface SampleInfo` or `struct SampleInfo`
with the operations of the PIM.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a repetition of the PIM in IDL; the operations
and fields are listed one by one under §2.2.2.5.5.

### §2.3.3 TypeSupport and FooTypeSupport

**Spec:** §2.3.3, pp. 165, 172 — "interface TypeSupport { // ReturnCode_t
register_type(…); // string get_type_name(); };" and "interface FooTypeSupport :
DDS::TypeSupport { DDS::ReturnCode_t register_type(…); string get_type_name();
};"

**Repo:** `crates/dcps/src/type_support.rs`

**Tests:** `crates/dcps/tests/type_support.rs::the_type_support_names_its_type`, `crates/dcps/tests/type_support.rs::the_same_type_support_registers_again_under_the_same_or_another_name`

**Status:** done — `TypeSupport<T>` with `register_type` and `get_type_name`; `FooTypeSupport` is `TypeSupport<Foo>`.

### §2.3.3 FooDataWriter and FooDataReader

**Spec:** §2.3.3, pp. 172–174 — "Implied IDL for type "Foo"": `interface
FooDataWriter : DDS::DataWriter` and `interface FooDataReader : DDS::DataReader`
with the typed operations.

**Repo:** `crates/dcps/src/publisher.rs` (`DataWriter<T>`);
`crates/dcps/src/subscriber.rs` (`DataReader<T>`); `crates/dcps/src/dds_type.rs`
(`DdsType`)

**Tests:** `crates/zerodds-build/tests/compile.rs::a_struct_becomes_a_type_with_a_codec`,
`crates/dcps/tests/sample_info_lifecycle.rs::writer_dispose_transitions_instance_to_disposed`

**Status:** done — the typed interfaces are the generics `DataWriter<T>` and
`DataReader<T>`; the individual operations are listed under §2.2.2.4.2 and
§2.2.2.5.3.

### §2.3.3 Placeholders TheParticipantFactory and *_QOS_DEFAULT

**Spec:** §2.3.3, p. 150 — "#define TheParticipantFactory #define
PARTICIPANT_QOS_DEFAULT … #define DATAWRITER_QOS_USE_TOPIC_QOS #define
DATAREADER_QOS_USE_TOPIC_QOS".

**Repo:** `crates/dcps/src/psm_constants.rs` (`PARTICIPANT_QOS_DEFAULT` …
`DATAREADER_QOS_USE_TOPIC_QOS`),
`crates/dcps/src/factory.rs::THE_PARTICIPANT_FACTORY`

**Tests:**
`crates/dcps/tests/default_qos.rs::the_participant_factory_is_the_instance`,
`crates/dcps/tests/default_qos.rs::datawriter_default_and_use_topic_qos`,
`crates/dcps/tests/default_qos.rs::datareader_default_and_use_topic_qos`

**Status:** `done` — the eight placeholders as values of `QosArg` and
`EndpointQosArg`, `TheParticipantFactory` as `THE_PARTICIPANT_FACTORY`.

## Annex A Compliance Points

### Annex A Minimum profile

**Spec:** Annex A, p. 175 — "Minimum profile: This profile contains just the
mandatory features of DCPS. None of the optional features are included."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — profile definition; the mandatory features are
listed item by item above.

### Annex A Content-subscription profile

**Spec:** Annex A, p. 175 — "Content-subscription profile: This profile adds the
optional classes: ContentFilteredTopic, QueryCondition, MultiTopic."

**Repo:** `crates/dcps/src/topic.rs` (`ContentFilteredTopic`, `MultiTopic`);
`crates/dcps/src/condition.rs` (`QueryCondition`)

**Tests:** `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`,
`crates/dcps/src/topic.rs::tests::cft_param_index_out_of_range_rejected`,
`crates/dcps/tests/multitopic.rs::lookup_topicdescription_finds_a_multitopic`, `crates/dcps/tests/query_condition.rs::read_w_condition_filters_per_sample`

**Status:** done — ContentFilteredTopic, QueryCondition and MultiTopic (SELECT, FROM, NATURAL JOIN, WHERE) are implemented.

### Annex A Persistence profile

**Spec:** Annex A, p. 175 — "Persistence profile: This profile adds the optional
Qos policy DURABILITY_SERVICE as well as the optional settings ‘TRANSIENT’ and
‘PERSISTENT’ of the DURABILITY QoS policy kind."

**Repo:** `crates/dcps/src/durability_service.rs`;
`crates/qos/src/policies/durability_service.rs`

**Tests:** `crates/dcps/tests/durability_e2e_late_joiner.rs::transient_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_e2e_late_joiner.rs::persistent_late_joiner_receives_backend_replay`,
`crates/dcps/tests/durability_service_entities.rs::a_reader_after_the_last_writer_gets_the_stored_samples`

**Status:** done — TRANSIENT and PERSISTENT deliver to late readers, with the fictitious entities of the service, `service_cleanup_delay` and LIFESPAN in the store (§2.2.3.4, §2.2.3.16).

### Annex A Ownership profile

**Spec:** Annex A, p. 175 — "Ownership profile: This profile adds two things
First the optional setting ‘EXCLUSIVE’ of the OWNERSHIP kind. Second support for
the optional OWNERSHIP_STRENGTH policy. Third the ability to set a depth > 1 for
the HISTORY QoS policy."

**Repo:** `crates/dcps/src/instance_tracker.rs`
(`should_accept_sample_under_exclusive_ownership`);
`crates/dcps/src/subscriber.rs` (`DataReader::evict_oldest_of_instance`)

**Tests:** `crates/dcps/tests/exclusive_ownership_take.rs::exclusive_ownership_stronger_writer_takes_over`,
`crates/dcps/tests/reader_history_resource_limits.rs::keep_last_n_keeps_the_newest_n_per_instance`,
`crates/dcps/tests/instance_writers_lifecycle.rs::only_the_owner_disposes_an_exclusive_instance`

**Status:** done — EXCLUSIVE, OWNERSHIP_STRENGTH and HISTORY depth > 1 are implemented, with the change of ownership on loss of liveliness, DEADLINE, unregistering and deletion (§2.2.3.9.2).

### Annex A Group access profile

**Spec:** Annex A, p. 175 — "Group access profile: This profile includes support
for the PRESENTATION access_scope setting of ‘GROUP’".

**Repo:** `crates/dcps/src/group_order.rs`; `crates/dcps/src/coherent_set.rs`

**Tests:**
`crates/dcps/tests/ordered_access_e2e.rs::group_scope_ordered_access_takes_one_sample_at_a_time`,
`crates/dcps/tests/coherent_changes_e2e.rs::group_coherent_reader_sees_the_set_only_after_end_coherent_changes`,
`crates/dcps/tests/entity_deletion.rs::group_ordered_access_orders_the_readers_by_group_sequence_number`

**Status:** done — `begin_access`, `get_datareaders` in GSN order, `end_access` (§2.2.2.5.2.10).

## Annex B Syntax for Queries and Filters

### B.1 Use of the SQL subset

**Spec:** Annex B.1, p. 177 — "A subset of SQL syntax is used in several parts
of the specification: The filter_expression in the ContentFilteredTopic … The
topic_expression in the MultiTopic … The query_expression in the
QueryReadCondition".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the grammar is listed in the
following items.

### B.2 FilterExpression ::= Condition

**Spec:** Annex B.2, p. 177 — "FilterExpression ::= Condition".

**Repo:** `crates/sql-filter/src/parser.rs` (`parse`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_simple_eq`,
`crates/sql-filter/src/parser.rs::tests::parse_rejects_trailing_garbage`

**Status:** done

### B.2 TopicExpression ::= SelectFrom {Where} ‘;’

**Spec:** Annex B.2, p. 177 — "TopicExpression ::= SelectFrom {Where } ‘;’".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::the_annex_b_example_parses`,
`crates/sql-filter/src/topic_expression.rs::tests::anything_else_is_refused`

**Status:** `done` — `zerodds_sql_filter::parse_topic_expression`; the closing `;` is optional, as in the example of Annex B.2; anything else is a `ParseError`.

### B.2 SelectFrom and Aggregation

**Spec:** Annex B.2, pp. 177–178 — "SelectFrom ::= ‘SELECT’ Aggregation ‘FROM’
Selection . Aggregation ::= ‘*’ | (SubjectFieldSpec // ‘,’)".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::star_one_topic_no_where_and_the_closing_semicolon`

**Status:** `done` — `*` and the list of fields.

### B.2 SubjectFieldSpec with AS

**Spec:** Annex B.2, pp. 177–178 — "SubjectFieldSpec ::= FIELDNAME | FIELDNAME
‘AS’ FIELDNAME | FIELDNAME FIELDNAME".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_field_takes_an_alias_with_as_or_juxtaposed`

**Status:** `done` — `FIELDNAME`, `FIELDNAME AS FIELDNAME` and `FIELDNAME FIELDNAME`; two fields of one result name are an error.

### B.2 Selection and JoinItem

**Spec:** Annex B.2, p. 178 — "Selection ::= TOPICNAME | TOPICTNAME NaturalJoin
JoinItem . JoinItem ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem | ‘(’
TOPICNAME NaturalJoin JoinItem ‘)’".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::join_items_may_nest_in_parentheses`,
`crates/sql-filter/src/topic_expression.rs::tests::join_items_nest_to_a_bound`

**Status:** `done` — parenthesized JoinItems too (depth up to 64); a parenthesized item without a join and a topic twice are errors.

### B.2 NaturalJoin and its aliases

**Spec:** Annex B.2, p. 178 — "NaturalJoin ::= ‘INNER NATURAL JOIN’ | ‘NATURAL
JOIN’ | ‘NATURAL INNER JOIN’ … NOTE: INNER NATURAL JOIN, NATURAL JOIN, and
NATURAL INNER JOIN are all aliases".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::the_three_spellings_of_the_natural_join`

**Status:** `done` — `INNER NATURAL JOIN`, `NATURAL JOIN`, `NATURAL INNER JOIN`, in any case.

### B.2 Where

**Spec:** Annex B.2, p. 178 — "Where ::= ‘WHERE’ Condition".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::where_in_a_string_is_not_the_keyword`

**Status:** `done` — what follows `WHERE` is a Condition; `WHERE` inside a string is no keyword.

### B.2 TOPICNAME

**Spec:** Annex B.2, p. 179 — "TOPICNAME - A topic name is an identifier for a
topic, and is defined as any series of characters ‘a’, ..., ‘z’, ‘A’, ..., ‘Z’,
‘0’, ..., ‘9’, ‘-’ but may not start with a digit.".

**Repo:** —

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_topic_name_is_letters_digits_and_hyphen_or_quoted`

**Status:** `done` — letters, digits and `-`, not starting with a digit; any other name in single quotes (as in the example of Annex B.2).

### B.2 QueryExpression — Condition

**Spec:** Annex B.2, p. 177 — "QueryExpression ::= {Condition}{‘ORDER BY’
(FIELDNAME // ‘,’) }".

**Repo:** `crates/dcps/src/condition.rs` (`QueryCondition`:
`zerodds_sql_filter::parse`)

**Tests:** `crates/sql-filter/src/topic_expression.rs::tests::a_query_expression_is_a_condition_and_an_order`

**Status:** `done` — `parse_query_expression`.

### B.2 QueryExpression — ORDER BY

**Spec:** Annex B.2, p. 177 — "{‘ORDER BY’ (FIELDNAME // ‘,’) }".

**Repo:** —

**Tests:** `crates/dcps/tests/query_condition.rs::order_by_orders_what_a_read_or_take_returns`,
`crates/zerodds-c-api/tests/query_condition_read.rs::order_by_orders_what_read_and_take_return`

**Status:** `done` — `ORDER BY` with one or more FIELDNAMEs; the samples a read or take with the QueryCondition returns are then ordered ascending (Rust and C API). The specification names no direction; ascending is the SQL default. In the C API `max_samples` is applied to the sorted selection.

### B.2 Condition

**Spec:** Annex B.2, p. 178 — "Condition ::= Predicate | Condition ‘AND’
Condition | Condition ‘OR’ Condition | ‘NOT’ Condition | ‘(’ Condition ‘)’".

**Repo:** `crates/sql-filter/src/parser.rs` (`parse_or`, `parse_and`,
`parse_not`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_and_or_precedence`,
`crates/sql-filter/src/parser.rs::tests::parse_not`,
`crates/sql-filter/src/parser.rs::tests::parse_parenthesized`,
`crates/sql-filter/src/evaluator.rs::tests::evaluates_boolean_not_or`

**Status:** done

### B.2 Predicate

**Spec:** Annex B.2, p. 178 — "Predicate ::= ComparisonPredicate |
BetweenPredicate".

**Repo:** `crates/sql-filter/src/parser.rs`

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_simple_eq`,
`crates/sql-filter/src/parser.rs::tests::parse_between_predicate`

**Status:** done

### B.2 ComparisonPredicate

**Spec:** Annex B.2, p. 178 — "ComparisonPredicate ::= FIELDNAME RelOp Parameter
| Parameter RelOp FIELDNAME | FIELDNAME RelOp FIELDNAME".

**Repo:** `crates/sql-filter/src/parser.rs` (`cmp = operand cmp_op operand`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_param`,
`crates/sql-filter/src/evaluator.rs::tests::evaluates_int_compare`

**Status:** done — the parser accepts any combination of field, literal and
parameter, also two literals.

### B.2 BetweenPredicate

**Spec:** Annex B.2, p. 178 — "BetweenPredicate ::= FIELDNAME ‘BETWEEN’ Range |
FIELDNAME ‘NOT BETWEEN’ Range".

**Repo:** `crates/sql-filter/src/parser.rs` (`Expr::Between`)

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_between_predicate`,
`crates/sql-filter/src/parser.rs::tests::parse_not_between_predicate`

**Status:** done

### B.2 RelOp

**Spec:** Annex B.2, p. 178 — "RelOp ::= ‘=’ | ‘>’ | ‘>=’ | ‘<’ | ‘<=’ | ‘<>’ |
like".

**Repo:** `crates/sql-filter/src/lexer.rs`; `crates/sql-filter/src/evaluator.rs`
(`like_match`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::lex_mixed_operators`,
`crates/sql-filter/src/evaluator.rs::tests::like_wildcards`

**Status:** done — in addition `!=` as a synonym of `<>`.

### B.2 Range

**Spec:** Annex B.2, p. 178 — "Range ::= Parameter ‘AND’ Parameter".

**Repo:** `crates/sql-filter/src/parser.rs`

**Tests:** `crates/sql-filter/src/parser.rs::tests::parse_between_with_params`

**Status:** done

### B.2 Parameter

**Spec:** Annex B.2, p. 179 — "Parameter ::= INTEGERVALUE | CHARVALUE |
FLOATVALUE | STRING | ENUMERATEDVALUE | PARAMETER".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — an enumeration; the tokens are listed one by
one below.

### B.2 FIELDNAME

**Spec:** Annex B.2, p. 179 — "FIELDNAME - A fieldname is a reference to a field
in the data-structure. The dot ‘.’ is used to navigate through nested structures
… The names of the field are those specified in the IDL definition of the
corresponding structure".

**Repo:** `crates/sql-filter/src/lexer.rs` (identifiers with `.`)

**Tests:** `crates/zerodds-c-api/src/subscriber_ffi.rs::tests::cft_type_with_a_dependency_filters_a_nested_member`,
`crates/sql-filter/src/evaluator.rs::tests::unknown_field_is_error`

**Status:** done

### B.2 INTEGERVALUE — decimal with a sign

**Spec:** Annex B.2, p. 179 — "INTEGERVALUE - Any series of digits, optionally
preceded by a plus or minus sign, representing a decimal integer value within
the range of the system."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::IntLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`

**Status:** `done` — `-` and `+`.

### B.2 INTEGERVALUE — plus sign and 0x

**Spec:** Annex B.2, p. 179 — "optionally preceded by a plus or minus sign … A
hexadecimal number is preceded by 0x and must be a valid hexadecimal
expression."

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`,
`crates/sql-filter/src/lexer.rs::tests::a_malformed_hexadecimal_is_refused`

**Status:** `done` — a `+` before a number and `0x1F`; a malformed hexadecimal is an error.

### B.2 CHARVALUE

**Spec:** Annex B.2, p. 179 — "CHARVALUE - A single character enclosed between
single quotes."

**Repo:** —

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_filter_compares_an_enumerated_and_a_character_member`

**Status:** `done` — a character in single quotes is a string of one character; a character member of a type with a `TypeObject` gives the same.

### B.2 FLOATVALUE

**Spec:** Annex B.2, p. 179 — "FLOATVALUE - Any series of digits, optionally
preceded by a plus or minus sign and optionally including a floating point
(‘.’). A power-of-ten expression may be postfixed, which has the syntax en".

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::FloatLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::integers_take_a_plus_sign_and_hexadecimal`

**Status:** `done` — with `+` too.

### B.2 STRING

**Spec:** Annex B.2, p. 179 — "STRING - Any series of characters encapsulated in
single quotes, except a new-line character or a right quote."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::StrLit`)

**Tests:** `crates/sql-filter/src/lexer.rs::tests::lex_basic_comparison`,
`crates/sql-filter/src/lexer.rs::tests::lex_string_with_escape_quote`

**Status:** `done` — characters in single quotes, `''` as a quote in addition (SQL-92); characters outside ASCII and the new-line: see the next item.

### B.2 STRING — characters outside ASCII and new-line

**Spec:** Annex B.2, p. 179 — "Any series of characters encapsulated in single
quotes, except a new-line character".

**Repo:** —

**Tests:** `crates/sql-filter/src/lexer.rs::tests::a_string_keeps_its_characters_and_refuses_a_new_line`

**Status:** `done` — characters are read as characters, not as bytes; a new-line in a string is an error.

### B.2 ENUMERATEDVALUE

**Spec:** Annex B.2, p. 179 — "ENUMERATEDVALUE - An enumerated value is a
reference to a value declared within an enumeration. Enumerated values consist
of the name of the enumeration label enclosed in single quotes."

**Repo:** —

**Tests:** `crates/dcps/src/serialized_type.rs::tests::a_filter_compares_an_enumerated_and_a_character_member`

**Status:** `done` — a label in single quotes is a string; an enumerated member of a type with a `TypeObject` gives the name of its label. A type that writes `DdsType::field_value` itself gives what it gives.

### B.2 PARAMETER %n, n < 100

**Spec:** Annex B.2, p. 179 — "PARAMETER - A parameter is of the form %n, where
n represents a natural number (zero included) smaller than 100. It refers to the
n + 1 th argument in the given context."

**Repo:** `crates/sql-filter/src/lexer.rs` (`Token::Param`);
`crates/sql-filter/src/evaluator.rs`

**Tests:** `crates/sql-filter/src/evaluator.rs::tests::evaluates_param`,
`crates/sql-filter/src/lexer.rs::tests::lex_rejects_bad_parameter`

**Status:** done — `%0` is the first argument; the lexer also accepts indices
from 100 on.

### B.2 Examples

**Spec:** Annex B.2, pp. 179–180 — "Example of a topic_expression: “SELECT
flight_name, x, y, z AS height FROM ‘Location’ NATURAL JOIN ‘FlightPlan’ WHERE
height < 1000 AND x <23” Example of a query_expression or a filter_expression:
“height < 1000 AND x <23”".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — examples.

---

## Audit-Status

796 done / 0 partial / 0 open / 63 n/a (informative) / 0 n/a (rejected).

Test run: to be added on the Linux test host.

