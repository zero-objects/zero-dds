// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! LocateRequest-Message — Spec §15.4.5.
//!
//! GIOP 1.0/1.1:
//! ```text
//! struct LocateRequestHeader_1_0 {
//!     unsigned long   request_id;
//!     sequence<octet> object_key;
//! };
//! ```
//!
//! GIOP 1.2:
//! ```text
//! struct LocateRequestHeader_1_2 {
//!     unsigned long request_id;
//!     TargetAddress target;
//! };
//! ```
//!
//! The client probes whether a server accepts an object reference
//! (or requires LOCATION_FORWARD).

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};
use crate::target_address::TargetAddress;
use crate::version::Version;

/// LocateRequest-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateRequest {
    /// `request_id`.
    pub request_id: u32,
    /// `target` — always the `Key` variant in GIOP 1.0/1.1.
    pub target: TargetAddress,
}

impl LocateRequest {
    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer write error, or a Profile/Reference variant under
    /// GIOP 1.0/1.1.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        w.write_u32(self.request_id)?;
        if version.uses_v1_2_request_layout() {
            self.target.encode(w)?;
        } else {
            let key = match &self.target {
                TargetAddress::Key(k) => k.as_slice(),
                _ => {
                    return Err(GiopError::Malformed(
                        "LocateRequest in GIOP 1.0/1.1 only supports KeyAddr".into(),
                    ));
                }
            };
            let n = u32::try_from(key.len())
                .map_err(|_| GiopError::Malformed("object_key too long".into()))?;
            w.write_u32(n)?;
            w.write_bytes(key)?;
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(version: Version, r: &mut BufferReader<'_>) -> GiopResult<Self> {
        let request_id = r.read_u32()?;
        let target = if version.uses_v1_2_request_layout() {
            TargetAddress::decode(r)?
        } else {
            let n = r.read_u32()? as usize;
            let bytes = r.read_bytes(n)?;
            TargetAddress::Key(bytes.to_vec())
        };
        Ok(Self { request_id, target })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn round_trip_giop_1_0() {
        let l = LocateRequest {
            request_id: 11,
            target: TargetAddress::Key(alloc::vec![0x01, 0x02]),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        l.encode(Version::V1_0, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = LocateRequest::decode(Version::V1_0, &mut r).unwrap();
        assert_eq!(decoded, l);
    }

    #[test]
    fn round_trip_giop_1_2_with_profile_addr() {
        let l = LocateRequest {
            request_id: 22,
            target: TargetAddress::Profile(alloc::vec![0xa, 0xb]),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        l.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = LocateRequest::decode(Version::V1_2, &mut r).unwrap();
        assert_eq!(decoded, l);
    }

    #[test]
    fn giop_1_0_rejects_profile_target() {
        let l = LocateRequest {
            request_id: 1,
            target: TargetAddress::Profile(alloc::vec![]),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        let err = l.encode(Version::V1_0, &mut w).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }
}
