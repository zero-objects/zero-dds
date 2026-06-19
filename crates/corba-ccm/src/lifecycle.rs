// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CCM 4.0 lifecycle constraints + receptacle state machine +
//! configurator interface — spec §6.4.2 / §6.5.2 / §6.10.
//!
//! These modules cover the data-model side of the lifecycle rules.
//! The actual container-runtime enforcement happens in
//! `container.rs`; here we provide the constraint validators and
//! state machines that this runtime can use.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use crate::component_def::{AttributeDef, ComponentDef};

// ---------------------------------------------------------------------------
// §6.4.2 Semantics of Facet References — Lifecycle-Constraints
// ---------------------------------------------------------------------------

/// Spec §6.4.2 — facet lifetime constraint.
///
/// "The lifetime of a facet [...] is bound to the lifetime of the
/// component instance that provides it." We model this as a
/// constraint check: a facet reference must NOT outlive
/// the component instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacetLifetimeViolation {
    /// The facet was used after the component was destroyed.
    UseAfterComponentDestroy,
    /// The facet reference is orphaned (no component back-ref).
    OrphanedFacetReference,
}

/// Validates a facet reference against the component lifecycle.
///
/// `component_alive` must be `true` if the component instance
/// still exists; `facet_in_component` must be `true` if the
/// facet really belongs to the component.
///
/// # Errors
/// `FacetLifetimeViolation` if the constraint is violated.
pub fn check_facet_lifetime(
    component_alive: bool,
    facet_in_component: bool,
) -> Result<(), FacetLifetimeViolation> {
    if !facet_in_component {
        return Err(FacetLifetimeViolation::OrphanedFacetReference);
    }
    if !component_alive {
        return Err(FacetLifetimeViolation::UseAfterComponentDestroy);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §6.5.2 Receptacles — Connection-State-Machine
// ---------------------------------------------------------------------------

/// Spec §6.5.2 — connection state machine for receptacles.
///
/// A receptacle moves through the states `Disconnected → Connected →
/// Disconnected`. `connect()` for simplex multiplicity is only allowed in
/// `Disconnected`; `disconnect()` only in `Connected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No provider bound.
    Disconnected,
    /// Provider bound (object reference present).
    Connected,
}

/// Connection lifecycle errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionError {
    /// `connect` on an already-connected simplex receptacle.
    AlreadyConnected,
    /// `disconnect` on an unbound receptacle.
    NoConnection,
    /// The receptacle does not exist in the component.
    UnknownReceptacle,
}

/// Receptacle connection manager.
///
/// Tracks the connection state per `(receptacle_name, connection_id)`.
/// For multiplex receptacles multiple connections can exist in
/// parallel; for simplex receptacles `connect` is
/// only allowed in `Disconnected`.
#[derive(Default)]
pub struct ReceptacleManager {
    states: Mutex<BTreeMap<(String, u64), ConnectionState>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl core::fmt::Debug for ReceptacleManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReceptacleManager").finish()
    }
}

impl ReceptacleManager {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §6.5.2 — `connect(receptacle, ref) -> connection_id`.
    /// Returns a new `ConnectionId`; for simplex (the caller must
    /// set `is_simplex`) an already-existing connection is
    /// rejected as `AlreadyConnected`.
    ///
    /// # Errors
    /// `ConnectionError::AlreadyConnected` if the simplex receptacle
    /// is already connected.
    pub fn connect(&self, receptacle: &str, is_simplex: bool) -> Result<u64, ConnectionError> {
        let mut g = self
            .states
            .lock()
            .map_err(|_| ConnectionError::UnknownReceptacle)?;
        if is_simplex
            && g.iter()
                .any(|((n, _), st)| n == receptacle && *st == ConnectionState::Connected)
        {
            return Err(ConnectionError::AlreadyConnected);
        }
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        g.insert((receptacle.to_string(), id), ConnectionState::Connected);
        Ok(id)
    }

    /// Spec §6.5.2 — `disconnect(receptacle, connection_id)`.
    ///
    /// # Errors
    /// `ConnectionError::NoConnection` if the connection does not
    /// exist or was already disconnected.
    pub fn disconnect(&self, receptacle: &str, id: u64) -> Result<(), ConnectionError> {
        let mut g = self
            .states
            .lock()
            .map_err(|_| ConnectionError::UnknownReceptacle)?;
        let key = (receptacle.to_string(), id);
        match g.get(&key) {
            Some(ConnectionState::Connected) => {
                g.insert(key, ConnectionState::Disconnected);
                Ok(())
            }
            _ => Err(ConnectionError::NoConnection),
        }
    }

