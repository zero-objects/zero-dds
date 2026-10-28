// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Programmatic builder for TypeObjects.
//!
//! Enables readable code like:
//!
//! ```no_run
//! use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
//! use zerodds_types::{PrimitiveKind, TypeIdentifier};
//!
//! let chatter = TypeObjectBuilder::struct_type("::chat::Chatter")
//!     .extensibility(Extensibility::Appendable)
//!     .member(
//!         "msg_id",
//!         TypeIdentifier::Primitive(PrimitiveKind::Int64),
//!         |m| m.key(),
//!     )
//!     .member(
//!         "text",
//!         TypeIdentifier::String8Small { bound: 255 },
//!         |m| m,
//!     )
//!     .build_complete();
//! ```
//!
//! Scope in T5: StructBuilder, EnumBuilder, AliasBuilder — the three
//! most frequently used top-level types. Union, collections,
//! bitmask, bitset, annotation follow on demand.

use alloc::string::String;
use alloc::vec::Vec;

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AnnotationParameterValue, AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations,
    CommonStructMember, CompleteMemberDetail, CompleteTypeDetail, NameHash,
    OptionalAppliedAnnotationSeq,
};
use crate::type_object::complete::{
    CompleteAliasBody, CompleteAliasHeader, CompleteAliasType, CompleteEnumeratedHeader,
    CompleteEnumeratedLiteral, CompleteEnumeratedType, CompleteStructHeader, CompleteStructMember,
    CompleteStructType,
};
use crate::type_object::flags::{
    AliasMemberFlag, AliasTypeFlag, EnumLiteralFlag, EnumTypeFlag, StructMemberFlag,
    StructTypeFlag, minimal_member_flags, minimal_type_flags, minimal_union_member_flags,
};
use crate::type_object::minimal::CommonAliasBody;
use crate::type_object::minimal::{
    CommonEnumeratedHeader, CommonEnumeratedLiteral, MinimalAliasBody, MinimalAliasType,
    MinimalEnumeratedHeader, MinimalEnumeratedLiteral, MinimalEnumeratedType, MinimalStructHeader,
    MinimalStructMember, MinimalStructType,
};

/// The `QualifiedTypeName` as serialized in a COMPLETE TypeObject
/// (§7.3.4.5.4): the fully-qualified name WITHOUT its leading `::` scope token.
/// CycloneDDS / FastDDS / RTI all omit it (byte-verified) — `::to::Plain`
/// serializes as `to::Plain`.
fn strip_leading_scope(name: &str) -> String {
    name.strip_prefix("::").unwrap_or(name).into()
}

/// Extensibility kind (§7.2.2.4), re-exported from the model.
///
/// Not declared again here. The frontend already decides what a
/// declaration's extensibility is — `@final` written, `@appendable`
/// defaulted, an `@autoid` aggregate inheriting one — and a second enum
/// with the same three variants would be a second answer to a question
/// that must have one. `zerodds_ast::Extensibility` is that answer and
/// this is a name for it.
pub use zerodds_ast::Extensibility;

/// The `StructTypeFlag` bit an extensibility takes on the wire.
///
/// The mapping and not the enum is what this crate owns: which of
/// `IS_FINAL`, `IS_APPENDABLE` and `IS_MUTABLE` a `TypeObject` carries
/// is a `TypeObject` question, and the model does not have a wire.
const fn extensibility_flag_bits(extensibility: Extensibility) -> u16 {
    match extensibility {
        Extensibility::Final => StructTypeFlag::IS_FINAL,
        Extensibility::Appendable => StructTypeFlag::IS_APPENDABLE,
        Extensibility::Mutable => StructTypeFlag::IS_MUTABLE,
    }
}

/// What a declaration with no extensibility annotation is taken to be.
///
/// §7.3.1.2.1.8's value, which is `@appendable`, and the same one
/// `zerodds_ast_layout::Dialect::Specification` takes. A vendor that
/// reads the absent annotation as `@final` — Cyclone does, and says so
/// in its own front end's output — reaches a different `TypeObject`, and
/// that difference is what GitHub issues #41 and #42 report.
pub const DEFAULT_EXTENSIBILITY: Extensibility = Extensibility::Appendable;

/// TryConstruct behaviour for a member (XTypes 1.3 §7.2.2.7 and
/// §7.3.1.2.1.12), re-exported from the model for the reason
/// [`Extensibility`] is.
pub use zerodds_ast::TryConstruct;

/// The two-bit `TRY_CONSTRUCT1`/`TRY_CONSTRUCT2` pattern (§7.3.1.2.1.1):
/// DISCARD = `TRY_CONSTRUCT1` (`01`), USE_DEFAULT = `TRY_CONSTRUCT2`
/// (`10`), TRIM = both (`11`). Matches the vendors' TypeObject
/// `member_flags` — byte-verified: Cyclone and Fast DDS set
/// `TRY_CONSTRUCT1` for DISCARD.
const fn try_construct_flag_bits(behaviour: TryConstruct) -> u16 {
    match behaviour {
        TryConstruct::Discard => StructMemberFlag::TRY_CONSTRUCT1,
        TryConstruct::UseDefault => StructMemberFlag::TRY_CONSTRUCT2,
        TryConstruct::Trim => StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2,
    }
}

/// Where a caller starts.
pub struct TypeObjectBuilder;

impl TypeObjectBuilder {
    /// Starts a struct builder with the given qualified
    /// name (e.g. "::sensors::Chatter").
    #[must_use]
    pub fn struct_type(name: impl Into<String>) -> StructBuilder {
        StructBuilder {
            name: name.into(),
            extensibility: DEFAULT_EXTENSIBILITY,
            nested: false,
            autoid_hash: false,
            base_type: TypeIdentifier::None,
            members: Vec::new(),
        }
    }

    /// Starts an enum builder.
    #[must_use]
    pub fn enum_type(name: impl Into<String>) -> EnumBuilder {
        EnumBuilder {
            name: name.into(),
            bit_bound: 32,
            extensibility: Extensibility::Appendable,
            ignore_literal_names: false,
            literals: Vec::new(),
        }
    }

    /// Starts an alias builder.
    #[must_use]
    pub fn alias(name: impl Into<String>, target: TypeIdentifier) -> AliasBuilder {
        AliasBuilder {
            name: name.into(),
            related_type: target,
        }
    }
}

// ============================================================================
// Struct
// ============================================================================

/// Builder for struct types.
pub struct StructBuilder {
    name: String,
    extensibility: Extensibility,
    nested: bool,
    autoid_hash: bool,
    base_type: TypeIdentifier,
    members: Vec<StructMemberSpec>,
}

/// Inner state of a struct member — set via [`StructMemberBuilder`].
pub struct StructMemberSpec {
    name: String,
    type_id: TypeIdentifier,
    explicit_id: Option<u32>,
    flags: u16,
    try_construct: TryConstruct,
    unit: Option<String>,
    min: Option<AnnotationParameterValue>,
    max: Option<AnnotationParameterValue>,
    hash_id: Option<String>,
    default_value: Option<String>,
}

