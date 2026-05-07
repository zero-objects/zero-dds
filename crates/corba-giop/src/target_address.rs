// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TargetAddress (GIOP 1.2+) — Spec §15.4.2.2 / §15.4.5.
//!
//! ```text
//! const short KeyAddr       = 0;
//! const short ProfileAddr   = 1;
//! const short ReferenceAddr = 2;
//!
//! struct IORAddressingInfo {
//!     unsigned long  selected_profile_index;
//!     IOR            ior;
//! };
//!
//! union TargetAddress switch (short) {
//!     case KeyAddr:       sequence<octet> object_key;
//!     case ProfileAddr:   IOP::TaggedProfile profile;
//!     case ReferenceAddr: IORAddressingInfo  ior;
//! };
//! ```
//!
//! Wir modellieren die Union-Variante. Der `IOP::TaggedProfile`-Type
//! und `IOR`-Type leben in `crates/corba-ior/`; um Crate-Zyklen zu
//! vermeiden, halten wir die Daten als opaque Bytes fuer Profile/IOR
//! und liefern Helper, mit denen `crates/corba-ior/` die Bytes
//! re-parsen kann.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};

/// Object-Key — `sequence<octet>`. Spec §13.6.5: vom POA generierte
/// opaque Identifier-Bytes (Lifespan + ID-Bytes etc.).
pub type ObjectKey = Vec<u8>;

/// `TargetAddress`-Discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum AddressingDisposition {
    /// `KeyAddr` (0) — direkter Object-Key.
    KeyAddr = 0,
    /// `ProfileAddr` (1) — `IOP::TaggedProfile`-Verweis.
    ProfileAddr = 1,
    /// `ReferenceAddr` (2) — voller `IORAddressingInfo` mit
    /// `selected_profile_index` + IOR.
    ReferenceAddr = 2,
}

/// `TargetAddress`-Union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAddress {
    /// `KeyAddr` — opaque Object-Key.
    Key(ObjectKey),
    /// `ProfileAddr` — opaque `TaggedProfile`-Bytes (Caller decodiert
    /// in `crates/corba-ior/`).
    Profile(Vec<u8>),
    /// `ReferenceAddr` — `selected_profile_index` + opaque `IOR`-Bytes.
    Reference {
        /// Index in den IOR-Profile-List.
        selected_profile_index: u32,
        /// Opaque IOR-Bytes (Caller decodiert).
        ior: Vec<u8>,
    },
}

impl TargetAddress {
    /// Discriminant.
    #[must_use]
    pub const fn disposition(&self) -> AddressingDisposition {
        match self {
            Self::Key(_) => AddressingDisposition::KeyAddr,
            Self::Profile(_) => AddressingDisposition::ProfileAddr,
            Self::Reference { .. } => AddressingDisposition::ReferenceAddr,
        }
    }

    /// CDR-Encode.
    ///
    /// # Errors
    /// Buffer-Schreibfehler.
    pub fn encode(&self, w: &mut BufferWriter) -> GiopResult<()> {
        // Union-Diskriminator ist `short` (= int16, alignment 2).
        let disc = self.disposition() as i16;
        w.align(2);
        // `short` per CDR — das Buffer-API hat write_u16, also wir
        // konvertieren signed→unsigned bit-identisch.
        w.write_u16(disc as u16)?;
        match self {
            Self::Key(k) => {
                let n = u32::try_from(k.len())
                    .map_err(|_| GiopError::Malformed("object_key too long".into()))?;
                w.write_u32(n)?;
                w.write_bytes(k)?;
            }
            Self::Profile(bytes) => {
                let n = u32::try_from(bytes.len())
                    .map_err(|_| GiopError::Malformed("profile too long".into()))?;
                w.write_u32(n)?;
                w.write_bytes(bytes)?;
            }
            Self::Reference {
                selected_profile_index,
                ior,
            } => {
                w.write_u32(*selected_profile_index)?;
                let n = u32::try_from(ior.len())
                    .map_err(|_| GiopError::Malformed("ior too long".into()))?;
                w.write_u32(n)?;
                w.write_bytes(ior)?;
            }
        }
        Ok(())
    }

    /// CDR-Decode.
    ///
    /// # Errors
    /// Buffer-Lesefehler oder unbekannte Disposition.
    pub fn decode(r: &mut BufferReader<'_>) -> GiopResult<Self> {
        r.align(2)?;
        let disc = r.read_u16()?;
        match disc {
            0 => {
                let n = r.read_u32()? as usize;
                let bytes = r.read_bytes(n)?;
                Ok(Self::Key(bytes.to_vec()))
            }
            1 => {
                let n = r.read_u32()? as usize;
                let bytes = r.read_bytes(n)?;
                Ok(Self::Profile(bytes.to_vec()))
            }
            2 => {
                let selected_profile_index = r.read_u32()?;
                let n = r.read_u32()? as usize;
                let bytes = r.read_bytes(n)?;
                Ok(Self::Reference {
                    selected_profile_index,
                    ior: bytes.to_vec(),
                })
            }
            other => Err(GiopError::UnknownAddressingDisposition(other)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn disposition_values_match_spec() {
        // Spec §15.4.2.2: KeyAddr=0, ProfileAddr=1, ReferenceAddr=2.
        assert_eq!(AddressingDisposition::KeyAddr as i16, 0);
        assert_eq!(AddressingDisposition::ProfileAddr as i16, 1);
        assert_eq!(AddressingDisposition::ReferenceAddr as i16, 2);
    }

    #[test]
    fn key_addr_round_trip() {
        let t = TargetAddress::Key(alloc::vec![0xde, 0xad, 0xbe, 0xef]);
        let mut w = BufferWriter::new(Endianness::Big);
        t.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = TargetAddress::decode(&mut r).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn profile_addr_round_trip() {
        let t = TargetAddress::Profile(alloc::vec![1, 2, 3]);
        let mut w = BufferWriter::new(Endianness::Little);
        t.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = TargetAddress::decode(&mut r).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn reference_addr_round_trip() {
        let t = TargetAddress::Reference {
            selected_profile_index: 7,
            ior: alloc::vec![0xff, 0x00, 0xaa],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        t.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = TargetAddress::decode(&mut r).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn unknown_disposition_is_diagnostic() {
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u16(99).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let err = TargetAddress::decode(&mut r).unwrap_err();
        assert!(matches!(err, GiopError::UnknownAddressingDisposition(99)));
    }
}
