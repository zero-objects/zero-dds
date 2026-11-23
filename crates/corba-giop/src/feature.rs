// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Feature support tied to the GIOP minor version — CORBA 3.3 Part 2
//! §7.9, Table 7.2.
//!
//! The table names, for every service context, IOR component, implicit
//! operation and TypeCode kind added after GIOP 1.0, the GIOP versions
//! (1.0 to 1.4) in which it is part of the protocol: not at all, as an
//! option, or as a part every ORB of that version has. [`support`] is the
//! table as data, one [`Feature`] per row and one column per version;
//! [`crate::service_context::ServiceContextTag::support`],
//! `zerodds_corba_ior::ComponentId::support` and the `IOP::Codec` of
//! `zerodds-corba-ccm` read it.
//!
//! "optional$" and "optional$$" of the GIOP 1.2 column are
//! [`Support::Optional`]: the note under the table says these features
//! came after CORBA 2.3 and must be supported by a GIOP 1.2
//! implementation that connects CORBA 2.4 or later ORBs, which this ORB
//! is. The Realtime CORBA rows are optional in every version they appear
//! in ("Realtime CORBA only").
//!
//! A version above 1.4 is answered with the 1.4 column: §7.8.2.1 asks for
//! the CDR encapsulation "for all future versions of GIOP as they arise",
//! and Table 7.2 has no later column.

use crate::version::Version;

/// One cell of Table 7.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Support {
    /// The cell is empty: the feature is not part of this GIOP version.
    No,
    /// "optional": part of the version for ORBs that implement it.
    Optional,
    /// "yes": part of the version.
    Yes,
}

impl Support {
    /// `true` for [`Support::Yes`] and [`Support::Optional`]: the feature
    /// exists in the version (CORBA 3.3 Part 2 §7.7.2, "valid for the
    /// supported GIOP version").
    #[must_use]
    pub const fn is_valid(self) -> bool {
        !matches!(self, Self::No)
    }
}

/// One row of Table 7.2, in the order of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// TransactionService Service Context.
    TransactionServiceContext,
    /// CodeSets Service Context.
    CodeSetsContext,
    /// DCOM Bridging Service Contexts: ChainBypassCheck,
    /// ChainBypassInfo, LogicalThreadId.
    DcomBridgingContexts,
    /// Object by Value Service Context: SendingContextRunTime.
    ObjectByValueContext,
    /// Bi-Directional IIOP Service Context: BI_DIR_IIOP.
    BiDirIiopContext,
    /// Asynch Messaging Service Context: INVOCATION_POLICIES.
    AsynchMessagingContext,
    /// Firewall Service Context: FORWARDED_IDENTITY.
    FirewallContext,
    /// Java Language Throwable Service Context: UnknownExceptionInfo.
    JavaThrowableContext,
    /// Realtime CORBA Service Contexts: RTCorbaPriority,
    /// RTCorbaPriorityRange.
    RealtimeCorbaContexts,
    /// ExceptionDetailMessage Service Context.
    ExceptionDetailMessageContext,
    /// FT_GROUP_VERSION.
    FtGroupVersionContext,
    /// FT_REQUEST.
    FtRequestContext,
    /// SecurityAttributeService.
    SecurityAttributeServiceContext,
    /// ActivityService.
    ActivityServiceContext,
    /// IOR components in IIOP profile.
    IorComponentsInIiopProfile,
    /// TAG_ORB_TYPE.
    TagOrbType,
    /// TAG_CODE_SETS.
    TagCodeSets,
    /// TAG_ALTERNATE_IIOP_ADDRESS.
    TagAlternateIiopAddress,
    /// TAG_ASSOCIATION_OPTION.
    TagAssociationOption,
    /// TAG_SEC_NAME.
    TagSecName,
    /// TAG_SSL_SEC_TRANS.
    TagSslSecTrans,
    /// TAG_GENERIC_SEC_MECH.
    TagGenericSecMech,
    /// TAG_*_SEC_MECH: TAG_SPKM_1_SEC_MECH, TAG_SPKM_2_SEC_MECH,
    /// TAG_KerberosV5_SEC_MECH, TAG_CSI_ECMA_Secret_SEC_MECH,
    /// TAG_CSI_ECMA_Hybrid_SEC_MECH, TAG_CSI_ECMA_Public_SEC_MECH.
    TagSecMech,
    /// TAG_JAVA_CODEBASE.
    TagJavaCodebase,
    /// TAG_FIREWALL_TRANS.
    TagFirewallTrans,
    /// TAG_SCCP_CONTACT_INFO.
    TagSccpContactInfo,
    /// TAG_TRANSACTION_POLICY.
    TagTransactionPolicy,
    /// TAG_MESSAGE_ROUTERS.
    TagMessageRouters,
    /// TAG_OTS_POLICY.
    TagOtsPolicy,
    /// TAG_INV_POLICY.
    TagInvPolicy,
    /// TAG_INET_SEC_TRANS.
    TagInetSecTrans,
    /// Extended IDL data types: `long long`, `unsigned long long`,
    /// `long double`, `wchar`, `wstring`, `fixed`.
    ExtendedIdlDataTypes,
    /// Bi-Directional GIOP Features.
    BiDirectionalGiop,
    /// Value types and Abstract Interfaces.
    ValueTypesAndAbstractInterfaces,
    /// TAG_CSI_SEC_MECH_LIST.
    TagCsiSecMechList,
    /// TAG_NULL_TAG.
    TagNullTag,
    /// TAG_SECIOP_SEC_TRANS, TAG_IIOP_SEC_TRANS.
    TagSeciopSecTrans,
    /// TAG_TLS_SEC_TRANS.
    TagTlsSecTrans,
    /// TAG_ACTIVITY_POLICY.
    TagActivityPolicy,
    /// The implicit operation `_component`.
    ComponentOperation,
    /// `tk_abstract_interface`, `tk_local_interface`.
    AbstractAndLocalInterfaceTypeCodes,
    /// `tk_component`, `tk_home`, `tk_event`.
    ComponentTypeCodes,
    /// The implicit operation `_repository_id`.
    RepositoryIdOperation,
    /// IPV6 addresses in IOR.
    Ipv6AddressesInIor,
    /// TAG_GROUP, TAG_GROUP_IIOP and TAG_UIPMC + Group features.
    GroupFeatures,
    /// RMICustomMaxStreamFormat, TAG_RMI_CUSTOM_MAX_STREAM_FORMAT.
    RmiCustomMaxStreamFormat,
}

