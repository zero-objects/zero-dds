// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Spec §7.3.1 `Root` metadata operations that are not DCPS entities:
//! `create/update/delete/get_qos_library`, `create/delete/get_qos_profile`,
//! `create/delete/get_type`. A `QosLibrary`/`QosProfile`/registered
//! `Type` is a named document the daemon stores and serves back — no
//! DCPS round-trip, so it stays out of `zerodds_web::DdsBackend` (whose
//! doc comment scopes it to §7.4 entity operations).
//!
//! In-memory, process-lifetime store. Concurrency: one `Mutex` per
//! collection, held only for the duration of the map operation.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// A stored resource: its XML body plus the content type it was stored
/// under (mirrors `zerodds_web::bridge::BackendResult::Body`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocument {
    /// MIME type, e.g. `application/dds-web+xml`.
    pub content_type: String,
    /// Raw body bytes.
    pub body: Vec<u8>,
}

/// Error from the metadata store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// The name already exists (create).
    Conflict(String),
    /// The name does not exist (update/delete/get).
    NotFound(String),
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict(n) => write!(f, "`{n}` already exists"),
            Self::NotFound(n) => write!(f, "`{n}` not found"),
        }
    }
}

impl std::error::Error for MetadataError {}

/// One named collection (QoS libraries, or the profiles of one
/// library, or the registered types) — `create` refuses a duplicate
/// name, `update`/`delete`/`get` refuse a missing one.
#[derive(Default)]
pub struct NamedStore(Mutex<BTreeMap<String, StoredDocument>>);

impl NamedStore {
    /// Empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// # Errors
    /// [`MetadataError::Conflict`] if `name` already exists.
    pub fn create(&self, name: &str, doc: StoredDocument) -> Result<(), MetadataError> {
        let mut map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.contains_key(name) {
            return Err(MetadataError::Conflict(name.to_string()));
        }
        map.insert(name.to_string(), doc);
        Ok(())
    }

    /// # Errors
    /// [`MetadataError::NotFound`] if `name` does not exist.
    pub fn update(&self, name: &str, doc: StoredDocument) -> Result<(), MetadataError> {
        let mut map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !map.contains_key(name) {
            return Err(MetadataError::NotFound(name.to_string()));
        }
        map.insert(name.to_string(), doc);
        Ok(())
    }

    /// # Errors
    /// [`MetadataError::NotFound`] if `name` does not exist.
    pub fn delete(&self, name: &str) -> Result<(), MetadataError> {
        let mut map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.remove(name).is_none() {
            return Err(MetadataError::NotFound(name.to_string()));
        }
        Ok(())
    }

    /// # Errors
    /// [`MetadataError::NotFound`] if `name` does not exist.
    pub fn get(&self, name: &str) -> Result<StoredDocument, MetadataError> {
        let map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(name)
            .cloned()
            .ok_or_else(|| MetadataError::NotFound(name.to_string()))
    }

    /// Every stored name, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.keys().cloned().collect()
    }
}

/// The three Spec §7.3.1 metadata collections a `Gateway` serves beside
/// its DCPS-backed `DdsBackend` operations.
#[derive(Default)]
pub struct MetadataStore {
    /// `Root::{create,update,delete,get}_qos_library`.
    pub qos_libraries: NamedStore,
    /// `QosLibrary::{create,update,delete,get}_qos_profile`, keyed
    /// `<library>/<profile>` — the profile namespace is per-library
    /// (Spec §7.3.1), so a flat map needs the composite key.
    pub qos_profiles: NamedStore,
    /// `Root::{create,delete,get}_type`.
    pub types: NamedStore,
}

impl MetadataStore {
    /// Empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The composite key `qos_profiles` uses.
    #[must_use]
    pub fn profile_key(library: &str, profile: &str) -> String {
        format!("{library}/{profile}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn doc(text: &str) -> StoredDocument {
        StoredDocument {
            content_type: "application/dds-web+xml".to_string(),
            body: text.as_bytes().to_vec(),
        }
    }

    #[test]
    fn create_then_get_round_trips() {
        let s = NamedStore::new();
        s.create("HighQos", doc("<qos_library/>")).unwrap();
        assert_eq!(s.get("HighQos").unwrap(), doc("<qos_library/>"));
    }

    #[test]
    fn create_twice_is_conflict() {
        let s = NamedStore::new();
        s.create("A", doc("x")).unwrap();
        assert_eq!(
            s.create("A", doc("y")).unwrap_err(),
            MetadataError::Conflict("A".to_string())
        );
    }

    #[test]
    fn update_missing_is_not_found() {
        let s = NamedStore::new();
        assert_eq!(
            s.update("A", doc("x")).unwrap_err(),
            MetadataError::NotFound("A".to_string())
        );
    }

    #[test]
    fn delete_removes_and_get_then_fails() {
        let s = NamedStore::new();
        s.create("A", doc("x")).unwrap();
        s.delete("A").unwrap();
        assert!(matches!(s.get("A"), Err(MetadataError::NotFound(_))));
    }

    #[test]
    fn names_lists_every_entry() {
        let s = NamedStore::new();
        s.create("B", doc("x")).unwrap();
        s.create("A", doc("x")).unwrap();
        assert_eq!(s.names(), vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn profile_key_is_library_slash_profile() {
        assert_eq!(MetadataStore::profile_key("Lib", "High"), "Lib/High");
    }

    #[test]
    fn profiles_of_different_libraries_do_not_collide() {
        let store = MetadataStore::new();
        store
            .qos_profiles
            .create(&MetadataStore::profile_key("LibA", "P"), doc("a"))
            .unwrap();
        store
            .qos_profiles
            .create(&MetadataStore::profile_key("LibB", "P"), doc("b"))
            .unwrap();
        assert_eq!(
            store
                .qos_profiles
                .get(&MetadataStore::profile_key("LibA", "P"))
                .unwrap(),
            doc("a")
        );
    }
}
