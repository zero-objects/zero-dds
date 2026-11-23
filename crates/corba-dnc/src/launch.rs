// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The launch of a plan with connector fragments — CORBA 3.3 Part 3
//! §7.5, §7.6.3.1 and §7.6.4.
//!
//! [`PlanLauncher`] does on a node what the deployment tool does for the
//! fragments of a flattened plan, in the two launch steps of D&C
//! (`NodeApplicationManager::startLaunch`, `Application::finishLaunch`):
//!
//! 1. [`PlanLauncher::start_launch`]: for each instance on the node whose
//!    implementation carries the execution parameter "home factory"
//!    (§7.6.3.1), it calls the entry point of that name, sets the
//!    instance's configuration on the home with
//!    `HomeConfiguration::set_configuration_values` and creates the
//!    fragment with `KeylessCCMHome::create_component` (§7.6.4). Every
//!    other instance is a component the container created and
//!    registered with [`PlanLauncher::register_component`]. It returns
//!    the provided references: for each plan connection whose provider
//!    (the target) is on the node, the object `provide_facet` gives.
//! 2. [`PlanLauncher::finish_launch`]: each plan connection whose user
//!    (the source) is on the node is a basic connection: `connect` on
//!    the source with the target's facet, from this node or, for a
//!    target on another node, from the provided references of that node.
//!    A connection whose user is on another node is that node's to make.
//! 3. [`PlanLauncher::start`] (`Application::start`): `configuration_complete`
//!    on every fragment and component, the handshake of §7.5.2.
//! 4. [`PlanLauncher::destroy`]: the connections are disconnected and the
//!    fragments removed with `CCMHome::remove_component`.
//!
//! [`launch_plan`] runs the steps over one launcher per node, as the
//! `DomainApplicationManager` does, so that every connection of the plan
//! is made once, by the launcher of its user's node: the connections
//! between the fragments of a composite connector (§7.2.3.4) placed on
//! different nodes among them.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_corba_ccm::fragment::{
    CcmError, CcmObject, ConfigValue, ConnectorFragment, ConnectorHome, Cookie, HomeFactories,
};
use zerodds_corba_ccm::orb_core::ObjectRef;
use zerodds_corba_rt::CorbaAny;

use crate::connector::HOME_FACTORY;
use crate::plan::{DeploymentPlan, PlanError};

/// The object references the instances of one node provide to the
/// connections of a plan, by connection name: what D&C's
/// `NodeApplicationManager::startLaunch` returns as `providedReference`
/// and `Application::finishLaunch` takes for the connections whose
/// provider is on another node.
pub type ProvidedReferences = BTreeMap<String, ObjectRef>;

/// Why a launch step failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The step is not allowed in the launcher's state.
    State(&'static str),
    /// An instance names an implementation the plan does not have.
    UnknownImplementation(String),
    /// "home factory" names an entry point the node does not know.
    UnknownEntryPoint {
        /// The instance.
        instance: String,
        /// The entry point.
        entry_point: String,
    },
    /// A component instance of the plan nobody registered.
    MissingComponent(String),
    /// The plan is inconsistent.
    Plan(PlanError),
    /// A connection whose user is on this node and whose provider is on
    /// another, without the provider's reference among the provided
    /// references.
    MissingProvidedReference(String),
    /// An instance of the plan on a node no launcher was given for.
    NoLauncher {
        /// The instance.
        instance: String,
        /// Its node.
        node: String,
    },
    /// An operation on an instance raised.
    Ccm {
        /// The instance.
        instance: String,
        /// What it raised.
        error: CcmError,
    },
}

impl core::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::State(s) => f.write_str(s),
            Self::UnknownImplementation(i) => write!(f, "unknown implementation `{i}`"),
            Self::UnknownEntryPoint {
                instance,
                entry_point,
            } => write!(f, "instance `{instance}`: no entry point `{entry_point}`"),
            Self::MissingComponent(i) => write!(f, "component instance `{i}` is not registered"),
            Self::Plan(e) => write!(f, "{e}"),
            Self::MissingProvidedReference(c) => {
                write!(f, "connection `{c}`: no reference of its provider")
            }
            Self::NoLauncher { instance, node } => {
                write!(f, "instance `{instance}`: no launcher for node `{node}`")
            }
            Self::Ccm { instance, error } => write!(f, "instance `{instance}`: {error}"),
        }
    }
}

