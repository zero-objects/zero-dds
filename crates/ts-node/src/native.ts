// native.ts — koffi FFI declarations for libzerodds.
//
// Mirrors `zerodds.h` (from `crates/zerodds-c-api`) via koffi.
// koffi was chosen over node-ffi-napi because:
//   * koffi is actively maintained (node-ffi-napi practically dead since 2022)
//   * supports modern Node 18+ without additional build tools
//   * no native compile steps at install time

import koffi from "koffi";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { existsSync } from "node:fs";

// The Rust target triple for the current platform/arch — so we can also find a
// library built with an explicit `cargo build --target <triple>` (the ZeroDDS
// CI builds that way; its artifacts land in target/<triple>/release/, not the
// host-default target/release/).
function targetTriple(): string | null {
  switch (process.platform) {
    case "linux":
      return process.arch === "arm64"
        ? "aarch64-unknown-linux-gnu"
        : "x86_64-unknown-linux-gnu";
    case "darwin":
      return process.arch === "arm64"
        ? "aarch64-apple-darwin"
        : "x86_64-apple-darwin";
    case "win32":
      return process.arch === "arm64"
        ? "aarch64-pc-windows-msvc"
        : "x86_64-pc-windows-msvc";
    default:
      return null;
  }
}

// Library path resolution. Order: explicit ZERODDS_LIB override, then the
// cargo target dirs (host-default and explicit-triple, release and debug), then
// the packaged dist/runtimes/, then the OS default search.
function findLibrary(): string {
  const here = dirname(fileURLToPath(import.meta.url));
  const ext =
    process.platform === "linux"
      ? ".so"
      : process.platform === "darwin"
      ? ".dylib"
      : ".dll";
  const prefix = process.platform === "win32" ? "" : "lib";
  const fname = `${prefix}zerodds${ext}`;

  // Explicit override (used by CI jobs and the cross-host verification harness).
  const override = process.env.ZERODDS_LIB;
  if (override && existsSync(override)) return override;

  // src/native.ts -> crates/ts-node/src/ -> ../../../ = repo root.
  const root = resolve(here, "..", "..", "..");
  const candidates: string[] = [];
  // Default-target builds: target/{release,debug}/
  candidates.push(resolve(root, "target", "release", fname));
  candidates.push(resolve(root, "target", "debug", fname));
  // Explicit-target builds: target/<triple>/{release,debug}/
  const triple = targetTriple();
  if (triple) {
    candidates.push(resolve(root, "target", triple, "release", fname));
    candidates.push(resolve(root, "target", triple, "debug", fname));
  }
  // Packaged dist/runtimes/, then OS default search.
  candidates.push(resolve(here, "..", "runtimes", `${process.platform}-${process.arch}`, fname));
  candidates.push(fname);

  for (const p of candidates) {
    if (existsSync(p)) return p;
  }
  return fname; // last attempt — koffi searches in standard paths
}

const lib = koffi.load(findLibrary());

// Status codes from `enum ZeroDdsStatus` (crates/zerodds-c-api). Only the codes
// the binding branches on are mirrored here.
export const ZeroDdsStatus = {
  Ok: 0,
  /// No sample currently available (DDS RETCODE_NO_DATA).
  NoData: -7,
} as const;

// Opaque-Pointer-Aliase.
export const RuntimePtr = koffi.pointer("ZeroDdsRuntime", koffi.opaque());
export const WriterPtr = koffi.pointer("ZeroDdsWriter", koffi.opaque());
export const ReaderPtr = koffi.pointer("ZeroDdsReader", koffi.opaque());

// ====== extern "C" Declarations ======

export const zerodds_runtime_create = lib.func(
  "ZeroDdsRuntime* zerodds_runtime_create(uint32_t domain_id)",
);
export const zerodds_runtime_destroy = lib.func(
  "void zerodds_runtime_destroy(ZeroDdsRuntime* runtime)",
);

