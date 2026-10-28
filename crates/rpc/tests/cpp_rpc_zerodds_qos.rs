// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The ZeroDDS binding of `dds/rpc.hpp` carries the QoS of DDS-RPC 1.0 §7.10
//! to the C API: the endpoint QoS at creation, a `@Qos` profile URL resolved
//! through `zerodds_endpoint_qos_from_profile_url`, and the mutable policies
//! changed at run time through `zerodds_writer_set_qos`. A policy that cannot
//! change is `ImmutablePolicyError`, whichever side finds out.
//!
//! As in `cpp_rpc_writer_declares_extensibility.rs` the C API symbols the
//! binding reaches are defined by the probe, so the arguments the binding
//! passes and the exceptions it raises are the assertion. The C side is held
//! by `crates/zerodds-c-api/tests/rpc_qos.rs`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/rpc has a grandparent")
        .to_path_buf()
}

fn cxx() -> &'static str {
    for candidate in ["c++", "g++", "clang++"] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
        {
            return candidate;
        }
    }
    panic!("no C++ compiler on PATH (tried c++, g++, clang++)")
}

const PROBE: &str = r####"
#include <dds/rpc/zerodds.hpp>

#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace {
int runtime_token = 0;
int writer_token = 0;
int reader_token = 0;
int env_status(const char *name) {
    const char *raw = std::getenv(name);
    return raw ? std::atoi(raw) : 0;
}
}  // namespace

