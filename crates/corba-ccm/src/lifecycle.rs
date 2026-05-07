// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CCM 4.0 Lifecycle-Constraints + Receptacle-State-Machine +
//! Configurator-Iface — Spec §6.4.2 / §6.5.2 / §6.10.
//!
//! Diese Module decken die Datenmodell-Seite der Lifecycle-Regeln
//! ab. Die echte Container-Runtime-Durchsetzung erfolgt in
//! `container.rs`; wir liefern hier die Constraint-Validators und
//! State-Machinen, die diese Runtime nutzen kann.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use crate::component_def::{AttributeDef, ComponentDef};

// ---------------------------------------------------------------------------
// §6.4.2 Semantics of Facet References — Lifecycle-Constraints
// ---------------------------------------------------------------------------

/// Spec §6.4.2 — Facet-Lifetime-Constraint.
///
/// "The lifetime of a facet [...] is bound to the lifetime of the
/// component instance that provides it." Wir modellieren das als
/// Constraint-Check: ein Facet-Reference darf NICHT laenger leben
/// als die Component-Instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacetLifetimeViolation {
    /// Facet wurde nach Component-Destroy noch verwendet.
    UseAfterComponentDestroy,
    /// Facet-Reference ist orphaned (keine Component-Backref).
    OrphanedFacetReference,
}

/// Validiert eine Facet-Reference gegen den Component-Lifecycle.
///
/// `component_alive` muss `true` sein wenn die Component-Instance
/// noch existiert; `facet_in_component` muss `true` sein wenn das
/// Facet wirklich zur Component gehoert.
///
/// # Errors
/// `FacetLifetimeViolation` wenn das Constraint verletzt ist.
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

/// Spec §6.5.2 — Connection-State-Machine fuer Receptacles.
///
/// Ein Receptacle durchlaeuft die States `Disconnected → Connected →
/// Disconnected`. `connect()` bei Simplex-Multiplicity ist nur in
/// `Disconnected` erlaubt; `disconnect()` nur in `Connected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Kein Provider gebunden.
    Disconnected,
    /// Provider gebunden (Object-Reference vorhanden).
    Connected,
}

/// Connection-Lifecycle-Errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionError {
    /// `connect` auf einem bereits verbundenen Simplex-Receptacle.
    AlreadyConnected,
    /// `disconnect` auf einem ungebundenen Receptacle.
    NoConnection,
    /// Receptacle existiert nicht in der Component.
    UnknownReceptacle,
}

/// Receptacle-Connection-Manager.
///
/// Pflegt pro `(receptacle_name, connection_id)` den Connection-
/// State. Bei Multiplex-Receptacles koennen mehrere Connections
/// parallel existieren; bei Simplex-Receptacles ist `connect`
/// nur bei `Disconnected` erlaubt.
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
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §6.5.2 — `connect(receptacle, ref) -> connection_id`.
    /// Liefert eine neue `ConnectionId`; bei Simplex (caller muss
    /// `is_simplex` setzen) wird ein bereits existierender Connect
    /// als `AlreadyConnected` rejected.
    ///
    /// # Errors
    /// `ConnectionError::AlreadyConnected` wenn Simplex-Receptacle
    /// schon verbunden.
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
    /// `ConnectionError::NoConnection` wenn die Connection nicht
    /// existiert oder bereits getrennt wurde.
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

    /// Anzahl aktive Connections (state = Connected).
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

/// Spec §6.10 — Configuration-Errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Attribut existiert nicht in der Component-Definition.
    UnknownAttribute(String),
    /// Attribut ist read-only und kann nicht gesetzt werden.
    ReadOnly(String),
    /// Wertkonvertierung fehlgeschlagen.
    InvalidValue(String),
}

/// Spec §6.10 — Configurator-Interface.
///
/// Ein Configurator setzt Attribute auf einer Component-Instance,
/// typischerweise vor `configuration_complete()`. Wir modellieren
/// das als Trait, sodass Caller eigene Configurators registrieren
/// koennen.
pub trait Configurator: Send + Sync {
    /// Setzt ein Attribut.
    ///
    /// # Errors
    /// Siehe [`ConfigError`].
    fn set_attribute(&self, name: &str, value: &[u8]) -> Result<(), ConfigError>;

    /// Liest ein Attribut.
    ///
    /// # Errors
    /// `ConfigError::UnknownAttribute` wenn nicht gesetzt.
    fn get_attribute(&self, name: &str) -> Result<Vec<u8>, ConfigError>;
}

/// Default-Configurator-Implementation auf Basis der
/// `ComponentDef::attributes`-Liste mit BTreeMap-Storage.
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
    /// Konstruktor — leitet Schema aus `ComponentDef::attributes` ab.
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

/// Configurator-Registry fuer Container-Wide Configurator-Lookup.
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
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert einen Configurator fuer eine Component-Repo-ID.
    pub fn register(&self, repo_id: &str, c: Arc<dyn Configurator>) {
        if let Ok(mut g) = self.by_repo_id.lock() {
            g.insert(repo_id.to_string(), c);
        }
    }

    /// Liefert den Configurator fuer eine Repo-ID.
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