impl Feature {
    /// Every row of Table 7.2, in the order of the table.
    pub const ALL: [Self; 46] = [
        Self::TransactionServiceContext,
        Self::CodeSetsContext,
        Self::DcomBridgingContexts,
        Self::ObjectByValueContext,
        Self::BiDirIiopContext,
        Self::AsynchMessagingContext,
        Self::FirewallContext,
        Self::JavaThrowableContext,
        Self::RealtimeCorbaContexts,
        Self::ExceptionDetailMessageContext,
        Self::FtGroupVersionContext,
        Self::FtRequestContext,
        Self::SecurityAttributeServiceContext,
        Self::ActivityServiceContext,
        Self::IorComponentsInIiopProfile,
        Self::TagOrbType,
        Self::TagCodeSets,
        Self::TagAlternateIiopAddress,
        Self::TagAssociationOption,
        Self::TagSecName,
        Self::TagSslSecTrans,
        Self::TagGenericSecMech,
        Self::TagSecMech,
        Self::TagJavaCodebase,
        Self::TagFirewallTrans,
        Self::TagSccpContactInfo,
        Self::TagTransactionPolicy,
        Self::TagMessageRouters,
        Self::TagOtsPolicy,
        Self::TagInvPolicy,
        Self::TagInetSecTrans,
        Self::ExtendedIdlDataTypes,
        Self::BiDirectionalGiop,
        Self::ValueTypesAndAbstractInterfaces,
        Self::TagCsiSecMechList,
        Self::TagNullTag,
        Self::TagSeciopSecTrans,
        Self::TagTlsSecTrans,
        Self::TagActivityPolicy,
        Self::ComponentOperation,
        Self::AbstractAndLocalInterfaceTypeCodes,
        Self::ComponentTypeCodes,
        Self::RepositoryIdOperation,
        Self::Ipv6AddressesInIor,
        Self::GroupFeatures,
        Self::RmiCustomMaxStreamFormat,
    ];