impl std::error::Error for LaunchError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Started,
    Connected,
    Running,
    Destroyed,
}

struct Made {
    connection: String,
    source: String,
    receptacle: String,
    cookie: Option<Cookie>,
}

/// Launches the fragments of a plan on one node and connects them.
pub struct PlanLauncher {
    factories: HomeFactories,
    components: BTreeMap<String, Arc<dyn CcmObject>>,
    fragments: BTreeMap<String, (Arc<ConnectorHome>, Arc<ConnectorFragment>)>,
    connections: Vec<Made>,
    /// The plan instances on this launcher's node.
    local: BTreeSet<String>,
    phase: Phase,
}

impl core::fmt::Debug for PlanLauncher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PlanLauncher")
            .field("components", &self.components.keys().collect::<Vec<_>>())
            .field("fragments", &self.fragments.keys().collect::<Vec<_>>())
            .field("phase", &self.phase)
            .finish()
    }
}

impl PlanLauncher {
    /// A launcher with the entry points of this node.
    #[must_use]
    pub fn new(factories: HomeFactories) -> Self {
        Self {
            factories,
            components: BTreeMap::new(),
            fragments: BTreeMap::new(),
            connections: Vec::new(),
            local: BTreeSet::new(),
            phase: Phase::Idle,
        }
    }

    /// Registers the component instance `name` the container created.
    pub fn register_component(&mut self, name: &str, object: Arc<dyn CcmObject>) {
        self.components.insert(name.to_string(), object);
    }

    /// The fragment of instance `name`, once created.
    #[must_use]
    pub fn fragment(&self, name: &str) -> Option<Arc<ConnectorFragment>> {
        self.fragments.get(name).map(|(_, f)| Arc::clone(f))
    }

    /// The names of the fragments created.
    #[must_use]
    pub fn fragment_names(&self) -> Vec<String> {
        self.fragments.keys().cloned().collect()
    }

    fn object(&self, name: &str) -> Option<Arc<dyn CcmObject>> {
        if let Some((_, f)) = self.fragments.get(name) {
            let object: Arc<dyn CcmObject> = Arc::clone(f) as Arc<dyn CcmObject>;
            return Some(object);
        }
        self.components.get(name).cloned()
    }

    /// Creates the fragments of the plan's instances on `node` (all
    /// instances for `None`) through their "home factory" entry points,
    /// each configured with its instance's `configProperty`, and returns
    /// the provided references: the facet of each plan connection whose
    /// provider is on the node, by connection name.
    ///
    /// # Errors
    /// [`LaunchError`]; a fragment's home raising `CreateFailure` and a
    /// provider without the facet are [`LaunchError::Ccm`].
    pub fn start_launch(
        &mut self,
        plan: &DeploymentPlan,
        node: Option<&str>,
    ) -> Result<ProvidedReferences, LaunchError> {
        if self.phase != Phase::Idle {
            return Err(LaunchError::State("start_launch requires a fresh launcher"));
        }
        plan.validate().map_err(LaunchError::Plan)?;
        for instance in plan
            .instances
            .iter()
            .filter(|i| node.is_none_or(|n| i.node == n))
        {
            let implementation = plan
                .implementations
                .iter()
                .find(|i| i.label == instance.implementation)
                .ok_or_else(|| {
                    LaunchError::UnknownImplementation(instance.implementation.clone())
                })?;
            self.local.insert(instance.name.clone());
            let Some(entry_point) = implementation.exec_params.get(HOME_FACTORY) else {
                if !self.components.contains_key(&instance.name) {
                    return Err(LaunchError::MissingComponent(instance.name.clone()));
                }
                continue;
            };
            let factory =
                self.factories
                    .get(entry_point)
                    .ok_or_else(|| LaunchError::UnknownEntryPoint {
                        instance: instance.name.clone(),
                        entry_point: entry_point.clone(),
                    })?;
            let home = ConnectorHome::new(factory());
            home.set_configuration_values(
                instance
                    .config_props
                    .iter()
                    .map(|p| ConfigValue {
                        name: p.name.clone(),
                        value: CorbaAny(p.value.clone()),
                    })
                    .collect(),
            );
            let fragment = home.create_component().map_err(|error| LaunchError::Ccm {
                instance: instance.name.clone(),
                error,
            })?;
            self.fragments
                .insert(instance.name.clone(), (home, fragment));
        }
        let mut provided = ProvidedReferences::new();
        for c in &plan.connections {
            if self.local.contains(&c.target_instance) {
                provided.insert(
                    c.name.clone(),
                    self.facet(&c.target_instance, &c.target_port)?,
                );
            }
        }
        self.phase = Phase::Started;
        Ok(provided)
    }

