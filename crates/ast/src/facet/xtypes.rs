// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes annotations. Spec §4.1.

use crate::facet::applied::AppliedAnnotation;
use crate::node::constant::ConstExprNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::Id;

/// Struct extensibility. DDS-XTypes 1.3 §7.2.4.4.
///
/// Ordered so that it can sit in a layout key, which the plan cache
/// keys on: the order is the declaration order and carries no meaning
/// of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Extensibility {
    /// `@final` — no evolution.
    Final,
    /// `@appendable` — members may be appended.
    Appendable,
    /// `@mutable` — members carry ids and may be reordered.
    Mutable,
}

impl Extensibility {
    /// Every extensibility XTypes defines.
    pub const ALL: [Self; 3] = [Self::Final, Self::Appendable, Self::Mutable];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Appendable => "appendable",
            Self::Mutable => "mutable",
        }
    }

    /// Parse a configuration value, in the spelling [`Self::as_str`]
    /// produces.
    ///
    /// An unknown value returns `None` rather than falling back to a
    /// default: the setting decides whether a DHEADER goes on the wire,
    /// and a typo in it must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "final" => Some(Self::Final),
            "appendable" => Some(Self::Appendable),
            "mutable" => Some(Self::Mutable),
            _ => None,
        }
    }
}

/// `@try_construct` behaviour. DDS-XTypes 1.3 §7.2.2.7, §7.3.1.2.1.12.
///
/// What a receiver does when an object of the sender's type cannot
/// construct any object of the receiver's type — an over-long string, a
/// sequence with more elements than the bound holds, an enumerator the
/// receiver does not declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TryConstruct {
    /// The failure propagates: the containing collection or aggregate
    /// cannot be constructed either, and at the top level the sample is
    /// discarded.
    Discard,
    /// The element or member takes the default value of its type
    /// (Table 9) and the container is constructed.
    UseDefault,
    /// A string, wstring, sequence or map is truncated to what the
    /// receiver's bound holds, preserving order. On any other type the
    /// clause leaves the behaviour unspecified.
    Trim,
}

impl TryConstruct {
    /// Every behaviour XTypes defines.
    pub const ALL: [Self; 3] = [Self::Discard, Self::UseDefault, Self::Trim];

    /// What applies where nothing was written.
    ///
    /// §7.2.2.7: "The default behavior unless otherwise specified using
    /// the TryConstruct annotation is DISCARD." Note that this is the
    /// default for an *absent* annotation. The annotation's own
    /// parameter defaults to `USE_DEFAULT` (§7.3.1.2.1.12), so a bare
    /// `@try_construct` is not the same as no annotation at all.
    pub const ABSENT: Self = Self::Discard;

    /// What a bare `@try_construct`, written with no argument, means.
    ///
    /// §7.3.1.2.1.12 declares `TryConstructFailAction value default
    /// USE_DEFAULT` and spells it out: "If the annotation is specified
    /// without a value, or if the value is set to USE_DEFAULT, then the
    /// behavior is set to DEFAULT". Measured: OpenDDS constructs the
    /// empty string for `@try_construct string<5>` against a writer's
    /// twelve-character value —
    /// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`.
    pub const BARE: Self = Self::UseDefault;

    /// The spelling the annotation argument uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discard => "DISCARD",
            Self::UseDefault => "USE_DEFAULT",
            Self::Trim => "TRIM",
        }
    }

    /// Parse an annotation argument, in the spelling [`Self::as_str`]
    /// produces.
    ///
    /// An unknown value returns `None` rather than the default: the
    /// setting decides whether a sample survives, and a typo in it must
    /// be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|behaviour| behaviour.as_str() == value)
    }

    /// Whether `TRIM` is defined for this type kind.
    ///
    /// Table 11: "This option only applies to elements or members of
    /// type string, wide string, sequence, or map. The behavior when
    /// applied to other element/member types is unspecified and may be
    /// treated as an error."
    #[must_use]
    pub const fn trim_is_defined_for(kind: &crate::TypeKind) -> bool {
        matches!(
            kind,
            crate::TypeKind::String { .. }
                | crate::TypeKind::Sequence { .. }
                | crate::TypeKind::Map { .. }
        )
    }
}

/// Where an effective member id came from.
///
/// Needed alongside the value: layout uses the number, a collision
/// diagnostic uses the origin to point at the declaration responsible.
/// Spec §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberIdOrigin {
    /// `@id(n)` on the member itself.
    Declared,
    /// `@hashid` or `@hashid("name")`.
    Hashed {
        /// The name that was hashed, which is the member name unless an
        /// explicit string was given.
        source_name: String,
    },
    /// `@autoid(SEQUENTIAL)` on the enclosing aggregate.
    AutoSequential,
    /// `@autoid(HASH)` on the enclosing aggregate.
    AutoHash,
    /// The default rule: the previous member's id plus one.
    PreviousPlusOne,
}

