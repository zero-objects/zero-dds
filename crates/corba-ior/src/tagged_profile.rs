// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TaggedProfile — Spec §13.6.2.
//!
//! ```text
//! struct TaggedProfile {
//!     ProfileId        tag;
//!     sequence<octet>  profile_data;
//! };
//! ```
//!
//! `profile_data` ist eine CDR-Encapsulation des Profile-spezifischen
//! Bodies. Fuer `TAG_INTERNET_IOP` ist das ein `IIOP::ProfileBody`
//! (Spec §15.7.2), siehe `crates/corba-iiop/src/profile_body.rs`.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_iiop::IiopProfileBody;
use zerodds_corba_iiop::profile_body::CdrError;

use crate::profile_tags::ProfileId;

/// `TaggedProfile` — Tag + opaque Encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedProfile {
    /// Profile-Tag.
    pub tag: ProfileId,
    /// Encapsulation-Bytes (Endianness-Octet + Body).
    pub profile_data: Vec<u8>,
}

impl TaggedProfile {
    /// Konstruiert ein `TAG_INTERNET_IOP`-Profile aus einem
    /// [`IiopProfileBody`] mit gewaehlter Endianness.
    ///
    /// # Errors
    /// CDR-Encode-Fehler.
    pub fn iiop(body: &IiopProfileBody, endianness: Endianness) -> Result<Self, CdrError> {
        let profile_data = body.encode_encapsulation(endianness)?;
        Ok(Self {
            tag: ProfileId::InternetIop,
            profile_data,
        })
    }

    /// Versucht, das Profile als `IIOP::ProfileBody` zu decodieren.
    /// Liefert `None`, wenn der Tag nicht `TAG_INTERNET_IOP` ist.
    ///
    /// # Errors
    /// CDR-Decode-Fehler bei korruptem Body.
    pub fn as_iiop(&self) -> Option<Result<IiopProfileBody, CdrError>> {
        if self.tag != ProfileId::InternetIop {
            return None;
        }
        Some(IiopProfileBody::decode_encapsulation(&self.profile_data))
    }

    /// CDR-Encode (length-prefixed Encapsulation).
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        w.write_u32(self.tag.as_u32())?;
        let n = u32::try_from(self.profile_data.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        w.write_bytes(&self.profile_data)?;
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, CdrError> {
        let tag = ProfileId::from_u32(r.read_u32()?);
        let n = r.read_u32()? as usize;
        let bytes = r.read_bytes(n)?;
        Ok(Self {
            tag,
            profile_data: bytes.to_vec(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_corba_iiop::IiopVersion;

    #[test]
    fn tagged_profile_round_trip() {
        let body = IiopProfileBody::new(
            IiopVersion::V1_2,
            "host.lab".into(),
            7777,
            alloc::vec![0xab],
        );
        let p = TaggedProfile::iiop(&body, Endianness::Big).unwrap();
        let mut w = BufferWriter::new(Endianness::Big);
        p.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = TaggedProfile::decode(&mut r).unwrap();
        assert_eq!(decoded, p);
    }

    #[test]
    fn iiop_profile_round_trip_via_helper() {
        let body = IiopProfileBody::new(
            IiopVersion::V1_2,
            "10.0.0.1".into(),
            8888,
            alloc::vec![0xde, 0xad],
        );
        let p = TaggedProfile::iiop(&body, Endianness::Little).unwrap();
        let recovered = p.as_iiop().unwrap().unwrap();
        assert_eq!(recovered, body);
    }

    #[test]
    fn non_iiop_profile_returns_none_from_helper() {
        let p = TaggedProfile {
            tag: ProfileId::SccpIop,
            profile_data: alloc::vec![0x00, 0xff],
        };
        assert!(p.as_iiop().is_none());
    }

    #[test]
    fn unknown_profile_tag_round_trips_as_other() {
        let p = TaggedProfile {
            tag: ProfileId::Other(99),
            profile_data: alloc::vec::Vec::new(),
        };
        let mut w = BufferWriter::new(Endianness::Big);
        p.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = TaggedProfile::decode(&mut r).unwrap();
        assert_eq!(decoded, p);
    }
}