export const zerodds_writer_create = lib.func(
  "ZeroDdsWriter* zerodds_writer_create(ZeroDdsRuntime* runtime, const char* topic, const char* type_name, int reliable)",
);
export const zerodds_writer_write = lib.func(
  "int zerodds_writer_write(ZeroDdsWriter* writer, const uint8_t* payload, size_t len)",
);
export const zerodds_writer_wait_for_matched = lib.func(
  "int zerodds_writer_wait_for_matched(ZeroDdsWriter* writer, int min_count, uint64_t timeout_ms)",
);
export const zerodds_writer_destroy = lib.func(
  "void zerodds_writer_destroy(ZeroDdsWriter* writer)",
);

export const zerodds_reader_create = lib.func(
  "ZeroDdsReader* zerodds_reader_create(ZeroDdsRuntime* runtime, const char* topic, const char* type_name, int reliable)",
);
// reader_take: out_buf is pointer-to-pointer. koffi: use _Out_ marker.
// NOTE: the Rust C-API takes FOUR params — the 4th, `out_repr` (the XCDR
// representation byte), was missing here. Omitting it let the native fn read a
// garbage 4th arg and write `*out_repr`, segfaulting nondeterministically.
export const zerodds_reader_take = lib.func(
  "int zerodds_reader_take(ZeroDdsReader* reader, _Out_ void** out_buf, _Out_ size_t* out_len, _Out_ uint8_t* out_repr)",
);
export const zerodds_reader_wait_for_matched = lib.func(
  "int zerodds_reader_wait_for_matched(ZeroDdsReader* reader, int min_count, uint64_t timeout_ms)",
);
export const zerodds_reader_destroy = lib.func(
  "void zerodds_reader_destroy(ZeroDdsReader* reader)",
);

export const zerodds_buffer_free = lib.func(
  "void zerodds_buffer_free(void* buf, size_t len)",
);

export const zerodds_version = lib.func("const char* zerodds_version()");

// ============================================================================
// DDS-PSM-Cxx-konforme Surface
// ============================================================================

export const FactoryPtr = koffi.pointer("ZeroDdsDomainParticipantFactory", koffi.opaque());
export const ParticipantPtr = koffi.pointer("ZeroDdsDomainParticipant", koffi.opaque());
export const TopicPtr = koffi.pointer("ZeroDdsTopic", koffi.opaque());
export const PublisherPtr = koffi.pointer("ZeroDdsPublisher", koffi.opaque());
export const SubscriberPtr = koffi.pointer("ZeroDdsSubscriber", koffi.opaque());
export const DataWriterPtr = koffi.pointer("ZeroDdsDataWriter", koffi.opaque());
export const DataReaderPtr = koffi.pointer("ZeroDdsDataReader", koffi.opaque());
export const GuardConditionPtr = koffi.pointer("ZeroDdsGuardCondition", koffi.opaque());
export const WaitSetPtr = koffi.pointer("ZeroDdsWaitSet", koffi.opaque());

export const zerodds_dpf_get_instance = lib.func(
  "const ZeroDdsDomainParticipantFactory* zerodds_dpf_get_instance()",
);
export const zerodds_dpf_create_participant = lib.func(
  "ZeroDdsDomainParticipant* zerodds_dpf_create_participant(const ZeroDdsDomainParticipantFactory* f, uint32_t domain_id, const void* qos)",
);
export const zerodds_dpf_delete_participant = lib.func(
  "int zerodds_dpf_delete_participant(const ZeroDdsDomainParticipantFactory* f, ZeroDdsDomainParticipant* p)",
);

