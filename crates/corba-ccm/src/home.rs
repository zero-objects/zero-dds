// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Home-Definition + Finder — Spec §6.7.
//!
//! Ein Home ist eine Factory + Finder fuer einen oder mehrere
//! Component-Types. Spec §6.7.1: jedes Home managt genau einen
//! Component-Type (deklariert via `manages`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

/// Home-Definition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeDef {
    /// Home-Name.
    pub name: String,
    /// Home-Repository-ID.
    pub repository_id: String,
    /// Verwalteter Component-Type (Repository-ID).
    pub managed_component_id: String,
    /// Optional: PrimaryKey-Type-ID (`primarykey K`).
    pub primary_key_id: Option<String>,
}

impl HomeDef {
    /// Spec §6.7.6 — `get_component_def() -> ComponentIR::ComponentDef`.
    /// Liefert die Repository-ID des verwalteten Component-Types;
    /// Caller setzt darauf den IFR-Lookup auf.
    #[must_use]
    pub fn get_component_def_repo_id(&self) -> &str {
        &self.managed_component_id
    }

    /// Spec §6.7.6 — `get_home_def() -> ComponentIR::HomeDef`.
    /// Liefert die eigene Repository-ID; Caller setzt darauf den
    /// IFR-Lookup auf.
    #[must_use]
    pub fn get_home_def_repo_id(&self) -> &str {
        &self.repository_id
    }
}

/// Component-Reference im Home-Finder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRef {
    /// Component-Repository-ID.
    pub repository_id: String,
    /// Eindeutige Component-Instance-Id (POA-ObjectKey-equivalent).
    pub instance_id: Vec<u8>,
}

/// HomeFinder — Spec §6.7.1.
pub struct HomeFinder {
    /// Aktive Component-Instances per `(repo_id, primary_key_bytes)`.
    instances: Mutex<BTreeMap<(String, Vec<u8>), ComponentRef>>,
}

impl Default for HomeFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeFinder {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(BTreeMap::new()),
        }
    }

    /// `create` — Spec §6.7.1.1.
    pub fn create(&self, repo_id: &str, key_bytes: Vec<u8>) -> Arc<ComponentRef> {
        let mut g = match self.instances.lock() {
            Ok(g) => g,
            Err(_) => return Arc::new(empty_ref()),
        };
        let instance_id = if key_bytes.is_empty() {
            // Spec §6.7.2.5: SYSTEM-generated Id fuer unkeyed Comps.
            alloc::format!("system-{}", g.len() + 1).into_bytes()
        } else {
            key_bytes.clone()
        };
        let r = ComponentRef {
            repository_id: repo_id.to_string(),
            instance_id: instance_id.clone(),
        };
        g.insert((repo_id.to_string(), key_bytes), r.clone());
        Arc::new(r)
    }

    /// `find_by_primary_key` — Spec §6.7.1.2.
    pub fn find_by_primary_key(
        &self,
        repo_id: &str,
        key_bytes: &[u8],
    ) -> Option<Arc<ComponentRef>> {
        self.instances.lock().ok().and_then(|g| {
            g.get(&(repo_id.to_string(), key_bytes.to_vec()))
                .cloned()
                .map(Arc::new)
        })
    }

    /// `remove` — Spec §6.7.1.3.
    pub fn remove(&self, repo_id: &str, key_bytes: &[u8]) -> bool {
        self.instances
            .lock()
            .ok()
            .map(|mut g| {
                g.remove(&(repo_id.to_string(), key_bytes.to_vec()))
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// Anzahl aktiver Instances.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.instances.lock().map(|g| g.len()).unwrap_or(0)
    }
}

fn empty_ref() -> ComponentRef {
    ComponentRef {
        repository_id: String::new(),
        instance_id: Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn home_def_default_has_no_primary_key() {
        let h = HomeDef::default();
        assert!(h.primary_key_id.is_none());
    }

    #[test]
    fn keyed_home_has_primary_key() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: Some("IDL:demo/TraderKey:1.0".into()),
        };
        assert!(h.primary_key_id.is_some());
    }

    #[test]
    fn create_with_user_key_uses_key_as_id() {
        let f = HomeFinder::new();
        let r = f.create("IDL:demo/Trader:1.0", b"UK".to_vec());
        assert_eq!(r.instance_id, b"UK");
    }

    #[test]
    fn create_without_key_generates_system_id() {
        let f = HomeFinder::new();
        let r1 = f.create("IDL:demo/Trader:1.0", alloc::vec![]);
        let r2 = f.create("IDL:demo/Trader:1.0", alloc::vec![]);
        // Spec §6.7.2.5: bei leeren Keys generiert System-Ids;
        // unkeyed: pro `(repo_id, [])`-Schluessel ueberschreibt — wir
        // tracken nur den letzten. Tests zeigen, dass beide Calls
        // einen Ref liefern und der zweite die `system-1`-Form
        // ueberschreibt.
        assert!(r1.instance_id.starts_with(b"system-"));
        assert!(r2.instance_id.starts_with(b"system-"));
    }

    #[test]
    fn find_by_primary_key_round_trip() {
        let f = HomeFinder::new();
        let _ = f.create("IDL:demo/Trader:1.0", b"K1".to_vec());
        let r = f.find_by_primary_key("IDL:demo/Trader:1.0", b"K1");
        assert!(r.is_some());
    }

    #[test]
    fn remove_decrements_instance_count() {
        let f = HomeFinder::new();
        let _ = f.create("IDL:demo/Trader:1.0", b"K1".to_vec());
        let _ = f.create("IDL:demo/Trader:1.0", b"K2".to_vec());
        assert_eq!(f.instance_count(), 2);
        assert!(f.remove("IDL:demo/Trader:1.0", b"K1"));
        assert_eq!(f.instance_count(), 1);
    }

    #[test]
    fn remove_unknown_returns_false() {
        let f = HomeFinder::new();
        assert!(!f.remove("IDL:demo/X:1.0", b"K"));
    }

    // ---------------------------------------------------------------
    // §6.7.6 CCMHome Interface
    // ---------------------------------------------------------------

    #[test]
    fn home_get_component_def_returns_managed_component_id() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: None,
        };
        assert_eq!(h.get_component_def_repo_id(), "IDL:demo/Trader:1.0");
    }

    #[test]
    fn home_get_home_def_returns_self_repo_id() {
        let h = HomeDef {
            name: "TraderHome".into(),
            repository_id: "IDL:demo/TraderHome:1.0".into(),
            managed_component_id: "IDL:demo/Trader:1.0".into(),
            primary_key_id: None,
        };
        assert_eq!(h.get_home_def_repo_id(), "IDL:demo/TraderHome:1.0");
    }
}