impl MemberIdOrigin {
    /// Short name, for diagnostics.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Declared => "@id",
            Self::Hashed { .. } => "@hashid",
            Self::AutoSequential => "@autoid(SEQUENTIAL)",
            Self::AutoHash => "@autoid(HASH)",
            Self::PreviousPlusOne => "previous member + 1",
        }
    }
}

/// The effective member id together with where it came from. Spec §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberIdAssignment {
    /// The id the wire format uses.
    pub effective: u32,
    /// How that id was arrived at.
    pub origin: MemberIdOrigin,
}

/// Where `@verbatim` text is placed. IDL 4.2 §8.3.5.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerbatimPlacement {
    /// Start of the generated file.
    BeginFile,
    /// Immediately before the declaration.
    BeforeDeclaration,
    /// Inside the declaration, at its start.
    BeginDeclaration,
    /// Inside the declaration, at its end.
    EndDeclaration,
    /// Immediately after the declaration.
    AfterDeclaration,
    /// End of the generated file.
    EndFile,
}

impl VerbatimPlacement {
    /// Every placement IDL 4.2 defines.
    pub const ALL: [Self; 6] = [
        Self::BeginFile,
        Self::BeforeDeclaration,
        Self::BeginDeclaration,
        Self::EndDeclaration,
        Self::AfterDeclaration,
        Self::EndFile,
    ];
}

/// One `@verbatim` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimDirective {
    /// Target language, or `*` for all.
    pub language: String,
    /// Where the text goes.
    pub placement: VerbatimPlacement,
    /// The text itself.
    pub text: String,
}

/// `@range`, `@min` and `@max` combined. DDS-XTypes 1.3 Annex B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeConstraint {
    /// Lower bound expression.
    pub min: Option<Id<ConstExprNode>>,
    /// Upper bound expression.
    pub max: Option<Id<ConstExprNode>>,
}