export const zerodds_dp_create_topic = lib.func(
  "ZeroDdsTopic* zerodds_dp_create_topic(ZeroDdsDomainParticipant* p, const char* name, const char* type_name, const void* qos)",
);
export const zerodds_dp_delete_topic = lib.func(
  "int zerodds_dp_delete_topic(ZeroDdsDomainParticipant* p, ZeroDdsTopic* t)",
);
export const zerodds_dp_create_publisher = lib.func(
  "ZeroDdsPublisher* zerodds_dp_create_publisher(ZeroDdsDomainParticipant* p, const void* qos)",
);
export const zerodds_dp_delete_publisher = lib.func(
  "int zerodds_dp_delete_publisher(ZeroDdsDomainParticipant* p, ZeroDdsPublisher* pub)",
);
export const zerodds_dp_create_subscriber = lib.func(
  "ZeroDdsSubscriber* zerodds_dp_create_subscriber(ZeroDdsDomainParticipant* p, const void* qos)",
);
export const zerodds_dp_delete_subscriber = lib.func(
  "int zerodds_dp_delete_subscriber(ZeroDdsDomainParticipant* p, ZeroDdsSubscriber* sub)",
);
export const zerodds_dp_delete_contained_entities = lib.func(
  "int zerodds_dp_delete_contained_entities(ZeroDdsDomainParticipant* p)",
);
export const zerodds_dp_get_domain_id = lib.func(
  "uint32_t zerodds_dp_get_domain_id(ZeroDdsDomainParticipant* p)",
);

export const zerodds_topic_get_name = lib.func(
  "char* zerodds_topic_get_name(ZeroDdsTopic* t)",
);
export const zerodds_topic_get_type_name = lib.func(
  "char* zerodds_topic_get_type_name(ZeroDdsTopic* t)",
);
export const zerodds_string_free = lib.func(
  "void zerodds_string_free(char* s)",
);

export const zerodds_pub_create_datawriter = lib.func(
  "ZeroDdsDataWriter* zerodds_pub_create_datawriter(ZeroDdsPublisher* pub, ZeroDdsTopic* topic, const void* qos)",
);
export const zerodds_pub_delete_datawriter = lib.func(
  "int zerodds_pub_delete_datawriter(ZeroDdsPublisher* pub, ZeroDdsDataWriter* dw)",
);
export const zerodds_dw_write = lib.func(
  "int zerodds_dw_write(ZeroDdsDataWriter* dw, const uint8_t* payload, size_t len, uint64_t handle)",
);
export const zerodds_dw_wait_for_matched = lib.func(
  "int zerodds_dw_wait_for_matched(ZeroDdsDataWriter* dw, int min, uint64_t timeout_ms)",
);

export const zerodds_sub_create_datareader = lib.func(
  "ZeroDdsDataReader* zerodds_sub_create_datareader(ZeroDdsSubscriber* sub, ZeroDdsTopic* topic, const void* qos)",
);
export const zerodds_sub_delete_datareader = lib.func(
  "int zerodds_sub_delete_datareader(ZeroDdsSubscriber* sub, ZeroDdsDataReader* dr)",
);
export const zerodds_dr_wait_for_matched = lib.func(
  "int zerodds_dr_wait_for_matched(ZeroDdsDataReader* dr, int min, uint64_t timeout_ms)",
);

// SampleInfo mirror of `struct zerodds_ZeroDdsSampleInfo` (zerodds.h). Only the
// subset the binding surfaces is read back; the full layout is declared so koffi
// allocates the right size for the out-parameter.
export const SampleInfo = koffi.struct("ZeroDdsSampleInfo", {
  sample_state: "uint32_t",
  view_state: "uint32_t",
  instance_state: "uint32_t",
  disposed_generation_count: "int32_t",
  no_writers_generation_count: "int32_t",
  sample_rank: "int32_t",
  generation_rank: "int32_t",
  absolute_generation_rank: "int32_t",
  source_timestamp_sec: "int32_t",
  source_timestamp_nanosec: "uint32_t",
  instance_handle: "uint64_t",
  publication_handle: "uint64_t",
  valid_data: "bool",
  // XCDR representation (0 = XCDR1, 1 = XCDR2) and wire byte order
  // (0 = little-endian, 1 = big-endian) of the payload, read from the
  // encapsulation header so the typed decoder can pick decode vs decode_be.
  representation: "uint8_t",
  big_endian: "uint8_t",
});

// take_next_sample / read_next_sample — single-sample data path for the DCPS
// DataReader. `take` removes the sample from the reader cache; `read` leaves it.
// rc == 0 with *out_len == 0 means "no data available right now".
export const zerodds_dr_take_next_sample = lib.func(
  "int zerodds_dr_take_next_sample(ZeroDdsDataReader* dr, _Out_ void** out_buf, _Out_ size_t* out_len, _Out_ ZeroDdsSampleInfo* out_info)",
);
export const zerodds_dr_read_next_sample = lib.func(
  "int zerodds_dr_read_next_sample(ZeroDdsDataReader* dr, _Out_ void** out_buf, _Out_ size_t* out_len, _Out_ ZeroDdsSampleInfo* out_info)",
);

