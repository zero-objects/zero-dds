// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type/Member-Flags (XTypes 1.3 §7.3.4.5).
//!
//! Alle Flags sind 16-bit Bitmasks. Die Bits sind spec-definiert und
//! werden wire-format-identisch enkodiert.

/// Flags fuer einen Struct-Typ (StructTypeFlag §7.3.4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructTypeFlag(pub u16);

impl StructTypeFlag {
    /// Extensibility = FINAL (strikte Gleichheit bei Assignability).
    pub const IS_FINAL: u16 = 1 << 0;
    /// Extensibility = APPENDABLE (Prefix-Match erlaubt).
    pub const IS_APPENDABLE: u16 = 1 << 1;
    /// Extensibility = MUTABLE (ID-basiertes Matching).
    pub const IS_MUTABLE: u16 = 1 << 2;
    /// Als `@nested` markiert — nicht selbstaendig topic-fuehrig.
    pub const IS_NESTED: u16 = 1 << 3;
    /// Member-IDs werden aus Namen gehasht (`@autoid(HASH)`).
    pub const IS_AUTOID_HASH: u16 = 1 << 4;

    /// Leer.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Testet ob ein Bit gesetzt ist.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags fuer einen Struct-Member (StructMemberFlag §7.3.4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructMemberFlag(pub u16);

impl StructMemberFlag {
    /// Try-Construct-Bit 1 (§7.2.4.4.1).
    pub const TRY_CONSTRUCT1: u16 = 1 << 0;
    /// Try-Construct-Bit 2.
    pub const TRY_CONSTRUCT2: u16 = 1 << 1;
    /// `@external` — indirect storage.
    pub const IS_EXTERNAL: u16 = 1 << 2;
    /// `@optional`.
    pub const IS_OPTIONAL: u16 = 1 << 3;
    /// `@must_understand`.
    pub const IS_MUST_UNDERSTAND: u16 = 1 << 4;
    /// `@key`.
    pub const IS_KEY: u16 = 1 << 5;
    /// `@default` — hat Defaultwert (Unions).
    pub const IS_DEFAULT: u16 = 1 << 6;

    /// Leer.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Testet ein Bit.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags fuer einen Union-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionTypeFlag(pub u16);

/// Flags fuer einen Union-Member (§7.3.4.5 UnionMemberFlag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionMemberFlag(pub u16);

impl UnionMemberFlag {
    /// Default-Case der Union.
    pub const IS_DEFAULT: u16 = 1 << 6;
    /// Leer.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// Flags fuer einen Union-Discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnionDiscriminatorFlag(pub u16);

/// Flags fuer Enum-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumTypeFlag(pub u16);

impl EnumTypeFlag {
    /// `@ignore_literal_names` (XTypes 1.3 §7.2.4.4.7). Wenn gesetzt,
    /// duerfen Compat-Vergleiche zwischen Enum-Literalen den Name-Hash
    /// ignorieren und nur den Ordinalwert pruefen.
    pub const IGNORE_LITERAL_NAMES: u16 = 1 << 0;

    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Bit-Test.
    #[must_use]
    pub const fn has(self, bit: u16) -> bool {
        (self.0 & bit) != 0
    }
}

/// Flags fuer Enum-Literal (`@default_literal`-Bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnumLiteralFlag(pub u16);

impl EnumLiteralFlag {
    /// `@default_literal`.
    pub const IS_DEFAULT_LITERAL: u16 = 1 << 0;
}

/// Flags fuer Bitmask-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitmaskTypeFlag(pub u16);

/// Flags fuer Bitflag (Bitmask-Member).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitflagFlag(pub u16);

/// Flags fuer Collection (Sequence/Array/Map).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionTypeFlag(pub u16);

/// Flags fuer Collection-Element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionElementFlag(pub u16);

/// Flags fuer Alias-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AliasTypeFlag(pub u16);

/// Flags fuer Alias-Member (ungebraucht in Minimal, für Wire-Symmetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AliasMemberFlag(pub u16);

/// Flags fuer Annotation-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationTypeFlag(pub u16);

/// Flags fuer Annotation-Parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationParameterFlag(pub u16);

/// Flags fuer Bitset-Typ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitsetTypeFlag(pub u16);

/// Flags fuer Bitfield (Bitset-Member).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitfieldFlag(pub u16);