extern "C" {

zerodds_ZeroDdsEndpointQos zerodds_endpoint_qos_default(void) {
    zerodds_ZeroDdsEndpointQos qos;
    std::memset(&qos, 0, sizeof(qos));
    return qos;
}
zerodds_ZeroDdsRuntime *zerodds_runtime_create(uint32_t) {
    return reinterpret_cast<zerodds_ZeroDdsRuntime *>(&runtime_token);
}
void zerodds_runtime_destroy(zerodds_ZeroDdsRuntime *) {}
zerodds_ZeroDdsWriter *zerodds_writer_create_qos(zerodds_ZeroDdsRuntime *, const char *,
                                                 const char *, int,
                                                 const zerodds_ZeroDdsEndpointQos *qos) {
    std::printf("CREATE reliable=%d transient=%d keep_all=%d depth=%u deadline=%llu\n",
                qos->reliable, qos->transient_local, qos->history_keep_all, qos->history_depth,
                static_cast<unsigned long long>(qos->deadline_ns));
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}
int zerodds_writer_set_wire_extensibility(zerodds_ZeroDdsWriter *, int) { return 0; }
int zerodds_writer_set_data_representation(zerodds_ZeroDdsWriter *, int) { return 0; }
void zerodds_writer_destroy(zerodds_ZeroDdsWriter *) {}
int zerodds_writer_guid(zerodds_ZeroDdsWriter *, uint8_t *out_guid) {
    std::memset(out_guid, 0x5a, 16);
    return 0;
}
static int64_t last_sequence_number = 0;
int zerodds_writer_write(zerodds_ZeroDdsWriter *, const uint8_t *, uintptr_t) {
    ++last_sequence_number;
    return 0;
}
int zerodds_writer_next_sequence_number(zerodds_ZeroDdsWriter *, int64_t *out_sn) {
    *out_sn = last_sequence_number + 1;
    return 0;
}
int zerodds_writer_write_to_readers(zerodds_ZeroDdsWriter *, const uint8_t *, uintptr_t,
                                    const uint8_t *, uintptr_t, int64_t *out_sn) {
    ++last_sequence_number;
    *out_sn = last_sequence_number;
    return 0;
}
uintptr_t zerodds_writer_matched_count(zerodds_ZeroDdsWriter *) { return 0; }
int zerodds_writer_write_related(zerodds_ZeroDdsWriter *, const uint8_t *, uintptr_t,
                                 const uint8_t *, uint64_t, int64_t *) {
    return 0;
}
int zerodds_writer_set_qos(zerodds_ZeroDdsWriter *, const zerodds_ZeroDdsEndpointQos *qos) {
    std::printf("SET deadline=%llu lifespan=%llu\n",
                static_cast<unsigned long long>(qos->deadline_ns),
                static_cast<unsigned long long>(qos->lifespan_ns));
    return env_status("ZD_PROBE_SET_QOS_RC");
}

zerodds_ZeroDdsReader *zerodds_reader_create_qos(zerodds_ZeroDdsRuntime *, const char *,
                                                 const char *, int,
                                                 const zerodds_ZeroDdsEndpointQos *) {
    return reinterpret_cast<zerodds_ZeroDdsReader *>(&reader_token);
}

zerodds_ZeroDdsWriter *zerodds_writer_create_qos_typed(
    zerodds_ZeroDdsRuntime *runtime, const char *topic, const char *type, int keyed,
    const zerodds_ZeroDdsEndpointQos *qos, const uint8_t *, uintptr_t, const uint8_t *const *,
    const uintptr_t *, uintptr_t) {
    return zerodds_writer_create_qos(runtime, topic, type, keyed, qos);
}
zerodds_ZeroDdsReader *zerodds_reader_create_qos_typed(
    zerodds_ZeroDdsRuntime *runtime, const char *topic, const char *type, int keyed,
    const zerodds_ZeroDdsEndpointQos *qos, const uint8_t *, uintptr_t, const uint8_t *const *,
    const uintptr_t *, uintptr_t) {
    return zerodds_reader_create_qos(runtime, topic, type, keyed, qos);
}

void zerodds_reader_destroy(zerodds_ZeroDdsReader *) {}
uintptr_t zerodds_reader_matched_count(zerodds_ZeroDdsReader *) { return 0; }
int zerodds_reader_set_qos(zerodds_ZeroDdsReader *, const zerodds_ZeroDdsEndpointQos *) {
    return 0;
}
int zerodds_reader_take_wire(zerodds_ZeroDdsReader *, uint8_t **out_buf, uintptr_t *out_len,
                             uint8_t *, uint8_t *, uint8_t *, int64_t *, uint8_t *, uint8_t *,
                             uint64_t *) {
    *out_buf = nullptr;
    *out_len = 0;
    return 0;
}
void zerodds_buffer_free(uint8_t *, uintptr_t) {}

int zerodds_reader_set_arrival_callback(zerodds_ZeroDdsReader *, void (*)(void *), void *,
                                        void (*)(void *)) {
    return 0;
}

int zerodds_endpoint_qos_from_profile_url(const char *url, zerodds_ZeroDdsEndpointQos *out) {
    std::printf("PROFILE %s\n", url);
    const int status = env_status("ZD_PROBE_PROFILE_RC");
    if (status != 0) {
        return status;
    }
    std::memset(out, 0, sizeof(*out));
    out->reliable = 0;
    out->transient_local = 1;
    out->history_keep_all = 0;
    out->history_depth = 32;
    out->deadline_ns = 2000000000ull;
    out->lifespan_ns = 3000000000ull;
    return 0;
}

// The discovery symbols of §7.6.2.
@@
}  // extern "C"

