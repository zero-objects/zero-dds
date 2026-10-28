// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An object reference and the base every valuetype has.
//!
//! Two of the thirteen Annex A.1 rows —
//! [`SpecialType::Object`][object] and
//! [`SpecialType::ValueBase`][valuebase] — and the Rust column of that
//! table already named this crate for both. It named types that were
//! not here: `zerodds-corba-codegen` compiles, because a name inside a
//! `&'static str` resolves to nothing, and generated code carrying it
//! would not.
//!
//! They are here rather than in the emitter for the same reason
//! [`crate::type_code`] and [`crate::any`] are: the emitter is a
//! build-time tool, and a program that never generates a line of code
//! still holds object references. `zerodds-wire` is the wrong place
//! too — an IOR is a CORBA structure, and `wire` decides nothing about
//! a type.
//!
//! [object]: https://www.omg.org/spec/CORBA/3.3/
//! [valuebase]: https://www.omg.org/spec/CORBA/3.3/

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

/// `TAG_INTERNET_IOP`. CORBA 3.3 part 2 §13.6.3.
const TAG_INTERNET_IOP: u32 = 0;

/// A CORBA object reference, in the form it takes on the wire.
///
/// CORBA 3.3 part 2 §13.6.2: an IOR is a `type_id` and a sequence of
/// tagged profiles. This carries the `TAG_INTERNET_IOP` profile
/// already encapsulated, because that is the one a ZeroDDS reference
/// has and the encapsulation is what goes into the `sequence<octet>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectReference {
    /// The repository identifier of the most-derived interface, e.g.
    /// `IDL:omg.org/CosNaming/NamingContext:1.0`.
    pub type_id: String,
    /// The `TAG_INTERNET_IOP` profile body, already encapsulated.
    pub iiop_profile: Vec<u8>,
}

impl ObjectReference {
    /// Writes the reference as an IOR. CORBA 3.3 part 2 §15.3.3.
    ///
    /// `struct IOR { string type_id; sequence<TaggedProfile> profiles; }`
    /// with `TaggedProfile { ProfileId tag; sequence<octet> profile_data; }`.
    /// This is the form omniORB, TAO and JacORB expect an `Object` in.
    pub fn encode(&self, w: &mut Writer) {
        w.write_string(&self.type_id);
        w.write_u32(1);
        w.write_u32(TAG_INTERNET_IOP);
        #[allow(clippy::cast_possible_truncation)]
        w.write_u32(self.iiop_profile.len() as u32);
        w.write_bytes(&self.iiop_profile);
    }

    /// Reads a reference from an IOR.
    ///
    /// Every profile in the sequence is consumed, so the reader ends
    /// where the IOR ends whatever else the peer put in it. The first
    /// `TAG_INTERNET_IOP` profile is kept; a reference with none keeps
    /// an empty profile rather than failing, because a multi-component
    /// IOR without an IIOP profile is well formed and simply not
    /// reachable over IIOP.
    ///
    /// # Errors
    /// [`WireError`] for a truncated or malformed encapsulation.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let type_id = r.read_string()?;
        let profile_count = r.read_u32()? as usize;
        // A profile is at least two length words, so a count larger
        // than a quarter of what is left cannot be true. Rejected
        // before it reaches an allocation.
        if profile_count > r.remaining() / 8 {
            return Err(WireError::ImpossibleLength {
                at: r.position(),
                claimed: profile_count,
                available: r.remaining(),
            });
        }
        let mut iiop_profile = Vec::new();
        for _ in 0..profile_count {
            let tag = r.read_u32()?;
            let length = r.read_u32()? as usize;
            let data = r.read_bytes(length)?;
            if tag == TAG_INTERNET_IOP && iiop_profile.is_empty() {
                iiop_profile.extend_from_slice(data);
            }
        }
        Ok(Self {
            type_id,
            iiop_profile,
        })
    }
}

/// The base every valuetype has. OMG IDL 4.2 §7.4.7, rule (131).
///
/// One operation, because one is what the base declares: which type
/// the value is. Everything else a valuetype has, it has from its own
/// declaration.
pub trait ValueBase {
    /// The repository identifier of this value's type.
    fn repository_id(&self) -> &str;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn an_object_reference_round_trips_as_an_ior() {
        let reference = ObjectReference {
            type_id: String::from("IDL:demo/Bench:1.0"),
            iiop_profile: alloc::vec![0x01, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef],
        };
        for endianness in [Endianness::Big, Endianness::Little] {
            let mut w = Writer::new(endianness);
            reference.encode(&mut w);
            let bytes = w.finish();

            // The structure §15.3.3 names, read field by field.
            let mut r = Reader::new(&bytes, endianness);
            assert_eq!(r.read_string().unwrap(), "IDL:demo/Bench:1.0");
            assert_eq!(r.read_u32().unwrap(), 1, "exactly one profile");
            assert_eq!(r.read_u32().unwrap(), TAG_INTERNET_IOP);
            assert_eq!(r.read_u32().unwrap(), 8, "profile_data length");

            let mut r = Reader::new(&bytes, endianness);
            assert_eq!(ObjectReference::decode(&mut r).unwrap(), reference);
        }
    }

    #[test]
    fn a_profile_count_larger_than_the_stream_is_refused() {
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:X:1.0");
        w.write_u32(1_000_000_000);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(matches!(
            ObjectReference::decode(&mut r),
            Err(WireError::ImpossibleLength { .. })
        ));
    }

    #[test]
    fn a_non_iiop_profile_is_consumed_and_not_kept() {
        // TAG_MULTIPLE_COMPONENTS (1) ahead of the IIOP profile: the
        // reader must skip it and still find the one it wants, and end
        // at the end of the IOR.
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:X:1.0");
        w.write_u32(2);
        w.write_u32(1);
        w.write_u32(2);
        w.write_bytes(&[0xaa, 0xbb]);
        w.write_u32(TAG_INTERNET_IOP);
        w.write_u32(3);
        w.write_bytes(&[1, 2, 3]);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let reference = ObjectReference::decode(&mut r).unwrap();
        assert_eq!(reference.iiop_profile, alloc::vec![1, 2, 3]);
        assert_eq!(r.remaining(), 0, "the whole IOR is consumed");
    }
}
