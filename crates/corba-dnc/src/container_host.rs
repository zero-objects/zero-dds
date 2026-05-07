// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ContainerHost — bindet einen [`zerodds_corba_ccm::container::Container`] an
//! einen D&C-Plan-Application-Run.
//!
//! Spec D&C §11 (Container Programming Model) legt fest, dass jede
//! Component-Instance einen Container braucht. ContainerHost ist die
//! Brücke: nimmt den Plan-Output (`NodeApplication`) entgegen und
//! installiert die Component-Instances in einen CCM-Container.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ccm::cidl::CompositionCategory;
use zerodds_corba_ccm::cif::ComponentExecutor;
use zerodds_corba_ccm::container::{Container, ContainerType, LifecycleState};
use zerodds_corba_ccm::context::ComponentContext;

/// ContainerHost-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// Instance schon installiert.
    AlreadyInstalled(String),
    /// Container-Operation lieferte Fehler.
    ContainerError(String),
}

impl core::fmt::Display for HostError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AlreadyInstalled(s) => write!(f, "instance `{s}` already installed"),
            Self::ContainerError(s) => write!(f, "container error: {s}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HostError {}

/// Default-Executor — fuer Plan-Boot, wenn der Caller keine konkrete
/// Executor-Implementation reicht. Implementiert `ComponentExecutor`
/// mit Default-Verhalten.
#[derive(Default)]
pub struct PlanExecutor {
    ctx: Option<Box<dyn ComponentContext>>,
}

impl core::fmt::Debug for PlanExecutor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PlanExecutor")
            .field("has_context", &self.ctx.is_some())
            .finish()
    }
}

impl ComponentExecutor for PlanExecutor {
    fn set_context(&mut self, context: Box<dyn ComponentContext>) {
        self.ctx = Some(context);
    }
}

/// Default-Context — anonyme Identity.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnonContext;

impl ComponentContext for AnonContext {
    fn get_caller_principal(&self) -> Option<Vec<u8>> {
        None
    }
}

/// ContainerHost — pro Node ein Host, der je `CompositionCategory`
/// einen Container haelt.
#[derive(Debug, Default)]
pub struct ContainerHost {
    containers: BTreeMap<ContainerType, Container>,
    instances: BTreeMap<String, ContainerType>,
}

impl ContainerHost {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn category_to_type(c: CompositionCategory) -> ContainerType {
        match c {
            CompositionCategory::Session => ContainerType::Session,
            CompositionCategory::Service => ContainerType::Service,
            CompositionCategory::Process => ContainerType::Process,
            CompositionCategory::Entity => ContainerType::Entity,
        }
    }

    fn ensure_container(&mut self, kind: ContainerType) -> &Container {
        self.containers
            .entry(kind)
            .or_insert_with(|| Container::new(kind))
    }

    /// Installiert eine Component-Instance mit Default-Executor +
    /// Anon-Context. Caller, der einen konkreten Executor hat, kann
    /// `install_with` benutzen.
    ///
    /// # Errors
    /// `HostError::AlreadyInstalled` wenn die Instance schon existiert.
    pub fn install(
        &mut self,
        instance_name: &str,
        category: CompositionCategory,
    ) -> Result<(), HostError> {
        self.install_with(
            instance_name,
            category,
            Box::<PlanExecutor>::default(),
            Box::new(AnonContext),
        )
    }

    /// Installiert mit konkretem Executor + Context.
    ///
    /// # Errors
    /// `HostError::AlreadyInstalled` wenn die Instance schon existiert.
    pub fn install_with(
        &mut self,
        instance_name: &str,
        category: CompositionCategory,
        executor: Box<dyn ComponentExecutor>,
        context: Box<dyn ComponentContext>,
    ) -> Result<(), HostError> {
        if self.instances.contains_key(instance_name) {
            return Err(HostError::AlreadyInstalled(instance_name.into()));
        }
        let kind = Self::category_to_type(category);
        let c = self.ensure_container(kind);
        c.install_component(instance_name.into(), executor, context)
            .map_err(|e| HostError::ContainerError(format_cif(&e)))?;
        self.instances.insert(instance_name.into(), kind);
        Ok(())
    }

