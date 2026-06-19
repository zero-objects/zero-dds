// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOP::ServiceContext + ServiceContextList — Spec §15.3.1.
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

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};

/// Standardisierte ServiceContext-Tags (Spec §13.7 / §15.3.1).
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
}

impl ServiceContextTag {
    /// Tag value (`unsigned long`).
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
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

/// ServiceContextList = `sequence<ServiceContext>`.
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

    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, w: &mut BufferWriter) -> GiopResult<()> {
        // sequence<ServiceContext> = unsigned long count + items.
        let count = u32::try_from(self.0.len())
            .map_err(|_| GiopError::Malformed("service_context list too long".into()))?;
        w.write_u32(count)?;
        for ctx in &self.0 {
            // ServiceContext = ServiceId + sequence<octet>.
            w.write_u32(ctx.context_id)?;
            // CDR sequence<octet>: length + data.
            let n = u32::try_from(ctx.context_data.len())
                .map_err(|_| GiopError::Malformed("context_data too long".into()))?;
            w.write_u32(n)?;
            w.write_bytes(&ctx.context_data)?;
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer read error or length overflow.
    pub fn decode(r: &mut BufferReader<'_>) -> GiopResult<Self> {
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
    use zerodds_cdr::Endianness;

    #[test]
    fn standard_tag_values_match_spec() {
        // Spec §13.7.1 + §15.3.1 — Tag-Allokationen.
        assert_eq!(ServiceContextTag::TransactionService.as_u32(), 0);
        assert_eq!(ServiceContextTag::CodeSets.as_u32(), 1);
        assert_eq!(ServiceContextTag::BiDirIiop.as_u32(), 5);
        assert_eq!(ServiceContextTag::SecurityAttributeService.as_u32(), 15);
    }

    #[test]
    fn empty_list_round_trip() {
        let list = ServiceContextList::default();
        let mut w = BufferWriter::new(Endianness::Big);
        list.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        // 4 bytes: count=0.
        assert_eq!(bytes.len(), 4);
        assert_eq!(&bytes, &[0, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn round_trip_with_two_entries_be() {
        let list = ServiceContextList(alloc::vec![
            ServiceContext::new(1, alloc::vec![0xde, 0xad]),
            ServiceContext::new(15, alloc::vec![0xbe, 0xef, 0xca, 0xfe]),
        ]);
        let mut w = BufferWriter::new(Endianness::Big);
        list.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn round_trip_le_byte_order() {
        let list = ServiceContextList(alloc::vec![ServiceContext::new(
            ServiceContextTag::CodeSets.as_u32(),
            alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        )]);
        let mut w = BufferWriter::new(Endianness::Little);
        list.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = ServiceContextList::decode(&mut r).unwrap();
        assert_eq!(decoded, list);
    }
}
