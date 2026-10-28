# `zerodds-c-api` v1.0 — cross-language C FFI spec

ZeroDDS vendor spec. Implemented in `crates/zerodds-c-api`; the C header
`include/zerodds.h` is the normative contract document.

## Motivation

There is no OMG DDS C PSM spec. RTI Connext, Cyclone and FastDDS
each have their own C APIs without standard compatibility. ZeroDDS
specifies here a **complete spec-conformant C FFI** as the
foundation for the tier-1 language bindings (C++, C#) as well as embedded C
and the ROS-2 RMW.

## Goals

- **Completeness:** every DDS spec §2.2.2 operation has a C FFI
  counterpart. No omissions, no shortcuts.
- **Cross-language hub:** C++ (`crates/cpp/`), C# (`crates/cs/`),
  embedded C, ROS-2 RMW consume exclusively this header.
- **Stable wire form:** ABI-stable structures for QoS, SampleInfo,
  status; opaque handles for the entity lifecycle.
- **Explicit memory ownership:** the caller pairs `*_create` with `*_destroy`,
  `*_take` with `*_buffer_free` etc.

## Non-goals

- Java + Python: their own bridges (`grpc-bridge`-based / `pyo3`),
  not via this header.
- Generic type acrobatics through the FFI: the C FFI is byte-oriented.
- QoS builder with complex default logic: the C FFI offers
  all fields flat via get/set pairs.

## §1 Architecture

```
                    ┌─────────────────┐
                    │ DomainParticipantFactory │
                    └────────┬────────┘
                             │
                       ┌─────▼─────┐
                       │DomainParticipant│
                       └─────┬─────┘
            ┌────────────────┼────────────────┐
            │                │                │
       ┌────▼────┐     ┌─────▼────┐     ┌─────▼────┐
       │ Topic<T>│     │ Publisher │     │Subscriber│
       └─────────┘     └────┬─────┘     └────┬─────┘
                            │                │
                       ┌────▼─────┐     ┌────▼────┐
                       │DataWriter│     │DataReader│
                       └──────────┘     └─────────┘
```

Every entity is an opaque handle (`zerodds_*`). Lifetime via a
`*_create`/`*_destroy` pair.

## §2 Entity hierarchy

### §2.1 DomainParticipantFactory (Singleton)

```c
zerodds_DomainParticipantFactory* zerodds_dpf_get_instance(void);
zerodds_DomainParticipant* zerodds_dpf_create_participant(
    zerodds_DomainParticipantFactory* f, uint32_t domain_id,
    const zerodds_DomainParticipantQos* qos);
int zerodds_dpf_delete_participant(
    zerodds_DomainParticipantFactory* f, zerodds_DomainParticipant* p);
zerodds_DomainParticipant* zerodds_dpf_lookup_participant(
    zerodds_DomainParticipantFactory* f, uint32_t domain_id);
int zerodds_dpf_get_default_participant_qos(
    zerodds_DomainParticipantFactory* f, zerodds_DomainParticipantQos* out);
int zerodds_dpf_set_default_participant_qos(
    zerodds_DomainParticipantFactory* f, const zerodds_DomainParticipantQos* qos);
int zerodds_dpf_get_qos(
    zerodds_DomainParticipantFactory* f, zerodds_DomainParticipantFactoryQos* out);
int zerodds_dpf_set_qos(
    zerodds_DomainParticipantFactory* f, const zerodds_DomainParticipantFactoryQos* qos);
```

### §2.2 DomainParticipant

```c
zerodds_Topic* zerodds_dp_create_topic(
    zerodds_DomainParticipant* p, const char* name, const char* type_name,
    const zerodds_TopicQos* qos);
int zerodds_dp_delete_topic(zerodds_DomainParticipant* p, zerodds_Topic* t);
zerodds_Topic* zerodds_dp_find_topic(zerodds_DomainParticipant* p, const char* name);
zerodds_Publisher* zerodds_dp_create_publisher(
    zerodds_DomainParticipant* p, const zerodds_PublisherQos* qos);
int zerodds_dp_delete_publisher(zerodds_DomainParticipant* p, zerodds_Publisher* pub);
zerodds_Subscriber* zerodds_dp_create_subscriber(
    zerodds_DomainParticipant* p, const zerodds_SubscriberQos* qos);
int zerodds_dp_delete_subscriber(zerodds_DomainParticipant* p, zerodds_Subscriber* sub);
zerodds_Subscriber* zerodds_dp_get_builtin_subscriber(zerodds_DomainParticipant* p);
zerodds_ContentFilteredTopic* zerodds_dp_create_contentfilteredtopic(
    zerodds_DomainParticipant* p, const char* name, zerodds_Topic* related,
    const char* filter_expression, const char** parameters, uintptr_t param_count);
int zerodds_dp_delete_contentfilteredtopic(
    zerodds_DomainParticipant* p, zerodds_ContentFilteredTopic* cft);
int zerodds_dp_ignore_participant(zerodds_DomainParticipant* p, uint64_t handle);
int zerodds_dp_ignore_topic(zerodds_DomainParticipant* p, uint64_t handle);
int zerodds_dp_ignore_publication(zerodds_DomainParticipant* p, uint64_t handle);
int zerodds_dp_ignore_subscription(zerodds_DomainParticipant* p, uint64_t handle);
uint32_t zerodds_dp_get_domain_id(zerodds_DomainParticipant* p);
int zerodds_dp_assert_liveliness(zerodds_DomainParticipant* p);
int zerodds_dp_get_current_time(zerodds_DomainParticipant* p, zerodds_Time* out);
int zerodds_dp_get_qos(zerodds_DomainParticipant* p, zerodds_DomainParticipantQos* out);
int zerodds_dp_set_qos(zerodds_DomainParticipant* p, const zerodds_DomainParticipantQos* qos);
int zerodds_dp_get_default_topic_qos(zerodds_DomainParticipant* p, zerodds_TopicQos* out);
int zerodds_dp_set_default_topic_qos(zerodds_DomainParticipant* p, const zerodds_TopicQos* qos);
int zerodds_dp_get_default_publisher_qos(zerodds_DomainParticipant* p, zerodds_PublisherQos* out);
int zerodds_dp_set_default_publisher_qos(zerodds_DomainParticipant* p, const zerodds_PublisherQos* qos);
int zerodds_dp_get_default_subscriber_qos(zerodds_DomainParticipant* p, zerodds_SubscriberQos* out);
int zerodds_dp_set_default_subscriber_qos(zerodds_DomainParticipant* p, const zerodds_SubscriberQos* qos);
int zerodds_dp_delete_contained_entities(zerodds_DomainParticipant* p);
int zerodds_dp_get_discovered_participants(
    zerodds_DomainParticipant* p, uint64_t* out_handles, uintptr_t* out_count, uintptr_t cap);
int zerodds_dp_get_discovered_topics(
    zerodds_DomainParticipant* p, uint64_t* out_handles, uintptr_t* out_count, uintptr_t cap);
int zerodds_dp_get_discovered_participant_data(
    zerodds_DomainParticipant* p, uint64_t handle, zerodds_ParticipantBuiltinTopicData* out);
int zerodds_dp_get_discovered_topic_data(
    zerodds_DomainParticipant* p, uint64_t handle, zerodds_TopicBuiltinTopicData* out);
int zerodds_dp_contains_entity(zerodds_DomainParticipant* p, uint64_t handle);
```

### §2.3 Publisher

```c
zerodds_DataWriter* zerodds_pub_create_datawriter(
    zerodds_Publisher* pub, zerodds_Topic* topic, const zerodds_DataWriterQos* qos);
int zerodds_pub_delete_datawriter(zerodds_Publisher* pub, zerodds_DataWriter* dw);
zerodds_DataWriter* zerodds_pub_lookup_datawriter(zerodds_Publisher* pub, const char* topic_name);
int zerodds_pub_suspend_publications(zerodds_Publisher* pub);
int zerodds_pub_resume_publications(zerodds_Publisher* pub);
int zerodds_pub_begin_coherent_changes(zerodds_Publisher* pub);
int zerodds_pub_end_coherent_changes(zerodds_Publisher* pub);
int zerodds_pub_wait_for_acknowledgments(zerodds_Publisher* pub, const zerodds_Duration* timeout);
int zerodds_pub_get_qos(zerodds_Publisher* pub, zerodds_PublisherQos* out);
int zerodds_pub_set_qos(zerodds_Publisher* pub, const zerodds_PublisherQos* qos);
int zerodds_pub_get_default_datawriter_qos(zerodds_Publisher* pub, zerodds_DataWriterQos* out);
int zerodds_pub_set_default_datawriter_qos(zerodds_Publisher* pub, const zerodds_DataWriterQos* qos);
int zerodds_pub_copy_from_topic_qos(
    zerodds_Publisher* pub, zerodds_DataWriterQos* dwqos_inout, const zerodds_TopicQos* tqos);
int zerodds_pub_delete_contained_entities(zerodds_Publisher* pub);
zerodds_DomainParticipant* zerodds_pub_get_participant(zerodds_Publisher* pub);
```

### §2.4 Subscriber

```c
zerodds_DataReader* zerodds_sub_create_datareader(
    zerodds_Subscriber* sub, zerodds_TopicDescription* td, const zerodds_DataReaderQos* qos);
int zerodds_sub_delete_datareader(zerodds_Subscriber* sub, zerodds_DataReader* dr);
zerodds_DataReader* zerodds_sub_lookup_datareader(zerodds_Subscriber* sub, const char* topic_name);
int zerodds_sub_begin_access(zerodds_Subscriber* sub);
int zerodds_sub_end_access(zerodds_Subscriber* sub);
int zerodds_sub_get_datareaders(
    zerodds_Subscriber* sub, zerodds_DataReader** out, uintptr_t* out_count, uintptr_t cap,
    uint32_t sample_states, uint32_t view_states, uint32_t instance_states);
int zerodds_sub_notify_datareaders(zerodds_Subscriber* sub);
int zerodds_sub_get_qos(zerodds_Subscriber* sub, zerodds_SubscriberQos* out);
int zerodds_sub_set_qos(zerodds_Subscriber* sub, const zerodds_SubscriberQos* qos);
int zerodds_sub_get_default_datareader_qos(zerodds_Subscriber* sub, zerodds_DataReaderQos* out);
int zerodds_sub_set_default_datareader_qos(zerodds_Subscriber* sub, const zerodds_DataReaderQos* qos);
int zerodds_sub_copy_from_topic_qos(
    zerodds_Subscriber* sub, zerodds_DataReaderQos* drqos_inout, const zerodds_TopicQos* tqos);
int zerodds_sub_delete_contained_entities(zerodds_Subscriber* sub);
zerodds_DomainParticipant* zerodds_sub_get_participant(zerodds_Subscriber* sub);
```

### §2.5 Topic

```c
int zerodds_topic_get_qos(zerodds_Topic* t, zerodds_TopicQos* out);
int zerodds_topic_set_qos(zerodds_Topic* t, const zerodds_TopicQos* qos);
int zerodds_topic_get_inconsistent_topic_status(
    zerodds_Topic* t, zerodds_InconsistentTopicStatus* out);
const char* zerodds_topic_get_name(zerodds_Topic* t);
const char* zerodds_topic_get_type_name(zerodds_Topic* t);
zerodds_DomainParticipant* zerodds_topic_get_participant(zerodds_Topic* t);
```

### §2.6 DataWriter

```c
int zerodds_dw_register_instance(zerodds_DataWriter* dw, const uint8_t* key, uintptr_t key_len, uint64_t* out_handle);
int zerodds_dw_register_instance_w_timestamp(
    zerodds_DataWriter* dw, const uint8_t* key, uintptr_t key_len, const zerodds_Time* ts, uint64_t* out_handle);
int zerodds_dw_unregister_instance(zerodds_DataWriter* dw, uint64_t handle);
int zerodds_dw_unregister_instance_w_timestamp(zerodds_DataWriter* dw, uint64_t handle, const zerodds_Time* ts);
int zerodds_dw_get_key_value(zerodds_DataWriter* dw, uint64_t handle, uint8_t* out_buf, uintptr_t* inout_len);
int zerodds_dw_lookup_instance(zerodds_DataWriter* dw, const uint8_t* key, uintptr_t key_len, uint64_t* out_handle);
int zerodds_dw_write(zerodds_DataWriter* dw, const uint8_t* payload, uintptr_t len, uint64_t handle);
int zerodds_dw_write_w_timestamp(zerodds_DataWriter* dw, const uint8_t* payload, uintptr_t len, uint64_t handle, const zerodds_Time* ts);
int zerodds_dw_dispose(zerodds_DataWriter* dw, uint64_t handle);
int zerodds_dw_dispose_w_timestamp(zerodds_DataWriter* dw, uint64_t handle, const zerodds_Time* ts);
int zerodds_dw_wait_for_acknowledgments(zerodds_DataWriter* dw, const zerodds_Duration* timeout);
int zerodds_dw_assert_liveliness(zerodds_DataWriter* dw);
int zerodds_dw_get_qos(zerodds_DataWriter* dw, zerodds_DataWriterQos* out);
int zerodds_dw_set_qos(zerodds_DataWriter* dw, const zerodds_DataWriterQos* qos);
int zerodds_dw_get_liveliness_lost_status(zerodds_DataWriter* dw, zerodds_LivelinessLostStatus* out);
int zerodds_dw_get_offered_deadline_missed_status(zerodds_DataWriter* dw, zerodds_OfferedDeadlineMissedStatus* out);
int zerodds_dw_get_offered_incompatible_qos_status(zerodds_DataWriter* dw, zerodds_OfferedIncompatibleQosStatus* out);
int zerodds_dw_get_publication_matched_status(zerodds_DataWriter* dw, zerodds_PublicationMatchedStatus* out);
int zerodds_dw_get_matched_subscriptions(zerodds_DataWriter* dw, uint64_t* out, uintptr_t* out_count, uintptr_t cap);
int zerodds_dw_get_matched_subscription_data(zerodds_DataWriter* dw, uint64_t h, zerodds_SubscriptionBuiltinTopicData* out);
zerodds_Topic* zerodds_dw_get_topic(zerodds_DataWriter* dw);
zerodds_Publisher* zerodds_dw_get_publisher(zerodds_DataWriter* dw);

// Loan-API (Zero-Copy)
int zerodds_dw_loan_message(zerodds_DataWriter* dw, uintptr_t len, uint8_t** out_ptr, uintptr_t* out_len);
int zerodds_dw_commit_loan(zerodds_DataWriter* dw, uint8_t* ptr, uintptr_t len);
int zerodds_dw_discard_loan(zerodds_DataWriter* dw, uint8_t* ptr, uintptr_t len);
int zerodds_dw_wait_for_matched(zerodds_DataWriter* dw, int min, uint64_t timeout_ms);
```

`zerodds_dw_set_qos` and `zerodds_dr_set_qos` follow the column
"Changeable" of DDS 1.4 §2.2.3: a change to an immutable
policy (DURABILITY, RELIABILITY, HISTORY, RESOURCE_LIMITS, OWNERSHIP,
LIVELINESS, DESTINATION_ORDER, PRESENTATION, DURABILITY_SERVICE,
DATA_REPRESENTATION, for the reader TYPE_CONSISTENCY_ENFORCEMENT) returns
`ZERODDS_IMMUTABLE_POLICY` and leaves the QoS as it was; a
changeable policy reaches the runtime and is announced again.
TRANSPORT_PRIORITY sets the IPv4 TOS octet or the IPv6
traffic class of the writer's datagrams (1 to 255; 0 is none).

### §2.7 DataReader

```c
int zerodds_dr_read(zerodds_DataReader* dr, zerodds_SampleArray* out, uintptr_t max_samples,
                    uint32_t sample_states, uint32_t view_states, uint32_t instance_states);
int zerodds_dr_take(zerodds_DataReader* dr, zerodds_SampleArray* out, uintptr_t max_samples,
                    uint32_t sample_states, uint32_t view_states, uint32_t instance_states);
int zerodds_dr_read_w_condition(zerodds_DataReader* dr, zerodds_SampleArray* out, uintptr_t max,
                                 zerodds_ReadCondition* cond);
int zerodds_dr_take_w_condition(zerodds_DataReader* dr, zerodds_SampleArray* out, uintptr_t max,
                                 zerodds_ReadCondition* cond);
int zerodds_dr_read_next_sample(zerodds_DataReader* dr, uint8_t** out_buf, uintptr_t* out_len, zerodds_SampleInfo* out_info);
int zerodds_dr_take_next_sample(zerodds_DataReader* dr, uint8_t** out_buf, uintptr_t* out_len, zerodds_SampleInfo* out_info);
int zerodds_dr_read_instance(zerodds_DataReader* dr, uint64_t handle, zerodds_SampleArray* out, uintptr_t max,
                              uint32_t sample_states, uint32_t view_states, uint32_t instance_states);
int zerodds_dr_take_instance(zerodds_DataReader* dr, uint64_t handle, zerodds_SampleArray* out, uintptr_t max,
                              uint32_t sample_states, uint32_t view_states, uint32_t instance_states);
int zerodds_dr_read_next_instance(zerodds_DataReader* dr, uint64_t prev_handle, zerodds_SampleArray* out, uintptr_t max,
                                   uint32_t s, uint32_t v, uint32_t i);
int zerodds_dr_take_next_instance(zerodds_DataReader* dr, uint64_t prev_handle, zerodds_SampleArray* out, uintptr_t max,
                                   uint32_t s, uint32_t v, uint32_t i);
int zerodds_dr_return_loan(zerodds_DataReader* dr, zerodds_SampleArray* arr);
int zerodds_dr_get_key_value(zerodds_DataReader* dr, uint64_t handle, uint8_t* out_buf, uintptr_t* inout_len);
int zerodds_dr_lookup_instance(zerodds_DataReader* dr, const uint8_t* key, uintptr_t key_len, uint64_t* out_handle);
zerodds_ReadCondition* zerodds_dr_create_readcondition(zerodds_DataReader* dr, uint32_t s, uint32_t v, uint32_t i);
zerodds_QueryCondition* zerodds_dr_create_querycondition(
    zerodds_DataReader* dr, uint32_t s, uint32_t v, uint32_t i,
    const char* expr, const char** params, uintptr_t param_count);
int zerodds_dr_delete_readcondition(zerodds_DataReader* dr, zerodds_ReadCondition* c);
int zerodds_dr_get_liveliness_changed_status(zerodds_DataReader* dr, zerodds_LivelinessChangedStatus* out);
int zerodds_dr_get_requested_deadline_missed_status(zerodds_DataReader* dr, zerodds_RequestedDeadlineMissedStatus* out);
int zerodds_dr_get_requested_incompatible_qos_status(zerodds_DataReader* dr, zerodds_RequestedIncompatibleQosStatus* out);
int zerodds_dr_get_sample_lost_status(zerodds_DataReader* dr, zerodds_SampleLostStatus* out);
int zerodds_dr_get_sample_rejected_status(zerodds_DataReader* dr, zerodds_SampleRejectedStatus* out);
int zerodds_dr_get_subscription_matched_status(zerodds_DataReader* dr, zerodds_SubscriptionMatchedStatus* out);
int zerodds_dr_wait_for_historical_data(zerodds_DataReader* dr, const zerodds_Duration* timeout);
int zerodds_dr_get_matched_publications(zerodds_DataReader* dr, uint64_t* out, uintptr_t* out_count, uintptr_t cap);
int zerodds_dr_get_matched_publication_data(zerodds_DataReader* dr, uint64_t h, zerodds_PublicationBuiltinTopicData* out);
zerodds_TopicDescription* zerodds_dr_get_topicdescription(zerodds_DataReader* dr);
zerodds_Subscriber* zerodds_dr_get_subscriber(zerodds_DataReader* dr);
int zerodds_dr_get_qos(zerodds_DataReader* dr, zerodds_DataReaderQos* out);
int zerodds_dr_set_qos(zerodds_DataReader* dr, const zerodds_DataReaderQos* qos);
int zerodds_dr_wait_for_matched(zerodds_DataReader* dr, int min, uint64_t timeout_ms);
```

## §2.8 List calls

`zerodds_dp_get_discovered_participants`, `zerodds_dp_get_discovered_topics`,
`zerodds_dw_get_matched_subscriptions`, `zerodds_dr_get_matched_publications`,
`zerodds_sub_get_datareaders`, `zerodds_waitset_get_conditions` and the list
of active conditions from `zerodds_waitset_wait` copy a list into a
caller buffer with `cap` slots. The same contract applies to all seven:

- The first `min(len, cap)` entries are written into `out` (`len` is
  the total length of the list). `*out_count` always receives `len`, not the number
  of copied entries.
- The return value is `ZERODDS_OUT_OF_RESOURCES` if `len > cap`; otherwise
  `ZERODDS_OK`. A caller that receives `ZERODDS_OUT_OF_RESOURCES` reads the
  total length from `*out_count` and repeats the call with a buffer
  of that size until it sees `ZERODDS_OK`. `zerodds_waitset_wait` instead
  returns `ZERODDS_TIMEOUT` if no condition has triggered within the
  deadline.
- `cap == 0` with `out == NULL` is the length query: the call writes
  nothing, sets `*out_count` to `len` and returns `ZERODDS_OUT_OF_RESOURCES`
  if `len > 0`, otherwise `ZERODDS_OK`. `out` may be NULL exactly when
  `cap == 0`; a NULL `out` with `cap > 0` is `ZERODDS_BAD_PARAMETER`.

## §3 QoS structures (all 22)

All QoS structures are exposed as `#[repr(C)]` `struct` with
exact field layouts. The caller fills them in directly, no builders.

```c
typedef struct { int32_t sec; uint32_t nanosec; } zerodds_Time;
typedef struct { int32_t sec; uint32_t nanosec; } zerodds_Duration;

typedef enum { ZERODDS_RELIABILITY_BEST_EFFORT, ZERODDS_RELIABILITY_RELIABLE } zerodds_ReliabilityKind;
typedef struct { zerodds_ReliabilityKind kind; zerodds_Duration max_blocking_time; } zerodds_ReliabilityQosPolicy;

typedef enum { ZERODDS_DURABILITY_VOLATILE, ZERODDS_DURABILITY_TRANSIENT_LOCAL,
              ZERODDS_DURABILITY_TRANSIENT, ZERODDS_DURABILITY_PERSISTENT } zerodds_DurabilityKind;
typedef struct { zerodds_DurabilityKind kind; } zerodds_DurabilityQosPolicy;

typedef enum { ZERODDS_HISTORY_KEEP_LAST, ZERODDS_HISTORY_KEEP_ALL } zerodds_HistoryKind;
typedef struct { zerodds_HistoryKind kind; int32_t depth; } zerodds_HistoryQosPolicy;

typedef struct { int32_t max_samples, max_instances, max_samples_per_instance; } zerodds_ResourceLimitsQosPolicy;
typedef struct { zerodds_Duration period; } zerodds_DeadlineQosPolicy;
typedef struct { zerodds_Duration duration; } zerodds_LatencyBudgetQosPolicy;

typedef enum { ZERODDS_OWNERSHIP_SHARED, ZERODDS_OWNERSHIP_EXCLUSIVE } zerodds_OwnershipKind;
typedef struct { zerodds_OwnershipKind kind; } zerodds_OwnershipQosPolicy;
typedef struct { int32_t value; } zerodds_OwnershipStrengthQosPolicy;

typedef enum { ZERODDS_LIVELINESS_AUTOMATIC, ZERODDS_LIVELINESS_MANUAL_BY_PARTICIPANT,
              ZERODDS_LIVELINESS_MANUAL_BY_TOPIC } zerodds_LivelinessKind;
typedef struct { zerodds_LivelinessKind kind; zerodds_Duration lease_duration; } zerodds_LivelinessQosPolicy;

typedef struct { zerodds_Duration duration; } zerodds_LifespanQosPolicy;

typedef enum { ZERODDS_DESTINATION_ORDER_BY_RECEPTION, ZERODDS_DESTINATION_ORDER_BY_SOURCE } zerodds_DestinationOrderKind;
typedef struct { zerodds_DestinationOrderKind kind; } zerodds_DestinationOrderQosPolicy;

typedef enum { ZERODDS_PRESENTATION_INSTANCE, ZERODDS_PRESENTATION_TOPIC, ZERODDS_PRESENTATION_GROUP } zerodds_PresentationAccessScope;
typedef struct { zerodds_PresentationAccessScope access_scope; bool coherent_access; bool ordered_access; } zerodds_PresentationQosPolicy;

typedef struct { const char** names; uintptr_t count; } zerodds_PartitionQosPolicy;
typedef struct { const char* name; const char* value; uint8_t propagate; } zerodds_PropertyEntry;
typedef struct { const char* name; const uint8_t* value; uintptr_t value_len; uint8_t propagate; } zerodds_BinaryPropertyEntry;
typedef struct { const zerodds_PropertyEntry* entries; uintptr_t entries_len;
                 const zerodds_BinaryPropertyEntry* binary_entries; uintptr_t binary_entries_len; } zerodds_PropertyQosPolicy;
typedef struct { const char* name; const char* value; } zerodds_Tag;
typedef struct { const zerodds_Tag* tags; uintptr_t tags_len; } zerodds_DataTagQosPolicy;
typedef struct { const uint8_t* data; uintptr_t len; } zerodds_UserDataQosPolicy;
typedef struct { const uint8_t* data; uintptr_t len; } zerodds_TopicDataQosPolicy;
typedef struct { const uint8_t* data; uintptr_t len; } zerodds_GroupDataQosPolicy;
typedef struct { zerodds_Duration min_separation; } zerodds_TimeBasedFilterQosPolicy;

typedef enum { ZERODDS_RDL_NO_AUTOPURGE = 0,
              ZERODDS_RDL_AUTOPURGE_NOWRITER_SAMPLES,
              ZERODDS_RDL_AUTOPURGE_DISPOSED_SAMPLES } zerodds_ReaderDataLifecycleKind;
typedef struct { zerodds_Duration autopurge_nowriter; zerodds_Duration autopurge_disposed; } zerodds_ReaderDataLifecycleQosPolicy;

typedef struct { bool autodispose_unregistered_instances; } zerodds_WriterDataLifecycleQosPolicy;

typedef struct { zerodds_Duration service_cleanup_delay; uint8_t history_kind; int32_t history_depth;
                 zerodds_ResourceLimitsQosPolicy resource_limits; } zerodds_DurabilityServiceQosPolicy;

typedef struct { bool entity_factory_autoenable; } zerodds_EntityFactoryQosPolicy;

typedef struct { uint8_t representation_id; const uint8_t* extra; uintptr_t extra_len; } zerodds_DataRepresentationQosPolicy;

typedef struct { uint8_t kind; bool ignore_sequence_bounds; bool ignore_string_bounds;
                 bool ignore_member_names; bool prevent_type_widening;
                 bool force_type_validation; } zerodds_TypeConsistencyEnforcementQosPolicy;
```

Composite structures (all `#[repr(C)]`):

```c
typedef struct { zerodds_UserDataQosPolicy user_data; zerodds_EntityFactoryQosPolicy entity_factory; } zerodds_DomainParticipantQos;
typedef struct { zerodds_EntityFactoryQosPolicy entity_factory; bool autoenable_created_entities; } zerodds_DomainParticipantFactoryQos;
typedef struct {
    zerodds_TopicDataQosPolicy topic_data; zerodds_DurabilityQosPolicy durability;
    zerodds_DurabilityServiceQosPolicy durability_service; zerodds_DeadlineQosPolicy deadline;
    zerodds_LatencyBudgetQosPolicy latency_budget; zerodds_LivelinessQosPolicy liveliness;
    zerodds_ReliabilityQosPolicy reliability; zerodds_DestinationOrderQosPolicy destination_order;
    zerodds_HistoryQosPolicy history; zerodds_ResourceLimitsQosPolicy resource_limits;
    zerodds_OwnershipQosPolicy ownership; zerodds_LifespanQosPolicy lifespan;
    zerodds_DataRepresentationQosPolicy data_representation; zerodds_TypeConsistencyEnforcementQosPolicy type_consistency;
} zerodds_TopicQos;
typedef struct {
    zerodds_PresentationQosPolicy presentation; zerodds_PartitionQosPolicy partition;
    zerodds_GroupDataQosPolicy group_data; zerodds_EntityFactoryQosPolicy entity_factory;
} zerodds_PublisherQos, zerodds_SubscriberQos;
typedef struct {
    zerodds_DurabilityQosPolicy durability; zerodds_DurabilityServiceQosPolicy durability_service;
    zerodds_DeadlineQosPolicy deadline; zerodds_LatencyBudgetQosPolicy latency_budget;
    zerodds_LivelinessQosPolicy liveliness; zerodds_ReliabilityQosPolicy reliability;
    zerodds_DestinationOrderQosPolicy destination_order; zerodds_HistoryQosPolicy history;
    zerodds_ResourceLimitsQosPolicy resource_limits; zerodds_TopicDataQosPolicy topic_data;
    zerodds_UserDataQosPolicy user_data; zerodds_OwnershipQosPolicy ownership;
    zerodds_OwnershipStrengthQosPolicy ownership_strength; zerodds_WriterDataLifecycleQosPolicy writer_data_lifecycle;
    zerodds_LifespanQosPolicy lifespan; zerodds_DataRepresentationQosPolicy data_representation;
    zerodds_TypeConsistencyEnforcementQosPolicy type_consistency;
    zerodds_PropertyQosPolicy property; zerodds_DataTagQosPolicy data_tags;
} zerodds_DataWriterQos;
typedef struct {
    zerodds_DurabilityQosPolicy durability; zerodds_DeadlineQosPolicy deadline;
    zerodds_LatencyBudgetQosPolicy latency_budget; zerodds_LivelinessQosPolicy liveliness;
    zerodds_ReliabilityQosPolicy reliability; zerodds_DestinationOrderQosPolicy destination_order;
    zerodds_HistoryQosPolicy history; zerodds_ResourceLimitsQosPolicy resource_limits;
    zerodds_TopicDataQosPolicy topic_data; zerodds_UserDataQosPolicy user_data;
    zerodds_OwnershipQosPolicy ownership; zerodds_TimeBasedFilterQosPolicy time_based_filter;
    zerodds_ReaderDataLifecycleQosPolicy reader_data_lifecycle;
    zerodds_DataRepresentationQosPolicy data_representation; zerodds_TypeConsistencyEnforcementQosPolicy type_consistency;
    zerodds_PropertyQosPolicy property; zerodds_DataTagQosPolicy data_tags;
} zerodds_DataReaderQos;
```

## §4 Status structures

```c
typedef struct { int32_t total_count; int32_t total_count_change; uint64_t last_instance_handle; } zerodds_InconsistentTopicStatus;
typedef struct { int32_t total_count; int32_t total_count_change; uint64_t last_instance_handle; } zerodds_SampleLostStatus;
typedef struct { int32_t total_count; int32_t total_count_change; uint8_t last_reason;
                 uint64_t last_instance_handle; } zerodds_SampleRejectedStatus;
typedef struct { int32_t total_count; int32_t total_count_change; uint64_t last_publication_handle; } zerodds_LivelinessLostStatus;
typedef struct { int32_t alive_count, not_alive_count, alive_count_change, not_alive_count_change;
                 uint64_t last_publication_handle; } zerodds_LivelinessChangedStatus;
typedef struct { int32_t total_count; int32_t total_count_change; uint64_t last_instance_handle; } zerodds_RequestedDeadlineMissedStatus;
typedef struct { int32_t total_count; int32_t total_count_change; uint64_t last_instance_handle; } zerodds_OfferedDeadlineMissedStatus;
typedef struct { int32_t total_count; int32_t total_count_change; int32_t last_policy_id;
                 const int32_t* policies; uintptr_t policies_count; } zerodds_RequestedIncompatibleQosStatus,
                                                                          zerodds_OfferedIncompatibleQosStatus;
typedef struct { int32_t total_count; int32_t total_count_change; int32_t current_count, current_count_change;
                 uint64_t last_publication_handle; } zerodds_SubscriptionMatchedStatus;
typedef struct { int32_t total_count; int32_t total_count_change; int32_t current_count, current_count_change;
                 uint64_t last_subscription_handle; } zerodds_PublicationMatchedStatus;
```

The `zerodds_*_get_*_status` functions read the status as in DDS 1.4
§2.2.4.1: `total_count` is cumulative, `current_count` is the current state,
the `*_change` fields count since the last read or the last
hand-over to a listener, and a read resets them. The
matched statuses name in the last handle the endpoint that most recently
joined or left, the deadline statuses the instance whose deadline
was most recently missed: the deadline is monitored per instance.

## §5 Sample API

```c
typedef struct {
    uint32_t sample_state;        // 1=READ, 2=NOT_READ
    uint32_t view_state;          // 1=NEW, 2=NOT_NEW
    uint32_t instance_state;      // 1=ALIVE, 2=NOT_ALIVE_DISPOSED, 4=NOT_ALIVE_NO_WRITERS
    int32_t  disposed_generation_count;
    int32_t  no_writers_generation_count;
    int32_t  sample_rank;
    int32_t  generation_rank;
    int32_t  absolute_generation_rank;
    zerodds_Time source_timestamp;
    uint64_t instance_handle;
    uint64_t publication_handle;
    bool     valid_data;
} zerodds_SampleInfo;

typedef struct {
    uint8_t** buffers;     // payload per sample (CDR bytes)
    uintptr_t* lengths;    // per sample
    zerodds_SampleInfo* infos;
    uintptr_t count;
    void* loan_token;      // internal; released via zerodds_dr_return_loan
} zerodds_SampleArray;
```

## §6 Conditions + WaitSet

```c
typedef struct zerodds_GuardCondition zerodds_GuardCondition;
typedef struct zerodds_StatusCondition zerodds_StatusCondition;
typedef struct zerodds_ReadCondition zerodds_ReadCondition;
typedef struct zerodds_QueryCondition zerodds_QueryCondition;
typedef struct zerodds_WaitSet zerodds_WaitSet;

zerodds_GuardCondition* zerodds_guardcondition_create(void);
int zerodds_guardcondition_set_trigger_value(zerodds_GuardCondition* c, bool v);
bool zerodds_condition_get_trigger_value(void* c);
zerodds_StatusCondition* zerodds_entity_get_statuscondition(void* entity);
int zerodds_statuscondition_set_enabled_statuses(zerodds_StatusCondition* c, uint32_t mask);

zerodds_WaitSet* zerodds_waitset_create(void);
void zerodds_waitset_destroy(zerodds_WaitSet* w);
int zerodds_waitset_attach_condition(zerodds_WaitSet* w, void* cond);
int zerodds_waitset_detach_condition(zerodds_WaitSet* w, void* cond);
int zerodds_waitset_wait(zerodds_WaitSet* w, void** out_active, uintptr_t cap, uintptr_t* out_count, const zerodds_Duration* timeout);
int zerodds_waitset_get_conditions(zerodds_WaitSet* w, void** out, uintptr_t cap, uintptr_t* out_count);
```

## §7 Built-in Topic Data

```c
typedef struct {
    uint8_t  guid[16];
    const uint8_t* user_data;      // release with zerodds_builtin_userdata_free
    uintptr_t user_data_len;
} zerodds_ZeroDdsParticipantBuiltinTopicData;

typedef struct {
    uint8_t  key[16];              // synthesized from topic and type name
    char*    name;
    char*    type_name;
    zerodds_ZeroDdsDurabilityQosPolicy        durability;
    zerodds_ZeroDdsDurabilityServiceQosPolicy durability_service;
    zerodds_ZeroDdsDeadlineQosPolicy          deadline;
    zerodds_ZeroDdsLatencyBudgetQosPolicy     latency_budget;
    zerodds_ZeroDdsLivelinessQosPolicy        liveliness;
    zerodds_ZeroDdsReliabilityQosPolicy       reliability;
    zerodds_ZeroDdsTransportPriorityQosPolicy transport_priority;
    zerodds_ZeroDdsLifespanQosPolicy          lifespan;
    zerodds_ZeroDdsDestinationOrderQosPolicy  destination_order;
    zerodds_ZeroDdsHistoryQosPolicy           history;
    zerodds_ZeroDdsResourceLimitsQosPolicy    resource_limits;
    zerodds_ZeroDdsOwnershipQosPolicy         ownership;
    zerodds_ZeroDdsTopicDataQosPolicy         topic_data;
} zerodds_ZeroDdsTopicBuiltinTopicData;

typedef struct {
    uint8_t  key[16];
    uint8_t  participant_key[16];
    char*    topic_name;
    char*    type_name;
    zerodds_ZeroDdsDurabilityQosPolicy        durability;
    zerodds_ZeroDdsDurabilityServiceQosPolicy durability_service;
    zerodds_ZeroDdsDeadlineQosPolicy          deadline;
    zerodds_ZeroDdsLatencyBudgetQosPolicy     latency_budget;
    zerodds_ZeroDdsLivelinessQosPolicy        liveliness;
    zerodds_ZeroDdsReliabilityQosPolicy       reliability;
    zerodds_ZeroDdsLifespanQosPolicy          lifespan;
    zerodds_ZeroDdsUserDataQosPolicy          user_data;
    zerodds_ZeroDdsOwnershipQosPolicy         ownership;
    zerodds_ZeroDdsOwnershipStrengthQosPolicy ownership_strength;
    zerodds_ZeroDdsDestinationOrderQosPolicy  destination_order;
    zerodds_ZeroDdsPresentationQosPolicy      presentation;
    zerodds_ZeroDdsPartitionQosPolicy         partition;
    zerodds_ZeroDdsTopicDataQosPolicy         topic_data;
    zerodds_ZeroDdsGroupDataQosPolicy         group_data;
} zerodds_ZeroDdsPublicationBuiltinTopicData;

typedef struct {
    uint8_t  key[16];
    uint8_t  participant_key[16];
    char*    topic_name;
    char*    type_name;
    zerodds_ZeroDdsDurabilityQosPolicy        durability;
    zerodds_ZeroDdsDeadlineQosPolicy          deadline;
    zerodds_ZeroDdsLatencyBudgetQosPolicy     latency_budget;
    zerodds_ZeroDdsLivelinessQosPolicy        liveliness;
    zerodds_ZeroDdsReliabilityQosPolicy       reliability;
    zerodds_ZeroDdsOwnershipQosPolicy         ownership;
    zerodds_ZeroDdsDestinationOrderQosPolicy  destination_order;
    zerodds_ZeroDdsUserDataQosPolicy          user_data;
    zerodds_ZeroDdsTimeBasedFilterQosPolicy   time_based_filter;
    zerodds_ZeroDdsPresentationQosPolicy      presentation;
    zerodds_ZeroDdsPartitionQosPolicy         partition;
    zerodds_ZeroDdsTopicDataQosPolicy         topic_data;
    zerodds_ZeroDdsGroupDataQosPolicy         group_data;
} zerodds_ZeroDdsSubscriptionBuiltinTopicData;

void zerodds_topic_builtin_topic_data_free(zerodds_ZeroDdsTopicBuiltinTopicData* data);
void zerodds_publication_builtin_topic_data_free(zerodds_ZeroDdsPublicationBuiltinTopicData* data);
void zerodds_subscription_builtin_topic_data_free(zerodds_ZeroDdsSubscriptionBuiltinTopicData* data);
```

Topic, publication and subscription carry every member that DDS 1.4
§2.2.5 names for TopicBuiltinTopicData, PublicationBuiltinTopicData and
SubscriptionBuiltinTopicData, as a policy structure; a duration is given
in seconds and nanoseconds. `zerodds_dp_get_discovered_topic_data`,
`zerodds_dr_get_matched_publication_data` and
`zerodds_dw_get_matched_subscription_data` fill in the names, the
octet sequences and the partition names as copies that belong to the caller
until it passes the structure to the matching `*_free` function. The
release sets the pointers to NULL; a second call frees nothing
more. A name with a NUL character has no C string: the call
returns `ZERODDS_INVALID_UTF8` and fills in nothing.

Both functions answer for every handle that
`zerodds_dr_get_matched_publications` or
`zerodds_dw_get_matched_subscriptions` returns, also for an endpoint
of the same participant. A handle that does not denote a matched endpoint
returns `ZERODDS_BAD_PARAMETER` (DDS 1.4 §2.2.2.4.2,
§2.2.2.5.3) and fills in nothing.

`zerodds_dp_get_discovered_topic_data` returns, for a handle that
`zerodds_dp_get_discovered_topics` has not returned,
`ZERODDS_BAD_PARAMETER` and fills in nothing.

## §8 Status codes

```c
#define ZERODDS_OK                              0
#define ZERODDS_ERROR                          -1
#define ZERODDS_BAD_HANDLE                     -2
#define ZERODDS_INVALID_UTF8                   -3
#define ZERODDS_TIMEOUT                        -4
#define ZERODDS_PRECONDITION_NOT_MET           -5
#define ZERODDS_BAD_PARAMETER                  -6
#define ZERODDS_NO_DATA                        -7
#define ZERODDS_OUT_OF_RESOURCES               -8
#define ZERODDS_NOT_ENABLED                    -9
#define ZERODDS_IMMUTABLE_POLICY              -10
#define ZERODDS_INCONSISTENT_POLICY           -11
#define ZERODDS_ALREADY_DELETED               -12
#define ZERODDS_UNSUPPORTED                   -13
#define ZERODDS_ILLEGAL_OPERATION             -14
```

## §9 Memory ownership contract

| Operation | Owner |
|-----------|-------|
| `*_create()` returns ptr | Caller (must `*_destroy`) |
| `dpf_get_instance()` | Singleton (no destroy) |
| `dr_take()`-payloads | Caller (must `return_loan`) |
| `*_get_qos()` reads existing strings | Static lifetime (no free) |
| `*_set_qos()` strings | borrowed; internal copy |
| `topic_get_name()` returns `const char*` | Static; not freed |
| `version()` returns `const char*` | Static; not freed |
| `*_get_matched_publication_data` / `*_get_matched_subscription_data` | Caller (must `zerodds_publication_builtin_topic_data_free` / `zerodds_subscription_builtin_topic_data_free`) |
| `dp_get_discovered_topic_data` | Caller (must `zerodds_topic_builtin_topic_data_free`) |

## §10 Stability

- The public header form (`include/zerodds.h`) is **ABI-stable** from `1.0.0-rc.1`.
- Struct layouts are `#[repr(C)]`, with a major bump on breaking layout changes.
- Status codes are stable; new codes are added additively with a higher absolute value.
- Function signatures are stable; new functions are allowed additively.

## §11 Test obligation

- Per function at least one round-trip test against a two-participant setup.
- Cross-vendor wire compat is not directly testable (the wire format comes from rtps + cdr); checks are done via the existing Cyclone/FastDDS live tests in dcps.
- Header compile test with `gcc -Wall -Werror -pedantic` + `clang -Weverything`.
- C++ compile test against `-std=c++17`.

## §12 Cross-reference to related vendor specs

This C FFI spec forms the foundation. The following vendor specs detail
specific sub-areas or document downstream layers:

- **`zerodds-listener-callbacks-1.1.md`** — listener callback pattern for
  all 6 entity types (DP/Pub/Sub/DW/DR/Topic). Spec §2.2.4 has no
  normative mapping path for C; this vendor spec defines the
  vtable+user_data pattern.
- **`zerodds-async-1.0.md`** — async DDS API (Rust counterpart to the sync DCPS),
  but does not use the C FFI; lives in parallel.
- **`zerodds-java-omgdds-1.0.md`** — pure Java path without JNI; does not use
  the C FFI, but InProcessBus + (v1.1) gRPC bridge.

## §13 FFI surface status

The authoritative item status is kept in the coverage audit
`docs/spec-coverage/zerodds-c-api-1.0.md`.

### Exposed surface (v1.0)

- DomainParticipantFactory: 7 functions (complete) ✅
- DomainParticipant: 21 functions (16 init + 5 extension) ✅
- Topic: 7 functions ✅
- Publisher + DataWriter: 24 functions (8 Pub + 16 DW) ✅
- Subscriber + DataReader: 24 functions (8 Sub + 16 DR) ✅
- QoS: 22 structures + 7 conversion functions ✅
- Conditions + WaitSet: 12 functions ✅
- Built-in Topics: 5 functions ✅
- Listener: 6 structures + 12 set/get functions, **API surface only, no active wireup** ⚠️
- Instance operations (register_instance/unregister/lookup/get_key_value): 7 functions, **vendor variant with raw key bytes** ✅
- Loan API (loan_message/commit_loan/discard_loan): 3 functions, **`Unsupported` stub** ⚠️
- Matched pubs/subs listings: 4 functions, **empty list / `Unsupported`** ⚠️

**Total: ~130 FFI functions exposed.**

### Planned extensions (v1.1)

- **Listener active wireup**: a runtime worker thread fires the callbacks
  on status counter increments. Status mask filter active.
- **Loan API live**: zero-copy path wired to the iceoryx backend.
- **Matched pubs/subs listings**: the runtime exposes `matched_subscription_handles`
  per writer EID; the FFI bridges the list.
- **`read` vs. `take`**: local read state map per reader; currently aliased.
- **`wait_for_historical_data`**: TransientLocal reader binds to the durability service.

### Optional extensions

- **TypeLookup service wireup** in the C FFI (not exposed in v1.0).
- **DDS-Security plugin FFI** for C/C++ plugin users.
- **DCPSPSM-Cxx-AnyXxx** type-erased wrappers in C headers.