/// Fluent builder for member attributes.
pub struct StructMemberBuilder<'a> {
    spec: &'a mut StructMemberSpec,
}

impl StructMemberBuilder<'_> {
    /// Marks a member as `@key`. A key member always has its "must
    /// understand" attribute set to true (DDS-XTypes 1.3 §7.2.2.4.4.4.8), as
    /// the lowering of IDL writes it.
    #[must_use]
    pub fn key(self) -> Self {
        self.spec.flags |= StructMemberFlag::IS_KEY | StructMemberFlag::IS_MUST_UNDERSTAND;
        self
    }

    /// `@optional`.
    #[must_use]
    pub fn optional(self) -> Self {
        self.spec.flags |= StructMemberFlag::IS_OPTIONAL;
        self
    }

    /// `@must_understand`.
    #[must_use]
    pub fn must_understand(self) -> Self {
        self.spec.flags |= StructMemberFlag::IS_MUST_UNDERSTAND;
        self
    }

    /// `@external` — indirect storage.
    #[must_use]
    pub fn external(self) -> Self {
        self.spec.flags |= StructMemberFlag::IS_EXTERNAL;
        self
    }

    /// Explizite `@id(n)`.
    #[must_use]
    pub fn id(self, id: u32) -> Self {
        self.spec.explicit_id = Some(id);
        self
    }

    /// `@unit("...")` — visible only in Complete.
    #[must_use]
    pub fn unit(self, unit: impl Into<String>) -> Self {
        self.spec.unit = Some(unit.into());
        self
    }

    /// `@min(val)` — visible only in Complete.
    #[must_use]
    pub fn min(self, min: AnnotationParameterValue) -> Self {
        self.spec.min = Some(min);
        self
    }

    /// `@max(val)` — visible only in Complete.
    #[must_use]
    pub fn max(self, max: AnnotationParameterValue) -> Self {
        self.spec.max = Some(max);
        self
    }

    /// `@hashid("name")`.
    #[must_use]
    pub fn hash_id(self, name: impl Into<String>) -> Self {
        self.spec.hash_id = Some(name.into());
        self
    }

    /// `@default(val)`, as the text of its literal. Carried in the complete
    /// TypeObject in memory via `AppliedBuiltinMemberAnnotations.default_value`;
    /// not written on the wire, which DDS-XTypes 1.3 gives no slot for.
    #[must_use]
    pub fn set_member_default(self, value: impl Into<String>) -> Self {
        self.spec.default_value = Some(value.into());
        self
    }

    /// `@try_construct(DISCARD|USE_DEFAULT|TRIM)` — XTypes 1.3 §7.2.2.4.4.4.4.
    /// Sets the two `TRY_CONSTRUCT1`/`TRY_CONSTRUCT2` member-flag bits. The
    /// default (unless this is called) is DISCARD.
    #[must_use]
    pub fn try_construct(self, kind: TryConstruct) -> Self {
        self.spec.try_construct = kind;
        self
    }
}

impl StructBuilder {
    /// Sets the extensibility (default: Appendable).
    #[must_use]
    pub fn extensibility(mut self, ext: Extensibility) -> Self {
        self.extensibility = ext;
        self
    }

    /// Marks as `@nested`.
    #[must_use]
    pub fn nested(mut self) -> Self {
        self.nested = true;
        self
    }

    /// Aktiviert `@autoid(HASH)`.
    #[must_use]
    pub fn autoid_hash(mut self) -> Self {
        self.autoid_hash = true;
        self
    }

    /// Sets a base type for inheritance.
    #[must_use]
    pub fn base(mut self, base: TypeIdentifier) -> Self {
        self.base_type = base;
        self
    }

    /// Adds a member.
    ///
    /// The callback receives a `StructMemberBuilder` to set flags
    /// and annotations.
    #[must_use]
    pub fn member<F>(mut self, name: impl Into<String>, ty: TypeIdentifier, f: F) -> Self
    where
        F: FnOnce(StructMemberBuilder<'_>) -> StructMemberBuilder<'_>,
    {
        let mut spec = StructMemberSpec {
            name: name.into(),
            type_id: ty,
            explicit_id: None,
            // Non-TryConstruct flag bits (key/optional/…) accumulate here; the
            // TryConstruct bits are held separately in `try_construct` and OR'd
            // in at `member_common`. Default TryConstructKind = DISCARD
            // (XTypes 1.3 §7.2.2.4.4.4.4 / §7.3.1.2.1.1 bits[0..2]=01), matching
            // the vendors' TypeObject member_flags.
            flags: 0,
            try_construct: TryConstruct::ABSENT,
            unit: None,
            min: None,
            max: None,
            hash_id: None,
            default_value: None,
        };
        let _ = f(StructMemberBuilder { spec: &mut spec });
        self.members.push(spec);
        self
    }

    fn struct_flags(&self) -> StructTypeFlag {
        let mut bits = extensibility_flag_bits(self.extensibility);
        if self.nested {
            bits |= StructTypeFlag::IS_NESTED;
        }
        if self.autoid_hash {
            bits |= StructTypeFlag::IS_AUTOID_HASH;
        }
        StructTypeFlag(bits)
    }

    /// Member-ID assignment (XTypes 1.3 §7.3.1.2.1), highest precedence first:
    /// - explicit `@id(n)`, if set
    /// - `@hashid("hint")` — per-member hash ID from the hint (or, for a bare
    ///   `@hashid`, the member name), `NameHash::member_id_from_name`
    /// - struct-level `@autoid(HASH)` — hash ID from the member name
    /// - otherwise the SEQUENTIAL fallback: the PREVIOUS member's resolved id + 1
    fn resolve_member_ids(&self) -> Vec<u32> {
        let mut ids = Vec::with_capacity(self.members.len());
        // XTypes 1.3 §7.3.1.2.1.1 / IDL 4.2 §7.4.15.4.2: the SEQUENTIAL fallback
        // is `previous member's RESOLVED id + 1` (the first member is 0). An
        // explicit `@id(n)` / `@hashid` / `@autoid(HASH)` shifts the run — the
        // next un-annotated member follows THAT id, e.g. `long a; @id(10) b;
        // octet c;` → a=0, b=10, c=11. Counting only un-annotated members (the
        // old behaviour) gave c=1, diverging from the data wire (idl-rust
        // `serialized_wire_ids`, idl-cpp `resolved_member_ids`, idl-java) and
        // from Cyclone/Fast-DDS. This is the SAME rule as
        // `zerodds_idl::semantics::member_id::resolve_member_id_sequence`
        // (that crate depends on this one, so the rule is inlined, not shared).
        let mut next_seq: u32 = 0;
        for spec in &self.members {
            let id = if let Some(explicit) = spec.explicit_id {
                explicit
            } else if let Some(hint) = &spec.hash_id {
                // `@hashid("hint")` (XTypes §7.3.1.2.1.4): the caller stores the
                // effective hint (an explicit argument, or the member name for a
                // bare `@hashid`); the ID is the canonical MD5-derived member ID.
                NameHash::member_id_from_name(hint)
            } else if self.autoid_hash {
                // `@autoid(HASH)` (XTypes §7.3.1.2.1.1): hash the member name.
                NameHash::member_id_from_name(&spec.name)
            } else {
                next_seq
            };
            // The next SEQUENTIAL member follows THIS member's resolved id.
            next_seq = id.wrapping_add(1);
            ids.push(id);
        }
        ids
    }

    fn member_common(spec: &StructMemberSpec, id: u32) -> CommonStructMember {
        CommonStructMember {
            member_id: id,
            // Combine the accumulated non-TryConstruct bits with the two
            // TryConstruct bits (§7.3.1.2.1.1).
            member_flags: StructMemberFlag(
                spec.flags | try_construct_flag_bits(spec.try_construct),
            ),
            member_type_id: spec.type_id.clone(),
        }
    }

    fn member_detail(spec: &StructMemberSpec) -> CompleteMemberDetail {
        CompleteMemberDetail {
            name: spec.name.clone(),
            ann_builtin: AppliedBuiltinMemberAnnotations {
                unit: spec.unit.clone(),
                min: spec.min.clone(),
                max: spec.max.clone(),
                hash_id: spec.hash_id.clone(),
                default_value: spec.default_value.clone(),
            },
            ann_custom: OptionalAppliedAnnotationSeq::default(),
        }
    }

    /// Builds a `MinimalStructType`.
    #[must_use]
    pub fn build_minimal(&self) -> MinimalStructType {
        let ids = self.resolve_member_ids();
        let member_seq = self
            .members
            .iter()
            .zip(ids.iter())
            .map(|(spec, id)| {
                let mut common = Self::member_common(spec, *id);
                common.member_flags = StructMemberFlag(minimal_member_flags(common.member_flags.0));
                MinimalStructMember {
                    common,
                    detail: NameHash::from_name(&spec.name),
                }
            })
            .collect();
        MinimalStructType {
            struct_flags: StructTypeFlag(minimal_type_flags(self.struct_flags().0)),
            header: MinimalStructHeader {
                base_type: self.base_type.clone(),
            },
            member_seq,
        }
    }

    /// Builds a `CompleteStructType`.
    #[must_use]
    pub fn build_complete(&self) -> CompleteStructType {
        let ids = self.resolve_member_ids();
        let member_seq = self
            .members
            .iter()
            .zip(ids.iter())
            .map(|(spec, id)| CompleteStructMember {
                common: Self::member_common(spec, *id),
                detail: Self::member_detail(spec),
            })
            .collect();
        CompleteStructType {
            struct_flags: self.struct_flags(),
            header: CompleteStructHeader {
                base_type: self.base_type.clone(),
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: strip_leading_scope(&self.name),
                },
            },
            member_seq,
        }
    }
}

