// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connectors in Deployment and Configuration — CORBA 3.3 Part 3 §7.6.
//!
//! # Component data model (§7.6.2)
//!
//! A connector is packaged like a component: a
//! [`ConnectorPackageDescription`] realizes a
//! [`ComponentInterfaceDescription`] (whose [`ComponentPortDescription`]s
//! know the port kinds `ExtendedPort` and `ExtendedMirrorPort` and the
//! template parameters of an extended port, §7.6.2.4) and has one or more
//! [`ConnectorImplementationDescription`]s, each monolithic or an
//! assembly of fragments (Figure 7.8, §7.2.3.4). In an assembly a
//! connector is no instance: an [`AssemblyConnectionDescription`] is
//! realized by a connector (§7.6.2.5, the association of cardinality 0..1).
//!
//! # Execution data model (§7.6.3)
//!
//! [`flatten_assembly`] produces the deployment plan from an assembly:
//! the components become instances; each use of a connector becomes
//! fragment instances, collocated with the components they serve
//! (§7.2.3: "Each fragment will be co-localized to the component using
//! them"), with their implementation (carrying the execution parameter
//! "home factory", §7.6.3.1), their artifacts and their configuration
//! (§7.6.4); and the connections between components and fragments are
//! basic connections between facets and receptacles.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_corba_ccm::connector::{ConnectorModelError, ConnectorTypeTable};
use zerodds_corba_rt::any::AnyValue;

use crate::plan::{
    ComponentPackageDescription, DeploymentPlan, ImplementationDescription,
    InstanceDeploymentDescription, PlanConnection, PlanError,
};
use crate::property::Property;

/// The name of the execution parameter that names the entry point of a
/// connector's home (§7.6.3.1).
pub const HOME_FACTORY: &str = "home factory";

/// `CCMComponentPortKind` with the two values §7.6.2.3 adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CcmComponentPortKind {
    /// A facet (`provides`).
    Facet,
    /// A simplex receptacle (`uses`).
    SimplexReceptacle,
    /// A multiplex receptacle (`uses multiple`).
    MultiplexReceptacle,
    /// `emits`.
    EventEmitter,
    /// `publishes`.
    EventPublisher,
    /// `consumes`.
    EventConsumer,
    /// `port` (§7.6.2.3).
    ExtendedPort,
    /// `mirrorport` (§7.6.2.3).
    ExtendedMirrorPort,
}

impl CcmComponentPortKind {
    /// All kinds, in the order of the enumeration.
    pub const ALL: [Self; 8] = [
        Self::Facet,
        Self::SimplexReceptacle,
        Self::MultiplexReceptacle,
        Self::EventEmitter,
        Self::EventPublisher,
        Self::EventConsumer,
        Self::ExtendedPort,
        Self::ExtendedMirrorPort,
    ];

    /// The literal of the enumeration.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Facet => "Facet",
            Self::SimplexReceptacle => "SimplexReceptacle",
            Self::MultiplexReceptacle => "MultiplexReceptacle",
            Self::EventEmitter => "EventEmitter",
            Self::EventPublisher => "EventPublisher",
            Self::EventConsumer => "EventConsumer",
            Self::ExtendedPort => "ExtendedPort",
            Self::ExtendedMirrorPort => "ExtendedMirrorPort",
        }
    }

    /// The kind of a literal.
    #[must_use]
    pub fn from_literal(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    fn is_receptacle(self) -> bool {
        matches!(self, Self::SimplexReceptacle | Self::MultiplexReceptacle)
    }

    fn is_event_source(self) -> bool {
        matches!(self, Self::EventEmitter | Self::EventPublisher)
    }
}

/// `ComponentPortDescription`, extended by `templateParam` (§7.6.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPortDescription {
    /// The port's name.
    pub name: String,
    /// The repository id of its type: interface, event type or port type.
    pub specific_type: String,
    /// The types it also supports (base interfaces).
    pub supported_type: Vec<String>,
    /// Whether the port provides (facet, event consumer).
    pub provider: bool,
    /// `exclusiveProvider`.
    pub exclusive_provider: bool,
    /// `exclusiveUser`.
    pub exclusive_user: bool,
    /// Whether the port may stay unconnected.
    pub optional: bool,
    /// The kind of port.
    pub kind: CcmComponentPortKind,
    /// The types of the template parameters of an extended port; empty
    /// for a basic port and for an extended port without template.
    pub template_param: Vec<String>,
}

impl ComponentPortDescription {
    /// A port of `kind` and type `specific_type`, the flags at their
    /// defaults (`provider` from the kind).
    #[must_use]
    pub fn new(name: &str, kind: CcmComponentPortKind, specific_type: &str) -> Self {
        Self {
            name: name.to_string(),
            specific_type: specific_type.to_string(),
            supported_type: Vec::new(),
            provider: matches!(
                kind,
                CcmComponentPortKind::Facet | CcmComponentPortKind::EventConsumer
            ),
            exclusive_provider: false,
            exclusive_user: false,
            optional: false,
            kind,
            template_param: Vec::new(),
        }
    }
}

/// `ComponentPropertyDescription`: a property and its type (written as
/// the IDL type name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPropertyDescription {
    /// The property's name.
    pub name: String,
    /// Its IDL type.
    pub type_name: String,
}

/// `ComponentInterfaceDescription` (§7.6.2.4): describes components and
/// connectors.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComponentInterfaceDescription {
    /// `label`.
    pub label: String,
    /// `UUID`.
    pub uuid: String,
    /// The repository id of the component or connector.
    pub specific_type: String,
    /// The repository ids it also supports.
    pub supported_type: Vec<String>,
    /// The IDL files.
    pub idl_file: Vec<String>,
    /// Default configuration.
    pub config_property: Vec<Property>,
    /// The ports.
    pub port: Vec<ComponentPortDescription>,
    /// The properties (attributes).
    pub property: Vec<ComponentPropertyDescription>,
    /// Non-functional information.
    pub info_property: Vec<Property>,
}

impl ComponentInterfaceDescription {
    /// The port `name`.
    #[must_use]
    pub fn port(&self, name: &str) -> Option<&ComponentPortDescription> {
        self.port.iter().find(|p| p.name == name)
    }

    /// §7.6.2.4: "If `templateParam` contains values, the kind attribute
    /// shall be `ExtendedPort` or `ExtendedMirrorPort`"; and port names
    /// are unique.
    ///
    /// # Errors
    /// [`FlattenError::TemplateParamOnBasicPort`],
    /// [`FlattenError::DuplicatePort`].
    pub fn validate(&self) -> Result<(), FlattenError> {
        let mut names = BTreeSet::new();
        for p in &self.port {
            if !names.insert(p.name.as_str()) {
                return Err(FlattenError::DuplicatePort {
                    owner: self.specific_type.clone(),
                    port: p.name.clone(),
                });
            }
            if !p.template_param.is_empty()
                && !matches!(
                    p.kind,
                    CcmComponentPortKind::ExtendedPort | CcmComponentPortKind::ExtendedMirrorPort
                )
            {
                return Err(FlattenError::TemplateParamOnBasicPort {
                    owner: self.specific_type.clone(),
                    port: p.name.clone(),
                });
            }
        }
        Ok(())
    }
}

/// `MonolithicImplementationDescription`: the primary artifacts and the
/// execution parameters (where a fragment's carry "home factory").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonolithicImplementationDescription {
    /// The locations of the primary artifacts.
    pub primary_artifact: Vec<String>,
    /// The execution parameters.
    pub exec_parameter: BTreeMap<String, String>,
}

/// How a connector implementation is realized (Figure 7.8: one of
/// `monolithicImpl` and `assemblyImpl`).
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectorImplementation {
    /// One artifact set; one fragment per component it serves.
    Monolithic(MonolithicImplementationDescription),
    /// An assembly of fragments (§7.2.3.4, composite connector).
    Assembly(ComponentAssemblyDescription),
}

/// `SatisfierPropertyKind` (Part 3 §16): how a satisfier property meets
/// the property of a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SatisfierPropertyKind {
    /// A count: the satisfier value is an `unsigned long`, the
    /// requirement's value is ignored.
    Quantity,
    /// A capacity: the requirement asks for at most the satisfier value.
    Capacity,
    /// A minimum: the requirement's value is at least the satisfier's.
    Minimum,
    /// A maximum: the requirement's value is at most the satisfier's.
    Maximum,
    /// An attribute: the requirement's value equals the satisfier's.
    Attribute,
    /// A selection: the requirement's value is one of the satisfier's.
    Selection,
}

impl SatisfierPropertyKind {
    /// All kinds, in the order of the enumeration.
    pub const ALL: [Self; 6] = [
        Self::Quantity,
        Self::Capacity,
        Self::Minimum,
        Self::Maximum,
        Self::Attribute,
        Self::Selection,
    ];

    /// The literal of the XML schema (Part 3 §17).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quantity => "Quantity",
            Self::Capacity => "Capacity",
            Self::Minimum => "Minimum",
            Self::Maximum => "Maximum",
            Self::Attribute => "Attribute",
            Self::Selection => "Selection",
        }
    }

    /// The kind of a literal.
    #[must_use]
    pub fn from_literal(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// `SatisfierProperty` (Part 3 §16): one property of a capability.
#[derive(Debug, Clone, PartialEq)]
pub struct SatisfierProperty {
    /// The name a requirement's property is matched by.
    pub name: String,
    /// How it is matched.
    pub kind: SatisfierPropertyKind,
    /// `dynamic`.
    pub dynamic: bool,
    /// The value: of the type Part 3 §15.4.5 gives the kind, a sequence
    /// of such values for [`SatisfierPropertyKind::Selection`].
    pub value: AnyValue,
}

/// `Capability` (Part 3 §16): a user-defined capability an
/// implementation is tagged with (§7.6.2.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Capability {
    /// Its name.
    pub name: String,
    /// The resource types it is of; a requirement names one of them.
    pub resource_type: Vec<String>,
    /// Its properties.
    pub property: Vec<SatisfierProperty>,
}

