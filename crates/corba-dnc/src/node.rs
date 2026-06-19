// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! NodeManager / NodeApplicationManager — D&C §9.2.
//!
//! Spec §9.2: a `NodeManager` lives on each deployment node and is
//! invoked by the `DomainApplicationManager` (per plan) to bring up
//! local instances.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::InstanceDeploymentDescription;

/// `NodeApplication` — D&C §9.2.3.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeApplication {
    /// Node name.
    pub node: String,
    /// List of local instances.
    pub instances: Vec<InstanceDeploymentDescription>,
    /// Active (post-launch) instance names.
    pub active: Vec<String>,
}

/// `NodeApplicationManager` — D&C §9.2.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeApplicationManager {
    node: String,
    instances: Vec<InstanceDeploymentDescription>,
}

impl NodeApplicationManager {
    /// Constructor.
    #[must_use]
    pub fn new(node: String, instances: Vec<InstanceDeploymentDescription>) -> Self {
        Self { node, instances }
    }

    /// Spec §9.2.2 `startLaunch` — brings the instances online.
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
    /// Node name.
    pub name: String,
    /// Active NodeApplications.
    apps: BTreeMap<String, NodeApplication>,
}

impl NodeManager {
    /// Constructor.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            apps: BTreeMap::new(),
        }
    }

    /// Spec §9.2.1 `preparePlan` — creates a `NodeApplicationManager`
    /// for a list of instances (those assigned to this node).
    #[must_use]
    pub fn prepare_plan(
        &self,
        instances: Vec<InstanceDeploymentDescription>,
    ) -> NodeApplicationManager {
        NodeApplicationManager::new(self.name.clone(), instances)
    }

    /// Registers a `NodeApplication` (the caller maintains this after
    /// `start_launch` has been called).
    pub fn register_application(&mut self, plan_label: String, app: NodeApplication) {
        self.apps.insert(plan_label, app);
    }

    /// Returns the active application plan labels on this node.
    #[must_use]
    pub fn active_plans(&self) -> Vec<String> {
        self.apps.keys().cloned().collect()
    }

    /// Pops a NodeApplication at tear-down.
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
        assert_eq!(app.instances.len(), 1, "instance metadata is preserved");
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
