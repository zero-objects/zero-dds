// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeLookup Service IDL (XTypes 1.3 §7.6.3.3).
//!
//! Das ist ein DDS-RPC-Service mit zwei Operationen:
//! - `getTypes(TypeIdentifier[])` → `TypeObject[]` (Minimal/Complete)
//! - `getTypeDependencies(TypeIdentifier[], continuation_point)`
//!   → `TypeIdentifierWithSize[] + continuation_point`
//!
//! Wir definieren die IDL-Strukturen manuell. Ein kuenftiger
//! `zerodds-idlc`-Codegen wird diese aus dem OMG-IDL-Schnipsel direkt
//! generieren.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_information::TypeIdentifierWithSize;
use crate::type_object::common::{decode_seq, encode_seq};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject};

// ============================================================================
// getTypes
// ============================================================================

/// Request fuer `TypeLookup::getTypes`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypesRequest {
    /// Angefragte TypeIdentifiers (typischerweise EK_MINIMAL/EK_COMPLETE).
    pub type_ids: Vec<TypeIdentifier>,
}

impl GetTypesRequest {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_seq(w, &self.type_ids, |w, t| t.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let type_ids = decode_seq(r, |rr| {
            TypeIdentifier::decode_from(rr).map_err(|e| zerodds_cdr::DecodeError::InvalidString {
                offset: 0,
                reason: match e {
                    zerodds_cdr::DecodeError::UnexpectedEof { .. } => "eof",
                    _ => "decode",
                },
            })
        })?;
        Ok(Self { type_ids })
    }
}

/// Ein getTypes-Reply-Item: ein TypeObject (Minimal oder Complete).
/// Der Kind wird im ersten Byte der serialisierten Form diskriminiert
/// (siehe [`crate::type_object::TypeObject`]).
///
/// Variant-Size wie [`crate::type_object::TypeObject`] — Boxing der
/// `Complete`-Variante waere Mikro-Optimierung mit Refactor-Pflicht
/// auf vielen Callsites; ReplyTypeObject lebt im TypeLookup-Reply-
/// Pfad, nicht auf dem Sample-Hot-Path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyTypeObject {
    /// Minimal-Variante.
    Minimal(MinimalTypeObject),
    /// Complete-Variante.
    Complete(CompleteTypeObject),
}

/// Antwort fuer `TypeLookup::getTypes`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypesReply {
    /// Liste der zurueckgelieferten TypeObjects. Nicht-gefundene werden
    /// typischerweise ausgelassen; der Caller muss nach TypeIdentifier
    /// matchen (ueber den Hash).
    pub types: Vec<ReplyTypeObject>,
}

impl GetTypesReply {
    /// Encode als sequence<TypeObject>.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_seq(w, &self.types, |w, t| match t {
            ReplyTypeObject::Minimal(m) => {
                crate::type_object::TypeObject::Minimal(m.clone()).encode_into(w)
            }
            ReplyTypeObject::Complete(c) => {
                crate::type_object::TypeObject::Complete(c.clone()).encode_into(w)
            }
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow / Unknown-TypeKind.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let n = r.read_u32()? as usize;
        // DoS-Cap: ein boeser Peer darf uns nicht mit `u32::MAX`-
        // Type-Objects GB an RAM allozieren. `safe_capacity` kappt
        // auf noch lesbare Bytes / min-elem-size (1 byte pro TypeObject
        // als konservative Untergrenze — ein echter TypeObject ist
        // mind. ~20 byte).
        let cap = crate::type_object::common::safe_capacity(n, 1, r.remaining());
        let mut types = Vec::with_capacity(cap);
        for _ in 0..n {
            let to = crate::type_object::TypeObject::decode_from(r)?;
            types.push(match to {
                crate::type_object::TypeObject::Minimal(m) => ReplyTypeObject::Minimal(m),
                crate::type_object::TypeObject::Complete(c) => ReplyTypeObject::Complete(c),
            });
        }
        Ok(Self { types })
    }
}

// ============================================================================
// getTypeDependencies
// ============================================================================

/// Opaque-Continuation-Point fuer paginierte Dependency-Listen.
/// XTypes spec §7.6.3.3.3 erlaubt bis zu 32 bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContinuationPoint(pub Vec<u8>);

impl ContinuationPoint {
    /// Maximum-Laenge (§7.6.3.3.3).
    pub const MAX_LEN: usize = 32;

