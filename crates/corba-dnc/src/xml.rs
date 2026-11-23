// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XML of the deployment descriptors — D&C §10 and CORBA 3.3 Part 3 §7.6.
//!
//! Reads and writes:
//!
//! * the deployment plan ([`parse_plan_xml`], [`plan_to_xml`]), with the
//!   execution parameters of an implementation (`<execParameter>`, where a
//!   fragment's implementation carries "home factory", Part 3 §7.6.3.1)
//!   and the typed configuration of an instance in the form of Part 3
//!   §7.6.4:
//!
//!   ```xml
//!   <deploymentPlan>
//!     <label>Plan1</label>
//!     <UUID>uuid-1</UUID>
//!     <implementation>
//!       <label>EchoImpl</label>
//!       <UUID>echo</UUID>
//!       <artifact>lib/echo.so</artifact>
//!       <realizes>IDL:demo/Echo:1.0</realizes>
//!       <execParameter><name>home factory</name><value>create_Echo</value></execParameter>
//!     </implementation>
//!     <instance>
//!       <name>echo1</name>
//!       <implementation>EchoImpl</implementation>
//!       <node>Node1</node>
//!       <configProperty><name>mcast_port</name><type>unsigned short</type><value>31337</value></configProperty>
//!       <coLocateWith>echo2</coLocateWith>
//!     </instance>
//!     <connection>
//!       <name>c</name>
//!       <sourceInstance>echo1</sourceInstance><sourcePort>out</sourcePort>
//!       <targetInstance>echo2</targetInstance><targetPort>in</targetPort>
//!     </connection>
//!   </deploymentPlan>
//!   ```
//!
//! * the component interface description of components and connectors
//!   (`.ccd`, [`parse_interface_xml`], [`interface_to_xml`]) with the port
//!   kinds `ExtendedPort` and `ExtendedMirrorPort` and `<templateParam>`
//!   (§7.6.2.3, §7.6.2.4);
//! * the connector package description with its implementations, each
//!   `<monolithicImpl>` or `<assemblyImpl>`
//!   ([`parse_connector_package_xml`], [`connector_package_to_xml`],
//!   §7.6.2.2, §7.6.2.3);
//! * the component assembly description, whose connections may name a
//!   connector ([`parse_assembly_xml`], [`assembly_to_xml`], §7.6.2.5).
//!
//! The implementations of a connector package carry their capabilities
//! and a connector use and a subcomponent instance their selection
//! requirements (`<capability>`, `<selectRequirement>`, §7.6.2.3,
//! §15.9.1):
//!
//! ```xml
//! <capability>
//!   <name>transport</name>
//!   <resourceType>Transport</resourceType>
//!   <property><name>kind</name><kind>Attribute</kind><dynamic>false</dynamic>
//!     <type>string</type><value>shm</value></property>
//! </capability>
//! <selectRequirement>
//!   <name>fast</name>
//!   <resourceType>Transport</resourceType>
//!   <property><name>kind</name><type>string</type><value>shm</value></property>
//! </selectRequirement>
//! ```
//!
//! A satisfier property of kind `Selection` has one `<value>` per value of
//! its sequence; every other kind has one.
//!
//! An element a descriptor does not define is refused, not skipped. The
//! XMI `id` of an `<instance>` is an identifier of the document and is not
//! read. An instance of a plan names its implementation by label, as the
//! text of `<implementation>`, or by reference, as the example of Part 3
//! §7.6.4 does: `<implementation ref="fragment_impl_1"/>` (also
//! `xmi:idref`) names the plan implementation of that `id` (`xmi:id`), or,
//! when none has it, the one of that label.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::connector::{
    AssemblyConnectionDescription, Capability, CcmComponentPortKind, ComponentAssemblyDescription,
    ComponentInterfaceDescription, ComponentPortDescription, ComponentPropertyDescription,
    ConnectorImplementation, ConnectorImplementationDescription, ConnectorPackageDescription,
    ConnectorUse, MonolithicImplementationDescription, Requirement, SatisfierProperty,
    SatisfierPropertyKind, SubcomponentInstantiationDescription, SubcomponentPackage,
    SubcomponentPortEndpoint,
};
use crate::plan::{
    ComponentPackageDescription, DeploymentPlan, ImplementationDependency,
    ImplementationDescription, InstanceDeploymentDescription, PlanConnection,
};
use crate::property::{
    Property, PropertyError, type_code_from_text, type_code_to_text, value_from_text, value_to_text,
};
use zerodds_corba_rt::any::AnyValue;

/// XML parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Expected tag not found.
    ExpectedTag(String),
    /// Tag is not closed.
    UnterminatedTag,
    /// Plan validation failed.
    ValidationFailed(String),
    /// The text is no well-formed XML.
    InvalidXml(String),
    /// An element the descriptor does not define at this place.
    UnexpectedElement {
        /// The enclosing element.
        parent: String,
        /// The element.
        element: String,
    },
    /// The text of an element is no value of its type.
    InvalidValue {
        /// The element.
        element: String,
        /// The text.
        value: String,
    },
    /// A property without a text form or with an invalid value.
    Property(PropertyError),
    /// The descriptors nest deeper than [`MAX_DEPTH`].
    TooDeep,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ExpectedTag(t) => write!(f, "expected tag <{t}>"),
            Self::UnterminatedTag => f.write_str("unterminated tag"),
            Self::ValidationFailed(msg) => write!(f, "validation: {msg}"),
            Self::InvalidXml(msg) => write!(f, "not well-formed XML: {msg}"),
            Self::UnexpectedElement { parent, element } => {
                write!(f, "<{element}> is not defined in <{parent}>")
            }
            Self::InvalidValue { element, value } => {
                write!(f, "`{value}` is no value of <{element}>")
            }
            Self::Property(e) => write!(f, "{e}"),
            Self::TooDeep => write!(f, "the descriptor nests deeper than {MAX_DEPTH}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

impl From<PropertyError> for ParseError {
    fn from(e: PropertyError) -> Self {
        Self::Property(e)
    }
}

/// The deepest nesting of elements read.
pub const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// The element tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Node {
    name: String,
    text: String,
    children: Vec<Node>,
    /// The attributes by local name (`xmi:idref` is `idref`).
    attributes: Vec<(String, String)>,
}

