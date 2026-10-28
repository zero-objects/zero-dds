// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Inheritance of Union Types — DDS-XTypes 1.3 §7.2.2.4.5.
//!
//! "A Union Type may inherit from another Union Type as long as the
//! following conditions are met:
//!
//! - The derived Union either has the extensibility kind unspecified, or
//!   else it has the same extensibility kind as the base type.
//! - The derived Union has the same discriminator type as the base Union.
//! - The derived Union discriminator does is not defined as key.
//! - The base Union, or any of its ancestors have not explicitly defined a
//!   'default' member of the Union.
//! - Any given discriminator value identifies at most one member of the
//!   derived Union or any of the base Union ancestor types. [...]
//!   Construct a hypothetical Union Type containing as members both the
//!   members of the derived Union and the members of all ancestor Unions.
//!   The resulting Union must be legal according to the rules in
//!   7.2.2.4.4.3 which require that a discriminator value can select at
//!   most one member."
//!
//! # What carries a base union
//!
//! None of the representations the specification defines has a place for
//! one: IDL 4.2 has no syntax for a union with a base, the XSD of §7.3.3
//! gives `baseType` to `structDecl` and `annotationTypeDecl` only, and
//! Annex B's `MinimalUnionHeader` and `CompleteUnionHeader` are empty (a
//! structure's header holds `base_type`). The conditions are rules of the
//! type system and are applied here to the objects that describe the
//! types, the ancestors listed from the root to the nearest base, so that
//! a representation that adds a base (the specification's own reading
//! of "extensibility kind unspecified" is a type whose
//! [`UnionTypeFlag`](crate::type_object::flags::UnionTypeFlag) holds none of `IS_FINAL`, `IS_APPENDABLE`,
//! `IS_MUTABLE`) can apply them, and so that the members "belonging to base
//! types" can be "considered expanded" (Table 19) with [`expand_union`].

use alloc::vec::Vec;
use core::fmt;

use crate::type_object::common::MemberId;
use crate::type_object::flags::{StructTypeFlag, UnionDiscriminatorFlag, UnionMemberFlag};
use crate::type_object::minimal::{MinimalUnionMember, MinimalUnionType};

/// Which condition of §7.2.2.4.5 a derived union breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnionInheritanceError {
    /// The derived union names an extensibility kind other than its base
    /// type's.
    ExtensibilityDiffers,
    /// The derived union has another discriminator type than its base.
    DiscriminatorTypeDiffers,
    /// The derived union's discriminator is a key.
    DiscriminatorIsKey,
    /// An ancestor defines a `default` member, so no derived union may add
    /// members: a value no label names would select two.
    AncestorHasDefaultMember {
        /// Position of the ancestor in the list, root first.
        ancestor: usize,
    },
    /// One discriminator value selects more than one member of the derived
    /// union and its ancestors.
    AmbiguousLabel {
        /// The value.
        label: i32,
    },
    /// Two members of the hypothetical union have one member id.
    DuplicateMemberId {
        /// The id.
        member_id: MemberId,
    },
    /// Two members of the hypothetical union have one name (minimal
    /// objects name a member by the hash of its name).
    DuplicateMemberName,
    /// More than one member of the hypothetical union is the default one.
    MoreThanOneDefaultMember,
}

impl fmt::Display for UnionInheritanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtensibilityDiffers => f.write_str(
                "the derived union names an extensibility kind other than its base type's \
                 (DDS-XTypes 1.3 §7.2.2.4.5)",
            ),
            Self::DiscriminatorTypeDiffers => f.write_str(
                "the derived union has another discriminator type than its base union \
                 (DDS-XTypes 1.3 §7.2.2.4.5)",
            ),
            Self::DiscriminatorIsKey => f.write_str(
                "the discriminator of a derived union is not defined as key \
                 (DDS-XTypes 1.3 §7.2.2.4.5)",
            ),
            Self::AncestorHasDefaultMember { ancestor } => write!(
                f,
                "ancestor {ancestor} explicitly defines a default member (DDS-XTypes 1.3 \
                 §7.2.2.4.5)"
            ),
            Self::AmbiguousLabel { label } => write!(
                f,
                "the discriminator value {label} identifies more than one member of the derived \
                 union and its ancestors (DDS-XTypes 1.3 §7.2.2.4.5, §7.2.2.4.4.3)"
            ),
            Self::DuplicateMemberId { member_id } => write!(
                f,
                "member id {member_id} is used twice in the derived union and its ancestors \
                 (DDS-XTypes 1.3 §7.2.2.4.4.3)"
            ),
            Self::DuplicateMemberName => f.write_str(
                "a member name is used twice in the derived union and its ancestors (DDS-XTypes \
                 1.3 §7.2.2.4.4.3)",
            ),
            Self::MoreThanOneDefaultMember => f.write_str(
                "more than one default member in the derived union and its ancestors (DDS-XTypes \
                 1.3 §7.2.2.4.4.3)",
            ),
        }
    }
}

