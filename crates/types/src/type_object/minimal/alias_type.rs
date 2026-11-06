// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalAliasType (XTypes §7.3.4.4.3).

use zerodds_wire::{Reader, WireError, Writer};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::flags::{AliasMemberFlag, AliasTypeFlag};

/// CommonAliasBody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonAliasBody {
    /// Flags on the alias target.
    pub related_flags: AliasMemberFlag,
    /// Target type.
    pub related_type: TypeIdentifier,
}

/// MinimalAliasBody = common + (no detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAliasBody {
    /// Common.
    pub common: CommonAliasBody,
}

impl MinimalAliasBody {
    /// `MinimalAliasBody` is `@appendable` (Annex B): a `DHEADER` in
    /// front of the `@final` `CommonAliasBody` under XCDR2.
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.common.related_flags.0);
            self.common.related_type.encode_into(w)
        })
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let related_flags = AliasMemberFlag(r.read_u16()?);
            let related_type = TypeIdentifier::decode_from(r)?;
            Ok(Self {
                common: CommonAliasBody {
                    related_flags,
                    related_type,
                },
            })
        })
    }
}

/// MinimalAliasType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAliasType {
    /// Flags.
    pub alias_flags: AliasTypeFlag,
    /// Header (empty in Minimal).
    /// Body.
    pub body: MinimalAliasBody,
}

impl MinimalAliasType {
    /// Encode.
    ///
    /// DDS-XTypes 1.3 Annex B (unchanged in 1.4 Beta 1): `MinimalAliasType`
    /// is `@final`, its empty `MinimalAliasHeader` and its
    /// `MinimalAliasBody` are `@appendable` — under XCDR2 the flags, a
    /// `DHEADER` of 0 and the body behind its own `DHEADER`. CycloneDDS
    /// 11.0.1 and Fast DDS 3.6.1 write and hash these octets
    /// (`tests/typeobject_bitmask_bitset_alias_vendors.rs`).
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.alias_flags.0);
        // MinimalAliasHeader: `@appendable` and empty, a DHEADER of 0.
        crate::framing::encode_appendable(w, |_| Ok::<_, WireError>(()))?;
        self.body.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let alias_flags = AliasTypeFlag(r.read_u16()?);
        // MinimalAliasHeader has no member yet; what a later revision
        // appends to it is skipped with its DHEADER.
        crate::framing::decode_appendable(r, |_| Ok(()))?;
        let body = MinimalAliasBody::decode_from(r)?;
        Ok(Self { alias_flags, body })
    }
}
