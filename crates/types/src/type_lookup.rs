// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The ten TypeLookup Service types of XTypes 1.3 §7.6.3.3.3.
//!
//! The clause declares ten types and this module carries all ten. Four of
//! them are the operation parameters — `TypeLookup_getTypes_In`,
//! `TypeLookup_getTypes_Out`, `TypeLookup_getTypeDependencies_In` and
//! `TypeLookup_getTypeDependencies_Out`, here [`GetTypesRequest`],
//! [`GetTypesReply`], [`GetTypeDependenciesRequest`] and
//! [`GetTypeDependenciesReply`]. Six more carry them: the two result
//! unions `TypeLookup_getTypes_Result` and
//! `TypeLookup_getTypeDependencies_Result`, the two dispatch unions
//! `TypeLookup_Call` and `TypeLookup_Return`, and the two RPC envelopes
//! `TypeLookup_Request` (`@RPCRequestType`) and `TypeLookup_Reply`
//! (`@RPCReplyType`) that Table 61 names as the topic data of the four
//! built-in endpoints. Without those six there is no call, only the
//! payload of a call.
//!
//! ## Framing
//!
//! The four parameter types are `@extensibility(MUTABLE)` and carry every
//! member with `@hashid`, the member id derived from the member's *name*
//! (§7.3.1.2.1.1) rather than from its position — see [`member_ids`].
//!
//! The four dispatch/result unions are appendable. The RPC envelopes and
//! their headers are final (DDSXTY14-30), as implemented by installed
//! Cyclone 0.11.0 and OpenDDS schemas. Replies use ReplyHeader, not an
//! echoed RequestHeader. The old local envelope was a private wire form.
//!
//! ## Encoding
//!
//! The codecs below follow the encoding version of the stream they are
//! given ([`crate::framing::EncodingVersion`]); [`TypeLookupEncoding`]
//! names the two a sample is sent in.
//!
//! - Encoding version 2, `CDR2_LE` `{0x00, 0x07}`, is the default. DDS-XTypes
//!   1.4 Beta 1 (ptc/2026-02-06) §7.6.3.3.3: "The Data Representation used
//!   by these endpoints shall be Extended CDR Representation (encoding
//!   version 2)", the resolution of OMG issue DDSXTY14-29; the same clause
//!   makes the envelopes `@final`. A `DHEADER` over each parameter list and
//!   an `EMHEADER` per member, the form every measured implementation writes.
//! - Encoding version 1, `CDR_LE` `{0x00, 0x01}`, is the setting `xcdr1`,
//!   the reading of DDS-XTypes 1.3, which names no representation for these
//!   endpoints: an empty `DataRepresentationQosPolicy` is `XCDR`
//!   (§7.6.3.1.1), the unannotated envelopes are appendable
//!   (§7.3.1.2.1.8), Table 60 gives `CDR_LE`, and DDSI-RTPS 2.5 §10.4
//!   Table 10.2 lists it. Final and appendable are the same bytes under
//!   version 1 (§7.4.1.1). The four parameter types are PL_CDR1 parameter
//!   lists; their `@hashid` ids are above `0x3F00` and take `PID_EXTENDED`
//!   (§7.4.1.2.2).
//!
//! ## Service IDL, including the RPC-envelope correction
//!
//! ```idl
//! module dds { module builtin {
//! const unsigned long TypeLookup_getTypes_HashId        = 0x018252d3;
//! const unsigned long TypeLookup_getDependencies_HashId = 0x05aafb31;
//!
//! @extensibility(MUTABLE)
//! struct TypeLookup_getTypes_In {
//!     @hashid sequence<TypeIdentifier> type_ids;
//! };
//! @extensibility(MUTABLE)
//! struct TypeLookup_getTypes_Out {
//!     @hashid sequence<TypeIdentifierTypeObjectPair> types;
//!     @hashid sequence<TypeIdentifierPair>           complete_to_minimal;
//! };
//! union TypeLookup_getTypes_Result switch(long) {
//!   case DDS_RETCODE_OK: TypeLookup_getTypes_Out result;
//! };
//!
//! @extensibility(MUTABLE)
//! struct TypeLookup_getTypeDependencies_In {
//!     @hashid sequence<TypeIdentifier> type_ids;
//!     @hashid sequence<octet, 32>      continuation_point;
//! };
//! @extensibility(MUTABLE)
//! struct TypeLookup_getTypeDependencies_Out {
//!     @hashid sequence<TypeIdentifierWithSize> dependent_typeids;
//!     @hashid sequence<octet, 32>              continuation_point;
//! };
//! union TypeLookup_getTypeDependencies_Result switch(long) {
//!   case DDS_RETCODE_OK: TypeLookup_getTypeDependencies_Out result;
//! };
//!
//! union TypeLookup_Call switch(long) {
//!   case TypeLookup_getTypes_HashId:        TypeLookup_getTypes_In            getTypes;
//!   case TypeLookup_getDependencies_HashId: TypeLookup_getTypeDependencies_In getTypeDependencies;
//! };
//! @RPCRequestType @final
//! struct TypeLookup_Request {
//!     dds::rpc::RequestHeader header;
//!     TypeLookup_Call         data;
//! };
//! union TypeLookup_Return switch(long) {
//!   case TypeLookup_getTypes_HashId:        TypeLookup_getTypes_Result            getType;
//!   case TypeLookup_getDependencies_HashId: TypeLookup_getTypeDependencies_Result getTypeDependencies;
//! };
//! @RPCReplyType @final
//! struct TypeLookup_Reply {
//!     dds::rpc::ReplyHeader header;
//!     TypeLookup_Return     return;
//! };
//! }} // dds::builtin
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::framing::{
    decode_mutable, encode_appendable, encode_mutable,
    encode_mutable_length_prefixed_member as encode_mutable_member,
};
use crate::type_identifier::TypeIdentifier;
use crate::type_information::TypeIdentifierWithSize;
use crate::type_object::TypeObject;
use crate::type_object::common::{
    decode_seq_appendable as decode_seq, encode_seq_appendable as encode_seq,
};

