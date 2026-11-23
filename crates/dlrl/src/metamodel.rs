// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DLRL 1.2 entity names, exception types and cache states.
//!
//! Spec source: `dds-1.2.pdf` §8 DLRL. The metamodel of §8.1.3.3 and the
//! mapping information of §8.1.4.4 are in [`crate::model`] and
//! [`crate::mapping`].

// ===========================================================================
// §8.1.3.2.1 ObjectRoot — inheritance base
// ===========================================================================

/// Spec §8.1.3.2.1 — `ObjectRoot` as the common abstract base for all
/// DLRL objects.
pub trait ObjectRoot: Send + Sync {
    /// Spec §8.1.3.1 — object identity (oid).
    fn oid(&self) -> u64;
    /// Repository ID of the object type.
    fn repository_id(&self) -> &str;
    /// Spec §8.1.3.1 — `is_modified()`.
    fn is_modified(&self) -> bool;
    /// Spec §8.1.3.1 — `is_deleted()`.
    fn is_deleted(&self) -> bool;
}

// ===========================================================================
// §8.1.6.2 — the DLRL entities of the table on page 187
// ===========================================================================

/// Spec §8.1.6.2 — the nineteen DLRL entities of the table on page 187,
/// in its order. The run-time types themselves are in [`crate::cache`];
/// this enum is the name table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlrlEntityKind {
    /// `CacheFactory`.
    CacheFactory,
    /// `CacheBase`.
    CacheBase,
    /// `Cache`.
    Cache,
    /// `CacheAccess`.
    CacheAccess,
    /// `CacheListener`.
    CacheListener,
    /// `Contract`.
    Contract,
    /// `ObjectHome`.
    ObjectHome,
    /// `ObjectListener`.
    ObjectListener,
    /// `Selection`.
    Selection,
    /// `SelectionCriterion`.
    SelectionCriterion,
    /// `FilterCriterion`.
    FilterCriterion,
    /// `QueryCriterion`.
    QueryCriterion,
    /// `SelectionListener`.
    SelectionListener,
    /// `ObjectRoot`.
    ObjectRoot,
    /// `Collection`.
    Collection,
    /// `List`.
    List,
    /// `Set`.
    Set,
    /// `StrMap`.
    StrMap,
    /// `IntMap`.
    IntMap,
}

impl DlrlEntityKind {
    /// Spec §8.1.6.2 — the class name of the table.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::CacheFactory => "CacheFactory",
            Self::CacheBase => "CacheBase",
            Self::Cache => "Cache",
            Self::CacheAccess => "CacheAccess",
            Self::CacheListener => "CacheListener",
            Self::Contract => "Contract",
            Self::ObjectHome => "ObjectHome",
            Self::ObjectListener => "ObjectListener",
            Self::Selection => "Selection",
            Self::SelectionCriterion => "SelectionCriterion",
            Self::FilterCriterion => "FilterCriterion",
            Self::QueryCriterion => "QueryCriterion",
            Self::SelectionListener => "SelectionListener",
            Self::ObjectRoot => "ObjectRoot",
            Self::Collection => "Collection",
            Self::List => "List",
            Self::Set => "Set",
            Self::StrMap => "StrMap",
            Self::IntMap => "IntMap",
        }
    }

    /// The nineteen entities in the order of the table on page 187.
    #[must_use]
    pub fn all() -> [Self; 19] {
        [
            Self::CacheFactory,
            Self::CacheBase,
            Self::Cache,
            Self::CacheAccess,
            Self::CacheListener,
            Self::Contract,
            Self::ObjectHome,
            Self::ObjectListener,
            Self::Selection,
            Self::SelectionCriterion,
            Self::FilterCriterion,
            Self::QueryCriterion,
            Self::SelectionListener,
            Self::ObjectRoot,
            Self::Collection,
            Self::List,
            Self::Set,
            Self::StrMap,
            Self::IntMap,
        ]
    }
}

/// The exceptions of §8.1.6.2 live in [`crate::error`].
pub use crate::error::DlrlException;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // §8.1.3.2.1 ObjectRoot
    struct DummyObject;
    impl ObjectRoot for DummyObject {
        fn oid(&self) -> u64 {
            42
        }
        fn repository_id(&self) -> &str {
            "IDL:demo/Dummy:1.0"
        }
        fn is_modified(&self) -> bool {
            false
        }
        fn is_deleted(&self) -> bool {
            false
        }
    }

    #[test]
    fn object_root_trait_callable() {
        let o = DummyObject;
        assert_eq!(o.oid(), 42);
        assert_eq!(o.repository_id(), "IDL:demo/Dummy:1.0");
        assert!(!o.is_modified());
        assert!(!o.is_deleted());
    }

    /// §8.1.6.2: the table on page 187 names nineteen entities (the
    /// specification's own count of seventeen is wrong).
    #[test]
    fn dlrl_8_1_6_2_entity_table_has_the_nineteen_names_of_page_187() {
        let names: alloc::vec::Vec<&str> = DlrlEntityKind::all()
            .iter()
            .map(|k| k.spec_name())
            .collect();
        assert_eq!(
            names,
            [
                "CacheFactory",
                "CacheBase",
                "Cache",
                "CacheAccess",
                "CacheListener",
                "Contract",
                "ObjectHome",
                "ObjectListener",
                "Selection",
                "SelectionCriterion",
                "FilterCriterion",
                "QueryCriterion",
                "SelectionListener",
                "ObjectRoot",
                "Collection",
                "List",
                "Set",
                "StrMap",
                "IntMap",
            ]
        );
    }
}
