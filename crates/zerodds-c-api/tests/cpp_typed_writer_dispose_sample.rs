// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds/dds.hpp`'s new sample-taking dispose/unregister overloads
//! (D10, C++ part): `Writer::dispose/unregister_instance/
//! unregister_with_dispose(const uint8_t *, std::size_t)` and
//! `TypedWriter<T>::dispose/unregister_instance/unregister_with_dispose
//! (const T &)`.
//!
//! # Why a stub link and not a live topic
//!
//! Same rationale as `cpp_typed_writer_declares_extensibility.rs`: what
//! is under test is a compile-time seam — which C ABI symbol each
//! overload resolves to, and what bytes it hands that symbol — not a
//! live pub/sub run. The C ABI symbols the probe touches are **defined
//! by the probe program itself**: nothing is linked against
//! `libzerodds`, nothing is published, and the recorded call name +
//! byte length is the assertion.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

/// The C++ compiler to build the probe with. No skip arm — see
/// `cpp_typed_writer_declares_extensibility.rs`'s own `cxx()` for why.
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
    panic!("no C++ compiler on PATH (tried c++, g++, clang++) — this test compiles zerodds/dds.hpp")
}

/// The probe: one keyed type with a `TypeObject` (so the writer takes the
/// typed-create path), a `Writer` and a `TypedWriter<Point>`, exercising
/// every dispose/unregister overload each offers.
const PROBE: &str = r####"
#include "zerodds/dds.hpp"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// --- the C ABI, as far as these calls reach it -----------------------------
//
// Defined here rather than linked: which symbol each overload calls, and
// with what bytes, is what this program exists to report.

namespace {
int runtime_token = 0;
int writer_token = 0;
const char *last_call = "none";
std::vector<uint8_t> last_bytes;
}  // namespace

extern "C" {

zerodds_ZeroDdsRuntime *zerodds_runtime_create(uint32_t) {
    return reinterpret_cast<zerodds_ZeroDdsRuntime *>(&runtime_token);
}
void zerodds_runtime_destroy(zerodds_ZeroDdsRuntime *) {}

zerodds_ZeroDdsWriter *zerodds_writer_create(zerodds_ZeroDdsRuntime *, const char *,
                                              const char *, int) {
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}
zerodds_ZeroDdsWriter *zerodds_writer_create_kind(zerodds_ZeroDdsRuntime *, const char *,
                                                   const char *, int, int) {
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}
zerodds_ZeroDdsWriter *zerodds_writer_create_typed(zerodds_ZeroDdsRuntime *, const char *,
                                                    const char *, int, int, const uint8_t *,
                                                    uintptr_t) {
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}
void zerodds_writer_destroy(zerodds_ZeroDdsWriter *) {}
int zerodds_writer_set_wire_extensibility(zerodds_ZeroDdsWriter *, int) { return 0; }
int zerodds_writer_write(zerodds_ZeroDdsWriter *, const uint8_t *, uintptr_t) { return 0; }
int zerodds_writer_wait_for_matched(zerodds_ZeroDdsWriter *, int, uint64_t) { return 0; }

int zerodds_writer_dispose(zerodds_ZeroDdsWriter *, const uint8_t *) {
    last_call = "dispose_hash";
    return 0;
}
int zerodds_writer_unregister(zerodds_ZeroDdsWriter *, const uint8_t *) {
    last_call = "unregister_hash";
    return 0;
}
int zerodds_writer_unregister_with_dispose(zerodds_ZeroDdsWriter *, const uint8_t *) {
    last_call = "unregister_with_dispose_hash";
    return 0;
}

int zerodds_writer_dispose_sample(zerodds_ZeroDdsWriter *, const uint8_t *sample, uintptr_t len) {
    last_call = "dispose_sample";
    last_bytes.assign(sample, sample + len);
    return 0;
}
int zerodds_writer_unregister_sample(zerodds_ZeroDdsWriter *, const uint8_t *sample,
                                      uintptr_t len) {
    last_call = "unregister_sample";
    last_bytes.assign(sample, sample + len);
    return 0;
}
int zerodds_writer_unregister_with_dispose_sample(zerodds_ZeroDdsWriter *, const uint8_t *sample,
                                                   uintptr_t len) {
    last_call = "unregister_with_dispose_sample";
    last_bytes.assign(sample, sample + len);
    return 0;
}

}  // extern "C"

// --- one keyed type with a TypeObject ---------------------------------------

struct Point {
    int32_t x;
    int32_t y;
};

static const uint8_t kTypeObject[] = {0x50, 0x00, 0x02, 0x00};

