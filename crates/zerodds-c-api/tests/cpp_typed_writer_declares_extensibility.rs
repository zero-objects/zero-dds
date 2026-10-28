// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds::TypedWriter<T>` declares the type's extensibility to the C
//! ABI.
//!
//! The encapsulation identifier the runtime stamps on a user payload is
//! chosen from the writer slot's `wire_extensibility`, which defaults to
//! FINAL — `PLAIN_CDR2_LE`, `0x0007`. An `@appendable` body opens with a
//! DHEADER and a `@mutable` one with an EMHEADER per member, so a writer
//! that never calls `zerodds_writer_set_wire_extensibility` announces
//! `0x0007` over a body that is neither. Measured on the Linux test host before this
//! call existed, one datagram per topic, three types that differ only in
//! extensibility:
//!
//! ```text
//! @final       CDR2_LE (0x0007)  07b20100 0e640300
//! @appendable  CDR2_LE (0x0007)  08000000 07b20100 0e640300
//! @mutable     CDR2_LE (0x0007)  10000000 01000020 07b20100 02000020 0e640300
//! ```
//!
//! CycloneDDS decoded all three correctly — it reads the body with its
//! own type and not with the header — which is what made the defect
//! invisible between two ZeroDDS peers as well. OpenDDS does not:
//! `dds/DCPS/DataReaderImpl_T.h` calls `to_encoding(encoding, encap,
//! type_support_->base_extensibility())`, and `to_encoding_i` in
//! `dds/DCPS/EncapsulationHeader.cpp` sets `wrong_extensibility =
//! expected_extensibility != FINAL` for `KIND_CDR2_LE` and returns
//! `false`, dropping the sample (OpenDDS `603cac1`, read at the source).
//!
//! # Why this is a stub link and not a live topic
//!
//! What is under test is one seam: the constructor of `TypedWriter<T>`
//! asks the trait for the extensibility and hands it to the C ABI. A
//! live pub/sub run would exercise a domain, discovery and a reader to
//! observe a value this test can read directly off the call — and would
//! not run at all where a network does not cooperate. So the C ABI
//! symbols the constructor touches are **defined by the test program
//! itself**: nothing is linked, nothing is published, and the recorded
//! argument is the assertion. The mapping from that argument to the four
//! octets on the wire is
//! `runtime.rs::user_payload_encap_maps_repr_and_extensibility`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

/// The C++ compiler to build the probe with.
///
/// No skip arm. A missing C++ compiler is not a passing test: this file
/// is the only gate on a header that ships to C++ callers, and a run
/// that silently did not compile it reports the same "ok" as a run that
/// did. Every host in this tree's own examples manifest already lists
/// `c++` as a requirement.
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

/// The probe: three trait specialisations that differ only in what
/// `extensibility()` answers, three `TypedWriter`s, and stubs for the C
/// ABI that record what the constructor asked for.
///
/// Two of the three carry a `TypeObject` and one does not, so both
/// creation paths — `zerodds_writer_create_typed` and
/// `zerodds_writer_create_kind` — are covered. A call that only reached
/// the typed path would leave every union topic and every type XTypes
/// cannot lower announcing FINAL.
const PROBE: &str = r####"
#include "zerodds/dds.hpp"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// --- the C ABI, as far as TypedWriter's constructor reaches it ---------------
//
// Defined here rather than linked: the argument of the extensibility call
// is what this program exists to report.

namespace {
int last_extensibility = -1;
const char *last_path = "none";
int runtime_token = 0;
int writer_token = 0;
}  // namespace

extern "C" {

zerodds_ZeroDdsRuntime *zerodds_runtime_create(uint32_t) {
    return reinterpret_cast<zerodds_ZeroDdsRuntime *>(&runtime_token);
}

void zerodds_runtime_destroy(zerodds_ZeroDdsRuntime *) {}

zerodds_ZeroDdsWriter *zerodds_writer_create_kind(zerodds_ZeroDdsRuntime *, const char *,
                                                  const char *, int, int) {
    last_path = "kind";
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}

zerodds_ZeroDdsWriter *zerodds_writer_create_typed(zerodds_ZeroDdsRuntime *, const char *,
                                                   const char *, int, int, const uint8_t *,
                                                   uintptr_t) {
    last_path = "typed";
    return reinterpret_cast<zerodds_ZeroDdsWriter *>(&writer_token);
}

void zerodds_writer_destroy(zerodds_ZeroDdsWriter *) {}

int zerodds_writer_set_wire_extensibility(zerodds_ZeroDdsWriter *, int extensibility) {
    last_extensibility = extensibility;
    return 0;
}

}  // extern "C"

// --- three types that differ only in extensibility ---------------------------

struct Fin {};
struct App {};
struct Mut {};

static const uint8_t kTypeObject[] = {0x50, 0x00, 0x02, 0x00};

namespace dds {
namespace topic {

template <>
struct topic_type_support<Fin> {
    static const char *type_name() { return "Fin"; }
    static constexpr bool is_keyed() { return false; }
    static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::FINAL;
    }
    static const uint8_t *type_object() { return kTypeObject; }
    static std::size_t type_object_len() { return sizeof(kTypeObject); }
};

template <>
struct topic_type_support<App> {
    static const char *type_name() { return "App"; }
    static constexpr bool is_keyed() { return false; }
    static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::APPENDABLE;
    }
    static const uint8_t *type_object() { return kTypeObject; }
    static std::size_t type_object_len() { return sizeof(kTypeObject); }
};

// No TypeObject: the `create_kind` path.
template <>
struct topic_type_support<Mut> {
    static const char *type_name() { return "Mut"; }
    static constexpr bool is_keyed() { return false; }
    static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::MUTABLE;
    }
    static const uint8_t *type_object() { return nullptr; }
    static std::size_t type_object_len() { return 0; }
};

}  // namespace topic
}  // namespace dds

int main() {
    zerodds::Runtime rt(0);
    {
        zerodds::TypedWriter<Fin> w(rt, "fin");
        std::printf("FINAL %d %s\n", last_extensibility, last_path);
    }
    {
        zerodds::TypedWriter<App> w(rt, "app");
        std::printf("APPENDABLE %d %s\n", last_extensibility, last_path);
    }
    {
        zerodds::TypedWriter<Mut> w(rt, "mut");
        std::printf("MUTABLE %d %s\n", last_extensibility, last_path);
    }
    return 0;
}
"####;

/// The constructor hands the C ABI `0`, `1` and `2` for `@final`,
/// `@appendable` and `@mutable`, on both creation paths.
///
/// `0`/`1`/`2` are `zerodds_writer_set_wire_extensibility`'s own
/// numbering, documented on the function and read there by
/// `ExtensibilityForRepr`.
#[test]
fn a_typed_writer_declares_the_types_extensibility_to_the_c_abi() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!(
        "zerodds-cpp-extensibility-{}-{:?}",
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

    // The path each type took is asserted too: a call placed only in the
    // `create_typed` arm would leave every type without a `TypeObject`
    // announcing FINAL, and the `Mut` row is the one that would catch it.
    for expected in ["FINAL 0 typed", "APPENDABLE 1 typed", "MUTABLE 2 kind"] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "`zerodds/dds.hpp` did not declare the extensibility: expected a line \
             `{expected}`, got:\n{stdout}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