int main() {
    auto participant = dds::rpc::zerodds::participant_on(0);
    dds::rpc::EndpointQos created;
    created.reliability = dds::rpc::ReliabilityKind::BEST_EFFORT;
    created.history = dds::rpc::HistoryKind::KEEP_LAST;
    created.history_depth = 4;
    created.deadline_ns = 10;
    auto writer = participant->publisher()->create_writer("T", "Ty", created);

    dds::rpc::EndpointQos tuned = created;
    tuned.deadline_ns = 20;
    tuned.lifespan_ns = 30;
    try {
        writer->set_qos(tuned);
    } catch (const dds::rpc::LocalError &) {
        std::printf("FIRST refused\n");
    }

    dds::rpc::EndpointQos history = created;
    history.history_depth = 5;
    try {
        writer->set_qos(history);
        std::printf("HISTORY accepted\n");
    } catch (const dds::rpc::ImmutablePolicyError &) {
        std::printf("HISTORY refused\n");
    }

    try {
        writer->set_qos(tuned);
        std::printf("SET accepted\n");
    } catch (const dds::rpc::ImmutablePolicyError &) {
        std::printf("SET refused immutable\n");
    } catch (const dds::rpc::LocalError &e) {
        std::printf("SET refused other %s\n", e.what());
    }

    try {
        const dds::rpc::EndpointQos profile = participant->load_qos_profile("file:///q.xml#Lib");
        std::printf("LOADED reliable=%d durability=%d history=%d depth=%u deadline=%llu lifespan=%llu\n",
                    static_cast<int>(profile.reliability), static_cast<int>(profile.durability),
                    static_cast<int>(profile.history), profile.history_depth,
                    static_cast<unsigned long long>(profile.deadline_ns),
                    static_cast<unsigned long long>(profile.lifespan_ns));
    } catch (const dds::rpc::InvalidArgumentError &) {
        std::printf("LOAD invalid argument\n");
    } catch (const dds::rpc::UnsupportedError &) {
        std::printf("LOAD unsupported\n");
    } catch (const dds::rpc::LocalError &e) {
        std::printf("LOAD failed %s\n", e.what());
    }
    return 0;
}
"####;

fn run_probe(binary: &Path, set_rc: &str, profile_rc: &str) -> Vec<String> {
    let run = Command::new(binary)
        .env("ZD_PROBE_SET_QOS_RC", set_rc)
        .env("ZD_PROBE_PROFILE_RC", profile_rc)
        .output()
        .expect("running the probe");
    assert!(run.status.success(), "the probe did not run");
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn the_zerodds_binding_carries_the_rpc_qos_to_the_c_api() {
    let root = repository_root();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-rpc-qos-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let source = dir.join("probe.cpp");
    let binary = dir.join("probe");
    std::fs::write(
        &source,
        PROBE.replace("@@", include_str!("cpp/rpc_discovery_stubs.inc")),
    )
    .expect("writing the probe");

    let out = Command::new(cxx())
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror", "-pthread"])
        .arg(&source)
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("running the C++ compiler");
    assert!(
        out.status.success(),
        "the probe did not compile:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // §7.10.2: the creation QoS reaches the C API; a change of DEADLINE and
    // LIFESPAN is applied; a change of HISTORY is refused before the C API is
    // asked; a profile resolves to the QoS the C API reports.
    let accepted = run_probe(&binary, "0", "0");
    assert_eq!(
        accepted,
        vec![
            "CREATE reliable=0 transient=0 keep_all=0 depth=4 deadline=10",
            "SET deadline=20 lifespan=30",
            "HISTORY refused",
            "SET deadline=20 lifespan=30",
            "SET accepted",
            "PROFILE file:///q.xml#Lib",
            "LOADED reliable=0 durability=1 history=0 depth=32 deadline=2000000000 \
             lifespan=3000000000",
        ]
    );

    // The runtime's `ImmutablePolicy` (-10) is an ImmutablePolicyError; a
    // profile the C API cannot resolve (-6 bad URL, -13 unsupported) is a
    // local error naming it, not a default.
    let immutable = run_probe(&binary, "-10", "-6");
    assert!(
        immutable.contains(&"SET refused immutable".to_string()),
        "{immutable:?}"
    );
    assert!(
        immutable.contains(&"LOAD invalid argument".to_string()),
        "{immutable:?}"
    );
    let unsupported = run_probe(&binary, "0", "-13");
    assert!(
        unsupported.contains(&"LOAD unsupported".to_string()),
        "{unsupported:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