    /// The facet `port` of the local instance `instance`.
    fn facet(&self, instance: &str, port: &str) -> Result<ObjectRef, LaunchError> {
        let object = self
            .object(instance)
            .ok_or_else(|| LaunchError::MissingComponent(instance.to_string()))?;
        object
            .provide_facet(port)
            .map_err(|error| LaunchError::Ccm {
                instance: instance.to_string(),
                error,
            })
    }

    /// Makes the plan's connections whose user (the source) is on this
    /// node: the target's facet, from this node or from `provided` for a
    /// target on another node, connected to the source's receptacle. A
    /// connection whose user is on another node is made by that node's
    /// launcher ([`launch_plan`]).
    ///
    /// # Errors
    /// [`LaunchError::MissingProvidedReference`] for a remote provider
    /// `provided` has no reference of; [`LaunchError::Ccm`] for
    /// `InvalidName`, `InvalidConnection`, `AlreadyConnected` and the
    /// rest.
    pub fn finish_launch(
        &mut self,
        plan: &DeploymentPlan,
        provided: &ProvidedReferences,
    ) -> Result<(), LaunchError> {
        if self.phase != Phase::Started {
            return Err(LaunchError::State("finish_launch requires start_launch"));
        }
        for c in &plan.connections {
            if !self.local.contains(&c.source_instance) {
                continue;
            }
            let source = self
                .object(&c.source_instance)
                .ok_or_else(|| LaunchError::MissingComponent(c.source_instance.clone()))?;
            let facet = if self.local.contains(&c.target_instance) {
                self.facet(&c.target_instance, &c.target_port)?
            } else {
                provided
                    .get(&c.name)
                    .cloned()
                    .ok_or_else(|| LaunchError::MissingProvidedReference(c.name.clone()))?
            };
            let cookie =
                source
                    .connect(&c.source_port, facet)
                    .map_err(|error| LaunchError::Ccm {
                        instance: c.source_instance.clone(),
                        error,
                    })?;
            self.connections.push(Made {
                connection: c.name.clone(),
                source: c.source_instance.clone(),
                receptacle: c.source_port.clone(),
                cookie,
            });
        }
        self.phase = Phase::Connected;
        Ok(())
    }

    /// `Application::start`: `configuration_complete` on every fragment,
    /// then on every component (§7.5.2: after the configuration of all
    /// components and fragments).
    ///
    /// # Errors
    /// [`LaunchError::Ccm`] with the first `InvalidConfiguration`.
    pub fn start(&mut self) -> Result<(), LaunchError> {
        if self.phase != Phase::Connected {
            return Err(LaunchError::State("start requires finish_launch"));
        }
        for (name, (_, fragment)) in &self.fragments {
            fragment
                .configuration_complete()
                .map_err(|error| LaunchError::Ccm {
                    instance: name.clone(),
                    error,
                })?;
        }
        for (name, component) in &self.components {
            component
                .configuration_complete()
                .map_err(|error| LaunchError::Ccm {
                    instance: name.clone(),
                    error,
                })?;
        }
        self.phase = Phase::Running;
        Ok(())
    }