/// The member ids `@hashid` gives the members of §7.6.3.3.3.
///
/// A `@hashid` with no argument hashes the member's own name
/// (§7.3.1.2.1.4), and §7.3.1.2.1.1 derives the id from it: the first
/// four octets of `MD5(name)` read as a **little-endian** `u32`, masked
/// with `0x0FFFFFFF`. [`NameHash::member_id_from_name`](crate::type_object::common::NameHash::member_id_from_name) is that
/// derivation and [`member_ids_are_the_hashid_of_their_name`] recomputes
/// every constant below from it, so a transcription slip here fails a
/// test rather than a peer.
///
/// The clause itself is the cross-check that the derivation is the right
/// way round: it publishes `TypeLookup_getTypes_HashId = 0x018252d3` for
/// the string `getTypes` and `TypeLookup_getDependencies_HashId =
/// 0x05aafb31` for `getDependencies`, and the same function reproduces
/// both ([`OPERATION_GET_TYPES`], [`OPERATION_GET_DEPENDENCIES`]).
///
/// [`member_ids_are_the_hashid_of_their_name`]: self
pub mod member_ids {
    /// `TypeLookup_getTypes_In::type_ids` and
    /// `TypeLookup_getTypeDependencies_In::type_ids`.
    pub const TYPE_IDS: u32 = 0x0c53_6065;
    /// `TypeLookup_getTypes_Out::types`.
    pub const TYPES: u32 = 0x0280_4ad1;
    /// `TypeLookup_getTypes_Out::complete_to_minimal`.
    pub const COMPLETE_TO_MINIMAL: u32 = 0x0b8e_6577;
    /// `TypeLookup_getTypeDependencies_In::continuation_point` and its
    /// `_Out` counterpart.
    pub const CONTINUATION_POINT: u32 = 0x0508_e3d2;
    /// `TypeLookup_getTypeDependencies_Out::dependent_typeids`.
    pub const DEPENDENT_TYPEIDS: u32 = 0x0ba4_dfc9;
}

/// `TypeLookup_getTypes_HashId` — the `TypeLookup_Call` and
/// `TypeLookup_Return` discriminator for the `getTypes` operation
/// (§7.6.3.3.3, "computed from `@hashid("getTypes")`").
pub const OPERATION_GET_TYPES: i32 = 0x0182_52d3;

/// `TypeLookup_getDependencies_HashId` — the discriminator for the
/// `getTypeDependencies` operation (§7.6.3.3.3, "computed from
/// `@hashid("getDependencies")`"). The operation is spelled
/// `getTypeDependencies` and the hash is taken over `getDependencies`;
/// that is the clause's own wording and the published constant agrees
/// with it, so the string is not a typo this code may quietly correct.
pub const OPERATION_GET_DEPENDENCIES: i32 = 0x05aa_fb31;

/// `DDS_RETCODE_OK`, the one case label both `_Result` unions carry.
pub const RETCODE_OK: i32 = 0;

// ============================================================================
// TypeIdentifierTypeObjectPair / TypeIdentifierPair (Annex B)
// ============================================================================

/// `@extensibility(FINAL) @nested struct TypeIdentifierTypeObjectPair`
/// (Annex B).
///
/// The identifier travels with the object. A reply that carried only the
/// objects would force the requester to re-hash every one of them to find
/// out which of its questions each answers — and to guess, for a
/// `COMPLETE` object returned against a `MINIMAL` question, that it is an
/// answer at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierTypeObjectPair {
    /// The identifier the object was asked for under.
    pub type_identifier: TypeIdentifier,
    /// The object itself.
    pub type_object: TypeObject,
}

impl TypeIdentifierTypeObjectPair {
    /// Encode. `FINAL`, so no `DHEADER` of its own; the `TypeObject`
    /// brings its own, being `APPENDABLE`.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        self.type_identifier.encode_into(w)?;
        self.type_object.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the object names a kind the
    /// specification does not define.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let type_identifier = TypeIdentifier::decode_from(r)?;
        let type_object = TypeObject::decode_from(r)?;
        Ok(Self {
            type_identifier,
            type_object,
        })
    }
}

/// `@extensibility(FINAL) @nested struct TypeIdentifierPair` (Annex B).
///
/// In `TypeLookup_getTypes_Out::complete_to_minimal` the first is the
/// `COMPLETE` identifier and the second its `MINIMAL` counterpart
/// (§7.6.3.3.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierPair {
    /// `type_identifier1`.
    pub type_identifier1: TypeIdentifier,
    /// `type_identifier2`.
    pub type_identifier2: TypeIdentifier,
}

impl TypeIdentifierPair {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        self.type_identifier1.encode_into(w)?;
        self.type_identifier2.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            type_identifier1: TypeIdentifier::decode_from(r)?,
            type_identifier2: TypeIdentifier::decode_from(r)?,
        })
    }
}

// ============================================================================
// getTypes
// ============================================================================

/// `TypeLookup_getTypes_In` — the request parameters of `getTypes`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypesRequest {
    /// `@hashid sequence<TypeIdentifier> type_ids`. §7.6.3.3.4.2: direct
    /// HASH identifiers only, and an SCC's own identifier rather than the
    /// identifiers of its members.
    pub type_ids: Vec<TypeIdentifier>,
}

impl GetTypesRequest {
    /// Encode with the `MUTABLE` framing of §7.6.3.3.3.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_mutable(w, |w| {
            encode_mutable_member(w, member_ids::TYPE_IDS, |w| {
                encode_seq(w, &self.type_ids, |w, t| t.encode_into(w))
            })
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or a member's `EMHEADER` names
    /// a length that runs past the member list.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        // The element's own error, not a re-labelled one. The `cdr`
        // version replaced it with `InvalidString { offset: 0, reason:
        // "eof" | "decode" }`, which threw away the offset the runtime
        // had already computed and named a string error for a type
        // identifier.
        Ok(decode_mutable(r, Self::default(), |acc, id, r| {
            if id == member_ids::TYPE_IDS {
                acc.type_ids = decode_seq(r, TypeIdentifier::decode_from)?;
            }
            Ok(())
        })?)
    }
}

/// `TypeLookup_getTypes_Out` — the reply parameters of `getTypes`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypesReply {
    /// `@hashid sequence<TypeIdentifierTypeObjectPair> types`. Types the
    /// server does not know are omitted; the identifier in each pair says
    /// which question the object answers.
    pub types: Vec<TypeIdentifierTypeObjectPair>,
    /// `@hashid sequence<TypeIdentifierPair> complete_to_minimal`.
    ///
    /// §7.6.3.3.4.2: the mapping from `COMPLETE` to `MINIMAL` identifiers
    /// for any `COMPLETE` identifier that appears inside a `COMPLETE`
    /// TypeObject sent in answer to a `MINIMAL` question. It is what lets
    /// a server answer every `getTypes` with `COMPLETE` objects.
    pub complete_to_minimal: Vec<TypeIdentifierPair>,
}