    /// The row of a TypeCode kind (`CORBA::TCKind` value, Part 2 Table
    /// 9.2) that Table 7.2 ties to a GIOP version; `None` for the kinds
    /// of GIOP 1.0.
    #[must_use]
    pub const fn of_tckind(kind: u32) -> Option<Self> {
        match kind {
            // tk_longlong, tk_ulonglong, tk_longdouble, tk_wchar,
            // tk_wstring, tk_fixed.
            23..=28 => Some(Self::ExtendedIdlDataTypes),
            // tk_value, tk_value_box.
            29..=30 => Some(Self::ValueTypesAndAbstractInterfaces),
            // tk_abstract_interface, tk_local_interface.
            32..=33 => Some(Self::AbstractAndLocalInterfaceTypeCodes),
            // tk_component, tk_home, tk_event.
            34..=36 => Some(Self::ComponentTypeCodes),
            _ => None,
        }
    }

    /// The row of an implicit operation (Part 2 §9.4.2); `None` for the
    /// operations of GIOP 1.0 (`_interface`, `_is_a`, `_non_existent`,
    /// `_domain_managers`) and for every other name.
    #[must_use]
    pub fn of_operation(operation: &str) -> Option<Self> {
        match operation {
            crate::object_ops::OP_COMPONENT => Some(Self::ComponentOperation),
            crate::object_ops::OP_REPOSITORY_ID => Some(Self::RepositoryIdOperation),
            _ => None,
        }
    }
}

/// The column of `version` in Table 7.2: 0 for 1.0 up to 4 for 1.4 and
/// every later version.
const fn column(version: Version) -> usize {
    if version.major == 0 {
        0
    } else if version.major > 1 || version.minor >= 4 {
        4
    } else {
        version.minor as usize
    }
}

/// The cells of one row, V1.0 to V1.4.
type Row = [Support; 5];

const N: Support = Support::No;
const O: Support = Support::Optional;
const Y: Support = Support::Yes;

/// From GIOP 1.0.
const ALL_VERSIONS: Row = [Y, Y, Y, Y, Y];
/// From GIOP 1.1.
const FROM_1_1: Row = [N, Y, Y, Y, Y];
/// From GIOP 1.2.
const FROM_1_2: Row = [N, N, Y, Y, Y];
/// Optional in GIOP 1.2, part of 1.3 and 1.4.
const OPTIONAL_1_2: Row = [N, N, O, Y, Y];
/// Optional from GIOP 1.2 on.
const REALTIME: Row = [N, N, O, O, O];
/// From GIOP 1.3.
const FROM_1_3: Row = [N, N, N, Y, Y];
/// GIOP 1.4 only.
const ONLY_1_4: Row = [N, N, N, N, Y];
/// Optional in GIOP 1.4.
const OPTIONAL_1_4: Row = [N, N, N, N, O];

/// The row of `feature`, transcribed from Table 7.2.
const fn row(feature: Feature) -> Row {
    match feature {
        Feature::TransactionServiceContext => ALL_VERSIONS,
        Feature::CodeSetsContext => FROM_1_1,
        Feature::DcomBridgingContexts
        | Feature::ObjectByValueContext
        | Feature::BiDirIiopContext
        | Feature::JavaThrowableContext => FROM_1_2,
        Feature::AsynchMessagingContext
        | Feature::FirewallContext
        | Feature::ExceptionDetailMessageContext
        | Feature::FtGroupVersionContext
        | Feature::FtRequestContext
        | Feature::SecurityAttributeServiceContext
        | Feature::ActivityServiceContext => OPTIONAL_1_2,
        Feature::RealtimeCorbaContexts => REALTIME,
        Feature::IorComponentsInIiopProfile
        | Feature::TagOrbType
        | Feature::TagCodeSets
        | Feature::TagAssociationOption
        | Feature::TagSecName
        | Feature::TagSslSecTrans
        | Feature::TagGenericSecMech
        | Feature::TagSecMech
        | Feature::ExtendedIdlDataTypes => FROM_1_1,
        Feature::TagAlternateIiopAddress
        | Feature::TagJavaCodebase
        | Feature::BiDirectionalGiop
        | Feature::ValueTypesAndAbstractInterfaces => FROM_1_2,
        Feature::TagFirewallTrans
        | Feature::TagSccpContactInfo
        | Feature::TagTransactionPolicy
        | Feature::TagMessageRouters
        | Feature::TagOtsPolicy
        | Feature::TagInvPolicy
        | Feature::TagInetSecTrans
        | Feature::TagCsiSecMechList
        | Feature::TagNullTag
        | Feature::TagSeciopSecTrans
        | Feature::TagTlsSecTrans
        | Feature::TagActivityPolicy
        | Feature::AbstractAndLocalInterfaceTypeCodes => OPTIONAL_1_2,
        Feature::ComponentOperation | Feature::ComponentTypeCodes => FROM_1_3,
        Feature::RepositoryIdOperation | Feature::Ipv6AddressesInIor => ONLY_1_4,
        Feature::GroupFeatures | Feature::RmiCustomMaxStreamFormat => OPTIONAL_1_4,
    }
}

