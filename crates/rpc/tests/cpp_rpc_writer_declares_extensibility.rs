// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `dds::rpc::zerodds::RuntimePublisher::create_writer` declares the framing of
//! the samples this binding writes to the C ABI.
//!
//! The encapsulation identifier the runtime stamps on a user payload is
//! chosen from the writer slot's `wire_extensibility`, which defaults to
//! FINAL. FINAL is also what this binding produces — `dds/rpc.hpp`'s
//! `encode_request_header` / `encode_reply_header` concatenate a header
//! of §7.5.1.1.1 with `dds_type_traits<T>::serialize` and put no outer
//! delimiter and no per-member header in front of it, the reading
//! `crates/rpc/src/common_types.rs` states for the Rust side ("Encoding:
//! XCDR2 with Final extensibility"). So the identifier is right either
//! way today, and this test is about which of the two reasons it is
//! right for: the C ABI's fallback, or this file saying so.
//!
//! Measured on the Linux test host against OpenDDS 3.35.0 through this binding's own
//! `create_writer`, three topics whose types differ in nothing but the
//! extensibility, 24 samples each, before and after the declaration:
//!
//! ```text
//! SAMPLES ZdExtUFinal       24  first[a=111111 b=222222]   <- the control
//! SAMPLES ZdExtUAppendable   0
//! SAMPLES ZdExtUMutable      0
//!
//! ERROR: to_encoding_i: expected appendable extensibility, but got
//!        XCDR2 Little Endian Plain                              (x24)
//! ```
//!
//! and `00070000` in front of all three bodies both times — the run is
//! byte for byte the same, because FINAL is what the binding already
//! announced by falling back. What the fix changes is that the value now
//! has a source: if `dds/rpc.hpp`'s codec ever grows an outer delimiter,
//! `kWireExtensibility` is the line that moves with it, and the row above
//! is what a delimited body under an unchanged identifier looks like — 0
//! of 24 beside a control of 24.
//!
//! # Why a recording stub and not a live topic
//!
//! What is under test is one seam: `create_writer` asks the binding's own
//! constant and hands it to the C ABI. A live run would need a domain,
//! discovery and a strict reader to observe a value this test reads
//! straight off the call. So the C ABI symbols the binding touches are
//! **defined by the test program itself**: nothing is linked, nothing is
//! published, and the recorded argument is the assertion. The mapping
//! from that argument to the four octets on the wire is
//! `crates/dcps/src/runtime.rs::user_payload_encap`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, from this crate's manifest directory.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/rpc has a grandparent")
        .to_path_buf()
}

/// The C++ compiler to build the probe with.
///
/// No skip arm. A missing C++ compiler is not a passing test: this file
/// is the only gate on the extensibility the RPC entity binding declares,
/// and a run that silently did not compile it reports the same "ok" as a
/// run that did.
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
    panic!(
        "no C++ compiler on PATH (tried c++, g++, clang++) — this test compiles \
         dds/rpc/zerodds.hpp"
    )
}

/// The probe: a participant, a writer created through the binding, and a
/// writer adopted from a handle the binding did not create.
///
/// The adopted one is the second row on purpose. The declaration belongs
/// to `create_writer` and not to `Writer`'s constructor, because that
/// constructor also takes a handle somebody else made and chose an
/// extensibility for; a call placed there would overwrite it. A stray
/// `EXT` line after `ADOPT` is that mistake.
const PROBE: &str = r####"
#include <dds/rpc/zerodds.hpp>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

// --- the C ABI, as far as the binding reaches it -----------------------------
//
// Defined here rather than linked: the argument of the extensibility call
// is what this program exists to report.

namespace {
int runtime_token = 0;
int writer_token = 0;
int reader_token = 0;

// The status `zerodds_writer_set_wire_extensibility` answers with, so the
// refusal path is reachable without a second program.
int set_status() {
    const char *raw = std::getenv("ZD_PROBE_SET_RC");
    return raw ? std::atoi(raw) : 0;
}
}  // namespace