impl GetTypesReply {
    /// Encode with the `MUTABLE` framing of §7.6.3.3.3.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_mutable(w, |w| {
            encode_mutable_member(w, member_ids::TYPES, |w| {
                encode_seq(w, &self.types, |w, t| t.encode_into(w))
            })?;
            encode_mutable_member(w, member_ids::COMPLETE_TO_MINIMAL, |w| {
                encode_seq(w, &self.complete_to_minimal, |w, t| t.encode_into(w))
            })
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow, an unknown TypeKind, or a member whose
    /// `EMHEADER` names a length that runs past the member list.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(decode_mutable(r, Self::default(), |acc, id, r| {
            match id {
                member_ids::TYPES => {
                    acc.types = decode_seq(r, |r| {
                        let at = r.position();
                        TypeIdentifierTypeObjectPair::decode_from(r).map_err(|e| e.into_wire(at))
                    })?;
                }
                member_ids::COMPLETE_TO_MINIMAL => {
                    acc.complete_to_minimal = decode_seq(r, TypeIdentifierPair::decode_from)?;
                }
                _ => {}
            }
            Ok(())
        })?)
    }
}

/// `union TypeLookup_getTypes_Result switch(long)` — a return code and,
/// for `DDS_RETCODE_OK`, the reply parameters.
///
/// A discriminator the union has no case for is not an error: the union
/// has no `default:` arm, so §7.4.3.5.3 leaves the branch absent, and a
/// server reporting a failure is exactly that shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetTypesResult {
    /// `case DDS_RETCODE_OK: TypeLookup_getTypes_Out result;`
    Ok(GetTypesReply),
    /// Any other return code, carrying no branch.
    Retcode(i32),
}

impl Default for GetTypesResult {
    fn default() -> Self {
        Self::Ok(GetTypesReply::default())
    }
}

impl GetTypesResult {
    /// The union's discriminator.
    #[must_use]
    pub const fn retcode(&self) -> i32 {
        match self {
            Self::Ok(_) => RETCODE_OK,
            Self::Retcode(c) => *c,
        }
    }

    /// Encode. The union carries no `@extensibility`, so §7.3.1.2.1.8
    /// makes it appendable and it opens with a `DHEADER`.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| {
            w.write_i32(self.retcode());
            match self {
                Self::Ok(out) => out.encode_into(w),
                Self::Retcode(_) => Ok(()),
            }
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let code = r.read_i32()?;
            if code == RETCODE_OK {
                let at = r.position();
                Ok(Self::Ok(
                    GetTypesReply::decode_from(r).map_err(|e| e.into_wire(at))?,
                ))
            } else {
                Ok(Self::Retcode(code))
            }
        })?)
    }
}

// ============================================================================
// getTypeDependencies
// ============================================================================

/// Opaque continuation point for paginated dependency lists —
/// `sequence<octet, 32>` in §7.6.3.3.3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContinuationPoint(pub Vec<u8>);

impl ContinuationPoint {
    /// Maximum length (§7.6.3.3.3).
    pub const MAX_LEN: usize = 32;

    /// Encode as `sequence<octet, 32>`.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        let len = u32::try_from(self.0.len().min(Self::MAX_LEN)).unwrap_or(Self::MAX_LEN as u32);
        w.write_u32(len);
        {
            w.write_bytes(&self.0[..len as usize]);
            Ok(())
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow / length > MAX_LEN.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let len = r.read_u32()? as usize;
        if len > Self::MAX_LEN {
            return Err(TypeCodecError::UnknownTypeKind { kind: 0 });
        }
        Ok(Self(r.read_bytes(len)?.to_vec()))
    }
}

/// `TypeLookup_getTypeDependencies_In` — the request parameters of
/// `getTypeDependencies`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypeDependenciesRequest {
    /// `@hashid sequence<TypeIdentifier> type_ids`.
    pub type_ids: Vec<TypeIdentifier>,
    /// `@hashid sequence<octet, 32> continuation_point` — the one from
    /// the previous reply, empty on the first request.
    pub continuation_point: ContinuationPoint,
}

impl GetTypeDependenciesRequest {
    /// Encode with the `MUTABLE` framing of §7.6.3.3.3.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_mutable(w, |w| {
            encode_mutable_member(w, member_ids::TYPE_IDS, |w| {
                encode_seq(w, &self.type_ids, |w, t| t.encode_into(w))
            })?;
            encode_mutable_member(w, member_ids::CONTINUATION_POINT, |w| {
                self.continuation_point.encode_into(w)
            })
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(decode_mutable(r, Self::default(), |acc, id, r| {
            match id {
                member_ids::TYPE_IDS => {
                    acc.type_ids = decode_seq(r, TypeIdentifier::decode_from)?;
                }
                member_ids::CONTINUATION_POINT => {
                    let at = r.position();
                    acc.continuation_point =
                        ContinuationPoint::decode_from(r).map_err(|e| e.into_wire(at))?;
                }
                _ => {}
            }
            Ok(())
        })?)
    }
}

/// `TypeLookup_getTypeDependencies_Out` — the reply parameters of
/// `getTypeDependencies`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GetTypeDependenciesReply {
    /// `@hashid sequence<TypeIdentifierWithSize> dependent_typeids`.
    pub dependent_typeids: Vec<TypeIdentifierWithSize>,
    /// `@hashid sequence<octet, 32> continuation_point` — empty means the
    /// list is complete.
    pub continuation_point: ContinuationPoint,
}

impl GetTypeDependenciesReply {
    /// Encode with the `MUTABLE` framing of §7.6.3.3.3.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_mutable(w, |w| {
            encode_mutable_member(w, member_ids::DEPENDENT_TYPEIDS, |w| {
                encode_seq(w, &self.dependent_typeids, |w, t| t.encode_into(w))
            })?;
            encode_mutable_member(w, member_ids::CONTINUATION_POINT, |w| {
                self.continuation_point.encode_into(w)
            })
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(decode_mutable(r, Self::default(), |acc, id, r| {
            match id {
                member_ids::DEPENDENT_TYPEIDS => {
                    acc.dependent_typeids = decode_seq(r, |rr| {
                        let at = rr.position();
                        TypeIdentifierWithSize::decode_from(rr).map_err(|e| e.into_wire(at))
                    })?;
                }
                member_ids::CONTINUATION_POINT => {
                    let at = r.position();
                    acc.continuation_point =
                        ContinuationPoint::decode_from(r).map_err(|e| e.into_wire(at))?;
                }
                _ => {}
            }
            Ok(())
        })?)
    }
}