export const zerodds_guardcondition_create = lib.func(
  "ZeroDdsGuardCondition* zerodds_guardcondition_create()",
);
export const zerodds_guardcondition_destroy = lib.func(
  "void zerodds_guardcondition_destroy(ZeroDdsGuardCondition* g)",
);
export const zerodds_guardcondition_set_trigger_value = lib.func(
  "int zerodds_guardcondition_set_trigger_value(ZeroDdsGuardCondition* g, bool v)",
);
export const zerodds_condition_get_trigger_value = lib.func(
  "bool zerodds_condition_get_trigger_value(const void* c)",
);
export const zerodds_waitset_create = lib.func(
  "ZeroDdsWaitSet* zerodds_waitset_create()",
);
export const zerodds_waitset_destroy = lib.func(
  "void zerodds_waitset_destroy(ZeroDdsWaitSet* w)",
);
export const zerodds_waitset_attach_condition = lib.func(
  "int zerodds_waitset_attach_condition(ZeroDdsWaitSet* w, void* c)",
);

// ============================================================================
// QoS structs (mirror of zerodds.h §QoS) + QoS-aware entity factories.
//
// Layout is byte-identical to the C-API structs so a koffi-encoded buffer can
// be passed straight through the `const void* qos` slots. Field ORDER matters:
// it follows the header declaration order exactly. Pointer fields (partition
// `const char *const *`, *_data `const uint8_t*`) are embedded as koffi
// pointers built by qos.ts and MUST outlive the create call.
// ============================================================================

// Duration (Spec §2.2.3.5): seconds + nanoseconds.
export const Duration = koffi.struct("ZeroDdsDuration", {
  sec: "int32",
  nanosec: "uint32",
});

// Individual policy structs (declaration order per zerodds.h).
export const DurabilityPolicy = koffi.struct("ZeroDdsDurabilityQosPolicy", {
  kind: "uint32",
});
export const DurabilityServicePolicy = koffi.struct(
  "ZeroDdsDurabilityServiceQosPolicy",
  {
    service_cleanup_delay: Duration,
    history_kind: "uint32",
    history_depth: "int32",
    max_samples: "int32",
    max_instances: "int32",
    max_samples_per_instance: "int32",
  },
);
export const DeadlinePolicy = koffi.struct("ZeroDdsDeadlineQosPolicy", {
  period: Duration,
});
export const LatencyBudgetPolicy = koffi.struct(
  "ZeroDdsLatencyBudgetQosPolicy",
  { duration: Duration },
);
export const LivelinessPolicy = koffi.struct("ZeroDdsLivelinessQosPolicy", {
  kind: "uint32",
  lease_duration: Duration,
});
export const ReliabilityPolicy = koffi.struct("ZeroDdsReliabilityQosPolicy", {
  kind: "uint32",
  max_blocking_time: Duration,
});
export const DestinationOrderPolicy = koffi.struct(
  "ZeroDdsDestinationOrderQosPolicy",
  { kind: "uint32" },
);
export const HistoryPolicy = koffi.struct("ZeroDdsHistoryQosPolicy", {
  kind: "uint32",
  depth: "int32",
});
export const ResourceLimitsPolicy = koffi.struct(
  "ZeroDdsResourceLimitsQosPolicy",
  {
    max_samples: "int32",
    max_instances: "int32",
    max_samples_per_instance: "int32",
  },
);
export const TransportPriorityPolicy = koffi.struct(
  "ZeroDdsTransportPriorityQosPolicy",
  { value: "int32" },
);
export const LifespanPolicy = koffi.struct("ZeroDdsLifespanQosPolicy", {
  duration: Duration,
});
export const OwnershipPolicy = koffi.struct("ZeroDdsOwnershipQosPolicy", {
  kind: "uint32",
});
export const OwnershipStrengthPolicy = koffi.struct(
  "ZeroDdsOwnershipStrengthQosPolicy",
  { value: "int32" },
);
export const PresentationPolicy = koffi.struct(
  "ZeroDdsPresentationQosPolicy",
  { access_scope: "uint32", coherent_access: "bool", ordered_access: "bool" },
);
export const TimeBasedFilterPolicy = koffi.struct(
  "ZeroDdsTimeBasedFilterQosPolicy",
  { minimum_separation: Duration },
);
export const WriterDataLifecyclePolicy = koffi.struct(
  "ZeroDdsWriterDataLifecycleQosPolicy",
  { autodispose_unregistered_instances: "bool" },
);
export const ReaderDataLifecyclePolicy = koffi.struct(
  "ZeroDdsReaderDataLifecycleQosPolicy",
  {
    autopurge_nowriter_samples_delay: Duration,
    autopurge_disposed_samples_delay: Duration,
  },
);
export const EntityFactoryPolicy = koffi.struct(
  "ZeroDdsEntityFactoryQosPolicy",
  { autoenable_created_entities: "bool" },
);
// UserData/TopicData/GroupData all share this { const uint8_t*; uintptr_t }.
export const BytesPolicy = koffi.struct("ZeroDdsUserDataQosPolicy", {
  value: koffi.pointer("uint8_t"),
  value_len: "size_t",
});
// Partition: { const char *const *; uintptr_t }.
export const PartitionPolicy = koffi.struct("ZeroDdsPartitionQosPolicy", {
  names: koffi.pointer("char *"),
  names_len: "size_t",
});

