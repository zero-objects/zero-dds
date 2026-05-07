// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CCM-Container — Spec §9.
//!
//! Der Container haeltet die Component-Lifecycle, ruft die CIF-
//! Lifecycle-Methods (`set_*_context`/`ccm_activate`/`ccm_passivate`/
//! `ccm_remove`) und liefert `ComponentContext` an den Executor.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use std::sync::Mutex;

use crate::cif::{CifError, ComponentExecutor};
use crate::port::PortRegistry;

/// Container-Type — Spec §9.1.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContainerType {
    /// `Session` — non-persistent stateful.
    Session,
    /// `Service` — stateless.
    Service,
    /// `Process` — long-running.
    Process,
    /// `Entity` — persistent.
    Entity,
}

/// Lifecycle-State eines Component-Executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    /// Erstellt, aber noch nicht initialisiert.
    Created,
    /// `set_*_context` aufgerufen, vor `ccm_activate`.
    Configured,
    /// `ccm_activate` aufgerufen.
    Active,
    /// `ccm_passivate` aufgerufen.
    Passive,
    /// `ccm_remove` aufgerufen — terminal.
    Removed,
}

/// Container-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    /// Component-Instance unbekannt.
    InstanceNotFound(String),
    /// Operation passt nicht zum aktuellen Lifecycle-State.
    InvalidState {
        /// Aktueller State.
        current: LifecycleState,
        /// Welche Operation versucht wurde.
        operation: String,
    },
    /// Cif-Fehler aus dem Executor.
    Cif(CifError),
}

impl From<CifError> for ContainerError {
    fn from(e: CifError) -> Self {
        Self::Cif(e)
    }
}

struct InstanceEntry {
    state: LifecycleState,
    executor: Box<dyn ComponentExecutor>,
}

/// CCM-Container.
pub struct Container {
    container_type: ContainerType,
    instances: Mutex<BTreeMap<String, InstanceEntry>>,
    /// Port-Registry, geshared mit Caller.
    pub ports: Arc<PortRegistry>,
}

impl core::fmt::Debug for Container {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.instances.lock().ok().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("Container")
            .field("type", &self.container_type)
            .field("instances", &n)
            .finish()
    }
}

impl Container {
    /// Konstruktor.
    #[must_use]
    pub fn new(container_type: ContainerType) -> Self {
        Self {
            container_type,
            instances: Mutex::new(BTreeMap::new()),
            ports: Arc::new(PortRegistry::new()),
        }
    }

    /// Container-Type.
    #[must_use]
    pub fn container_type(&self) -> ContainerType {
        self.container_type
    }

    /// Registriert einen neuen Component-Executor unter einer
    /// Instance-Id und konfiguriert ihn.
    ///
    /// # Errors
    /// `Cif` wenn der Executor-Setup fehlschlaegt.
    pub fn install_component(
        &self,
        instance_id: String,
        mut executor: Box<dyn ComponentExecutor>,
        context: Box<dyn crate::context::ComponentContext>,
    ) -> Result<(), ContainerError> {
        executor.set_context(context);
        let mut g = self.instances.lock().map_err(|_| {
            ContainerError::Cif(CifError::CcmException("instances mutex poisoned".into()))
        })?;
        g.insert(
            instance_id,
            InstanceEntry {
                state: LifecycleState::Configured,
                executor,
            },
        );
        Ok(())
    }

    /// `ccm_activate` — Spec §9.5.4.
    ///
    /// # Errors
    /// `InstanceNotFound` / `InvalidState` / `Cif`.
    pub fn activate(&self, instance_id: &str) -> Result<(), ContainerError> {
        self.transition(instance_id, "ccm_activate", LifecycleState::Active, |e| {
            e.ccm_activate()
        })
    }

    /// `ccm_passivate` — Spec §9.5.5.
    ///
    /// # Errors
    /// Wie oben.
    pub fn passivate(&self, instance_id: &str) -> Result<(), ContainerError> {
        self.transition(instance_id, "ccm_passivate", LifecycleState::Passive, |e| {
            e.ccm_passivate()
        })
    }

    /// `ccm_remove` — Spec §9.5.6.
    ///
    /// # Errors
    /// Wie oben.
    pub fn remove(&self, instance_id: &str) -> Result<(), ContainerError> {
        self.transition(instance_id, "ccm_remove", LifecycleState::Removed, |e| {
            e.ccm_remove()
        })?;
        if let Ok(mut g) = self.instances.lock() {
            g.remove(instance_id);
        }
        Ok(())
    }

    /// Anzahl Instances.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.instances.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Lifecycle-State einer Instance.
    #[must_use]
    pub fn state_of(&self, instance_id: &str) -> Option<LifecycleState> {
        self.instances
            .lock()
            .ok()
            .and_then(|g| g.get(instance_id).map(|e| e.state))
    }