/// `Requirement` (Part 3 §16): a selection requirement of a package
/// configuration or a subcomponent instance (§7.6.2.3, §15.9.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Requirement {
    /// Its name.
    pub name: String,
    /// The resource type a capability has to be of.
    pub resource_type: String,
    /// The properties a capability has to satisfy.
    pub property: Vec<Property>,
}

impl Requirement {
    /// Whether `capability` satisfies the requirement: it is of the
    /// requirement's resource type and, for each property of the
    /// requirement, has a satisfier property of the same name that meets
    /// it by its kind (Part 3 §15.4.5).
    ///
    /// # Errors
    /// [`FlattenError::RequirementValue`] for a value whose type
    /// §15.4.5 does not give the kind.
    pub fn satisfied_by(&self, capability: &Capability) -> Result<bool, FlattenError> {
        if !capability.resource_type.contains(&self.resource_type) {
            return Ok(false);
        }
        for wanted in &self.property {
            let Some(offered) = capability.property.iter().find(|s| s.name == wanted.name) else {
                return Ok(false);
            };
            if !self.meets(offered, &wanted.value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn meets(&self, offered: &SatisfierProperty, wanted: &AnyValue) -> Result<bool, FlattenError> {
        use SatisfierPropertyKind as K;
        let invalid = || FlattenError::RequirementValue {
            requirement: self.name.clone(),
            property: offered.name.clone(),
        };
        let ordered = |a: &AnyValue, b: &AnyValue| -> Option<core::cmp::Ordering> {
            match (a, b) {
                (AnyValue::ULong(a), AnyValue::ULong(b)) => Some(a.cmp(b)),
                (AnyValue::Long(a), AnyValue::Long(b)) => Some(a.cmp(b)),
                (AnyValue::Double(a), AnyValue::Double(b)) => a.partial_cmp(b),
                _ => None,
            }
        };
        let equal = |s: &AnyValue, r: &AnyValue| -> Option<bool> {
            match (s, r) {
                (AnyValue::Long(a), AnyValue::Long(b)) => Some(a == b),
                (AnyValue::Double(a), AnyValue::Double(b)) => Some(a == b),
                (AnyValue::Str(a), AnyValue::Str(b)) => Some(a == b),
                // "If the value of the SatisfierProperty is of
                // enumeration type, the value of the Property is of type
                // string, containing the enumeration value."
                (AnyValue::Enum { value, members, .. }, AnyValue::Str(b)) => {
                    let index = usize::try_from(*value).ok()?;
                    Some(members.get(index)? == b)
                }
                _ => None,
            }
        };
        match offered.kind {
            K::Quantity => match offered.value {
                AnyValue::ULong(available) => Ok(available >= 1),
                _ => Err(invalid()),
            },
            K::Capacity => match (&offered.value, wanted) {
                (AnyValue::ULong(_), AnyValue::ULong(_))
                | (AnyValue::Double(_), AnyValue::Double(_)) => Ok(ordered(wanted, &offered.value)
                    .is_some_and(|o| o != core::cmp::Ordering::Greater)),
                _ => Err(invalid()),
            },
            K::Maximum | K::Minimum => match (&offered.value, wanted) {
                (AnyValue::Long(_), AnyValue::Long(_))
                | (AnyValue::Double(_), AnyValue::Double(_)) => {
                    let order = ordered(wanted, &offered.value);
                    Ok(if offered.kind == K::Maximum {
                        order.is_some_and(|o| o != core::cmp::Ordering::Greater)
                    } else {
                        order.is_some_and(|o| o != core::cmp::Ordering::Less)
                    })
                }
                _ => Err(invalid()),
            },
            K::Attribute => equal(&offered.value, wanted).ok_or_else(invalid),
            K::Selection => match &offered.value {
                AnyValue::Seq { items, .. } => {
                    let mut found = false;
                    for item in items {
                        found |= equal(item, wanted).ok_or_else(invalid)?;
                    }
                    Ok(found)
                }
                _ => Err(invalid()),
            },
        }
    }
}

/// Whether the capabilities of an implementation satisfy every
/// requirement: each by one of them.
fn capabilities_satisfy(
    capabilities: &[Capability],
    requirements: &[Requirement],
) -> Result<bool, FlattenError> {
    for requirement in requirements {
        let mut satisfied = false;
        for capability in capabilities {
            if requirement.satisfied_by(capability)? {
                satisfied = true;
                break;
            }
        }
        if !satisfied {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `ConnectorImplementationDescription` (§7.6.2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorImplementationDescription {
    /// `label`.
    pub label: String,
    /// `UUID`.
    pub uuid: String,
    /// The connector interface it implements.
    pub implements: ComponentInterfaceDescription,
    /// The realization.
    pub implementation: ConnectorImplementation,
    /// The default configuration of each fragment.
    pub config_property: Vec<Property>,
    /// User-defined capabilities, which selection requirements are
    /// matched against (§7.6.2.3).
    pub capability: Vec<Capability>,
}

/// `ConnectorPackageDescription` (§7.6.2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorPackageDescription {
    /// `label`.
    pub label: String,
    /// `UUID`.
    pub uuid: String,
    /// The connector interface.
    pub realizes: ComponentInterfaceDescription,
    /// Configuration that overrides the implementation's and that a
    /// package configuration may override.
    pub config_property: Vec<Property>,
    /// The alternative implementations; planning takes the first whose
    /// capabilities satisfy the selection requirements of the use.
    pub implementation: Vec<ConnectorImplementationDescription>,
}

impl ConnectorPackageDescription {
    /// The implementation planning takes: the first, in the package's
    /// order, whose capabilities satisfy every selection requirement
    /// (§7.6.2.3: "Administrators can then select among implementations
    /// using selection requirements in a PackageConfiguration"); without
    /// requirements, the first.
    ///
    /// # Errors
    /// [`FlattenError::NoImplementation`] for a package without one,
    /// [`FlattenError::NoSatisfyingImplementation`] when none satisfies
    /// the requirements, [`FlattenError::RequirementValue`].
    pub fn select_implementation(
        &self,
        requirements: &[Requirement],
    ) -> Result<&ConnectorImplementationDescription, FlattenError> {
        if self.implementation.is_empty() {
            return Err(FlattenError::NoImplementation(self.label.clone()));
        }
        for implementation in &self.implementation {
            if capabilities_satisfy(&implementation.capability, requirements)? {
                return Ok(implementation);
            }
        }
        Err(FlattenError::NoSatisfyingImplementation(self.label.clone()))
    }
}

/// The implementation of a component package planning takes: the first.
/// An implementation of this data model carries no capabilities, so a
/// selection requirement is satisfied by none of them.
fn select_component_implementation<'p>(
    package: &'p ComponentPackageDescription,
    requirements: &[Requirement],
) -> Result<&'p ImplementationDescription, FlattenError> {
    let first = package
        .implementations
        .first()
        .ok_or_else(|| FlattenError::NoImplementation(package.label.clone()))?;
    if requirements.is_empty() {
        Ok(first)
    } else {
        Err(FlattenError::NoSatisfyingImplementation(
            package.label.clone(),
        ))
    }
}

/// What a subcomponent instance is an instance of (Figure 7.6: the
/// association with `ConnectorPackageDescription` excludes the one with
/// `ComponentPackageDescription`).
#[derive(Debug, Clone, PartialEq)]
pub enum SubcomponentPackage {
    /// A component package.
    Component(ComponentPackageDescription),
    /// A connector package: a fragment inside a connector assembly.
    Connector(ConnectorPackageDescription),
}

/// `SubcomponentInstantiationDescription`.
#[derive(Debug, Clone, PartialEq)]
pub struct SubcomponentInstantiationDescription {
    /// The instance's name, unique in its assembly.
    pub name: String,
    /// What it is an instance of.
    pub package: SubcomponentPackage,
    /// Its configuration.
    pub config_property: Vec<Property>,
    /// The selection requirements that choose the implementation of its
    /// package (Part 3 §15.9.1).
    pub select_requirement: Vec<Requirement>,
}

/// `SubcomponentPortEndpoint`: a port of a subcomponent (by name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubcomponentPortEndpoint {
    /// The subcomponent.
    pub instance: String,
    /// Its port.
    pub port_name: String,
}

/// The connector that realizes a connection: the package, the
/// selection requirements and the configuration of this use of it (the
/// `PackageConfiguration` with `baseConnectorPackage` of Figure 7.6,
/// whose `selectRequirement` chooses among the package's implementations
/// and whose `configProperty` overrides the package's).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorUse {
    /// The connector package.
    pub package: ConnectorPackageDescription,
    /// The selection requirements (§7.6.2.3).
    pub select_requirement: Vec<Requirement>,
    /// The configuration of this use.
    pub config_property: Vec<Property>,
}

/// `AssemblyConnectionDescription` with the connector of §7.6.2.5.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyConnectionDescription {
    /// The connection's name.
    pub name: String,
    /// The connector realizing it; `None` for a basic connection.
    pub connector: Option<ConnectorUse>,
    /// The subcomponent ports it connects.
    pub internal_endpoint: Vec<SubcomponentPortEndpoint>,
    /// The ports of the assembly itself it connects to
    /// (`ComponentExternalPortEndpoint::portName`): in a connector's
    /// assembly, the connector's ports.
    pub external_endpoint: Vec<String>,
}

/// `ComponentAssemblyDescription`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComponentAssemblyDescription {
    /// The subcomponent instances.
    pub instance: Vec<SubcomponentInstantiationDescription>,
    /// The connections.
    pub connection: Vec<AssemblyConnectionDescription>,
}

impl ComponentAssemblyDescription {
    fn instance(&self, name: &str) -> Option<&SubcomponentInstantiationDescription> {
        self.instance.iter().find(|i| i.name == name)
    }
}

