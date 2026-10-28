// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The equality of two values of a type. DDS-XTypes 1.3 §7.5.2.11.5.
//!
//! "Two data samples are considered to be equal if and only if all of the
//! following conditions hold: their respective type definitions are equal;
//! all contained values are equal and occur in the same order; if the
//! samples' type is an aggregated type, the previous rule shall be amended
//! as follows: members shall be compared without regard to their order;
//! one of the samples may omit a non-optional member that is present in
//! the other if that member takes its default value in the latter sample."
//!
//! [`DynamicData`]'s own `==` is the structural one — it carries no type,
//! so it cannot know a member's default or which members are optional —
//! and it takes an aggregate's members in the order they went in. This is
//! the equality the clause names, and it needs the type: the type says
//! which member may be missing, and what a missing one is.

use alloc::vec::Vec;

use super::data::DynamicData;
use super::descriptor::TypeKind;
use super::error::DynamicError;
use super::type_::DynamicType;

/// How deep the walk follows a value before it gives up; the same bound
/// the other walks over a type use.
const MAX_DEPTH: usize = 64;

impl DynamicType {
    /// Whether two values of this type are equal in the sense of
    /// §7.5.2.11.5.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a reference the model does not
    /// close, [`DynamicError::Inconsistent`] where a member's default
    /// cannot be named, and [`DynamicError::Recursive`] for a value nested
    /// deeper than the walk follows.
    pub fn values_equal(&self, a: &DynamicData, b: &DynamicData) -> Result<bool, DynamicError> {
        self.values_equal_at(a, b, 0)
    }

    fn values_equal_at(
        &self,
        a: &DynamicData,
        b: &DynamicData,
        depth: usize,
    ) -> Result<bool, DynamicError> {
        if depth > MAX_DEPTH {
            return Err(DynamicError::Recursive {
                what: self.name().into(),
            });
        }
        let ty = self.resolve_alias()?;
        match ty.kind() {
            TypeKind::Structure | TypeKind::Annotation => ty.aggregates_equal(a, b, depth),
            TypeKind::Union => ty.unions_equal(a, b, depth),
            TypeKind::Sequence | TypeKind::Array => {
                let (DynamicData::Collection(left), DynamicData::Collection(right)) = (a, b) else {
                    return Ok(false);
                };
                if left.len() != right.len() {
                    return Ok(false);
                }
                let element = ty.element_type().ok_or_else(|| DynamicError::Unresolved {
                    at: ty.name().into(),
                })?;
                for (x, y) in left.iter().zip(right) {
                    if !element.values_equal_at(x, y, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            TypeKind::Map => ty.maps_equal(a, b, depth),
            TypeKind::Bitmask => {
                let (DynamicData::Bits(left), DynamicData::Bits(right)) = (a, b) else {
                    return Ok(false);
                };
                // Flags are a set: the order they were set in is not a
                // property of the value.
                Ok(left.len() == right.len() && left.iter().all(|flag| right.contains(flag)))
            }
            _ => Ok(a == b),
        }
    }

    /// The members of a structure, the base's first.
    fn all_members(&self) -> Result<Vec<super::type_::DynamicTypeMember>, DynamicError> {
        let mut chain = Vec::new();
        let mut current = Some(self.clone());
        while let Some(ty) = current {
            if chain.len() >= MAX_DEPTH {
                return Err(DynamicError::Recursive {
                    what: self.name().into(),
                });
            }
            current = ty.base_type().and_then(|base| base.resolve_alias().ok());
            chain.push(ty);
        }
        Ok(chain
            .into_iter()
            .rev()
            .flat_map(|ty| ty.members())
            .collect())
    }

    fn aggregates_equal(
        &self,
        a: &DynamicData,
        b: &DynamicData,
        depth: usize,
    ) -> Result<bool, DynamicError> {
        let (Some(left), Some(right)) = (a.members(), b.members()) else {
            return Ok(false);
        };
        let members = self.all_members()?;
        for member in &members {
            let member_type = member.dynamic_type()?;
            match (a.member(member.name()), b.member(member.name())) {
                (Some(x), Some(y)) => {
                    if !member_type.values_equal_at(x, y, depth + 1)? {
                        return Ok(false);
                    }
                }
                (None, None) => {}
                (Some(present), None) | (None, Some(present)) => {
                    // An optional member that is present in one sample and
                    // not in the other is a difference. A member that is
                    // not optional may be left out of one sample if it
                    // takes its default in the other.
                    if member.descriptor()?.is_optional {
                        return Ok(false);
                    }
                    let default: DynamicData = member.default_value()?;
                    if !member_type.values_equal_at(present, &default, depth + 1)? {
                        return Ok(false);
                    }
                }
            }
        }
        // What the type does not declare is compared as it is.
        let declared = |name: &str| members.iter().any(|member| member.name() == name);
        let extra = |entries: &[(alloc::string::String, DynamicData)]| {
            entries
                .iter()
                .filter(|(name, _)| !declared(name))
                .cloned()
                .collect::<Vec<_>>()
        };
        let (left_extra, right_extra) = (extra(left), extra(right));
        Ok(left_extra.len() == right_extra.len()
            && left_extra.iter().all(|entry| right_extra.contains(entry)))
    }

    fn unions_equal(
        &self,
        a: &DynamicData,
        b: &DynamicData,
        depth: usize,
    ) -> Result<bool, DynamicError> {
        let (Some(left), Some(right)) = (a.members(), b.members()) else {
            return Ok(false);
        };
        let discriminator = self
            .discriminator_type()
            .ok_or_else(|| DynamicError::Unresolved {
                at: self.name().into(),
            })?;
        let (Some(x), Some(y)) = (a.member("_d"), b.member("_d")) else {
            return Ok(false);
        };
        if !discriminator.values_equal_at(x, y, depth + 1)? {
            return Ok(false);
        }
        // The selected member, by name; "the number of members in the
        // object" is two where the discriminator selects one and one
        // where it does not (§7.5.2.11.6).
        if left.len() != right.len() {
            return Ok(false);
        }
        for (name, x) in left.iter().filter(|(name, _)| name != "_d") {
            let Some(y) = b.member(name) else {
                return Ok(false);
            };
            let equal = match self.member_by_name(name) {
                Some(member) => member.dynamic_type()?.values_equal_at(x, y, depth + 1)?,
                None => x == y,
            };
            if !equal {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn maps_equal(
        &self,
        a: &DynamicData,
        b: &DynamicData,
        depth: usize,
    ) -> Result<bool, DynamicError> {
        let (DynamicData::Collection(left), DynamicData::Collection(right)) = (a, b) else {
            return Ok(false);
        };
        if left.len() != right.len() {
            return Ok(false);
        }
        let (Some(key), Some(value)) = (self.key_element_type(), self.element_type()) else {
            return Err(DynamicError::Unresolved {
                at: self.name().into(),
            });
        };
        // A map is unordered: every entry of one has an equal entry in
        // the other, and the lengths agree.
        'entries: for entry in left {
            let DynamicData::Collection(pair) = entry else {
                return Ok(false);
            };
            let [entry_key, entry_value] = pair.as_slice() else {
                return Ok(false);
            };
            for other in right {
                let DynamicData::Collection(other_pair) = other else {
                    return Ok(false);
                };
                let [other_key, other_value] = other_pair.as_slice() else {
                    return Ok(false);
                };
                if key.values_equal_at(entry_key, other_key, depth + 1)?
                    && value.values_equal_at(entry_value, other_value, depth + 1)?
                {
                    continue 'entries;
                }
            }
            return Ok(false);
        }
        Ok(true)
    }
}