/// XTypes information attached to a node. Spec §4.1.
///
/// Absent rather than defaulted when a node carries no XTypes
/// information: a node with no extensibility annotation must not be
/// taken to claim `Final`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XTypesFacet {
    /// `@final` / `@appendable` / `@mutable`.
    pub extensibility: Option<Extensibility>,
    /// `@key`.
    pub is_key: bool,
    /// `@optional`.
    pub is_optional: bool,
    /// `@external`.
    pub is_external: bool,
    /// `@must_understand`.
    pub is_must_understand: bool,
    /// The declaration spelled the name with the escape underscore —
    /// `_long` for `long`.
    ///
    /// IDL 4.2 §7.2.3.2 makes the underscore "a purely lexical
    /// convention" that is not part of the name, so the member name is
    /// stored stripped and this flag is the only trace of the spelling.
    /// One consumer: member-id hashing under
    /// `HashedNameSpelling::AsWritten`, the measured RTI Connext
    /// reading, which hashes `_long` and `_foo` as written — keyword or
    /// not (rtiddsgen 7.7.0, TypeCode member ids 79659193 for `_long`
    /// against 56906255 for `long`). §7.2.3 admits no identifier that
    /// starts with an underscore, so the spelling is always exactly
    /// `_` plus the stored name and a flag carries it whole.
    pub escaped: bool,
    /// `@non_serialized`.
    pub is_non_serialized: bool,
    /// `@nested`.
    pub is_nested: bool,
    /// `@external` written on the element of a sequence or array, or on
    /// the value of a map — `sequence<@external Foo, 42>`.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.4: "To declare the elements of a
    /// collection type external, apply the annotation to the collection
    /// declaration"; it is the `element_external` property of the Type
    /// System Model and the `IS_EXTERNAL` bit of the `CollectionElementFlag`
    /// of the collection's `TypeIdentifier`. Set on the collection's own
    /// node, like `element_try_construct`.
    pub element_is_external: bool,
    /// `@external` written on the key of a map —
    /// `map<@external Key, Value>`. The `IS_EXTERNAL` bit of the map
    /// identifier's `key_flags`.
    pub key_is_external: bool,
    /// `@nested` was written on this node, with whichever value.
    ///
    /// `@nested(FALSE)` on a type in a module annotated
    /// `@default_nested(TRUE)` has to be told from a type that says
    /// nothing (DDS-XTypes 1.3 §7.3.1.2.1.7): both have
    /// `is_nested == false`, and only the first one is not nested.
    pub nested_explicit: bool,
    /// `@default_literal` on an enumerator.
    pub is_default_literal: bool,
    /// `@try_construct` written on this node itself.
    ///
    /// On a member or a union case it is that member's behaviour. On an
    /// alias it is the behaviour a use of the alias takes when the use
    /// site says nothing — see
    /// [`crate::Ast::effective_try_construct`].
    pub try_construct: Option<TryConstruct>,
    /// `@try_construct` written on a collection's element.
    ///
    /// `sequence<@try_construct(TRIM) String5, 10>` is §7.3.1.2.1.12.4's
    /// own example. It lives on the collection and not on the element
    /// type, because the element type is shared and the behaviour is
    /// this collection's: two sequences of one `String5` may want
    /// different answers. For a `map` this is the **value**; the key has
    /// its own field.
    ///
    /// For an array declared as a member the annotation is written in
    /// front of the member's type, and Table 17's `ARRAY_TYPE` row knows
    /// no member-level behaviour at all — only "the TryConstruct
    /// behavior associated with T1 element type" — so an array member's
    /// annotation lands here rather than in `try_construct`.
    pub element_try_construct: Option<TryConstruct>,
    /// `@try_construct` written on a `map`'s key.
    ///
    /// Table 17 admits only `DISCARD` here: "If a key element of O2
    /// cannot construct the corresponding key type of T1 the entire map
    /// O2 cannot construct any object of type T1", with no TryConstruct
    /// escape, and `opendds_idl` refuses anything else outright. The
    /// field exists so that a declaration that writes `DISCARD`
    /// explicitly round-trips, and so the validation rule has something
    /// to read.
    pub key_try_construct: Option<TryConstruct>,
    /// `@default_nested` written on a module — DDS-XTypes 1.3
    /// §7.3.1.2.1.7.
    ///
    /// ```idl
    /// @annotation default_nested { boolean value default TRUE; };
    /// ```
    ///
    /// "Applied to a module ... the annotation indicates whether the
    /// types defined within it are nested by default. ... value defaults
    /// to that of the enclosing module. If a top-level module is not
    /// annotated, the default is FALSE."
    ///
    /// `None` is "the module says nothing", which is what makes the
    /// inheritance from the enclosing module work — a module annotated
    /// `@default_nested(FALSE)` inside one annotated
    /// `@default_nested(TRUE)` has to be distinguishable from one that
    /// was not annotated at all. [`crate::Ast::effective_nested`] walks
    /// the chain.
    pub default_nested: Option<bool>,
    /// `@key` written on a union's discriminator —
    /// `@try_construct` written on a union's discriminator.
    ///
    /// `union U switch (@try_construct K)` — §7.3.1.2.1.12.5. Table 19
    /// admits only `DEFAULT` for the discriminator: a discriminator is
    /// not a string, sequence or map, so `TRIM` is Table 11's
    /// unspecified case.
    pub discriminator_try_construct: Option<TryConstruct>,
    /// The user-declared annotations written in front of the element type
    /// of a collection — `sequence<@Ann long, 4>`, `map<string, @Ann
    /// long, 4>` — or after an array declarator's name, `long a @Ann [4]`.
    ///
    /// DDS-XTypes 1.3 §7.2.2.6: an annotation is attached to "an
    /// element/member/literal of an aggregated/collection/enumerated
    /// type". The collection is anonymous, so the annotations of its
    /// element sit on the collection node, beside the element's
    /// `@try_construct` and `@external`; they reach the `ann_custom` of
    /// `CompleteCollectionElement` and make the collection a non-plain
    /// type (§7.3.4.1).
    pub element_annotations: Vec<AppliedAnnotation>,
    /// The user-declared annotations written in front of a map's key
    /// type: `map<@Ann string, long, 4>`. As [`Self::element_annotations`],
    /// for the key.
    pub key_annotations: Vec<AppliedAnnotation>,
    /// The user-declared annotations written in front of a union's
    /// discriminator type: `union U switch (@Ann long)`.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.5: "Annotations can be applied to the
    /// implicit discriminator member of a union type by applying them to
    /// the discriminator type declaration in the header of the union
    /// type's definition". They reach the `ann_custom` of
    /// `CompleteDiscriminatorMember`.
    pub discriminator_annotations: Vec<AppliedAnnotation>,
    /// `@key` written on a union's discriminator.
    ///
    /// `union MyCommand switch (@key CommandKind)` — DDS-XTypes 1.3
    /// Table 21 lists `@key` against "Structure Members, Union
    /// discriminator" and §7.2.2.4.4.4.8 gives that declaration as its
    /// own example, so the clause permits it outright.
    ///
    /// It sits on the union rather than on a member because a
    /// discriminator is not a `MemberNode`: it is a type reference the
    /// union holds, and `is_key` on a member has nothing to attach to.
    /// A union that carries this is a keyed topic type whose key holder
    /// is the discriminator alone.
    ///
    /// **What reads it.** It reaches `UnionDiscriminatorFlag::IS_KEY` in
    /// the TypeObject, which is where Table 19's `UNION_TYPE` rule —
    /// "Either the discriminators of both T1 and T2 are keys or neither
    /// are keys" — can see it, and it reaches
    /// `zerodds_ast_layout::keyhash::union_key_discriminator`, which
    /// builds the key holder from the discriminator alone.
    ///
    /// Filled by the front end unless it was asked to refuse the
    /// annotation — `zerodds_idl_lower::KeyOnUnionDiscriminator::Refused`.
    /// The measured toolchains refuse the declaration (Cyclone's `idlc`,
    /// `rtiddsgen`) or generate no topic type for a union
    /// (`fastddsgen`), so a KeyHash over a union discriminator is one
    /// none of them computes; the clause permits it all the same.
    pub discriminator_is_key: bool,
    /// Effective member id and its origin.
    pub member_id: Option<MemberIdAssignment>,
    /// `@bit_bound`.
    pub bit_bound: Option<u16>,
    /// `@default`.
    pub default_value: Option<Id<ConstExprNode>>,
    /// `@range` / `@min` / `@max`.
    pub range: Option<RangeConstraint>,
    /// `@unit`.
    pub unit: Option<String>,
    /// `@verbatim`, in declaration order.
    pub verbatim: Vec<VerbatimDirective>,
    /// `@autoid` on an aggregate, which decides how its members without
    /// an `@id` are numbered.
    ///
    /// Kept alongside each member's own [`MemberIdOrigin`]: the origin
    /// says how one id was arrived at, this says what the declaration
    /// asked for, and a backend regenerating IDL needs the latter.
    pub autoid: Option<AutoId>,
    /// `@topic`.
    pub topic: Option<TopicAnnotation>,
    /// `@data_representation`, as the mask of allowed representations.
    pub data_representation: Option<DataRepresentationMask>,
    /// `@ignore_literal_names` on an enumeration.
    pub ignore_literal_names: bool,
}

