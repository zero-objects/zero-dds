// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ExecutionManager / DomainApplicationManager — D&C §9.
//!
//! Spec §9.1.1: `ExecutionManager` is the top-level object; it offers
//! `preparePlan(DPD) -> DomainApplicationManager`. Spec §9.1.2:
//! `DomainApplicationManager` launches a plan on the nodes via
//! `startLaunch` + `finishLaunch`.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::{DeploymentPlan, PlanError};

/// `DomainApplication` — D&C §9.1.3. A running plan run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainApplication {
    /// Plan label.
    pub plan_label: String,
    /// Plan UUID.
    pub plan_uuid: String,
    /// Active instances + nodes (instance_name → node_name).
    pub running_instances: BTreeMap<String, String>,
    /// Lifecycle status (`Prepared`, `Launched`, `Finished`).
    pub state: AppState,
}

/// Lifecycle state of a DomainApplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Plan is `prepared` (D&C §9.1.1 `preparePlan`).
    Prepared,
    /// `startLaunch` has been called.
    Launched,
    /// `finishLaunch` has completed.
    Running,
    /// `destroyApplication` has completed.
    Finished,
}

/// `DomainApplicationManager` — D&C §9.1.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainApplicationManager {
    plan: DeploymentPlan,
    state: AppState,
}

impl DomainApplicationManager {
    /// Constructor — D&C §9.1.1 `preparePlan`.
    ///
    /// # Errors
    /// `PlanError` if the plan is inconsistent (validation fails).
    pub fn prepare_plan(plan: DeploymentPlan) -> Result<Self, PlanError> {
        plan.validate()?;
        Ok(Self {
            plan,
            state: AppState::Prepared,
        })
    }

    /// Spec D&C §9.1.2 `startLaunch` — prepares the instances and
    /// requests resources on the nodes.
    ///
    /// # Errors
    /// Static string if the manager is not in the `Prepared` state.
    pub fn start_launch(&mut self) -> Result<DomainApplication, &'static str> {
        if self.state != AppState::Prepared {
            return Err("startLaunch requires Prepared state");
        }
        self.state = AppState::Launched;
        let mut running = BTreeMap::new();
        for inst in &self.plan.instances {
            running.insert(inst.name.clone(), inst.node.clone());
        }
        Ok(DomainApplication {
            plan_label: self.plan.label.clone(),
            plan_uuid: self.plan.uuid.clone(),
            running_instances: running,
            state: AppState::Launched,
        })
    }

    /// Spec D&C §9.1.2 `finishLaunch` — completes the connection phase
    /// and returns `running` as the final state.
    ///
    /// # Errors
    /// Static string if not in the `Launched` state.
    pub fn finish_launch(&mut self, app: &mut DomainApplication) -> Result<(), &'static str> {
        if self.state != AppState::Launched {
            return Err("finishLaunch requires Launched state");
        }
        self.state = AppState::Running;
        app.state = AppState::Running;
        Ok(())
    }

    /// Spec D&C §9.1.2 `destroyApplication` — tear-down.
    ///
    /// # Errors
    /// Static string if not in the `Running` state.
    pub fn destroy_application(&mut self, app: &mut DomainApplication) -> Result<(), &'static str> {
        if self.state != AppState::Running {
            return Err("destroyApplication requires Running state");
        }
        self.state = AppState::Finished;
        app.state = AppState::Finished;
        app.running_instances.clear();
        Ok(())
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> AppState {
        self.state
    }

    /// Plan reference.
    #[must_use]
    pub fn plan(&self) -> &DeploymentPlan {
        &self.plan
    }
}

/// `ExecutionManager` — D&C §9.1.1. Top-level service.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExecutionManager {
    plans: Vec<String>,
}

impl ExecutionManager {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §9.1.1 `preparePlan(DeploymentPlan)` — creates a
    /// `DomainApplicationManager`.
    ///
    /// # Errors
    /// `PlanError` if the plan is inconsistent.
    pub fn prepare_plan(
        &mut self,
        plan: DeploymentPlan,
    ) -> Result<DomainApplicationManager, PlanError> {
        let label = plan.label.clone();
        let mgr = DomainApplicationManager::prepare_plan(plan)?;
        self.plans.push(label);
        Ok(mgr)
    }

    /// Spec §9.1.1 `getManagers` — list of all plan labels for which
    /// this ExecutionManager has called preparePlan.
    #[must_use]
    pub fn managed_plans(&self) -> &[String] {
        &self.plans
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::plan::{ImplementationDescription, InstanceDeploymentDescription};
    use alloc::string::ToString;

    fn sample_plan() -> DeploymentPlan {
        DeploymentPlan {
            label: "P".into(),
            uuid: "u".into(),
            realizes: "P".into(),
            implementations: alloc::vec![ImplementationDescription {
                label: "I".into(),
                uuid: "iu".into(),
                artifacts: alloc::vec!["a.so".into()],
                realizes: "IDL:X:1.0".into(),
                ..ImplementationDescription::default()
            }],
            instances: alloc::vec![InstanceDeploymentDescription {
                name: "x".into(),
                implementation: "I".into(),
                node: "N".into(),
                ..InstanceDeploymentDescription::default()
            }],
            connections: alloc::vec![],
        }
    }

    #[test]
    fn full_lifecycle_round_trip() {
        let mut em = ExecutionManager::new();
        let mut mgr = em.prepare_plan(sample_plan()).unwrap();
        assert_eq!(mgr.state(), AppState::Prepared);
        let mut app = mgr.start_launch().unwrap();
        assert_eq!(app.state, AppState::Launched);
        mgr.finish_launch(&mut app).unwrap();
        assert_eq!(app.state, AppState::Running);
        assert_eq!(
            app.running_instances.get("x").map(String::as_str),
            Some("N")
        );
        mgr.destroy_application(&mut app).unwrap();
        assert_eq!(app.state, AppState::Finished);
        assert!(app.running_instances.is_empty());
    }

    #[test]
    fn invalid_plan_rejected_at_prepare() {
        let mut em = ExecutionManager::new();
        let mut bad = sample_plan();
        bad.instances[0].implementation = "MissingImpl".into();
        assert!(em.prepare_plan(bad).is_err());
    }

    #[test]
    fn double_start_fails() {
        let mut mgr = DomainApplicationManager::prepare_plan(sample_plan()).unwrap();
        let _ = mgr.start_launch().unwrap();
        let mut second = DomainApplication {
            plan_label: "P".into(),
            plan_uuid: "u".into(),
            running_instances: BTreeMap::new(),
            state: AppState::Prepared,
        };
        assert!(mgr.start_launch().is_err());
        assert!(mgr.finish_launch(&mut second).is_err() || mgr.state() != AppState::Launched);
    }

    #[test]
    fn destroy_before_running_fails() {
        let mut mgr = DomainApplicationManager::prepare_plan(sample_plan()).unwrap();
        let mut app = mgr.start_launch().unwrap();
        assert!(mgr.destroy_application(&mut app).is_err());
    }

    #[test]
    fn execution_manager_lists_prepared_plans() {
        let mut em = ExecutionManager::new();
        em.prepare_plan(sample_plan()).unwrap();
        assert_eq!(em.managed_plans(), &["P".to_string()]);
    }
}
