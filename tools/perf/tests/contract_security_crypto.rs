// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-perf` with `zerodds-security-crypto`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-crypto`, all in `main.rs`:
//! - `aes_gcm_hw::report()`, printed as is by `zerodds-perf hw-info`.
//! - `aes_gcm_hw::cached()` and `HwCapabilities::backend_label()`, printed in
//!   the header line of `zerodds-perf aes-gcm` next to the throughput, so
//!   that a number is read together with the hardware it was measured on.
//!   The benchmark itself runs on `ring` directly.
//!
//! What it relies on: the capabilities are those of the CPU the binary runs
//! on, the cached value does not change between the two calls, the label
//! names hardware AES only when the CPU has it, and the report carries the
//! same values as `cached()`.
//!
//! Oracles: the CPU feature detection of the Rust standard library
//! (`is_x86_feature_detected!`, `is_aarch64_feature_detected!`), which is
//! outside the producer, and the rustdoc of `aes_gcm_hw` ("The first caller
//! triggers the detection; further callers read the OnceLock value";
//! `aes_gcm_fully_accelerated` is "both the AES block and GHASH in HW").
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_security_crypto::aes_gcm_hw::{self, Arch, HwCapabilities};

#[test]
fn the_cached_capabilities_are_those_the_standard_library_detects() {
    let caps = aes_gcm_hw::cached();
    #[cfg(target_arch = "x86_64")]
    {
        assert_eq!(caps.arch, Arch::X86_64);
        assert_eq!(caps.aes, std::arch::is_x86_feature_detected!("aes"));
        assert_eq!(
            caps.pclmul,
            std::arch::is_x86_feature_detected!("pclmulqdq")
        );
        assert_eq!(caps.sha2, std::arch::is_x86_feature_detected!("sha"));
        assert_eq!(caps.simd, std::arch::is_x86_feature_detected!("avx2"));
    }
    #[cfg(target_arch = "aarch64")]
    {
        assert_eq!(caps.arch, Arch::Aarch64);
        assert_eq!(caps.aes, std::arch::is_aarch64_feature_detected!("aes"));
        assert_eq!(
            caps.pclmul,
            std::arch::is_aarch64_feature_detected!("pmull")
        );
        assert_eq!(caps.sha2, std::arch::is_aarch64_feature_detected!("sha2"));
        assert_eq!(caps.simd, std::arch::is_aarch64_feature_detected!("neon"));
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        assert_eq!(caps, HwCapabilities::default());
        assert_eq!(caps.arch, Arch::Other);
    }
}

#[test]
fn the_cached_value_is_the_same_on_every_call() {
    // `hw-info` and the header of `aes-gcm` read it at different times.
    let first = aes_gcm_hw::cached();
    for _ in 0..8 {
        assert_eq!(aes_gcm_hw::cached(), first);
    }
    assert_eq!(first, HwCapabilities::detect());
}

#[test]
fn the_backend_label_names_hardware_aes_only_when_the_cpu_has_it() {
    for arch in [Arch::X86_64, Arch::Aarch64, Arch::Other] {
        for aes in [false, true] {
            for pclmul in [false, true] {
                let caps = HwCapabilities {
                    arch,
                    aes,
                    pclmul,
                    sha2: false,
                    simd: false,
                };
                let label = caps.backend_label();
                let software = label.contains("software");
                // A number measured without the AES block must not be read
                // as a hardware number, and the other way round.
                assert_eq!(
                    software,
                    arch == Arch::Other || !aes,
                    "{arch:?} aes={aes} pclmul={pclmul}: {label}"
                );
                assert_eq!(caps.aes_gcm_fully_accelerated(), aes && pclmul);
            }
        }
    }
    // Two CPUs that differ in what they accelerate do not share a label.
    let partial = HwCapabilities {
        arch: Arch::X86_64,
        aes: true,
        pclmul: false,
        sha2: false,
        simd: false,
    };
    let full = HwCapabilities {
        pclmul: true,
        ..partial
    };
    assert_ne!(partial.backend_label(), full.backend_label());
}

#[test]
fn the_report_carries_the_values_of_the_cached_capabilities() {
    let caps = aes_gcm_hw::cached();
    let report = aes_gcm_hw::report();
    let fields: Vec<(&str, &str)> = report
        .split_whitespace()
        .map(|pair| pair.split_once('=').expect("key=value"))
        .collect();
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
            .unwrap_or_else(|| panic!("`{name}` missing in `{report}`"))
    };
    assert_eq!(field("arch"), caps.arch.label());
    assert_eq!(field("aes"), caps.aes.to_string());
    assert_eq!(field("pclmul"), caps.pclmul.to_string());
    assert_eq!(field("sha2"), caps.sha2.to_string());
    assert_eq!(field("simd"), caps.simd.to_string());
    assert_eq!(field("backend"), caps.backend_label());
    assert_eq!(
        field("fully_accelerated"),
        caps.aes_gcm_fully_accelerated().to_string()
    );
}
