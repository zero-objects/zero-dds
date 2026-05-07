// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CIDL-Datenmodell — Spec §7.
//!
//! CIDL (Component Implementation Definition Language) erweitert
//! IDL um:
//!
//! * `composition` — koppelt einen Home-Executor an einen
//!   Storage-Type.
//! * `home executor <Name> <Home> { ... }` — Implementer-Skeleton
//!   fuer das Home.
//! * `storagetype` / `storagehome` — persistente State-Mapping
//!   (Spec §7.4 + Persistent State Service §10).
//!
//! Wir liefern das Datenmodell; CIDL-Parser ist Caller-Layer (kann
//! auf `crates/idl/`-Parser-Erweiterung aufbauen).

use alloc::string::String;
use alloc::vec::Vec;

/// Storage-Type-Definition — Spec §7.4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageType {
    /// Name.
    pub name: String,
    /// Repository-ID.
    pub repository_id: String,
    /// Optional: Single-Inheritance auf einen Base-StorageType.
    pub base: Option<String>,
    /// Storage-State-Member als (name, idl-type) Liste.
    pub state_members: Vec<(String, String)>,
}

/// Storage-Home — Spec §7.4.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageHome {
    /// Name.
    pub name: String,
    /// Repository-ID.
    pub repository_id: String,
    /// Storage-Type, der verwaltet wird.
    pub managed_storage_type: String,
    /// Optional: PrimaryKey-Type-ID.
    pub primary_key: Option<String>,
}

/// Home-Executor — Spec §7.5.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeExecutor {
    /// Executor-Name.
    pub name: String,
    /// Implementiertes Home-Repository-ID.
    pub home_id: String,
    /// Component-Executor-Reference (durch composition gebunden).
    pub component_executor: Option<String>,
}

/// Composition — Spec §7.3.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composition {
    /// Composition-Name.
    pub name: String,
    /// Category — `session` / `entity` / `service` / `process`
    /// (Spec §7.3.1).
    pub category: CompositionCategory,
    /// Home-Executor-Name.
    pub home_executor: String,
    /// Storage-Home-Name (optional, nur bei `entity`-Category).
    pub home_storage: Option<String>,
}

/// Composition-Category — Spec §7.3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompositionCategory {
    /// `session` — non-persistent (Default).
    #[default]
    Session,
    /// `service` — stateless service.
    Service,
    /// `process` — long-running process.
    Process,
    /// `entity` — persistent entity (verlangt Storage-Home).
    Entity,
}

impl Composition {
    /// Spec §7.3.1: `entity`-Compositions verlangen ein
    /// Storage-Home, andere nicht.
    #[must_use]
    pub fn requires_storage_home(&self) -> bool {
        matches!(self.category, CompositionCategory::Entity)
    }

    /// Spec-Validierung: bei `entity` muss `home_storage` gesetzt sein.
    ///
    /// # Errors
    /// Static-String wenn die Composition inkonsistent ist.
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
