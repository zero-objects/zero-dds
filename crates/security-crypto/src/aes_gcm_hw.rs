// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HW-Crypto-Capability-Detection — CPU-Features + AES-GCM-Backend-Label.
//!
//! Diese Datei detektiert zur **Runtime**, welche AES-/SHA-/Polynomial-
//! Multiplikations-Beschleuniger die aktuelle CPU bietet. Der eigentliche
//! AEAD-Pfad geht weiter durch `ring::aead`, das die HW-Ausnutzung
//! intern uebernimmt — diese Datei macht das *sichtbar*, damit
//! Operations-Teams pruefen koennen, was deployt ist.
//!
//! Der Mehrwert ggue. einer reinen ring-Nutzung:
//!
//! 1. **Deployment-Audit**: ein `zerodds-perf aes-gcm`-Run druckt das Label
//!    (`aes-ni`, `armv8-aes`, `none`), so dass Container-Images mit
//!    fehlenden CPU-Features auffallen.
//! 2. **Trade-off-Doku**: wenn `none` raus kommt, ist das ein klarer
//!    Hinweis darauf, dass Latenz-SLOs auf dem Host nicht zu halten
//!    sind, ohne dass jemand mit `lscpu` rumprobieren muss.
//! 3. **Bench-Bezug**: die `tools/perf::aes-gcm`-Throughput-Zahlen sind
//!    nur sinnvoll, wenn das HW-Label dabei steht.
//!
//! Spec-Bezug: keiner — die OMG-DDS-Security-Spec macht keine Vorgaben
//! zu HW-Crypto. Dieses Modul ist Operations-Tooling.

#[cfg(feature = "std")]
use std::sync::OnceLock;

/// CPU-Architektur-Label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// `x86_64` (Intel/AMD 64-bit).
    X86_64,
    /// `aarch64` (ARMv8 64-bit).
    Aarch64,
    /// Andere Architektur — Detektion gibt nur Software-Fallback zurueck.
    Other,
}

impl Arch {
    /// Aktuelle Compile-Time-Architektur.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else {
            Self::Other
        }
    }

    /// Maschinenlesbares Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Other => "other",
        }
    }
}

/// Detected HW-Capabilities einer CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HwCapabilities {
    /// Architektur.
    pub arch: Arch,
    /// AES-Block-Beschleuniger (AES-NI auf x86_64, FEAT_AES auf
    /// aarch64).
    pub aes: bool,
    /// Polynomial-Multiply-Unit fuer GHASH (PCLMULQDQ auf x86_64,
    /// FEAT_PMULL auf aarch64).
    pub pclmul: bool,
    /// SHA-2-Beschleuniger (SHA-NI auf x86_64, FEAT_SHA2 auf aarch64).
    pub sha2: bool,
    /// Erweiterte SIMD (AVX2 auf x86_64, NEON auf aarch64).
    pub simd: bool,
}

impl Default for Arch {
    fn default() -> Self {
        Self::Other
    }
}

impl HwCapabilities {
    /// Detektiert die HW-Caps der **laufenden** CPU. Auf Architekturen
    /// ohne expliziten Detect-Pfad gibt es Default (alle `false`).
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self {
                arch: Arch::X86_64,
                aes: std::arch::is_x86_feature_detected!("aes"),
                pclmul: std::arch::is_x86_feature_detected!("pclmulqdq"),
                sha2: std::arch::is_x86_feature_detected!("sha"),
                simd: std::arch::is_x86_feature_detected!("avx2"),
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self {
                arch: Arch::Aarch64,
                aes: std::arch::is_aarch64_feature_detected!("aes"),
                pclmul: std::arch::is_aarch64_feature_detected!("pmull"),
                sha2: std::arch::is_aarch64_feature_detected!("sha2"),
                simd: std::arch::is_aarch64_feature_detected!("neon"),
            }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self {
                arch: Arch::Other,
                aes: false,
                pclmul: false,
                sha2: false,
                simd: false,
            }
        }
    }

    /// `true`, wenn AES-GCM auf dieser CPU mit voller HW-Beschleunigung
    /// laufen kann (sowohl AES-Block als auch GHASH per HW).
    #[must_use]
    pub const fn aes_gcm_fully_accelerated(&self) -> bool {
        self.aes && self.pclmul
    }

    /// Backend-Label fuer Reports und Logs.
    #[must_use]
    pub const fn backend_label(&self) -> &'static str {
        match (self.arch, self.aes, self.pclmul) {
            (Arch::X86_64, true, true) => "aes-ni+pclmulqdq",
            (Arch::X86_64, true, false) => "aes-ni-only",
            (Arch::X86_64, false, _) => "x86_64-software",
            (Arch::Aarch64, true, true) => "armv8-aes+pmull",
            (Arch::Aarch64, true, false) => "armv8-aes-only",
            (Arch::Aarch64, false, _) => "aarch64-software",
            (Arch::Other, _, _) => "software",
        }
    }
}