// ============================================================================
// Enum
// ============================================================================

/// Builder for enumerated types.
pub struct EnumBuilder {
    name: String,
    bit_bound: u16,
    extensibility: Extensibility,
    ignore_literal_names: bool,
    literals: Vec<EnumLiteralSpec>,
}

struct EnumLiteralSpec {
    name: String,
    value: i32,
    is_default: bool,
}

impl EnumBuilder {
    /// Sets the bit width (8/16/32, default 32).
    #[must_use]
    pub fn bit_bound(mut self, bits: u16) -> Self {
        self.bit_bound = bits;
        self
    }

    /// Sets `EnumTypeFlag::IGNORE_LITERAL_NAMES` (`@ignore_literal_names`,
    /// XTypes 1.3 §7.3.1.2.1.11). Applies to both the minimal and complete
    /// TypeObject.
    #[must_use]
    pub fn ignore_literal_names(mut self, on: bool) -> Self {
        self.ignore_literal_names = on;
        self
    }

    /// Sets the extensibility (default `Appendable`, the IDL default).
    /// `Mutable` is not defined for an enumeration; it is written as
    /// given so a caller sees its own choice on the wire.
    #[must_use]
    pub fn extensibility(mut self, ext: Extensibility) -> Self {
        self.extensibility = ext;
        self
    }

    /// The enum-type flags: the extensibility position of `TypeFlag`
    /// (as RTI, OpenDDS and Cyclone write it) plus the
    /// `IGNORE_LITERAL_NAMES` bit.
    fn enum_flags(&self) -> EnumTypeFlag {
        let mut bits = match self.extensibility {
            Extensibility::Final => EnumTypeFlag::IS_FINAL,
            Extensibility::Appendable => EnumTypeFlag::IS_APPENDABLE,
            Extensibility::Mutable => StructTypeFlag::IS_MUTABLE,
        };
        if self.ignore_literal_names {
            bits |= EnumTypeFlag::IGNORE_LITERAL_NAMES;
        }
        EnumTypeFlag(bits)
    }

    /// The literal flags in declaration order: `IS_DEFAULT_LITERAL` on
    /// the literal added with `default_literal`, else on the first
    /// literal (§7.2.2.4.1.1).
    fn literal_flags(&self) -> impl Iterator<Item = EnumLiteralFlag> + '_ {
        let annotated = self.literals.iter().any(|l| l.is_default);
        self.literals.iter().enumerate().map(move |(index, l)| {
            let is_default = if annotated { l.is_default } else { index == 0 };
            EnumLiteralFlag(if is_default {
                EnumLiteralFlag::IS_DEFAULT_LITERAL
            } else {
                0
            })
        })
    }

    /// Adds a literal.
    #[must_use]
    pub fn literal(mut self, name: impl Into<String>, value: i32) -> Self {
        self.literals.push(EnumLiteralSpec {
            name: name.into(),
            value,
            is_default: false,
        });
        self
    }

    /// Adds the default literal (`@default_literal`).
    #[must_use]
    pub fn default_literal(mut self, name: impl Into<String>, value: i32) -> Self {
        self.literals.push(EnumLiteralSpec {
            name: name.into(),
            value,
            is_default: true,
        });
        self
    }

    /// Builds a `MinimalEnumeratedType`.
    #[must_use]
    pub fn build_minimal(&self) -> MinimalEnumeratedType {
        MinimalEnumeratedType {
            enum_flags: self.enum_flags(),
            header: MinimalEnumeratedHeader {
                common: CommonEnumeratedHeader {
                    bit_bound: self.bit_bound,
                },
            },
            literal_seq: {
                let mut sequence: Vec<MinimalEnumeratedLiteral> = self
                    .literals
                    .iter()
                    .zip(self.literal_flags())
                    .map(|(l, flags)| MinimalEnumeratedLiteral {
                        common: CommonEnumeratedLiteral {
                            value: l.value,
                            flags,
                        },
                        detail: NameHash::from_name(&l.name),
                    })
                    .collect();
                // §7.3.4.5: in increasing values of their numeric value.
                sequence.sort_by_key(|literal| literal.common.value);
                sequence
            },
        }
    }

    /// Builds a `CompleteEnumeratedType`.
    #[must_use]
    pub fn build_complete(&self) -> CompleteEnumeratedType {
        CompleteEnumeratedType {
            enum_flags: self.enum_flags(),
            header: CompleteEnumeratedHeader {
                common: CommonEnumeratedHeader {
                    bit_bound: self.bit_bound,
                },
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: strip_leading_scope(&self.name),
                },
            },
            literal_seq: {
                let mut sequence: Vec<CompleteEnumeratedLiteral> = self
                    .literals
                    .iter()
                    .zip(self.literal_flags())
                    .map(|(l, flags)| CompleteEnumeratedLiteral {
                        common: CommonEnumeratedLiteral {
                            value: l.value,
                            flags,
                        },
                        detail: CompleteMemberDetail {
                            name: l.name.clone(),
                            ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                            ann_custom: OptionalAppliedAnnotationSeq::default(),
                        },
                    })
                    .collect();
                // §7.3.4.5: in increasing values of their numeric value.
                sequence.sort_by_key(|literal| literal.common.value);
                sequence
            },
        }
    }
}

