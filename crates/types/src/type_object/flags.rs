// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type/member flags (XTypes 1.3 §7.3.4.5).
//!
//! All flags are 16-bit bitmasks. The bits are spec-defined and are
//! encoded wire-format-identically.

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

/// Flags for an enum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumTypeFlag(pub u16);

impl EnumTypeFlag {
    /// `@ignore_literal_names` (XTypes 1.3 §7.2.4.4.7). When set,
    /// compatibility comparisons between enum literals may ignore the
    /// name hash and check only the ordinal value.
    pub const IGNORE_LITERAL_NAMES: u16 = 1 << 0;

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

/// Flags for an enum literal (`@default_literal` bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumLiteralFlag(pub u16);

impl EnumLiteralFlag {
    /// `@default_literal`.
    pub const IS_DEFAULT_LITERAL: u16 = 1 << 0;
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
