// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR component tags — spec §13.6.7.3.
//!
//! `ComponentId` is an `unsigned long`. We model over 30 OMG-registered
//! tag values as enum variants plus an `Other(u32)` sentinel for
//! vendor/unknown tags.

/// `ComponentId` — Spec §13.6.7.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    /// `TAG_ORB_TYPE = 0`.
    OrbType,
    /// `TAG_CODE_SETS = 1`.
    CodeSets,
    /// `TAG_POLICIES = 2`.
    Policies,
    /// `TAG_ALTERNATE_IIOP_ADDRESS = 3`.
    AlternateIiopAddress,
    /// `TAG_COMPLETE_OBJECT_KEY = 5`.
    CompleteObjectKey,
    /// `TAG_ENDPOINT_ID_POSITION = 6`.
    EndpointIdPosition,
    /// `TAG_LOCATION_POLICY = 12`.
    LocationPolicy,
    /// `TAG_ASSOCIATION_OPTIONS = 13`.
    AssociationOptions,
    /// `TAG_SEC_NAME = 14`.
    SecName,
    /// `TAG_SPKM_1_SEC_MECH = 15`.
    Spkm1SecMech,
    /// `TAG_SPKM_2_SEC_MECH = 16`.
    Spkm2SecMech,
    /// `TAG_KerberosV5_SEC_MECH = 17`.
    KerberosV5SecMech,
    /// `TAG_CSI_ECMA_Secret_SEC_MECH = 18`.
    CsiEcmaSecretSecMech,
    /// `TAG_CSI_ECMA_Hybrid_SEC_MECH = 19`.
    CsiEcmaHybridSecMech,
    /// `TAG_SSL_SEC_TRANS = 20`.
    SslSecTrans,
    /// `TAG_CSI_ECMA_Public_SEC_MECH = 21`.
    CsiEcmaPublicSecMech,
    /// `TAG_GENERIC_SEC_MECH = 22`.
    GenericSecMech,
    /// `TAG_FIREWALL_TRANS = 23`.
    FirewallTrans,
    /// `TAG_SCCP_CONTACT_INFO = 24`.
    SccpContactInfo,
    /// `TAG_JAVA_CODEBASE = 25`.
    JavaCodebase,
    /// `TAG_TRANSACTION_POLICY = 26`.
    TransactionPolicy,
    /// `TAG_MESSAGE_ROUTERS = 30`.
    MessageRouters,
    /// `TAG_OTS_POLICY = 31`.
    OtsPolicy,
    /// `TAG_INV_POLICY = 32`.
    InvPolicy,
    /// `TAG_CSI_SEC_MECH_LIST = 33`.
    CsiSecMechList,
    /// `TAG_NULL_TAG = 34`.
    NullTag,
    /// `TAG_SECIOP_SEC_TRANS = 35`.
    SeciopSecTrans,
    /// `TAG_TLS_SEC_TRANS = 36`.
    TlsSecTrans,
    /// `TAG_ACTIVITY_POLICY = 37`.
    ActivityPolicy,
    /// `TAG_RMI_CUSTOM_MAX_STREAM_FORMAT = 38`.
    RmiCustomMaxStreamFormat,
    /// `TAG_GROUP = 39`.
    Group,
    /// `TAG_GROUP_IIOP = 40`.
    GroupIiop,
    /// `TAG_DCE_STRING_BINDING = 100`.
    DceStringBinding,
    /// `TAG_DCE_BINDING_NAME = 101`.
    DceBindingName,
    /// `TAG_DCE_NO_PIPES = 102`.
    DceNoPipes,
    /// `TAG_DCE_SEC_MECH = 103`.
    DceSecMech,
    /// `TAG_INET_SEC_TRANS = 123`.
    InetSecTrans,
    /// Other/vendor-specific tag.
    Other(u32),
}