// ============================================================================
// Alias
// ============================================================================

/// Builder for alias/typedef.
pub struct AliasBuilder {
    name: String,
    related_type: TypeIdentifier,
}

impl AliasBuilder {
    /// Minimal-Alias.
    #[must_use]
    pub fn build_minimal(&self) -> MinimalAliasType {
        MinimalAliasType {
            alias_flags: AliasTypeFlag::default(),
            body: MinimalAliasBody {
                common: CommonAliasBody {
                    related_flags: AliasMemberFlag::default(),
                    related_type: self.related_type.clone(),
                },
            },
        }
    }

    /// Complete-Alias.
    #[must_use]
    pub fn build_complete(&self) -> CompleteAliasType {
        CompleteAliasType {
            alias_flags: AliasTypeFlag::default(),
            header: CompleteAliasHeader {
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: strip_leading_scope(&self.name),
                },
            },
            body: CompleteAliasBody {
                related_flags: AliasMemberFlag::default(),
                related_type: self.related_type.clone(),
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        }
    }
}

// ============================================================================
// Union
// ============================================================================

/// Builder for union types.
pub struct UnionBuilder {
    name: String,
    extensibility: Extensibility,
    discriminator_type: TypeIdentifier,
    cases: Vec<UnionCaseSpec>,
}

struct UnionCaseSpec {
    name: String,
    member_id: Option<u32>,
    type_id: TypeIdentifier,
    labels: Vec<i32>,
    is_default: bool,
}

impl UnionBuilder {
    /// Sets the extensibility (default Appendable).
    #[must_use]
    pub fn extensibility(mut self, ext: Extensibility) -> Self {
        self.extensibility = ext;
        self
    }

    /// Adds a case member.
    #[must_use]
    pub fn case(mut self, name: impl Into<String>, ty: TypeIdentifier, labels: Vec<i32>) -> Self {
        self.cases.push(UnionCaseSpec {
            name: name.into(),
            member_id: None,
            type_id: ty,
            labels,
            is_default: false,
        });
        self
    }

    /// Adds the default case.
    #[must_use]
    pub fn default_case(mut self, name: impl Into<String>, ty: TypeIdentifier) -> Self {
        self.cases.push(UnionCaseSpec {
            name: name.into(),
            member_id: None,
            type_id: ty,
            labels: Vec::new(),
            is_default: true,
        });
        self
    }

    fn union_flags(&self) -> crate::type_object::flags::UnionTypeFlag {
        use crate::type_object::flags::UnionTypeFlag;
        // UnionTypeFlag analogously uses the struct bits for
        // extensibility (§7.3.4.5) — reuse the bit positions.
        UnionTypeFlag(extensibility_flag_bits(self.extensibility))
    }

    fn resolve_case_ids(&self) -> Vec<u32> {
        let mut ids = Vec::with_capacity(self.cases.len());
        // Sequential @autoid is 0-based (§7.2.2.4.9) — byte-verified against
        // Cyclone + FastDDS union goldens.
        let mut next: u32 = 0;
        for c in &self.cases {
            ids.push(c.member_id.unwrap_or_else(|| {
                let v = next;
                next += 1;
                v
            }));
        }
        ids
    }

    /// Builds a `MinimalUnionType`.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalUnionType {
        use crate::type_object::common::CommonUnionMember;
        use crate::type_object::flags::{UnionDiscriminatorFlag, UnionMemberFlag};
        use crate::type_object::minimal::{
            CommonDiscriminatorMember, MinimalDiscriminatorMember, MinimalUnionMember,
            MinimalUnionType,
        };
        let ids = self.resolve_case_ids();
        let member_seq = self
            .cases
            .iter()
            .zip(ids.iter())
            .map(|(c, id)| MinimalUnionMember {
                common: CommonUnionMember {
                    member_id: *id,
                    member_flags: UnionMemberFlag(minimal_union_member_flags(
                        UnionMemberFlag::TRY_CONSTRUCT1
                            | if c.is_default {
                                UnionMemberFlag::IS_DEFAULT
                            } else {
                                0
                            },
                    )),
                    type_id: c.type_id.clone(),
                    label_seq: c.labels.clone(),
                },
                detail: NameHash::from_name(&c.name),
            })
            .collect();
        MinimalUnionType {
            union_flags: crate::type_object::flags::UnionTypeFlag(minimal_type_flags(
                self.union_flags().0,
            )),
            discriminator: MinimalDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag(minimal_member_flags(
                        UnionDiscriminatorFlag::discriminator_default().0,
                    )),
                    type_id: self.discriminator_type.clone(),
                },
            },
            member_seq,
        }
    }

    /// Builds a `CompleteUnionType`.
    #[must_use]
    pub fn build_complete(&self) -> crate::type_object::complete::CompleteUnionType {
        use crate::type_object::common::CommonUnionMember;
        use crate::type_object::complete::{
            CompleteDiscriminatorMember, CompleteUnionHeader, CompleteUnionMember,
            CompleteUnionType,
        };
        use crate::type_object::flags::{UnionDiscriminatorFlag, UnionMemberFlag};
        use crate::type_object::minimal::CommonDiscriminatorMember;
        let ids = self.resolve_case_ids();
        let member_seq = self
            .cases
            .iter()
            .zip(ids.iter())
            .map(|(c, id)| CompleteUnionMember {
                common: CommonUnionMember {
                    member_id: *id,
                    member_flags: UnionMemberFlag(
                        UnionMemberFlag::TRY_CONSTRUCT1
                            | if c.is_default {
                                UnionMemberFlag::IS_DEFAULT
                            } else {
                                0
                            },
                    ),
                    type_id: c.type_id.clone(),
                    label_seq: c.labels.clone(),
                },
                detail: CompleteMemberDetail {
                    name: c.name.clone(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            })
            .collect();
        CompleteUnionType {
            union_flags: self.union_flags(),
            header: CompleteUnionHeader {
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: strip_leading_scope(&self.name),
                },
            },
            discriminator: CompleteDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag::discriminator_default(),
                    type_id: self.discriminator_type.clone(),
                },
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
            member_seq,
        }
    }
}

