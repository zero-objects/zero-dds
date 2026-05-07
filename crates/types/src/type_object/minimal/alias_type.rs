// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalAliasType (XTypes §7.3.4.4.3).

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::flags::{AliasMemberFlag, AliasTypeFlag};

/// CommonAliasBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonAliasBody {
    /// Flags auf dem Alias-Target.
    pub related_flags: AliasMemberFlag,
    /// Ziel-Typ.
    pub related_type: TypeIdentifier,
}

/// MinimalAliasBody = common + (kein detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAliasBody {
    /// Common.
    pub common: CommonAliasBody,
}

impl MinimalAliasBody {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.common.related_flags.0)?;
        self.common.related_type.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let related_flags = AliasMemberFlag(r.read_u16()?);
        let related_type = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            common: CommonAliasBody {
                related_flags,
                related_type,
            },
        })
    }
}

/// MinimalAliasType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAliasType {
    /// Flags.
    pub alias_flags: AliasTypeFlag,
    /// Header (leer in Minimal).
    /// Body.
    pub body: MinimalAliasBody,
}

impl MinimalAliasType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.alias_flags.0)?;
        self.body.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let alias_flags = AliasTypeFlag(r.read_u16()?);
        let body = MinimalAliasBody::decode_from(r)?;
        Ok(Self { alias_flags, body })
    }
}