    /// Disconnects what [`Self::finish_launch`] connected and removes the
    /// fragments through their homes.
    ///
    /// # Errors
    /// [`LaunchError::Ccm`] with the first failure; the remaining
    /// connections and fragments are still torn down.
    pub fn destroy(&mut self) -> Result<(), LaunchError> {
        if matches!(self.phase, Phase::Idle | Phase::Destroyed) {
            return Err(LaunchError::State("destroy requires a launched plan"));
        }
        let mut first: Option<LaunchError> = None;
        for made in core::mem::take(&mut self.connections).into_iter().rev() {
            if let Some(source) = self.object(&made.source)
                && let Err(error) = source.disconnect(&made.receptacle, made.cookie.as_ref())
            {
                first.get_or_insert(LaunchError::Ccm {
                    instance: format!("{} ({})", made.source, made.connection),
                    error,
                });
            }
        }
        for (name, (home, fragment)) in core::mem::take(&mut self.fragments) {
            if let Err(error) = home.remove_component(&fragment) {
                first.get_or_insert(LaunchError::Ccm {
                    instance: name,
                    error,
                });
            }
        }
        self.phase = Phase::Destroyed;
        first.map_or(Ok(()), Err)
    }
}

/// Launches `plan` over one launcher per node, as D&C's
/// `DomainApplicationManager` does: `start_launch` on every node, the
/// provided references of all nodes gathered, `finish_launch` on every
/// node with them. Each connection of the plan is made once, by the
/// launcher of the node its user is on, whichever node its provider is
/// on.
///
/// # Errors
/// [`LaunchError::NoLauncher`] for an instance whose node has no
/// launcher; else the first [`LaunchError`] of a node.
pub fn launch_plan(
    plan: &DeploymentPlan,
    launchers: &mut BTreeMap<String, PlanLauncher>,
) -> Result<(), LaunchError> {
    for instance in &plan.instances {
        if !launchers.contains_key(&instance.node) {
            return Err(LaunchError::NoLauncher {
                instance: instance.name.clone(),
                node: instance.node.clone(),
            });
        }
    }
    let mut provided = ProvidedReferences::new();
    for (node, launcher) in launchers.iter_mut() {
        provided.extend(launcher.start_launch(plan, Some(node))?);
    }
    for launcher in launchers.values_mut() {
        launcher.finish_launch(plan, &provided)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::connector::flatten_assembly;
    use crate::connector::tests::{
        CONTROL, DATA, SOCKET, assembly, composite_assembly, nodes, port_types,
    };
    use alloc::boxed::Box;
    use std::sync::Mutex;
    use zerodds_corba_ccm::component_def::ComponentDef;
    use zerodds_corba_ccm::connector::{ConnectorDef, ConnectorMember, ConnectorTypeTable};
    use zerodds_corba_ccm::fragment::{
        ConnectorHomeExecutor, FragmentExecutor, HomeFactory, LocalFacet,
    };
    use zerodds_corba_ccm::orb_core::ObjectRef;
    use zerodds_corba_rt::any::AnyValue;

    /// What the fragment executors saw: one log per test, reached through
    /// the entry point the test registers.
    static LOG_LAUNCH: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static LOG_CONFIG: Mutex<Vec<String>> = Mutex::new(Vec::new());

    type Log = &'static Mutex<Vec<String>>;

    struct Servant;

    struct Exec {
        id: usize,
        log: Log,
    }

    impl Exec {
        fn seen(&self, entry: String) {
            self.log.lock().unwrap().push(entry);
        }
    }

    impl FragmentExecutor for Exec {
        fn provide(&self, facet: &str) -> Option<ObjectRef> {
            let type_id = if facet == "cc_control" { CONTROL } else { DATA };
            Some(LocalFacet::object(type_id, &[], Arc::new(Servant)))
        }
        fn set_attribute(&self, name: &str, value: &CorbaAny) -> Result<(), CcmError> {
            self.seen(format!("{} set {name}={:?}", self.id, value.0));
            Ok(())
        }
        fn connected(&self, receptacle: &str, _: &ObjectRef) -> Result<(), CcmError> {
            self.seen(format!("{} connected {receptacle}", self.id));
            Ok(())
        }
        fn configuration_complete(&self, fragment: &ConnectorFragment) -> Result<(), CcmError> {
            self.seen(format!(
                "{} complete with {} on cc_consumer",
                self.id,
                fragment.connected_objects("cc_consumer")?.len()
            ));
            Ok(())
        }
        fn remove(&self) -> Result<(), CcmError> {
            self.seen(format!("{} removed", self.id));
            Ok(())
        }
    }

    static NEXT: Mutex<usize> = Mutex::new(0);

    struct Home {
        log: Log,
    }

    impl ConnectorHomeExecutor for Home {
        fn ports(&self) -> ComponentDef {
            let mut table: ConnectorTypeTable = port_types();
            table.insert_connector(ConnectorDef {
                name: "Data_Cnx".into(),
                repository_id: crate::connector::tests::CONNECTOR.into(),
                base: None,
                members: alloc::vec![
                    ConnectorMember::MirrorPort {
                        name: "cc".into(),
                        port_type_id: crate::connector::tests::PORT_TYPE.into(),
                    },
                    ConnectorMember::Provides {
                        name: "p".into(),
                        interface_id: DATA.into(),
                    },
                    ConnectorMember::Attribute(attribute("mcast_addr")),
                    ConnectorMember::Attribute(attribute("mcast_port")),
                    ConnectorMember::Attribute(attribute("msg_size")),
                ],
            });
            table
                .basic_ports(crate::connector::tests::CONNECTOR)
                .unwrap()
        }
        fn create(&self) -> Result<Arc<dyn FragmentExecutor>, CcmError> {
            let mut n = NEXT.lock().unwrap();
            *n += 1;
            Ok(Arc::new(Exec {
                id: *n,
                log: self.log,
            }))
        }
    }

    fn attribute(name: &str) -> zerodds_corba_ccm::component_def::AttributeDef {
        zerodds_corba_ccm::component_def::AttributeDef {
            name: name.into(),
            type_spec: "any".into(),
            readonly: false,
            set_raises: Vec::new(),
            get_raises: Vec::new(),
        }
    }

    fn create_for_launch() -> Box<dyn ConnectorHomeExecutor> {
        Box::new(Home { log: &LOG_LAUNCH })
    }

    fn create_for_config() -> Box<dyn ConnectorHomeExecutor> {
        Box::new(Home { log: &LOG_CONFIG })
    }

    /// A component with the basic ports the flattened plan connects:
    /// the consumer's `p_consumer` facet and `p_control` receptacle, the
    /// producer's `out` receptacle.
    #[derive(Default)]
    struct Component {
        connected: Mutex<Vec<String>>,
        completed: Mutex<bool>,
    }

    impl CcmObject for Component {
        fn provide_facet(&self, name: &str) -> Result<ObjectRef, CcmError> {
            match name {
                "p_consumer" => Ok(LocalFacet::object(DATA, &[], Arc::new(Servant))),
                _ => Err(CcmError::InvalidName(name.into())),
            }
        }
        fn connect(&self, name: &str, connection: ObjectRef) -> Result<Option<Cookie>, CcmError> {
            let expected = match name {
                "p_control" | "pull" => CONTROL,
                "out" => DATA,
                _ => return Err(CcmError::InvalidName(name.into())),
            };
            if LocalFacet::is_a(&connection, expected) != Some(true) {
                return Err(CcmError::InvalidConnection(name.into()));
            }
            self.connected.lock().unwrap().push(name.into());
            Ok(None)
        }
        fn disconnect(&self, name: &str, _: Option<&Cookie>) -> Result<ObjectRef, CcmError> {
            self.connected.lock().unwrap().retain(|n| n != name);
            Ok(LocalFacet::object(DATA, &[], Arc::new(Servant)))
        }
        fn configuration_complete(&self) -> Result<(), CcmError> {
            *self.completed.lock().unwrap() = true;
            Ok(())
        }
        fn remove(&self) -> Result<(), CcmError> {
            Ok(())
        }
    }

    fn factories() -> HomeFactories {
        let mut f = HomeFactories::new();
        let entry: HomeFactory = create_for_launch;
        f.register("create_Data_Cnx", entry);
        f
    }

    /// §7.6.4: the configProperty of a fragment instance in the plan
    /// reaches `HomeConfiguration::set_configuration_values` and, through
    /// `create_component`, the fragment, typed as the plan wrote it.
    #[test]
    fn corba_p3_7_6_4_fragment_config_properties_reach_set_configuration_values() {
        let plan = flatten_assembly(&assembly(), "App", "u", &nodes(), &port_types()).unwrap();
        let mut f = HomeFactories::new();
        let entry: HomeFactory = create_for_config;
        f.register("create_Data_Cnx", entry);
        let mut launcher = PlanLauncher::new(f);
        launcher.register_component("consumer1", Arc::new(Component::default()));
        launcher.register_component("producer1", Arc::new(Component::default()));
        launcher.start_launch(&plan, None).unwrap();
        let log = LOG_CONFIG.lock().unwrap().clone();
        for expected in [
            "set mcast_addr=Str(\"224.1.1.1\")",
            "set mcast_port=UShort(31337)",
            "set msg_size=ULong(50)",
        ] {
            assert_eq!(
                log.iter().filter(|l| l.ends_with(expected)).count(),
                2,
                "{expected} in {log:?}"
            );
        }
        // A configuration the connector refuses fails the creation.
        let mut refused_plan = plan.clone();
        refused_plan.instances[2]
            .config_props
            .push(crate::property::Property {
                name: "no_such_attribute".into(),
                value: AnyValue::Long(1),
            });
        let mut refused = PlanLauncher::new(factories());
        refused.register_component("consumer1", Arc::new(Component::default()));
        refused.register_component("producer1", Arc::new(Component::default()));
        assert!(matches!(
            refused.start_launch(&refused_plan, None),
            Err(LaunchError::Ccm {
                error: CcmError::CreateFailure(_),
                ..
            })
        ));
    }

    /// §7.6.3.1: the "home factory" execution parameter names the entry
    /// point whose home creates the fragment; the launch connects the
    /// basic ports, `Application::start` completes the configuration
    /// (§7.5.2) and the teardown removes the fragments.
    #[test]
    fn corba_p3_7_6_3_1_home_factory_entry_point_creates_the_fragment() {
        let plan = flatten_assembly(&assembly(), "App", "u", &nodes(), &port_types()).unwrap();
        let consumer = Arc::new(Component::default());
        let producer = Arc::new(Component::default());
        let mut launcher = PlanLauncher::new(factories());
        launcher.register_component("consumer1", consumer.clone());
        launcher.register_component("producer1", producer.clone());
        let provided = launcher.start_launch(&plan, None).unwrap();
        assert_eq!(
            launcher.fragment_names(),
            ["link_consumer1_p", "link_producer1_out"]
        );
        // Every connection's provider is on this one node.
        assert_eq!(provided.len(), plan.connections.len());
        launcher.finish_launch(&plan, &provided).unwrap();
        assert_eq!(*consumer.connected.lock().unwrap(), ["p_control"]);
        assert_eq!(*producer.connected.lock().unwrap(), ["out"]);
        let consumer_fragment = launcher.fragment("link_consumer1_p").unwrap();
        assert_eq!(
            consumer_fragment
                .connected_objects("cc_consumer")
                .unwrap()
                .len(),
            1
        );
        launcher.start().unwrap();
        assert!(*consumer.completed.lock().unwrap());
        assert!(consumer_fragment.is_configured());
        let log = LOG_LAUNCH.lock().unwrap().clone();
        assert!(
            log.iter()
                .any(|l| l.ends_with("complete with 1 on cc_consumer"))
        );
        assert!(
            log.iter()
                .any(|l| l.ends_with("complete with 0 on cc_consumer"))
        );
        launcher.destroy().unwrap();
        assert!(consumer_fragment.is_removed());
        assert!(consumer.connected.lock().unwrap().is_empty());
        assert_eq!(
            LOG_LAUNCH
                .lock()
                .unwrap()
                .iter()
                .filter(|l| l.ends_with("removed"))
                .count(),
            2
        );
        assert!(matches!(launcher.destroy(), Err(LaunchError::State(_))));

        // Per node: only the fragment collocated with the consumer.
        let mut on_node1 = PlanLauncher::new(factories());
        on_node1.register_component("consumer1", Arc::new(Component::default()));
        on_node1.start_launch(&plan, Some("node1")).unwrap();
        assert_eq!(on_node1.fragment_names(), ["link_consumer1_p"]);

        // An entry point the node does not know, a component nobody
        // registered.
        let mut unknown = PlanLauncher::new(HomeFactories::new());
        unknown.register_component("consumer1", Arc::new(Component::default()));
        unknown.register_component("producer1", Arc::new(Component::default()));
        assert!(matches!(
            unknown.start_launch(&plan, None),
            Err(LaunchError::UnknownEntryPoint { .. })
        ));
        let mut missing = PlanLauncher::new(factories());
        assert_eq!(
            missing.start_launch(&plan, None).err(),
            Some(LaunchError::MissingComponent("consumer1".into()))
        );
        // Steps out of order.
        let mut early = PlanLauncher::new(factories());
        assert!(matches!(early.start(), Err(LaunchError::State(_))));
        assert!(matches!(
            early.finish_launch(&plan, &ProvidedReferences::new()),
            Err(LaunchError::State(_))
        ));
    }

    /// The home of a fragment of the composite connector of
    /// [`composite_assembly`]: its basic ports and the `queue` attribute
    /// the package of `FIFO_Socket_f_pull` configures.
    struct PartHome {
        id: &'static str,
        ports: &'static [(&'static str, &'static str, bool)],
    }

    struct PartExec;

    impl FragmentExecutor for PartExec {
        fn provide(&self, facet: &str) -> Option<ObjectRef> {
            let type_id = match facet {
                "pull" => CONTROL,
                "link" => SOCKET,
                _ => DATA,
            };
            Some(LocalFacet::object(type_id, &[], Arc::new(Servant)))
        }
        fn set_attribute(&self, _: &str, _: &CorbaAny) -> Result<(), CcmError> {
            Ok(())
        }
        fn configuration_complete(&self, _: &ConnectorFragment) -> Result<(), CcmError> {
            Ok(())
        }
    }

    impl ConnectorHomeExecutor for PartHome {
        fn ports(&self) -> ComponentDef {
            let mut members: Vec<ConnectorMember> = self
                .ports
                .iter()
                .map(|(name, type_id, provides)| {
                    if *provides {
                        ConnectorMember::Provides {
                            name: (*name).into(),
                            interface_id: (*type_id).into(),
                        }
                    } else {
                        ConnectorMember::Uses {
                            name: (*name).into(),
                            interface_id: (*type_id).into(),
                            multiple: false,
                        }
                    }
                })
                .collect();
            members.push(ConnectorMember::Attribute(attribute("queue")));
            let mut table = ConnectorTypeTable::new();
            table.insert_connector(ConnectorDef {
                name: self.id.into(),
                repository_id: self.id.into(),
                base: None,
                members,
            });
            table.basic_ports(self.id).unwrap()
        }
        fn create(&self) -> Result<Arc<dyn FragmentExecutor>, CcmError> {
            Ok(Arc::new(PartExec))
        }
    }

    fn create_socket_client() -> Box<dyn ConnectorHomeExecutor> {
        Box::new(PartHome {
            id: "IDL:Demo/SocketClient:1.0",
            ports: &[("push", DATA, true), ("link", SOCKET, false)],
        })
    }

    fn create_socket_server() -> Box<dyn ConnectorHomeExecutor> {
        Box::new(PartHome {
            id: "IDL:Demo/SocketServer:1.0",
            ports: &[("link", SOCKET, true), ("out", DATA, false)],
        })
    }

    fn create_conn_fifo() -> Box<dyn ConnectorHomeExecutor> {
        Box::new(PartHome {
            id: "IDL:Demo/ConnFIFO:1.0",
            ports: &[("in", DATA, true), ("pull", CONTROL, true)],
        })
    }

    fn part_factories() -> HomeFactories {
        let mut f = HomeFactories::new();
        let client: HomeFactory = create_socket_client;
        let server: HomeFactory = create_socket_server;
        let fifo: HomeFactory = create_conn_fifo;
        f.register("create_SocketClient", client);
        f.register("create_SocketServer", server);
        f.register("create_ConnFIFO", fifo);
        f
    }

    /// §7.2.3.4 with one launcher per node: the fragments of the
    /// composite connector lie on two nodes (`q_client` with the producer
    /// on node2, `q_puller_server` and `q_puller_fifo` with the consumer
    /// on node1), and the connection `q_client.link ->
    /// q_puller_server.link` crosses them. node1's launcher provides the
    /// server's facet, node2's connects the client's receptacle to it.
    #[test]
    fn corba_p3_7_2_3_4_launchers_per_node_connect_the_fragments_of_a_composite_connector() {
        let plan =
            flatten_assembly(&composite_assembly(), "A", "u", &nodes(), &port_types()).unwrap();
        let crossing = plan
            .connections
            .iter()
            .find(|c| c.source_instance == "q_client" && c.target_instance == "q_puller_server")
            .expect("the connection between the nodes")
            .name
            .clone();
        let consumer = Arc::new(Component::default());
        let producer = Arc::new(Component::default());
        let mut node1 = PlanLauncher::new(part_factories());
        node1.register_component("consumer1", consumer.clone());
        let mut node2 = PlanLauncher::new(part_factories());
        node2.register_component("producer1", producer.clone());
        let mut launchers = BTreeMap::new();
        launchers.insert("node1".to_string(), node1);
        launchers.insert("node2".to_string(), node2);
        launch_plan(&plan, &mut launchers).unwrap();

        let client = launchers["node2"].fragment("q_client").unwrap();
        assert_eq!(launchers["node2"].fragment_names(), ["q_client"]);
        assert_eq!(
            client.connected_objects("link").unwrap().len(),
            1,
            "q_client.link is connected to the server on the other node"
        );
        assert_eq!(
            launchers["node1"].fragment_names(),
            ["q_puller_fifo", "q_puller_server"]
        );
        assert_eq!(
            launchers["node1"]
                .fragment("q_puller_server")
                .unwrap()
                .connected_objects("out")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(*producer.connected.lock().unwrap(), ["out"]);
        assert_eq!(*consumer.connected.lock().unwrap(), ["pull"]);
        for launcher in launchers.values_mut() {
            launcher.start().unwrap();
        }
        assert!(*consumer.completed.lock().unwrap());
        for launcher in launchers.values_mut() {
            launcher.destroy().unwrap();
        }
        assert!(client.is_removed());

        // Without the references node1 provides, node2 cannot make the
        // connection, and says which one.
        let mut alone = PlanLauncher::new(part_factories());
        alone.register_component("producer1", Arc::new(Component::default()));
        alone.start_launch(&plan, Some("node2")).unwrap();
        assert_eq!(
            alone.finish_launch(&plan, &ProvidedReferences::new()),
            Err(LaunchError::MissingProvidedReference(crossing))
        );
        // An instance on a node no launcher was given for.
        let mut only_node1 = BTreeMap::new();
        let mut launcher = PlanLauncher::new(part_factories());
        launcher.register_component("consumer1", Arc::new(Component::default()));
        only_node1.insert("node1".to_string(), launcher);
        assert!(matches!(
            launch_plan(&plan, &mut only_node1),
            Err(LaunchError::NoLauncher { .. })
        ));
    }
}
