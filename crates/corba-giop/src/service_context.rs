// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOP::ServiceContext + ServiceContextList — CORBA 3.3 Part 2 §7.7.
//!
//! ```text
//! typedef unsigned long  ServiceId;
//! struct ServiceContext {
//!     ServiceId        context_id;
//!     sequence<octet>  context_data;
//! };
//! typedef sequence<ServiceContext> ServiceContextList;
//! ```
//!
//! Transported in every request/reply header and is the
//! standard slot for codeset negotiation, transactional context,
//! security tokens (CSIv2), bidirectional-GIOP listening points, etc.
//!
//! The processing rules of §7.7.2: a context is processed when it is in
//! the OMG range and valid for the GIOP version of the message
//! ([`is_omg_range`], [`ServiceContextTag::support`], Table 7.2); one
//! that is not valid for the version, and one outside the OMG range, may
//! be ignored but is passed on through a bridge and made available to
//! interceptors, and no exception is raised for it.
//! [`ServiceContextList::processable`] is the part of a received list an
//! ORB of `version` acts on; the whole list stays with the message.

use alloc::vec::Vec;

use zerodds_wire::{Reader, Writer};

use crate::error::{GiopError, GiopResult};
use crate::feature::{Feature, Support, support};
use crate::version::Version;

/// The highest service context id of the OMG range: VSCIDs 0 to 15 are
/// reserved for OMG-defined contexts, so ids 0 to 4095 (Part 2 §7.7).
pub const OMG_SERVICE_ID_MAX: u32 = 0x0fff;

/// `true` when `context_id` lies in the range the OMG reserves for its
/// standard service contexts (Part 2 §7.7: the high-order 24 bits, the
/// VSCID, are 0 to 15).
#[must_use]
pub const fn is_omg_range(context_id: u32) -> bool {
    context_id <= OMG_SERVICE_ID_MAX
}

/// Whether a context with `context_id` is valid in GIOP `version`
/// (Part 2 §7.7.2, Table 7.2): `false` only for a standard context whose
/// row in Table 7.2 has an empty cell for the version. A context outside
/// the OMG range, a standard id without a row in Table 7.2 and an id the
/// OMG has not assigned are not tied to a version.
#[must_use]
pub fn is_valid_for(context_id: u32, version: Version) -> bool {
    ServiceContextTag::from_u32(context_id)
        .and_then(|t| t.support(version))
        .is_none_or(Support::is_valid)
}

/// The standard service context ids (CORBA 3.3 Part 2 §7.7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ServiceContextTag {
    /// `IOP::TransactionService` (Spec §15.3.1).
    TransactionService = 0,
    /// `IOP::CodeSets` — Codeset-Negotiation (Spec §13.10).
    CodeSets = 1,
    /// `IOP::ChainBypassCheck`.
    ChainBypassCheck = 2,
    /// `IOP::ChainBypassInfo`.
    ChainBypassInfo = 3,
    /// `IOP::LogicalThreadId`.
    LogicalThreadId = 4,
    /// `IOP::BI_DIR_IIOP` — Bidirectional-GIOP (Spec §15.9).
    BiDirIiop = 5,
    /// `IOP::SendingContextRunTime`.
    SendingContextRunTime = 6,
    /// `IOP::INVOCATION_POLICIES`.
    InvocationPolicies = 7,
    /// `IOP::FORWARDED_IDENTITY`.
    ForwardedIdentity = 8,
    /// `IOP::UnknownExceptionInfo`.
    UnknownExceptionInfo = 9,
    /// `IOP::RTCorbaPriority`.
    RtCorbaPriority = 10,
    /// `IOP::RTCorbaPriorityRange`.
    RtCorbaPriorityRange = 11,
    /// `IOP::FT_GROUP_VERSION`.
    FtGroupVersion = 12,
    /// `IOP::FT_REQUEST`.
    FtRequest = 13,
    /// `IOP::ExceptionDetailMessage`.
    ExceptionDetailMessage = 14,
    /// `IOP::SecurityAttributeService` — CSIv2 (Spec §24).
    SecurityAttributeService = 15,
    /// `IOP::ActivityService`.
    ActivityService = 16,
    /// `IOP::RMICustomMaxStreamFormat`.
    RmiCustomMaxStreamFormat = 17,
    /// `IOP::ACCESS_SESSION_ID`.
    AccessSessionId = 18,
    /// `IOP::SERVICE_SESSION_ID`.
    ServiceSessionId = 19,
    /// `IOP::FIREWALL_PATH`.
    FirewallPath = 20,
    /// `IOP::FIREWALL_PATH_RESP`.
    FirewallPathResp = 21,
}