    /// Encode als fixed-length octet[32] — wir kodieren es als
    /// sequence<octet> aus Vereinfachung (spec erlaubt beides via
    /// @bound; die meisten Implementierungen nutzen sequence).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        let len = u32::try_from(self.0.len().min(Self::MAX_LEN)).unwrap_or(Self::MAX_LEN as u32);
        w.write_u32(len)?;
        w.write_bytes(&self.0[..len as usize])
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow / Laenge > MAX_LEN.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let len = r.read_u32()? as usize;
        if len > Self::MAX_LEN {
            return Err(TypeCodecError::UnknownTypeKind { kind: 0 });
        }
        Ok(Self(r.read_bytes(len)?.to_vec()))
    }
}

/// Request fuer `TypeLookup::getTypeDependencies`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypeDependenciesRequest {
    /// TypeIds, deren Dependencies wir brauchen.
    pub type_ids: Vec<TypeIdentifier>,
    /// Continuation-Point aus vorigem Reply (leer bei erstem Request).
    pub continuation_point: ContinuationPoint,
}

impl GetTypeDependenciesRequest {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_seq(w, &self.type_ids, |w, t| t.encode_into(w))?;
        self.continuation_point.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let type_ids = decode_seq(r, |rr| {
            TypeIdentifier::decode_from(rr).map_err(|_| zerodds_cdr::DecodeError::InvalidString {
                offset: 0,
                reason: "type_id",
            })
        })?;
        let continuation_point = ContinuationPoint::decode_from(r)?;
        Ok(Self {
            type_ids,
            continuation_point,
        })
    }
}

/// Antwort fuer `TypeLookup::getTypeDependencies`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypeDependenciesReply {
    /// Dependencies.
    pub dependent_typeids: Vec<TypeIdentifierWithSize>,
    /// Continuation-Point (leer = Ende).
    pub continuation_point: ContinuationPoint,
}

impl GetTypeDependenciesReply {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        encode_seq(w, &self.dependent_typeids, |w, t| t.encode_into(w))?;
        self.continuation_point.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let dependent_typeids = decode_seq(r, |rr| {
            TypeIdentifierWithSize::decode_from(rr).map_err(|_| {
                zerodds_cdr::DecodeError::InvalidString {
                    offset: 0,
                    reason: "ti_size",
                }
            })
        })?;
        let continuation_point = ContinuationPoint::decode_from(r)?;
        Ok(Self {
            dependent_typeids,
            continuation_point,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::type_identifier::{EquivalenceHash, PrimitiveKind};
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

    fn roundtrip_get_types_request(req: GetTypesRequest) {
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypesRequest::decode_from(&mut r).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn get_types_request_roundtrips() {
        roundtrip_get_types_request(GetTypesRequest {
            type_ids: alloc::vec![
                TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0x01; 14])),
                TypeIdentifier::Primitive(PrimitiveKind::Int64),
            ],
        });
    }

    #[test]
    fn continuation_point_roundtrip_and_max_len() {
        let cp = ContinuationPoint(alloc::vec![0x11, 0x22, 0x33]);
        let mut w = BufferWriter::new(Endianness::Little);
        cp.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ContinuationPoint::decode_from(&mut r).unwrap(), cp);