    /// Number of active connections (state = Connected).
    pub fn active_connections(&self, receptacle: &str) -> usize {
        self.states.lock().map_or(0, |g| {
            g.iter()
                .filter(|((n, _), st)| n == receptacle && **st == ConnectionState::Connected)
                .count()
        })
    }
}

// ---------------------------------------------------------------------------
// §6.10 Configuration with Attributes — Configurator
// ---------------------------------------------------------------------------

/// Spec §6.10 — configuration errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The attribute does not exist in the component definition.
    UnknownAttribute(String),
    /// The attribute is read-only and cannot be set.
    ReadOnly(String),
    /// Value conversion failed.
    InvalidValue(String),
}

/// Spec §6.10 — configurator interface.
///
/// A configurator sets attributes on a component instance,
/// typically before `configuration_complete()`. We model
/// this as a trait, so that callers can register their own
/// configurators.
pub trait Configurator: Send + Sync {
    /// Sets an attribute.
    ///
    /// # Errors
    /// See [`ConfigError`].
    fn set_attribute(&self, name: &str, value: &[u8]) -> Result<(), ConfigError>;

    /// Reads an attribute.
    ///
    /// # Errors
    /// `ConfigError::UnknownAttribute` if not set.
    fn get_attribute(&self, name: &str) -> Result<Vec<u8>, ConfigError>;
}

/// Default configurator implementation based on the
/// `ComponentDef::attributes` list with BTreeMap storage.
pub struct StandardConfigurator {
    schema: Vec<AttributeDef>,
    values: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl core::fmt::Debug for StandardConfigurator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StandardConfigurator")
            .field("attribute_count", &self.schema.len())
            .finish()
    }
}

impl StandardConfigurator {
    /// Constructor — derives the schema from `ComponentDef::attributes`.
    #[must_use]
    pub fn new(component: &ComponentDef) -> Self {
        Self {
            schema: component.attributes.clone(),
            values: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Configurator for StandardConfigurator {
    fn set_attribute(&self, name: &str, value: &[u8]) -> Result<(), ConfigError> {
        let attr = self
            .schema
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| ConfigError::UnknownAttribute(name.to_string()))?;
        if attr.readonly {
            return Err(ConfigError::ReadOnly(name.to_string()));
        }
        if let Ok(mut g) = self.values.lock() {
            g.insert(name.to_string(), value.to_vec());
            Ok(())
        } else {
            Err(ConfigError::InvalidValue(name.to_string()))
        }
    }

    fn get_attribute(&self, name: &str) -> Result<Vec<u8>, ConfigError> {
        let g = self
            .values
            .lock()
            .map_err(|_| ConfigError::UnknownAttribute(name.to_string()))?;
        g.get(name)
            .cloned()
            .ok_or_else(|| ConfigError::UnknownAttribute(name.to_string()))
    }
}

/// Configurator registry for container-wide configurator lookup.
#[derive(Default)]
pub struct ConfiguratorRegistry {
    by_repo_id: Mutex<BTreeMap<String, Arc<dyn Configurator>>>,
}

impl core::fmt::Debug for ConfiguratorRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConfiguratorRegistry").finish()
    }
}

impl ConfiguratorRegistry {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a configurator for a component repo ID.
    pub fn register(&self, repo_id: &str, c: Arc<dyn Configurator>) {
        if let Ok(mut g) = self.by_repo_id.lock() {
            g.insert(repo_id.to_string(), c);
        }
    }