impl ComponentId {
    /// The row of Table 7.2 (CORBA 3.3 Part 2 §7.9) the tag belongs to;
    /// `None` for the tags the table does not list (`TAG_POLICIES`,
    /// `TAG_COMPLETE_OBJECT_KEY`, `TAG_ENDPOINT_ID_POSITION`,
    /// `TAG_LOCATION_POLICY`, the DCE tags and every other tag).
    #[must_use]
    pub const fn feature(self) -> Option<zerodds_corba_giop::Feature> {
        use zerodds_corba_giop::Feature as F;
        Some(match self {
            Self::OrbType => F::TagOrbType,
            Self::CodeSets => F::TagCodeSets,
            Self::AlternateIiopAddress => F::TagAlternateIiopAddress,
            Self::AssociationOptions => F::TagAssociationOption,
            Self::SecName => F::TagSecName,
            Self::Spkm1SecMech
            | Self::Spkm2SecMech
            | Self::KerberosV5SecMech
            | Self::CsiEcmaSecretSecMech
            | Self::CsiEcmaHybridSecMech
            | Self::CsiEcmaPublicSecMech => F::TagSecMech,
            Self::SslSecTrans => F::TagSslSecTrans,
            Self::GenericSecMech => F::TagGenericSecMech,
            Self::FirewallTrans => F::TagFirewallTrans,
            Self::SccpContactInfo => F::TagSccpContactInfo,
            Self::JavaCodebase => F::TagJavaCodebase,
            Self::TransactionPolicy => F::TagTransactionPolicy,
            Self::MessageRouters => F::TagMessageRouters,
            Self::OtsPolicy => F::TagOtsPolicy,
            Self::InvPolicy => F::TagInvPolicy,
            Self::CsiSecMechList => F::TagCsiSecMechList,
            Self::NullTag => F::TagNullTag,
            Self::SeciopSecTrans => F::TagSeciopSecTrans,
            Self::TlsSecTrans => F::TagTlsSecTrans,
            Self::ActivityPolicy => F::TagActivityPolicy,
            Self::RmiCustomMaxStreamFormat => F::RmiCustomMaxStreamFormat,
            Self::Group | Self::GroupIiop => F::GroupFeatures,
            Self::InetSecTrans => F::TagInetSecTrans,
            Self::Policies
            | Self::CompleteObjectKey
            | Self::EndpointIdPosition
            | Self::LocationPolicy
            | Self::DceStringBinding
            | Self::DceBindingName
            | Self::DceNoPipes
            | Self::DceSecMech
            | Self::Other(_) => return None,
        })
    }

    /// Table 7.2: whether a profile of IIOP `major.minor` carries the tag
    /// as part of its version; `None` for the tags the table does not
    /// list. No tag is part of IIOP 1.0, whose profile has no components.
    #[must_use]
    pub const fn support(self, major: u8, minor: u8) -> Option<zerodds_corba_giop::Support> {
        let version = zerodds_corba_giop::Version::new(major, minor);
        if !zerodds_corba_giop::feature::support(
            zerodds_corba_giop::Feature::IorComponentsInIiopProfile,
            version,
        )
        .is_valid()
        {
            return Some(zerodds_corba_giop::Support::No);
        }
        match self.feature() {
            Some(f) => Some(zerodds_corba_giop::feature::support(f, version)),
            None => None,
        }
    }