    fn transition<F>(
        &self,
        instance_id: &str,
        op: &str,
        target: LifecycleState,
        f: F,
    ) -> Result<(), ContainerError>
    where
        F: FnOnce(&mut Box<dyn ComponentExecutor>) -> Result<(), CifError>,
    {
        let mut g = self.instances.lock().map_err(|_| {
            ContainerError::Cif(CifError::CcmException("instances mutex poisoned".into()))
        })?;
        let entry = g
            .get_mut(instance_id)
            .ok_or_else(|| ContainerError::InstanceNotFound(instance_id.to_string()))?;
        // Spec §9.5: erlaubte Transitions:
        // Configured -> Active (via activate)
        // Active     -> Passive (via passivate)
        // Active     -> Removed (via remove)
        // Passive    -> Active (via activate)
        // Passive    -> Removed (via remove)
        let valid = matches!(
            (entry.state, target),
            (LifecycleState::Configured, LifecycleState::Active)
                | (LifecycleState::Active, LifecycleState::Passive)
                | (LifecycleState::Passive, LifecycleState::Active)
                | (LifecycleState::Active, LifecycleState::Removed)
                | (LifecycleState::Passive, LifecycleState::Removed)
                | (LifecycleState::Configured, LifecycleState::Removed)
        );
        if !valid {
            return Err(ContainerError::InvalidState {
                current: entry.state,
                operation: op.to_string(),
            });
        }
        f(&mut entry.executor)?;
        entry.state = target;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::context::ComponentContext;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct AnonContext;
    impl ComponentContext for AnonContext {
        fn get_caller_principal(&self) -> Option<alloc::vec::Vec<u8>> {
            None
        }
    }

    struct CountingExecutor {
        ctx_set: AtomicUsize,
        activations: AtomicUsize,
        passivations: AtomicUsize,
        removals: AtomicUsize,
    }

    impl ComponentExecutor for CountingExecutor {
        fn set_context(&mut self, _: Box<dyn ComponentContext>) {
            self.ctx_set.fetch_add(1, Ordering::Relaxed);
        }
        fn ccm_activate(&mut self) -> Result<(), CifError> {
            self.activations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn ccm_passivate(&mut self) -> Result<(), CifError> {
            self.passivations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn ccm_remove(&mut self) -> Result<(), CifError> {
            self.removals.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    fn fresh_executor() -> Box<dyn ComponentExecutor> {
        Box::new(CountingExecutor {
            ctx_set: AtomicUsize::new(0),
            activations: AtomicUsize::new(0),
            passivations: AtomicUsize::new(0),
            removals: AtomicUsize::new(0),
        })
    }

    #[test]
    fn install_and_full_lifecycle_round_trip() {
        let c = Container::new(ContainerType::Session);
        c.install_component("inst-1".into(), fresh_executor(), Box::new(AnonContext))
            .unwrap();
        assert_eq!(c.state_of("inst-1"), Some(LifecycleState::Configured));
        c.activate("inst-1").unwrap();
        assert_eq!(c.state_of("inst-1"), Some(LifecycleState::Active));
        c.passivate("inst-1").unwrap();
        assert_eq!(c.state_of("inst-1"), Some(LifecycleState::Passive));
        c.activate("inst-1").unwrap(); // re-activate
        assert_eq!(c.state_of("inst-1"), Some(LifecycleState::Active));
        c.remove("inst-1").unwrap();
        assert_eq!(c.state_of("inst-1"), None); // removed.
    }

    #[test]
    fn passivate_in_configured_state_invalid() {
        let c = Container::new(ContainerType::Session);
        c.install_component("inst-1".into(), fresh_executor(), Box::new(AnonContext))
            .unwrap();
        let err = c.passivate("inst-1").unwrap_err();
        assert!(matches!(err, ContainerError::InvalidState { .. }));
    }

    #[test]
    fn unknown_instance_returns_not_found() {
        let c = Container::new(ContainerType::Session);
        let err = c.activate("inst-x").unwrap_err();
        assert!(matches!(err, ContainerError::InstanceNotFound(_)));
    }

    #[test]
    fn instance_count_tracks_install_and_remove() {
        let c = Container::new(ContainerType::Service);
        c.install_component("a".into(), fresh_executor(), Box::new(AnonContext))
            .unwrap();
        c.install_component("b".into(), fresh_executor(), Box::new(AnonContext))
            .unwrap();
        assert_eq!(c.instance_count(), 2);
        c.activate("a").unwrap();
        c.remove("a").unwrap();
        assert_eq!(c.instance_count(), 1);
    }
}