impl ServiceContextTag {
    /// Every standard id of §7.7.1, in the order of the IDL.
    pub const ALL: [Self; 22] = [
        Self::TransactionService,
        Self::CodeSets,
        Self::ChainBypassCheck,
        Self::ChainBypassInfo,
        Self::LogicalThreadId,
        Self::BiDirIiop,
        Self::SendingContextRunTime,
        Self::InvocationPolicies,
        Self::ForwardedIdentity,
        Self::UnknownExceptionInfo,
        Self::RtCorbaPriority,
        Self::RtCorbaPriorityRange,
        Self::FtGroupVersion,
        Self::FtRequest,
        Self::ExceptionDetailMessage,
        Self::SecurityAttributeService,
        Self::ActivityService,
        Self::RmiCustomMaxStreamFormat,
        Self::AccessSessionId,
        Self::ServiceSessionId,
        Self::FirewallPath,
        Self::FirewallPathResp,
    ];

    /// Tag value (`unsigned long`).
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// The standard id `value`, `None` for every other id.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        if (value as usize) < Self::ALL.len() {
            Some(Self::ALL[value as usize])
        } else {
            None
        }
    }

    /// The row of Table 7.2 the id belongs to; `None` for
    /// `ACCESS_SESSION_ID`, `SERVICE_SESSION_ID`, `FIREWALL_PATH` and
    /// `FIREWALL_PATH_RESP`, which the table does not list.
    #[must_use]
    pub const fn feature(self) -> Option<Feature> {
        Some(match self {
            Self::TransactionService => Feature::TransactionServiceContext,
            Self::CodeSets => Feature::CodeSetsContext,
            Self::ChainBypassCheck | Self::ChainBypassInfo | Self::LogicalThreadId => {
                Feature::DcomBridgingContexts
            }
            Self::BiDirIiop => Feature::BiDirIiopContext,
            Self::SendingContextRunTime => Feature::ObjectByValueContext,
            Self::InvocationPolicies => Feature::AsynchMessagingContext,
            Self::ForwardedIdentity => Feature::FirewallContext,
            Self::UnknownExceptionInfo => Feature::JavaThrowableContext,
            Self::RtCorbaPriority | Self::RtCorbaPriorityRange => Feature::RealtimeCorbaContexts,
            Self::FtGroupVersion => Feature::FtGroupVersionContext,
            Self::FtRequest => Feature::FtRequestContext,
            Self::ExceptionDetailMessage => Feature::ExceptionDetailMessageContext,
            Self::SecurityAttributeService => Feature::SecurityAttributeServiceContext,
            Self::ActivityService => Feature::ActivityServiceContext,
            Self::RmiCustomMaxStreamFormat => Feature::RmiCustomMaxStreamFormat,
            Self::AccessSessionId
            | Self::ServiceSessionId
            | Self::FirewallPath
            | Self::FirewallPathResp => return None,
        })
    }

    /// Table 7.2: whether the context is part of GIOP `version`; `None`
    /// for the ids the table does not list.
    #[must_use]
    pub const fn support(self, version: Version) -> Option<Support> {
        match self.feature() {
            Some(f) => Some(support(f, version)),
            None => None,
        }
    }
}

/// ServiceContext — tag + opaque octet sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceContext {
    /// `context_id` (`ServiceId` = `unsigned long`).
    pub context_id: u32,
    /// `context_data` (`sequence<octet>`).
    pub context_data: Vec<u8>,
}

impl ServiceContext {
    /// Constructor.
    #[must_use]
    pub fn new(context_id: u32, context_data: Vec<u8>) -> Self {
        Self {
            context_id,
            context_data,
        }
    }
}

