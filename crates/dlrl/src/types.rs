// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Value types of the DLRL PSM (DDS 1.2 §8.2.1.2.1).
//!
//! `DLRLOid` is the object identifier the application sees (§8.1.3.1:
//! "Object identity is given by an oid (object ID) part of any DLRL
//! object"). The PSM declares it as
//! `struct DLRLOid { DLRL_OID_TYPE_NATIVE value[3]; }` with
//! `#define DLRL_OID_TYPE_NATIVE long`.
//!
//! The other enumerations, typedefs and valuetypes of the PSM that are
//! not tied to the DCPS binding are here too; those of the run-time
//! entities (`CacheUsage`, `CacheKind`, `DCPSState`, `ObjectState`,
//! `ObjectScope`) are in [`crate::cache`] (feature `dcps`).

use alloc::string::String;
use alloc::vec::Vec;

/// DDS 1.2 §8.2.1.2.1 `DLRLOid`: three `long` values.
///
/// How the three values are filled is left to the implementation
/// (§8.1.4.2.2, footnote 6: "e.g., by means of two sub-fields, one to
/// designate the author and one for a sequence number").
///
/// The value `{0, 0, 0}` ([`DlrlOid::NIL`]) is not the oid of an object:
/// `Model::check_value` refuses a reference to it, `Mapping::rows_of`
/// an object with it, and `Mapping::reconstruct` a row whose owner key
/// holds it. In a DCPS cell it stands for a reference to no object (an
/// unset to-1 relation), which §8.1.4.2.3 leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DlrlOid {
    /// The three `long` values of the PSM struct.
    pub value: [i32; 3],
}

impl DlrlOid {
    /// The oid that designates no object.
    pub const NIL: Self = Self { value: [0, 0, 0] };

    /// An oid from its three values.
    #[must_use]
    pub const fn new(a: i32, b: i32, c: i32) -> Self {
        Self { value: [a, b, c] }
    }

    /// True for [`DlrlOid::NIL`].
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.value[0] == 0 && self.value[1] == 0 && self.value[2] == 0
    }
}

impl core::fmt::Display for DlrlOid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.value[0], self.value[1], self.value[2])
    }
}

/// PSM `LongSeq` (§8.2.1.2.1): the indexes of a `List` and the keys of
/// an `IntMap`.
pub type LongSeq = Vec<i32>;

/// PSM `ClassName` (§8.2.1.2.1).
pub type ClassName = String;

/// PSM `CacheName` (§8.2.1.2.1).
pub type CacheName = String;

/// PSM `RelationName` (§8.2.1.2.1).
pub type RelationName = String;

/// PSM `RelatedObjectDepth` (§8.2.1.2.1, `typedef short`).
pub type RelatedObjectDepth = i16;

/// PSM `UNLIMITED_RELATED_OBJECTS` (§8.2.1.2.1): the depth of a contract
/// that clones all navigable objects. `Contract::depth` is a `long` in
/// the IDL; compare with `i32::from(UNLIMITED_RELATED_OBJECTS)`.
pub const UNLIMITED_RELATED_OBJECTS: RelatedObjectDepth = -1;

/// PSM `ReferenceScope` (§8.2.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceScope {
    /// `SIMPLE_CONTENT_SCOPE`: only the reference content.
    SimpleContent,
    /// `REFERENCED_CONTENTS_SCOPE`: also the referenced contents.
    ReferencedContents,
}

/// PSM `CriterionKind` (§8.2.1.2.1): the `kind` of a
/// `SelectionCriterion` (§8.1.6.3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CriterionKind {
    /// `QUERY`: a `QueryCriterion`.
    Query,
    /// `FILTER`: a `FilterCriterion`.
    Filter,
}

/// PSM `MembershipState` (§8.2.1.2.1): whether an object passed the
/// criterion of a Selection the last time it was examined
/// (`FilterCriterion::check_object`, §8.1.6.3.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MembershipState {
    /// `UNDEFINED_MEMBERSHIP`: the Selection has not examined the object.
    UndefinedMembership,
    /// `ALREADY_MEMBER`: the object is a member.
    AlreadyMember,
    /// `NOT_MEMBER`: the object was examined and is not a member.
    NotMember,
}

/// PSM `RelationKind` (§8.2.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    /// `REF_RELATION`: a to-1 relation.
    RefRelation,
    /// `LIST_RELATION`.
    ListRelation,
    /// `INT_MAP_RELATION`.
    IntMapRelation,
    /// `STR_MAP_RELATION`.
    StrMapRelation,
    /// Not in the PSM: a `Set` relation (§8.1.3.2.2 allows a `Set` as the
    /// basis of a multi-relation, the PSM's `RelationKind` has no value
    /// for it). The kind of [`RelationDescription::Set`].
    SetRelation,
}

/// PSM `RelationDescription` and its three subtypes (§8.2.1.2.1): a
/// relation of an object, with the index or key of one item for a
/// collection (`ObjectRoot::which_contained_modified`, §8.1.6.3.14).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationDescription {
    /// `RelationDescription` of kind `REF_RELATION`.
    Ref {
        /// `name`.
        name: RelationName,
    },
    /// `ListRelationDescription`.
    List {
        /// `name`.
        name: RelationName,
        /// `index`.
        index: i32,
    },
    /// `IntMapRelationDescription`.
    IntMap {
        /// `name`.
        name: RelationName,
        /// `key`.
        key: i32,
    },
    /// `StrMapRelationDescription`.
    StrMap {
        /// `name`.
        name: RelationName,
        /// `key`.
        key: String,
    },
    /// A `RelationDescription` of a `Set` relation, kind
    /// [`RelationKind::SetRelation`] (not in the PSM); a set has no index
    /// or key.
    Set {
        /// `name`.
        name: RelationName,
    },
}

impl RelationDescription {
    /// `kind`.
    #[must_use]
    pub const fn kind(&self) -> RelationKind {
        match self {
            Self::Ref { .. } => RelationKind::RefRelation,
            Self::List { .. } => RelationKind::ListRelation,
            Self::IntMap { .. } => RelationKind::IntMapRelation,
            Self::StrMap { .. } => RelationKind::StrMapRelation,
            Self::Set { .. } => RelationKind::SetRelation,
        }
    }

    /// `name`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Ref { name }
            | Self::List { name, .. }
            | Self::IntMap { name, .. }
            | Self::StrMap { name, .. }
            | Self::Set { name } => name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlrl_8_2_1_relation_description_kind_follows_its_subtype() {
        let all = [
            RelationDescription::Ref { name: "a".into() },
            RelationDescription::List {
                name: "b".into(),
                index: 2,
            },
            RelationDescription::IntMap {
                name: "c".into(),
                key: -4,
            },
            RelationDescription::StrMap {
                name: "d".into(),
                key: "k".into(),
            },
            RelationDescription::Set { name: "e".into() },
        ];
        let kinds: Vec<RelationKind> = all.iter().map(RelationDescription::kind).collect();
        assert_eq!(
            kinds,
            [
                RelationKind::RefRelation,
                RelationKind::ListRelation,
                RelationKind::IntMapRelation,
                RelationKind::StrMapRelation,
                RelationKind::SetRelation
            ]
        );
        let names: Vec<&str> = all.iter().map(RelationDescription::name).collect();
        assert_eq!(names, ["a", "b", "c", "d", "e"]);
        assert_eq!(UNLIMITED_RELATED_OBJECTS, -1);
    }
}