// ============================================================================
// Sequence / Array / Map
// ============================================================================

/// Builder for `sequence<T, N>`.
pub struct SequenceBuilder {
    element: TypeIdentifier,
    bound: u32,
}

impl SequenceBuilder {
    /// Builds a MinimalSequenceType.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalSequenceType {
        use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
        use crate::type_object::minimal::{
            CommonCollectionElement, MinimalCollectionElement, MinimalSequenceType,
        };
        MinimalSequenceType {
            collection_flag: CollectionTypeFlag::default(),
            bound: self.bound,
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: self.element.clone(),
                },
            },
        }
    }
}

/// Builder for `T[D1, D2, ...]`.
pub struct ArrayBuilder {
    element: TypeIdentifier,
    dimensions: Vec<u32>,
}

impl ArrayBuilder {
    /// Builds a MinimalArrayType.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalArrayType {
        use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
        use crate::type_object::minimal::{
            CommonCollectionElement, MinimalArrayType, MinimalCollectionElement,
        };
        MinimalArrayType {
            collection_flag: CollectionTypeFlag::default(),
            bound_seq: self.dimensions.clone(),
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: self.element.clone(),
                },
            },
        }
    }
}

/// Validation error when building collection types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderError {
    /// MUTABLE extensibility is not allowed on maps
    /// (XTypes 1.3 §7.4.3.5.3 Rules 11-16). The spec allows only FINAL/
    /// APPENDABLE; MUTABLE has historically been silently treated as
    /// APPENDABLE. This error prevents the silent demotion.
    MutableMapExtensibilityNotAllowed,
}

/// Builder for `map<K, V, N>`.
pub struct MapBuilder {
    key: TypeIdentifier,
    value: TypeIdentifier,
    bound: u32,
}

impl MapBuilder {
    /// Validating builder: allows only FINAL/APPENDABLE extensibility.
    /// MUTABLE wirft `BuilderError::MutableMapExtensibilityNotAllowed`
    /// (XTypes 1.3 §7.4.3.5.3 Rules 11-16).
    ///
    /// # Errors
    /// `MutableMapExtensibilityNotAllowed` if `ext == Mutable`.
    pub fn add_map_member(
        self,
        ext: Extensibility,
    ) -> Result<crate::type_object::minimal::MinimalMapType, BuilderError> {
        if matches!(ext, Extensibility::Mutable) {
            return Err(BuilderError::MutableMapExtensibilityNotAllowed);
        }
        Ok(self.build_minimal())
    }

    /// Builds a MinimalMapType.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalMapType {
        use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
        use crate::type_object::minimal::{
            CommonCollectionElement, MinimalCollectionElement, MinimalMapType,
        };
        let element = MinimalCollectionElement {
            common: CommonCollectionElement {
                element_flags: CollectionElementFlag::default(),
                type_id: self.value.clone(),
            },
        };
        let key = MinimalCollectionElement {
            common: CommonCollectionElement {
                element_flags: CollectionElementFlag::default(),
                type_id: self.key.clone(),
            },
        };
        MinimalMapType {
            collection_flag: CollectionTypeFlag::default(),
            bound: self.bound,
            key,
            element,
        }
    }
}

// ============================================================================
// Bitmask / Bitset
// ============================================================================

/// Builder for `bitmask` types.
pub struct BitmaskBuilder {
    name: String,
    bit_bound: u16,
    flags: Vec<(String, u16)>,
}

impl BitmaskBuilder {
    /// Bit width (default 32).
    #[must_use]
    pub fn bit_bound(mut self, bits: u16) -> Self {
        self.bit_bound = bits;
        self
    }

    /// Adds a bit flag.
    #[must_use]
    pub fn flag(mut self, name: impl Into<String>, position: u16) -> Self {
        self.flags.push((name.into(), position));
        self
    }

    /// Builds a MinimalBitmaskType.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalBitmaskType {
        use crate::type_object::flags::{BitflagFlag, BitmaskTypeFlag};
        use crate::type_object::minimal::{CommonBitflag, MinimalBitflag, MinimalBitmaskType};
        MinimalBitmaskType {
            bitmask_flags: BitmaskTypeFlag::default(),
            bit_bound: self.bit_bound,
            flag_seq: {
                let mut sequence: Vec<MinimalBitflag> = self
                    .flags
                    .iter()
                    .map(|(n, p)| MinimalBitflag {
                        common: CommonBitflag {
                            position: *p,
                            flags: BitflagFlag::default(),
                        },
                        detail: NameHash::from_name(n),
                    })
                    .collect();
                // §7.3.4.5: in increasing values of their position.
                sequence.sort_by_key(|flag| flag.common.position);
                sequence
            },
        }
    }

    /// Builds a `CompleteBitmaskType` (with names + annotation placeholders).
    #[must_use]
    pub fn build_complete(&self) -> crate::type_object::complete::CompleteBitmaskType {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteBitflag, CompleteBitmaskType};
        use crate::type_object::flags::{BitflagFlag, BitmaskTypeFlag};
        CompleteBitmaskType {
            bitmask_flags: BitmaskTypeFlag::default(),
            bit_bound: self.bit_bound,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: strip_leading_scope(&self.name),
            },
            flag_seq: {
                let mut sequence: Vec<CompleteBitflag> = self
                    .flags
                    .iter()
                    .map(|(n, p)| CompleteBitflag {
                        common: crate::type_object::minimal::CommonBitflag {
                            position: *p,
                            flags: BitflagFlag::default(),
                        },
                        detail: CompleteMemberDetail {
                            name: n.clone(),
                            ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                            ann_custom: OptionalAppliedAnnotationSeq::default(),
                        },
                    })
                    .collect();
                // §7.3.4.5: in increasing values of their position.
                sequence.sort_by_key(|flag| flag.common.position);
                sequence
            },
        }
    }

    /// Getter — mostly used via `build_complete`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Builder for `bitset` types.
pub struct BitsetBuilder {
    name: String,
    fields: Vec<BitfieldSpec>,
}

struct BitfieldSpec {
    name: String,
    position: u16,
    bitcount: u8,
    holder_type: u8,
}

impl BitsetBuilder {
    /// Adds a bitfield.
    #[must_use]
    pub fn field(
        mut self,
        name: impl Into<String>,
        position: u16,
        bitcount: u8,
        holder_type: u8,
    ) -> Self {
        self.fields.push(BitfieldSpec {
            name: name.into(),
            position,
            bitcount,
            holder_type,
        });
        self
    }