/// Table 7.2: whether `feature` is part of GIOP `version`.
#[must_use]
pub const fn support(feature: Feature, version: Version) -> Support {
    row(feature)[column(version)]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn versions() -> [Version; 5] {
        [
            Version::new(1, 0),
            Version::new(1, 1),
            Version::new(1, 2),
            Version::new(1, 3),
            Version::new(1, 4),
        ]
    }

    /// Table 7.2 written out cell by cell, as the specification prints
    /// it: "" for an empty cell, "yes", "optional" (the `$` marks
    /// dropped). Kept apart from the row constants above so that a
    /// wrong constant shows up here.
    const TABLE: [(Feature, [&str; 5]); 46] = [
        (
            Feature::TransactionServiceContext,
            ["yes", "yes", "yes", "yes", "yes"],
        ),
        (Feature::CodeSetsContext, ["", "yes", "yes", "yes", "yes"]),
        (Feature::DcomBridgingContexts, ["", "", "yes", "yes", "yes"]),
        (Feature::ObjectByValueContext, ["", "", "yes", "yes", "yes"]),
        (Feature::BiDirIiopContext, ["", "", "yes", "yes", "yes"]),
        (
            Feature::AsynchMessagingContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::FirewallContext, ["", "", "optional", "yes", "yes"]),
        (Feature::JavaThrowableContext, ["", "", "yes", "yes", "yes"]),
        (
            Feature::RealtimeCorbaContexts,
            ["", "", "optional", "optional", "optional"],
        ),
        (
            Feature::ExceptionDetailMessageContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::FtGroupVersionContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::FtRequestContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::SecurityAttributeServiceContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::ActivityServiceContext,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::IorComponentsInIiopProfile,
            ["", "yes", "yes", "yes", "yes"],
        ),
        (Feature::TagOrbType, ["", "yes", "yes", "yes", "yes"]),
        (Feature::TagCodeSets, ["", "yes", "yes", "yes", "yes"]),
        (
            Feature::TagAlternateIiopAddress,
            ["", "", "yes", "yes", "yes"],
        ),
        (
            Feature::TagAssociationOption,
            ["", "yes", "yes", "yes", "yes"],
        ),
        (Feature::TagSecName, ["", "yes", "yes", "yes", "yes"]),
        (Feature::TagSslSecTrans, ["", "yes", "yes", "yes", "yes"]),
        (Feature::TagGenericSecMech, ["", "yes", "yes", "yes", "yes"]),
        (Feature::TagSecMech, ["", "yes", "yes", "yes", "yes"]),
        (Feature::TagJavaCodebase, ["", "", "yes", "yes", "yes"]),
        (
            Feature::TagFirewallTrans,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::TagSccpContactInfo,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::TagTransactionPolicy,
            ["", "", "optional", "yes", "yes"],
        ),
        (
            Feature::TagMessageRouters,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::TagOtsPolicy, ["", "", "optional", "yes", "yes"]),
        (Feature::TagInvPolicy, ["", "", "optional", "yes", "yes"]),
        (Feature::TagInetSecTrans, ["", "", "optional", "yes", "yes"]),
        (
            Feature::ExtendedIdlDataTypes,
            ["", "yes", "yes", "yes", "yes"],
        ),
        (Feature::BiDirectionalGiop, ["", "", "yes", "yes", "yes"]),
        (
            Feature::ValueTypesAndAbstractInterfaces,
            ["", "", "yes", "yes", "yes"],
        ),
        (
            Feature::TagCsiSecMechList,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::TagNullTag, ["", "", "optional", "yes", "yes"]),
        (
            Feature::TagSeciopSecTrans,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::TagTlsSecTrans, ["", "", "optional", "yes", "yes"]),
        (
            Feature::TagActivityPolicy,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::ComponentOperation, ["", "", "", "yes", "yes"]),
        (
            Feature::AbstractAndLocalInterfaceTypeCodes,
            ["", "", "optional", "yes", "yes"],
        ),
        (Feature::ComponentTypeCodes, ["", "", "", "yes", "yes"]),
        (Feature::RepositoryIdOperation, ["", "", "", "", "yes"]),
        (Feature::Ipv6AddressesInIor, ["", "", "", "", "yes"]),
        (Feature::GroupFeatures, ["", "", "", "", "optional"]),
        (
            Feature::RmiCustomMaxStreamFormat,
            ["", "", "", "", "optional"],
        ),
    ];

    #[test]
    fn corba_p2_7_9_table_7_2_every_row_and_column() {
        assert_eq!(TABLE.len(), Feature::ALL.len());
        for ((feature, cells), listed) in TABLE.iter().zip(Feature::ALL) {
            assert_eq!(*feature, listed, "rows in the order of the table");
            for (version, cell) in versions().into_iter().zip(cells) {
                let expected = match *cell {
                    "" => Support::No,
                    "optional" => Support::Optional,
                    "yes" => Support::Yes,
                    other => panic!("cell {other}"),
                };
                assert_eq!(
                    support(*feature, version),
                    expected,
                    "{feature:?} in GIOP {}.{}",
                    version.major,
                    version.minor
                );
            }
        }
    }

    #[test]
    fn corba_p2_7_9_codesets_context_is_not_valid_in_giop_1_0() {
        assert!(!support(Feature::CodeSetsContext, Version::V1_0).is_valid());
        assert!(support(Feature::CodeSetsContext, Version::V1_1).is_valid());
    }

    #[test]
    fn corba_p2_7_9_alternate_iiop_address_needs_iiop_1_2() {
        assert_eq!(
            support(Feature::TagAlternateIiopAddress, Version::V1_1),
            Support::No
        );
        assert_eq!(
            support(Feature::TagAlternateIiopAddress, Version::V1_2),
            Support::Yes
        );
    }

    #[test]
    fn corba_p2_7_9_versions_after_1_4_take_the_1_4_column() {
        for feature in Feature::ALL {
            assert_eq!(
                support(feature, Version::new(1, 9)),
                support(feature, Version::new(1, 4))
            );
            assert_eq!(
                support(feature, Version::new(2, 0)),
                support(feature, Version::new(1, 4))
            );
        }
    }

    #[test]
    fn corba_p2_7_9_tckinds_and_operations_map_to_their_rows() {
        // GIOP 1.0 kinds have no row.
        for kind in 0..=22 {
            assert_eq!(Feature::of_tckind(kind), None, "kind {kind}");
        }
        for kind in [23, 24, 25, 26, 27, 28] {
            assert_eq!(
                Feature::of_tckind(kind),
                Some(Feature::ExtendedIdlDataTypes)
            );
        }
        assert_eq!(
            Feature::of_tckind(29),
            Some(Feature::ValueTypesAndAbstractInterfaces)
        );
        assert_eq!(
            Feature::of_tckind(30),
            Some(Feature::ValueTypesAndAbstractInterfaces)
        );
        assert_eq!(Feature::of_tckind(31), None, "tk_native");
        assert_eq!(
            Feature::of_tckind(33),
            Some(Feature::AbstractAndLocalInterfaceTypeCodes)
        );
        for kind in [34, 35, 36] {
            assert_eq!(Feature::of_tckind(kind), Some(Feature::ComponentTypeCodes));
        }
        assert_eq!(
            Feature::of_operation("_component"),
            Some(Feature::ComponentOperation)
        );
        assert_eq!(
            Feature::of_operation("_repository_id"),
            Some(Feature::RepositoryIdOperation)
        );
        assert_eq!(Feature::of_operation("_is_a"), None);
    }
}
