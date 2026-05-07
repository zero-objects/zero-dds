// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR-Struct — Spec §13.6.2.
//!
//! ```text
//! struct IOR {
//!     string                  type_id;
//!     sequence<TaggedProfile> profiles;
//! };
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_iiop::profile_body::CdrError;

use crate::tagged_profile::TaggedProfile;

/// Interoperable Object Reference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ior {
    /// Repository-ID des Object-Types (z.B. `IDL:omg.org/CosNaming/
    /// NamingContext:1.0`). Leerstring fuer "nil"-Object-Reference.
    pub type_id: String,
    /// Profile-Liste (mind. ein Eintrag fuer non-nil Refs).
    pub profiles: Vec<TaggedProfile>,
}

impl Ior {
    /// Konstruktor.
    #[must_use]
    pub const fn new(type_id: String, profiles: Vec<TaggedProfile>) -> Self {
        Self { type_id, profiles }
    }

    /// `true` wenn IOR die nil-Object-Reference repraesentiert (Spec
    /// §13.6.2: leerer `type_id` + leere Profile-Liste).
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.type_id.is_empty() && self.profiles.is_empty()
    }

    /// CDR-Encode (ohne Encapsulation-Wrapper).
    ///
    /// # Errors
    /// Buffer-Schreibfehler oder Length-Overflow.
    pub fn encode(&self, w: &mut BufferWriter) -> Result<(), CdrError> {
        w.write_string(&self.type_id)?;
        let n = u32::try_from(self.profiles.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n)?;
        for p in &self.profiles {
            p.encode(w)?;
        }
        Ok(())
    }

    /// CDR-Decode (ohne Encapsulation-Wrapper).
    ///
    /// # Errors
    /// Buffer-Lesefehler.
    pub fn decode(r: &mut BufferReader<'_>) -> Result<Self, CdrError> {
        let type_id = r.read_string()?;
        let n = r.read_u32()? as usize;
        let mut profiles = Vec::with_capacity(n.min(8));
        for _ in 0..n {
            profiles.push(TaggedProfile::decode(r)?);
        }
        Ok(Self { type_id, profiles })
    }

    /// Encodiert eine CDR-Encapsulation `endianness-byte + body`.
    ///
    /// Diese Form wird von `to_stringified` und vom IOR-Container in
    /// vielen Vendor-Codes (TAO, omniORB) genutzt.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        let mut out = Vec::with_capacity(64);
        out.push(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        let mut w = BufferWriter::new(endianness);
        self.encode(&mut w)?;
        out.extend_from_slice(w.as_bytes());
        Ok(out)
    }

    /// Decodiert aus einer CDR-Encapsulation.
    ///
    /// # Errors
    /// Buffer-Lesefehler oder Endianness-Octet ungueltig.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        if bytes.is_empty() {
            return Err(CdrError::Truncated);
        }
        let endianness = match bytes[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            _ => return Err(CdrError::InvalidEndianness),
        };
        let mut r = BufferReader::new(&bytes[1..], endianness);
        Self::decode(&mut r)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::profile_tags::ProfileId;
    use zerodds_corba_iiop::{IiopProfileBody, IiopVersion};

    fn sample_iiop_profile() -> TaggedProfile {
        TaggedProfile::iiop(
            &IiopProfileBody::new(
                IiopVersion::V1_2,
                "host.example".into(),
                7777,
                alloc::vec![0xab, 0xcd],
            ),
            Endianness::Big,
        )
        .unwrap()
    }

    #[test]
    fn nil_ior_is_recognized() {
        let nil = Ior::default();
        assert!(nil.is_nil());
    }

    #[test]
    fn round_trip_iiop_only_ior_be() {
        let ior = Ior::new(
            "IDL:omg.org/CosNaming/NamingContext:1.0".into(),
            alloc::vec![sample_iiop_profile()],
        );
        let bytes = ior.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = Ior::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, ior);
        assert_eq!(bytes[0], 0); // BE marker.
    }

    #[test]
    fn round_trip_multi_profile_le() {
        let other_profile = TaggedProfile {
            tag: ProfileId::MultipleComponents,
            profile_data: alloc::vec![1, 0, 0, 0],
        };
        let ior = Ior::new(
            "IDL:demo/Echo:1.0".into(),
            alloc::vec![sample_iiop_profile(), other_profile],
        );
        let bytes = ior.encode_encapsulation(Endianness::Little).unwrap();
        let decoded = Ior::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, ior);
        assert_eq!(decoded.profiles.len(), 2);
        assert_eq!(bytes[0], 1); // LE marker.
    }

    #[test]
    fn empty_encapsulation_is_truncated() {
        let err = Ior::decode_encapsulation(&[]).unwrap_err();
        assert!(matches!(err, CdrError::Truncated));
    }

    #[test]
    fn invalid_endianness_octet_is_diagnostic() {
        let err = Ior::decode_encapsulation(&[0xff, 0, 0]).unwrap_err();
        assert!(matches!(err, CdrError::InvalidEndianness));
    }
}