/// `union TypeLookup_getTypeDependencies_Result switch(long)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetTypeDependenciesResult {
    /// `case DDS_RETCODE_OK: TypeLookup_getTypeDependencies_Out result;`
    Ok(GetTypeDependenciesReply),
    /// Any other return code, carrying no branch.
    Retcode(i32),
}

impl Default for GetTypeDependenciesResult {
    fn default() -> Self {
        Self::Ok(GetTypeDependenciesReply::default())
    }
}

impl GetTypeDependenciesResult {
    /// The union's discriminator.
    #[must_use]
    pub const fn retcode(&self) -> i32 {
        match self {
            Self::Ok(_) => RETCODE_OK,
            Self::Retcode(c) => *c,
        }
    }

    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| {
            w.write_i32(self.retcode());
            match self {
                Self::Ok(out) => out.encode_into(w),
                Self::Retcode(_) => Ok(()),
            }
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let code = r.read_i32()?;
            if code == RETCODE_OK {
                let at = r.position();
                Ok(Self::Ok(
                    GetTypeDependenciesReply::decode_from(r).map_err(|e| e.into_wire(at))?,
                ))
            } else {
                Ok(Self::Retcode(code))
            }
        })?)
    }
}

// ============================================================================
// The call, the return, and the two RPC envelopes
// ============================================================================

/// `union TypeLookup_Call switch(long)` — which of the two operations is
/// being invoked, and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupCall {
    /// `case TypeLookup_getTypes_HashId: TypeLookup_getTypes_In getTypes;`
    GetTypes(GetTypesRequest),
    /// `case TypeLookup_getDependencies_HashId:
    /// TypeLookup_getTypeDependencies_In getTypeDependencies;`
    GetTypeDependencies(GetTypeDependenciesRequest),
}

impl TypeLookupCall {
    /// The operation hash this call selects.
    #[must_use]
    pub const fn operation(&self) -> i32 {
        match self {
            Self::GetTypes(_) => OPERATION_GET_TYPES,
            Self::GetTypeDependencies(_) => OPERATION_GET_DEPENDENCIES,
        }
    }

    /// Encode. Appendable by §7.3.1.2.1.8, so a `DHEADER` first.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| {
            w.write_i32(self.operation());
            match self {
                Self::GetTypes(r) => r.encode_into(w),
                Self::GetTypeDependencies(r) => r.encode_into(w),
            }
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the discriminator names
    /// neither of the two operations §7.6.3.3.3 defines. The union has no
    /// `default:` arm, so an unknown operation is refused rather than
    /// silently read as one of the two.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let at = r.position();
            let op = r.read_i32()?;
            match op {
                OPERATION_GET_TYPES => Ok(Self::GetTypes(
                    GetTypesRequest::decode_from(r).map_err(|e| e.into_wire(at))?,
                )),
                OPERATION_GET_DEPENDENCIES => Ok(Self::GetTypeDependencies(
                    GetTypeDependenciesRequest::decode_from(r).map_err(|e| e.into_wire(at))?,
                )),
                other => Err(WireError::OutOfRange {
                    at,
                    expected: alloc::format!(
                        "one of the two TypeLookup operation hashes of \
                         XTypes 1.3 §7.6.3.3.3, and 0x{other:08x} is neither"
                    ),
                }),
            }
        })?)
    }
}

/// `union TypeLookup_Return switch(long)` — which operation is being
/// answered, and its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupReturn {
    /// `case TypeLookup_getTypes_HashId: TypeLookup_getTypes_Result getType;`
    GetTypes(GetTypesResult),
    /// `case TypeLookup_getDependencies_HashId:
    /// TypeLookup_getTypeDependencies_Result getTypeDependencies;`
    GetTypeDependencies(GetTypeDependenciesResult),
}

impl TypeLookupReturn {
    /// The operation hash this return answers.
    #[must_use]
    pub const fn operation(&self) -> i32 {
        match self {
            Self::GetTypes(_) => OPERATION_GET_TYPES,
            Self::GetTypeDependencies(_) => OPERATION_GET_DEPENDENCIES,
        }
    }

    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| {
            w.write_i32(self.operation());
            match self {
                Self::GetTypes(r) => r.encode_into(w),
                Self::GetTypeDependencies(r) => r.encode_into(w),
            }
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the discriminator names
    /// neither operation.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(crate::framing::decode_appendable(r, |r| {
            let at = r.position();
            let op = r.read_i32()?;
            match op {
                OPERATION_GET_TYPES => Ok(Self::GetTypes(
                    GetTypesResult::decode_from(r).map_err(|e| e.into_wire(at))?,
                )),
                OPERATION_GET_DEPENDENCIES => Ok(Self::GetTypeDependencies(
                    GetTypeDependenciesResult::decode_from(r).map_err(|e| e.into_wire(at))?,
                )),
                other => Err(WireError::OutOfRange {
                    at,
                    expected: alloc::format!(
                        "one of the two TypeLookup operation hashes of \
                         XTypes 1.3 §7.6.3.3.3, and 0x{other:08x} is neither"
                    ),
                }),
            }
        })?)
    }
}

/// `dds::rpc::RequestHeader` carried by the TypeLookup request envelope.
///
/// [DDS-RPC] §7.5.1.1.1: a `SampleIdentity` (the sending writer's GUID
/// and the sample's sequence number) and the service instance name.
/// §7.6.3.3.4 fixes the latter for this service: `"dds.builtin.TOS."`
/// followed by the 32-character lower-case hexadecimal participant GUID
/// — [`crate::type_lookup::service_instance_name`].
///
/// It is declared here rather than taken from `zerodds-rpc` because that
/// crate depends on this one; `zerodds-rpc`'s
/// `common_types::RequestHeader` uses the same high/low conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestHeader {
    /// The sending writer's GUID.
    pub writer_guid: [u8; 16],
    /// The request's sequence number.
    pub sequence_number: u64,
    /// The service instance name.
    pub instance_name: String,
}

/// RTPS SequenceNumber is two ordered 32-bit words, not a CDR uint64.
#[must_use]
pub const fn sequence_number_parts(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, value as u32)
}