    /// Builds a MinimalBitsetType.
    #[must_use]
    pub fn build_minimal(&self) -> crate::type_object::minimal::MinimalBitsetType {
        use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
        use crate::type_object::minimal::{CommonBitfield, MinimalBitfield, MinimalBitsetType};
        MinimalBitsetType {
            bitset_flags: BitsetTypeFlag::default(),
            field_seq: self
                .fields
                .iter()
                .map(|f| MinimalBitfield {
                    common: CommonBitfield {
                        position: f.position,
                        flags: BitfieldFlag::default(),
                        bitcount: f.bitcount,
                        holder_type: f.holder_type,
                    },
                    name_hash: NameHash::from_name(&f.name),
                })
                .collect(),
        }
    }

    /// Builds a `CompleteBitsetType` (with names + annotation placeholders).
    #[must_use]
    pub fn build_complete(&self) -> crate::type_object::complete::CompleteBitsetType {
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteBitfield, CompleteBitsetType};
        use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
        CompleteBitsetType {
            bitset_flags: BitsetTypeFlag::default(),
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: strip_leading_scope(&self.name),
            },
            field_seq: self
                .fields
                .iter()
                .map(|f| CompleteBitfield {
                    common: crate::type_object::minimal::CommonBitfield {
                        position: f.position,
                        flags: BitfieldFlag::default(),
                        bitcount: f.bitcount,
                        holder_type: f.holder_type,
                    },
                    detail: CompleteMemberDetail {
                        name: f.name.clone(),
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                })
                .collect(),
        }
    }

    /// Getter — mostly used via `build_complete`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ============================================================================
// Entry points for the new builders
// ============================================================================

impl TypeObjectBuilder {
    /// Starts a union builder. `discriminator_type` is typically
    /// an enum or an integer primitive.
    #[must_use]
    pub fn union_type(name: impl Into<String>, discriminator_type: TypeIdentifier) -> UnionBuilder {
        UnionBuilder {
            name: name.into(),
            extensibility: DEFAULT_EXTENSIBILITY,
            discriminator_type,
            cases: Vec::new(),
        }
    }

    /// Starts a sequence builder. `bound=0` = unbounded.
    #[must_use]
    pub fn sequence(element: TypeIdentifier, bound: u32) -> SequenceBuilder {
        SequenceBuilder { element, bound }
    }

    /// Starts an array builder with the given list of dimensions.
    #[must_use]
    pub fn array(element: TypeIdentifier, dimensions: Vec<u32>) -> ArrayBuilder {
        ArrayBuilder {
            element,
            dimensions,
        }
    }

    /// Starts a map builder.
    #[must_use]
    pub fn map(key: TypeIdentifier, value: TypeIdentifier, bound: u32) -> MapBuilder {
        MapBuilder { key, value, bound }
    }

    /// Starts a bitmask builder.
    #[must_use]
    pub fn bitmask(name: impl Into<String>) -> BitmaskBuilder {
        BitmaskBuilder {
            name: name.into(),
            bit_bound: 32,
            flags: Vec::new(),
        }
    }

    /// Starts a bitset builder.
    #[must_use]
    pub fn bitset(name: impl Into<String>) -> BitsetBuilder {
        BitsetBuilder {
            name: name.into(),
            fields: Vec::new(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::type_identifier::PrimitiveKind;
    use crate::type_object::flags::StructTypeFlag;

    #[test]
    fn struct_builder_basic_with_two_members() {
        let st = TypeObjectBuilder::struct_type("::chat::Chatter")
            .member(
                "sensor_id",
                TypeIdentifier::Primitive(PrimitiveKind::Int64),
                |m| m.key(),
            )
            .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m)
            .build_minimal();

        assert_eq!(st.member_seq.len(), 2);
        // Sequential @autoid is 0-based (XTypes §7.2.2.4.9): first member = 0.
        assert_eq!(st.member_seq[0].common.member_id, 0);
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(StructMemberFlag::IS_KEY)
        );
        assert_eq!(st.member_seq[1].common.member_id, 1);
        // NameHash deterministisch: MD5("sensor_id")[0..4]
        assert_eq!(st.member_seq[0].detail, NameHash::from_name("sensor_id"));
        assert_eq!(st.member_seq[1].detail, NameHash::from_name("text"));
    }

    #[test]
    fn member_default_try_construct_is_discard_bits() {
        // No @try_construct → DISCARD = TRY_CONSTRUCT1 (0b01), byte-identical to
        // the previous hard default. Combines with @key (bit 5).
        let st = TypeObjectBuilder::struct_type("::T")
            .member("a", TypeIdentifier::String8Small { bound: 4 }, |m| m.key())
            .build_minimal();
        let flags = st.member_seq[0].common.member_flags.0;
        assert_eq!(
            flags & StructMemberFlag::TRY_CONSTRUCT1,
            StructMemberFlag::TRY_CONSTRUCT1
        );
        assert_eq!(flags & StructMemberFlag::TRY_CONSTRUCT2, 0);
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(StructMemberFlag::IS_KEY)
        );
    }

    #[test]
    fn member_use_default_sets_try_construct2_bit() {
        // @try_construct(USE_DEFAULT) → TRY_CONSTRUCT2 alone (0b10).
        let st = TypeObjectBuilder::struct_type("::T")
            .member("a", TypeIdentifier::String8Small { bound: 4 }, |m| {
                m.try_construct(TryConstruct::UseDefault)
            })
            .build_complete();
        let flags = st.member_seq[0].common.member_flags.0;
        assert_eq!(flags & StructMemberFlag::TRY_CONSTRUCT1, 0);
        assert_eq!(
            flags & StructMemberFlag::TRY_CONSTRUCT2,
            StructMemberFlag::TRY_CONSTRUCT2
        );
    }

    #[test]
    fn member_trim_sets_both_try_construct_bits() {
        // @try_construct(TRIM) → TRY_CONSTRUCT1 | TRY_CONSTRUCT2 (0b11).
        let st = TypeObjectBuilder::struct_type("::T")
            .member("a", TypeIdentifier::String8Small { bound: 4 }, |m| {
                m.try_construct(TryConstruct::Trim)
            })
            .build_minimal();
        let flags = st.member_seq[0].common.member_flags.0;
        assert_eq!(
            flags & (StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2),
            StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2
        );
    }