extern "C" {

zerodds_ZeroDdsRuntime *zerodds_runtime_create(uint32_t) {
    return reinterpret_cast<zerodds_ZeroDdsRuntime *>(&runtime_token);
}

void zerodds_runtime_destroy(zerodds_ZeroDdsRuntime *) {}

zerodds_ZeroDdsEndpointQos zerodds_endpoint_qos_default(void) {
    zerodds_ZeroDdsEndpointQos qos;
    std::memset(&qos, 0, sizeof(qos));
    qos.reliable = 1;
    qos.history_keep_all = 1;
    qos.history_depth = 1;
    qos.max_samples = -1;
    qos.max_instances = -1;
    qos.max_samples_per_instance = -1;
    return qos;
}

zerodds_ZeroDdsWriter *zerodds_writer_create_qos(zerodds_ZeroDdsRuntime *, const char *topic,
                                                 const char *, int is_keyed,
                                                 const zerodds_ZeroDdsEndpointQos *qos) {
    std::printf("CREATE %s reliable=%d keyed=%d\n", topic, qos->reliable, is_keyed);
    std::fflush(stdout);
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}

int zerodds_writer_set_wire_extensibility(zerodds_ZeroDdsWriter *, int extensibility) {
    std::printf("EXT %d\n", extensibility);
    std::fflush(stdout);
    return set_status();
}

int zerodds_writer_set_data_representation(zerodds_ZeroDdsWriter *, int) { return 0; }

void zerodds_writer_destroy(zerodds_ZeroDdsWriter *) {
    std::printf("DESTROY\n");
    std::fflush(stdout);
}

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

int zerodds_reader_take_wire(zerodds_ZeroDdsReader *, uint8_t **out_buf, uintptr_t *out_len,
                             uint8_t *, uint8_t *, uint8_t *, int64_t *, uint8_t *, uint8_t *,
                             uint64_t *) {
    *out_buf = nullptr;
    *out_len = 0;
    return 0;
}

int zerodds_writer_write_related(zerodds_ZeroDdsWriter *, const uint8_t *, uintptr_t,
                                 const uint8_t *, uint64_t, int64_t *) {
    return 0;
}

int zerodds_writer_set_qos(zerodds_ZeroDdsWriter *, const zerodds_ZeroDdsEndpointQos *) {
    return 0;
}

int zerodds_reader_set_qos(zerodds_ZeroDdsReader *, const zerodds_ZeroDdsEndpointQos *) {
    return 0;
}

int zerodds_endpoint_qos_from_profile_url(const char *, zerodds_ZeroDdsEndpointQos *) {
    return -1;
}

int zerodds_reader_set_arrival_callback(zerodds_ZeroDdsReader *, void (*)(void *), void *,
                                        void (*)(void *)) {
    return 0;
}

uintptr_t zerodds_reader_matched_count(zerodds_ZeroDdsReader *) { return 0; }

void zerodds_buffer_free(uint8_t *, uintptr_t) {}

// The discovery symbols of §7.6.2.
@@
}  // extern "C"

int main() {
    auto participant = dds::rpc::zerodds::participant_on(0);
    try {
        auto writer = participant->publisher()->create_writer(
            "Calculator_Service_Request", "Calculator_Request", dds::rpc::EndpointQos{});
        std::printf("HANDED_OUT\n");
        (void)writer;
    } catch (const dds::rpc::LocalError &e) {
        std::printf("THREW %s\n", e.what());
    }
    std::fflush(stdout);

    // A writer over a handle this binding did not create: its owner chose
    // the framing, so nothing here may declare one over the top.
    {
        std::printf("ADOPT\n");
        std::fflush(stdout);
        dds::rpc::zerodds::Writer adopted(
            reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token), false);
        (void)adopted;
    }
    std::printf("END\n");
    return 0;
}
"####;

/// Builds the probe once and runs it with the given stub status.
fn run_probe(binary: &Path, set_rc: &str) -> String {
    let run = Command::new(binary)
        .env("ZD_PROBE_SET_RC", set_rc)
        .output()
        .expect("running the probe");
    assert!(
        run.status.success(),
        "the probe did not run (ZD_PROBE_SET_RC={set_rc}):\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).into_owned()
}

/// `create_writer` hands the C ABI the binding's own `kWireExtensibility`,
/// and a refused declaration destroys the writer instead of returning it.
#[test]
fn the_rpc_participant_declares_the_bindings_framing_to_the_c_abi() {
    let root = repository_root();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-rpc-extensibility-{}-{:?}",
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

    // `0` is `zerodds_writer_set_wire_extensibility`'s own numbering for
    // FINAL, documented on the function and read there by
    // `ExtensibilityForRepr`. `keyed=0` is asserted alongside it because
    // §7.4.2 gives a request and a reply topic no key member, and a
    // WithKey writer would not match a NoKey reader at all — a zero on
    // every row for a reason that has nothing to do with framing.
    let declared = run_probe(&binary, "0");
    assert_eq!(
        declared.lines().collect::<Vec<_>>(),
        vec![
            "CREATE Calculator_Service_Request reliable=1 keyed=0",
            "EXT 0",
            "HANDED_OUT",
            // The writer it handed out owns its handle and tears it down when
            // the try block ends. The adopted one below does not (`owned ==
            // false`), which is why no second DESTROY follows `ADOPT`.
            "DESTROY",
            "ADOPT",
            "END",
        ],
        "`dds/rpc/zerodds.hpp` did not declare its framing:\n{declared}"
    );

    // A writer that could not declare its own framing is destroyed and the
    // create fails, rather than being handed on to publish samples under an
    // identifier that does not name them.
    let refused = run_probe(&binary, "-7");
    assert_eq!(
        refused.lines().collect::<Vec<_>>(),
        vec![
            "CREATE Calculator_Service_Request reliable=1 keyed=0",
            "EXT 0",
            "DESTROY",
            "THREW zerodds_writer_set_wire_extensibility/set_data_representation returned -7",
            "ADOPT",
            "END",
        ],
        "a refused declaration was not fatal:\n{refused}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