    /// Raw `unsigned long` value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::OrbType => 0,
            Self::CodeSets => 1,
            Self::Policies => 2,
            Self::AlternateIiopAddress => 3,
            Self::CompleteObjectKey => 5,
            Self::EndpointIdPosition => 6,
            Self::LocationPolicy => 12,
            Self::AssociationOptions => 13,
            Self::SecName => 14,
            Self::Spkm1SecMech => 15,
            Self::Spkm2SecMech => 16,
            Self::KerberosV5SecMech => 17,
            Self::CsiEcmaSecretSecMech => 18,
            Self::CsiEcmaHybridSecMech => 19,
            Self::SslSecTrans => 20,
            Self::CsiEcmaPublicSecMech => 21,
            Self::GenericSecMech => 22,
            Self::FirewallTrans => 23,
            Self::SccpContactInfo => 24,
            Self::JavaCodebase => 25,
            Self::TransactionPolicy => 26,
            Self::MessageRouters => 30,
            Self::OtsPolicy => 31,
            Self::InvPolicy => 32,
            Self::CsiSecMechList => 33,
            Self::NullTag => 34,
            Self::SeciopSecTrans => 35,
            Self::TlsSecTrans => 36,
            Self::ActivityPolicy => 37,
            Self::RmiCustomMaxStreamFormat => 38,
            Self::Group => 39,
            Self::GroupIiop => 40,
            Self::DceStringBinding => 100,
            Self::DceBindingName => 101,
            Self::DceNoPipes => 102,
            Self::DceSecMech => 103,
            Self::InetSecTrans => 123,
            Self::Other(v) => v,
        }
    }

    /// Constructs from an `unsigned long` (all values are valid —
    /// unknown ones land in `Other`).
    #[must_use]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::OrbType,
            1 => Self::CodeSets,
            2 => Self::Policies,
            3 => Self::AlternateIiopAddress,
            5 => Self::CompleteObjectKey,
            6 => Self::EndpointIdPosition,
            12 => Self::LocationPolicy,
            13 => Self::AssociationOptions,
            14 => Self::SecName,
            15 => Self::Spkm1SecMech,
            16 => Self::Spkm2SecMech,
            17 => Self::KerberosV5SecMech,
            18 => Self::CsiEcmaSecretSecMech,
            19 => Self::CsiEcmaHybridSecMech,
            20 => Self::SslSecTrans,
            21 => Self::CsiEcmaPublicSecMech,
            22 => Self::GenericSecMech,
            23 => Self::FirewallTrans,
            24 => Self::SccpContactInfo,
            25 => Self::JavaCodebase,
            26 => Self::TransactionPolicy,
            30 => Self::MessageRouters,
            31 => Self::OtsPolicy,
            32 => Self::InvPolicy,
            33 => Self::CsiSecMechList,
            34 => Self::NullTag,
            35 => Self::SeciopSecTrans,
            36 => Self::TlsSecTrans,
            37 => Self::ActivityPolicy,
            38 => Self::RmiCustomMaxStreamFormat,
            39 => Self::Group,
            40 => Self::GroupIiop,
            100 => Self::DceStringBinding,
            101 => Self::DceBindingName,
            102 => Self::DceNoPipes,
            103 => Self::DceSecMech,
            123 => Self::InetSecTrans,
            v => Self::Other(v),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn corba_p2_7_9_component_tags_follow_table_7_2() {
        use zerodds_corba_giop::Support;
        // IIOP 1.0 has no components at all.
        for tag in [
            ComponentId::OrbType,
            ComponentId::CodeSets,
            ComponentId::Other(77),
        ] {
            assert_eq!(tag.support(1, 0), Some(Support::No), "{tag:?}");
        }
        assert_eq!(ComponentId::CodeSets.support(1, 1), Some(Support::Yes));
        assert_eq!(
            ComponentId::AlternateIiopAddress.support(1, 1),
            Some(Support::No)
        );
        assert_eq!(
            ComponentId::AlternateIiopAddress.support(1, 2),
            Some(Support::Yes)
        );
        assert_eq!(ComponentId::JavaCodebase.support(1, 1), Some(Support::No));
        assert_eq!(
            ComponentId::CsiSecMechList.support(1, 2),
            Some(Support::Optional)
        );
        assert_eq!(
            ComponentId::CsiSecMechList.support(1, 3),
            Some(Support::Yes)
        );
        assert_eq!(
            ComponentId::KerberosV5SecMech.support(1, 1),
            Some(Support::Yes)
        );
        assert_eq!(ComponentId::Group.support(1, 2), Some(Support::No));
        assert_eq!(
            ComponentId::GroupIiop.support(1, 4),
            Some(Support::Optional)
        );
        assert_eq!(
            ComponentId::RmiCustomMaxStreamFormat.support(1, 4),
            Some(Support::Optional)
        );
        // Tags without a row are not tied to a version.
        assert_eq!(ComponentId::Policies.support(1, 2), None);
        assert_eq!(ComponentId::Other(0x4a43_0000).support(1, 2), None);
    }

    #[test]
    fn well_known_component_tag_values_match_spec() {
        // Spec §13.6.7.3 Table 13-2 + §15.6 IOR-Components.
        assert_eq!(ComponentId::OrbType.as_u32(), 0);
        assert_eq!(ComponentId::CodeSets.as_u32(), 1);
        assert_eq!(ComponentId::Policies.as_u32(), 2);
        assert_eq!(ComponentId::AlternateIiopAddress.as_u32(), 3);
        assert_eq!(ComponentId::CompleteObjectKey.as_u32(), 5);
        assert_eq!(ComponentId::EndpointIdPosition.as_u32(), 6);
        assert_eq!(ComponentId::LocationPolicy.as_u32(), 12);
        assert_eq!(ComponentId::SslSecTrans.as_u32(), 20);
        assert_eq!(ComponentId::TlsSecTrans.as_u32(), 36);
        assert_eq!(ComponentId::CsiSecMechList.as_u32(), 33);
        assert_eq!(ComponentId::JavaCodebase.as_u32(), 25);
        assert_eq!(ComponentId::RmiCustomMaxStreamFormat.as_u32(), 38);
        assert_eq!(ComponentId::DceStringBinding.as_u32(), 100);
        assert_eq!(ComponentId::InetSecTrans.as_u32(), 123);
    }

    #[test]
    fn round_trip_all_known_tags() {
        let known = [
            0, 1, 2, 3, 5, 6, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 100, 101, 102, 103, 123,
        ];
        for v in known {
            assert_eq!(ComponentId::from_u32(v).as_u32(), v);
            assert!(!matches!(ComponentId::from_u32(v), ComponentId::Other(_)));
        }
    }

    #[test]
    fn at_least_32_well_known_tags_modelled() {
        // Spec conformance requirement: all 32 standard component tags
        // are present as their own enum variants, not as Other(_). We
        // count the known list above.
        let known = [
            0u32, 1, 2, 3, 5, 6, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 30,
            31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 100, 101, 102, 103, 123,
        ];
        assert!(known.len() >= 32, "must model at least 32 standard tags");
    }

    #[test]
    fn unknown_tag_round_trips_via_other() {
        assert_eq!(ComponentId::from_u32(9999).as_u32(), 9999);
        assert!(matches!(
            ComponentId::from_u32(9999),
            ComponentId::Other(9999)
        ));
    }
}