/// `ServiceContextList` = `sequence<ServiceContext>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceContextList(pub Vec<ServiceContext>);

impl ServiceContextList {
    /// Constructor.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The first context with `context_id`.
    #[must_use]
    pub fn get(&self, context_id: u32) -> Option<&ServiceContext> {
        self.0.iter().find(|c| c.context_id == context_id)
    }

    /// The contexts a receiver of GIOP `version` processes (Part 2
    /// §7.7.2): every context except a standard one that Table 7.2 does
    /// not have in `version`. The others stay in `self`, for the
    /// interceptors and for a bridge that passes the message on.
    #[must_use]
    pub fn processable(&self, version: Version) -> Self {
        Self(
            self.0
                .iter()
                .filter(|c| is_valid_for(c.context_id, version))
                .cloned()
                .collect(),
        )
    }

    /// CDR encode.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, w: &mut Writer) -> GiopResult<()> {
        // sequence<ServiceContext> = unsigned long count + items.
        let count = u32::try_from(self.0.len())
            .map_err(|_| GiopError::Malformed("service_context list too long".into()))?;
        w.write_u32(count);
        for ctx in &self.0 {
            // ServiceContext = ServiceId + sequence<octet>.
            w.write_u32(ctx.context_id);
            // CDR sequence<octet>: length + data.
            let n = u32::try_from(ctx.context_data.len())
                .map_err(|_| GiopError::Malformed("context_data too long".into()))?;
            w.write_u32(n);
            w.write_bytes(&ctx.context_data);
        }
        Ok(())
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error or length overflow.
    pub fn decode(r: &mut Reader<'_>) -> GiopResult<Self> {
        let count = r.read_u32()? as usize;
        let mut out = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            let context_id = r.read_u32()?;
            let n = r.read_u32()? as usize;
            let data = r.read_bytes(n)?;
            out.push(ServiceContext {
                context_id,
                context_data: data.to_vec(),
            });
        }
        Ok(Self(out))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn standard_tag_values_match_spec() {
        // Spec §13.7.1 + §15.3.1 — Tag-Allokationen.
        assert_eq!(ServiceContextTag::TransactionService.as_u32(), 0);
        assert_eq!(ServiceContextTag::CodeSets.as_u32(), 1);
        assert_eq!(ServiceContextTag::BiDirIiop.as_u32(), 5);
        assert_eq!(ServiceContextTag::SecurityAttributeService.as_u32(), 15);
    }

    #[test]
    fn corba_p2_7_7_1_all_standard_service_ids_match_the_idl() {
        // The `const ServiceId` list of Part 2 §7.7.1, in IDL order.
        let idl: [(&str, u32); 22] = [
            ("TransactionService", 0),
            ("CodeSets", 1),
            ("ChainBypassCheck", 2),
            ("ChainBypassInfo", 3),
            ("LogicalThreadId", 4),
            ("BI_DIR_IIOP", 5),
            ("SendingContextRunTime", 6),
            ("INVOCATION_POLICIES", 7),
            ("FORWARDED_IDENTITY", 8),
            ("UnknownExceptionInfo", 9),
            ("RTCorbaPriority", 10),
            ("RTCorbaPriorityRange", 11),
            ("FT_GROUP_VERSION", 12),
            ("FT_REQUEST", 13),
            ("ExceptionDetailMessage", 14),
            ("SecurityAttributeService", 15),
            ("ActivityService", 16),
            ("RMICustomMaxStreamFormat", 17),
            ("ACCESS_SESSION_ID", 18),
            ("SERVICE_SESSION_ID", 19),
            ("FIREWALL_PATH", 20),
            ("FIREWALL_PATH_RESP", 21),
        ];
        for ((name, value), tag) in idl.iter().zip(ServiceContextTag::ALL) {
            assert_eq!(tag.as_u32(), *value, "{name}");
            assert_eq!(ServiceContextTag::from_u32(*value), Some(tag), "{name}");
        }
        assert_eq!(ServiceContextTag::from_u32(22), None);
        assert_eq!(ServiceContextTag::from_u32(0x4a43_0001), None);
    }