namespace dds {
namespace topic {

template <>
struct topic_type_support<Point> {
    static const char *type_name() { return "Point"; }
    static constexpr bool is_keyed() { return true; }
    static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::FINAL;
    }
    static const uint8_t *type_object() { return kTypeObject; }
    static std::size_t type_object_len() { return sizeof(kTypeObject); }
    static std::vector<uint8_t> encode(const Point &v) {
        std::vector<uint8_t> out(8);
        std::memcpy(out.data(), &v.x, 4);
        std::memcpy(out.data() + 4, &v.y, 4);
        return out;
    }
    static Point decode(const uint8_t *buf, std::size_t len) {
        Point p{};
        if (len >= 8) {
            std::memcpy(&p.x, buf, 4);
            std::memcpy(&p.y, buf + 4, 4);
        }
        return p;
    }
};

}  // namespace topic
}  // namespace dds

int main() {
    zerodds::Runtime rt(0);

    // Writer: the pre-existing key-hash overloads still resolve, and the
    // new sample-taking overloads resolve to the new symbols.
    {
        zerodds::Writer w(rt, "t1", "Point");
        uint8_t kh[16] = {0};
        w.dispose(kh);
        std::printf("writer.dispose(hash) call=%s\n", last_call);
        w.unregister_instance(kh);
        std::printf("writer.unregister_instance(hash) call=%s\n", last_call);
        w.unregister_with_dispose(kh);
        std::printf("writer.unregister_with_dispose(hash) call=%s\n", last_call);

        std::vector<uint8_t> sample = {1, 2, 3, 4};
        w.dispose(sample.data(), sample.size());
        std::printf("writer.dispose(sample) call=%s len=%zu\n", last_call, last_bytes.size());
        w.unregister_instance(sample.data(), sample.size());
        std::printf("writer.unregister_instance(sample) call=%s len=%zu\n", last_call,
                     last_bytes.size());
        w.unregister_with_dispose(sample.data(), sample.size());
        std::printf("writer.unregister_with_dispose(sample) call=%s len=%zu\n", last_call,
                     last_bytes.size());
    }

    // TypedWriter<Point>: the value-taking overloads encode through the
    // trait exactly like write() does, then call the sample symbols.
    {
        zerodds::TypedWriter<Point> tw(rt, "t2");
        Point p{42, 7};

        tw.dispose(p);
        std::printf("typed.dispose(value) call=%s len=%zu\n", last_call, last_bytes.size());
        tw.unregister_instance(p);
        std::printf("typed.unregister_instance(value) call=%s len=%zu\n", last_call,
                     last_bytes.size());
        tw.unregister_with_dispose(p);
        std::printf("typed.unregister_with_dispose(value) call=%s len=%zu\n", last_call,
                     last_bytes.size());
    }

    std::printf("OK\n");
    return 0;
}
"####;

/// Every dispose/unregister overload `zerodds/dds.hpp` offers resolves to
/// the C ABI symbol its own doc comment names, and the sample-taking
/// overloads (both on `Writer` and on `TypedWriter<T>`) hand that symbol
/// the encoded 8-byte sample, not the 16-byte key hash the older
/// overloads take.
#[test]
fn dispose_and_unregister_sample_overloads_resolve_to_the_new_symbols() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!(
        "zerodds-cpp-dispose-sample-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let source = dir.join("probe.cpp");
    let binary = dir.join("probe");
    std::fs::write(&source, PROBE).expect("writing the probe");

    let out = Command::new(cxx())
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-I")
        .arg(manifest.join("include"))
        .arg("-I")
        .arg(manifest.join("../cpp/include"))
        .arg("-I")
        .arg(manifest.join("../../runtime/cpp/include"))
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
    let stdout = String::from_utf8_lossy(&run.stdout);

    for expected in [
        "writer.dispose(hash) call=dispose_hash",
        "writer.unregister_instance(hash) call=unregister_hash",
        "writer.unregister_with_dispose(hash) call=unregister_with_dispose_hash",
        "writer.dispose(sample) call=dispose_sample len=4",
        "writer.unregister_instance(sample) call=unregister_sample len=4",
        "writer.unregister_with_dispose(sample) call=unregister_with_dispose_sample len=4",
        "typed.dispose(value) call=dispose_sample len=8",
        "typed.unregister_instance(value) call=unregister_sample len=8",
        "typed.unregister_with_dispose(value) call=unregister_with_dispose_sample len=8",
    ] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "`zerodds/dds.hpp` did not resolve as expected: wanted a line `{expected}`, got:\n{stdout}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
