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
/// CORBA 3.3 part 2 §7.6.2: an IOR is a `type_id` and a sequence of
/// tagged profiles. `iiop_profile` is the first `TAG_INTERNET_IOP`
/// profile, already encapsulated, because that is the one a ZeroDDS
/// reference has and the encapsulation is what goes into the
/// `sequence<octet>`. A reference read from an IOR with other or further
/// profiles keeps all of them, in their order, in `profiles` (part 2
/// §7.6.3, Full IOR conformance): [`Self::encode`] writes them back
/// unchanged, and a client tries the further IIOP profiles when the
/// first cannot be used.
///
/// `policy_overrides` are the client-side policy overrides of this
/// reference (CORBA 3.3 part 1 §8.3.9 `set_policy_overrides`): they
/// belong to the client's copy of the reference, are not part of the
/// IOR, and neither [`Self::encode`] writes nor [`Self::decode`] reads
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectReference {
    /// The repository identifier of the most-derived interface, e.g.
    /// `IDL:omg.org/CosNaming/NamingContext:1.0`.
    pub type_id: String,
    /// The first `TAG_INTERNET_IOP` profile body, already encapsulated;
    /// empty when the reference has none.
    pub iiop_profile: Vec<u8>,
    /// The policy overrides at the object scope, at most one per policy
    /// type (part 1 §8.3.9, §8.9.1).
    pub policy_overrides: Vec<PolicyOverride>,
    /// Every profile of the IOR in its order when the IOR holds more or
    /// other profiles than the one `TAG_INTERNET_IOP` profile in
    /// `iiop_profile`, or that one profile with empty `profile_data`;
    /// empty otherwise. When it is not empty, its first
    /// `TAG_INTERNET_IOP` profile is `iiop_profile`.
    pub profiles: Vec<RawProfile>,
}

/// One tagged profile of an IOR as it was received: the `ProfileId` and
/// the `profile_data` octets (CORBA 3.3 part 2 §7.6.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawProfile {
    /// `IOP::ProfileId`.
    pub tag: u32,
    /// `IOP::ProfileData`.
    pub data: Vec<u8>,
}

/// One client-side policy override of an [`ObjectReference`] (CORBA 3.3
/// part 1 §8.3.9): the policy type and the value of the policy as the
/// CDR encapsulation of an `any` (the byte-order octet, then the
/// `TypeCode` and the value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOverride {
    /// `CORBA::PolicyType`.
    pub policy_type: u32,
    /// The `any` of the policy's value, encapsulated.
    pub value: Vec<u8>,
}

impl ObjectReference {
    /// A reference with the profiles of an IOR in their order (CORBA 3.3
    /// part 2 §7.6.3): `iiop_profile` is the first `TAG_INTERNET_IOP`
    /// profile, `profiles` keeps the list unless it is exactly that one
    /// profile with data. A single `TAG_INTERNET_IOP` profile with empty
    /// `profile_data` stays in `profiles`, because an empty
    /// `iiop_profile` alone means no profile.
    #[must_use]
    pub fn from_profiles(type_id: String, profiles: Vec<RawProfile>) -> Self {
        let iiop_profile = profiles
            .iter()
            .find(|p| p.tag == TAG_INTERNET_IOP)
            .map(|p| p.data.clone())
            .unwrap_or_default();
        let only_iiop = profiles.len() == 1
            && profiles[0].tag == TAG_INTERNET_IOP
            && !profiles[0].data.is_empty();
        Self {
            type_id,
            iiop_profile,
            policy_overrides: Vec::new(),
            profiles: if only_iiop { Vec::new() } else { profiles },
        }
    }

    /// The profiles of the reference in their order: `profiles`, or the
    /// one `TAG_INTERNET_IOP` profile `iiop_profile` when it is empty;
    /// none for a reference without profiles (the nil reference, part 2
    /// §7.6.3).
    #[must_use]
    pub fn tagged_profiles(&self) -> Vec<RawProfile> {
        if self.profiles.is_empty() {
            if self.iiop_profile.is_empty() {
                return Vec::new();
            }
            alloc::vec![RawProfile {
                tag: TAG_INTERNET_IOP,
                data: self.iiop_profile.clone(),
            }]
        } else {
            self.profiles.clone()
        }
    }

    /// The encapsulated `TAG_INTERNET_IOP` profile bodies in the order of
    /// the IOR; none for a reference without one.
    #[must_use]
    pub fn iiop_profiles(&self) -> Vec<&[u8]> {
        if self.profiles.is_empty() {
            if self.iiop_profile.is_empty() {
                Vec::new()
            } else {
                alloc::vec![self.iiop_profile.as_slice()]
            }
        } else {
            self.profiles
                .iter()
                .filter(|p| p.tag == TAG_INTERNET_IOP)
                .map(|p| p.data.as_slice())
                .collect()
        }
    }