    #[test]
    fn corba_p2_7_7_omg_range_is_vscid_0_to_15() {
        assert!(is_omg_range(0));
        assert!(is_omg_range(21));
        assert!(is_omg_range(4095));
        assert!(!is_omg_range(4096));
        assert!(!is_omg_range(0x4a43_0001));
        assert!(!is_omg_range(u32::MAX));
    }

    #[test]
    fn corba_p2_7_7_2_contexts_follow_table_7_2_per_version() {
        use crate::feature::Support;
        let v = |m| Version::new(1, m);
        assert_eq!(ServiceContextTag::CodeSets.support(v(0)), Some(Support::No));
        assert_eq!(
            ServiceContextTag::CodeSets.support(v(1)),
            Some(Support::Yes)
        );
        assert_eq!(
            ServiceContextTag::SecurityAttributeService.support(v(2)),
            Some(Support::Optional)
        );
        assert_eq!(
            ServiceContextTag::RmiCustomMaxStreamFormat.support(v(3)),
            Some(Support::No)
        );
        assert_eq!(
            ServiceContextTag::RmiCustomMaxStreamFormat.support(v(4)),
            Some(Support::Optional)
        );
        for tag in [
            ServiceContextTag::AccessSessionId,
            ServiceContextTag::ServiceSessionId,
            ServiceContextTag::FirewallPath,
            ServiceContextTag::FirewallPathResp,
        ] {
            assert_eq!(tag.support(v(2)), None, "{tag:?} has no row");
            assert!(is_valid_for(tag.as_u32(), v(0)));
        }
        assert!(!is_valid_for(1, v(0)));
        assert!(!is_valid_for(5, v(1)));
        assert!(is_valid_for(5, v(2)));
        // Outside the OMG range: not tied to a version.
        assert!(is_valid_for(0x4a43_0001, v(0)));
    }

    #[test]
    fn corba_p2_7_7_2_processable_keeps_the_valid_contexts_and_the_list_whole() {
        let list = ServiceContextList(alloc::vec![
            ServiceContext::new(0, alloc::vec![1]),
            ServiceContext::new(1, alloc::vec![2]),
            ServiceContext::new(15, alloc::vec![3]),
            ServiceContext::new(21, alloc::vec![4]),
            ServiceContext::new(0x4a43_0001, alloc::vec![5]),
        ]);
        let ids = |l: &ServiceContextList| l.0.iter().map(|c| c.context_id).collect::<Vec<_>>();
        assert_eq!(ids(&list.processable(Version::V1_0)), [0, 21, 0x4a43_0001]);
        assert_eq!(
            ids(&list.processable(Version::V1_1)),
            [0, 1, 21, 0x4a43_0001]
        );
        assert_eq!(
            ids(&list.processable(Version::V1_2)),
            [0, 1, 15, 21, 0x4a43_0001]
        );
        assert_eq!(list.len(), 5);
        assert_eq!(
            list.get(15).map(|c| c.context_data.clone()),
            Some(alloc::vec![3])
        );
        assert!(list.get(16).is_none());
    }

    #[test]
    fn empty_list_round_trip() {
        let list = ServiceContextList::default();
        let mut w = Writer::new(Endianness::Big);
        list.encode(&mut w).unwrap();
        let bytes = w.finish();
        // 4 bytes: count=0.
        assert_eq!(bytes.len(), 4);
        assert_eq!(&bytes, &[0, 0, 0, 0]);
        let mut r = Reader::new(&bytes, Endianness::Big);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn round_trip_with_two_entries_be() {
        let list = ServiceContextList(alloc::vec![
            ServiceContext::new(1, alloc::vec![0xde, 0xad]),
            ServiceContext::new(15, alloc::vec![0xbe, 0xef, 0xca, 0xfe]),
        ]);
        let mut w = Writer::new(Endianness::Big);
        list.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn round_trip_le_byte_order() {
        let list = ServiceContextList(alloc::vec![ServiceContext::new(
            ServiceContextTag::CodeSets.as_u32(),
            alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        )]);
        let mut w = Writer::new(Endianness::Little);
        list.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert_eq!(decoded, list);
    }
}
