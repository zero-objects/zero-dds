// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IOR struct — spec §13.6.2.
//!
//! ```text
//! struct IOR {
//!     string                  type_id;
//!     sequence<TaggedProfile> profiles;
//! };
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_iiop::profile_body::CdrError;
use zerodds_wire::{Endianness, Reader, Writer};

use crate::tagged_profile::TaggedProfile;

/// Interoperable Object Reference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ior {
    /// Repository ID of the object type (e.g. `IDL:omg.org/CosNaming/
    /// NamingContext:1.0`). Empty string for the "nil" object reference.
    pub type_id: String,
    /// Profile list (at least one entry for non-nil refs).
    pub profiles: Vec<TaggedProfile>,
}

impl Ior {
    /// Constructor.
    #[must_use]
    pub const fn new(type_id: String, profiles: Vec<TaggedProfile>) -> Self {
        Self { type_id, profiles }
    }

    /// `true` if the IOR represents the nil object reference (spec
    /// §13.6.2: empty `type_id` + empty profile list).
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.type_id.is_empty() && self.profiles.is_empty()
    }

    /// CDR encode (without encapsulation wrapper).
    ///
    /// # Errors
    /// Buffer write error or length overflow.
    pub fn encode(&self, w: &mut Writer) -> Result<(), CdrError> {
        w.write_string(&self.type_id);
        let n = u32::try_from(self.profiles.len()).map_err(|_| CdrError::Overflow)?;
        w.write_u32(n);
        for p in &self.profiles {
            p.encode(w)?;
        }
        Ok(())
    }

    /// CDR decode (without encapsulation wrapper).
    ///
    /// # Errors
    /// Buffer read error.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, CdrError> {
        let type_id = r.read_string()?;
        let n = r.read_u32()? as usize;
        let mut profiles = Vec::with_capacity(n.min(8));
        for _ in 0..n {
            profiles.push(TaggedProfile::decode(r)?);
        }
        Ok(Self { type_id, profiles })
    }

    /// Encodes a CDR encapsulation `endianness-byte + body`.
    ///
    /// This form is used by `to_stringified` and by the IOR container in
    /// many vendor code bases (TAO, omniORB).
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, CdrError> {
        // The byte-order octet goes into the **same** writer, so every
        // alignment inside the body is counted from the start of the
        // encapsulation because that is where it starts. This used to
        // write the octet into a `Vec`, build a second writer, and tell
        // that writer its byte 0 was byte 1 — the same shape as the
        // buffer `corba-giop` had that was told its byte 0 was byte 12.
        // §15.3.3 says the encapsulation *is* the octet plus the body;
        // a coordinate a buffer is given is a coordinate that can be
        // given wrongly, and one it has cannot be.
        let mut w = Writer::new(endianness);
        // Byte-order octet: bit 0 is 0 for big-endian, 1 for little.
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        self.encode(&mut w)?;
        Ok(w.finish())
    }

    /// Decodes from a CDR encapsulation.
    ///
    /// # Errors
    /// Buffer read error or invalid endianness octet.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, CdrError> {
        if bytes.is_empty() {
            return Err(CdrError::Truncated);
        }
        let endianness = match bytes[0] {
            0 => Endianness::Big,
            1 => Endianness::Little,
            _ => return Err(CdrError::InvalidEndianness),
        };
        // The reader is over the whole encapsulation, and the
        // byte-order octet is read out of it rather than sliced off.
        // Alignment then counts from the encapsulation's own byte 0,
        // which is what §15.3.3 means, with no origin handed over.
        let mut r = Reader::new(bytes, endianness);
        r.read_u8()?;
        Self::decode(&mut r)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::component_tags::ComponentId;
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
    fn a_security_ior_carries_mech_list_tls_and_null_tag_through_a_round_trip() {
        // Spec Part 2 §10.5: a target that offers CSIv2 publishes it in
        // the IOR — `TAG_CSI_SEC_MECH_LIST` (33) in the IIOP profile's
        // component list, the transport mechanism it names
        // (`TAG_TLS_SEC_TRANS`, 36) beside it, and `TAG_NULL_TAG` (34)
        // where a compound mechanism has no transport of its own. All
        // three must survive the IOR the client receives, not merely the
        // component encapsulation each was built from.
        use crate::components::{
            AlternateIiopAddress, StructuredComponent, TaggedComponent as IorComponent, TlsSecTrans,
        };
        use zerodds_corba_csiv2::{
            AsContextSec, AssociationOptions, CompoundSecMech, CompoundSecMechList, SasContextSec,
        };
        use zerodds_corba_iiop::profile_body::TaggedComponent as IiopComponent;

        let mech_list = CompoundSecMechList {
            stateful: true,
            mechanism_list: alloc::vec![CompoundSecMech {
                target_requires: AssociationOptions(
                    AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY,
                ),
                transport_mech_tag: ComponentId::TlsSecTrans.as_u32(),
                transport_mech_data: alloc::vec![0x01, 0x02],
                as_context: AsContextSec {
                    target_supports: AssociationOptions(0x0040),
                    target_requires: AssociationOptions(0x0040),
                    client_authentication_mech: alloc::vec![0xAA],
                    target_name: alloc::vec![0xBB],
                },
                sas_context: SasContextSec {
                    target_supports: AssociationOptions(0x0080),
                    target_requires: AssociationOptions(0),
                    privilege_authorities: alloc::vec![],
                    supported_naming_mechanisms: alloc::vec![alloc::vec![0xCC]],
                    supported_identity_types: 2,
                },
            }],
        };
        let tls = TlsSecTrans {
            target_supports: AssociationOptions::INTEGRITY | AssociationOptions::CONFIDENTIALITY,
            target_requires: AssociationOptions::CONFIDENTIALITY,
            addresses: alloc::vec![AlternateIiopAddress {
                host: "tls.example".into(),
                port: 683,
            }],
        };

        let encap = |c: &StructuredComponent| c.encode_encapsulation(Endianness::Big).unwrap();
        let mech_bytes = encap(&StructuredComponent::CsiSecMechList(mech_list.clone()));
        let tls_bytes = encap(&StructuredComponent::TlsSecTrans(tls.clone()));
        // TAG_NULL_TAG carries an empty encapsulation body.
        let null_bytes = alloc::vec![0u8];

        let mut body = IiopProfileBody::new(
            IiopVersion::V1_2,
            "host.example".into(),
            7777,
            alloc::vec![0xab, 0xcd],
        );
        body.components = alloc::vec![
            IiopComponent {
                tag: ComponentId::CsiSecMechList.as_u32(),
                component_data: mech_bytes,
            },
            IiopComponent {
                tag: ComponentId::TlsSecTrans.as_u32(),
                component_data: tls_bytes,
            },
            IiopComponent {
                tag: ComponentId::NullTag.as_u32(),
                component_data: null_bytes,
            },
        ];

        let ior = Ior::new(
            "IDL:demo/SecureEcho:1.0".into(),
            alloc::vec![TaggedProfile::iiop(&body, Endianness::Big).unwrap()],
        );
        let bytes = ior.encode_encapsulation(Endianness::Big).unwrap();
        let decoded = Ior::decode_encapsulation(&bytes).unwrap();
        assert_eq!(decoded, ior);

        let profile = decoded.profiles[0].as_iiop().unwrap().unwrap();
        let tags: Vec<u32> = profile.components.iter().map(|c| c.tag).collect();
        assert_eq!(tags, alloc::vec![33, 36, 34]);

        // The three bodies are still the ones that went in, and the two
        // structured ones decode back to the values they were built from.
        let structured = |tag: ComponentId| {
            let raw = profile
                .components
                .iter()
                .find(|c| c.tag == tag.as_u32())
                .expect("the tag survived the IOR");
            IorComponent {
                tag,
                component_data: raw.component_data.clone(),
            }
            .structured()
            .expect("the component body is decodable")
        };
        assert_eq!(
            structured(ComponentId::CsiSecMechList),
            StructuredComponent::CsiSecMechList(mech_list)
        );
        assert_eq!(
            structured(ComponentId::TlsSecTrans),
            StructuredComponent::TlsSecTrans(tls)
        );
        // TAG_NULL_TAG has no structured form; it stays the empty
        // encapsulation the target wrote.
        assert_eq!(
            structured(ComponentId::NullTag),
            StructuredComponent::Opaque {
                tag: ComponentId::NullTag,
                bytes: alloc::vec![0u8],
            }
        );
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
