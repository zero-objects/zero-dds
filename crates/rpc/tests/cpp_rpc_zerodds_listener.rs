// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The ZeroDDS binding of `dds/rpc.hpp` calls a reader's data callback when
//! samples arrive (DDS-RPC 1.0 §7.9.2.2, §7.11.1.4.7-.10).
//!
//! The C API's arrival callback runs on the runtime's receive thread and may
//! not call ZeroDDS, while a listener takes a request and writes a reply.
//! The binding therefore signals a thread of its own reader from the C
//! callback, and that thread calls the callback. The probe defines the C
//! symbol, plays the receive thread itself, and checks that the callback is
//! called for the backlog and for an arrival, not after it is removed, that
//! the C user data is released exactly once per hook, and that a callback
//! that removes itself does not wait for itself. The C side is held by
//! `crates/zerodds-c-api/tests/arrival_callback.rs`.

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

#include <atomic>
#include <condition_variable>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>

namespace {
int runtime_token = 0;
int reader_token = 0;
void (*hook)(void *) = nullptr;
void *hook_data = nullptr;
void (*hook_release)(void *) = nullptr;
std::atomic<int> releases{0};
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
                                                 const zerodds_ZeroDdsEndpointQos *) {
    return nullptr;
}
int zerodds_writer_set_wire_extensibility(zerodds_ZeroDdsWriter *, int) { return 0; }
int zerodds_writer_set_data_representation(zerodds_ZeroDdsWriter *, int) { return 0; }
void zerodds_writer_destroy(zerodds_ZeroDdsWriter *) {}
int zerodds_writer_guid(zerodds_ZeroDdsWriter *, uint8_t *) { return 0; }
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
int zerodds_writer_set_qos(zerodds_ZeroDdsWriter *, const zerodds_ZeroDdsEndpointQos *) {
    return 0;
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
int zerodds_endpoint_qos_from_profile_url(const char *, zerodds_ZeroDdsEndpointQos *) {
    return -1;
}

// The runtime drops the hook it had when it takes another, or none, and the
// hook releases its user data then.
int zerodds_reader_set_arrival_callback(zerodds_ZeroDdsReader *, void (*callback)(void *),
                                        void *user_data, void (*release)(void *)) {
    if (hook_release != nullptr) {
        hook_release(hook_data);
        ++releases;
    }
    hook = callback;
    hook_data = callback != nullptr ? user_data : nullptr;
    hook_release = callback != nullptr ? release : nullptr;
    return 0;
}

// The discovery symbols of §7.6.2.
@@
}  // extern "C"

int main() {
    auto participant = dds::rpc::zerodds::participant_on(0);
    auto reader = participant->subscriber()->create_reader("T", "Ty", dds::rpc::EndpointQos{});

    std::mutex lock;
    std::condition_variable changed;
    int calls = 0;
    auto wait_for = [&](int count) {
        std::unique_lock<std::mutex> guard(lock);
        changed.wait(guard, [&]() { return calls >= count; });
    };

    reader->set_data_callback([&]() {
        std::lock_guard<std::mutex> guard(lock);
        ++calls;
        changed.notify_all();
    });
    wait_for(1);
    std::printf("BACKLOG\n");

    // The receive thread: the C callback, with the user data it was given.
    hook(hook_data);
    wait_for(2);
    std::printf("ARRIVAL\n");

    reader->set_data_callback(nullptr);
    int after_clear = 0;
    {
        std::lock_guard<std::mutex> guard(lock);
        after_clear = calls;
    }
    std::printf("CLEARED calls=%d released=%d hook=%d\n", after_clear, releases.load(),
                hook != nullptr ? 1 : 0);

    // A callback that removes itself does not wait for its own thread.
    int removed = 0;
    reader->set_data_callback([&]() {
        reader->set_data_callback(nullptr);
        std::lock_guard<std::mutex> guard(lock);
        removed = 1;
        changed.notify_all();
    });
    {
        std::unique_lock<std::mutex> guard(lock);
        changed.wait(guard, [&]() { return removed == 1; });
    }
    std::printf("SELF-CLEAR\n");
    return 0;
}
"####;

#[test]
fn the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal() {
    let root = repository_root();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-rpc-listener-{}-{:?}",
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

    let run = Command::new(&binary).output().expect("running the probe");
    assert!(
        run.status.success(),
        "the probe did not run:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = String::from_utf8_lossy(&run.stdout);
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        vec![
            "BACKLOG",
            "ARRIVAL",
            // One call for the samples already waiting, one for the arrival;
            // the first hook was released when it was removed, and removing
            // left no hook.
            "CLEARED calls=2 released=1 hook=0",
            "SELF-CLEAR",
        ],
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
