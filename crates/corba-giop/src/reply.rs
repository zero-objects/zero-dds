// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Reply-Message — Spec §15.4.3.
//!
//! GIOP 1.0/1.1 (`§15.4.3.1`):
//! ```text
//! enum ReplyStatusType_1_0 {
//!     NO_EXCEPTION, USER_EXCEPTION, SYSTEM_EXCEPTION, LOCATION_FORWARD
//! };
//! struct ReplyHeader_1_0 {
//!     IOP::ServiceContextList service_context;
//!     unsigned long           request_id;
//!     ReplyStatusType_1_0     reply_status;
//! };
//! ```
//!
//! GIOP 1.2 (`§15.4.3.2`):
//! ```text
//! enum ReplyStatusType_1_2 {
//!     NO_EXCEPTION, USER_EXCEPTION, SYSTEM_EXCEPTION,
//!     LOCATION_FORWARD, LOCATION_FORWARD_PERM, NEEDS_ADDRESSING_MODE
//! };
//! struct ReplyHeader_1_2 {
//!     unsigned long           request_id;
//!     ReplyStatusType_1_2     reply_status;
//!     IOP::ServiceContextList service_context;
//!     // body 8-aligned
//! };
//! ```

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};
use crate::service_context::ServiceContextList;
use crate::version::Version;

/// Reply-Status — alle 6 Spec-Werte (Spec §15.4.3.1 + §15.4.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ReplyStatusType {
    /// `NO_EXCEPTION` — Operation success.
    NoException = 0,
    /// `USER_EXCEPTION` — IDL-deklarierte User-Exception.
    UserException = 1,
    /// `SYSTEM_EXCEPTION` — System-Level-Fault.
    SystemException = 2,
    /// `LOCATION_FORWARD` — Server bittet Client, an einen anderen
    /// Endpoint zu gehen (transient).
    LocationForward = 3,
    /// `LOCATION_FORWARD_PERM` — wie oben, aber persistent (GIOP 1.2+).
    LocationForwardPerm = 4,
    /// `NEEDS_ADDRESSING_MODE` — Server moechte einen anderen
    /// `TargetAddress`-Discriminator (GIOP 1.2+).
    NeedsAddressingMode = 5,
}

impl ReplyStatusType {
    /// Diskriminanten-Wert.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Parsing aus `unsigned long`.
    ///
    /// # Errors
    /// `UnknownReplyStatus` ausserhalb 0..=5; `Malformed` wenn der
    /// Wert nicht zur GIOP-Version passt (LOCATION_FORWARD_PERM /
    /// NEEDS_ADDRESSING_MODE in 1.0/1.1 nicht erlaubt).
    pub fn from_u32(value: u32, version: Version) -> GiopResult<Self> {
        match value {
            0 => Ok(Self::NoException),
            1 => Ok(Self::UserException),
            2 => Ok(Self::SystemException),
            3 => Ok(Self::LocationForward),
            4 if version.uses_v1_2_request_layout() => Ok(Self::LocationForwardPerm),
            5 if version.uses_v1_2_request_layout() => Ok(Self::NeedsAddressingMode),
            4..=5 => Err(GiopError::Malformed(alloc::format!(
                "ReplyStatus {value} only valid in GIOP 1.2+, got {}.{}",
                version.major,
                version.minor
            ))),
            other => Err(GiopError::UnknownReplyStatus(other)),
        }
    }
}

/// Reply-Message-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// `request_id`.
    pub request_id: u32,
    /// Reply-Status-Discriminator.
    pub reply_status: ReplyStatusType,
    /// `service_context_list`.
    pub service_context: ServiceContextList,
    /// Body-Bytes (CDR-encoded `out`/`return`/`Exception`-Inhalt).
    /// In GIOP 1.2 8-Byte-aligned ab Header-Start.
    pub body: Vec<u8>,
}