/// The extensibility kind a union names, `None` for "unspecified".
fn named_extensibility(union: &MinimalUnionType) -> Option<u16> {
    let kinds =
        StructTypeFlag::IS_FINAL | StructTypeFlag::IS_APPENDABLE | StructTypeFlag::IS_MUTABLE;
    match union.union_flags.0 & kinds {
        0 => None,
        kind => Some(kind),
    }
}

fn is_default(member: &MinimalUnionMember) -> bool {
    member.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0
}

/// The members of the hypothetical union that holds the members of
/// `ancestors` (root first) and then those of `derived`.
///
/// §7.2.2.4.5 and Table 19: "members belonging to base types of T1 or T2
/// shall be considered 'expanded' inside T1 or T2 respectively, as if they
/// had been directly defined as part of the sub-type." The discriminator
/// and the flags are the derived union's.
#[must_use]
pub fn expand_union(
    ancestors: &[MinimalUnionType],
    derived: &MinimalUnionType,
) -> MinimalUnionType {
    let mut members: Vec<MinimalUnionMember> = Vec::new();
    for ancestor in ancestors {
        members.extend(ancestor.member_seq.iter().cloned());
    }
    members.extend(derived.member_seq.iter().cloned());
    MinimalUnionType {
        union_flags: derived.union_flags,
        discriminator: derived.discriminator.clone(),
        member_seq: members,
    }
}

