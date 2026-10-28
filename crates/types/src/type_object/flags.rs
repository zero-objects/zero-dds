// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type/member flags (XTypes 1.3 §7.3.4.5).
//!
//! All flags are 16-bit bitmasks. The bits are spec-defined and are
//! encoded wire-format-identically.

/// `MemberFlagMinimalMask` (DDS-XTypes 1.3 Annex B, p. 280): "Mask used to
/// remove the flags that do no affect assignability". The value `0x003f`
/// keeps `TRY_CONSTRUCT1`, `TRY_CONSTRUCT2`, `IS_EXTERNAL`, `IS_OPTIONAL`,
/// `IS_MUST_UNDERSTAND` and `IS_KEY`; the comment beside it in Annex B
/// lists `D` (`IS_DEFAULT`, bit 6) among the flags it selects, which the
/// value does not. See [`minimal_member_flags`] and
/// [`minimal_union_member_flags`].
pub const MEMBER_FLAG_MINIMAL_MASK: u16 = 0x003f;

/// `TypeFlagMinimalMask` (Annex B, p. 281): selects `IS_FINAL`,
/// `IS_APPENDABLE` and `IS_MUTABLE`, and removes `IS_NESTED` and
/// `IS_AUTOID_HASH` from the type flags of a MINIMAL `TypeObject`.
pub const TYPE_FLAG_MINIMAL_MASK: u16 = 0x0007;

/// The member flags a `MinimalTypeObject` carries for a member whose
/// complete flags are `bits`: `bits & MemberFlagMinimalMask`.
#[must_use]
pub const fn minimal_member_flags(bits: u16) -> u16 {
    bits & MEMBER_FLAG_MINIMAL_MASK
}

/// The member flags of a union member in a `MinimalTypeObject`.
///
/// `MemberFlagMinimalMask` as a value (`0x003f`) leaves out
/// `IS_DEFAULT`; the comment next to it in Annex B names `D` as selected.
/// A minimal union member has no label for the `default` case
/// (`label_seq` is empty), so `IS_DEFAULT` is the only place the minimal
/// form says which member that is, and the assignability rules of Table 19
/// read it there. The flag is therefore kept for union members, in
/// agreement with the comment.
#[must_use]
pub const fn minimal_union_member_flags(bits: u16) -> u16 {
    bits & (MEMBER_FLAG_MINIMAL_MASK | UnionMemberFlag::IS_DEFAULT)
}

/// The type flags a `MinimalTypeObject` carries for a struct or union
/// whose complete flags are `bits`: `bits & TypeFlagMinimalMask`.
#[must_use]
pub const fn minimal_type_flags(bits: u16) -> u16 {
    bits & TYPE_FLAG_MINIMAL_MASK
}

/// Flags for a struct type (StructTypeFlag §7.3.4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructTypeFlag(pub u16);

