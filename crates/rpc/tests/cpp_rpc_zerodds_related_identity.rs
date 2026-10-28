// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The ZeroDDS binding of `dds/rpc.hpp` sends and reads
//! `PID_RELATED_SAMPLE_IDENTITY` (DDS-RPC 1.0 §7.8.2).
//!
//! `Writer::write_related` hands the C API the related identity as a GUID
//! and one 64-bit sequence number and takes the stamped sequence number
//! back as the identity of its own sample. `Reader::take` reports the writer
//! GUID and sequence number of each DATA and its related identity, which
//! the Enhanced profile (§7.8.2.1, §7.8.2.2) correlates by.
//!
//! As in `cpp_rpc_writer_declares_extensibility.rs`, the C API symbols the
//! binding reaches are defined by the probe itself, so the arguments the
//! binding passes and the values it reads back are the assertion. The
//! symbols themselves are held against the real C API by
//! `crates/zerodds-c-api/tests/related_sample_identity.rs`.

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
int takes = 0;
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

int zerodds_writer_write_related(zerodds_ZeroDdsWriter *, const uint8_t *payload, uintptr_t len,
                                 const uint8_t *related_guid, uint64_t related_sn,
                                 int64_t *out_sn) {
    std::printf("WRITE_RELATED len=%lu first=%u rguid0=%02x rsn=%llu\n",
                static_cast<unsigned long>(len), payload[0], related_guid[0],
                static_cast<unsigned long long>(related_sn));
    *out_sn = 77;
    last_sequence_number = 77;
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
void zerodds_buffer_free(uint8_t *buffer, uintptr_t) { std::free(buffer); }

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

int zerodds_reader_take_wire(zerodds_ZeroDdsReader *, uint8_t **out_buf, uintptr_t *out_len,
                             uint8_t *, uint8_t *, uint8_t *out_writer_guid, int64_t *out_sn,
                             uint8_t *out_has_related, uint8_t *out_related_guid,
                             uint64_t *out_related_sn) {
    if (takes++ == 0) {
        auto *buffer = static_cast<uint8_t *>(std::malloc(4));
        std::memset(buffer, 9, 4);
        *out_buf = buffer;
        *out_len = 4;
        std::memset(out_writer_guid, 0x11, 16);
        *out_sn = 5;
        *out_has_related = 1;
        std::memset(out_related_guid, 0x22, 16);
        *out_related_sn = (1ull << 32) | 9ull;
    } else {
        *out_buf = nullptr;
        *out_len = 0;
        *out_has_related = 0;
    }
    return 0;
}

// The discovery symbols of §7.6.2.
@@
}  // extern "C"

int main() {
    auto participant = dds::rpc::zerodds::participant_on(0);
    auto writer = participant->publisher()->create_writer("T", "Ty", dds::rpc::EndpointQos{});
    dds::SampleIdentity related;
    related.writer_guid[0] = 0xab;
    related.sequence_number.high = 1;
    related.sequence_number.low = 9;
    const dds::rpc::Octets payload{1, 2, 3};
    const dds::SampleIdentity id = writer->write_related(payload, related);
    std::printf("ID guid0=%02x hi=%d lo=%u next_lo=%u\n", id.writer_guid[0],
                id.sequence_number.high, id.sequence_number.low,
                writer->next_identity().sequence_number.low);

    auto reader = participant->subscriber()->create_reader("T", "Ty", dds::rpc::EndpointQos{});
    const auto taken = reader->take();
    std::printf("TAKEN %zu\n", taken.size());
    for (const auto &sample : taken) {
        const dds::SampleInfo &info = sample.second;
        std::printf("SAMPLE len=%zu guid0=%02x sn=%d/%u related=%d rguid0=%02x rsn=%d/%u\n",
                    sample.first.size(), info.identity.writer_guid[0],
                    info.identity.sequence_number.high, info.identity.sequence_number.low,
                    static_cast<int>(info.has_related_identity), info.related_identity.writer_guid[0],
                    info.related_identity.sequence_number.high,
                    info.related_identity.sequence_number.low);
    }
    return 0;
}
"####;

/// §7.8.2: the related identity goes out as one GUID and one 64-bit sequence
/// number, comes back as the sequence number of the new sample, and a take
/// reports the identity of the DATA and its related identity.
#[test]
fn the_zerodds_binding_sends_and_reads_the_related_sample_identity() {
    let root = repository_root();
    let dir = std::env::temp_dir().join(format!(
        "zerodds-rpc-related-{}-{:?}",
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
    assert!(run.status.success(), "the probe did not run");
    let text = String::from_utf8_lossy(&run.stdout);
    // The related identity {GUID ab.., high 1, low 9} is the 64-bit
    // sequence number (1 << 32) | 9 = 4294967305. The stamped number 77 is
    // the new sample's identity and the next one is 78.
    // The take reports the DATA's writer GUID 0x11 and sequence number 5,
    // and the related identity GUID 0x22 with the sequence number
    // (1 << 32) | 9 split back into high 1 and low 9.
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        vec![
            "WRITE_RELATED len=3 first=1 rguid0=ab rsn=4294967305",
            "ID guid0=5a hi=0 lo=77 next_lo=78",
            "TAKEN 1",
            "SAMPLE len=4 guid0=11 sn=0/5 related=1 rguid0=22 rsn=1/9",
        ],
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