// Aggregate QoS structs (field order MUST match zerodds.h exactly).
export const DataWriterQosStruct = koffi.struct("ZeroDdsDataWriterQos", {
  reliability: ReliabilityPolicy,
  durability: DurabilityPolicy,
  durability_service: DurabilityServicePolicy,
  deadline: DeadlinePolicy,
  latency_budget: LatencyBudgetPolicy,
  liveliness: LivelinessPolicy,
  destination_order: DestinationOrderPolicy,
  lifespan: LifespanPolicy,
  ownership: OwnershipPolicy,
  ownership_strength: OwnershipStrengthPolicy,
  partition: PartitionPolicy,
  presentation: PresentationPolicy,
  history: HistoryPolicy,
  resource_limits: ResourceLimitsPolicy,
  transport_priority: TransportPriorityPolicy,
  writer_data_lifecycle: WriterDataLifecyclePolicy,
  user_data: BytesPolicy,
  topic_data: BytesPolicy,
  group_data: BytesPolicy,
});

export const DataReaderQosStruct = koffi.struct("ZeroDdsDataReaderQos", {
  reliability: ReliabilityPolicy,
  durability: DurabilityPolicy,
  deadline: DeadlinePolicy,
  latency_budget: LatencyBudgetPolicy,
  liveliness: LivelinessPolicy,
  destination_order: DestinationOrderPolicy,
  ownership: OwnershipPolicy,
  partition: PartitionPolicy,
  presentation: PresentationPolicy,
  history: HistoryPolicy,
  resource_limits: ResourceLimitsPolicy,
  time_based_filter: TimeBasedFilterPolicy,
  reader_data_lifecycle: ReaderDataLifecyclePolicy,
  user_data: BytesPolicy,
  topic_data: BytesPolicy,
  group_data: BytesPolicy,
});

export const PublisherQosStruct = koffi.struct("ZeroDdsPublisherQos", {
  presentation: PresentationPolicy,
  partition: PartitionPolicy,
  group_data: BytesPolicy,
  entity_factory: EntityFactoryPolicy,
});
// SubscriberQos is structurally identical to PublisherQos (header typedef).
export const SubscriberQosStruct = PublisherQosStruct;