/// Reassemble the high/low wire words, preserving the existing public u64 API.
#[must_use]
pub const fn sequence_number_from_parts(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

impl RequestHeader {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_bytes(&self.writer_guid);
        let (high, low) = sequence_number_parts(self.sequence_number);
        w.write_u32(high);
        w.write_u32(low);
        w.write_string(&self.instance_name);
        Ok(())
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the name is not UTF-8.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let mut writer_guid = [0u8; 16];
        writer_guid.copy_from_slice(r.read_bytes(16)?);
        let sequence_number = sequence_number_from_parts(r.read_u32()?, r.read_u32()?);
        let instance_name = r.read_string()?;
        Ok(Self {
            writer_guid,
            sequence_number,
            instance_name,
        })
    }
}

/// DDS-RPC reply correlation header. A reply carries a remote exception code,
/// not the request's instance-name string (Cyclone/OpenDDS service schemas).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyHeader {
    /// Original request writer identity, not the reply writer's GUID.
    pub writer_guid: [u8; 16],
    /// Original request sequence number.
    pub sequence_number: u64,
    /// DDS-RPC RemoteExceptionCode; zero means normal operation result.
    pub remote_ex: u32,
}

impl ReplyHeader {
    /// Encode the final DDS-RPC reply header.
    pub fn encode_into(&self, w: &mut Writer) {
        w.write_bytes(&self.writer_guid);
        let (high, low) = sequence_number_parts(self.sequence_number);
        w.write_u32(high);
        w.write_u32(low);
        w.write_u32(self.remote_ex);
    }

    /// Decode the final DDS-RPC reply header.
    ///
    /// # Errors
    /// The input ends inside the fixed header.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let mut writer_guid = [0; 16];
        writer_guid.copy_from_slice(r.read_bytes(16)?);
        Ok(Self {
            writer_guid,
            sequence_number: sequence_number_from_parts(r.read_u32()?, r.read_u32()?),
            remote_ex: r.read_u32()?,
        })
    }
}

/// The prefix §7.6.3.3.4 puts in front of the participant GUID in the
/// service instance name.
pub const SERVICE_INSTANCE_NAME_PREFIX: &str = "dds.builtin.TOS.";

/// The `instanceName` of §7.6.3.3.4: the prefix, then the participant
/// GUID as lower-case hexadecimal with no `0x` (participant EntityId 000001c1).
#[must_use]
pub fn service_instance_name(guid_prefix: &[u8; 12]) -> String {
    let mut out = String::from(SERVICE_INSTANCE_NAME_PREFIX);
    for b in guid_prefix {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0x0F), 16).unwrap_or('0'));
    }
    out.push_str("000001c1");
    out
}

/// The encoding the TypeLookup service writes its samples in; see the
/// module documentation. `wire.type_lookup_encoding` of the DCPS runtime.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypeLookupEncoding {
    /// `CDR_LE` `{0x00, 0x01}`, encoding version 1: XTypes 1.3 Table 60
    /// for an appendable type under the default representation, and one
    /// of the identifiers of DDSI-RTPS 2.5 Table 10.2.
    Xcdr1,
    /// `CDR2_LE` `{0x00, 0x07}`, version 2, FINAL envelopes (1.4b1 §7.6.3.3.3).
    #[default]
    Xcdr2,
}

impl TypeLookupEncoding {
    /// `xcdr1` or `xcdr2`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "xcdr1" => Some(Self::Xcdr1),
            "xcdr2" => Some(Self::Xcdr2),
            _ => None,
        }
    }

    /// The name [`Self::parse`] takes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xcdr1 => "xcdr1",
            Self::Xcdr2 => "xcdr2",
        }
    }

    /// The `RepresentationIdentifier` a little-endian sample opens with.
    #[must_use]
    pub const fn representation_identifier(self) -> [u8; 2] {
        match self {
            Self::Xcdr1 => [0x00, 0x01],
            Self::Xcdr2 => [0x00, 0x07],
        }
    }

    /// A little-endian writer of this encoding.
    #[must_use]
    pub fn writer(self) -> Writer {
        match self {
            Self::Xcdr1 => crate::framing::xcdr1_writer(zerodds_wire::Endianness::Little),
            Self::Xcdr2 => crate::framing::xcdr2_writer(),
        }
    }
}

/// What the encapsulation header of a received TypeLookup sample says
/// about the bytes behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLookupEncapsulation {
    /// The byte order of the body.
    pub endianness: zerodds_wire::Endianness,
    /// The encoding version of the body.
    pub version: crate::framing::EncodingVersion,
    /// The options word (XTypes 1.3 §7.6.3.1.2): its low two bits count
    /// the padding octets after the body.
    pub options: [u8; 2],
}

impl TypeLookupEncapsulation {
    /// Read the first four octets of a sample. `CDR_BE` and `CDR_LE` are
    /// encoding version 1 and `CDR2_BE` and `CDR2_LE` version 2, which is
    /// what the identifiers name (Table 60); any other identifier is
    /// returned as the error, for the caller to report.
    ///
    /// # Errors
    /// The identifier, or `None` for a payload shorter than four octets.
    pub fn read(payload: &[u8]) -> Result<Self, Option<[u8; 2]>> {
        use crate::framing::EncodingVersion::{Xcdr1, Xcdr2};
        use zerodds_wire::Endianness::{Big, Little};
        let Some(&[id0, id1, o0, o1]) = payload.get(..4) else {
            return Err(None);
        };
        let (endianness, version) = match [id0, id1] {
            [0, 0] => (Big, Xcdr1),
            [0, 1] => (Little, Xcdr1),
            [0, 6] => (Big, Xcdr2),
            [0, 7] => (Little, Xcdr2),
            other => return Err(Some(other)),
        };
        Ok(Self {
            endianness,
            version,
            options: [o0, o1],
        })
    }

    /// A reader over `body` in this encapsulation's byte order and
    /// encoding version.
    #[must_use]
    pub fn reader<'a>(&self, body: &'a [u8]) -> Reader<'a> {
        match self.version {
            crate::framing::EncodingVersion::Xcdr1 => {
                crate::framing::xcdr1_reader(body, self.endianness)
            }
            crate::framing::EncodingVersion::Xcdr2 => {
                Reader::with_max_align(body, self.endianness, crate::framing::XCDR2_MAX_ALIGN)
            }
        }
    }
}