/// zerodds-lint: recursion-depth 64 — bounded by [`MAX_DEPTH`].
fn convert(node: roxmltree::Node<'_, '_>, depth: usize) -> Result<Node, ParseError> {
    if depth > MAX_DEPTH {
        return Err(ParseError::TooDeep);
    }
    let mut out = Node {
        name: node.tag_name().name().to_string(),
        text: String::new(),
        children: Vec::new(),
        attributes: node
            .attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect(),
    };
    for child in node.children() {
        if child.is_element() {
            out.children.push(convert(child, depth + 1)?);
        } else if child.is_text() {
            out.text.push_str(child.text().unwrap_or_default());
        }
    }
    Ok(out)
}

fn parse_root(input: &str, root: &str) -> Result<Node, ParseError> {
    let doc =
        roxmltree::Document::parse(input).map_err(|e| ParseError::InvalidXml(e.to_string()))?;
    let node = convert(doc.root_element(), 0)?;
    if node.name != root {
        return Err(ParseError::ExpectedTag(root.to_string()));
    }
    Ok(node)
}

impl Node {
    /// Refuses a child not in `allowed`.
    fn only(&self, allowed: &[&str]) -> Result<(), ParseError> {
        for c in &self.children {
            if !allowed.contains(&c.name.as_str()) {
                return Err(ParseError::UnexpectedElement {
                    parent: self.name.clone(),
                    element: c.name.clone(),
                });
            }
        }
        Ok(())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    fn require(&self, name: &str) -> Result<&Node, ParseError> {
        self.child(name)
            .ok_or_else(|| ParseError::ExpectedTag(name.to_string()))
    }

    /// The trimmed text of the child `name`, empty when there is none.
    fn text_of(&self, name: &str) -> String {
        self.child(name)
            .map(|c| c.text.trim().to_string())
            .unwrap_or_default()
    }

    /// The attribute of local name `name`.
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn texts(&self, name: &str) -> Vec<String> {
        self.all(name).map(|c| c.text.trim().to_string()).collect()
    }

    fn flag(&self, name: &str) -> Result<bool, ParseError> {
        match self.text_of(name).as_str() {
            "" | "false" => Ok(false),
            "true" => Ok(true),
            other => Err(ParseError::InvalidValue {
                element: name.to_string(),
                value: other.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

struct Writer {
    out: String,
    depth: usize,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

impl Writer {
    fn new() -> Self {
        Self {
            out: String::new(),
            depth: 0,
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    fn open(&mut self, name: &str) {
        self.indent();
        self.out.push_str(&format!("<{name}>\n"));
        self.depth += 1;
    }

    fn close(&mut self, name: &str) {
        self.depth -= 1;
        self.indent();
        self.out.push_str(&format!("</{name}>\n"));
    }

    fn leaf(&mut self, name: &str, text: &str) {
        self.indent();
        self.out
            .push_str(&format!("<{name}>{}</{name}>\n", escape(text)));
    }

    fn leaves(&mut self, name: &str, texts: &[String]) {
        for t in texts {
            self.leaf(name, t);
        }
    }

    fn properties(&mut self, name: &str, properties: &[Property]) -> Result<(), ParseError> {
        for p in properties {
            let (type_name, text) = value_to_text(&p.value)?;
            self.open(name);
            self.leaf("name", &p.name);
            self.leaf("type", type_name);
            self.leaf("value", &text);
            self.close(name);
        }
        Ok(())
    }

    /// `<capability>` (§7.6.2.3): name, resource types and satisfier
    /// properties.
    fn capabilities(&mut self, capabilities: &[Capability]) -> Result<(), ParseError> {
        for c in capabilities {
            self.open("capability");
            self.leaf("name", &c.name);
            self.leaves("resourceType", &c.resource_type);
            for p in &c.property {
                self.open("property");
                self.leaf("name", &p.name);
                self.leaf("kind", p.kind.as_str());
                self.leaf("dynamic", if p.dynamic { "true" } else { "false" });
                match (p.kind, &p.value) {
                    (SatisfierPropertyKind::Selection, AnyValue::Seq { element, items }) => {
                        self.leaf("type", type_code_to_text(element)?);
                        for item in items {
                            let (_, text) = value_to_text(item)?;
                            self.leaf("value", &text);
                        }
                    }
                    (SatisfierPropertyKind::Selection, _) => {
                        return Err(ParseError::InvalidValue {
                            element: "value".into(),
                            value: format!("{}: a Selection that is no sequence", p.name),
                        });
                    }
                    (_, value) => {
                        let (type_name, text) = value_to_text(value)?;
                        self.leaf("type", type_name);
                        self.leaf("value", &text);
                    }
                }
                self.close("property");
            }
            self.close("capability");
        }
        Ok(())
    }

    /// `<selectRequirement>` (§15.9.1): name, resource type, properties.
    fn requirements(&mut self, requirements: &[Requirement]) -> Result<(), ParseError> {
        for r in requirements {
            self.open("selectRequirement");
            self.leaf("name", &r.name);
            self.leaf("resourceType", &r.resource_type);
            self.properties("property", &r.property)?;
            self.close("selectRequirement");
        }
        Ok(())
    }

    fn named_values(&mut self, name: &str, values: &BTreeMap<String, String>) {
        for (n, v) in values {
            self.open(name);
            self.leaf("name", n);
            self.leaf("value", v);
            self.close(name);
        }
    }
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

/// `<configProperty>`: name, type and value (Part 3 §7.6.4).
fn read_properties(node: &Node, name: &str) -> Result<Vec<Property>, ParseError> {
    node.all(name)
        .map(|p| {
            p.only(&["name", "type", "value"])?;
            let value = p.child("value").map(|v| v.text.clone()).unwrap_or_default();
            Ok(Property::parse(
                &p.text_of("name"),
                &p.require("type")?.text,
                &value,
            )?)
        })
        .collect()
}

/// `<capability>` (§7.6.2.3).
fn read_capabilities(node: &Node) -> Result<Vec<Capability>, ParseError> {
    let mut out = Vec::new();
    for c in node.all("capability") {
        c.only(&["name", "resourceType", "property"])?;
        let mut property = Vec::new();
        for p in c.all("property") {
            p.only(&["name", "kind", "dynamic", "type", "value"])?;
            let kind_text = p.text_of("kind");
            let kind = SatisfierPropertyKind::from_literal(&kind_text).ok_or(
                ParseError::InvalidValue {
                    element: "kind".into(),
                    value: kind_text,
                },
            )?;
            let type_name = &p.require("type")?.text;
            let mut values = Vec::new();
            for v in p.all("value") {
                values.push(value_from_text(type_name, &v.text)?);
            }
            let value = if kind == SatisfierPropertyKind::Selection {
                AnyValue::Seq {
                    element: type_code_from_text(type_name)?,
                    items: values,
                }
            } else {
                let count = values.len();
                let mut values = values.into_iter();
                match (values.next(), values.next()) {
                    (Some(one), None) => one,
                    _ => {
                        return Err(ParseError::InvalidValue {
                            element: "value".into(),
                            value: format!("{count} values of a {} property", kind.as_str()),
                        });
                    }
                }
            };
            property.push(SatisfierProperty {
                name: p.text_of("name"),
                kind,
                dynamic: p.flag("dynamic")?,
                value,
            });
        }
        out.push(Capability {
            name: c.text_of("name"),
            resource_type: c.texts("resourceType"),
            property,
        });
    }
    Ok(out)
}

/// `<selectRequirement>` (§15.9.1).
fn read_requirements(node: &Node) -> Result<Vec<Requirement>, ParseError> {
    let mut out = Vec::new();
    for r in node.all("selectRequirement") {
        r.only(&["name", "resourceType", "property"])?;
        out.push(Requirement {
            name: r.text_of("name"),
            resource_type: r.text_of("resourceType"),
            property: read_properties(r, "property")?,
        });
    }
    Ok(out)
}

/// The implementation an instance of a plan names: the label written as
/// the text of `<implementation>`, or the reference of Part 3 §7.6.4,
/// `<implementation ref="fragment_impl_1"/>` (or `xmi:idref`), which is
/// the `id` of a plan implementation or, when none has it, a label.
fn instance_implementation(
    block: &Node,
    ids: &BTreeMap<String, String>,
) -> Result<String, ParseError> {
    let Some(node) = block.child("implementation") else {
        return Ok(String::new());
    };
    let text = node.text.trim();
    let Some(reference) = node.attribute("ref").or_else(|| node.attribute("idref")) else {
        return Ok(text.to_string());
    };
    let target = ids.get(reference).map_or(reference, String::as_str);
    if !text.is_empty() && text != target {
        return Err(ParseError::InvalidValue {
            element: "implementation".into(),
            value: format!("{text} (ref {reference})"),
        });
    }
    Ok(target.to_string())
}

fn read_named_values(node: &Node, name: &str) -> Result<BTreeMap<String, String>, ParseError> {
    let mut out = BTreeMap::new();
    for p in node.all(name) {
        p.only(&["name", "value"])?;
        out.insert(p.text_of("name"), p.text_of("value"));
    }
    Ok(out)
}

const IMPLEMENTATION: &[&str] = &[
    "label",
    "UUID",
    "artifact",
    "realizes",
    "execParameter",
    "dependsOn",
];

fn read_implementation(block: &Node) -> Result<ImplementationDescription, ParseError> {
    block.only(IMPLEMENTATION)?;
    let mut depends_on = Vec::new();
    for d in block.all("dependsOn") {
        d.only(&["name", "minVersion"])?;
        depends_on.push(ImplementationDependency {
            name: d.text_of("name"),
            min_version: d.text_of("minVersion"),
        });
    }
    Ok(ImplementationDescription {
        label: block.text_of("label"),
        uuid: block.text_of("UUID"),
        artifacts: block.texts("artifact"),
        realizes: block.text_of("realizes"),
        exec_params: read_named_values(block, "execParameter")?,
        depends_on,
    })
}

fn write_implementation(w: &mut Writer, name: &str, i: &ImplementationDescription) {
    w.open(name);
    w.leaf("label", &i.label);
    w.leaf("UUID", &i.uuid);
    w.leaves("artifact", &i.artifacts);
    w.leaf("realizes", &i.realizes);
    w.named_values("execParameter", &i.exec_params);
    for d in &i.depends_on {
        w.open("dependsOn");
        w.leaf("name", &d.name);
        w.leaf("minVersion", &d.min_version);
        w.close("dependsOn");
    }
    w.close(name);
}

// ---------------------------------------------------------------------------
// Deployment plan
// ---------------------------------------------------------------------------

/// Parses a D&C `<deploymentPlan>` XML into a [`DeploymentPlan`] and
/// validates it.
///
/// # Errors
/// See [`ParseError`].
pub fn parse_plan_xml(input: &str) -> Result<DeploymentPlan, ParseError> {
    let root = parse_root(input, "deploymentPlan")?;
    root.only(&[
        "label",
        "UUID",
        "realizes",
        "implementation",
        "instance",
        "connection",
    ])?;
    let mut plan = DeploymentPlan {
        label: root.text_of("label"),
        uuid: root.text_of("UUID"),
        realizes: root.text_of("realizes"),
        ..DeploymentPlan::default()
    };
    // The XMI ids of the implementations, by which an instance may name
    // its implementation (§7.6.4).
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for block in root.all("implementation") {
        let implementation = read_implementation(block)?;
        if let Some(id) = block.attribute("id")
            && ids
                .insert(id.to_string(), implementation.label.clone())
                .is_some()
        {
            return Err(ParseError::InvalidValue {
                element: "implementation".into(),
                value: format!("id {id} given twice"),
            });
        }
        plan.implementations.push(implementation);
    }
    for block in root.all("instance") {
        block.only(&[
            "name",
            "implementation",
            "node",
            "configProperty",
            "coLocateWith",
        ])?;
        plan.instances.push(InstanceDeploymentDescription {
            name: block.text_of("name"),
            implementation: instance_implementation(block, &ids)?,
            node: block.text_of("node"),
            config_props: read_properties(block, "configProperty")?,
            co_locate_with: block.texts("coLocateWith"),
        });
    }
    for block in root.all("connection") {
        block.only(&[
            "name",
            "sourceInstance",
            "sourcePort",
            "targetInstance",
            "targetPort",
        ])?;
        plan.connections.push(PlanConnection {
            name: block.text_of("name"),
            source_instance: block.text_of("sourceInstance"),
            source_port: block.text_of("sourcePort"),
            target_instance: block.text_of("targetInstance"),
            target_port: block.text_of("targetPort"),
        });
    }
    plan.validate()
        .map_err(|e| ParseError::ValidationFailed(format!("{e}")))?;
    Ok(plan)
}

/// Writes a plan as the XML [`parse_plan_xml`] reads.
///
/// # Errors
/// [`ParseError::Property`] for a configuration value without a text
/// form.
pub fn plan_to_xml(plan: &DeploymentPlan) -> Result<String, ParseError> {
    let mut w = Writer::new();
    w.open("deploymentPlan");
    w.leaf("label", &plan.label);
    w.leaf("UUID", &plan.uuid);
    w.leaf("realizes", &plan.realizes);
    for i in &plan.implementations {
        write_implementation(&mut w, "implementation", i);
    }
    for i in &plan.instances {
        w.open("instance");
        w.leaf("name", &i.name);
        w.leaf("implementation", &i.implementation);
        w.leaf("node", &i.node);
        w.properties("configProperty", &i.config_props)?;
        w.leaves("coLocateWith", &i.co_locate_with);
        w.close("instance");
    }
    for c in &plan.connections {
        w.open("connection");
        w.leaf("name", &c.name);
        w.leaf("sourceInstance", &c.source_instance);
        w.leaf("sourcePort", &c.source_port);
        w.leaf("targetInstance", &c.target_instance);
        w.leaf("targetPort", &c.target_port);
        w.close("connection");
    }
    w.close("deploymentPlan");
    Ok(w.out)
}

// ---------------------------------------------------------------------------
// Component interface description (.ccd)
// ---------------------------------------------------------------------------

fn read_interface(node: &Node) -> Result<ComponentInterfaceDescription, ParseError> {
    node.only(&[
        "label",
        "UUID",
        "specificType",
        "supportedType",
        "idlFile",
        "configProperty",
        "port",
        "property",
        "infoProperty",
    ])?;
    let mut port = Vec::new();
    for p in node.all("port") {
        p.only(&[
            "name",
            "specificType",
            "supportedType",
            "provider",
            "exclusiveProvider",
            "exclusiveUser",
            "optional",
            "kind",
            "templateParam",
        ])?;
        let kind_text = p.text_of("kind");
        let kind =
            CcmComponentPortKind::from_literal(&kind_text).ok_or(ParseError::InvalidValue {
                element: "kind".into(),
                value: kind_text,
            })?;
        port.push(ComponentPortDescription {
            name: p.text_of("name"),
            specific_type: p.text_of("specificType"),
            supported_type: p.texts("supportedType"),
            provider: p.flag("provider")?,
            exclusive_provider: p.flag("exclusiveProvider")?,
            exclusive_user: p.flag("exclusiveUser")?,
            optional: p.flag("optional")?,
            kind,
            template_param: p.texts("templateParam"),
        });
    }
    let mut property = Vec::new();
    for p in node.all("property") {
        p.only(&["name", "type"])?;
        property.push(ComponentPropertyDescription {
            name: p.text_of("name"),
            type_name: p.text_of("type"),
        });
    }
    let interface = ComponentInterfaceDescription {
        label: node.text_of("label"),
        uuid: node.text_of("UUID"),
        specific_type: node.text_of("specificType"),
        supported_type: node.texts("supportedType"),
        idl_file: node.texts("idlFile"),
        config_property: read_properties(node, "configProperty")?,
        port,
        property,
        info_property: read_properties(node, "infoProperty")?,
    };
    interface
        .validate()
        .map_err(|e| ParseError::ValidationFailed(format!("{e}")))?;
    Ok(interface)
}

fn write_interface(
    w: &mut Writer,
    name: &str,
    i: &ComponentInterfaceDescription,
) -> Result<(), ParseError> {
    w.open(name);
    w.leaf("label", &i.label);
    w.leaf("UUID", &i.uuid);
    w.leaf("specificType", &i.specific_type);
    w.leaves("supportedType", &i.supported_type);
    w.leaves("idlFile", &i.idl_file);
    w.properties("configProperty", &i.config_property)?;
    for p in &i.port {
        w.open("port");
        w.leaf("name", &p.name);
        w.leaf("specificType", &p.specific_type);
        w.leaves("supportedType", &p.supported_type);
        w.leaf("provider", if p.provider { "true" } else { "false" });
        w.leaf(
            "exclusiveProvider",
            if p.exclusive_provider {
                "true"
            } else {
                "false"
            },
        );
        w.leaf(
            "exclusiveUser",
            if p.exclusive_user { "true" } else { "false" },
        );
        w.leaf("optional", if p.optional { "true" } else { "false" });
        w.leaf("kind", p.kind.as_str());
        w.leaves("templateParam", &p.template_param);
        w.close("port");
    }
    for p in &i.property {
        w.open("property");
        w.leaf("name", &p.name);
        w.leaf("type", &p.type_name);
        w.close("property");
    }
    w.properties("infoProperty", &i.info_property)?;
    w.close(name);
    Ok(())
}

/// Parses a `<componentInterfaceDescription>` (`.ccd`, Part 3 §15.6.3).
///
/// # Errors
/// See [`ParseError`]; `ValidationFailed` for the rule of §7.6.2.4.
pub fn parse_interface_xml(input: &str) -> Result<ComponentInterfaceDescription, ParseError> {
    read_interface(&parse_root(input, "componentInterfaceDescription")?)
}

/// Writes a `<componentInterfaceDescription>`.
///
/// # Errors
/// [`ParseError::Property`].
pub fn interface_to_xml(interface: &ComponentInterfaceDescription) -> Result<String, ParseError> {
    let mut w = Writer::new();
    write_interface(&mut w, "componentInterfaceDescription", interface)?;
    Ok(w.out)
}

// ---------------------------------------------------------------------------
// Connector package description, assembly description
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 64 — the document depth bounds it.
fn read_connector_package(node: &Node) -> Result<ConnectorPackageDescription, ParseError> {
    node.only(&[
        "label",
        "UUID",
        "realizes",
        "configProperty",
        "implementation",
    ])?;
    let mut implementation = Vec::new();
    for i in node.all("implementation") {
        i.only(&[
            "label",
            "UUID",
            "implements",
            "monolithicImpl",
            "assemblyImpl",
            "configProperty",
            "capability",
        ])?;
        let realization = match (i.child("monolithicImpl"), i.child("assemblyImpl")) {
            (Some(m), None) => {
                m.only(&["primaryArtifact", "execParameter"])?;
                ConnectorImplementation::Monolithic(MonolithicImplementationDescription {
                    primary_artifact: m.texts("primaryArtifact"),
                    exec_parameter: read_named_values(m, "execParameter")?,
                })
            }
            (None, Some(a)) => ConnectorImplementation::Assembly(read_assembly(a)?),
            _ => {
                return Err(ParseError::ExpectedTag(
                    "monolithicImpl or assemblyImpl".into(),
                ));
            }
        };
        implementation.push(ConnectorImplementationDescription {
            label: i.text_of("label"),
            uuid: i.text_of("UUID"),
            implements: read_interface(i.require("implements")?)?,
            implementation: realization,
            config_property: read_properties(i, "configProperty")?,
            capability: read_capabilities(i)?,
        });
    }
    if implementation.is_empty() {
        return Err(ParseError::ExpectedTag("implementation".into()));
    }
    Ok(ConnectorPackageDescription {
        label: node.text_of("label"),
        uuid: node.text_of("UUID"),
        realizes: read_interface(node.require("realizes")?)?,
        config_property: read_properties(node, "configProperty")?,
        implementation,
    })
}

/// zerodds-lint: recursion-depth 64 — the document depth bounds it.
fn write_connector_package(
    w: &mut Writer,
    name: &str,
    p: &ConnectorPackageDescription,
) -> Result<(), ParseError> {
    w.open(name);
    w.leaf("label", &p.label);
    w.leaf("UUID", &p.uuid);
    write_interface(w, "realizes", &p.realizes)?;
    w.properties("configProperty", &p.config_property)?;
    for i in &p.implementation {
        w.open("implementation");
        w.leaf("label", &i.label);
        w.leaf("UUID", &i.uuid);
        write_interface(w, "implements", &i.implements)?;
        match &i.implementation {
            ConnectorImplementation::Monolithic(m) => {
                w.open("monolithicImpl");
                w.leaves("primaryArtifact", &m.primary_artifact);
                w.named_values("execParameter", &m.exec_parameter);
                w.close("monolithicImpl");
            }
            ConnectorImplementation::Assembly(a) => write_assembly(w, "assemblyImpl", a)?,
        }
        w.properties("configProperty", &i.config_property)?;
        w.capabilities(&i.capability)?;
        w.close("implementation");
    }
    w.close(name);
    Ok(())
}

fn read_component_package(node: &Node) -> Result<ComponentPackageDescription, ParseError> {
    node.only(&["label", "UUID", "componentId", "realizes", "implementation"])?;
    let realizes = read_interface(node.require("realizes")?)?;
    let component_id = match node.text_of("componentId") {
        id if id.is_empty() => realizes.specific_type.clone(),
        id => id,
    };
    Ok(ComponentPackageDescription {
        label: node.text_of("label"),
        uuid: node.text_of("UUID"),
        component_id,
        realizes,
        implementations: node
            .all("implementation")
            .map(read_implementation)
            .collect::<Result<_, _>>()?,
    })
}

fn write_component_package(
    w: &mut Writer,
    name: &str,
    p: &ComponentPackageDescription,
) -> Result<(), ParseError> {
    w.open(name);
    w.leaf("label", &p.label);
    w.leaf("UUID", &p.uuid);
    w.leaf("componentId", &p.component_id);
    write_interface(w, "realizes", &p.realizes)?;
    for i in &p.implementations {
        write_implementation(w, "implementation", i);
    }
    w.close(name);
    Ok(())
}

/// zerodds-lint: recursion-depth 64 — the document depth bounds it.
fn read_assembly(node: &Node) -> Result<ComponentAssemblyDescription, ParseError> {
    node.only(&["instance", "connection"])?;
    let mut assembly = ComponentAssemblyDescription::default();
    for i in node.all("instance") {
        i.only(&[
            "name",
            "basePackage",
            "baseConnectorPackage",
            "configProperty",
            "selectRequirement",
        ])?;
        let package = match (i.child("basePackage"), i.child("baseConnectorPackage")) {
            (Some(p), None) => SubcomponentPackage::Component(read_component_package(p)?),
            (None, Some(p)) => SubcomponentPackage::Connector(read_connector_package(p)?),
            _ => {
                return Err(ParseError::ExpectedTag(
                    "basePackage or baseConnectorPackage".into(),
                ));
            }
        };
        assembly
            .instance
            .push(SubcomponentInstantiationDescription {
                name: i.text_of("name"),
                package,
                config_property: read_properties(i, "configProperty")?,
                select_requirement: read_requirements(i)?,
            });
    }
    for c in node.all("connection") {
        c.only(&["name", "connector", "internalEndpoint", "externalEndpoint"])?;
        let connector = match c.child("connector") {
            None => None,
            Some(u) => {
                u.only(&[
                    "baseConnectorPackage",
                    "selectRequirement",
                    "configProperty",
                ])?;
                Some(ConnectorUse {
                    package: read_connector_package(u.require("baseConnectorPackage")?)?,
                    select_requirement: read_requirements(u)?,
                    config_property: read_properties(u, "configProperty")?,
                })
            }
        };
        let mut internal_endpoint = Vec::new();
        for e in c.all("internalEndpoint") {
            e.only(&["instance", "portName"])?;
            internal_endpoint.push(SubcomponentPortEndpoint {
                instance: e.text_of("instance"),
                port_name: e.text_of("portName"),
            });
        }
        let mut external_endpoint = Vec::new();
        for e in c.all("externalEndpoint") {
            e.only(&["portName"])?;
            external_endpoint.push(e.text_of("portName"));
        }
        assembly.connection.push(AssemblyConnectionDescription {
            name: c.text_of("name"),
            connector,
            internal_endpoint,
            external_endpoint,
        });
    }
    Ok(assembly)
}

/// zerodds-lint: recursion-depth 64 — the document depth bounds it.
fn write_assembly(
    w: &mut Writer,
    name: &str,
    a: &ComponentAssemblyDescription,
) -> Result<(), ParseError> {
    w.open(name);
    for i in &a.instance {
        w.open("instance");
        w.leaf("name", &i.name);
        match &i.package {
            SubcomponentPackage::Component(p) => write_component_package(w, "basePackage", p)?,
            SubcomponentPackage::Connector(p) => {
                write_connector_package(w, "baseConnectorPackage", p)?;
            }
        }
        w.properties("configProperty", &i.config_property)?;
        w.requirements(&i.select_requirement)?;
        w.close("instance");
    }
    for c in &a.connection {
        w.open("connection");
        w.leaf("name", &c.name);
        if let Some(u) = &c.connector {
            w.open("connector");
            write_connector_package(w, "baseConnectorPackage", &u.package)?;
            w.requirements(&u.select_requirement)?;
            w.properties("configProperty", &u.config_property)?;
            w.close("connector");
        }
        for e in &c.internal_endpoint {
            w.open("internalEndpoint");
            w.leaf("instance", &e.instance);
            w.leaf("portName", &e.port_name);
            w.close("internalEndpoint");
        }
        for e in &c.external_endpoint {
            w.open("externalEndpoint");
            w.leaf("portName", e);
            w.close("externalEndpoint");
        }
        w.close("connection");
    }
    w.close(name);
    Ok(())
}

/// Parses a `<connectorPackageDescription>` (Part 3 §7.6.2.2).
///
/// # Errors
/// See [`ParseError`].
pub fn parse_connector_package_xml(input: &str) -> Result<ConnectorPackageDescription, ParseError> {
    read_connector_package(&parse_root(input, "connectorPackageDescription")?)
}

/// Writes a `<connectorPackageDescription>`.
///
/// # Errors
/// [`ParseError::Property`].
pub fn connector_package_to_xml(
    package: &ConnectorPackageDescription,
) -> Result<String, ParseError> {
    let mut w = Writer::new();
    write_connector_package(&mut w, "connectorPackageDescription", package)?;
    Ok(w.out)
}

/// Parses a `<componentAssemblyDescription>` (Part 3 §7.6.2.5).
///
/// # Errors
/// See [`ParseError`].
pub fn parse_assembly_xml(input: &str) -> Result<ComponentAssemblyDescription, ParseError> {
    read_assembly(&parse_root(input, "componentAssemblyDescription")?)
}

/// Writes a `<componentAssemblyDescription>`.
///
/// # Errors
/// [`ParseError::Property`].
pub fn assembly_to_xml(assembly: &ComponentAssemblyDescription) -> Result<String, ParseError> {
    let mut w = Writer::new();
    write_assembly(&mut w, "componentAssemblyDescription", assembly)?;
    Ok(w.out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
<deploymentPlan>
  <label>Plan1</label>
  <UUID>uuid-1</UUID>
  <realizes>RootApp</realizes>
  <implementation>
    <label>EchoImpl</label>
    <UUID>echo-uuid</UUID>
    <artifact>lib/echo.so</artifact>
    <realizes>IDL:demo/Echo:1.0</realizes>
  </implementation>
  <instance>
    <name>echo1</name>
    <implementation>EchoImpl</implementation>
    <node>Node1</node>
  </instance>
</deploymentPlan>
"#;

    #[test]
    fn parses_minimal_plan() {
        let plan = parse_plan_xml(SAMPLE).unwrap();
        assert_eq!(plan.label, "Plan1");
        assert_eq!(plan.uuid, "uuid-1");
        assert_eq!(plan.implementations.len(), 1);
        assert_eq!(
            plan.implementations[0].artifacts,
            alloc::vec!["lib/echo.so".to_string()]
        );
        assert_eq!(plan.instances.len(), 1);
        assert_eq!(plan.instances[0].node, "Node1");
    }

    #[test]
    fn parses_multiple_artifacts() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <implementation>
    <label>I1</label>
    <UUID>i1</UUID>
    <artifact>a.so</artifact>
    <artifact>b.so</artifact>
    <realizes>IDL:X:1.0</realizes>
  </implementation>
</deploymentPlan>
"#;
        let plan = parse_plan_xml(xml).unwrap();
        assert_eq!(plan.implementations[0].artifacts.len(), 2);
    }

    #[test]
    fn validation_fails_on_unknown_impl_reference() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <instance>
    <name>x</name>
    <implementation>NoSuchImpl</implementation>
    <node>N</node>
  </instance>
</deploymentPlan>
"#;
        let err = parse_plan_xml(xml).unwrap_err();
        match err {
            ParseError::ValidationFailed(_) => {}
            e => panic!("unexpected error variant: {e}"),
        }
    }

    #[test]
    fn missing_root_tag_returns_expected_tag() {
        let err = parse_plan_xml("<empty/>").unwrap_err();
        assert_eq!(err, ParseError::ExpectedTag("deploymentPlan".into()));
    }

    #[test]
    fn co_locate_with_parsed() {
        let xml = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <implementation>
    <label>I</label>
    <UUID>i</UUID>
    <artifact>a.so</artifact>
    <realizes>IDL:X:1.0</realizes>
  </implementation>
  <instance>
    <name>a</name>
    <implementation>I</implementation>
    <node>N</node>
  </instance>
  <instance>
    <name>b</name>
    <implementation>I</implementation>
    <node>N</node>
    <coLocateWith>a</coLocateWith>
  </instance>
</deploymentPlan>
"#;
        let plan = parse_plan_xml(xml).unwrap();
        assert_eq!(
            plan.instances[1].co_locate_with,
            alloc::vec!["a".to_string()]
        );
    }

    // -- CORBA 3.3 Part 3 §7.6 ---------------------------------------------

    use crate::connector::tests::{
        CONNECTOR, PORT_TYPE, assembly, connector_interface, monolithic_package, nodes, port_types,
        two_implementations,
    };
    use crate::connector::{Capability, Requirement, SatisfierProperty, SatisfierPropertyKind};
    use zerodds_corba_rt::any::AnyValue;

    /// The instance of the example of §7.6.4 as the specification writes
    /// it, `<implementation ref="fragment_impl_1"/>` included, in a plan
    /// whose implementation has that id and carries the entry point.
    const FRAGMENT_PLAN: &str = r#"
<deploymentPlan>
  <label>P</label>
  <UUID>u</UUID>
  <implementation id="fragment_impl_1">
    <label>Data_Cnx_fragment</label>
    <UUID>f</UUID>
    <artifact>lib/fragment.so</artifact>
    <realizes>IDL:Demo/Data_Cnx:1.0</realizes>
    <execParameter><name>home factory</name><value>create_Data_Cnx</value></execParameter>
  </implementation>
<!-- ********************************************** -->
<!-- *************** INSTANCES ******************** -->
<!-- ********************************************** -->
<!-- Instance for fragment_instance_1 -->
<instance id="fragment_instance_1">
     <name>fragment_instance_1</name>
     <node>node1</node>
     <implementation ref="fragment_impl_1"/>
     <configProperty>
          <name>mcast_addr</name>
          <type>string</type>
          <value>224.1.1.1</value>
     </configProperty>
     <configProperty>
          <name>mcast_port</name>
          <type>unsigned short</type>
          <value>31337</value>
     </configProperty>
     <configProperty>
          <name>msg_size</name>
          <type>unsigned long</type>
          <value>50</value>
     </configProperty>
</instance>
</deploymentPlan>
"#;

    #[test]
    fn corba_p3_7_6_4_plan_xml_carries_typed_config_properties_and_exec_parameters() {
        let plan = parse_plan_xml(FRAGMENT_PLAN).unwrap();
        // `ref="fragment_impl_1"` names the implementation of that id.
        assert_eq!(plan.instances[0].implementation, "Data_Cnx_fragment");
        assert_eq!(
            plan.implementations[0]
                .exec_params
                .get(crate::connector::HOME_FACTORY)
                .map(String::as_str),
            Some("create_Data_Cnx")
        );
        let config: Vec<(&str, &AnyValue)> = plan.instances[0]
            .config_props
            .iter()
            .map(|p| (p.name.as_str(), &p.value))
            .collect();
        assert_eq!(
            config,
            [
                ("mcast_addr", &AnyValue::Str("224.1.1.1".into())),
                ("mcast_port", &AnyValue::UShort(31337)),
                ("msg_size", &AnyValue::ULong(50)),
            ]
        );
        let written = plan_to_xml(&plan).unwrap();
        assert!(written.contains("<type>unsigned short</type>"));
        assert_eq!(parse_plan_xml(&written).unwrap(), plan);
        // A value that is not of its type, an unknown type, an element the
        // plan does not define.
        let bad = FRAGMENT_PLAN.replace("<value>31337</value>", "<value>-1</value>");
        assert!(matches!(parse_plan_xml(&bad), Err(ParseError::Property(_))));
        let bad = FRAGMENT_PLAN.replace("<type>unsigned long</type>", "<type>Demo::T</type>");
        assert!(matches!(parse_plan_xml(&bad), Err(ParseError::Property(_))));
        let bad = FRAGMENT_PLAN.replace("<node>node1</node>", "<node>node1</node><nodeHint/>");
        assert!(matches!(
            parse_plan_xml(&bad),
            Err(ParseError::UnexpectedElement { .. })
        ));
        assert!(matches!(
            parse_plan_xml("<deploymentPlan><label>"),
            Err(ParseError::InvalidXml(_))
        ));
    }

    #[test]
    fn corba_p3_7_6_2_4_extended_port_kinds_and_template_params_in_xml() {
        let mut interface = connector_interface();
        interface.port[0].template_param = alloc::vec!["IDL:Demo/Sample:1.0".into()];
        interface.idl_file = alloc::vec!["Data_Cnx.idl".into()];
        let xml = interface_to_xml(&interface).unwrap();
        assert!(xml.contains("<kind>ExtendedMirrorPort</kind>"));
        assert!(xml.contains("<templateParam>IDL:Demo/Sample:1.0</templateParam>"));
        assert!(xml.contains(&alloc::format!("<specificType>{PORT_TYPE}</specificType>")));
        assert_eq!(parse_interface_xml(&xml).unwrap(), interface);
        // §7.6.2.4: template parameters only on an extended port.
        let facet = xml.replace("<kind>ExtendedMirrorPort</kind>", "<kind>Facet</kind>");
        assert!(matches!(
            parse_interface_xml(&facet),
            Err(ParseError::ValidationFailed(_))
        ));
        let unknown = xml.replace("<kind>ExtendedMirrorPort</kind>", "<kind>Mirror</kind>");
        assert!(matches!(
            parse_interface_xml(&unknown),
            Err(ParseError::InvalidValue { .. })
        ));
    }

    #[test]
    fn corba_p3_7_6_2_connector_package_and_assembly_round_trip_through_xml() {
        let package = monolithic_package();
        let xml = connector_package_to_xml(&package).unwrap();
        assert!(xml.contains("<monolithicImpl>"));
        assert!(xml.contains("<name>home factory</name>"));
        assert_eq!(parse_connector_package_xml(&xml).unwrap(), package);
        // A package without implementation is refused (1..*).
        let start = xml.find("  <implementation>").unwrap();
        let end = xml.rfind("</implementation>").unwrap() + "</implementation>\n".len();
        let mut without = xml.clone();
        without.replace_range(start..end, "");
        assert!(parse_connector_package_xml(&without).is_err());
        // The assembly with a connection realized by the connector, and
        // the plan flattened from what was read.
        let a = assembly();
        let xml = assembly_to_xml(&a).unwrap();
        assert!(xml.contains("<connector>"));
        assert!(xml.contains("<baseConnectorPackage>"));
        let read = parse_assembly_xml(&xml).unwrap();
        assert_eq!(read, a);
        let plan =
            crate::connector::flatten_assembly(&read, "App", "u", &nodes(), &port_types()).unwrap();
        assert!(plan.instances.iter().any(|i| i.name == "link_consumer1_p"));
        assert!(plan.implementations.iter().any(|i| i.realizes == CONNECTOR));
        let plan_xml = plan_to_xml(&plan).unwrap();
        assert_eq!(parse_plan_xml(&plan_xml).unwrap(), plan);
    }

    /// §7.6.4: an instance names its implementation by reference, the way
    /// the example writes it, or by label as text. A reference names the
    /// implementation of that id, else of that label; text and reference
    /// that disagree, a reference to nothing and an id given twice are
    /// refused.
    #[test]
    fn corba_p3_7_6_4_an_instance_names_its_implementation_by_reference_or_by_label() {
        let by_label = FRAGMENT_PLAN.replace(
            r#"<implementation ref="fragment_impl_1"/>"#,
            "<implementation>Data_Cnx_fragment</implementation>",
        );
        assert_eq!(
            parse_plan_xml(&by_label).unwrap(),
            parse_plan_xml(FRAGMENT_PLAN).unwrap()
        );
        let xmi = FRAGMENT_PLAN
            .replace(r#"<implementation id="#, r#"<implementation xmlns:xmi="http://www.omg.org/XMI" xmi:id="#)
            .replace(
                r#"<implementation ref="fragment_impl_1"/>"#,
                r#"<implementation xmlns:xmi="http://www.omg.org/XMI" xmi:idref="fragment_impl_1"/>"#,
            );
        assert_eq!(
            parse_plan_xml(&xmi).unwrap().instances[0].implementation,
            "Data_Cnx_fragment"
        );
        // Without an id, a reference names the implementation of that label.
        let no_id = FRAGMENT_PLAN
            .replace(
                r#"<implementation id="fragment_impl_1">"#,
                "<implementation>",
            )
            .replace(
                r#"<implementation ref="fragment_impl_1"/>"#,
                r#"<implementation ref="Data_Cnx_fragment"/>"#,
            );
        assert_eq!(
            parse_plan_xml(&no_id).unwrap().instances[0].implementation,
            "Data_Cnx_fragment"
        );
        let nowhere = FRAGMENT_PLAN.replace(r#"ref="fragment_impl_1""#, r#"ref="other""#);
        assert!(matches!(
            parse_plan_xml(&nowhere),
            Err(ParseError::ValidationFailed(_))
        ));
        let disagree = FRAGMENT_PLAN.replace(
            r#"<implementation ref="fragment_impl_1"/>"#,
            r#"<implementation ref="fragment_impl_1">other</implementation>"#,
        );
        assert!(matches!(
            parse_plan_xml(&disagree),
            Err(ParseError::InvalidValue { .. })
        ));
        let twice = FRAGMENT_PLAN.replace(
            "</implementation>\n",
            "</implementation>\n  <implementation id=\"fragment_impl_1\"><label>X</label></implementation>\n",
        );
        assert!(matches!(
            parse_plan_xml(&twice),
            Err(ParseError::InvalidValue { .. })
        ));
    }

    /// §7.6.2.3 and §15.9.1: the capabilities of an implementation, every
    /// satisfier kind among them, and the selection requirements of a
    /// connector use and of an instance survive writing and reading, and
    /// the plan of what was read takes the selected implementation.
    #[test]
    fn corba_p3_7_6_2_3_capabilities_and_selection_requirements_round_trip_through_xml() {
        let mut package = two_implementations();
        package.implementation[1].capability.push(Capability {
            name: "limits".into(),
            resource_type: alloc::vec!["Transport".into(), "Memory".into()],
            property: alloc::vec![
                SatisfierProperty {
                    name: "buffers".into(),
                    kind: SatisfierPropertyKind::Quantity,
                    dynamic: true,
                    value: AnyValue::ULong(4),
                },
                SatisfierProperty {
                    name: "memory".into(),
                    kind: SatisfierPropertyKind::Capacity,
                    dynamic: false,
                    value: AnyValue::Double(2.5),
                },
                SatisfierProperty {
                    name: "latency".into(),
                    kind: SatisfierPropertyKind::Minimum,
                    dynamic: false,
                    value: AnyValue::Long(10),
                },
                SatisfierProperty {
                    name: "codec".into(),
                    kind: SatisfierPropertyKind::Selection,
                    dynamic: false,
                    value: AnyValue::Seq {
                        element: zerodds_corba_rt::TypeCode::String(0),
                        items: alloc::vec![
                            AnyValue::Str("cdr".into()),
                            AnyValue::Str("xcdr2".into())
                        ],
                    },
                },
            ],
        });
        let xml = connector_package_to_xml(&package).unwrap();
        assert!(xml.contains("<capability>"));
        assert!(xml.contains("<kind>Selection</kind>"));
        assert_eq!(parse_connector_package_xml(&xml).unwrap(), package);

        let requirement = Requirement {
            name: "fast".into(),
            resource_type: "Transport".into(),
            property: alloc::vec![Property::string("kind", "shm")],
        };
        let mut a = assembly();
        if let Some(used) = a.connection[0].connector.as_mut() {
            used.package = package;
            used.select_requirement = alloc::vec![requirement.clone()];
        }
        a.instance[1].select_requirement = alloc::vec![requirement];
        let xml = assembly_to_xml(&a).unwrap();
        assert_eq!(xml.matches("<selectRequirement>").count(), 2);
        let read = parse_assembly_xml(&xml).unwrap();
        assert_eq!(read, a);
        // The instance's requirement cannot be met by a component
        // implementation; without it, the connector use selects shm_impl.
        let mut read = read;
        read.instance[1].select_requirement.clear();
        let plan =
            crate::connector::flatten_assembly(&read, "App", "u", &nodes(), &port_types()).unwrap();
        assert!(
            plan.instances
                .iter()
                .filter(|i| i.name.starts_with("link_"))
                .all(|i| i.implementation == "shm_impl")
        );

        // A kind the schema does not have, two values where one belongs.
        let xml = connector_package_to_xml(&two_implementations()).unwrap();
        let unknown = xml.replacen("<kind>Attribute</kind>", "<kind>Mostly</kind>", 1);
        assert!(matches!(
            parse_connector_package_xml(&unknown),
            Err(ParseError::InvalidValue { .. })
        ));
        let two = xml.replacen(
            "<value>udp</value>",
            "<value>udp</value><value>tcp</value>",
            1,
        );
        assert!(matches!(
            parse_connector_package_xml(&two),
            Err(ParseError::InvalidValue { .. })
        ));
    }
}