export const TopicQosStruct = koffi.struct("ZeroDdsTopicQos", {
  durability: DurabilityPolicy,
  durability_service: DurabilityServicePolicy,
  deadline: DeadlinePolicy,
  latency_budget: LatencyBudgetPolicy,
  liveliness: LivelinessPolicy,
  reliability: ReliabilityPolicy,
  destination_order: DestinationOrderPolicy,
  history: HistoryPolicy,
  resource_limits: ResourceLimitsPolicy,
  transport_priority: TransportPriorityPolicy,
  lifespan: LifespanPolicy,
  ownership: OwnershipPolicy,
  topic_data: BytesPolicy,
});

export const ContentFilteredTopicPtr = koffi.pointer(
  "ZeroDdsContentFilteredTopic",
  koffi.opaque(),
);

// QoS-aware factory variants. The C-API takes `const ZeroDds*Qos*`; koffi
// passes a Buffer's address straight through. Declared separately from the
// `const void*` variants above so koffi typechecks the struct buffer.
export const zerodds_dp_create_topic_qos = lib.func(
  "ZeroDdsTopic* zerodds_dp_create_topic(ZeroDdsDomainParticipant* p, const char* name, const char* type_name, ZeroDdsTopicQos* qos)",
);
export const zerodds_dp_create_publisher_qos = lib.func(
  "ZeroDdsPublisher* zerodds_dp_create_publisher(ZeroDdsDomainParticipant* p, ZeroDdsPublisherQos* qos)",
);
export const zerodds_dp_create_subscriber_qos = lib.func(
  "ZeroDdsSubscriber* zerodds_dp_create_subscriber(ZeroDdsDomainParticipant* p, ZeroDdsPublisherQos* qos)",
);
export const zerodds_pub_create_datawriter_qos = lib.func(
  "ZeroDdsDataWriter* zerodds_pub_create_datawriter(ZeroDdsPublisher* pub, ZeroDdsTopic* topic, ZeroDdsDataWriterQos* qos)",
);
export const zerodds_sub_create_datareader_qos = lib.func(
  "ZeroDdsDataReader* zerodds_sub_create_datareader(ZeroDdsSubscriber* sub, ZeroDdsTopic* topic, ZeroDdsDataReaderQos* qos)",
);

// ContentFilteredTopic (Spec §2.2.2.3.3).
export const zerodds_dp_create_contentfilteredtopic = lib.func(
  "ZeroDdsContentFilteredTopic* zerodds_dp_create_contentfilteredtopic(ZeroDdsDomainParticipant* p, const char* name, ZeroDdsTopic* related, const char* filter_expression, char** parameters, size_t param_count)",
);
export const zerodds_cft_set_schema = lib.func(
  "int zerodds_cft_set_schema(ZeroDdsContentFilteredTopic* cft, char** names, const uint32_t* kinds, size_t count)",
);
export const zerodds_dp_delete_contentfilteredtopic = lib.func(
  "int zerodds_dp_delete_contentfilteredtopic(ZeroDdsDomainParticipant* p, ZeroDdsContentFilteredTopic* cft)",
);
export const zerodds_sub_create_datareader_with_cft = lib.func(
  "ZeroDdsDataReader* zerodds_sub_create_datareader_with_cft(ZeroDdsSubscriber* sub, ZeroDdsContentFilteredTopic* cft, const void* qos)",
);

// ---- Keyed lifecycle (Spec §2.2.2.4.2 DataWriter instance ops) ----
export const zerodds_dw_register_instance = lib.func(
  "int zerodds_dw_register_instance(ZeroDdsDataWriter* dw, const uint8_t* key, size_t key_len, _Out_ uint64_t* out_handle)",
);
export const zerodds_dw_register_instance_w_timestamp = lib.func(
  "int zerodds_dw_register_instance_w_timestamp(ZeroDdsDataWriter* dw, const uint8_t* key, size_t key_len, int32_t ts_sec, uint32_t ts_nanosec, _Out_ uint64_t* out_handle)",
);
export const zerodds_dw_unregister_instance = lib.func(
  "int zerodds_dw_unregister_instance(ZeroDdsDataWriter* dw, uint64_t handle)",
);
export const zerodds_dw_unregister_instance_w_timestamp = lib.func(
  "int zerodds_dw_unregister_instance_w_timestamp(ZeroDdsDataWriter* dw, uint64_t handle, int32_t ts_sec, uint32_t ts_nanosec)",
);
export const zerodds_dw_lookup_instance = lib.func(
  "int zerodds_dw_lookup_instance(ZeroDdsDataWriter* dw, const uint8_t* key, size_t key_len, _Out_ uint64_t* out_handle)",
);
export const zerodds_dw_dispose = lib.func(
  "int zerodds_dw_dispose(ZeroDdsDataWriter* dw, const uint8_t* key_hash, uint64_t handle)",
);
export const zerodds_dw_dispose_w_timestamp = lib.func(
  "int zerodds_dw_dispose_w_timestamp(ZeroDdsDataWriter* dw, const uint8_t* key_hash, uint64_t handle, int32_t ts_sec, uint32_t ts_nanosec)",
);
export const zerodds_dr_lookup_instance = lib.func(
  "int zerodds_dr_lookup_instance(ZeroDdsDataReader* dr, const uint8_t* key, size_t key_len, _Out_ uint64_t* out_handle)",
);