/// A sample of the TypeLookup service: the encapsulation header of
/// `encoding`, the body `encode` writes, and the zero octets that take it
/// to a four-octet offset.
///
/// XTypes 1.3 §7.6.3.1.2: the two low bits of the options word "shall" be
/// the number of padding octets "needed after the end of the serialized
/// payload in order to reach the next 4-byte aligned offset". A reader
/// takes them off ([`TypeLookupEncapsulation::options`]); Cyclone DDS
/// 11.0.1 writes them the same way (`0001` on a request one octet short of
/// four, measured on the Linux test host).
fn encode_payload(
    encoding: TypeLookupEncoding,
    encode: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Result<Vec<u8>, WireError> {
    let mut w = encoding.writer();
    encode(&mut w)?;
    let body = w.finish();
    let padding = (4 - body.len() % 4) % 4;
    let mut payload = Vec::with_capacity(4 + body.len() + padding);
    payload.extend_from_slice(&encoding.representation_identifier());
    #[allow(clippy::cast_possible_truncation)] // 0 to 3
    payload.extend_from_slice(&[0, padding as u8]);
    payload.extend_from_slice(&body);
    payload.resize(payload.len() + padding, 0);
    Ok(payload)
}

/// `@RPCRequestType struct TypeLookup_Request` — the topic data of
/// `ENTITYID_TL_SVC_REQ_WRITER` / `_READER` (Table 61).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupRequest {
    /// `dds::rpc::RequestHeader header;`
    pub header: RequestHeader,
    /// `TypeLookup_Call data;`
    pub data: TypeLookupCall,
}

impl TypeLookupRequest {
    /// The whole sample in `encoding`: encapsulation header, then the
    /// request.
    ///
    /// # Errors
    /// As [`Self::encode_into`].
    pub fn to_payload(&self, encoding: TypeLookupEncoding) -> Result<Vec<u8>, WireError> {
        encode_payload(encoding, |w| self.encode_into(w))
    }

    /// Encode the final RPC envelope (DDSXTY14-30).
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        self.header.encode_into(w)?;
        self.data.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the call names an operation
    /// §7.6.3.3.3 does not define.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let header = RequestHeader::decode_from(r)?;
        let data = TypeLookupCall::decode_from(r)?;
        Ok(Self { header, data })
    }
}

/// `@RPCReplyType struct TypeLookup_Reply` — the topic data of
/// `ENTITYID_TL_SVC_REPLY_WRITER` / `_READER` (Table 61).
///
/// The clause's second member is spelled `return`, which is a keyword in
/// IDL and in Rust alike; it is `ret` here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupReplyMessage {
    /// `dds::rpc::ReplyHeader header;`
    pub header: ReplyHeader,
    /// `TypeLookup_Return return;`
    pub ret: TypeLookupReturn,
}

impl TypeLookupReplyMessage {
    /// The whole sample in `encoding`: encapsulation header, then the
    /// reply.
    ///
    /// # Errors
    /// As [`Self::encode_into`].
    pub fn to_payload(&self, encoding: TypeLookupEncoding) -> Result<Vec<u8>, WireError> {
        encode_payload(encoding, |w| self.encode_into(w))
    }

    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        self.header.encode_into(w);
        self.ret.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value, or the return names an
    /// operation §7.6.3.3.3 does not define.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let header = ReplyHeader::decode_from(r)?;
        let ret = TypeLookupReturn::decode_from(r)?;
        Ok(Self { header, ret })
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
    use crate::type_object::common::NameHash;

    fn roundtrip_get_types_request(req: GetTypesRequest) {
        let mut w = crate::framing::xcdr2_writer();
        req.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = GetTypesRequest::decode_from(&mut r).unwrap();
        assert_eq!(decoded, req);
    }