impl Reply {
    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        // Status-Validierung gegen Version.
        match self.reply_status {
            ReplyStatusType::LocationForwardPerm | ReplyStatusType::NeedsAddressingMode
                if !version.uses_v1_2_request_layout() =>
            {
                return Err(GiopError::Malformed(alloc::format!(
                    "ReplyStatus {:?} only valid in GIOP 1.2+",
                    self.reply_status
                )));
            }
            _ => {}
        }
        if version.uses_v1_2_request_layout() {
            w.write_u32(self.request_id)?;
            w.write_u32(self.reply_status.as_u32())?;
            self.service_context.encode(w)?;
            w.align(8);
        } else {
            self.service_context.encode(w)?;
            w.write_u32(self.request_id)?;
            w.write_u32(self.reply_status.as_u32())?;
        }
        w.write_bytes(&self.body)?;
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler oder unbekannter Status.
    pub fn decode(version: Version, r: &mut BufferReader<'_>) -> GiopResult<Self> {
        if version.uses_v1_2_request_layout() {
            let request_id = r.read_u32()?;
            let status_raw = r.read_u32()?;
            let reply_status = ReplyStatusType::from_u32(status_raw, version)?;
            let service_context = ServiceContextList::decode(r)?;
            r.align(8)?;
            let body = r.read_bytes(r.remaining())?.to_vec();
            Ok(Self {
                request_id,
                reply_status,
                service_context,
                body,
            })
        } else {
            let service_context = ServiceContextList::decode(r)?;
            let request_id = r.read_u32()?;
            let status_raw = r.read_u32()?;
            let reply_status = ReplyStatusType::from_u32(status_raw, version)?;
            let body = r.read_bytes(r.remaining())?.to_vec();
            Ok(Self {
                request_id,
                reply_status,
                service_context,
                body,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn reply_status_values_match_spec() {
        // Spec §15.4.3.1 + §15.4.3.2.
        assert_eq!(ReplyStatusType::NoException.as_u32(), 0);
        assert_eq!(ReplyStatusType::UserException.as_u32(), 1);
        assert_eq!(ReplyStatusType::SystemException.as_u32(), 2);
        assert_eq!(ReplyStatusType::LocationForward.as_u32(), 3);
        assert_eq!(ReplyStatusType::LocationForwardPerm.as_u32(), 4);
        assert_eq!(ReplyStatusType::NeedsAddressingMode.as_u32(), 5);
    }

    #[test]
    fn round_trip_giop_1_0_no_exception() {
        let r = Reply {
            request_id: 42,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xde, 0xad],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        r.encode(Version::V1_0, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut rd = BufferReader::new(&bytes, Endianness::Big);
        let decoded = Reply::decode(Version::V1_0, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn round_trip_giop_1_2_user_exception() {
        let r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        r.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut rd = BufferReader::new(&bytes, Endianness::Little);
        let decoded = Reply::decode(Version::V1_2, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn round_trip_location_forward_perm_in_1_2() {
        let r = Reply {
            request_id: 5,
            reply_status: ReplyStatusType::LocationForwardPerm,
            service_context: ServiceContextList::default(),
            body: alloc::vec::Vec::new(),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        r.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut rd = BufferReader::new(&bytes, Endianness::Big);
        let decoded = Reply::decode(Version::V1_2, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn location_forward_perm_in_giop_1_0_is_rejected() {
        let r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::LocationForwardPerm,
            service_context: ServiceContextList::default(),
            body: alloc::vec::Vec::new(),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        let err = r.encode(Version::V1_0, &mut w).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }

    #[test]
    fn unknown_reply_status_is_diagnostic() {
        let mut w = BufferWriter::new(Endianness::Big);
        // Service-context list (empty) + request_id + invalid status.
        ServiceContextList::default().encode(&mut w).unwrap();
        w.write_u32(1).unwrap();
        w.write_u32(99).unwrap();
        let bytes = w.into_bytes();
        let mut rd = BufferReader::new(&bytes, Endianness::Big);
        let err = Reply::decode(Version::V1_0, &mut rd).unwrap_err();
        assert!(matches!(err, GiopError::UnknownReplyStatus(99)));
    }
}
