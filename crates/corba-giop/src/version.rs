// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP Version (Spec §15.4.1; CORBA 3.3 Part 2 §7.9, §9.4.1, §9.7).
//!
//! GIOP 1.3 and 1.4 keep the message formats of GIOP 1.2: the IDL of
//! Part 2 §9.4 declares `MessageHeader_1_3`, `RequestHeader_1_3`,
//! `ReplyHeader_1_3`, `LocateRequestHeader_1_3`, `LocateReplyHeader_1_3`
//! and `FragmentHeader_1_3` as typedefs of their 1.2 forms, and Table 7.2
//! adds features, not formats. GIOP 1.4 has no typedefs of its own in the
//! text; it is read with the 1.3 forms, which are the 1.2 forms.

/// GIOP version (`octet major; octet minor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    /// Major version (typically `1`).
    pub major: u8,
    /// Minor version (`0` to `4`).
    pub minor: u8,
}

impl Version {
    /// GIOP 1.0 — Spec §15.4.1.
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// GIOP 1.1 — Spec §15.4.1.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// GIOP 1.2 — Spec §15.4.1.
    pub const V1_2: Self = Self { major: 1, minor: 2 };
    /// GIOP 1.3 — Part 2 §7.9 (the 1.2 formats, Table 7.2 column V1.3).
    pub const V1_3: Self = Self { major: 1, minor: 3 };
    /// GIOP 1.4 — Part 2 §7.9 (the 1.2 formats, Table 7.2 column V1.4).
    pub const V1_4: Self = Self { major: 1, minor: 4 };
    /// The highest version this codec reads and writes. A server answers
    /// a message of a higher minor version with a `MessageError` of this
    /// version and closes the connection (Part 2 §9.4.1).
    pub const MAX: Self = Self::V1_4;

    /// Constructor.
    #[must_use]
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// `true` when the version supports fragment messages
    /// (Spec §15.4.9: from GIOP 1.1; in 1.1 only Request+Reply, in 1.2
    /// all messages).
    #[must_use]
    pub const fn supports_fragments(self) -> bool {
        self.is_at_least(1, 1)
    }

    /// `true` when fragment messages are allowed for **all** message
    /// types (Spec §15.4.9: from GIOP 1.2).
    #[must_use]
    pub const fn supports_universal_fragments(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` when a message of this version may be sent as a ZIOP
    /// message (CORBA 3.3 Part 2 §12.1: GIOP 1.2 and later).
    #[must_use]
    pub const fn supports_ziop(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` when the `flags` octet (instead of `byte_order`) is
    /// present in the header (Spec §15.4.1: from GIOP 1.1).
    #[must_use]
    pub const fn uses_flags_octet(self) -> bool {
        self.is_at_least(1, 1)
    }

    /// `true` when the Request/Reply header uses the GIOP 1.2 layout
    /// (`request_id` first, `service_context` last, `TargetAddress`
    /// instead of `object_key`).
    #[must_use]
    pub const fn uses_v1_2_request_layout(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` when bidirectional GIOP is supported (Spec §15.9:
    /// from GIOP 1.2 with `BiDirIIOPServiceContext`).
    #[must_use]
    pub const fn supports_bidirectional(self) -> bool {
        self.is_at_least(1, 2)
    }

    /// `true` for a version this codec reads and writes: 1.0 to 1.4.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        self.major == 1 && self.minor <= Self::MAX.minor
    }

    /// `true` when the implicit operation `_component` belongs to the
    /// version (Table 7.2: from GIOP 1.3).
    #[must_use]
    pub const fn supports_component_operation(self) -> bool {
        self.is_at_least(1, 3)
    }

    /// `true` when the implicit operation `_repository_id` belongs to the
    /// version (Table 7.2: GIOP 1.4).
    #[must_use]
    pub const fn supports_repository_id_operation(self) -> bool {
        self.is_at_least(1, 4)
    }

    /// `true` when a TypeCode indirection may point from one top-level
    /// TypeCode to another top-level TypeCode of the same message
    /// (Part 2 §9.3.5.1: GIOP 1.3 and above); below 1.3 it stays inside
    /// one top-level TypeCode.
    #[must_use]
    pub const fn allows_indirection_between_top_level_typecodes(self) -> bool {
        self.is_at_least(1, 3)
    }

    /// `true` when the host of an IIOP profile may be an IPv6 address
    /// (Table 7.2 "IPV6 addresses in IOR": GIOP 1.4).
    #[must_use]
    pub const fn allows_ipv6_in_ior(self) -> bool {
        self.is_at_least(1, 4)
    }

    /// `true` when UCS-2 and UCS-4 follow the byte order rules of UTF-16
    /// — big-endian unless a byte order mark says otherwise — instead of
    /// the byte order of the message (Part 2 §9.3.1.6: GIOP 1.4; GIOP
    /// 1.1 to 1.3 use the message's byte order).
    #[must_use]
    pub const fn ucs_uses_utf16_byte_order(self) -> bool {
        self.is_at_least(1, 4)
    }

    /// const-fn-capable version comparison.
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
        assert!(Version::V1_2 < Version::V1_3);
        assert!(Version::V1_3 < Version::V1_4);
    }

    /// Part 2 §7.9, §9.4: 1.3 and 1.4 use the 1.2 formats; 1.4 is the
    /// highest version; 1.5 and 2.0 are not supported.
    #[test]
    fn corba_p2_7_9_giop_1_3_and_1_4_take_the_1_2_formats() {
        for v in [Version::V1_3, Version::V1_4] {
            assert!(v.uses_flags_octet());
            assert!(v.uses_v1_2_request_layout());
            assert!(v.supports_universal_fragments());
            assert!(v.supports_bidirectional());
            assert!(v.supports_ziop());
            assert!(v.is_supported());
        }
        assert_eq!(Version::MAX, Version::new(1, 4));
        assert!(!Version::new(1, 5).is_supported());
        assert!(!Version::new(2, 0).is_supported());
        assert!(!Version::new(0, 9).is_supported());
    }

    /// Table 7.2 and §9.3: the features that start at 1.3 and at 1.4.
    #[test]
    fn corba_p2_7_9_features_of_1_3_and_1_4() {
        let all = [
            Version::V1_0,
            Version::V1_1,
            Version::V1_2,
            Version::V1_3,
            Version::V1_4,
        ];
        let from = |f: fn(Version) -> bool| -> Vec<u8> {
            all.iter().filter(|v| f(**v)).map(|v| v.minor).collect()
        };
        assert_eq!(from(Version::supports_component_operation), [3, 4]);
        assert_eq!(
            from(Version::allows_indirection_between_top_level_typecodes),
            [3, 4]
        );
        assert_eq!(from(Version::supports_repository_id_operation), [4]);
        assert_eq!(from(Version::allows_ipv6_in_ior), [4]);
        assert_eq!(from(Version::ucs_uses_utf16_byte_order), [4]);
    }
}