/// Why an assembly does not flatten into a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlattenError {
    /// An endpoint names an instance the assembly does not have.
    UnknownInstance(String),
    /// An endpoint names a port the instance's interface does not have.
    UnknownPort {
        /// The instance.
        instance: String,
        /// The port.
        port: String,
    },
    /// Two ports of one interface have the same name.
    DuplicatePort {
        /// The component or connector.
        owner: String,
        /// The port.
        port: String,
    },
    /// §7.6.2.4: template parameters on a port that is not extended.
    TemplateParamOnBasicPort {
        /// The component or connector.
        owner: String,
        /// The port.
        port: String,
    },
    /// The planner gave no node for a component instance.
    NoNode(String),
    /// A package without implementation.
    NoImplementation(String),
    /// A connector implementation without the "home factory" execution
    /// parameter (§7.6.3.1).
    MissingHomeFactory(String),
    /// A connector package as an instance of the top assembly: "using a
    /// connector ... shall not appear as a new component instance in the
    /// assembly" (§7.6.2.5).
    ConnectorAsInstance(String),
    /// No implementation of the package satisfies the selection
    /// requirements (§7.6.2.3).
    NoSatisfyingImplementation(String),
    /// A requirement property or a satisfier property whose value has
    /// a type Part 3 §15.4.5 does not give the satisfier's kind.
    RequirementValue {
        /// The requirement.
        requirement: String,
        /// The property.
        property: String,
    },
    /// No port of the connector matches the component's port.
    NoMatchingConnectorPort {
        /// The connection.
        connection: String,
        /// The component instance.
        instance: String,
        /// Its port.
        port: String,
    },
    /// More than one port of the connector matches.
    AmbiguousConnectorPort {
        /// The connection.
        connection: String,
        /// The component instance.
        instance: String,
        /// Its port.
        port: String,
    },
    /// Two ports that cannot be connected.
    IncompatiblePorts {
        /// The connection.
        connection: String,
        /// The first endpoint (`instance.port`).
        a: String,
        /// The second endpoint.
        b: String,
    },
    /// A basic connection without exactly two endpoints.
    EndpointCount(String),
    /// An extended port names a port type the planner does not know.
    UnknownPortType(String),
    /// The port type table refused the matching of two port types.
    PortTypeModel(zerodds_corba_ccm::connector::ConnectorModelError),
    /// A connector port has no fragment in the connector's assembly, or
    /// more than one.
    UnmappedConnectorPort {
        /// The connector.
        connector: String,
        /// Its port.
        port: String,
    },
    /// A fragment of a connector assembly that serves no component of
    /// the connection (it has no node), or serves two.
    FragmentPlacement {
        /// The connection.
        connection: String,
        /// The fragment.
        fragment: String,
    },
    /// A connection inside a connector's assembly realized by another
    /// connector.
    NestedConnector(String),
    /// The plan is inconsistent.
    Plan(PlanError),
}

impl core::fmt::Display for FlattenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FlattenError {}

/// Later properties override earlier ones of the same name.
fn merge(layers: &[&[Property]]) -> Vec<Property> {
    let mut out: Vec<Property> = Vec::new();
    for layer in layers {
        for p in *layer {
            if let Some(existing) = out.iter_mut().find(|e| e.name == p.name) {
                *existing = p.clone();
            } else {
                out.push(p.clone());
            }
        }
    }
    out
}

/// Where a fragment goes: the node and the component it is collocated
/// with.
#[derive(Clone)]
struct Placement {
    node: String,
    with: String,
}

struct Flattener<'a> {
    port_types: &'a ConnectorTypeTable,
    plan: DeploymentPlan,
}

