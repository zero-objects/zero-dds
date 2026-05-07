// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP Version (Spec §15.4.1).

/// GIOP-Version (`octet major; octet minor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    /// Major-Version (typisch `1`).
    pub major: u8,
    /// Minor-Version (`0`, `1`, `2`).
    pub minor: u8,
}

impl Version {
    /// GIOP 1.0 — Spec §15.4.1.
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// GIOP 1.1 — Spec §15.4.1.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// GIOP 1.2 — Spec §15.4.1.
    pub const V1_2: Self = Self { major: 1, minor: 2 };

    /// Konstruktor.
    #[must_use]
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// `true` wenn die Version Fragment-Messages unterstuetzt
    /// (Spec §15.4.9: ab GIOP 1.1; in 1.1 nur Request+Reply, in 1.2
    /// alle Messages).
    #[must_use]
    pub const fn supports_fragments(self) -> bool {
        self.is_at_least(1, 1)
    }

    /// `true` wenn Fragment-Messages fuer **alle** Message-Types
    /// erlaubt sind (Spec §15.4.9: ab GIOP 1.2).
    #[must_use]
    pub const fn supports_universal_fragments(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` wenn das `flags`-Octet (statt `byte_order`) im Header
    /// steht (Spec §15.4.1: ab GIOP 1.1).
    #[must_use]
    pub const fn uses_flags_octet(self) -> bool {
        self.is_at_least(1, 1)
    }

    /// `true` wenn der Request/Reply-Header das GIOP-1.2-Layout
    /// nutzt (`request_id` zuerst, `service_context` zuletzt,
    /// `TargetAddress` statt `object_key`).
    #[must_use]
    pub const fn uses_v1_2_request_layout(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` wenn Bidirectional-GIOP unterstuetzt ist (Spec §15.9:
    /// ab GIOP 1.2 mit `BiDirIIOPServiceContext`).
    #[must_use]
    pub const fn supports_bidirectional(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// const-fn-faehiger Versions-Vergleich.
    #[must_use]
    pub const fn is_at_least(self, major: u8, minor: u8) -> bool {
        if self.major > major {
            true
        } else if self.major == major {
            self.minor >= minor
        } else {
            false
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn version_constants_match_spec() {
        assert_eq!(Version::V1_0, Version::new(1, 0));
        assert_eq!(Version::V1_1, Version::new(1, 1));
        assert_eq!(Version::V1_2, Version::new(1, 2));
    }

    #[test]
    fn fragment_support_starts_at_1_1() {
        assert!(!Version::V1_0.supports_fragments());
        assert!(Version::V1_1.supports_fragments());
        assert!(Version::V1_2.supports_fragments());
    }

    #[test]
    fn universal_fragments_only_in_1_2() {
        assert!(!Version::V1_0.supports_universal_fragments());
        assert!(!Version::V1_1.supports_universal_fragments());
        assert!(Version::V1_2.supports_universal_fragments());
    }

    #[test]
    fn flags_octet_replaces_byte_order_at_1_1() {
        assert!(!Version::V1_0.uses_flags_octet());
        assert!(Version::V1_1.uses_flags_octet());
        assert!(Version::V1_2.uses_flags_octet());
    }

    #[test]
    fn v1_2_request_layout_only_in_1_2_plus() {
        assert!(!Version::V1_0.uses_v1_2_request_layout());
        assert!(!Version::V1_1.uses_v1_2_request_layout());
        assert!(Version::V1_2.uses_v1_2_request_layout());
    }

    #[test]
    fn bidirectional_starts_at_1_2() {
        assert!(!Version::V1_0.supports_bidirectional());
        assert!(!Version::V1_1.supports_bidirectional());
        assert!(Version::V1_2.supports_bidirectional());
    }

    #[test]
    fn version_ordering_works() {
        assert!(Version::V1_0 < Version::V1_1);
        assert!(Version::V1_1 < Version::V1_2);
    }
}