    /// Returns the configurator for a repo ID.
    pub fn get(&self, repo_id: &str) -> Option<Arc<dyn Configurator>> {
        self.by_repo_id
            .lock()
            .ok()
            .and_then(|g| g.get(repo_id).cloned())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::component_def::{AttributeDef, ComponentDef};

    fn sample_component() -> ComponentDef {
        ComponentDef {
            name: "Sample".into(),
            repository_id: "IDL:demo/Sample:1.0".into(),
            base_component: None,
            supported_interfaces: alloc::vec![],
            facets: alloc::vec![],
            receptacles: alloc::vec![],
            event_sources: alloc::vec![],
            event_sinks: alloc::vec![],
            attributes: alloc::vec![
                AttributeDef {
                    name: "rate".into(),
                    type_spec: "long".into(),
                    readonly: false,
                    set_raises: alloc::vec![],
                    get_raises: alloc::vec![],
                },
                AttributeDef {
                    name: "version".into(),
                    type_spec: "string".into(),
                    readonly: true,
                    set_raises: alloc::vec![],
                    get_raises: alloc::vec![],
                },
            ],
            primary_key: alloc::vec![],
        }
    }

    // §6.4.2 Facet-Lifetime
    #[test]
    fn facet_lifetime_passes_when_alive() {
        assert!(check_facet_lifetime(true, true).is_ok());
    }

    #[test]
    fn facet_lifetime_rejects_use_after_destroy() {
        assert_eq!(
            check_facet_lifetime(false, true),
            Err(FacetLifetimeViolation::UseAfterComponentDestroy)
        );
    }

    #[test]
    fn facet_lifetime_rejects_orphaned() {
        assert_eq!(
            check_facet_lifetime(true, false),
            Err(FacetLifetimeViolation::OrphanedFacetReference)
        );
    }

    // §6.5.2 Receptacle-State-Machine
    #[test]
    fn receptacle_simplex_connect_then_disconnect() {
        let m = ReceptacleManager::new();
        let id = m.connect("port", true).expect("ok");
        assert_eq!(m.active_connections("port"), 1);
        m.disconnect("port", id).expect("ok");
        assert_eq!(m.active_connections("port"), 0);
    }

    #[test]
    fn receptacle_simplex_double_connect_rejected() {
        let m = ReceptacleManager::new();
        let _ = m.connect("port", true).expect("ok");
        assert_eq!(
            m.connect("port", true),
            Err(ConnectionError::AlreadyConnected)
        );
    }

    #[test]
    fn receptacle_multiplex_allows_multiple_connects() {
        let m = ReceptacleManager::new();
        let _ = m.connect("multi", false).expect("ok");
        let _ = m.connect("multi", false).expect("ok");
        assert_eq!(m.active_connections("multi"), 2);
    }

    #[test]
    fn receptacle_disconnect_unknown_id_rejected() {
        let m = ReceptacleManager::new();
        assert_eq!(
            m.disconnect("port", 999),
            Err(ConnectionError::NoConnection)
        );
    }

    #[test]
    fn receptacle_double_disconnect_rejected() {
        let m = ReceptacleManager::new();
        let id = m.connect("port", true).expect("ok");
        m.disconnect("port", id).expect("ok");
        assert_eq!(m.disconnect("port", id), Err(ConnectionError::NoConnection));
    }

    // §6.10 Configurator
    #[test]
    fn configurator_set_get_roundtrip() {
        let c = StandardConfigurator::new(&sample_component());
        c.set_attribute("rate", b"42").expect("ok");
        assert_eq!(c.get_attribute("rate").expect("ok"), b"42");
    }

    #[test]
    fn configurator_rejects_unknown_attribute() {
        let c = StandardConfigurator::new(&sample_component());
        assert!(matches!(
            c.set_attribute("bogus", b"x"),
            Err(ConfigError::UnknownAttribute(_))
        ));
    }

    #[test]
    fn configurator_rejects_readonly_attribute() {
        let c = StandardConfigurator::new(&sample_component());
        assert!(matches!(
            c.set_attribute("version", b"x"),
            Err(ConfigError::ReadOnly(_))
        ));
    }

    #[test]
    fn configurator_get_unknown_returns_unknown_attribute() {
        let c = StandardConfigurator::new(&sample_component());
        assert!(matches!(
            c.get_attribute("never_set"),
            Err(ConfigError::UnknownAttribute(_))
        ));
    }

    #[test]
    fn configurator_registry_register_and_lookup() {
        let r = ConfiguratorRegistry::new();
        let c: Arc<dyn Configurator> = Arc::new(StandardConfigurator::new(&sample_component()));
        r.register("IDL:demo/Sample:1.0", c);
        assert!(r.get("IDL:demo/Sample:1.0").is_some());
        assert!(r.get("IDL:demo/Other:1.0").is_none());
    }
}