/// Checks the conditions of §7.2.2.4.5 for `derived`, a union that
/// inherits from the last of `ancestors` (listed from the root to the
/// nearest base).
///
/// # Errors
/// The first condition `derived` breaks, in the order the clause lists
/// them.
pub fn check_union_derivation(
    ancestors: &[MinimalUnionType],
    derived: &MinimalUnionType,
) -> Result<(), UnionInheritanceError> {
    let Some(base) = ancestors.last() else {
        return Ok(());
    };
    // "either the extensibility kind unspecified, or else the same
    // extensibility kind as the base type".
    if let Some(kind) = named_extensibility(derived) {
        if named_extensibility(base) != Some(kind) {
            return Err(UnionInheritanceError::ExtensibilityDiffers);
        }
    }
    // "the same discriminator type as the base Union".
    if derived.discriminator.common.type_id != base.discriminator.common.type_id {
        return Err(UnionInheritanceError::DiscriminatorTypeDiffers);
    }
    // "The derived Union discriminator [...] is not defined as key."
    if derived.discriminator.common.member_flags.0 & UnionDiscriminatorFlag::IS_KEY != 0 {
        return Err(UnionInheritanceError::DiscriminatorIsKey);
    }
    // "The base Union, or any of its ancestors have not explicitly defined
    // a 'default' member".
    for (position, ancestor) in ancestors.iter().enumerate() {
        if ancestor.member_seq.iter().any(is_default) {
            return Err(UnionInheritanceError::AncestorHasDefaultMember { ancestor: position });
        }
    }
    // The hypothetical union is legal (§7.2.2.4.4.3): unique member ids
    // and names, at most one default member, and a discriminator value
    // selects at most one member.
    let expanded = expand_union(ancestors, derived);
    let members = &expanded.member_seq;
    for (at, member) in members.iter().enumerate() {
        for other in &members[..at] {
            if other.common.member_id == member.common.member_id {
                return Err(UnionInheritanceError::DuplicateMemberId {
                    member_id: member.common.member_id,
                });
            }
            if other.detail == member.detail {
                return Err(UnionInheritanceError::DuplicateMemberName);
            }
            for label in &member.common.label_seq {
                if other.common.label_seq.contains(label) {
                    return Err(UnionInheritanceError::AmbiguousLabel { label: *label });
                }
            }
        }
    }
    if members.iter().filter(|member| is_default(member)).count() > 1 {
        return Err(UnionInheritanceError::MoreThanOneDefaultMember);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
    use crate::type_object::common::{CommonUnionMember, NameHash};
    use crate::type_object::minimal::{CommonDiscriminatorMember, MinimalDiscriminatorMember};
    use alloc::vec;

    fn long() -> TypeIdentifier {
        TypeIdentifier::Primitive(PrimitiveKind::Int32)
    }

    fn short() -> TypeIdentifier {
        TypeIdentifier::Primitive(PrimitiveKind::Int16)
    }

    fn member(id: MemberId, name: &str, labels: &[i32], default: bool) -> MinimalUnionMember {
        MinimalUnionMember {
            common: CommonUnionMember {
                member_id: id,
                member_flags: UnionMemberFlag(if default {
                    UnionMemberFlag::IS_DEFAULT
                } else {
                    0
                }),
                type_id: long(),
                label_seq: labels.to_vec(),
            },
            detail: NameHash::from_name(name),
        }
    }

    fn union(
        extensibility: u16,
        discriminator: TypeIdentifier,
        key: bool,
        members: Vec<MinimalUnionMember>,
    ) -> MinimalUnionType {
        MinimalUnionType {
            union_flags: crate::type_object::flags::UnionTypeFlag(extensibility),
            discriminator: MinimalDiscriminatorMember {
                common: CommonDiscriminatorMember {
                    member_flags: UnionDiscriminatorFlag(
                        UnionDiscriminatorFlag::IS_MUST_UNDERSTAND
                            | if key {
                                UnionDiscriminatorFlag::IS_KEY
                            } else {
                                0
                            },
                    ),
                    type_id: discriminator,
                },
            },
            member_seq: members,
        }
    }

    const APPENDABLE: u16 = StructTypeFlag::IS_APPENDABLE;
    const FINAL: u16 = StructTypeFlag::IS_FINAL;

    fn base() -> MinimalUnionType {
        union(APPENDABLE, long(), false, vec![member(1, "a", &[1], false)])
    }

    #[test]
    fn a_derived_union_that_meets_the_five_conditions_is_accepted() {
        let derived = union(
            APPENDABLE,
            long(),
            false,
            vec![member(2, "b", &[2, 3], false), member(3, "c", &[], true)],
        );
        assert_eq!(check_union_derivation(&[base()], &derived), Ok(()));
        // The extensibility kind may be left unspecified.
        let unspecified = union(0, long(), false, vec![member(2, "b", &[2], false)]);
        assert_eq!(check_union_derivation(&[base()], &unspecified), Ok(()));
        // And a union without a base has nothing to check here.
        assert_eq!(check_union_derivation(&[], &derived), Ok(()));
    }

    #[test]
    fn a_derived_union_of_another_extensibility_kind_is_refused() {
        let derived = union(FINAL, long(), false, vec![member(2, "b", &[2], false)]);
        assert_eq!(
            check_union_derivation(&[base()], &derived),
            Err(UnionInheritanceError::ExtensibilityDiffers)
        );
    }

    #[test]
    fn a_derived_union_with_another_discriminator_type_is_refused() {
        let derived = union(
            APPENDABLE,
            short(),
            false,
            vec![member(2, "b", &[2], false)],
        );
        assert_eq!(
            check_union_derivation(&[base()], &derived),
            Err(UnionInheritanceError::DiscriminatorTypeDiffers)
        );
    }

    #[test]
    fn the_discriminator_of_a_derived_union_is_not_a_key() {
        let derived = union(APPENDABLE, long(), true, vec![member(2, "b", &[2], false)]);
        assert_eq!(
            check_union_derivation(&[base()], &derived),
            Err(UnionInheritanceError::DiscriminatorIsKey)
        );
    }

    #[test]
    fn no_ancestor_defines_a_default_member() {
        let with_default = union(
            APPENDABLE,
            long(),
            false,
            vec![member(1, "a", &[1], false), member(5, "d", &[], true)],
        );
        let derived = union(APPENDABLE, long(), false, vec![member(2, "b", &[2], false)]);
        assert_eq!(
            check_union_derivation(&[with_default.clone()], &derived),
            Err(UnionInheritanceError::AncestorHasDefaultMember { ancestor: 0 })
        );
        // Also the root of a longer chain.
        let middle = union(APPENDABLE, long(), false, vec![member(3, "m", &[3], false)]);
        assert_eq!(
            check_union_derivation(&[with_default, middle], &derived),
            Err(UnionInheritanceError::AncestorHasDefaultMember { ancestor: 0 })
        );
    }

    #[test]
    fn a_discriminator_value_selects_at_most_one_member_of_the_hypothetical_union() {
        let derived = union(
            APPENDABLE,
            long(),
            false,
            vec![member(2, "b", &[1, 2], false)],
        );
        assert_eq!(
            check_union_derivation(&[base()], &derived),
            Err(UnionInheritanceError::AmbiguousLabel { label: 1 })
        );
        // A label of a member of a more distant ancestor counts as well.
        let middle = union(APPENDABLE, long(), false, vec![member(3, "m", &[3], false)]);
        let derived = union(APPENDABLE, long(), false, vec![member(4, "d", &[3], false)]);
        assert_eq!(
            check_union_derivation(&[base(), middle], &derived),
            Err(UnionInheritanceError::AmbiguousLabel { label: 3 })
        );
    }

    #[test]
    fn the_hypothetical_union_has_unique_member_ids_and_names() {
        let same_id = union(APPENDABLE, long(), false, vec![member(1, "b", &[2], false)]);
        assert_eq!(
            check_union_derivation(&[base()], &same_id),
            Err(UnionInheritanceError::DuplicateMemberId { member_id: 1 })
        );
        let same_name = union(APPENDABLE, long(), false, vec![member(2, "a", &[2], false)]);
        assert_eq!(
            check_union_derivation(&[base()], &same_name),
            Err(UnionInheritanceError::DuplicateMemberName)
        );
    }

    #[test]
    fn the_members_of_the_ancestors_are_expanded_ahead_of_the_derived_ones() {
        let middle = union(APPENDABLE, long(), false, vec![member(3, "m", &[3], false)]);
        let derived = union(APPENDABLE, long(), false, vec![member(4, "d", &[4], false)]);
        let expanded = expand_union(&[base(), middle], &derived);
        let ids: Vec<MemberId> = expanded
            .member_seq
            .iter()
            .map(|member| member.common.member_id)
            .collect();
        assert_eq!(ids, vec![1, 3, 4]);
        assert_eq!(expanded.union_flags, derived.union_flags);
    }
}