// ---- DataReader status getters (Spec §2.2.4.1) ----
export const RequestedDeadlineMissedStatus = koffi.struct(
  "ZeroDdsRequestedDeadlineMissedStatus",
  {
    total_count: "int32",
    total_count_change: "int32",
    last_instance_handle: "uint64",
  },
);
export const LivelinessChangedStatus = koffi.struct(
  "ZeroDdsLivelinessChangedStatus",
  {
    alive_count: "int32",
    not_alive_count: "int32",
    alive_count_change: "int32",
    not_alive_count_change: "int32",
    last_publication_handle: "uint64",
  },
);
export const zerodds_dr_get_requested_deadline_missed_status = lib.func(
  "int zerodds_dr_get_requested_deadline_missed_status(ZeroDdsDataReader* dr, _Out_ ZeroDdsRequestedDeadlineMissedStatus* out)",
);
export const zerodds_dr_get_liveliness_changed_status = lib.func(
  "int zerodds_dr_get_liveliness_changed_status(ZeroDdsDataReader* dr, _Out_ ZeroDdsLivelinessChangedStatus* out)",
);

// Manual writer liveliness assertion (Spec §2.2.2.4.2.22).
export const zerodds_dw_assert_liveliness = lib.func(
  "int zerodds_dw_assert_liveliness(ZeroDdsDataWriter* dw)",
);

// Instance-state codes carried by ZeroDdsSampleInfo.instance_state.
export const InstanceState = {
  Alive: 1,
  NotAliveDisposed: 2,
  NotAliveNoWriters: 4,
} as const;

// ---- Batch take/read (Spec §2.2.2.5.3) ----
// The batch `zerodds_dr_take` path applies the ContentFilteredTopic filter
// (§2.2.2.3.3) AND EXCLUSIVE-ownership arbitration (§2.2.3.23) AND resolves the
// per-instance InstanceHandle — none of which the single-sample
// `take_next_sample` path does. The binding uses it for CFT / exclusive-owned
// readers so those QoS effects are observed.
export const SampleArray = koffi.struct("ZeroDdsSampleArray", {
  buffers: koffi.pointer("uint8_t *"),
  lengths: koffi.pointer("size_t"),
  infos: koffi.pointer(SampleInfo),
  count: "size_t",
  loan_token: "void *",
});
export const zerodds_dr_take = lib.func(
  "int zerodds_dr_take(ZeroDdsDataReader* dr, _Out_ ZeroDdsSampleArray* out, size_t max_samples, uint32_t sample_states, uint32_t view_states, uint32_t instance_states)",
);
export const zerodds_dr_read = lib.func(
  "int zerodds_dr_read(ZeroDdsDataReader* dr, _Out_ ZeroDdsSampleArray* out, size_t max_samples, uint32_t sample_states, uint32_t view_states, uint32_t instance_states)",
);
export const zerodds_dr_return_loan = lib.func(
  "int zerodds_dr_return_loan(ZeroDdsDataReader* dr, ZeroDdsSampleArray* arr)",
);
// ANY_* state masks (Spec §2.2.2.5.4): pass 0 = "any" in this C-API.
export const STATE_ANY = 0;
