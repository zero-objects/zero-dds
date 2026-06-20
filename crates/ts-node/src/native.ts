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