    /// Writes the reference as an IOR. CORBA 3.3 part 2 §15.3.3.
    ///
    /// `struct IOR { string type_id; sequence<TaggedProfile> profiles; }`
    /// with `TaggedProfile { ProfileId tag; sequence<octet> profile_data; }`.
    /// This is the form omniORB, TAO and JacORB expect an `Object` in.
    /// Every profile a received IOR held is written back in its order.
    pub fn encode(&self, w: &mut Writer) {
        w.write_string(&self.type_id);
        let profiles = self.tagged_profiles();
        #[allow(clippy::cast_possible_truncation)]
        w.write_u32(profiles.len() as u32);
        for p in &profiles {
            w.write_u32(p.tag);
            #[allow(clippy::cast_possible_truncation)]
            w.write_u32(p.data.len() as u32);
            w.write_bytes(&p.data);
        }
    }

    /// Reads a reference from an IOR; an IOR without profiles is a
    /// reference without profiles.
    ///
    /// Every profile in the sequence is read and kept in its order
    /// ([`Self::from_profiles`]), so the reader ends where the IOR ends
    /// and the reference writes the same IOR back. A reference without a
    /// `TAG_INTERNET_IOP` profile has an empty `iiop_profile` rather than
    /// failing, because a multi-component IOR without an IIOP profile is
    /// well formed and simply not reachable over IIOP.
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
        let mut profiles = Vec::with_capacity(profile_count);
        for _ in 0..profile_count {
            let tag = r.read_u32()?;
            let length = r.read_u32()? as usize;
            let data = r.read_bytes(length)?;
            profiles.push(RawProfile {
                tag,
                data: data.to_vec(),
            });
        }
        if profiles.is_empty() {
            return Ok(Self {
                type_id,
                ..Self::default()
            });
        }
        Ok(Self::from_profiles(type_id, profiles))
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
            policy_overrides: Vec::new(),
            profiles: Vec::new(),
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

    /// Part 2 §7.6.3 Full IOR conformance: a TAG_MULTIPLE_COMPONENTS
    /// profile ahead of two IIOP profiles is kept, the first IIOP profile
    /// is the one the reference uses, and the IOR written back is the
    /// IOR read, profile for profile in the same order.
    #[test]
    fn corba_p2_7_6_3_every_profile_is_kept_in_its_order() {
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:X:1.0");
        w.write_u32(3);
        w.write_u32(1);
        w.write_u32(2);
        w.write_bytes(&[0xaa, 0xbb]);
        w.write_u32(TAG_INTERNET_IOP);
        w.write_u32(3);
        w.write_bytes(&[1, 2, 3]);
        w.write_u32(TAG_INTERNET_IOP);
        w.write_u32(1);
        w.write_bytes(&[4]);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let reference = ObjectReference::decode(&mut r).unwrap();
        assert_eq!(r.remaining(), 0, "the whole IOR is consumed");
        assert_eq!(reference.iiop_profile, alloc::vec![1, 2, 3]);
        assert_eq!(
            reference.iiop_profiles(),
            alloc::vec![&[1u8, 2, 3][..], &[4u8][..]]
        );
        assert_eq!(reference.profiles.len(), 3);
        let mut w = Writer::new(Endianness::Big);
        reference.encode(&mut w);
        assert_eq!(w.finish(), bytes, "written back unchanged");

        // A single IIOP profile with empty `profile_data` is a profile:
        // the IOR comes back with it, not without profiles.
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:X:1.0");
        w.write_u32(1);
        w.write_u32(TAG_INTERNET_IOP);
        w.write_u32(0);
        let bytes = w.finish();
        let reference = ObjectReference::decode(&mut Reader::new(&bytes, Endianness::Big)).unwrap();
        let empty_iiop = RawProfile {
            tag: TAG_INTERNET_IOP,
            data: Vec::new(),
        };
        assert_eq!(reference.tagged_profiles(), alloc::vec![empty_iiop.clone()]);
        assert_eq!(
            ObjectReference::from_profiles("IDL:X:1.0".into(), alloc::vec![empty_iiop]),
            reference
        );
        let mut w = Writer::new(Endianness::Big);
        reference.encode(&mut w);
        assert_eq!(w.finish(), bytes, "written back with its profile");
    }

    /// A reference without an IIOP profile has none to use and still
    /// writes its IOR back.
    #[test]
    fn corba_p2_7_6_3_a_reference_without_an_iiop_profile_keeps_its_profile() {
        let reference = ObjectReference::from_profiles(
            "IDL:X:1.0".into(),
            alloc::vec![RawProfile {
                tag: 3,
                data: alloc::vec![7]
            }],
        );
        assert!(reference.iiop_profiles().is_empty());
        let mut w = Writer::new(Endianness::Big);
        reference.encode(&mut w);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert_eq!(ObjectReference::decode(&mut r).unwrap(), reference);
    }
}
