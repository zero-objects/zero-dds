// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! LocateReply message — spec §15.4.6.
//!
//! ```text
//! enum LocateStatusType_1_0 {
//!     UNKNOWN_OBJECT, OBJECT_HERE, OBJECT_FORWARD
//! };
//! enum LocateStatusType_1_2 {
//!     UNKNOWN_OBJECT, OBJECT_HERE, OBJECT_FORWARD,
//!     OBJECT_FORWARD_PERM, LOC_SYSTEM_EXCEPTION,
//!     LOC_NEEDS_ADDRESSING_MODE
//! };
//!
//! struct LocateReplyHeader {
//!     unsigned long       request_id;
//!     LocateStatusType    locate_status;
//!     // body depending on status (GIOP 1.2: 8-aligned)
//! };
//! ```

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};
use crate::version::Version;

/// LocateStatusType — all 6 values (spec §15.4.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum LocateStatusType {
    /// `UNKNOWN_OBJECT` — the server does not know the object.
    UnknownObject = 0,
    /// `OBJECT_HERE` — the object is available here.
    ObjectHere = 1,
    /// `OBJECT_FORWARD` — the server requests a forward (transient).
    ObjectForward = 2,
    /// `OBJECT_FORWARD_PERM` — persistent forward hint (GIOP 1.2+).
    ObjectForwardPerm = 3,
    /// `LOC_SYSTEM_EXCEPTION` — system exception instead of a locate reply
    /// (GIOP 1.2+).
    LocSystemException = 4,
    /// `LOC_NEEDS_ADDRESSING_MODE` — the server needs a different
    /// `TargetAddress` discriminator (GIOP 1.2+).
    LocNeedsAddressingMode = 5,
}

impl LocateStatusType {
    /// Discriminant value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Parses from `unsigned long`.
    ///
    /// # Errors
    /// `UnknownLocateStatus` outside 0..=5; `Malformed` if the value
    /// does not match the version.
    pub fn from_u32(value: u32, version: Version) -> GiopResult<Self> {
        match value {
            0 => Ok(Self::UnknownObject),
            1 => Ok(Self::ObjectHere),
            2 => Ok(Self::ObjectForward),
            3 if version.uses_v1_2_request_layout() => Ok(Self::ObjectForwardPerm),
            4 if version.uses_v1_2_request_layout() => Ok(Self::LocSystemException),
            5 if version.uses_v1_2_request_layout() => Ok(Self::LocNeedsAddressingMode),
            3..=5 => Err(GiopError::Malformed(alloc::format!(
                "LocateStatus {value} only valid in GIOP 1.2+"
            ))),
            other => Err(GiopError::UnknownLocateStatus(other)),
        }
    }
}

/// LocateReply body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateReply {
    /// `request_id`.
    pub request_id: u32,
    /// Status.
    pub locate_status: LocateStatusType,
    /// Body (status-dependent). Spec §15.4.6.2:
    /// * `OBJECT_FORWARD[_PERM]` → IOR bytes (encapsulated).
    /// * `LOC_SYSTEM_EXCEPTION` → SystemExceptionReplyBody.
    /// * `LOC_NEEDS_ADDRESSING_MODE` → `AddressingDisposition` short.
    /// * otherwise → empty.
    pub body: Vec<u8>,
}

impl LocateReply {
    /// CDR encode.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        // Status version validation.
        match self.locate_status {
            LocateStatusType::ObjectForwardPerm
            | LocateStatusType::LocSystemException
            | LocateStatusType::LocNeedsAddressingMode
                if !version.uses_v1_2_request_layout() =>
            {
                return Err(GiopError::Malformed(alloc::format!(
                    "LocateStatus {:?} only valid in GIOP 1.2+",
                    self.locate_status
                )));
            }
            _ => {}
        }
        w.write_u32(self.request_id)?;
        w.write_u32(self.locate_status.as_u32())?;
        // Body (IOR for OBJECT_FORWARD*) only if present. For ObjectHere/
        // UnknownObject it is empty → do NOT emit 8-byte align padding,
        // otherwise trailing padding hangs after the last field and foreign ORBs
        // (omniORB giopImpl12.cc) report "Garbage left at end of input message".
        if !self.body.is_empty() {
            if version.uses_v1_2_request_layout() {
                w.align(8);
            }
            w.write_bytes(&self.body)?;
        }
        Ok(())
    }

    /// CDR decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(version: Version, r: &mut BufferReader<'_>) -> GiopResult<Self> {
        let request_id = r.read_u32()?;
        let status_raw = r.read_u32()?;
        let locate_status = LocateStatusType::from_u32(status_raw, version)?;
        // Body only if more bytes follow (see encode: ObjectHere/
        // UnknownObject have no body and no trailing padding).
        let body = if r.remaining() > 0 {
            if version.uses_v1_2_request_layout() {
                r.align(8)?;
            }
            r.read_bytes(r.remaining())?.to_vec()
        } else {
            Vec::new()
        };
        Ok(Self {
            request_id,
            locate_status,
            body,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn locate_status_values_match_spec() {
        assert_eq!(LocateStatusType::UnknownObject.as_u32(), 0);
        assert_eq!(LocateStatusType::ObjectHere.as_u32(), 1);
        assert_eq!(LocateStatusType::ObjectForward.as_u32(), 2);
        assert_eq!(LocateStatusType::ObjectForwardPerm.as_u32(), 3);
        assert_eq!(LocateStatusType::LocSystemException.as_u32(), 4);
        assert_eq!(LocateStatusType::LocNeedsAddressingMode.as_u32(), 5);
    }

    #[test]
    fn round_trip_giop_1_0_object_here() {
        let l = LocateReply {
            request_id: 1,
            locate_status: LocateStatusType::ObjectHere,
            body: alloc::vec::Vec::new(),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        l.encode(Version::V1_0, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = LocateReply::decode(Version::V1_0, &mut r).unwrap();
        assert_eq!(decoded, l);
    }

    #[test]
    fn round_trip_giop_1_2_object_forward_perm() {
        let l = LocateReply {
            request_id: 2,
            locate_status: LocateStatusType::ObjectForwardPerm,
            body: alloc::vec![0xff; 4],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        l.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = LocateReply::decode(Version::V1_2, &mut r).unwrap();
        assert_eq!(decoded, l);
    }

    #[test]
    fn forward_perm_in_1_0_is_rejected() {
        let l = LocateReply {
            request_id: 1,
            locate_status: LocateStatusType::ObjectForwardPerm,
            body: alloc::vec::Vec::new(),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        let err = l.encode(Version::V1_0, &mut w).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }
}
