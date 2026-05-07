// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteAliasType (XTypes §7.3.4.4).

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AppliedBuiltinMemberAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq,
};
use crate::type_object::flags::{AliasMemberFlag, AliasTypeFlag};

/// CompleteAliasHeader = detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAliasHeader {
    /// Name + Annotationen.
    pub detail: CompleteTypeDetail,
}

/// CompleteAliasBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAliasBody {
    /// Flags auf dem Alias-Target.
    pub related_flags: AliasMemberFlag,
    /// Zieltyp.
    pub related_type: TypeIdentifier,
    /// Builtin-Annotations auf dem Alias-Body (selten).
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
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.alias_flags.0)?;
        self.header.detail.encode_into(w)?;
        w.write_u16(self.body.related_flags.0)?;
        self.body.related_type.encode_into(w)?;
        self.body.ann_builtin.encode_into(w)?;
        self.body.ann_custom.encode_into(w)
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let alias_flags = AliasTypeFlag(r.read_u16()?);
        let detail = CompleteTypeDetail::decode_from(r)?;
        let related_flags = AliasMemberFlag(r.read_u16()?);
        let related_type = TypeIdentifier::decode_from(r)?;
        let ann_builtin = AppliedBuiltinMemberAnnotations::decode_from(r)?;
        let ann_custom = OptionalAppliedAnnotationSeq::decode_from(r)?;
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