/// `@autoid` policy. DDS-XTypes 1.3 §7.3.1.2.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoId {
    /// Ids follow declaration order.
    Sequential,
    /// Ids are the hash of the member name.
    Hash,
}

impl AutoId {
    /// Both policies XTypes defines.
    pub const ALL: [Self; 2] = [Self::Sequential, Self::Hash];

    /// Source spelling of the argument.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sequential => "SEQUENTIAL",
            Self::Hash => "HASH",
        }
    }
}

/// `@topic(name = "...", platform = "...")`. DDS-XTypes 1.3 Annex B.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicAnnotation {
    /// The topic name, when one was given.
    pub name: Option<String>,
    /// The platform the annotation applies to, `*` for all.
    pub platform: Option<String>,
}

/// The data representations a type allows.
///
/// A mask rather than one value: `@data_representation(XCDR1 | XCDR2)`
/// is legal and means both. DDS-XTypes 1.3 §7.6.3.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataRepresentationMask {
    /// `XCDR1` is allowed.
    pub xcdr1: bool,
    /// `XCDR2` is allowed.
    pub xcdr2: bool,
    /// `XML` is allowed.
    pub xml: bool,
}

impl DataRepresentationMask {
    /// The `DataRepresentationMask` value of a type that carries no
    /// `@data_representation`: "interpreted as if the
    /// DataRepresentationMask value was set to 0xffffffff" (DDS-XTypes
    /// 1.3 §7.3.1.2.1.15), which allows every representation.
    pub const ALL: Self = Self {
        xcdr1: true,
        xcdr2: true,
        xml: true,
    };

    /// The value `0xffffffff` of the annotation-less case, as the 32-bit
    /// mask of §7.3.1.2.1.15.
    pub const ALL_BITS: u32 = 0xffff_ffff;

    /// The positions of the `DataRepresentationMask` bitmask:
    /// `XCDR1` 0, `XML` 1, `XCDR2` 2 (§7.3.1.2.1.15).
    #[must_use]
    pub const fn bits(self) -> u32 {
        (self.xcdr1 as u32) | ((self.xml as u32) << 1) | ((self.xcdr2 as u32) << 2)
    }

    /// Whether the mask allows nothing, which no declaration should
    /// produce and a validation rule reports.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.xcdr1 && !self.xcdr2 && !self.xml
    }
}

impl XTypesFacet {
    /// The effective member id, if one has been assigned.
    #[must_use]
    pub fn effective_member_id(&self) -> Option<u32> {
        self.member_id.as_ref().map(|a| a.effective)
    }
}
