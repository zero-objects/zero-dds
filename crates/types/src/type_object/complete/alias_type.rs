// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteAliasType (XTypes §7.3.4.4).

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AppliedBuiltinMemberAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
};
use crate::type_object::flags::{AliasMemberFlag, AliasTypeFlag};

/// CompleteAliasHeader = detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAliasHeader {
    /// Name + annotations.
    pub detail: CompleteTypeDetail,
}

/// CompleteAliasBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAliasBody {
    /// Flags on the alias target.
    pub related_flags: AliasMemberFlag,
    /// Zieltyp.
    pub related_type: TypeIdentifier,
    /// Builtin annotations on the alias body (rare).
    pub ann_builtin: AppliedBuiltinMemberAnnotations,
    /// Custom.
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

/// CompleteAliasType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAliasType {
    /// Flags.
    pub alias_flags: AliasTypeFlag,
    /// Header.
    pub header: CompleteAliasHeader,
    /// Body.
    pub body: CompleteAliasBody,
}

impl CompleteAliasType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.alias_flags.0);
        // CompleteAliasHeader is @appendable → DHEADER around the detail.
        crate::framing::encode_appendable(w, |w| self.header.detail.encode_into(w))?;
        // CompleteAliasBody is @appendable → DHEADER around common + the two
        // @optional annotation members 1 and 2 (ann_builtin is absent when
        // empty).
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.body.related_flags.0);
            self.body.related_type.encode_into(w)?;
            self.body.ann_builtin.encode_as_member(w, 1)?;
            self.body.ann_custom.encode_as_member(w, 2)
        })
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let alias_flags = AliasTypeFlag(r.read_u16()?);
        let detail = crate::framing::decode_appendable(r, CompleteTypeDetail::decode_from)?;
        let (related_flags, related_type, ann_builtin, ann_custom) =
            crate::framing::decode_appendable(r, |r| {
                let related_flags = AliasMemberFlag(r.read_u16()?);
                let related_type = TypeIdentifier::decode_from(r)?;
                let ann_builtin = AppliedBuiltinMemberAnnotations::decode_as_member(r, 1)?;
                let ann_custom = OptionalAppliedAnnotationSeq::decode_as_member(r, 2)?;
                Ok::<_, zerodds_wire::WireError>((
                    related_flags,
                    related_type,
                    ann_builtin,
                    ann_custom,
                ))
            })?;
        Ok(Self {
            alias_flags,
            header: CompleteAliasHeader { detail },
            body: CompleteAliasBody {
                related_flags,
                related_type,
                ann_builtin,
                ann_custom,
            },
        })
    }
}