    fn sample_minimal() -> MinimalTypeObject {
        use crate::builder::TypeObjectBuilder;
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        )
    }

    fn sample_complete() -> CompleteTypeObject {
        use crate::builder::TypeObjectBuilder;
        CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_complete(),
        )
    }

    fn pair(object: TypeObject) -> TypeIdentifierTypeObjectPair {
        let type_identifier = crate::hash::to_hashed_type_identifier(&object).unwrap();
        TypeIdentifierTypeObjectPair {
            type_identifier,
            type_object: object,
        }
    }

    use crate::type_object::{CompleteTypeObject, MinimalTypeObject};

    // ------------------------------------------------------------------
    // The member ids of §7.6.3.3.3
    // ------------------------------------------------------------------

    /// Every constant of [`member_ids`] is the `@hashid` of the member
    /// name §7.6.3.3.3 gives it, recomputed rather than transcribed.
    #[test]
    fn member_ids_are_the_hashid_of_their_name() {
        assert_eq!(
            member_ids::TYPE_IDS,
            NameHash::member_id_from_name("type_ids")
        );
        assert_eq!(member_ids::TYPES, NameHash::member_id_from_name("types"));
        assert_eq!(
            member_ids::COMPLETE_TO_MINIMAL,
            NameHash::member_id_from_name("complete_to_minimal")
        );
        assert_eq!(
            member_ids::CONTINUATION_POINT,
            NameHash::member_id_from_name("continuation_point")
        );
        assert_eq!(
            member_ids::DEPENDENT_TYPEIDS,
            NameHash::member_id_from_name("dependent_typeids")
        );
    }

    /// §7.6.3.3.3 publishes both operation hashes, so the same derivation
    /// that produced the member ids has a check with a printed answer:
    /// `getTypes` is `0x018252d3` and `getDependencies` is `0x05aafb31`.
    /// Read the four MD5 octets big-endian instead and they come out
    /// `0x00d3528…` — the clause's own numbers are what settles the
    /// direction.
    #[test]
    fn the_operation_hashes_are_the_ones_the_clause_prints() {
        assert_eq!(
            OPERATION_GET_TYPES as u32,
            NameHash::member_id_from_name("getTypes")
        );
        assert_eq!(
            OPERATION_GET_DEPENDENCIES as u32,
            NameHash::member_id_from_name("getDependencies")
        );
        assert_eq!(OPERATION_GET_TYPES, 0x0182_52d3);
        assert_eq!(OPERATION_GET_DEPENDENCIES, 0x05aa_fb31);
    }

    // ------------------------------------------------------------------
    // The MUTABLE framing
    // ------------------------------------------------------------------

    /// The probe `tl-mutable-framing` of
    /// `docs/spec-coverage/dds-xtypes-1.3.gaps.md` §C.6: `04000000
    /// 00000000` is a `DHEADER` of 4 in front of an empty member list,
    /// and it used to be read as a sequence length of four — four
    /// `TK_NONE` identifiers out of eight zero octets, byte-identical on
    /// the way back, no error anywhere.
    ///
    /// With the framing of §7.6.3.3.3 in place the same octets are a
    /// member list of four bytes holding one word, and a word is not an
    /// `EMHEADER` plus its member: the stream is refused.
    #[test]
    fn a_dheader_of_four_is_not_a_sequence_length_of_four() {
        let bytes = [0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let outcome = GetTypesRequest::decode_from(&mut r);
        assert!(
            outcome.is_err(),
            "a DHEADER of 4 over a single word is not a member list: {outcome:?}"
        );
    }

    /// An empty `getTypes` request is a `DHEADER`, one `EMHEADER` with
    /// `LC = 5`, its reused sequence DHEADER of four, and count zero —
    /// sixteen octets, not four.
    #[test]
    fn an_empty_get_types_request_carries_its_member_header() {
        let mut w = crate::framing::xcdr2_writer();
        GetTypesRequest::default().encode_into(&mut w).unwrap();
        let bytes = w.finish();
        assert_eq!(bytes.len(), 16, "{bytes:02x?}");
        // DHEADER over the member list.
        assert_eq!(&bytes[0..4], &[12, 0, 0, 0]);
        // EMHEADER: member id in the low 28 bits, LC = 5 in bits 28..30.
        let emheader = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(emheader & 0x0FFF_FFFF, member_ids::TYPE_IDS);
        assert_eq!((emheader >> 28) & 0x7, 5);
        // NEXTINT is the nonprimitive sequence DHEADER, followed by count.
        assert_eq!(&bytes[8..12], &[4, 0, 0, 0]);
        assert_eq!(&bytes[12..16], &[0, 0, 0, 0]);
    }

    /// A member id §7.6.3.3.3 does not define is stepped over, which is
    /// what the clause means by "Implementers may add their own members
    /// to these MUTABLE types."
    #[test]
    fn an_unknown_member_id_is_skipped_rather_than_refused() {
        let mut w = crate::framing::xcdr2_writer();
        encode_appendable::<WireError>(&mut w, |w| {
            crate::framing::encode_mutable_member(
                w,
                NameHash::member_id_from_name("acme.com/extra1"),
                |w| {
                    w.write_i32(-1);
                    Ok(())
                },
            )?;
            encode_mutable_member(w, member_ids::TYPE_IDS, |w| {
                encode_seq(
                    w,
                    &[TypeIdentifier::Primitive(PrimitiveKind::Int64)],
                    |w, t| t.encode_into(w),
                )
            })
        })
        .unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = GetTypesRequest::decode_from(&mut r).unwrap();
        assert_eq!(
            decoded.type_ids,
            alloc::vec![TypeIdentifier::Primitive(PrimitiveKind::Int64)]
        );
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
    fn empty_get_types_request_roundtrips() {
        roundtrip_get_types_request(GetTypesRequest::default());
    }

    // ------------------------------------------------------------------
    // getTypes_Out carries both of its members
    // ------------------------------------------------------------------

    /// §7.6.3.3.3 gives `TypeLookup_getTypes_Out` two members, and
    /// §7.6.3.3.4.2 says what the second is for. The probe
    /// `tl-gettypes-out` fed eight zero octets — two empty sequences —
    /// and got four back.
    #[test]
    fn get_types_reply_carries_types_and_complete_to_minimal() {
        let reply = GetTypesReply {
            types: alloc::vec![
                pair(TypeObject::Minimal(sample_minimal())),
                pair(TypeObject::Complete(sample_complete())),
            ],
            complete_to_minimal: alloc::vec![TypeIdentifierPair {
                type_identifier1: TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(
                    [0xC0; 14]
                )),
                type_identifier2: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                    [0x11; 14]
                )),
            }],
        };
        let mut w = crate::framing::xcdr2_writer();
        reply.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = GetTypesReply::decode_from(&mut r).unwrap();
        assert_eq!(decoded, reply);
        assert_eq!(decoded.complete_to_minimal.len(), 1);
    }

    /// The reply carries the identifier the object was asked for under,
    /// not the object alone.
    #[test]
    fn a_reply_pair_names_the_identifier_it_answers() {
        let object = TypeObject::Minimal(sample_minimal());
        let p = pair(object.clone());
        assert_eq!(
            p.type_identifier,
            crate::hash::to_hashed_type_identifier(&object).unwrap()
        );
        let mut w = crate::framing::xcdr2_writer();
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(
            TypeIdentifierTypeObjectPair::decode_from(&mut r).unwrap(),
            p
        );
    }

    #[test]
    fn empty_get_types_reply_roundtrips() {
        let reply = GetTypesReply::default();
        let mut w = crate::framing::xcdr2_writer();
        reply.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = GetTypesReply::decode_from(&mut r).unwrap();
        assert_eq!(decoded, reply);
    }

    // ------------------------------------------------------------------
    // ContinuationPoint
    // ------------------------------------------------------------------

    #[test]
    fn continuation_point_roundtrip_and_max_len() {
        let cp = ContinuationPoint(alloc::vec![0x11, 0x22, 0x33]);
        let mut w = crate::framing::xcdr2_writer();
        cp.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(ContinuationPoint::decode_from(&mut r).unwrap(), cp);

        // MAX_LEN clamp
        let oversized = ContinuationPoint(alloc::vec![0xFF; 64]);
        let mut w = crate::framing::xcdr2_writer();
        oversized.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = ContinuationPoint::decode_from(&mut r).unwrap();
        assert_eq!(decoded.0.len(), ContinuationPoint::MAX_LEN);
    }

    #[test]
    fn continuation_point_too_large_on_decode_rejected() {
        let mut w = crate::framing::xcdr2_writer();
        w.write_u32(100); // > MAX_LEN=32
        w.write_bytes(&[0u8; 100]);
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let err = ContinuationPoint::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, TypeCodecError::UnknownTypeKind { .. }));
    }

    #[test]
    fn continuation_point_len_zero_encodes_to_four_zero_bytes() {
        let cp = ContinuationPoint(alloc::vec![]);
        let mut w = crate::framing::xcdr2_writer();
        cp.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        assert_eq!(bytes, alloc::vec![0, 0, 0, 0]);
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(ContinuationPoint::decode_from(&mut r).unwrap(), cp);
    }

    #[test]
    fn continuation_point_len_max_roundtrips() {
        let cp = ContinuationPoint(alloc::vec![0xAB; ContinuationPoint::MAX_LEN]);
        let mut w = crate::framing::xcdr2_writer();
        cp.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(ContinuationPoint::decode_from(&mut r).unwrap(), cp);
    }

    #[test]
    fn continuation_point_len_max_plus_one_on_decode_rejected() {
        let mut w = crate::framing::xcdr2_writer();
        w.write_u32((ContinuationPoint::MAX_LEN + 1) as u32);
        w.write_bytes(&[0u8; 33]);
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let err = ContinuationPoint::decode_from(&mut r).unwrap_err();
        assert!(matches!(err, TypeCodecError::UnknownTypeKind { .. }));
    }

    // ------------------------------------------------------------------
    // getTypeDependencies
    // ------------------------------------------------------------------

    #[test]
    fn get_type_dependencies_request_reply_roundtrip() {
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                [0xAA; 14]
            ))],
            continuation_point: ContinuationPoint::default(),
        };
        let mut w = crate::framing::xcdr2_writer();
        req.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
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
        let mut w = crate::framing::xcdr2_writer();
        reply.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(
            GetTypeDependenciesReply::decode_from(&mut r).unwrap(),
            reply
        );
    }

    #[test]
    fn empty_get_type_dependencies_request_roundtrips() {
        let req = GetTypeDependenciesRequest::default();
        let mut w = crate::framing::xcdr2_writer();
        req.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(
            GetTypeDependenciesRequest::decode_from(&mut r).unwrap(),
            req
        );
    }

    #[test]
    fn empty_get_type_dependencies_reply_roundtrips() {
        let reply = GetTypeDependenciesReply::default();
        let mut w = crate::framing::xcdr2_writer();
        reply.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(
            GetTypeDependenciesReply::decode_from(&mut r).unwrap(),
            reply
        );
    }

    #[test]
    fn get_type_dependencies_request_with_continuation_payload_roundtrips() {
        let req = GetTypeDependenciesRequest {
            type_ids: alloc::vec![],
            continuation_point: ContinuationPoint(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let mut w = crate::framing::xcdr2_writer();
        req.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(
            GetTypeDependenciesRequest::decode_from(&mut r).unwrap(),
            req
        );
    }

    // ------------------------------------------------------------------
    // The six types §7.6.3.3.3 declares and the tree used not to have
    // ------------------------------------------------------------------

    #[test]
    fn get_types_result_roundtrips_both_arms() {
        for value in [
            GetTypesResult::Ok(GetTypesReply {
                types: alloc::vec![pair(TypeObject::Minimal(sample_minimal()))],
                complete_to_minimal: alloc::vec![],
            }),
            GetTypesResult::Retcode(1),
        ] {
            let mut w = crate::framing::xcdr2_writer();
            value.encode_into(&mut w).unwrap();
            let bytes = w.finish();
            let mut r = crate::framing::xcdr2_reader(&bytes);
            assert_eq!(GetTypesResult::decode_from(&mut r).unwrap(), value);
        }
    }

    #[test]
    fn get_type_dependencies_result_roundtrips_both_arms() {
        for value in [
            GetTypeDependenciesResult::Ok(GetTypeDependenciesReply::default()),
            GetTypeDependenciesResult::Retcode(23),
        ] {
            let mut w = crate::framing::xcdr2_writer();
            value.encode_into(&mut w).unwrap();
            let bytes = w.finish();
            let mut r = crate::framing::xcdr2_reader(&bytes);
            assert_eq!(
                GetTypeDependenciesResult::decode_from(&mut r).unwrap(),
                value
            );
        }
    }

    #[test]
    fn type_lookup_call_roundtrips_both_operations() {
        for value in [
            TypeLookupCall::GetTypes(GetTypesRequest {
                type_ids: alloc::vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                    [0x07; 14]
                ))],
            }),
            TypeLookupCall::GetTypeDependencies(GetTypeDependenciesRequest::default()),
        ] {
            let mut w = crate::framing::xcdr2_writer();
            value.encode_into(&mut w).unwrap();
            let bytes = w.finish();
            // The discriminator sits right behind the DHEADER.
            assert_eq!(
                i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                value.operation()
            );
            let mut r = crate::framing::xcdr2_reader(&bytes);
            assert_eq!(TypeLookupCall::decode_from(&mut r).unwrap(), value);
        }
    }

    /// The union of §7.6.3.3.3 has two cases and no `default:` arm, so an
    /// operation hash that is neither is refused. Reading it as one of
    /// the two would hand the wrong parameters to the wrong operation
    /// and report nothing.
    #[test]
    fn an_unknown_operation_hash_is_refused() {
        let mut w = crate::framing::xcdr2_writer();
        encode_appendable::<WireError>(&mut w, |w| {
            w.write_i32(0x0BAD_F00D_u32 as i32);
            Ok(())
        })
        .unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert!(TypeLookupCall::decode_from(&mut r).is_err());
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert!(TypeLookupReturn::decode_from(&mut r).is_err());
    }

    #[test]
    fn type_lookup_return_roundtrips_both_operations() {
        for value in [
            TypeLookupReturn::GetTypes(GetTypesResult::default()),
            TypeLookupReturn::GetTypeDependencies(GetTypeDependenciesResult::Retcode(-1)),
        ] {
            let mut w = crate::framing::xcdr2_writer();
            value.encode_into(&mut w).unwrap();
            let bytes = w.finish();
            let mut r = crate::framing::xcdr2_reader(&bytes);
            assert_eq!(TypeLookupReturn::decode_from(&mut r).unwrap(), value);
        }
    }

    #[test]
    fn type_lookup_request_and_reply_roundtrip() {
        let header = RequestHeader {
            writer_guid: [0x5A; 16],
            sequence_number: 42,
            instance_name: service_instance_name(&[0xABu8; 12]),
        };
        let request = TypeLookupRequest {
            header: header.clone(),
            data: TypeLookupCall::GetTypes(GetTypesRequest::default()),
        };
        let mut w = crate::framing::xcdr2_writer();
        request.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(TypeLookupRequest::decode_from(&mut r).unwrap(), request);

        let reply = TypeLookupReplyMessage {
            header: ReplyHeader {
                writer_guid: header.writer_guid,
                sequence_number: header.sequence_number,
                remote_ex: 0,
            },
            ret: TypeLookupReturn::GetTypes(GetTypesResult::Ok(GetTypesReply::default())),
        };
        let mut w = crate::framing::xcdr2_writer();
        reply.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        assert_eq!(TypeLookupReplyMessage::decode_from(&mut r).unwrap(), reply);
    }

    /// §7.6.3.3.4: the prefix, then the GUID in lower-case hexadecimal
    /// with no `0x` ahead of it.
    #[test]
    fn the_service_instance_name_is_the_prefix_and_lowercase_hex() {
        let name =
            service_instance_name(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0, 0, 0, 0]);
        assert_eq!(name, "dds.builtin.TOS.0123456789abcdef00000000000001c1");
        assert!(!name.contains("0x"));
    }
}
