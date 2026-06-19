// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteUnionType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AppliedBuiltinTypeAnnotations, CommonUnionMember, CompleteMemberDetail, CompleteTypeDetail,
    OptionalAppliedAnnotationSeq, decode_seq, encode_seq,
};
use crate::type_object::flags::{UnionDiscriminatorFlag, UnionTypeFlag};
use crate::type_object::minimal::CommonDiscriminatorMember;

/// CompleteUnionHeader = detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteUnionHeader {
    /// Type detail.
    pub detail: CompleteTypeDetail,
}

/// CompleteDiscriminator — common + ann_builtin + ann_custom (no name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteDiscriminatorMember {
    /// Discriminator type/flags.
    pub common: CommonDiscriminatorMember,
    /// `@verbatim` etc.
    pub ann_builtin: AppliedBuiltinTypeAnnotations,
    /// Custom.
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

/// CompleteUnionMember = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteUnionMember {
    /// Common.
    pub common: CommonUnionMember,
    /// Name + annotations.
    pub detail: CompleteMemberDetail,
}

/// CompleteUnionType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteUnionType {
    /// Flags.
    pub union_flags: UnionTypeFlag,
    /// Header.
    pub header: CompleteUnionHeader,
    /// Discriminator.
    pub discriminator: CompleteDiscriminatorMember,
    /// Members.
    pub member_seq: Vec<CompleteUnionMember>,
}

impl CompleteUnionType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.union_flags.0)?;
        self.header.detail.encode_into(w)?;
        w.write_u16(self.discriminator.common.member_flags.0)?;
        self.discriminator.common.type_id.encode_into(w)?;
        self.discriminator.ann_builtin.encode_into(w)?;
        self.discriminator.ann_custom.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, m| {
            m.common.encode_into(w)?;
            m.detail.encode_into(w)
        })
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let union_flags = UnionTypeFlag(r.read_u16()?);
        let detail = CompleteTypeDetail::decode_from(r)?;
        let disc_flags = UnionDiscriminatorFlag(r.read_u16()?);
        let disc_type = TypeIdentifier::decode_from(r)?;
        let disc_ann_builtin = AppliedBuiltinTypeAnnotations::decode_from(r)?;
        let disc_ann_custom = OptionalAppliedAnnotationSeq::decode_from(r)?;
        let member_seq = decode_seq(r, |r| {
            let common = CommonUnionMember::decode_from(r)?;
            let detail = CompleteMemberDetail::decode_from(r)?;
            Ok(CompleteUnionMember { common, detail })
        })?;
        Ok(Self {
            union_flags,
            header: CompleteUnionHeader { detail },
            discriminator: CompleteDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: disc_flags,
                    type_id: disc_type,
                },
                ann_builtin: disc_ann_builtin,
                ann_custom: disc_ann_custom,
            },
            member_seq,
        })
    }
}
