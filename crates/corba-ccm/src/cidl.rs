// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CIDL data model — spec §7.
//!
//! CIDL (Component Implementation Definition Language) extends
//! IDL with:
//!
//! * `composition` — couples a home executor to a
//!   storage type.
//! * `home executor <Name> <Home> { ... }` — implementer skeleton
//!   for the home.
//! * `storagetype` / `storagehome` — persistent state mapping
//!   (spec §7.4 + Persistent State Service §10).
//!
//! We provide the data model; the CIDL parser is the caller layer
//! (can build on the `crates/idl/` parser extension).

use alloc::string::String;
use alloc::vec::Vec;

/// Storage type definition — spec §7.4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageType {
    /// Name.
    pub name: String,
    /// Repository ID.
    pub repository_id: String,
    /// Optional: single inheritance from a base storage type.
    pub base: Option<String>,
    /// Storage state members as a (name, idl-type) list.
    pub state_members: Vec<(String, String)>,
}

/// Storage home — spec §7.4.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageHome {
    /// Name.
    pub name: String,
    /// Repository ID.
    pub repository_id: String,
    /// The storage type that is managed.
    pub managed_storage_type: String,
    /// Optional: primary key type ID.
    pub primary_key: Option<String>,
}

/// Home executor — spec §7.5.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeExecutor {
    /// Executor name.
    pub name: String,
    /// Implemented home repository ID.
    pub home_id: String,
    /// Component executor reference (bound via composition).
    pub component_executor: Option<String>,
}

/// Composition — spec §7.3.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composition {
    /// Composition name.
    pub name: String,
    /// Category — `session` / `entity` / `service` / `process`
    /// (spec §7.3.1).
    pub category: CompositionCategory,
    /// Home executor name.
    pub home_executor: String,
    /// Storage home name (optional, only for the `entity` category).
    pub home_storage: Option<String>,
}

/// Composition category — spec §7.3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompositionCategory {
    /// `session` — non-persistent (default).
    #[default]
    Session,
    /// `service` — stateless service.
    Service,
    /// `process` — long-running process.
    Process,
    /// `entity` — persistent entity (requires a storage home).
    Entity,
}

impl Composition {
    /// Spec §7.3.1: `entity` compositions require a
    /// storage home, others do not.
    #[must_use]
    pub fn requires_storage_home(&self) -> bool {
        matches!(self.category, CompositionCategory::Entity)
    }

    /// Spec validation: for `entity`, `home_storage` must be set.
    ///
    /// # Errors
    /// Static string if the composition is inconsistent.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.requires_storage_home() && self.home_storage.is_none() {
            return Err("entity composition requires home_storage");
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn session_composition_does_not_require_storage() {
        let c = Composition {
            name: "TraderImpl".into(),
            category: CompositionCategory::Session,
            home_executor: "TraderHomeExec".into(),
            home_storage: None,
        };
        assert!(c.validate().is_ok());
    }

    #[test]
    fn entity_composition_without_storage_invalid() {
        let c = Composition {
            name: "OrderImpl".into(),
            category: CompositionCategory::Entity,
            home_executor: "OrderHomeExec".into(),
            home_storage: None,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn entity_composition_with_storage_valid() {
        let c = Composition {
            name: "OrderImpl".into(),
            category: CompositionCategory::Entity,
            home_executor: "OrderHomeExec".into(),
            home_storage: Some("OrderStorageHome".into()),
        };
        assert!(c.validate().is_ok());
    }

    #[test]
    fn storage_type_can_inherit() {
        let st = StorageType {
            name: "OrderStorage".into(),
            repository_id: "IDL:demo/OrderStorage:1.0".into(),
            base: Some("BaseStorage".into()),
            state_members: alloc::vec![
                ("id".into(), "long".into()),
                ("amount".into(), "double".into()),
            ],
        };
        assert!(st.base.is_some());
        assert_eq!(st.state_members.len(), 2);
    }

    #[test]
    fn home_executor_optional_component_binding() {
        let he = HomeExecutor {
            name: "TraderHomeExec".into(),
            home_id: "IDL:demo/TraderHome:1.0".into(),
            component_executor: Some("TraderExec".into()),
        };
        assert!(he.component_executor.is_some());
    }

    #[test]
    fn all_four_composition_categories_distinct() {
        let cats = [
            CompositionCategory::Session,
            CompositionCategory::Service,
            CompositionCategory::Process,
            CompositionCategory::Entity,
        ];
        for (i, a) in cats.iter().enumerate() {
            for b in cats.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }
}
