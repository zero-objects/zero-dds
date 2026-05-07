// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! NodeManager / NodeApplicationManager — D&C §9.2.
//!
//! Spec §9.2: ein `NodeManager` lebt auf jedem Deployment-Node und
//! wird vom `DomainApplicationManager` (per Plan) gerufen, um lokale
//! Instances aufzubringen.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::InstanceDeploymentDescription;

/// `NodeApplication` — D&C §9.2.3.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeApplication {
    /// Node-Name.
    pub node: String,
    /// Liste der lokalen Instances.
    pub instances: Vec<InstanceDeploymentDescription>,
    /// Aktive (post-launch) Instance-Namen.
    pub active: Vec<String>,
}

/// `NodeApplicationManager` — D&C §9.2.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeApplicationManager {
    node: String,
    instances: Vec<InstanceDeploymentDescription>,
}

impl NodeApplicationManager {
    /// Konstruktor.
    #[must_use]
    pub fn new(node: String, instances: Vec<InstanceDeploymentDescription>) -> Self {
        Self { node, instances }
    }

    /// Spec §9.2.2 `startLaunch` — bringt die Instances online.
    #[must_use]
    pub fn start_launch(&self) -> NodeApplication {
        NodeApplication {
            node: self.node.clone(),
            instances: self.instances.clone(),
            active: self.instances.iter().map(|i| i.name.clone()).collect(),
        }
    }

    /// Spec §9.2.2 `destroyApplication`.
    pub fn destroy_application(app: &mut NodeApplication) {
        app.active.clear();
    }
}

/// `NodeManager` — D&C §9.2.1.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NodeManager {
    /// Node-Name.
    pub name: String,
    /// Aktive NodeApplications.
    apps: BTreeMap<String, NodeApplication>,
}

impl NodeManager {
    /// Konstruktor.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            apps: BTreeMap::new(),
        }
    }

    /// Spec §9.2.1 `preparePlan` — fuer eine Liste von Instances
    /// (die fuer diesen Node bestimmt sind) einen
    /// `NodeApplicationManager` erzeugen.
    #[must_use]
    pub fn prepare_plan(
        &self,
        instances: Vec<InstanceDeploymentDescription>,
    ) -> NodeApplicationManager {
        NodeApplicationManager::new(self.name.clone(), instances)
    }

    /// Registriert eine `NodeApplication` (Caller pflegt das, nachdem
    /// `start_launch` aufgerufen wurde).
    pub fn register_application(&mut self, plan_label: String, app: NodeApplication) {
        self.apps.insert(plan_label, app);
    }

    /// Liefert die aktiven Application-Plan-Labels auf diesem Node.
    #[must_use]
    pub fn active_plans(&self) -> Vec<String> {
        self.apps.keys().cloned().collect()
    }

    /// Pop eine NodeApplication beim Tear-down.
    pub fn unregister_application(&mut self, plan_label: &str) -> Option<NodeApplication> {
        self.apps.remove(plan_label)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn make_inst(name: &str) -> InstanceDeploymentDescription {
        InstanceDeploymentDescription {
            name: name.into(),
            implementation: "I".into(),
            node: "N1".into(),
            ..InstanceDeploymentDescription::default()
        }
    }

    #[test]
    fn prepare_plan_then_start_launch() {
        let nm = NodeManager::new("N1".into());
        let app_mgr = nm.prepare_plan(alloc::vec![make_inst("a"), make_inst("b")]);
        let app = app_mgr.start_launch();
        assert_eq!(app.node, "N1");
        assert_eq!(app.active.len(), 2);
        assert!(app.active.iter().any(|s| s == "a"));
        assert!(app.active.iter().any(|s| s == "b"));
    }

    #[test]
    fn destroy_application_clears_active() {
        let nm = NodeManager::new("N1".into());
        let mut app = nm.prepare_plan(alloc::vec![make_inst("a")]).start_launch();
        NodeApplicationManager::destroy_application(&mut app);
        assert!(app.active.is_empty());
        assert_eq!(app.instances.len(), 1, "instance metadata bleibt erhalten");
    }

    #[test]
    fn register_and_unregister_application() {
        let mut nm = NodeManager::new("N1".into());
        let app = nm.prepare_plan(alloc::vec![make_inst("a")]).start_launch();
        nm.register_application("Plan1".into(), app);
        assert_eq!(nm.active_plans(), alloc::vec!["Plan1".to_string()]);
        let popped = nm.unregister_application("Plan1");
        assert!(popped.is_some());
        assert!(nm.active_plans().is_empty());
    }

    #[test]
    fn unregister_unknown_returns_none() {
        let mut nm = NodeManager::new("N".into());
        assert!(nm.unregister_application("nope").is_none());
    }
}