/// Cached, prozessweite HW-Detection. Erste Aufrufer triggert die
/// Detektion; weitere Aufrufer lesen den OnceLock-Wert.
#[cfg(feature = "std")]
#[must_use]
pub fn cached() -> HwCapabilities {
    static CACHE: OnceLock<HwCapabilities> = OnceLock::new();
    *CACHE.get_or_init(HwCapabilities::detect)
}

/// Mehrzeiliger Report. Geeignet fuer `zerodds-perf aes-gcm --info`.
#[cfg(feature = "std")]
#[must_use]
pub fn report() -> alloc::string::String {
    let caps = cached();
    alloc::format!(
        "arch={} aes={} pclmul={} sha2={} simd={} backend={} fully_accelerated={}",
        caps.arch.label(),
        caps.aes,
        caps.pclmul,
        caps.sha2,
        caps.simd,
        caps.backend_label(),
        caps.aes_gcm_fully_accelerated(),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn arch_label_round_trip() {
        assert_eq!(Arch::X86_64.label(), "x86_64");
        assert_eq!(Arch::Aarch64.label(), "aarch64");
        assert_eq!(Arch::Other.label(), "other");
    }

    #[test]
    fn arch_current_matches_cfg() {
        let a = Arch::current();
        if cfg!(target_arch = "x86_64") {
            assert_eq!(a, Arch::X86_64);
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(a, Arch::Aarch64);
        } else {
            assert_eq!(a, Arch::Other);
        }
    }

    #[test]
    fn detect_returns_arch_matching_cfg() {
        let caps = HwCapabilities::detect();
        assert_eq!(caps.arch, Arch::current());
    }

    #[test]
    fn cached_is_idempotent() {
        let a = cached();
        let b = cached();
        assert_eq!(a, b);
    }

    #[test]
    fn fully_accelerated_requires_both_aes_and_pclmul() {
        let caps = HwCapabilities {
            arch: Arch::X86_64,
            aes: true,
            pclmul: false,
            sha2: false,
            simd: false,
        };
        assert!(!caps.aes_gcm_fully_accelerated());
        let caps_full = HwCapabilities {
            pclmul: true,
            ..caps
        };
        assert!(caps_full.aes_gcm_fully_accelerated());
    }

    #[test]
    fn backend_label_x86_full_hw() {
        let caps = HwCapabilities {
            arch: Arch::X86_64,
            aes: true,
            pclmul: true,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "aes-ni+pclmulqdq");
    }

    #[test]
    fn backend_label_x86_aes_only() {
        let caps = HwCapabilities {
            arch: Arch::X86_64,
            aes: true,
            pclmul: false,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "aes-ni-only");
    }

    #[test]
    fn backend_label_x86_no_aes() {
        let caps = HwCapabilities {
            arch: Arch::X86_64,
            aes: false,
            pclmul: false,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "x86_64-software");
    }

    #[test]
    fn backend_label_arm_full_hw() {
        let caps = HwCapabilities {
            arch: Arch::Aarch64,
            aes: true,
            pclmul: true,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "armv8-aes+pmull");
    }

    #[test]
    fn backend_label_arm_aes_only() {
        let caps = HwCapabilities {
            arch: Arch::Aarch64,
            aes: true,
            pclmul: false,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "armv8-aes-only");
    }

    #[test]
    fn backend_label_arm_no_aes() {
        let caps = HwCapabilities {
            arch: Arch::Aarch64,
            aes: false,
            pclmul: false,
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "aarch64-software");
    }

    #[test]
    fn backend_label_other_arch() {
        let caps = HwCapabilities {
            arch: Arch::Other,
            aes: true,    // even with feature flags set,
            pclmul: true, // Other-arch reports "software".
            sha2: false,
            simd: false,
        };
        assert_eq!(caps.backend_label(), "software");
    }

    #[test]
    fn report_is_non_empty_and_includes_arch() {
        let r = report();
        assert!(r.contains("arch="));
        assert!(r.contains("backend="));
    }

    #[test]
    fn host_arch_is_one_of_supported_or_other() {
        // Smoke-test: detect() darf nicht panicen, egal welche Arch.
        let caps = HwCapabilities::detect();
        assert!(matches!(
            caps.arch,
            Arch::X86_64 | Arch::Aarch64 | Arch::Other
        ));
    }
}