impl Flattener<'_> {
    fn add_implementation(&mut self, implementation: ImplementationDescription) {
        if !self
            .plan
            .implementations
            .iter()
            .any(|i| i.label == implementation.label)
        {
            self.plan.implementations.push(implementation);
        }
    }

    /// The basic connections between two ports (§7.2.2.2, §7.6.3).
    fn link(
        &mut self,
        connection: &str,
        a: (&str, &ComponentPortDescription),
        b: (&str, &ComponentPortDescription),
    ) -> Result<(), FlattenError> {
        use CcmComponentPortKind as K;
        let incompatible = || FlattenError::IncompatiblePorts {
            connection: connection.to_string(),
            a: format!("{}.{}", a.0, a.1.name),
            b: format!("{}.{}", b.0, b.1.name),
        };
        let types_meet = |user: &ComponentPortDescription, provider: &ComponentPortDescription| {
            user.specific_type == provider.specific_type
                || provider.supported_type.contains(&user.specific_type)
        };
        let push = |source: (&str, &str), target: (&str, &str), plan: &mut DeploymentPlan| {
            plan.connections.push(PlanConnection {
                name: format!("{connection}_{}_{}", source.0, source.1),
                source_instance: source.0.to_string(),
                source_port: source.1.to_string(),
                target_instance: target.0.to_string(),
                target_port: target.1.to_string(),
            });
        };
        let (pa, pb) = (a.1, b.1);
        match (pa.kind, pb.kind) {
            (K::Facet, k) if k.is_receptacle() => {
                if !types_meet(pb, pa) {
                    return Err(incompatible());
                }
                push((b.0, &pb.name), (a.0, &pa.name), &mut self.plan);
            }
            (k, K::Facet) if k.is_receptacle() => {
                if !types_meet(pa, pb) {
                    return Err(incompatible());
                }
                push((a.0, &pa.name), (b.0, &pb.name), &mut self.plan);
            }
            (k, K::EventConsumer) if k.is_event_source() => {
                if pa.specific_type != pb.specific_type {
                    return Err(incompatible());
                }
                push((a.0, &pa.name), (b.0, &pb.name), &mut self.plan);
            }
            (K::EventConsumer, k) if k.is_event_source() => {
                if pa.specific_type != pb.specific_type {
                    return Err(incompatible());
                }
                push((b.0, &pb.name), (a.0, &pa.name), &mut self.plan);
            }
            (K::ExtendedPort, K::ExtendedMirrorPort) | (K::ExtendedMirrorPort, K::ExtendedPort) => {
                let (port, mirror) = if pa.kind == K::ExtendedPort {
                    (a, b)
                } else {
                    (b, a)
                };
                if port.1.template_param != mirror.1.template_param {
                    return Err(incompatible());
                }
                if !self.port_types_match(&port.1.specific_type, &mirror.1.specific_type)? {
                    return Err(incompatible());
                }
                let basic = |id: &str| {
                    let t = self.port_types.port_type(id);
                    let provides: Vec<String> = t
                        .map(|t| {
                            t.members
                                .iter()
                                .filter_map(|m| match m {
                                    zerodds_corba_ccm::connector::PortTypeMember::Provides {
                                        name,
                                        ..
                                    } => Some(name.clone()),
                                    _ => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let uses: Vec<String> = t
                        .map(|t| {
                            t.members
                                .iter()
                                .filter_map(|m| match m {
                                    zerodds_corba_ccm::connector::PortTypeMember::Uses {
                                        name,
                                        ..
                                    } => Some(name.clone()),
                                    _ => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (provides, uses)
                };
                let (port_provides, port_uses) = basic(&port.1.specific_type);
                let (mirror_provides, mirror_uses) = basic(&mirror.1.specific_type);
                // A provides of the port is a facet `<port>_<d>`, the
                // matching provides of the mirror port a receptacle.
                for (d, md) in port_provides.iter().zip(&mirror_provides) {
                    let facet = format!("{}_{d}", port.1.name);
                    let receptacle = format!("{}_{md}", mirror.1.name);
                    push((mirror.0, &receptacle), (port.0, &facet), &mut self.plan);
                }
                // A uses of the port is a receptacle, the matching uses of
                // the mirror port a facet.
                for (u, mu) in port_uses.iter().zip(&mirror_uses) {
                    let receptacle = format!("{}_{u}", port.1.name);
                    let facet = format!("{}_{mu}", mirror.1.name);
                    push((port.0, &receptacle), (mirror.0, &facet), &mut self.plan);
                }
            }
            _ => return Err(incompatible()),
        }
        Ok(())
    }

    /// Whether two port types match (§7.6.5); an id the table does not
    /// know is [`FlattenError::UnknownPortType`].
    fn port_types_match(&self, a: &str, b: &str) -> Result<bool, FlattenError> {
        let equal = |x: &str, y: &str| x == y;
        self.port_types
            .port_types_match(a, b, &equal)
            .map_err(|error| match error {
                ConnectorModelError::UnknownPortType { port_type, .. } => {
                    FlattenError::UnknownPortType(port_type)
                }
                other => FlattenError::PortTypeModel(other),
            })
    }

    /// The connector port that serves a component's port: the
    /// complementary kind with a matching type.
    fn connector_port<'p>(
        &self,
        connection: &str,
        endpoint: &SubcomponentPortEndpoint,
        component_port: &ComponentPortDescription,
        connector: &'p ComponentInterfaceDescription,
    ) -> Result<&'p ComponentPortDescription, FlattenError> {
        use CcmComponentPortKind as K;
        let fits = |c: &ComponentPortDescription| -> Result<bool, FlattenError> {
            Ok(match (component_port.kind, c.kind) {
                (K::Facet, k) if k.is_receptacle() => {
                    c.specific_type == component_port.specific_type
                        || component_port.supported_type.contains(&c.specific_type)
                }
                (k, K::Facet) if k.is_receptacle() => {
                    c.specific_type == component_port.specific_type
                        || c.supported_type.contains(&component_port.specific_type)
                }
                (k, K::EventConsumer) if k.is_event_source() => {
                    c.specific_type == component_port.specific_type
                }
                (K::EventConsumer, k) if k.is_event_source() => {
                    c.specific_type == component_port.specific_type
                }
                (K::ExtendedPort, K::ExtendedMirrorPort)
                | (K::ExtendedMirrorPort, K::ExtendedPort) => {
                    c.template_param == component_port.template_param
                        && self.port_types_match(&component_port.specific_type, &c.specific_type)?
                }
                _ => false,
            })
        };
        let mut candidates: Vec<&ComponentPortDescription> = Vec::new();
        for c in &connector.port {
            if fits(c)? {
                candidates.push(c);
            }
        }
        match candidates.as_slice() {
            [one] => Ok(one),
            [] => Err(FlattenError::NoMatchingConnectorPort {
                connection: connection.to_string(),
                instance: endpoint.instance.clone(),
                port: endpoint.port_name.clone(),
            }),
            _ => Err(FlattenError::AmbiguousConnectorPort {
                connection: connection.to_string(),
                instance: endpoint.instance.clone(),
                port: endpoint.port_name.clone(),
            }),
        }
    }

    /// Deploys a monolithic fragment instance.
    fn deploy_fragment(
        &mut self,
        instance: &str,
        package: &ConnectorPackageDescription,
        implementation: &ConnectorImplementationDescription,
        monolithic: &MonolithicImplementationDescription,
        placement: &Placement,
        layers: &[&[Property]],
    ) -> Result<(), FlattenError> {
        if !monolithic.exec_parameter.contains_key(HOME_FACTORY) {
            return Err(FlattenError::MissingHomeFactory(
                implementation.label.clone(),
            ));
        }
        self.add_implementation(ImplementationDescription {
            label: implementation.label.clone(),
            uuid: implementation.uuid.clone(),
            artifacts: monolithic.primary_artifact.clone(),
            realizes: package.realizes.specific_type.clone(),
            exec_params: monolithic.exec_parameter.clone(),
            depends_on: Vec::new(),
        });
        let mut all: Vec<&[Property]> = alloc::vec![
            implementation.config_property.as_slice(),
            package.config_property.as_slice(),
        ];
        all.extend_from_slice(layers);
        self.plan.instances.push(InstanceDeploymentDescription {
            name: instance.to_string(),
            implementation: implementation.label.clone(),
            node: placement.node.clone(),
            config_props: merge(&all),
            co_locate_with: alloc::vec![placement.with.clone()],
        });
        Ok(())
    }

    /// Deploys a component that is a part of a connector's assembly
    /// implementation (§7.2.3.4: "such an implementation may be an
    /// assembly of other components"): an instance of the component's
    /// implementation, placed and collocated like a fragment, configured
    /// like one, and created by the container like every component.
    fn deploy_component_part(
        &mut self,
        instance: &str,
        package: &ComponentPackageDescription,
        requirements: &[Requirement],
        placement: &Placement,
        layers: &[&[Property]],
    ) -> Result<(), FlattenError> {
        package.realizes.validate()?;
        let implementation = select_component_implementation(package, requirements)?;
        self.add_implementation(implementation.clone());
        self.plan.instances.push(InstanceDeploymentDescription {
            name: instance.to_string(),
            implementation: implementation.label.clone(),
            node: placement.node.clone(),
            config_props: merge(layers),
            co_locate_with: alloc::vec![placement.with.clone()],
        });
        Ok(())
    }

    /// Deploys the part `part` of a connector's assembly as `instance`:
    /// a connector fragment through [`Self::deploy_package`], a component
    /// through [`Self::deploy_component_part`]. `layers` is the
    /// configuration from the enclosing connector; the part's own comes
    /// last.
    ///
    /// zerodds-lint: recursion-depth 32 — through `deploy_package`, whose
    /// `depth` argument bounds it.
    fn deploy_part(
        &mut self,
        connection: &str,
        instance: &str,
        part: &SubcomponentInstantiationDescription,
        placement: &Placement,
        layers: &[&[Property]],
        depth: usize,
    ) -> Result<(), FlattenError> {
        let mut all: Vec<&[Property]> = layers.to_vec();
        all.push(part.config_property.as_slice());
        match &part.package {
            SubcomponentPackage::Connector(inner) => self.deploy_package(
                connection,
                instance,
                inner,
                &part.select_requirement,
                placement,
                &all,
                depth,
            ),
            SubcomponentPackage::Component(component) => self.deploy_component_part(
                instance,
                component,
                &part.select_requirement,
                placement,
                &all,
            ),
        }
    }

    /// Deploys the fragment instance `instance` of `package`, with the
    /// implementation `requirements` select, and, for an assembly
    /// implementation, its parts with their internal connections.
    ///
    /// zerodds-lint: recursion-depth 32 — the `depth` argument bounds the
    /// nesting of assembly implementations.
    #[allow(clippy::too_many_arguments)]
    fn deploy_package(
        &mut self,
        connection: &str,
        instance: &str,
        package: &ConnectorPackageDescription,
        requirements: &[Requirement],
        placement: &Placement,
        layers: &[&[Property]],
        depth: usize,
    ) -> Result<(), FlattenError> {
        if depth > 32 {
            return Err(FlattenError::NestedConnector(instance.to_string()));
        }
        package.realizes.validate()?;
        let implementation = package.select_implementation(requirements)?;
        match &implementation.implementation {
            ConnectorImplementation::Monolithic(m) => {
                self.deploy_fragment(instance, package, implementation, m, placement, layers)
            }
            ConnectorImplementation::Assembly(assembly) => {
                let mut inner_layers: Vec<&[Property]> = alloc::vec![
                    implementation.config_property.as_slice(),
                    package.config_property.as_slice(),
                ];
                inner_layers.extend_from_slice(layers);
                for part in &assembly.instance {
                    let name = format!("{instance}_{}", part.name);
                    self.deploy_part(connection, &name, part, placement, &inner_layers, depth + 1)?;
                }
                self.link_internal(connection, instance, assembly, depth)
            }
        }
    }

    /// The internal connections of a connector assembly deployed under
    /// `prefix`.
    fn link_internal(
        &mut self,
        connection: &str,
        prefix: &str,
        assembly: &ComponentAssemblyDescription,
        depth: usize,
    ) -> Result<(), FlattenError> {
        for c in &assembly.connection {
            if c.connector.is_some() {
                return Err(FlattenError::NestedConnector(c.name.clone()));
            }
            if !c.external_endpoint.is_empty() {
                continue;
            }
            let [a, b] = c.internal_endpoint.as_slice() else {
                return Err(FlattenError::EndpointCount(c.name.clone()));
            };
            let (ai, ap) = leaf(prefix, assembly, a, depth)?;
            let (bi, bp) = leaf(prefix, assembly, b, depth)?;
            self.link(&format!("{connection}_{}", c.name), (&ai, &ap), (&bi, &bp))?;
        }
        Ok(())
    }
}

/// The fragment instance and port an endpoint of a connector assembly
/// deployed under `prefix` ends at: the part itself when it is
/// monolithic, else, through the assembly connection that maps the
/// part's port, a part of the part.
///
/// zerodds-lint: recursion-depth 32 — the `depth` argument.
fn leaf(
    prefix: &str,
    assembly: &ComponentAssemblyDescription,
    endpoint: &SubcomponentPortEndpoint,
    depth: usize,
) -> Result<(String, ComponentPortDescription), FlattenError> {
    if depth > 32 {
        return Err(FlattenError::NestedConnector(endpoint.instance.clone()));
    }
    let part = assembly
        .instance(&endpoint.instance)
        .ok_or_else(|| FlattenError::UnknownInstance(endpoint.instance.clone()))?;
    let name = format!("{prefix}_{}", part.name);
    let package = match &part.package {
        SubcomponentPackage::Connector(package) => package,
        // A component part is deployed as itself: its port is the leaf.
        SubcomponentPackage::Component(component) => {
            let port = component
                .realizes
                .port(&endpoint.port_name)
                .ok_or_else(|| FlattenError::UnknownPort {
                    instance: name.clone(),
                    port: endpoint.port_name.clone(),
                })?;
            return Ok((name, port.clone()));
        }
    };
    // The same selection as the deployment of the part.
    let implementation = package.select_implementation(&part.select_requirement)?;
    match &implementation.implementation {
        ConnectorImplementation::Monolithic(_) => {
            let port = package.realizes.port(&endpoint.port_name).ok_or_else(|| {
                FlattenError::UnknownPort {
                    instance: name.clone(),
                    port: endpoint.port_name.clone(),
                }
            })?;
            Ok((name, port.clone()))
        }
        ConnectorImplementation::Assembly(inner) => {
            let inner_endpoint = mapped_endpoint(inner, &package.realizes, &endpoint.port_name)?;
            leaf(&name, inner, &inner_endpoint, depth + 1)
        }
    }
}

/// The part's endpoint the connector port `port` is mapped to in the
/// connector's assembly: the connection whose external endpoint is the
/// port, with exactly one internal endpoint.
fn mapped_endpoint(
    assembly: &ComponentAssemblyDescription,
    connector: &ComponentInterfaceDescription,
    port: &str,
) -> Result<SubcomponentPortEndpoint, FlattenError> {
    let unmapped = || FlattenError::UnmappedConnectorPort {
        connector: connector.specific_type.clone(),
        port: port.to_string(),
    };
    let mut found = assembly
        .connection
        .iter()
        .filter(|c| c.external_endpoint.iter().any(|e| e == port));
    let c = found.next().ok_or_else(unmapped)?;
    if found.next().is_some() {
        return Err(unmapped());
    }
    match c.internal_endpoint.as_slice() {
        [one] => Ok(one.clone()),
        _ => Err(unmapped()),
    }
}

/// §7.6.3: the deployment plan of `assembly`, fully flattened.
///
/// * Each component instance becomes a plan instance on the node the
///   planner chose (`nodes`), with the first implementation of its
///   package; an implementation of this data model has no capabilities,
///   so an instance with selection requirements has none to take.
/// * A basic connection becomes the plan connections between the basic
///   ports of its two endpoints; an extended port and a mirror port of
///   matching port types (§7.6.5) become one connection per basic port.
/// * A connection realized by a connector becomes, for each component
///   endpoint, the fragment that serves it, on the component's node and
///   collocated with it: with a monolithic implementation one fragment
///   instance `<connection>_<instance>_<port>` connected to the
///   connector port that matches the component's port; with an assembly
///   implementation (§7.2.3.4) the part the connector port is mapped to,
///   `<connection>_<part>`, every part placed with the component it
///   serves and the parts of nested assemblies with it, and the internal
///   connections of the assemblies as plan connections. A part is a
///   connector fragment or a component (§7.2.3.4: "an assembly of other
///   components"). Each fragment's implementation carries "home
///   factory" (§7.6.3.1); its configuration is that of the
///   implementation, overridden by the package's, by the use's and by
///   the part's (§7.6.2.2, §7.6.4). A component part is configured with
///   the same layers and created by the container.
/// * The implementation of a connector package is the first whose
///   capabilities satisfy the selection requirements of the use or of
///   the part (§7.6.2.3, §15.9.1); none satisfying them is an error.
///
/// `port_types` resolves the port types of extended ports.
///
/// # Errors
/// [`FlattenError`].
pub fn flatten_assembly(
    assembly: &ComponentAssemblyDescription,
    label: &str,
    uuid: &str,
    nodes: &BTreeMap<String, String>,
    port_types: &ConnectorTypeTable,
) -> Result<DeploymentPlan, FlattenError> {
    let mut f = Flattener {
        port_types,
        plan: DeploymentPlan {
            label: label.to_string(),
            uuid: uuid.to_string(),
            realizes: label.to_string(),
            ..DeploymentPlan::default()
        },
    };
    let mut interfaces: BTreeMap<&str, &ComponentInterfaceDescription> = BTreeMap::new();
    for inst in &assembly.instance {
        let package = match &inst.package {
            SubcomponentPackage::Component(p) => p,
            SubcomponentPackage::Connector(_) => {
                return Err(FlattenError::ConnectorAsInstance(inst.name.clone()));
            }
        };
        package.realizes.validate()?;
        let implementation = select_component_implementation(package, &inst.select_requirement)?;
        let node = nodes
            .get(&inst.name)
            .ok_or_else(|| FlattenError::NoNode(inst.name.clone()))?;
        f.add_implementation(implementation.clone());
        f.plan.instances.push(InstanceDeploymentDescription {
            name: inst.name.clone(),
            implementation: implementation.label.clone(),
            node: node.clone(),
            config_props: inst.config_property.clone(),
            co_locate_with: Vec::new(),
        });
        interfaces.insert(inst.name.as_str(), &package.realizes);
    }
    let port_of = |e: &SubcomponentPortEndpoint| -> Result<ComponentPortDescription, FlattenError> {
        let interface = interfaces
            .get(e.instance.as_str())
            .ok_or_else(|| FlattenError::UnknownInstance(e.instance.clone()))?;
        interface
            .port(&e.port_name)
            .cloned()
            .ok_or_else(|| FlattenError::UnknownPort {
                instance: e.instance.clone(),
                port: e.port_name.clone(),
            })
    };
    for c in &assembly.connection {
        match &c.connector {
            None => {
                let [a, b] = c.internal_endpoint.as_slice() else {
                    return Err(FlattenError::EndpointCount(c.name.clone()));
                };
                let (pa, pb) = (port_of(a)?, port_of(b)?);
                f.link(&c.name, (&a.instance, &pa), (&b.instance, &pb))?;
            }
            Some(used) => {
                let package = &used.package;
                package.realizes.validate()?;
                let implementation = package.select_implementation(&used.select_requirement)?;
                let mut placed: BTreeMap<String, Placement> = BTreeMap::new();
                let mut links: Vec<(SubcomponentPortEndpoint, ComponentPortDescription, String)> =
                    Vec::new();
                for endpoint in &c.internal_endpoint {
                    let component_port = port_of(endpoint)?;
                    let connector_port = f
                        .connector_port(&c.name, endpoint, &component_port, &package.realizes)?
                        .clone();
                    let node = nodes
                        .get(&endpoint.instance)
                        .ok_or_else(|| FlattenError::NoNode(endpoint.instance.clone()))?;
                    let placement = Placement {
                        node: node.clone(),
                        with: endpoint.instance.clone(),
                    };
                    match &implementation.implementation {
                        ConnectorImplementation::Monolithic(m) => {
                            let fragment =
                                format!("{}_{}_{}", c.name, endpoint.instance, endpoint.port_name);
                            f.deploy_fragment(
                                &fragment,
                                package,
                                implementation,
                                m,
                                &placement,
                                &[used.config_property.as_slice()],
                            )?;
                            f.link(
                                &c.name,
                                (&endpoint.instance, &component_port),
                                (&fragment, &connector_port),
                            )?;
                        }
                        ConnectorImplementation::Assembly(inner) => {
                            let part =
                                mapped_endpoint(inner, &package.realizes, &connector_port.name)?;
                            if placed.insert(part.instance.clone(), placement).is_some() {
                                return Err(FlattenError::FragmentPlacement {
                                    connection: c.name.clone(),
                                    fragment: part.instance.clone(),
                                });
                            }
                            links.push((endpoint.clone(), component_port, connector_port.name));
                        }
                    }
                }
                if let ConnectorImplementation::Assembly(inner) = &implementation.implementation {
                    for part in &inner.instance {
                        let placement = placed.get(&part.name).ok_or_else(|| {
                            FlattenError::FragmentPlacement {
                                connection: c.name.clone(),
                                fragment: part.name.clone(),
                            }
                        })?;
                        let layers: [&[Property]; 3] = [
                            implementation.config_property.as_slice(),
                            package.config_property.as_slice(),
                            used.config_property.as_slice(),
                        ];
                        let name = format!("{}_{}", c.name, part.name);
                        f.deploy_part(&c.name, &name, part, placement, &layers, 1)?;
                    }
                    f.link_internal(&c.name, &c.name, inner, 0)?;
                    for (endpoint, component_port, connector_port) in links {
                        let mapped = mapped_endpoint(inner, &package.realizes, &connector_port)?;
                        let (fragment, fragment_port) = leaf(&c.name, inner, &mapped, 0)?;
                        f.link(
                            &c.name,
                            (&endpoint.instance, &component_port),
                            (&fragment, &fragment_port),
                        )?;
                    }
                }
            }
        }
    }
    f.plan.validate().map_err(FlattenError::Plan)?;
    Ok(f.plan)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use zerodds_corba_ccm::connector::{PortTypeDef, PortTypeMember};
    use zerodds_corba_rt::any::AnyValue;

    pub(crate) const DATA: &str = "IDL:Demo/Data_Pusher:1.0";
    pub(crate) const CONTROL: &str = "IDL:Demo/FlowControl:1.0";
    pub(crate) const PORT_TYPE: &str = "IDL:Demo/Data_ControlledConsumer:1.0";
    pub(crate) const CONNECTOR: &str = "IDL:Demo/Data_Cnx:1.0";

    pub(crate) fn port_types() -> ConnectorTypeTable {
        let mut t = ConnectorTypeTable::new();
        t.insert_port_type(PortTypeDef {
            name: "Data_ControlledConsumer".into(),
            repository_id: PORT_TYPE.into(),
            members: alloc::vec![
                PortTypeMember::Provides {
                    name: "consumer".into(),
                    interface_id: DATA.into(),
                },
                PortTypeMember::Uses {
                    name: "control".into(),
                    interface_id: CONTROL.into(),
                    multiple: false,
                },
            ],
        });
        t
    }

    pub(crate) fn connector_interface() -> ComponentInterfaceDescription {
        ComponentInterfaceDescription {
            label: "Data_Cnx".into(),
            uuid: "cnx-iface".into(),
            specific_type: CONNECTOR.into(),
            port: alloc::vec![
                ComponentPortDescription::new(
                    "cc",
                    CcmComponentPortKind::ExtendedMirrorPort,
                    PORT_TYPE
                ),
                ComponentPortDescription::new("p", CcmComponentPortKind::Facet, DATA),
            ],
            property: alloc::vec![ComponentPropertyDescription {
                name: "mcast_addr".into(),
                type_name: "string".into(),
            }],
            ..ComponentInterfaceDescription::default()
        }
    }

    pub(crate) fn monolithic_package() -> ConnectorPackageDescription {
        let mut exec = BTreeMap::new();
        exec.insert(HOME_FACTORY.to_string(), "create_Data_Cnx".to_string());
        ConnectorPackageDescription {
            label: "Data_Cnx".into(),
            uuid: "cnx-pkg".into(),
            realizes: connector_interface(),
            config_property: alloc::vec![
                Property::string("mcast_addr", "224.1.1.1"),
                Property {
                    name: "msg_size".into(),
                    value: AnyValue::ULong(50),
                },
            ],
            implementation: alloc::vec![ConnectorImplementationDescription {
                label: "fragment_impl_1".into(),
                uuid: "frag-impl".into(),
                implements: connector_interface(),
                implementation: ConnectorImplementation::Monolithic(
                    MonolithicImplementationDescription {
                        primary_artifact: alloc::vec!["lib/data_cnx.so".into()],
                        exec_parameter: exec,
                    }
                ),
                config_property: alloc::vec![
                    Property {
                        name: "mcast_port".into(),
                        value: AnyValue::UShort(1),
                    },
                    Property::string("mcast_addr", "239.0.0.1"),
                ],
                capability: Vec::new(),
            }],
        }
    }

    pub(crate) fn component(
        label: &str,
        id: &str,
        ports: Vec<ComponentPortDescription>,
    ) -> ComponentPackageDescription {
        ComponentPackageDescription {
            label: label.into(),
            uuid: format!("{label}-uuid"),
            component_id: id.into(),
            realizes: ComponentInterfaceDescription {
                label: label.into(),
                specific_type: id.into(),
                port: ports,
                ..ComponentInterfaceDescription::default()
            },
            implementations: alloc::vec![ImplementationDescription {
                label: format!("{label}Impl"),
                uuid: format!("{label}-impl"),
                artifacts: alloc::vec![format!("lib/{label}.so")],
                realizes: id.into(),
                ..ImplementationDescription::default()
            }],
        }
    }

    pub(crate) fn consumer() -> SubcomponentInstantiationDescription {
        SubcomponentInstantiationDescription {
            name: "consumer1".into(),
            package: SubcomponentPackage::Component(component(
                "Consumer",
                "IDL:Demo/Consumer:1.0",
                alloc::vec![ComponentPortDescription::new(
                    "p",
                    CcmComponentPortKind::ExtendedPort,
                    PORT_TYPE
                )],
            )),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        }
    }

    pub(crate) fn producer() -> SubcomponentInstantiationDescription {
        SubcomponentInstantiationDescription {
            name: "producer1".into(),
            package: SubcomponentPackage::Component(component(
                "Producer",
                "IDL:Demo/Producer:1.0",
                alloc::vec![ComponentPortDescription::new(
                    "out",
                    CcmComponentPortKind::SimplexReceptacle,
                    DATA
                )],
            )),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        }
    }

    pub(crate) fn nodes() -> BTreeMap<String, String> {
        let mut n = BTreeMap::new();
        n.insert("consumer1".to_string(), "node1".to_string());
        n.insert("producer1".to_string(), "node2".to_string());
        n
    }

    pub(crate) fn assembly() -> ComponentAssemblyDescription {
        ComponentAssemblyDescription {
            instance: alloc::vec![consumer(), producer()],
            connection: alloc::vec![AssemblyConnectionDescription {
                name: "link".into(),
                connector: Some(ConnectorUse {
                    package: monolithic_package(),
                    select_requirement: Vec::new(),
                    config_property: alloc::vec![Property {
                        name: "mcast_port".into(),
                        value: AnyValue::UShort(31337),
                    }],
                }),
                internal_endpoint: alloc::vec![
                    SubcomponentPortEndpoint {
                        instance: "consumer1".into(),
                        port_name: "p".into(),
                    },
                    SubcomponentPortEndpoint {
                        instance: "producer1".into(),
                        port_name: "out".into(),
                    },
                ],
                external_endpoint: Vec::new(),
            }],
        }
    }

    fn connection_set(plan: &DeploymentPlan) -> Vec<String> {
        plan.connections
            .iter()
            .map(|c| {
                format!(
                    "{}.{} -> {}.{}",
                    c.source_instance, c.source_port, c.target_instance, c.target_port
                )
            })
            .collect()
    }

    #[test]
    fn corba_p3_7_6_3_an_assembly_with_a_connector_flattens_to_fragment_instances() {
        let plan =
            flatten_assembly(&assembly(), "App", "app-uuid", &nodes(), &port_types()).unwrap();
        let instances: Vec<(&str, &str, &str, &[String])> = plan
            .instances
            .iter()
            .map(|i| {
                (
                    i.name.as_str(),
                    i.implementation.as_str(),
                    i.node.as_str(),
                    i.co_locate_with.as_slice(),
                )
            })
            .collect();
        assert_eq!(
            instances,
            [
                ("consumer1", "ConsumerImpl", "node1", &[][..]),
                ("producer1", "ProducerImpl", "node2", &[][..]),
                (
                    "link_consumer1_p",
                    "fragment_impl_1",
                    "node1",
                    &["consumer1".to_string()][..]
                ),
                (
                    "link_producer1_out",
                    "fragment_impl_1",
                    "node2",
                    &["producer1".to_string()][..]
                ),
            ]
        );
        // The fragment implementation once, with its artifact and entry point.
        let fragment_impl: Vec<&ImplementationDescription> = plan
            .implementations
            .iter()
            .filter(|i| i.label == "fragment_impl_1")
            .collect();
        assert_eq!(fragment_impl.len(), 1);
        assert_eq!(fragment_impl[0].artifacts, ["lib/data_cnx.so"]);
        assert_eq!(fragment_impl[0].realizes, CONNECTOR);
        assert_eq!(
            fragment_impl[0]
                .exec_params
                .get(HOME_FACTORY)
                .map(String::as_str),
            Some("create_Data_Cnx")
        );
        // Configuration: implementation < package < use (§7.6.2.2).
        let config = &plan.instances[2].config_props;
        let find = |n: &str| config.iter().find(|p| p.name == n).map(|p| p.value.clone());
        assert_eq!(find("mcast_addr"), Some(AnyValue::Str("224.1.1.1".into())));
        assert_eq!(find("mcast_port"), Some(AnyValue::UShort(31337)));
        assert_eq!(find("msg_size"), Some(AnyValue::ULong(50)));
        // Basic connections: the port's provides is a facet of the
        // component and a receptacle of the mirror port; its uses the
        // other way round; the producer's receptacle to the connector's
        // facet.
        assert_eq!(
            connection_set(&plan),
            [
                "link_consumer1_p.cc_consumer -> consumer1.p_consumer",
                "consumer1.p_control -> link_consumer1_p.cc_control",
                "producer1.out -> link_producer1_out.p",
            ]
        );
    }

    #[test]
    fn corba_p3_7_6_3_flattening_refuses_what_has_no_plan() {
        let pt = port_types();
        // No node for a component.
        let mut few = nodes();
        few.remove("producer1");
        assert_eq!(
            flatten_assembly(&assembly(), "A", "u", &few, &pt).unwrap_err(),
            FlattenError::NoNode("producer1".into())
        );
        // A fragment implementation without the entry point.
        let mut a = assembly();
        if let Some(used) = a.connection[0].connector.as_mut()
            && let ConnectorImplementation::Monolithic(m) =
                &mut used.package.implementation[0].implementation
        {
            m.exec_parameter.clear();
        }
        assert_eq!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt).unwrap_err(),
            FlattenError::MissingHomeFactory("fragment_impl_1".into())
        );
        // A connector as an instance of the assembly.
        let mut a = assembly();
        a.instance.push(SubcomponentInstantiationDescription {
            name: "cnx".into(),
            package: SubcomponentPackage::Connector(monolithic_package()),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        });
        assert_eq!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt).unwrap_err(),
            FlattenError::ConnectorAsInstance("cnx".into())
        );
        // A component port the connector has no port for.
        let mut a = assembly();
        a.connection[0].internal_endpoint[1].port_name = "nope".into();
        assert!(matches!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt),
            Err(FlattenError::UnknownPort { .. })
        ));
        let mut a = assembly();
        if let SubcomponentPackage::Component(p) = &mut a.instance[1].package {
            p.realizes.port[0].specific_type = CONTROL.into();
        }
        assert!(matches!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt),
            Err(FlattenError::NoMatchingConnectorPort { .. })
        ));
        // Two connector ports that both fit.
        let mut a = assembly();
        if let Some(used) = a.connection[0].connector.as_mut() {
            used.package
                .realizes
                .port
                .push(ComponentPortDescription::new(
                    "p2",
                    CcmComponentPortKind::Facet,
                    DATA,
                ));
        }
        assert!(matches!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt),
            Err(FlattenError::AmbiguousConnectorPort { .. })
        ));
        // An unknown port type is said as such, not as a port without a
        // match.
        assert_eq!(
            flatten_assembly(&assembly(), "A", "u", &nodes(), &ConnectorTypeTable::new()),
            Err(FlattenError::UnknownPortType(PORT_TYPE.into()))
        );
        // §7.6.2.4: template parameters on a basic port.
        let mut a = assembly();
        if let SubcomponentPackage::Component(p) = &mut a.instance[1].package {
            p.realizes.port[0].template_param = alloc::vec!["IDL:Demo/T:1.0".into()];
        }
        assert!(matches!(
            flatten_assembly(&a, "A", "u", &nodes(), &pt),
            Err(FlattenError::TemplateParamOnBasicPort { .. })
        ));
    }

    #[test]
    fn corba_p3_7_6_3_a_basic_connection_and_a_direct_extended_port_link() {
        let mut a = ComponentAssemblyDescription {
            instance: alloc::vec![consumer(), producer()],
            connection: Vec::new(),
        };
        // A third component with a mirror port of the same port type and
        // a facet the producer uses.
        a.instance.push(SubcomponentInstantiationDescription {
            name: "server1".into(),
            package: SubcomponentPackage::Component(component(
                "Server",
                "IDL:Demo/Server:1.0",
                alloc::vec![
                    ComponentPortDescription::new(
                        "m",
                        CcmComponentPortKind::ExtendedMirrorPort,
                        PORT_TYPE,
                    ),
                    ComponentPortDescription::new("sink", CcmComponentPortKind::Facet, DATA),
                ],
            )),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        });
        let endpoint = |i: &str, p: &str| SubcomponentPortEndpoint {
            instance: i.into(),
            port_name: p.into(),
        };
        a.connection.push(AssemblyConnectionDescription {
            name: "direct".into(),
            connector: None,
            internal_endpoint: alloc::vec![endpoint("consumer1", "p"), endpoint("server1", "m")],
            external_endpoint: Vec::new(),
        });
        a.connection.push(AssemblyConnectionDescription {
            name: "basic".into(),
            connector: None,
            internal_endpoint: alloc::vec![
                endpoint("server1", "sink"),
                endpoint("producer1", "out")
            ],
            external_endpoint: Vec::new(),
        });
        let mut n = nodes();
        n.insert("server1".into(), "node3".into());
        let plan = flatten_assembly(&a, "A", "u", &n, &port_types()).unwrap();
        assert_eq!(
            connection_set(&plan),
            [
                "server1.m_consumer -> consumer1.p_consumer",
                "consumer1.p_control -> server1.m_control",
                "producer1.out -> server1.sink",
            ]
        );
        // Three endpoints on a basic connection.
        a.connection[1]
            .internal_endpoint
            .push(endpoint("consumer1", "p"));
        assert_eq!(
            flatten_assembly(&a, "A", "u", &n, &port_types()).unwrap_err(),
            FlattenError::EndpointCount("basic".into())
        );
    }

    pub(crate) const SOCKET: &str = "IDL:Demo/Socket:1.0";

    /// A connector package with one monolithic implementation
    /// `<label>_impl` whose entry point is `create_<label>`.
    pub(crate) fn leaf_package(
        label: &str,
        ports: Vec<ComponentPortDescription>,
    ) -> ConnectorPackageDescription {
        let mut exec = BTreeMap::new();
        exec.insert(HOME_FACTORY.to_string(), format!("create_{label}"));
        let interface = ComponentInterfaceDescription {
            label: label.into(),
            specific_type: format!("IDL:Demo/{label}:1.0"),
            port: ports,
            ..ComponentInterfaceDescription::default()
        };
        ConnectorPackageDescription {
            label: label.into(),
            uuid: format!("{label}-pkg"),
            realizes: interface.clone(),
            config_property: Vec::new(),
            implementation: alloc::vec![ConnectorImplementationDescription {
                label: format!("{label}_impl"),
                uuid: format!("{label}-impl"),
                implements: interface,
                implementation: ConnectorImplementation::Monolithic(
                    MonolithicImplementationDescription {
                        primary_artifact: alloc::vec![format!("lib/{label}.so")],
                        exec_parameter: exec,
                    },
                ),
                config_property: Vec::new(),
                capability: Vec::new(),
            }],
        }
    }

    /// Part 3 §7.2.3.4, Figures 7.4 and 7.5: a distributed FIFO whose
    /// implementation is an assembly of the fragments `SocketClient`
    /// (with the producer) and `FIFO_Socket_f_pull` (with the consumer);
    /// the second is itself an assembly of `SocketServer` and `ConnFIFO`.
    pub(crate) fn composite_assembly() -> ComponentAssemblyDescription {
        let port = ComponentPortDescription::new;
        use CcmComponentPortKind as K;
        let socket_client = leaf_package(
            "SocketClient",
            alloc::vec![
                port("push", K::Facet, DATA),
                port("link", K::SimplexReceptacle, SOCKET)
            ],
        );
        let socket_server = leaf_package(
            "SocketServer",
            alloc::vec![
                port("link", K::Facet, SOCKET),
                port("out", K::SimplexReceptacle, DATA)
            ],
        );
        let conn_fifo = leaf_package(
            "ConnFIFO",
            alloc::vec![port("in", K::Facet, DATA), port("pull", K::Facet, CONTROL)],
        );
        let endpoint = |i: &str, p: &str| SubcomponentPortEndpoint {
            instance: i.into(),
            port_name: p.into(),
        };
        let part = |name: &str, package: ConnectorPackageDescription| {
            SubcomponentInstantiationDescription {
                name: name.into(),
                package: SubcomponentPackage::Connector(package),
                config_property: Vec::new(),
                select_requirement: Vec::new(),
            }
        };
        // FIFO_Socket_f_pull = SocketServer + ConnFIFO (Figure 7.5).
        let pull_interface = ComponentInterfaceDescription {
            label: "FIFO_Socket_f_pull".into(),
            specific_type: "IDL:Demo/FIFO_Socket_f_pull:1.0".into(),
            port: alloc::vec![
                port("link", K::Facet, SOCKET),
                port("pull", K::Facet, CONTROL)
            ],
            ..ComponentInterfaceDescription::default()
        };
        let pull = ConnectorPackageDescription {
            label: "FIFO_Socket_f_pull".into(),
            uuid: "pull-pkg".into(),
            realizes: pull_interface.clone(),
            config_property: alloc::vec![Property::string("queue", "fifo")],
            implementation: alloc::vec![ConnectorImplementationDescription {
                label: "pull_assembly".into(),
                uuid: "pull-impl".into(),
                implements: pull_interface,
                implementation: ConnectorImplementation::Assembly(ComponentAssemblyDescription {
                    instance: alloc::vec![part("server", socket_server), part("fifo", conn_fifo)],
                    connection: alloc::vec![
                        AssemblyConnectionDescription {
                            name: "inner".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![
                                endpoint("server", "out"),
                                endpoint("fifo", "in")
                            ],
                            external_endpoint: Vec::new(),
                        },
                        AssemblyConnectionDescription {
                            name: "x_link".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![endpoint("server", "link")],
                            external_endpoint: alloc::vec!["link".into()],
                        },
                        AssemblyConnectionDescription {
                            name: "x_pull".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![endpoint("fifo", "pull")],
                            external_endpoint: alloc::vec!["pull".into()],
                        },
                    ],
                }),
                config_property: Vec::new(),
                capability: Vec::new(),
            }],
        };
        // The FIFO connector = SocketClient + FIFO_Socket_f_pull (Figure 7.4).
        let fifo_interface = ComponentInterfaceDescription {
            label: "FIFO".into(),
            specific_type: "IDL:Demo/FIFO:1.0".into(),
            port: alloc::vec![
                port("push", K::Facet, DATA),
                port("pull", K::Facet, CONTROL)
            ],
            ..ComponentInterfaceDescription::default()
        };
        let fifo = ConnectorPackageDescription {
            label: "FIFO".into(),
            uuid: "fifo-pkg".into(),
            realizes: fifo_interface.clone(),
            config_property: Vec::new(),
            implementation: alloc::vec![ConnectorImplementationDescription {
                label: "fifo_assembly".into(),
                uuid: "fifo-impl".into(),
                implements: fifo_interface,
                implementation: ConnectorImplementation::Assembly(ComponentAssemblyDescription {
                    instance: alloc::vec![part("client", socket_client), part("puller", pull)],
                    connection: alloc::vec![
                        AssemblyConnectionDescription {
                            name: "socket".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![
                                endpoint("client", "link"),
                                endpoint("puller", "link")
                            ],
                            external_endpoint: Vec::new(),
                        },
                        AssemblyConnectionDescription {
                            name: "x_push".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![endpoint("client", "push")],
                            external_endpoint: alloc::vec!["push".into()],
                        },
                        AssemblyConnectionDescription {
                            name: "x_pull".into(),
                            connector: None,
                            internal_endpoint: alloc::vec![endpoint("puller", "pull")],
                            external_endpoint: alloc::vec!["pull".into()],
                        },
                    ],
                }),
                config_property: Vec::new(),
                capability: Vec::new(),
            }],
        };
        let producer = SubcomponentInstantiationDescription {
            name: "producer1".into(),
            package: SubcomponentPackage::Component(component(
                "Producer",
                "IDL:Demo/Producer:1.0",
                alloc::vec![port("out", K::SimplexReceptacle, DATA)],
            )),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        };
        let consumer = SubcomponentInstantiationDescription {
            name: "consumer1".into(),
            package: SubcomponentPackage::Component(component(
                "Consumer",
                "IDL:Demo/Consumer:1.0",
                alloc::vec![port("pull", K::SimplexReceptacle, CONTROL)],
            )),
            config_property: Vec::new(),
            select_requirement: Vec::new(),
        };
        ComponentAssemblyDescription {
            instance: alloc::vec![producer, consumer],
            connection: alloc::vec![AssemblyConnectionDescription {
                name: "q".into(),
                connector: Some(ConnectorUse {
                    package: fifo,
                    select_requirement: Vec::new(),
                    config_property: Vec::new(),
                }),
                internal_endpoint: alloc::vec![
                    endpoint("producer1", "out"),
                    endpoint("consumer1", "pull")
                ],
                external_endpoint: Vec::new(),
            }],
        }
    }

    /// Part 3 §7.2.3.4: the composite connector of
    /// [`composite_assembly`] flattens into its fragments, each placed
    /// with the component it serves.
    #[test]
    fn corba_p3_7_2_3_4_a_composite_connector_flattens_through_its_fragment_assemblies() {
        let a = composite_assembly();
        let plan = flatten_assembly(&a, "A", "u", &nodes(), &port_types()).unwrap();
        let placed: Vec<(&str, &str, &str)> = plan
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i.implementation.as_str(), i.node.as_str()))
            .collect();
        assert_eq!(
            placed,
            [
                ("producer1", "ProducerImpl", "node2"),
                ("consumer1", "ConsumerImpl", "node1"),
                ("q_client", "SocketClient_impl", "node2"),
                ("q_puller_server", "SocketServer_impl", "node1"),
                ("q_puller_fifo", "ConnFIFO_impl", "node1"),
            ]
        );
        // The part's package configuration reaches the parts of its
        // assembly.
        let server = plan
            .instances
            .iter()
            .find(|i| i.name == "q_puller_server")
            .unwrap();
        assert_eq!(server.config_props, [Property::string("queue", "fifo")]);
        assert_eq!(
            connection_set(&plan),
            [
                "q_puller_server.out -> q_puller_fifo.in",
                "q_client.link -> q_puller_server.link",
                "producer1.out -> q_client.push",
                "consumer1.pull -> q_puller_fifo.pull",
            ]
        );
        // Every fragment implementation has its entry point.
        let fragments: Vec<&ImplementationDescription> = plan
            .implementations
            .iter()
            .filter(|i| i.label.ends_with("_impl"))
            .collect();
        assert_eq!(fragments.len(), 3);
        assert!(
            fragments
                .iter()
                .all(|i| i.exec_params.contains_key(HOME_FACTORY))
        );
        // A part no component reaches has no node.
        let mut unplaced = a.clone();
        unplaced.connection[0].internal_endpoint.pop();
        assert!(matches!(
            flatten_assembly(&unplaced, "A", "u", &nodes(), &port_types()),
            Err(FlattenError::FragmentPlacement { .. })
        ));
    }

    fn satisfier(name: &str, kind: SatisfierPropertyKind, value: AnyValue) -> SatisfierProperty {
        SatisfierProperty {
            name: name.into(),
            kind,
            dynamic: false,
            value,
        }
    }

    fn requirement(resource_type: &str, property: Vec<Property>) -> Requirement {
        Requirement {
            name: "r".into(),
            resource_type: resource_type.into(),
            property,
        }
    }

    /// The monolithic package of [`assembly`] with a second
    /// implementation: `udp_impl` (first) and `shm_impl`, each tagged
    /// with a `Transport` capability.
    pub(crate) fn two_implementations() -> ConnectorPackageDescription {
        let mut package = monolithic_package();
        let tagged = |label: &str, kind: &str, max_msg: i32| {
            let mut implementation = package.implementation[0].clone();
            implementation.label = label.into();
            implementation.uuid = format!("{label}-uuid");
            implementation.capability = alloc::vec![Capability {
                name: "transport".into(),
                resource_type: alloc::vec!["Transport".into()],
                property: alloc::vec![
                    satisfier(
                        "kind",
                        SatisfierPropertyKind::Attribute,
                        AnyValue::Str(kind.into())
                    ),
                    satisfier(
                        "max_msg",
                        SatisfierPropertyKind::Maximum,
                        AnyValue::Long(max_msg)
                    ),
                ],
            }];
            implementation
        };
        package.implementation = alloc::vec![
            tagged("udp_impl", "udp", 1024),
            tagged("shm_impl", "shm", 65536)
        ];
        package
    }

    /// §7.6.2.3: "Implementations may be tagged with user-defined
    /// capabilities. Administrators can then select among implementations
    /// using selection requirements in a PackageConfiguration." Planning
    /// takes the first implementation whose capabilities satisfy the
    /// requirements of the use; none satisfying them is an error.
    #[test]
    fn corba_p3_7_6_2_3_selection_requirements_choose_among_implementations() {
        let package = two_implementations();
        let label = |requirements: &[Requirement]| {
            package
                .select_implementation(requirements)
                .map(|i| i.label.clone())
        };
        assert_eq!(label(&[]), Ok("udp_impl".into()));
        let shm = requirement("Transport", alloc::vec![Property::string("kind", "shm")]);
        assert_eq!(label(core::slice::from_ref(&shm)), Ok("shm_impl".into()));
        // `max_msg` is a Maximum: 2048 is above udp_impl's 1024.
        let large = requirement(
            "Transport",
            alloc::vec![Property {
                name: "max_msg".into(),
                value: AnyValue::Long(2048),
            }],
        );
        assert_eq!(label(&[large]), Ok("shm_impl".into()));
        let udp_large = requirement(
            "Transport",
            alloc::vec![
                Property::string("kind", "udp"),
                Property {
                    name: "max_msg".into(),
                    value: AnyValue::Long(2048),
                },
            ],
        );
        assert_eq!(
            label(&[udp_large]),
            Err(FlattenError::NoSatisfyingImplementation("Data_Cnx".into()))
        );
        // Another resource type, a property no capability has.
        assert!(label(&[requirement("Storage", Vec::new())]).is_err());
        assert!(
            label(&[requirement(
                "Transport",
                alloc::vec![Property::string("latency", "low")]
            )])
            .is_err()
        );

        // Through the plan: the fragments take the implementation the
        // use's requirements select.
        let mut a = assembly();
        if let Some(used) = a.connection[0].connector.as_mut() {
            used.package = two_implementations();
            used.select_requirement = alloc::vec![shm];
        }
        let plan = flatten_assembly(&a, "A", "u", &nodes(), &port_types()).unwrap();
        let fragments: Vec<&str> = plan
            .instances
            .iter()
            .filter(|i| i.name.starts_with("link_"))
            .map(|i| i.implementation.as_str())
            .collect();
        assert_eq!(fragments, ["shm_impl", "shm_impl"]);
        assert!(plan.implementations.iter().all(|i| i.label != "udp_impl"));
        if let Some(used) = a.connection[0].connector.as_mut() {
            used.select_requirement = alloc::vec![requirement("Storage", Vec::new())];
        }
        assert_eq!(
            flatten_assembly(&a, "A", "u", &nodes(), &port_types()),
            Err(FlattenError::NoSatisfyingImplementation("Data_Cnx".into()))
        );
        // A component implementation carries no capabilities: a component
        // instance with a selection requirement has none to take.
        let mut a = assembly();
        a.instance[0].select_requirement = alloc::vec![requirement("Transport", Vec::new())];
        assert_eq!(
            flatten_assembly(&a, "A", "u", &nodes(), &port_types()),
            Err(FlattenError::NoSatisfyingImplementation("Consumer".into()))
        );
    }

    /// Part 3 §15.4.5: each satisfier kind meets the requirement's value
    /// of the same type by its own rule; a value of another type is an
    /// error, not a mismatch.
    #[test]
    fn corba_p3_7_6_2_3_each_satisfier_kind_meets_a_requirement_by_its_rule() {
        let meets = |kind: SatisfierPropertyKind, offered: AnyValue, wanted: AnyValue| {
            let capability = Capability {
                name: "c".into(),
                resource_type: alloc::vec!["T".into()],
                property: alloc::vec![satisfier("p", kind, offered)],
            };
            requirement(
                "T",
                alloc::vec![Property {
                    name: "p".into(),
                    value: wanted,
                }],
            )
            .satisfied_by(&capability)
        };
        use SatisfierPropertyKind as K;
        // Quantity: an unsigned long count; the requirement's value is
        // ignored.
        assert_eq!(
            meets(K::Quantity, AnyValue::ULong(2), AnyValue::Null),
            Ok(true)
        );
        assert_eq!(
            meets(K::Quantity, AnyValue::ULong(0), AnyValue::Null),
            Ok(false)
        );
        // Capacity: at most the capacity.
        assert_eq!(
            meets(K::Capacity, AnyValue::ULong(100), AnyValue::ULong(50)),
            Ok(true)
        );
        assert_eq!(
            meets(K::Capacity, AnyValue::ULong(100), AnyValue::ULong(150)),
            Ok(false)
        );
        assert_eq!(
            meets(K::Capacity, AnyValue::Double(1.5), AnyValue::Double(1.5)),
            Ok(true)
        );
        // Maximum and Minimum.
        assert_eq!(
            meets(K::Maximum, AnyValue::Long(10), AnyValue::Long(10)),
            Ok(true)
        );
        assert_eq!(
            meets(K::Maximum, AnyValue::Long(10), AnyValue::Long(11)),
            Ok(false)
        );
        assert_eq!(
            meets(K::Minimum, AnyValue::Long(10), AnyValue::Long(12)),
            Ok(true)
        );
        assert_eq!(
            meets(K::Minimum, AnyValue::Double(10.0), AnyValue::Double(9.5)),
            Ok(false)
        );
        // Attribute: equal; an enumeration against its literal.
        assert_eq!(
            meets(
                K::Attribute,
                AnyValue::Str("x".into()),
                AnyValue::Str("x".into())
            ),
            Ok(true)
        );
        assert_eq!(
            meets(K::Attribute, AnyValue::Long(3), AnyValue::Long(4)),
            Ok(false)
        );
        let colour = AnyValue::Enum {
            repo_id: "IDL:Demo/Colour:1.0".into(),
            name: "Colour".into(),
            value: 1,
            members: alloc::vec!["RED".into(), "GREEN".into()],
        };
        assert_eq!(
            meets(K::Attribute, colour.clone(), AnyValue::Str("GREEN".into())),
            Ok(true)
        );
        assert_eq!(
            meets(K::Attribute, colour, AnyValue::Str("RED".into())),
            Ok(false)
        );
        // Selection: one of the values.
        let choice = AnyValue::Seq {
            element: zerodds_corba_rt::TypeCode::String(0),
            items: alloc::vec![AnyValue::Str("a".into()), AnyValue::Str("b".into())],
        };
        assert_eq!(
            meets(K::Selection, choice.clone(), AnyValue::Str("b".into())),
            Ok(true)
        );
        assert_eq!(
            meets(K::Selection, choice, AnyValue::Str("c".into())),
            Ok(false)
        );
        // A value of a type the kind does not take.
        let invalid = Err(FlattenError::RequirementValue {
            requirement: "r".into(),
            property: "p".into(),
        });
        assert_eq!(
            meets(K::Maximum, AnyValue::Long(1), AnyValue::Str("x".into())),
            invalid
        );
        assert_eq!(
            meets(K::Quantity, AnyValue::Long(1), AnyValue::Null),
            invalid
        );
        assert_eq!(
            meets(K::Capacity, AnyValue::ULong(1), AnyValue::Long(1)),
            invalid
        );
        assert_eq!(
            meets(
                K::Selection,
                AnyValue::Str("a".into()),
                AnyValue::Str("a".into())
            ),
            invalid
        );
    }

    /// §7.2.3.4: "such an implementation may be an assembly of other
    /// components … These fragments can be considered as sub-components
    /// within an assembly". A component as a part of a connector's
    /// assembly is deployed with the component it serves, configured like
    /// the fragments beside it, and connected through its ports.
    #[test]
    fn corba_p3_7_2_3_4_a_component_part_of_a_connector_assembly_is_deployed_with_the_component_it_serves()
     {
        use CcmComponentPortKind as K;
        let port = ComponentPortDescription::new;
        let mut a = composite_assembly();
        let fifo_component = component(
            "ConnFIFO",
            "IDL:Demo/ConnFIFO:1.0",
            alloc::vec![port("in", K::Facet, DATA), port("pull", K::Facet, CONTROL)],
        );
        {
            let used = a.connection[0].connector.as_mut().unwrap();
            let ConnectorImplementation::Assembly(fifo) =
                &mut used.package.implementation[0].implementation
            else {
                panic!("the FIFO is an assembly");
            };
            let puller = fifo
                .instance
                .iter_mut()
                .find(|i| i.name == "puller")
                .unwrap();
            let SubcomponentPackage::Connector(pull) = &mut puller.package else {
                panic!("the puller is a connector fragment");
            };
            let ConnectorImplementation::Assembly(inner) =
                &mut pull.implementation[0].implementation
            else {
                panic!("the puller is an assembly");
            };
            let part = inner
                .instance
                .iter_mut()
                .find(|i| i.name == "fifo")
                .unwrap();
            part.package = SubcomponentPackage::Component(fifo_component);
        }
        let plan = flatten_assembly(&a, "A", "u", &nodes(), &port_types()).unwrap();
        let fifo = plan
            .instances
            .iter()
            .find(|i| i.name == "q_puller_fifo")
            .expect("the component part");
        assert_eq!(fifo.implementation, "ConnFIFOImpl");
        assert_eq!(fifo.node, "node1");
        assert_eq!(fifo.co_locate_with, ["consumer1"]);
        assert_eq!(fifo.config_props, [Property::string("queue", "fifo")]);
        let implementation = plan
            .implementations
            .iter()
            .find(|i| i.label == "ConnFIFOImpl")
            .expect("its implementation in the plan");
        assert_eq!(implementation.realizes, "IDL:Demo/ConnFIFO:1.0");
        assert_eq!(
            connection_set(&plan),
            [
                "q_puller_server.out -> q_puller_fifo.in",
                "q_client.link -> q_puller_server.link",
                "producer1.out -> q_client.push",
                "consumer1.pull -> q_puller_fifo.pull",
            ]
        );
        // A port the component does not have.
        let mut wrong = a.clone();
        if let Some(used) = wrong.connection[0].connector.as_mut()
            && let ConnectorImplementation::Assembly(fifo) =
                &mut used.package.implementation[0].implementation
            && let SubcomponentPackage::Connector(pull) = &mut fifo.instance[1].package
            && let ConnectorImplementation::Assembly(inner) =
                &mut pull.implementation[0].implementation
        {
            inner.connection[0].internal_endpoint[1].port_name = "nope".into();
        }
        assert!(matches!(
            flatten_assembly(&wrong, "A", "u", &nodes(), &port_types()),
            Err(FlattenError::UnknownPort { .. })
        ));
    }
}