impl StructTypeFlag {
    /// Extensibility = FINAL (strict equality for assignability).
    pub const IS_FINAL: u16 = 1 << 0;
    /// Extensibility = APPENDABLE (prefix match allowed).
    pub const IS_APPENDABLE: u16 = 1 << 1;
    /// Extensibility = MUTABLE (ID-based matching).
    pub const IS_MUTABLE: u16 = 1 << 2;
    /// Marked as `@nested` — not a standalone topic type.
    pub const IS_NESTED: u16 = 1 << 3;
    /// Member IDs are hashed from names (`@autoid(HASH)`).
    pub const IS_AUTOID_HASH: u16 = 1 << 4;

    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Tests whether a bit is set.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags for a struct member (StructMemberFlag §7.3.4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructMemberFlag(pub u16);

impl StructMemberFlag {
    /// Try-construct bit 1 (§7.2.4.4.1).
    pub const TRY_CONSTRUCT1: u16 = 1 << 0;
    /// Try-construct bit 2.
    pub const TRY_CONSTRUCT2: u16 = 1 << 1;
    /// `@external` — indirect storage.
    pub const IS_EXTERNAL: u16 = 1 << 2;
    /// `@optional`.
    pub const IS_OPTIONAL: u16 = 1 << 3;
    /// `@must_understand`.
    pub const IS_MUST_UNDERSTAND: u16 = 1 << 4;
    /// `@key`.
    pub const IS_KEY: u16 = 1 << 5;
    /// `@default` — has a default value (unions).
    pub const IS_DEFAULT: u16 = 1 << 6;

    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Tests a bit.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags for a union type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionTypeFlag(pub u16);

/// Flags for a union member (§7.3.4.5 UnionMemberFlag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionMemberFlag(pub u16);

impl UnionMemberFlag {
    /// TryConstruct = DISCARD (the default TryConstructKind for a member,
    /// §7.3.4.5). Set on every union member by the vendors (byte-verified
    /// against Cyclone + FastDDS).
    pub const TRY_CONSTRUCT1: u16 = 1 << 0;
    /// Default case of the union.
    pub const IS_DEFAULT: u16 = 1 << 6;
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// Flags for a union discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionDiscriminatorFlag(pub u16);

impl UnionDiscriminatorFlag {
    /// TryConstruct = DISCARD (default TryConstructKind, §7.3.4.5).
    pub const TRY_CONSTRUCT1: u16 = 1 << 0;
    /// The discriminator is always must-understand (§7.3.4.4.2): a reader
    /// that does not understand the discriminator cannot interpret the union.
    pub const IS_MUST_UNDERSTAND: u16 = 1 << 4;
    /// `@key` on the discriminator — `union U switch (@key K)`.
    ///
    /// The same bit position a struct member's `@key` takes: Annex B
    /// gives every member flag word the same bit layout
    /// (`@position(5) IS_KEY`), §7.3.4.5 gives one `MemberFlag` layout
    /// for every member kind, and the discriminator is a member
    /// position. Table 21 lists `@key` against "Structure Members, Union
    /// discriminator" and §7.2.2.4.4.4.8's own example is
    /// `union MyCommand switch (@key CommandKind)`.
    ///
    /// Table 19's `UNION_TYPE` row is the reader: "Either the
    /// discriminators of both T1 and T2 are keys or neither are keys."
    ///
    /// No measured implementation sets it, because none accepts the
    /// declaration that would: `idlc`, `rtiddsgen` and `fastddsgen` all
    /// refuse it. The front end carries the annotation unless it is
    /// asked to refuse it —
    /// `zerodds_idl_lower::KeyOnUnionDiscriminator::Refused`.
    pub const IS_KEY: u16 = 1 << 5;
    /// The default discriminator flags as emitted by Cyclone + FastDDS:
    /// `IS_MUST_UNDERSTAND | TRY_CONSTRUCT1` = 0x11 (byte-verified).
    #[must_use]
    pub const fn discriminator_default() -> Self {
        Self(Self::IS_MUST_UNDERSTAND | Self::TRY_CONSTRUCT1)
    }

    /// Tests a bit.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags for an enum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumTypeFlag(pub u16);

impl EnumTypeFlag {
    /// `@final`. Annex B calls the word unused; Table 18 compares the
    /// extensibility of two enumerations, and RTI, OpenDDS and Cyclone
    /// write the `TypeFlag` position for it (`docs/omg-xtypes-issues.md`
    /// §B8).
    pub const IS_FINAL: u16 = 1 << 0;
    /// `@appendable`, the default extensibility of an enumeration.
    pub const IS_APPENDABLE: u16 = 1 << 1;
    /// `@ignore_literal_names` (XTypes 1.3 §7.3.1.2.1.11). When set on
    /// both types, the assignability check compares literal values only.
    /// No position in Annex B; the first bit above the five `TypeFlag`
    /// positions so it cannot be read as one of them.
    pub const IGNORE_LITERAL_NAMES: u16 = 1 << 5;

    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Bit test.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags for an enum literal (`EnumeratedLiteralFlag`, Annex B: `D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumLiteralFlag(pub u16);

impl EnumLiteralFlag {
    /// The default literal: `MemberFlag::IS_DEFAULT`, `@position(6)`.
    /// Carried by the literal annotated `@default_literal`, else by the
    /// first literal (§7.2.2.4.1.1's default). Was written at bit 0 —
    /// `TRY_CONSTRUCT1` — until measured against RTI, OpenDDS and
    /// Cyclone, which all write bit 6 on the first literal.
    pub const IS_DEFAULT_LITERAL: u16 = 1 << 6;
}

/// Flags for a bitmask type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitmaskTypeFlag(pub u16);

/// Flags for a bitflag (bitmask member).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitflagFlag(pub u16);

/// Flags for a collection (sequence/array/map).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionTypeFlag(pub u16);

/// Flags for a collection element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionElementFlag(pub u16);

/// Flags for an alias type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AliasTypeFlag(pub u16);

/// Flags for an alias member (unused in minimal, for wire symmetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AliasMemberFlag(pub u16);

/// Flags for an annotation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationTypeFlag(pub u16);

/// Flags for an annotation parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationParameterFlag(pub u16);

/// Flags for a bitset type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitsetTypeFlag(pub u16);

/// Flags for a bitfield (bitset member).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitfieldFlag(pub u16);