    /// Aktiviert eine Instance — `ccm_activate`.
    ///
    /// # Errors
    /// Wenn die Instance unbekannt oder die Container-Transition
    /// fehlschlaegt.
    pub fn activate(&self, instance_name: &str) -> Result<(), HostError> {
        let kind = *self.instances.get(instance_name).ok_or_else(|| {
            HostError::ContainerError(alloc::format!("unknown instance `{instance_name}`"))
        })?;
        let c = self
            .containers
            .get(&kind)
            .ok_or_else(|| HostError::ContainerError("container vanished".into()))?;
        c.activate(instance_name)
            .map_err(|e| HostError::ContainerError(format_cif(&e)))
    }

    /// Passiviert eine Instance — `ccm_passivate`.
    ///
    /// # Errors
    /// Wenn die Instance unbekannt oder die Container-Transition
    /// fehlschlaegt.
    pub fn passivate(&self, instance_name: &str) -> Result<(), HostError> {
        let kind = *self.instances.get(instance_name).ok_or_else(|| {
            HostError::ContainerError(alloc::format!("unknown instance `{instance_name}`"))
        })?;
        let c = self
            .containers
            .get(&kind)
            .ok_or_else(|| HostError::ContainerError("container vanished".into()))?;
        c.passivate(instance_name)
            .map_err(|e| HostError::ContainerError(format_cif(&e)))
    }

    /// Tear-down einer Instance — `ccm_remove`.
    ///
    /// # Errors
    /// Wenn die Instance unbekannt oder die Container-Transition
    /// fehlschlaegt.
    pub fn remove(&mut self, instance_name: &str) -> Result<(), HostError> {
        let kind = self.instances.remove(instance_name).ok_or_else(|| {
            HostError::ContainerError(alloc::format!("unknown instance `{instance_name}`"))
        })?;
        let c = self
            .containers
            .get(&kind)
            .ok_or_else(|| HostError::ContainerError("container vanished".into()))?;
        c.remove(instance_name)
            .map_err(|e| HostError::ContainerError(format_cif(&e)))
    }

    /// Aktueller Lifecycle-State einer Instance.
    #[must_use]
    pub fn state(&self, instance_name: &str) -> Option<LifecycleState> {
        let kind = self.instances.get(instance_name)?;
        self.containers.get(kind)?.state_of(instance_name)
    }

    /// Liste aller Instance-Namen.
    #[must_use]
    pub fn instances(&self) -> Vec<String> {
        self.instances.keys().cloned().collect()
    }
}

fn format_cif<E: core::fmt::Debug>(e: &E) -> String {
    alloc::format!("{e:?}")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn install_creates_container_on_demand() {
        let mut host = ContainerHost::new();
        host.install("e1", CompositionCategory::Session).unwrap();
        assert!(host.instances().contains(&"e1".to_string()));
        assert_eq!(host.state("e1"), Some(LifecycleState::Configured));
    }

    #[test]
    fn install_then_activate_reaches_active() {
        let mut host = ContainerHost::new();
        host.install("e1", CompositionCategory::Session).unwrap();
        host.activate("e1").unwrap();
        assert_eq!(host.state("e1"), Some(LifecycleState::Active));
    }

    #[test]
    fn full_lifecycle_round_trip() {
        let mut host = ContainerHost::new();
        host.install("e1", CompositionCategory::Session).unwrap();
        host.activate("e1").unwrap();
        host.passivate("e1").unwrap();
        assert_eq!(host.state("e1"), Some(LifecycleState::Passive));
        host.remove("e1").unwrap();
        assert!(host.state("e1").is_none());
    }

    #[test]
    fn duplicate_install_rejected() {
        let mut host = ContainerHost::new();
        host.install("e1", CompositionCategory::Session).unwrap();
        let err = host
            .install("e1", CompositionCategory::Session)
            .unwrap_err();
        assert!(matches!(err, HostError::AlreadyInstalled(_)));
    }

    #[test]
    fn remove_unknown_fails_cleanly() {
        let mut host = ContainerHost::new();
        assert!(host.remove("nope").is_err());
    }

    #[test]
    fn entity_and_session_share_host() {
        let mut host = ContainerHost::new();
        host.install("a", CompositionCategory::Session).unwrap();
        host.install("b", CompositionCategory::Entity).unwrap();
        assert_eq!(host.instances().len(), 2);
    }
}