        // MAX_LEN clamp
        let oversized = ContinuationPoint(alloc::vec![0xFF; 64]);
        let mut w = BufferWriter::new(Endianness::Little);
        oversized.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = ContinuationPoint::decode_from(&mut r).unwrap();
        assert_eq!(decoded.0.len(), ContinuationPoint::MAX_LEN);
    }

    #[test]
    fn get_types_reply_roundtrip_with_mixed_minimal_and_complete() {
        use crate::builder::TypeObjectBuilder;
        let min = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let com = CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_complete(),
        );
        let reply = GetTypesReply {
            types: alloc::vec![
                ReplyTypeObject::Minimal(min),
                ReplyTypeObject::Complete(com),
            ],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypesReply::decode_from(&mut r).unwrap();
        assert_eq!(decoded.types.len(), 2);
    }

    #[test]
    fn continuation_point_too_large_on_decode_rejected() {
        // u32-length > MAX_LEN → UnknownTypeKind (Platzhalter fuer
        // "length out of range"; passende ErrorVariante folgt.)
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(100).unwrap(); // > MAX_LEN=32
        w.write_bytes(&[0u8; 100]).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = ContinuationPoint::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, TypeCodecError::UnknownTypeKind { .. }));
    }

    #[test]
    fn get_type_dependencies_request_reply_roundtrip() {
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                [0xAA; 14]
            ))],
            continuation_point: ContinuationPoint::default(),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(
            GetTypeDependenciesRequest::decode_from(&mut r).unwrap(),
            req
        );

        let reply = GetTypeDependenciesReply {
            dependent_typeids: alloc::vec![TypeIdentifierWithSize {
                type_id: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0xBB; 14])),
                typeobject_serialized_size: 128,
            }],
            continuation_point: ContinuationPoint(alloc::vec![0x42]),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(
            GetTypeDependenciesReply::decode_from(&mut r).unwrap(),
            reply
        );
    }

    // ---- Edge cases for empty and boundary sequences --------------------

    #[test]
    fn empty_get_types_request_roundtrips() {
        roundtrip_get_types_request(GetTypesRequest::default());
    }

    #[test]
    fn empty_get_types_reply_roundtrips() {
        let reply = GetTypesReply::default();
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypesReply::decode_from(&mut r).unwrap();
        assert_eq!(decoded.types.len(), 0);
        assert_eq!(decoded, reply);
    }

    #[test]
    fn empty_get_type_dependencies_request_roundtrips() {
        let req = GetTypeDependenciesRequest::default();
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypeDependenciesRequest::decode_from(&mut r).unwrap();
        assert!(decoded.type_ids.is_empty());
        assert!(decoded.continuation_point.0.is_empty());
        assert_eq!(decoded, req);
    }

    #[test]
    fn empty_get_type_dependencies_reply_roundtrips() {
        let reply = GetTypeDependenciesReply::default();
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypeDependenciesReply::decode_from(&mut r).unwrap();
        assert!(decoded.dependent_typeids.is_empty());
        assert!(decoded.continuation_point.0.is_empty());
    }

    #[test]
    fn continuation_point_len_zero_encodes_to_four_zero_bytes() {
        let cp = ContinuationPoint(alloc::vec![]);
        let mut w = BufferWriter::new(Endianness::Little);
        cp.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        // length prefix only, no payload.
        assert_eq!(bytes, alloc::vec![0, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(ContinuationPoint::decode_from(&mut r).unwrap(), cp);
    }

    #[test]
    fn continuation_point_len_max_roundtrips() {
        let cp = ContinuationPoint(alloc::vec![0xAB; ContinuationPoint::MAX_LEN]);
        let mut w = BufferWriter::new(Endianness::Little);
        cp.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = ContinuationPoint::decode_from(&mut r).unwrap();
        assert_eq!(decoded.0.len(), ContinuationPoint::MAX_LEN);
        assert_eq!(decoded, cp);
    }

    #[test]
    fn continuation_point_len_max_plus_one_on_decode_rejected() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32((ContinuationPoint::MAX_LEN + 1) as u32)
            .unwrap();
        w.write_bytes(&[0u8; 33]).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let err = ContinuationPoint::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, TypeCodecError::UnknownTypeKind { .. }));
    }

    #[test]
    fn reply_type_object_minimal_and_complete_serialize_distinct_discriminator() {
        use crate::builder::TypeObjectBuilder;
        let min = ReplyTypeObject::Minimal(MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        ));
        let com = ReplyTypeObject::Complete(crate::type_object::CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_complete(),
        ));
        let reply = GetTypesReply {
            types: alloc::vec![min, com],
        };
        let mut w = BufferWriter::new(Endianness::Little);
        reply.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        // First 4 bytes are the sequence length = 2.
        assert_eq!(&bytes[..4], &[2, 0, 0, 0]);
        // Round-trip yields same variants order.
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = GetTypesReply::decode_from(&mut r).unwrap();
        assert!(matches!(decoded.types[0], ReplyTypeObject::Minimal(_)));
        assert!(matches!(decoded.types[1], ReplyTypeObject::Complete(_)));
    }

    #[test]
    fn get_type_dependencies_request_with_continuation_payload_roundtrips() {
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![],
            continuation_point: ContinuationPoint(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(
            GetTypeDependenciesRequest::decode_from(&mut r).unwrap(),
            req
        );
    }
}