    #[test]
    fn struct_builder_explicit_ids_respected() {
        let st = TypeObjectBuilder::struct_type("::X")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(100)
            })
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(200)
            })
            .build_minimal();
        assert_eq!(st.member_seq[0].common.member_id, 100);
        assert_eq!(st.member_seq[1].common.member_id, 200);
    }

    #[test]
    fn mutable_map_extensibility_is_error() {
        // §7.4.3.5.3 Rules 11-16 — a MUTABLE map is forbidden (silent demotion).
        let res = TypeObjectBuilder::map(
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            10,
        )
        .add_map_member(Extensibility::Mutable);
        assert!(matches!(
            res,
            Err(BuilderError::MutableMapExtensibilityNotAllowed)
        ));
    }

    #[test]
    fn appendable_map_extensibility_is_ok() {
        let res = TypeObjectBuilder::map(
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            10,
        )
        .add_map_member(Extensibility::Appendable);
        assert!(res.is_ok());
    }

    #[test]
    fn final_map_extensibility_is_ok() {
        let res = TypeObjectBuilder::map(
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            10,
        )
        .add_map_member(Extensibility::Final);
        assert!(res.is_ok());
    }

    #[test]
    fn member_with_explicit_default_used_when_field_missing() {
        // `set_member_default` sets
        // `AppliedBuiltinMemberAnnotations.default_value`. The test ensures
        // that the value reaches the complete TypeObject the builder returns.
        let st = TypeObjectBuilder::struct_type("::S")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.set_member_default("42")
            })
            .build_complete();
        assert_eq!(
            st.member_seq[0].detail.ann_builtin.default_value.as_deref(),
            Some("42")
        );
    }

    #[test]
    fn default_overrides_implicit_zero() {
        // Encoder-side hint: an explicit `@default("99")` must appear in the
        // complete TypeObject — the decoder uses it
        // when the field is missing for optional-mutable members, instead of the
        // type-implicit zero.
        let st = TypeObjectBuilder::struct_type("::S")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.set_member_default("99")
            })
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete();
        assert_eq!(
            st.member_seq[0].detail.ann_builtin.default_value.as_deref(),
            Some("99")
        );
        assert_eq!(st.member_seq[1].detail.ann_builtin.default_value, None);
    }

    #[test]
    fn default_value_stays_off_the_wire() {
        // DDS-XTypes 1.3 has no slot for `@default` in
        // `AppliedBuiltinMemberAnnotations`; the four members it has
        // round-trip, the default does not travel.
        use crate::type_object::common::AppliedBuiltinMemberAnnotations;
        let ann = AppliedBuiltinMemberAnnotations {
            unit: Some("meters".into()),
            min: Some(AnnotationParameterValue::Int32(0)),
            max: Some(AnnotationParameterValue::Int32(100)),
            hash_id: None,
            default_value: Some("17".into()),
        };
        let mut w = crate::framing::xcdr2_writer();
        ann.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = AppliedBuiltinMemberAnnotations::decode_from(&mut r).unwrap();
        assert_eq!(
            decoded,
            AppliedBuiltinMemberAnnotations {
                default_value: None,
                ..ann
            }
        );
    }

    #[test]
    fn struct_builder_extensibility_mutable() {
        let st = TypeObjectBuilder::struct_type("::Y")
            .extensibility(Extensibility::Mutable)
            .build_minimal();
        assert!(st.struct_flags.has(StructTypeFlag::IS_MUTABLE));
        assert!(!st.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
    }

    #[test]
    fn complete_builder_preserves_names() {
        let st = TypeObjectBuilder::struct_type("::sensors::Chatter")
            .extensibility(Extensibility::Appendable)
            .member(
                "sensor_id",
                TypeIdentifier::Primitive(PrimitiveKind::Int64),
                |m| m.key().unit("celsius"),
            )
            .build_complete();
        assert_eq!(st.header.detail.type_name, "sensors::Chatter"); // §7.3.4.5.4: no leading ::
        assert_eq!(st.member_seq[0].detail.name, "sensor_id");
        assert_eq!(
            st.member_seq[0].detail.ann_builtin.unit.as_deref(),
            Some("celsius")
        );
    }

    #[test]
    fn struct_builder_autoid_hash_collision_free_for_distinct_names() {
        let st = TypeObjectBuilder::struct_type("::H")
            .autoid_hash()
            .member(
                "alpha",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m,
            )
            .member(
                "beta",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m,
            )
            .build_minimal();
        // TypeFlagMinimalMask (Annex B) leaves IS_AUTOID_HASH out of the
        // minimal form; the complete one carries it.
        assert!(!st.struct_flags.has(StructTypeFlag::IS_AUTOID_HASH));
        assert_ne!(
            st.member_seq[0].common.member_id,
            st.member_seq[1].common.member_id
        );
    }

    #[test]
    fn member_id_from_name_matches_known_md5_vectors() {
        // XTypes §7.3.1.2.1.1: id = MD5(name)[0..4] as LE u32, & 0x0FFFFFFF.
        // Independently computed (hashlib): "color" → 0x0FA5DD70,
        // "my_hint" → 0x026C50E0.
        assert_eq!(NameHash::member_id_from_name("color"), 0x0FA5_DD70);
        assert_eq!(NameHash::member_id_from_name("my_hint"), 0x026C_50E0);
        // Top 4 bits are always clear (28-bit member-ID space).
        assert_eq!(NameHash::member_id_from_name("anything") & 0xF000_0000, 0);
    }

    #[test]
    fn autoid_hash_member_id_uses_central_derivation() {
        // @autoid(HASH) member "color" → member_id_from_name("color").
        let st = TypeObjectBuilder::struct_type("::H")
            .autoid_hash()
            .member(
                "color",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m,
            )
            .build_minimal();
        assert_eq!(
            st.member_seq[0].common.member_id,
            NameHash::member_id_from_name("color")
        );
    }

    #[test]
    fn hashid_hint_overrides_positional_member_id() {
        // A per-member @hashid("my_hint") derives its id from the hint, taking
        // precedence over the sequential 0-based default.
        let st = TypeObjectBuilder::struct_type("::H")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member("v", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.hash_id("my_hint")
            })
            .build_minimal();
        assert_eq!(st.member_seq[0].common.member_id, 0); // sequential
        assert_eq!(
            st.member_seq[1].common.member_id,
            NameHash::member_id_from_name("my_hint")
        );
    }

    #[test]
    fn explicit_id_wins_over_hashid() {
        // @id(9) beats @hashid on the same member (highest precedence).
        let st = TypeObjectBuilder::struct_type("::H")
            .member("v", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.hash_id("my_hint").id(9)
            })
            .build_minimal();
        assert_eq!(st.member_seq[0].common.member_id, 9);
    }

    #[test]
    fn enum_builder_roundtrip_ready() {
        let e = TypeObjectBuilder::enum_type("::Color")
            .bit_bound(16)
            .default_literal("RED", 0)
            .literal("GREEN", 1)
            .literal("BLUE", 2)
            .build_minimal();
        assert_eq!(e.header.common.bit_bound, 16);
        assert_eq!(e.literal_seq.len(), 3);
        assert!(e.literal_seq[0].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0);
    }

    /// RTI Connext 7.7.0 (`--print-typeid`), OpenDDS
    /// `typeobject_generator.cpp` and Cyclone `descriptor_type_meta.c`
    /// agree on the two bits: the `TypeFlag` extensibility position in
    /// `enum_flags`, and `IS_DEFAULT` (bit 6) on the first literal of an
    /// unannotated enumeration. Fast DDS writes neither. The complete
    /// hash of `::Test::E1` from the dds-xtypes suite as RTI prints it:
    /// `2144825d4dd81e0d3bf8fbbaf2a9`.
    #[test]
    fn an_unannotated_enum_carries_the_bits_rti_recomputes() {
        let e = TypeObjectBuilder::enum_type("::Test::E1")
            .literal("VAL0", 0)
            .literal("VAL1", 1)
            .literal("VAL2", 2)
            .build_complete();
        assert_eq!(e.enum_flags, EnumTypeFlag(EnumTypeFlag::IS_APPENDABLE));
        assert_eq!(e.literal_seq[0].common.flags.0, 1 << 6);
        assert_eq!(e.literal_seq[1].common.flags.0, 0);
        let hash = crate::compute_hash(&crate::TypeObject::Complete(
            crate::type_object::complete::CompleteTypeObject::Enumerated(e),
        ))
        .unwrap();
        assert_eq!(
            hash.0.to_vec(),
            [
                0x21, 0x44, 0x82, 0x5d, 0x4d, 0xd8, 0x1e, 0x0d, 0x3b, 0xf8, 0xfb, 0xba, 0xf2, 0xa9
            ]
        );
        let f = TypeObjectBuilder::enum_type("::Test::E3")
            .extensibility(Extensibility::Final)
            .literal("B0", 0)
            .default_literal("B1", 1)
            .build_minimal();
        assert_eq!(f.enum_flags, EnumTypeFlag(EnumTypeFlag::IS_FINAL));
        assert_eq!(f.literal_seq[0].common.flags.0, 0);
        assert_eq!(f.literal_seq[1].common.flags.0, 1 << 6);
    }

    #[test]
    fn alias_builder_minimal_and_complete() {
        let a_min =
            TypeObjectBuilder::alias("::Count", TypeIdentifier::Primitive(PrimitiveKind::UInt64))
                .build_minimal();
        assert!(matches!(
            a_min.body.common.related_type,
            TypeIdentifier::Primitive(PrimitiveKind::UInt64)
        ));

        let a_cmp =
            TypeObjectBuilder::alias("::Count", TypeIdentifier::Primitive(PrimitiveKind::UInt64))
                .build_complete();
        assert_eq!(a_cmp.header.detail.type_name, "Count");
    }

    // ------------------------------------------------------------------
    // Neue Builder (T#2 — Union / Collections / Bitmask / Bitset)
    // ------------------------------------------------------------------

    #[test]
    fn union_builder_minimal_with_two_cases_and_default() {
        let u = TypeObjectBuilder::union_type(
            "::Shape",
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
        )
        .case(
            "circle",
            TypeIdentifier::Primitive(PrimitiveKind::Float64),
            alloc::vec![1, 2],
        )
        .default_case("other", TypeIdentifier::String8Small { bound: 64 })
        .build_minimal();
        assert_eq!(u.member_seq.len(), 2);
        assert_eq!(u.member_seq[0].common.label_seq, alloc::vec![1, 2]);
        // Default case has the IS_DEFAULT bit.
        assert!(
            u.member_seq[1].common.member_flags.0
                & crate::type_object::flags::UnionMemberFlag::IS_DEFAULT
                != 0
        );
    }

    #[test]
    fn union_builder_complete_preserves_names_and_extensibility() {
        let u = TypeObjectBuilder::union_type(
            "::Shape",
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
        )
        .extensibility(Extensibility::Mutable)
        .case(
            "a",
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            alloc::vec![1],
        )
        .build_complete();
        assert_eq!(u.header.detail.type_name, "Shape");
        assert_eq!(u.member_seq[0].detail.name, "a");
        assert_eq!(
            u.union_flags.0 & StructTypeFlag::IS_MUTABLE,
            StructTypeFlag::IS_MUTABLE
        );
    }

    #[test]
    fn sequence_builder_minimal() {
        let s = TypeObjectBuilder::sequence(TypeIdentifier::Primitive(PrimitiveKind::Int16), 100)
            .build_minimal();
        assert_eq!(s.bound, 100);
        assert!(matches!(
            s.element.common.type_id,
            TypeIdentifier::Primitive(PrimitiveKind::Int16)
        ));
    }

    #[test]
    fn array_builder_3d() {
        let a = TypeObjectBuilder::array(
            TypeIdentifier::Primitive(PrimitiveKind::Float32),
            alloc::vec![4, 4, 4],
        )
        .build_minimal();
        assert_eq!(a.bound_seq, alloc::vec![4, 4, 4]);
    }

    #[test]
    fn map_builder_string_to_int() {
        let m = TypeObjectBuilder::map(
            TypeIdentifier::String8Small { bound: 64 },
            TypeIdentifier::Primitive(PrimitiveKind::Int64),
            1_000,
        )
        .build_minimal();
        assert_eq!(m.bound, 1_000);
        assert!(matches!(
            m.key.common.type_id,
            TypeIdentifier::String8Small { bound: 64 }
        ));
    }

    #[test]
    fn bitmask_builder_three_flags() {
        let b = TypeObjectBuilder::bitmask("::Permissions")
            .bit_bound(32)
            .flag("READ", 0)
            .flag("WRITE", 1)
            .flag("EXEC", 2)
            .build_minimal();
        assert_eq!(b.bit_bound, 32);
        assert_eq!(b.flag_seq.len(), 3);
        assert_eq!(b.flag_seq[0].common.position, 0);
        assert_eq!(b.flag_seq[2].common.position, 2);
    }

    #[test]
    fn bitset_builder_two_fields() {
        let b = TypeObjectBuilder::bitset("::Packed")
            .field("header", 0, 4, 0x07) // TK_UINT32
            .field("body", 4, 28, 0x07)
            .build_minimal();
        assert_eq!(b.field_seq.len(), 2);
        assert_eq!(b.field_seq[0].common.bitcount, 4);
        assert_eq!(b.field_seq[1].common.position, 4);
    }

    #[test]
    fn bitmask_builder_complete_preserves_names() {
        let b = TypeObjectBuilder::bitmask("::Perm")
            .bit_bound(16)
            .flag("READ", 0)
            .flag("WRITE", 1)
            .build_complete();
        assert_eq!(b.detail.type_name, "Perm");
        assert_eq!(b.bit_bound, 16);
        assert_eq!(b.flag_seq.len(), 2);
        assert_eq!(b.flag_seq[0].detail.name, "READ");
        assert_eq!(b.flag_seq[1].detail.name, "WRITE");
    }

    #[test]
    fn bitset_builder_complete_preserves_names() {
        let b = TypeObjectBuilder::bitset("::Packed")
            .field("header", 0, 4, 0x07)
            .field("body", 4, 28, 0x07)
            .build_complete();
        assert_eq!(b.detail.type_name, "Packed");
        assert_eq!(b.field_seq.len(), 2);
        assert_eq!(b.field_seq[0].detail.name, "header");
        assert_eq!(b.field_seq[1].detail.name, "body");
    }
}
